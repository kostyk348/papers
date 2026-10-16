//! MCP stdio client: one server process addressed over stdin/stdout with
//! newline-delimited JSON-RPC 2.0, plus the append-only call log the harness writes.
//!
//! Port of the `MCPProcess`/`CallLog`/`Session` classes of `code/run_protocol.py`
//! (standard library only: `std::process`, `std::thread`, `std::sync::mpsc`).

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::json::{self, Json};
use crate::util::{basename, ensure_dir, round3, time_time};

/// A failed JSON-RPC round trip. The harness distinguishes a timeout (a budget the
/// server exceeded) from every other fault (malformed framing, dead process).
#[derive(Debug)]
pub enum ReqError {
    Timeout(String),
    Other(String),
}

impl ReqError {
    pub fn message(&self) -> &str {
        match self {
            ReqError::Timeout(m) | ReqError::Other(m) => m,
        }
    }
}

impl std::fmt::Display for ReqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl From<ReqError> for String {
    fn from(e: ReqError) -> String {
        e.message().to_string()
    }
}

// ---------------------------------------------------------------------------- call log

/// Append-only JSONL record of everything the harness sent and received.
pub struct CallLog {
    writer: Option<std::io::BufWriter<std::fs::File>>,
}

impl CallLog {
    pub fn new(path: &str) -> Result<CallLog, String> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let parent = parent.to_string_lossy().to_string();
            if !parent.is_empty() {
                ensure_dir(&parent)?;
            }
        }
        let file = std::fs::File::create(path).map_err(|e| format!("cannot write {path}: {e}"))?;
        Ok(CallLog {
            writer: Some(std::io::BufWriter::new(file)),
        })
    }

    pub fn write(&mut self, obj: &Json) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.write_all(json::dump_compact(obj, false).as_bytes());
            let _ = w.write_all(b"\r\n");
            let _ = w.flush();
        }
    }

    /// `{"ts": round(time.time(), 3), "kind": "event", **obj}`
    pub fn event(&mut self, obj: Vec<(String, Json)>) {
        let mut members = vec![
            ("ts".to_string(), Json::Float(round3(time_time()))),
            ("kind".to_string(), Json::Str("event".to_string())),
        ];
        for (k, v) in obj {
            json::set_member(&mut members, &k, v);
        }
        self.write(&Json::Obj(members));
    }

    pub fn close(&mut self) {
        if let Some(mut w) = self.writer.take() {
            let _ = w.flush();
        }
    }
}

// ------------------------------------------------------------------------- MCP process

#[derive(Clone, Copy)]
enum StreamKind {
    Out,
    Err,
}

/// One server process, addressed over stdin/stdout with newline-delimited JSON-RPC.
pub struct McpProcess {
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    pub client_info: Json,
    pub protocol_version: String,
    pub timeout: f64,
    pub log: Option<Rc<RefCell<CallLog>>>,
    pub init_result: Json,
    pub stderr_tail: Vec<String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<Receiver<(StreamKind, Option<String>)>>,
    next_id: u64,
    pid: u32,
}

impl McpProcess {
    pub fn new(
        name: &str,
        argv: Vec<String>,
        cwd: &str,
        env: Vec<(String, String)>,
        client_info: Json,
        protocol_version: &str,
        timeout: f64,
        log: Option<Rc<RefCell<CallLog>>>,
    ) -> McpProcess {
        McpProcess {
            name: name.to_string(),
            argv,
            cwd: cwd.to_string(),
            env,
            client_info,
            protocol_version: protocol_version.to_string(),
            timeout,
            log,
            init_result: Json::Null,
            stderr_tail: Vec::new(),
            child: None,
            stdin: None,
            rx: None,
            next_id: 0,
            pid: 0,
        }
    }

    fn event(&mut self, kind: &str, payload: Vec<(String, Json)>) {
        if let Some(log) = self.log.clone() {
            let mut members = vec![
                ("event".to_string(), Json::Str(kind.to_string())),
                ("server".to_string(), Json::Str(self.name.clone())),
            ];
            for (k, v) in payload {
                json::set_member(&mut members, &k, v);
            }
            log.borrow_mut().event(members);
        }
    }

    pub fn start(&mut self) -> Result<Json, ReqError> {
        let mut cmd = Command::new(&self.argv[0]);
        cmd.args(&self.argv[1..]);
        cmd.current_dir(&self.cwd);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd.env("PYTHONIOENCODING", "utf-8");
        cmd.env("PYTHONUNBUFFERED", "1");
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| ReqError::Other(format!("cannot spawn {}: {e}", self.argv.join(" "))))?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ReqError::Other("no stdout pipe".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ReqError::Other("no stderr pipe".to_string()))?;
        let (tx, rx) = channel();
        let tx_out = tx.clone();
        std::thread::spawn(move || read_lines(stdout, StreamKind::Out, tx_out));
        std::thread::spawn(move || read_lines(stderr, StreamKind::Err, tx));
        let pid = child.id();
        self.child = Some(child);
        self.stdin = stdin;
        self.rx = Some(rx);
        self.next_id = 0;
        self.pid = pid;
        self.stderr_tail.clear();
        let init = self.handshake()?;
        self.init_result = init.clone();
        self.event(
            "spawn",
            vec![
                (
                    "argv".to_string(),
                    Json::Arr(self.argv.iter().cloned().map(Json::Str).collect()),
                ),
                ("cwd".to_string(), Json::Str(self.cwd.clone())),
                ("pid".to_string(), Json::Int(pid as i64)),
                (
                    "server_info".to_string(),
                    init.get("serverInfo").cloned().unwrap_or(Json::Null),
                ),
            ],
        );
        Ok(init)
    }

    fn handshake(&mut self) -> Result<Json, ReqError> {
        let params = Json::Obj(vec![
            (
                "protocolVersion".to_string(),
                Json::Str(self.protocol_version.clone()),
            ),
            ("capabilities".to_string(), Json::Obj(vec![])),
            ("clientInfo".to_string(), self.client_info.clone()),
        ]);
        let resp = self.request("initialize", Some(params), None)?;
        let result = resp
            .get("result")
            .filter(|v| matches!(v, Json::Obj(_)))
            .cloned()
            .unwrap_or(Json::Obj(vec![]));
        self.notify("notifications/initialized", Some(Json::Obj(vec![])))?;
        Ok(result)
    }

    pub fn stop(&mut self) {
        let mut child = match self.child.take() {
            Some(c) => c,
            None => return,
        };
        self.stdin.take(); // closes the child's stdin
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut status: Option<std::process::ExitStatus> = None;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let status: Option<std::process::ExitStatus> = match status {
            Some(s) => Some(s),
            None => {
                let _ = child.kill();
                child.wait().ok()
            }
        };
        self.rx = None;
        let code = status.and_then(|s| s.code()).map(|c| Json::Int(c as i64));
        let tail: Vec<Json> = self
            .stderr_tail
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(|s| Json::Str(s.clone()))
            .collect();
        self.event(
            "exit",
            vec![
                ("pid".to_string(), Json::Int(self.pid as i64)),
                ("returncode".to_string(), code.unwrap_or(Json::Null)),
                ("stderr_tail".to_string(), Json::Arr(tail)),
            ],
        );
    }

    pub fn restart(&mut self) -> Result<Json, ReqError> {
        self.stop();
        self.start()
    }

    fn send(&mut self, msg: &Json) -> Result<(), ReqError> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| ReqError::Other("server is not running".to_string()))?;
        let line = json::dump_compact(msg, false);
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| ReqError::Other(format!("cannot write to the server: {e}")))
    }

    pub fn notify(&mut self, method: &str, params: Option<Json>) -> Result<(), ReqError> {
        let msg = Json::Obj(vec![
            ("jsonrpc".to_string(), Json::Str("2.0".to_string())),
            ("method".to_string(), Json::Str(method.to_string())),
            (
                "params".to_string(),
                params.unwrap_or_else(|| Json::Obj(vec![])),
            ),
        ]);
        self.send(&msg)
    }

    pub fn request(
        &mut self,
        method: &str,
        params: Option<Json>,
        timeout: Option<f64>,
    ) -> Result<Json, ReqError> {
        self.next_id += 1;
        let rid = self.next_id;
        let msg = Json::Obj(vec![
            ("jsonrpc".to_string(), Json::Str("2.0".to_string())),
            ("id".to_string(), Json::Int(rid as i64)),
            ("method".to_string(), Json::Str(method.to_string())),
            (
                "params".to_string(),
                params.unwrap_or_else(|| Json::Obj(vec![])),
            ),
        ]);
        self.send(&msg)?;
        let limit = timeout.unwrap_or(self.timeout);
        let deadline = Instant::now() + Duration::from_secs_f64(limit.max(0.0));
        let timeout_message = format!("{method} timed out after {limit:.1}s");
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ReqError::Timeout(timeout_message));
            }
            let rx = match self.rx.as_ref() {
                Some(rx) => rx,
                None => return Err(ReqError::Other("server is not running".to_string())),
            };
            let received = rx.recv_timeout(remaining);
            let (kind, line) = match received {
                Ok(v) => v,
                Err(RecvTimeoutError::Timeout) => return Err(ReqError::Timeout(timeout_message)),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(ReqError::Other(format!(
                        "server closed the channel while waiting for {method}"
                    )))
                }
            };
            match kind {
                StreamKind::Err => {
                    if let Some(line) = line {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let cut: String = trimmed.chars().take(400).collect();
                            self.stderr_tail.push(cut);
                            if self.stderr_tail.len() > 20 {
                                let drop = self.stderr_tail.len() - 20;
                                self.stderr_tail.drain(0..drop);
                            }
                        }
                    }
                    continue;
                }
                StreamKind::Out => {
                    let line = match line {
                        Some(l) => l,
                        None => {
                            return Err(ReqError::Other(format!(
                                "server closed stdout while waiting for {method}"
                            )))
                        }
                    };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let parsed = match json::parse(&line) {
                        Ok(v) => v,
                        Err(_) => {
                            // server logging on stdout: recorded, not fatal
                            let cut: String = line.chars().take(200).collect();
                            self.event("stdout_noise", vec![("line".to_string(), Json::Str(cut))]);
                            continue;
                        }
                    };
                    if parsed.get("id").and_then(|v| v.as_i64()) == Some(rid as i64) {
                        return Ok(parsed);
                    }
                }
            }
        }
    }
}

fn read_lines<R: Read + Send + 'static>(
    stream: R,
    kind: StreamKind,
    tx: std::sync::mpsc::Sender<(StreamKind, Option<String>)>,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut buf: Vec<u8> = Vec::new();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                while buf.last() == Some(&b'\n') || buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).to_string();
                if tx.send((kind, Some(line))).is_err() {
                    return;
                }
            }
            Err(_) => break,
        }
    }
    let _ = tx.send((kind, None));
}

// ------------------------------------------------------------------------------ session

/// One server process bound to one fresh data directory.
pub struct Session {
    pub spec: Json,
    pub name: String,
    pub python: String,
    pub server_root: String,
    pub pythonpath: Option<String>,
    pub protocol_version: String,
    pub timeout: f64,
    pub client_info: Json,
    pub argv: Vec<String>,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    pub proc: Option<McpProcess>,
    pub tools: Vec<Json>,
    pub init_result: Json,
    /// Directory this session's process was launched against (for the snapshot probe).
    pub data_dir: String,
}

pub fn join_path(a: &str, b: &str) -> String {
    let sep = if cfg!(windows) { '\\' } else { '/' };
    if a.is_empty() {
        return b.to_string();
    }
    if a.ends_with('/') || a.ends_with('\\') {
        format!("{a}{b}")
    } else {
        format!("{a}{sep}{b}")
    }
}

impl Session {
    pub fn new(
        spec: &Json,
        python: &str,
        server_root: &str,
        pythonpath: Option<&str>,
        protocol_version: &str,
        timeout: f64,
    ) -> Session {
        Session {
            spec: spec.clone(),
            name: spec
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            python: python.to_string(),
            server_root: server_root.to_string(),
            pythonpath: pythonpath.map(|s| s.to_string()),
            protocol_version: protocol_version.to_string(),
            timeout,
            client_info: Json::Obj(vec![
                (
                    "name".to_string(),
                    Json::Str("agent-memory-protocol-harness".to_string()),
                ),
                ("version".to_string(), Json::Str("0.1".to_string())),
            ]),
            argv: Vec::new(),
            cwd: String::new(),
            env: Vec::new(),
            proc: None,
            tools: Vec::new(),
            init_result: Json::Obj(vec![]),
            data_dir: String::new(),
        }
    }

    fn launch_spec(&self) -> &Json {
        self.spec.get("launch").unwrap_or(&Json::Null)
    }

    fn sub(&self, value: &str, data_dir: &str) -> String {
        value
            .replace("{python}", &self.python)
            .replace("{data_dir}", data_dir)
            .replace("{server_root}", &self.server_root)
    }

    pub fn argv_for(&self, data_dir: &str) -> Vec<String> {
        self.launch_spec()
            .get("argv")
            .and_then(|v| v.as_arr())
            .map(|items| {
                items
                    .iter()
                    .map(|i| self.sub(i.as_str().unwrap_or(""), data_dir))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn env_for(&self, data_dir: &str) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = vec![
            ("USERPROFILE".to_string(), data_dir.to_string()),
            ("HOME".to_string(), data_dir.to_string()),
        ];
        if let Some(pp) = &self.pythonpath {
            env.push(("PYTHONPATH".to_string(), pp.clone()));
        }
        if let Some(extra) = self.launch_spec().get("env").and_then(|v| v.as_obj()) {
            for (k, v) in extra {
                env.push((k.clone(), self.sub(v.as_str().unwrap_or(""), data_dir)));
            }
        }
        env
    }

    pub fn launch(
        &mut self,
        data_dir: &str,
        log: Option<Rc<RefCell<CallLog>>>,
    ) -> Result<Vec<Json>, ReqError> {
        let rel_cwd = self
            .launch_spec()
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.cwd = join_path(&self.server_root, &rel_cwd);
        self.argv = self.argv_for(data_dir);
        self.env = self.env_for(data_dir);
        self.data_dir = data_dir.to_string();
        let mut proc = McpProcess::new(
            &self.name,
            self.argv.clone(),
            &self.cwd,
            self.env.clone(),
            self.client_info.clone(),
            &self.protocol_version,
            self.timeout,
            log.clone(),
        );
        let init = proc.start()?;
        self.init_result = init.clone();
        if let Some(log) = log.clone() {
            log.borrow_mut().event(vec![
                ("event".to_string(), Json::Str("init_result".to_string())),
                ("result".to_string(), init.clone()),
            ]);
        }
        self.proc = Some(proc);
        let listing = self.request("tools/list", None, None)?;
        self.tools = listing
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_arr())
            .cloned()
            .unwrap_or_default();
        if let Some(log) = log {
            let mut names: Vec<String> = self
                .tools
                .iter()
                .map(|t| {
                    t.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            names.sort();
            log.borrow_mut().event(vec![
                ("event".to_string(), Json::Str("tools".to_string())),
                ("count".to_string(), Json::Int(self.tools.len() as i64)),
                (
                    "names".to_string(),
                    Json::Arr(names.into_iter().map(Json::Str).collect()),
                ),
            ]);
        }
        Ok(self.tools.clone())
    }

    pub fn request(
        &mut self,
        method: &str,
        params: Option<Json>,
        timeout: Option<f64>,
    ) -> Result<Json, ReqError> {
        let timeout = timeout.unwrap_or(self.timeout);
        match self.proc.as_mut() {
            Some(p) => p.request(method, params, Some(timeout)),
            None => Err(ReqError::Other("server is not running".to_string())),
        }
    }

    pub fn restart(&mut self) -> Result<Json, ReqError> {
        match self.proc.as_mut() {
            Some(p) => p.restart(),
            None => Err(ReqError::Other("server is not running".to_string())),
        }
    }

    pub fn stop(&mut self) {
        if let Some(p) = self.proc.as_mut() {
            p.stop();
        }
    }
}

/// The raw log file names the harness uses, in the order it registers them.
pub fn snapshot_dir_name(src: &str) -> String {
    format!("snapshot-{}", basename(src))
}

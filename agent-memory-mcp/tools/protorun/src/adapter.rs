//! Adapters: map the protocol's operations onto each server's tool surface.
//!
//! Port of the `Adapter`/`LiquidAdapter`/`SintMemoryAdapter`/`OpResult`/`Repetition`
//! classes of `code/run_protocol.py`. The call sequence is part of the measurement, so
//! this follows the Python version call for call.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use crate::json::{self, Json};
use crate::mcp::{CallLog, ReqError, Session};
use crate::util::{py_len, round3, time_time, OrderedMap};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Liquid,
    SintMemory,
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub count: i64,
    pub examples: Vec<String>,
}

/// One repetition: drives every family against one server process.
pub struct Rep {
    pub calls: usize,
    pub max_calls: usize,
    pub timeout: f64,
    pub seed: i64,
    pub family: String,
    pub failures: OrderedMap<OrderedMap<Failure>>,
    pub started: Instant,
    /// The per-repetition process log: the snapshot re-open probe writes its calls there,
    /// not into the family log (this mirrors `rep.log` in the Python harness).
    pub process_log: Option<Rc<RefCell<CallLog>>>,
}

impl Rep {
    pub fn new(seed: i64, timeout: f64, max_calls: usize) -> Rep {
        Rep {
            calls: 0,
            max_calls,
            timeout,
            seed,
            family: String::new(),
            failures: OrderedMap::new(),
            started: Instant::now(),
            process_log: None,
        }
    }

    pub fn select(&mut self, family: &str) {
        self.family = family.to_string();
    }

    pub fn failure(&mut self, code: &str, example: &str) {
        if !self.failures.contains(&self.family) {
            self.failures.set(&self.family.clone(), OrderedMap::new());
        }
        let fam = self
            .failures
            .get_mut(&self.family)
            .expect("family bucket just created");
        if !fam.contains(code) {
            fam.set(
                code,
                Failure {
                    count: 0,
                    examples: Vec::new(),
                },
            );
        }
        let entry = fam.get_mut(code).expect("code bucket just created");
        entry.count += 1;
        if entry.examples.len() < 5 {
            let cut: String = example.chars().take(300).collect();
            entry.examples.push(cut);
        }
    }
}

pub struct OpResult {
    pub tool: String,
    pub args: Json,
    pub request: Option<Json>,
    pub response: Option<Json>,
    pub latency_ms: f64,
    pub error_code: Option<String>,
    pub payload: Option<Json>,
    pub text_chars: usize,
    pub records: Vec<Json>,
}

impl OpResult {
    fn nop(tool: &str, args: Json) -> OpResult {
        OpResult {
            tool: tool.to_string(),
            args,
            request: None,
            response: None,
            latency_ms: 0.0,
            error_code: None,
            payload: None,
            text_chars: 0,
            records: Vec::new(),
        }
    }
}

pub struct Adapter {
    pub kind: AdapterKind,
    pub session: Session,
    pub tool_specs: Vec<Json>,
    pub tools: HashSet<String>,
    pub rep: Rc<RefCell<Rep>>,
    pub log: Option<Rc<RefCell<CallLog>>>,
    status_by_content: OrderedMap<String>,
    id_by_content: OrderedMap<Json>,
    ids: Vec<Json>,
    register_counts: OrderedMap<i64>,
    pub revisions: i64,
}

impl Adapter {
    pub fn new(
        kind: AdapterKind,
        session: Session,
        tool_specs: Vec<Json>,
        rep: Rc<RefCell<Rep>>,
    ) -> Adapter {
        let tools: HashSet<String> = tool_specs
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        Adapter {
            kind,
            session,
            tool_specs,
            tools,
            rep,
            log: None,
            status_by_content: OrderedMap::new(),
            id_by_content: OrderedMap::new(),
            ids: Vec::new(),
            register_counts: OrderedMap::new(),
            revisions: 0,
        }
    }

    pub fn record_content_field(&self) -> &'static str {
        match self.kind {
            AdapterKind::Liquid => "claim",
            AdapterKind::SintMemory => "content",
        }
    }

    pub fn record_source_field(&self) -> &'static str {
        "source"
    }

    pub fn record_time_field(&self) -> &'static str {
        match self.kind {
            AdapterKind::Liquid => "created",
            AdapterKind::SintMemory => "timestamp",
        }
    }

    pub fn supports_revise(&self) -> bool {
        self.kind == AdapterKind::Liquid
    }

    pub fn supports_ranked_search(&self) -> bool {
        self.kind == AdapterKind::SintMemory
    }

    pub fn content_of(&self, record: &Json) -> Option<String> {
        record
            .get(self.record_content_field())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn has(&self, tool: &str) -> bool {
        self.tools.contains(tool)
    }

    fn log_call(&mut self, res: &OpResult) {
        let log = match self.log.clone() {
            Some(l) => l,
            None => return,
        };
        let (family, seed) = {
            let rep = self.rep.borrow();
            (rep.family.clone(), rep.seed)
        };
        let name = self.session.name.clone();
        let members: Vec<(String, Json)> = vec![
            ("ts".to_string(), Json::Float(round3(time_time()))),
            ("kind".to_string(), Json::Str("call".to_string())),
            ("server".to_string(), Json::Str(name.to_string())),
            ("family".to_string(), Json::Str(family)),
            ("repetition".to_string(), Json::Int(seed)),
            ("tool".to_string(), Json::Str(res.tool.clone())),
            ("arguments".to_string(), res.args.clone()),
            (
                "latency_ms".to_string(),
                Json::Float(round3(res.latency_ms)),
            ),
            (
                "error_code".to_string(),
                match &res.error_code {
                    Some(c) => Json::Str(c.clone()),
                    None => Json::Null,
                },
            ),
            (
                "request".to_string(),
                res.request.clone().unwrap_or(Json::Null),
            ),
            (
                "response".to_string(),
                res.response.clone().unwrap_or(Json::Null),
            ),
            (
                "parsed_records".to_string(),
                Json::Int(res.records.len() as i64),
            ),
        ];
        log.borrow_mut().write(&Json::Obj(members));
    }

    pub fn call(&mut self, tool: &str, args: Json, timeout: Option<f64>) -> OpResult {
        let mut res = OpResult::nop(tool, args);
        if !self.has(tool) {
            res.error_code = Some("not_implemented".to_string());
            self.rep.borrow_mut().failure(
                "not_implemented",
                &format!("tool {tool} absent from tools/list"),
            );
            self.log_call(&res);
            return res;
        }
        let max_calls = self.rep.borrow().max_calls;
        if self.rep.borrow().calls >= max_calls {
            res.error_code = Some("budget_exhausted".to_string());
            self.rep.borrow_mut().failure(
                "budget_exhausted",
                &format!("max_calls={max_calls} reached"),
            );
            self.log_call(&res);
            return res;
        }
        self.rep.borrow_mut().calls += 1;
        let params = Json::Obj(vec![
            ("name".to_string(), Json::Str(tool.to_string())),
            ("arguments".to_string(), res.args.clone()),
        ]);
        res.request = Some(Json::Obj(vec![
            ("method".to_string(), Json::Str("tools/call".to_string())),
            ("params".to_string(), params.clone()),
        ]));
        let t0 = Instant::now();
        let limit = timeout.unwrap_or(self.rep.borrow().timeout);
        let answered = self
            .session
            .request("tools/call", Some(params), Some(limit));
        match answered {
            Ok(response) => res.response = Some(response),
            Err(ReqError::Timeout(msg)) => {
                res.error_code = Some("timeout".to_string());
                res.response = Some(Json::Obj(vec![(
                    "harness_error".to_string(),
                    Json::Str(msg),
                )]));
            }
            Err(ReqError::Other(msg)) => {
                res.error_code = Some("protocol_error".to_string());
                res.response = Some(Json::Obj(vec![(
                    "harness_error".to_string(),
                    Json::Str(msg),
                )]));
            }
        }
        res.latency_ms = round3(t0.elapsed().as_secs_f64() * 1000.0);
        if let Some(response) = &res.response {
            if response.get("error").is_some() {
                res.error_code = Some("server_error".to_string());
            }
            if let Some(result) = response.get("result") {
                if result.get("isError").map(|v| v.truthy()).unwrap_or(false) {
                    res.error_code = Some("server_error".to_string());
                }
            }
        }
        res.payload = extract_payload(&res.response);
        res.text_chars = payload_chars(&res.response);
        res.records = as_records(&res.payload);
        if let Some(code) = res.error_code.clone() {
            if code == "server_error" || code == "timeout" || code == "protocol_error" {
                let args_txt: String = json::dump_compact(&res.args, false)
                    .chars()
                    .take(120)
                    .collect();
                let resp_txt: String =
                    json::dump_compact(res.response.as_ref().unwrap_or(&Json::Null), false)
                        .chars()
                        .take(200)
                        .collect();
                self.rep
                    .borrow_mut()
                    .failure(&code, &format!("{tool}({args_txt}) -> {resp_txt}"));
            }
        }
        self.log_call(&res);
        res
    }

    // ---------------------------------------------------------------- operations

    pub fn write(&mut self, fact: &Json) -> OpResult {
        let content = fact
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let source = fact
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let confidence = fact.get("confidence").cloned().unwrap_or(Json::Null);
        let tags: Vec<Json> = fact
            .get("tags")
            .and_then(|v| v.as_arr())
            .cloned()
            .unwrap_or_default();
        match self.kind {
            AdapterKind::Liquid => {
                let status = fact
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let domain = tags
                    .first()
                    .and_then(|t| t.as_str())
                    .unwrap_or("protocol")
                    .to_string();
                let res = self.call(
                    "liquid_belief",
                    Json::Obj(vec![
                        ("claim".to_string(), Json::Str(content.clone())),
                        ("status".to_string(), Json::Str(status.clone())),
                        ("confidence".to_string(), confidence),
                        ("source".to_string(), Json::Str(source)),
                        ("domain".to_string(), Json::Str(domain)),
                        ("tags".to_string(), Json::Arr(tags)),
                    ]),
                    None,
                );
                self.status_by_content.set(&content, status);
                if let Some(first) = res.records.first() {
                    let id = first.get("id").cloned().unwrap_or(Json::Null);
                    self.id_by_content.set(&content, id);
                }
                res
            }
            AdapterKind::SintMemory => {
                let register = fact
                    .get("register")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let project = fact
                    .get("project")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let res = self.call(
                    "memory_write_block",
                    Json::Obj(vec![
                        ("register".to_string(), Json::Str(register.clone())),
                        ("content".to_string(), Json::Str(content.clone())),
                        ("project".to_string(), Json::Str(project)),
                        ("source".to_string(), Json::Str(source)),
                        ("confidence".to_string(), confidence),
                        ("tags".to_string(), Json::Arr(tags)),
                        ("links".to_string(), Json::Arr(vec![])),
                    ]),
                    None,
                );
                let mut payload = res.payload.clone();
                if let Some(Json::Arr(items)) = &payload {
                    payload = items.first().cloned();
                }
                let bid = payload
                    .as_ref()
                    .and_then(|p| p.get("id"))
                    .cloned()
                    .unwrap_or(Json::Null);
                if !bid.is_null() {
                    self.ids.push(bid.clone());
                    self.id_by_content.set(&content, bid);
                }
                self.register_counts.bump(&register);
                res
            }
        }
    }

    pub fn read_content(&mut self, content: &str, _k: usize) -> OpResult {
        match self.kind {
            AdapterKind::Liquid => self.call(
                "liquid_query",
                Json::Obj(vec![(
                    "claim_query".to_string(),
                    Json::Str(content.to_string()),
                )]),
                None,
            ),
            AdapterKind::SintMemory => {
                let bid = self.id_by_content.get(content).cloned();
                match bid {
                    Some(id) if !id.is_null() => self.call(
                        "memory_read_block",
                        Json::Obj(vec![("block_id".to_string(), id)]),
                        None,
                    ),
                    _ => {
                        let mut res = OpResult::nop("memory_read_block", Json::Obj(vec![]));
                        res.error_code = Some("not_implemented".to_string());
                        self.rep.borrow_mut().failure(
                            "not_implemented",
                            "memory_read_block needs a block id, and this content was never written",
                        );
                        res
                    }
                }
            }
        }
    }

    pub fn search(&mut self, query: &str, k: usize) -> OpResult {
        match self.kind {
            // The only read operation in this server is an exact-match lookup; `k` is not
            // expressible, so the harness passes the query through unchanged.
            AdapterKind::Liquid => self.call(
                "liquid_query",
                Json::Obj(vec![(
                    "claim_query".to_string(),
                    Json::Str(query.to_string()),
                )]),
                None,
            ),
            AdapterKind::SintMemory => self.call(
                "memory_search",
                Json::Obj(vec![
                    ("query".to_string(), Json::Str(query.to_string())),
                    ("limit".to_string(), Json::Int(k as i64)),
                ]),
                None,
            ),
        }
    }

    pub fn read_probe(&mut self, content: &str, k: usize) -> OpResult {
        self.search(content, k)
    }

    pub fn stats(&mut self) -> OpResult {
        match self.kind {
            AdapterKind::Liquid => self.call("liquid_stats", Json::Obj(vec![]), None),
            AdapterKind::SintMemory => self.call("memory_stats", Json::Obj(vec![]), None),
        }
    }

    pub fn revise(
        &mut self,
        content: &str,
        new_status: &str,
        new_confidence: f64,
        reason: &str,
    ) -> OpResult {
        match self.kind {
            AdapterKind::Liquid => {
                let cid = self.id_by_content.get(content).cloned();
                let cid = match cid {
                    Some(c) if !c.is_null() => c,
                    _ => {
                        let mut res = OpResult::nop("liquid_revise", Json::Obj(vec![]));
                        res.error_code = Some("not_implemented".to_string());
                        self.rep.borrow_mut().failure(
                            "not_implemented",
                            "liquid_revise needs a claim id that the store never returned",
                        );
                        return res;
                    }
                };
                let res = self.call(
                    "liquid_revise",
                    Json::Obj(vec![
                        ("claim_id".to_string(), cid),
                        ("new_status".to_string(), Json::Str(new_status.to_string())),
                        ("new_confidence".to_string(), Json::Float(new_confidence)),
                        ("reason".to_string(), Json::Str(reason.to_string())),
                    ]),
                    None,
                );
                self.status_by_content.set(content, new_status.to_string());
                self.revisions += 1;
                res
            }
            AdapterKind::SintMemory => {
                let mut res = OpResult::nop("memory_revise", Json::Obj(vec![]));
                res.error_code = Some("not_implemented".to_string());
                self.rep.borrow_mut().failure(
                    "not_implemented",
                    "the block store is append-only: no operation rewrites or retires a block",
                );
                res
            }
        }
    }

    pub fn history(&mut self, content: &str) -> OpResult {
        let bid = self.id_by_content.get(content).cloned();
        let tool = match self.kind {
            AdapterKind::Liquid => "liquid_history",
            AdapterKind::SintMemory => "memory_trace_provenance",
        };
        match bid {
            Some(id) if !id.is_null() => {
                let key = match self.kind {
                    AdapterKind::Liquid => "claim_id",
                    AdapterKind::SintMemory => "block_id",
                };
                self.call(tool, Json::Obj(vec![(key.to_string(), id)]), None)
            }
            _ => {
                let mut res = OpResult::nop(tool, Json::Obj(vec![]));
                res.error_code = Some("not_implemented".to_string());
                let note = match self.kind {
                    AdapterKind::Liquid => {
                        "liquid_history needs a claim id that the store never returned"
                    }
                    AdapterKind::SintMemory => "no block id for this content",
                };
                self.rep.borrow_mut().failure("not_implemented", note);
                res
            }
        }
    }

    pub fn anomalies(&mut self) -> OpResult {
        match self.kind {
            AdapterKind::Liquid => self.call("liquid_stats", Json::Obj(vec![]), None),
            AdapterKind::SintMemory => self.call("memory_verify_chain", Json::Obj(vec![]), None),
        }
    }

    pub fn maintenance(&mut self) -> OpResult {
        match self.kind {
            AdapterKind::Liquid => {
                let mut res = OpResult::nop("liquid_maintenance", Json::Obj(vec![]));
                res.error_code = Some("not_implemented".to_string());
                self.rep.borrow_mut().failure(
                    "not_implemented",
                    "no consolidation or maintenance operation in the tool surface",
                );
                res
            }
            AdapterKind::SintMemory => self.call("memory_apply_decay", Json::Obj(vec![]), None),
        }
    }

    // ------------------------------------------------------------- store ground truth

    pub fn expected_store(&self) -> Json {
        match self.kind {
            AdapterKind::Liquid => {
                let mut by_status: OrderedMap<i64> = OrderedMap::new();
                for status in self.status_by_content.values() {
                    by_status.bump(status);
                }
                Json::Obj(vec![
                    (
                        "total_beliefs".to_string(),
                        Json::Int(self.status_by_content.len() as i64),
                    ),
                    (
                        "by_status".to_string(),
                        Json::Obj(
                            by_status
                                .iter()
                                .map(|(k, v)| (k.clone(), Json::Int(*v)))
                                .collect(),
                        ),
                    ),
                ])
            }
            AdapterKind::SintMemory => Json::Obj(vec![
                ("total_blocks".to_string(), Json::Int(self.ids.len() as i64)),
                (
                    "blocks_by_register".to_string(),
                    Json::Obj(
                        self.register_counts
                            .iter()
                            .map(|(k, v)| (k.clone(), Json::Int(*v)))
                            .collect(),
                    ),
                ),
            ]),
        }
    }

    pub fn server_store(&mut self) -> Json {
        let payload = self.stats().payload;
        let payload = match payload {
            Some(p) if matches!(p, Json::Obj(_)) => p,
            _ => return Json::Obj(vec![]),
        };
        match self.kind {
            AdapterKind::Liquid => Json::Obj(vec![
                (
                    "total_beliefs".to_string(),
                    payload.get("total_beliefs").cloned().unwrap_or(Json::Null),
                ),
                (
                    "by_status".to_string(),
                    payload.get("by_status").cloned().unwrap_or(Json::Null),
                ),
            ]),
            AdapterKind::SintMemory => Json::Obj(vec![
                (
                    "total_blocks".to_string(),
                    payload.get("total_blocks").cloned().unwrap_or(Json::Null),
                ),
                (
                    "blocks_by_register".to_string(),
                    payload
                        .get("blocks_by_register")
                        .cloned()
                        .unwrap_or(Json::Null),
                ),
            ]),
        }
    }

    /// `(ratio, note)` — the note is part of the metric note in the report.
    pub fn monitoring(&mut self) -> (Option<f64>, String) {
        let exp = self.expected_store();
        let got = self.server_store();
        let exp_empty = exp.as_obj().map(|o| o.is_empty()).unwrap_or(true);
        let got_empty = got.as_obj().map(|o| o.is_empty()).unwrap_or(true);
        if exp_empty || got_empty {
            return (None, "no store self-report available".to_string());
        }
        let (mut compared, mut matched) = (0i64, 0i64);
        if let Some(members) = exp.as_obj() {
            for (key, val) in members {
                match got.get(key) {
                    Some(g) => {
                        compared += 1;
                        if g == val {
                            matched += 1;
                        }
                    }
                    None => continue,
                }
            }
        }
        let ratio = if compared != 0 {
            Some(matched as f64 / compared as f64)
        } else {
            None
        };
        (ratio, format!("compared {compared} self-reported counters"))
    }

    pub fn restart_session(&mut self) -> Result<Json, ReqError> {
        self.session.restart()
    }

    pub fn tool_specs_principal_fraction(&self) -> Option<f64> {
        principal_scoped_fraction(&self.tool_specs)
    }
}

/// Fraction of advertised tools whose input schema can express who is asking.
pub fn principal_scoped_fraction(tool_specs: &[Json]) -> Option<f64> {
    if tool_specs.is_empty() {
        return None;
    }
    let callers = [
        "caller",
        "principal",
        "user",
        "user_id",
        "actor",
        "identity",
        "subject",
        "scope",
    ];
    let mut scoped = 0usize;
    for t in tool_specs {
        let props = t
            .get("inputSchema")
            .and_then(|s| s.get("properties"))
            .and_then(|p| p.as_obj())
            .cloned()
            .unwrap_or_default();
        let hit = props.iter().any(|(k, _)| {
            let lower = k.to_lowercase();
            callers.contains(&lower.as_str())
        });
        if hit {
            scoped += 1;
        }
    }
    Some(scoped as f64 / tool_specs.len() as f64)
}

// ------------------------------------------------------------- payload interpretation

/// Parse the tool result.
///
/// FastMCP sends one `content` item per record when a tool returns a list, so a
/// multi-record answer arrives as several JSON texts, not as one JSON array. Reading
/// only the first item silently truncates every ranked result to one record.
pub fn extract_payload(response: &Option<Json>) -> Option<Json> {
    let response = response.as_ref()?;
    let result = response.get("result")?;
    let content = result.get("content")?.as_arr()?;
    let mut parsed: Vec<Json> = Vec::new();
    for c in content {
        if c.get("type").and_then(|t| t.as_str()) != Some("text") {
            continue;
        }
        let raw = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
        parsed.push(json::parse(raw).unwrap_or_else(|_| Json::Str(raw.to_string())));
    }
    if parsed.is_empty() {
        return None;
    }
    if parsed.len() == 1 {
        Some(parsed.into_iter().next().unwrap())
    } else {
        Some(Json::Arr(parsed))
    }
}

pub fn payload_chars(response: &Option<Json>) -> usize {
    let response = match response.as_ref() {
        Some(r) => r,
        None => return 0,
    };
    let content = match response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_arr())
    {
        Some(c) => c,
        None => return 0,
    };
    content
        .iter()
        .map(|c| py_len(c.get("text").and_then(|t| t.as_str()).unwrap_or("")))
        .sum()
}

/// Normalise a tool payload into a list of record dicts.
pub fn as_records(payload: &Option<Json>) -> Vec<Json> {
    match payload {
        Some(Json::Arr(items)) => items
            .iter()
            .filter(|p| matches!(p, Json::Obj(_)) && p.get("error").is_none())
            .cloned()
            .collect(),
        Some(obj @ Json::Obj(_)) => {
            if obj.get("error").is_some() {
                vec![]
            } else {
                vec![obj.clone()]
            }
        }
        _ => vec![],
    }
}

/// Record count a store reports about itself, whatever its stats tool calls it.
pub fn store_total(payload: &Option<Json>) -> Option<i64> {
    let obj = payload.as_ref()?.as_obj()?;
    for key in ["total_beliefs", "total_blocks", "total_claims", "total"] {
        for (k, v) in obj {
            if k == key {
                if let Some(i) = v.as_i64() {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// `hashlib.sha256(json.dumps(records, sort_keys=True, ensure_ascii=False)).hexdigest()[:16]`
pub fn digest(records: &[Json]) -> Option<String> {
    if records.is_empty() {
        return None;
    }
    let text = json::dump_compact(&Json::Arr(records.to_vec()), true);
    Some(crate::util::sha256_hex(text.as_bytes())[..16].to_string())
}

/// Digest of the *answer* a probe receives, not of the whole record.
pub fn answer_digest(records: &[Json], adapter: &Adapter) -> Option<String> {
    if records.is_empty() {
        return None;
    }
    let record = &records[0];
    let answer = Json::Obj(vec![
        (
            "content".to_string(),
            match adapter.content_of(record) {
                Some(c) => Json::Str(c),
                None => Json::Null,
            },
        ),
        (
            "source".to_string(),
            record
                .get(adapter.record_source_field())
                .cloned()
                .unwrap_or(Json::Null),
        ),
        (
            "confidence".to_string(),
            record.get("confidence").cloned().unwrap_or(Json::Null),
        ),
    ]);
    let text = json::dump_compact(&answer, true);
    Some(crate::util::sha256_hex(text.as_bytes())[..16].to_string())
}

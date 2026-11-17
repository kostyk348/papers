//! `papertool measure` -- run the experiment plan, keep the raw logs, aggregate the numbers.
//!
//! Every number that reaches the paper comes from here. The command:
//!   * spawns each subject exactly as `plan.toml` says (no shells, no string building),
//!   * writes the child's stdout/stderr to `evidence/raw/<run>/<seed>_<rep>.out|err`,
//!   * polls the child's peak working set while it runs (Windows `K32GetProcessMemoryInfo`,
//!     Linux `/proc/<pid>/status`), because an O(1)-memory claim is a memory claim,
//!   * turns stdout into metrics (whole-stdout JSON, or explicit `literal -> nth number`
//!     extractions for subjects that only print prose),
//!   * aggregates over seeds/repeats and writes `data/measurements.json` with the raw
//!     value list kept alongside the statistics, so any table can be recomputed by hand.

use crate::json::{self, Json};
use crate::plan::{self, Mode, Plan, Run};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Options<'a> {
    pub root: &'a str,
    pub plan: &'a str,
    pub out: &'a str,
    pub logs: &'a str,
    pub only: Option<String>,
    pub check: bool,
    pub derive_only: bool,
}

#[derive(Clone, Debug)]
enum Val {
    Num(f64),
    Bool(bool),
    Text(String),
}

#[derive(Default)]
struct Record {
    seed: i64,
    rep: u32,
    elapsed_s: f64,
    peak_ws_bytes: f64,
    metrics: Vec<(String, Val)>,
    stdout_bytes: usize,
    exit_code: i32,
    timed_out: bool,
}

pub fn run(opts: &Options) -> i32 {
    match run_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("papertool measure: {e}");
            2
        }
    }
}

fn run_inner(opts: &Options) -> Result<i32, String> {
    let plan_path = resolve(opts.root, opts.plan);
    let plan: Plan = plan::load(&plan_path.to_string_lossy())?;
    if opts.check {
        return check_freshness(opts, &plan);
    }
    if opts.derive_only {
        return refresh_derives(opts, &plan);
    }
    let out_path = resolve(opts.root, opts.out);
    let logs_dir = resolve(opts.root, opts.logs);
    std::fs::create_dir_all(&logs_dir).map_err(|e| format!("mkdir {}: {e}", logs_dir.display()))?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }

    let mut runs_json: Vec<Json> = Vec::new();
    let mut failures = 0usize;
    for run in &plan.runs {
        if let Some(only) = &opts.only {
            if &run.id != only && &run.group != only {
                continue;
            }
        }
        let mut records: Vec<Record> = Vec::new();
        let log_dir = logs_dir.join(&run.id);
        std::fs::create_dir_all(&log_dir).map_err(|e| format!("mkdir {}: {e}", log_dir.display()))?;
        for seed in &run.seeds {
            for rep in 0..run.repeats {
                let rec = execute(opts.root, run, *seed, rep, &log_dir)?;
                if rec.timed_out {
                    eprintln!(
                        "[measure] {} seed {seed} rep {rep}: TIMEOUT after {}s",
                        run.id, run.timeout_s
                    );
                    failures += 1;
                } else if rec.exit_code != 0 {
                    // A non-zero exit is a measurement too (the subjects use it to report a
                    // failed check), so it is recorded and reported rather than fatal.
                    eprintln!(
                        "[measure] {} seed {seed} rep {rep}: exit {}",
                        run.id, rec.exit_code
                    );
                }
                records.push(rec);
            }
        }
        println!(
            "[measure] {:<28} seeds {:>3}  n {:>3}  peak {:.1} MB",
            run.id,
            run.seeds.len(),
            records.len(),
            records.iter().map(|r| r.peak_ws_bytes).fold(0.0, f64::max) / 1_048_576.0
        );
        runs_json.push(aggregate(run, &records));
    }
    // `--only` re-measures a subset; the other runs' measurements are kept, so a partial
    // re-run cannot silently shrink the artifact.
    if opts.only.is_some() {
        if let Ok(existing) = std::fs::read_to_string(&out_path) {
            if let Ok(doc) = json::parse(&existing) {
                if let Some(Json::Arr(old)) = doc.get("runs") {
                    let fresh: Vec<String> = runs_json
                        .iter()
                        .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                        .collect();
                    let mut merged: Vec<Json> = old
                        .iter()
                        .filter(|r| {
                            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let derived = r.get("mode").and_then(|v| v.as_str()) == Some("derived");
                            !derived && !fresh.iter().any(|f| f == id)
                        })
                        .cloned()
                        .collect();
                    merged.extend(runs_json.clone());
                    runs_json = merged;
                    println!(
                        "[measure] merged: {} measurement(s) in total",
                        runs_json.len()
                    );
                }
            }
        }
    }

    // Derived quantities: formulas over the aggregates, so a ratio quoted in the prose has
    // a provenance line too. Later derives may reference earlier ones.
    let mut values = crate::derive::collect_values(&runs_json);
    for d in &plan.derives {
        match crate::derive::evaluate(&d.expr, &values) {
            Ok(v) => {
                values.insert(format!("{}.value", d.id), v);
                println!("[measure] {:<28} derived {:.6} (n=1, from means)", d.id, v);
                runs_json.push(derived_run(d, v));
            }
            Err(e) => return Err(format!("derive '{}': {e}", d.id)),
        }
    }

    let mut doc = Json::Obj(Vec::new());
    doc.set("tool", Json::Str("papertool measure".to_string()));
    doc.set("version", Json::Str(env!("CARGO_PKG_VERSION").to_string()));
    doc.set("generated_utc", Json::Str(now_utc()));
    doc.set("plan", Json::Str(opts.plan.to_string()));
    let env: Vec<Json> = plan
        .meta
        .iter()
        .map(|(k, v)| {
            let mut o = Json::Obj(Vec::new());
            o.set("key", Json::Str(k.clone()));
            o.set("value", Json::Str(v.clone()));
            o
        })
        .collect();
    doc.set("environment", Json::Arr(env));
    doc.set("runs", Json::Arr(runs_json));
    let text = json::dump(&doc, 2, false) + "\n";
    std::fs::write(&out_path, text).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    println!("[measure] wrote {}", out_path.display());
    Ok(if failures > 0 { 1 } else { 0 })
}

fn resolve(root: &str, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(root).join(p)
    }
}

fn execute(root: &str, run: &Run, seed: i64, rep: u32, log_dir: &Path) -> Result<Record, String> {
    let mut argv: Vec<String> = run.cmd.clone();
    if let Some(flag) = &run.seed_flag {
        argv.push(flag.clone());
        argv.push(seed.to_string());
    }
    let cwd = resolve(root, &run.cwd);
    let out_file = log_dir.join(format!("seed{seed}_rep{rep}.out"));
    let err_file = log_dir.join(format!("seed{seed}_rep{rep}.err"));
    let stdout = std::fs::File::create(&out_file).map_err(|e| format!("create {:?}: {e}", out_file))?;
    let stderr = std::fs::File::create(&err_file).map_err(|e| format!("create {:?}: {e}", err_file))?;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    for (k, v) in &run.env {
        cmd.env(k, v);
    }
    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn {} in {}: {e}", argv.join(" "), cwd.display()))?;
    let mut peak: f64 = 0.0;
    let mut timed_out = false;
    let status;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                status = Some(st);
                break;
            }
            Ok(None) => {}
            Err(e) => return Err(format!("wait {}: {e}", run.id)),
        }
        if let Some(bytes) = peak_ws(&child) {
            if bytes > peak {
                peak = bytes;
            }
        }
        if started.elapsed().as_secs() >= run.timeout_s {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            status = None;
            break;
        }
        std::thread::sleep(Duration::from_millis(12));
    }
    let elapsed = started.elapsed().as_secs_f64();
    let exit_code = status.and_then(|s| s.code()).unwrap_or(-1);
    let stdout_text = std::fs::read_to_string(&out_file).unwrap_or_default();
    let mut metrics: Vec<(String, Val)> = Vec::new();
    match run.mode {
        Mode::Json => {
            if !timed_out {
                match json::parse(&stdout_text) {
                    Ok(doc) => flatten("", &doc, &mut metrics),
                    Err(e) => {
                        if exit_code == 0 {
                            return Err(format!(
                                "{}: stdout is not JSON ({e}); see {}",
                                run.id,
                                out_file.display()
                            ));
                        }
                    }
                }
            }
        }
        Mode::Capture => {}
    }
    for spec in &run.extract {
        if let Some(v) = nth_number_after(&stdout_text, &spec.literal, spec.nth) {
            metrics.retain(|(n, _)| n != &spec.name);
            metrics.push((spec.name.clone(), Val::Num(v)));
        }
    }
    if run.mode == Mode::Capture || !run.extract.is_empty() {
        metrics.push((
            "stdout_sha256".to_string(),
            Val::Text(sha256_hex(stdout_text.as_bytes())),
        ));
    }
    Ok(Record {
        seed,
        rep,
        elapsed_s: elapsed,
        peak_ws_bytes: peak,
        metrics,
        stdout_bytes: stdout_text.len(),
        exit_code,
        timed_out,
    })
}

fn flatten(prefix: &str, value: &Json, out: &mut Vec<(String, Val)>) {
    match value {
        Json::Obj(pairs) => {
            for (k, v) in pairs {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, out);
            }
        }
        Json::Arr(items) => {
            for (i, v) in items.iter().enumerate() {
                let key = format!("{prefix}[{i}]");
                flatten(&key, v, out);
            }
        }
        Json::Int(i) => out.push((prefix.to_string(), Val::Num(*i as f64))),
        Json::Float(f) => out.push((prefix.to_string(), Val::Num(*f))),
        Json::Bool(b) => out.push((prefix.to_string(), Val::Bool(*b))),
        _ => {}
    }
}

fn nth_number_after(text: &str, literal: &str, nth: usize) -> Option<f64> {
    let mut seen = 0usize;
    for line in text.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find(literal) {
            rest = &rest[pos + literal.len()..];
            for tok in number_tokens(rest) {
                seen += 1;
                if seen == nth {
                    return tok.parse::<f64>().ok();
                }
            }
        }
    }
    None
}

fn number_tokens(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let starts = c.is_ascii_digit()
            || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit());
        if starts {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_ascii_digit() || chars[j] == '.')
                && !(chars[j] == '.' && (j + 1 >= chars.len() || !chars[j + 1].is_ascii_digit()))
            {
                j += 1;
            }
            out.push(chars[i..j].iter().collect());
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn refresh_derives(opts: &Options, plan: &Plan) -> Result<i32, String> {
    let out_path = resolve(opts.root, opts.out);
    let text = std::fs::read_to_string(&out_path)
        .map_err(|e| format!("{}: {e} (run `papertool measure` first)", out_path.display()))?;
    let mut doc = json::parse(&text)?;
    let mut runs: Vec<Json> = match doc.get("runs") {
        Some(Json::Arr(a)) => a
            .iter()
            .filter(|r| r.get("mode").and_then(|v| v.as_str()) != Some("derived"))
            .cloned()
            .collect(),
        _ => return Err("measurements.json has no runs".to_string()),
    };
    let mut values = crate::derive::collect_values(&runs);
    for d in &plan.derives {
        let v = crate::derive::evaluate(&d.expr, &values)
            .map_err(|e| format!("derive '{}': {e}", d.id))?;
        values.insert(format!("{}.value", d.id), v);
        println!("[measure] {:<28} derived {:.6} (n=1, from means)", d.id, v);
        runs.push(derived_run(d, v));
    }
    doc.set("runs", Json::Arr(runs));
    doc.set("derives_refreshed_utc", Json::Str(now_utc()));
    let out = json::dump(&doc, 2, false) + "\n";
    std::fs::write(&out_path, out).map_err(|e| format!("write {}: {e}", out_path.display()))?;
    println!("[measure] refreshed derived values in {}", out_path.display());
    Ok(0)
}

fn derived_run(d: &plan::Derive, value: f64) -> Json {
    let mut metric = Json::Obj(Vec::new());
    metric.set("name", Json::Str("value".to_string()));
    metric.set("kind", Json::Str("num".to_string()));
    if !d.unit.is_empty() {
        metric.set("unit", Json::Str(d.unit.clone()));
    }
    metric.set("n", Json::Int(1));
    metric.set("mean", Json::Float(value));
    metric.set("sd", Json::Float(0.0));
    metric.set("min", Json::Float(value));
    metric.set("max", Json::Float(value));
    metric.set("values", Json::Arr(vec![Json::Float(value)]));
    let mut doc = Json::Obj(Vec::new());
    doc.set("id", Json::Str(d.id.clone()));
    doc.set("group", Json::Str(d.group.clone()));
    doc.set("x", Json::Str(d.x.clone()));
    doc.set("note", Json::Str(d.note.clone()));
    doc.set("mode", Json::Str("derived".to_string()));
    doc.set("expr", Json::Str(d.expr.clone()));
    doc.set("n", Json::Int(1));
    doc.set("metrics", Json::Arr(vec![metric]));
    doc
}

fn aggregate(run: &Run, records: &[Record]) -> Json {
    let mut names: Vec<String> = Vec::new();
    for r in records {
        for (name, _) in &r.metrics {
            if !names.iter().any(|n| n == name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    let mut metrics: Vec<Json> = Vec::new();
    for name in names {
        let vals: Vec<&Val> = records
            .iter()
            .filter_map(|r| r.metrics.iter().find(|(n, _)| n == &name).map(|(_, v)| v))
            .collect();
        let is_num = vals.iter().all(|v| matches!(v, Val::Num(_)));
        let is_bool = vals.iter().all(|v| matches!(v, Val::Bool(_)));
        let mut m = Json::Obj(Vec::new());
        m.set("name", Json::Str(name.clone()));
        if let Some(unit) = run.units.get(&name) {
            m.set("unit", Json::Str(unit.clone()));
        }
        if is_num {
            let xs: Vec<f64> = vals
                .iter()
                .map(|v| match v {
                    Val::Num(x) => *x,
                    _ => 0.0,
                })
                .collect();
            let n = xs.len();
            let mean = xs.iter().sum::<f64>() / n as f64;
            let sd = if n > 1 {
                let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n as f64 - 1.0);
                var.sqrt()
            } else {
                0.0
            };
            let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            m.set("kind", Json::Str("num".to_string()));
            m.set("n", Json::Int(n as i64));
            m.set("mean", Json::Float(mean));
            m.set("sd", Json::Float(sd));
            m.set("min", Json::Float(min));
            m.set("max", Json::Float(max));
            m.set("values", Json::Arr(xs.iter().map(|x| Json::Float(*x)).collect()));
        } else if is_bool {
            let xs: Vec<bool> = vals
                .iter()
                .map(|v| match v {
                    Val::Bool(b) => *b,
                    _ => false,
                })
                .collect();
            m.set("kind", Json::Str("bool".to_string()));
            m.set("n", Json::Int(xs.len() as i64));
            m.set("true_count", Json::Int(xs.iter().filter(|b| **b).count() as i64));
            m.set("values", Json::Arr(xs.iter().map(|b| Json::Bool(*b)).collect()));
        } else {
            let mut uniq: Vec<String> = Vec::new();
            for v in &vals {
                let s = match v {
                    Val::Text(t) => t.clone(),
                    Val::Num(x) => x.to_string(),
                    Val::Bool(b) => b.to_string(),
                };
                if !uniq.iter().any(|u| u == &s) {
                    uniq.push(s);
                }
            }
            m.set("kind", Json::Str("text".to_string()));
            m.set("n", Json::Int(vals.len() as i64));
            m.set("unique", Json::Int(uniq.len() as i64));
            m.set("values", Json::Arr(uniq.iter().map(|u| Json::Str(u.clone())).collect()));
        }
        metrics.push(m);
    }
    let mut doc = Json::Obj(Vec::new());
    doc.set("id", Json::Str(run.id.clone()));
    doc.set("group", Json::Str(run.group.clone()));
    doc.set("x", Json::Str(run.x.clone()));
    doc.set("note", Json::Str(run.note.clone()));
    doc.set(
        "mode",
        Json::Str(match run.mode {
            Mode::Json => "json".to_string(),
            Mode::Capture => "capture".to_string(),
        }),
    );
    doc.set("cwd", Json::Str(run.cwd.clone()));
    doc.set(
        "argv",
        Json::Arr(run.cmd.iter().map(|c| Json::Str(c.clone())).collect()),
    );
    doc.set("seeds", Json::Arr(run.seeds.iter().map(|s| Json::Int(*s)).collect()));
    doc.set("repeats", Json::Int(run.repeats as i64));
    doc.set("n", Json::Int(records.len() as i64));
    doc.set(
        "exit_codes",
        Json::Arr(
            records
                .iter()
                .map(|r| Json::Obj(vec![
                    ("seed".to_string(), Json::Int(r.seed)),
                    ("rep".to_string(), Json::Int(r.rep as i64)),
                    ("code".to_string(), Json::Int(r.exit_code as i64)),
                    ("timed_out".to_string(), Json::Bool(r.timed_out)),
                    ("stdout_bytes".to_string(), Json::Int(r.stdout_bytes as i64)),
                ]))
                .collect(),
        ),
    );
    let elapsed: Vec<f64> = records.iter().map(|r| r.elapsed_s).collect();
    let peaks: Vec<f64> = records.iter().map(|r| r.peak_ws_bytes).collect();
    doc.set("elapsed_s", stats(&elapsed));
    doc.set("peak_ws_bytes", stats(&peaks));
    doc.set("metrics", Json::Arr(metrics));
    if let Some(cmp) = &run.compare_to {
        let mut r = Json::Obj(Vec::new());
        r.set("path", Json::Str(cmp.clone()));
        let all_same = records
            .iter()
            .filter_map(|rec| {
                rec.metrics
                    .iter()
                    .find(|(n, _)| n == "stdout_sha256")
                    .map(|(_, v)| match v {
                        Val::Text(t) => t.clone(),
                        _ => String::new(),
                    })
            })
            .collect::<Vec<String>>();
        let unique = {
            let mut u: Vec<String> = Vec::new();
            for s in &all_same {
                if !u.iter().any(|x| x == s) {
                    u.push(s.clone());
                }
            }
            u.len()
        };
        r.set("runs_identical", Json::Bool(unique == 1 && !all_same.is_empty()));
        doc.set("reference", r);
    }
    doc
}

fn stats(xs: &[f64]) -> Json {
    let mut m = Json::Obj(Vec::new());
    if xs.is_empty() {
        m.set("n", Json::Int(0));
        return m;
    }
    let n = xs.len();
    let mean = xs.iter().sum::<f64>() / n as f64;
    let sd = if n > 1 {
        (xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n as f64 - 1.0)).sqrt()
    } else {
        0.0
    };
    let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    m.set("n", Json::Int(n as i64));
    m.set("mean", Json::Float(mean));
    m.set("sd", Json::Float(sd));
    m.set("min", Json::Float(min));
    m.set("max", Json::Float(max));
    m
}

fn check_freshness(opts: &Options, plan: &Plan) -> Result<i32, String> {
    let out_path = resolve(opts.root, opts.out);
    let text = std::fs::read_to_string(&out_path)
        .map_err(|e| format!("{}: {e} (run `papertool measure` first)", out_path.display()))?;
    let doc = json::parse(&text)?;
    let runs = match doc.get("runs") {
        Some(Json::Arr(a)) => a.clone(),
        _ => return Err("measurements.json has no runs array".to_string()),
    };
    let mut problems = 0usize;
    for run in &plan.runs {
        let found = runs.iter().find(|r| {
            r.get("id").and_then(|v| v.as_str()) == Some(run.id.as_str())
        });
        match found {
            None => {
                eprintln!("STALE: run '{}' has no measurements", run.id);
                problems += 1;
            }
            Some(r) => {
                let n = r.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                let want = (run.seeds.len() as i64) * (run.repeats as i64);
                if n != want {
                    eprintln!("STALE: run '{}' has n={n}, plan asks for {want}", run.id);
                    problems += 1;
                }
            }
        }
    }
    if problems > 0 {
        println!("{problems} stale measurement(s): re-run `papertool measure`");
        Ok(1)
    } else {
        println!("measurements match the plan ({} runs)", plan.runs.len());
        Ok(0)
    }
}

// ------------------------------------------------------------------ peak memory

#[cfg(windows)]
fn peak_ws(child: &std::process::Child) -> Option<f64> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    #[allow(non_snake_case)]
    struct ProcessMemoryCounters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }
    extern "system" {
        fn K32GetProcessMemoryInfo(
            process: isize,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }
    let handle = child.as_raw_handle() as isize;
    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        PageFaultCount: 0,
        PeakWorkingSetSize: 0,
        WorkingSetSize: 0,
        QuotaPeakPagedPoolUsage: 0,
        QuotaPagedPoolUsage: 0,
        QuotaPeakNonPagedPoolUsage: 0,
        QuotaNonPagedPoolUsage: 0,
        PagefileUsage: 0,
        PeakPagefileUsage: 0,
    };
    let ok = unsafe { K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) };
    if ok == 0 {
        None
    } else {
        Some(counters.PeakWorkingSetSize as f64)
    }
}

#[cfg(not(windows))]
fn peak_ws(child: &std::process::Child) -> Option<f64> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", child.id())).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            let kb: f64 = rest.trim().trim_end_matches(" kB").trim().parse().ok()?;
            return Some(kb * 1024.0);
        }
    }
    None
}

// ---------------------------------------------------------------------- helpers

pub fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs as i64)
}

fn format_utc(epoch: i64) -> String {
    // Civil-from-days (Howard Hinnant), no dependencies.
    let days = epoch.div_euclid(86_400);
    let rem = epoch.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    // Small self-contained SHA-256 (FIPS 180-4); used to fingerprint raw stdout.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    let bit_len = (bytes.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in data.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = String::new();
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

pub fn write_file(path: &Path, text: &str) -> Result<(), String> {
    let mut f = std::fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    f.write_all(text.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

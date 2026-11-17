//! Parity: the two checks that make the port a checkable claim instead of a rewrite.
//!
//! `protorun parity streams` compares the facts this binary generates (and the exact
//! order in which the harness writes them) with the `arguments` recorded in the raw logs
//! of a run produced by the pre-port Python harness.
//!
//! `protorun parity run-vs-report` compares a report produced by this binary with one
//! produced by the Python harness, field by field, exempting only the fields that cannot
//! be equal (timing, machine description, the harness hash) and normalising the paths
//! that name the run directory.

use std::path::Path;

use crate::json::{self, Json};
use crate::report::FAMILY_ORDER;
use crate::streams::{self, FactOpts};
use crate::util::{basename, OrderedMap};

use crate::adapter::AdapterKind;

fn parse(args: &[String], valued: &[&str]) -> Result<OrderedMap<String>, String> {
    let mut out: OrderedMap<String> = OrderedMap::new();
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some((name, value)) = rest.split_once('=') {
                out.set(name, value.to_string());
                i += 1;
                continue;
            }
            if valued.contains(&rest) {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("--{rest} needs a value"))?;
                out.set(rest, value.clone());
                i += 2;
                continue;
            }
            out.set(rest, String::new());
            i += 1;
            continue;
        }
        i += 1;
    }
    Ok(out)
}

fn get(opts: &OrderedMap<String>, name: &str) -> Option<String> {
    opts.get(name).cloned()
}

fn seeds_of(opts: &OrderedMap<String>) -> Result<Vec<i64>, String> {
    let raw = match get(opts, "seeds") {
        Some(v) if !v.is_empty() => v,
        _ => {
            // read the seeds off the reference report when it is present
            if let Some(dir) = get(opts, "reference") {
                if let Ok(rep) = json::parse_file(&format!("{dir}/report.json")) {
                    if let Some(arr) = rep
                        .get("runner")
                        .and_then(|r| r.get("seeds"))
                        .and_then(|s| s.as_arr())
                    {
                        return Ok(arr.iter().filter_map(|v| v.as_i64()).collect());
                    }
                }
            }
            "1,2,3".to_string()
        }
    };
    raw.split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<i64>())
        .collect::<Result<Vec<i64>, _>>()
        .map_err(|_| "--seeds must be a comma-separated list of integers".to_string())
}

// ------------------------------------------------------------- generated vs recorded

fn liquid_args(fact: &Json, tags: &[String], domain: &str) -> Json {
    Json::Obj(vec![
        (
            "claim".to_string(),
            fact.get("content").cloned().unwrap_or(Json::Null),
        ),
        (
            "status".to_string(),
            fact.get("status").cloned().unwrap_or(Json::Null),
        ),
        (
            "confidence".to_string(),
            fact.get("confidence").cloned().unwrap_or(Json::Null),
        ),
        (
            "source".to_string(),
            fact.get("source").cloned().unwrap_or(Json::Null),
        ),
        ("domain".to_string(), Json::Str(domain.to_string())),
        (
            "tags".to_string(),
            Json::Arr(tags.iter().cloned().map(Json::Str).collect()),
        ),
    ])
}

fn memory_args(fact: &Json, tags: &[String]) -> Json {
    Json::Obj(vec![
        (
            "register".to_string(),
            fact.get("register").cloned().unwrap_or(Json::Null),
        ),
        (
            "content".to_string(),
            fact.get("content").cloned().unwrap_or(Json::Null),
        ),
        (
            "project".to_string(),
            fact.get("project").cloned().unwrap_or(Json::Null),
        ),
        (
            "source".to_string(),
            fact.get("source").cloned().unwrap_or(Json::Null),
        ),
        (
            "confidence".to_string(),
            fact.get("confidence").cloned().unwrap_or(Json::Null),
        ),
        (
            "tags".to_string(),
            Json::Arr(tags.iter().cloned().map(Json::Str).collect()),
        ),
        ("links".to_string(), Json::Arr(vec![])),
    ])
}

fn write_args(kind: AdapterKind, fact: &Json) -> Json {
    let tags: Vec<String> = fact
        .get("tags")
        .and_then(|v| v.as_arr())
        .map(|a| {
            a.iter()
                .map(|t| t.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    match kind {
        AdapterKind::Liquid => {
            let domain = tags
                .first()
                .cloned()
                .unwrap_or_else(|| "protocol".to_string());
            liquid_args(fact, &tags, &domain)
        }
        AdapterKind::SintMemory => memory_args(fact, &tags),
    }
}

fn arr<'a>(st: &'a Json, key: &str) -> &'a [Json] {
    st.get(key)
        .and_then(|v| v.as_arr())
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

fn str_of<'a>(v: &'a Json, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

/// The facts the family runner writes, in the order it writes them.
pub fn expected_writes(kind: AdapterKind, fam: &str, st: &Json) -> Vec<Json> {
    let mut out: Vec<Json> = Vec::new();
    match fam {
        "T1" => {
            let facts = arr(st, "facts");
            for fact in facts {
                out.push(write_args(kind, fact));
            }
            let retries = st
                .get("retry_keys")
                .and_then(|v| v.as_arr())
                .map(|a| a.len())
                .unwrap_or(0);
            for fact in facts.iter().take(retries) {
                out.push(write_args(kind, fact));
            }
        }
        "T2" => {
            for fact in arr(st, "corpus") {
                out.push(write_args(kind, fact));
            }
        }
        "T3" => {
            for fact in arr(st, "facts") {
                out.push(write_args(kind, fact));
            }
        }
        "T4" => {
            for record in arr(st, "records") {
                out.push(write_args(kind, record));
            }
        }
        "T5" => {
            for fact in arr(st, "wave") {
                out.push(write_args(kind, fact));
            }
            for fact in arr(st, "unrelated") {
                out.push(write_args(kind, fact));
            }
            for c in arr(st, "conflicts") {
                if let Some(second) = c
                    .get("written")
                    .and_then(|v| v.as_arr())
                    .and_then(|w| w.get(1))
                {
                    out.push(write_args(kind, second));
                }
            }
        }
        "T6" => {
            for episode in arr(st, "episodes") {
                out.push(write_args(kind, episode));
            }
        }
        "T7" => {
            for it in arr(st, "intentions") {
                let content = format!(
                    "{} :: {} :: context {}",
                    str_of(it, "cue"),
                    str_of(it, "intent"),
                    str_of(it, "context")
                );
                let fact = streams::make_fact(
                    str_of(it, "key"),
                    &content,
                    FactOpts::new()
                        .tags(vec!["t7intention".to_string()])
                        .confidence(0.9)
                        .source("user")
                        .subject(str_of(it, "cue"))
                        .value(str_of(it, "intent")),
                );
                out.push(write_args(kind, &fact));
            }
        }
        "T8" => {
            for item in arr(st, "in_coverage") {
                for fact in item
                    .get("write")
                    .and_then(|v| v.as_arr())
                    .cloned()
                    .unwrap_or_default()
                {
                    out.push(write_args(kind, &fact));
                }
            }
        }
        "T9" => {}
        "T10" => {
            for p in arr(st, "poison") {
                let content = format!(
                    "{} = {}",
                    str_of(p, "target_subject"),
                    str_of(p, "control_value")
                );
                let fact = streams::make_fact(
                    str_of(p, "target_key"),
                    &content,
                    FactOpts::new()
                        .subject(str_of(p, "target_subject"))
                        .value(str_of(p, "control_value"))
                        .tags(vec!["t10target".to_string()])
                        .confidence(0.8)
                        .source("user"),
                );
                out.push(write_args(kind, &fact));
            }
            for member in arr(st, "members") {
                out.push(write_args(kind, member));
            }
        }
        _ => {}
    }
    out
}

fn reference_writes(path: &str, tool: &str) -> Result<Vec<Json>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut out: Vec<Json> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let obj = match json::parse(line) {
            Ok(o) => o,
            Err(_) => continue,
        };
        if obj.get("kind").and_then(|v| v.as_str()) != Some("call") {
            continue;
        }
        if obj.get("tool").and_then(|v| v.as_str()) != Some(tool) {
            continue;
        }
        if let Some(args) = obj.get("arguments") {
            out.push(args.clone());
        }
    }
    Ok(out)
}

fn first_difference(a: &Json, b: &Json) -> String {
    match (a, b) {
        (Json::Obj(ma), Json::Obj(mb)) => {
            for (k, va) in ma {
                match mb.iter().find(|(mk, _)| mk == k) {
                    Some((_, vb)) => {
                        if va != vb {
                            return format!(
                                "{k}: {} != {}",
                                json::dump_compact(va, false),
                                json::dump_compact(vb, false)
                            );
                        }
                    }
                    None => return format!("{k}: present in the generated stream only"),
                }
            }
            for (k, _) in mb {
                if !ma.iter().any(|(mk, _)| mk == k) {
                    return format!("{k}: present in the recorded request only");
                }
            }
            "objects differ".to_string()
        }
        _ => format!(
            "{} != {}",
            json::dump_compact(a, false),
            json::dump_compact(b, false)
        ),
    }
}

pub fn cmd_parity_streams(args: &[String]) -> Result<i32, String> {
    let opts = parse(args, &["reference", "server", "root", "seeds"])?;
    let reference = match get(&opts, "reference") {
        Some(r) => r,
        None => return Err("--reference <run dir of the Python harness> is required".to_string()),
    };
    let server = match get(&opts, "server") {
        Some(s) => s,
        None => return Err("--server <name> is required".to_string()),
    };
    let root = get(&opts, "root").unwrap_or_else(|| ".".to_string());
    let registry = json::parse_file(&format!(
        "{}/code/servers.json",
        root.trim_end_matches(['/', '\\'])
    ))?;
    let spec = registry
        .get("servers")
        .and_then(|v| v.as_arr())
        .and_then(|servers| {
            servers
                .iter()
                .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(server.as_str()))
                .cloned()
        })
        .ok_or_else(|| format!("server {server} is not in code/servers.json"))?;
    let kind = match spec.get("adapter").and_then(|v| v.as_str()) {
        Some("liquid") => AdapterKind::Liquid,
        Some("sint_memory") => AdapterKind::SintMemory,
        other => return Err(format!("no writer mirror for adapter {other:?}")),
    };
    let tool = match kind {
        AdapterKind::Liquid => "liquid_belief",
        AdapterKind::SintMemory => "memory_write_block",
    };
    let seeds = seeds_of(&opts)?;
    let mut failures = 0usize;
    let mut checked = 0usize;
    println!(
        "stream parity: generated facts vs the recorded requests\nreference: {reference}\nserver:    {server} (tool {tool})"
    );
    for seed in seeds {
        let built = streams::build(seed);
        for fam in FAMILY_ORDER.iter() {
            let log = format!("{reference}/raw/{server}.seed{seed}.{fam}.jsonl");
            if !Path::new(&log).exists() {
                println!("  seed={seed} {fam}: no reference log at {log}, skipped");
                continue;
            }
            let expected = match built.get(fam) {
                Some(st) => expected_writes(kind, fam, st),
                None => Vec::new(),
            };
            let recorded = reference_writes(&log, tool)?;
            checked += expected.len();
            if expected == recorded {
                println!(
                    "  seed={seed} {fam}: MATCH ({} recorded write calls, arguments identical)",
                    recorded.len()
                );
                continue;
            }
            failures += 1;
            println!(
                "  seed={seed} {fam}: MISMATCH (generated {} writes, recorded {} calls)",
                expected.len(),
                recorded.len()
            );
            for (i, (a, b)) in expected.iter().zip(recorded.iter()).enumerate() {
                if a != b {
                    println!("      #{i}: {}", first_difference(a, b));
                    if i > 3 {
                        break;
                    }
                }
            }
            if expected.len() != recorded.len() {
                if let Some(extra) = expected.get(recorded.len()) {
                    println!(
                        "      first generated write with no recorded call: {}",
                        json::dump_compact(extra, false)
                    );
                }
            }
        }
    }
    println!(
        "result: {checked} write calls compared across the families and seeds above, {failures} mismatch(es)"
    );
    if failures == 0 {
        println!("STREAM PARITY OK");
        Ok(0)
    } else {
        println!("STREAM PARITY FAILED");
        Ok(1)
    }
}

// ------------------------------------------------------------------ report vs report

struct Diff {
    hard: Vec<String>,
    timing: Vec<String>,
    normalised: Vec<String>,
    volatile: Vec<String>,
    compared: usize,
}

fn is_volatile(path: &str) -> bool {
    path.ends_with(".harness_commit")
        || path.ends_with(".hardware_class")
        || path.ends_with(".environment")
        || path.ends_with(".known_deviations")
}

/// True for a path that names a metric object, e.g. `families[3].metrics[1]` or
/// `operations.O1_WRITE.metrics[5]`.
fn is_metric_path(path: &str) -> bool {
    match path.rfind(".metrics[") {
        Some(idx) => {
            let rest = &path[idx + ".metrics[".len()..];
            !rest.is_empty() && rest.ends_with(']') && !rest.contains('.') && !rest.contains('[')
        }
        None => false,
    }
}

fn is_latency(metric: &str) -> bool {
    metric.starts_with("write_latency") || metric.starts_with("read_latency")
}

fn magnitude(v: f64) -> i64 {
    if v == 0.0 {
        return i64::MIN;
    }
    v.abs().log10().floor() as i64
}

fn normalised(value: &str) -> String {
    basename(value)
}

fn compare(a: &Json, b: &Json, path: &str, metric: Option<&str>, d: &mut Diff) {
    if is_volatile(path) {
        d.volatile.push(path.to_string());
        return;
    }
    match (a, b) {
        (Json::Obj(ma), Json::Obj(mb)) => {
            let child_metric = if is_metric_path(path) {
                a.get("name").and_then(|v| v.as_str())
            } else {
                metric
            };
            for (k, va) in ma {
                let child = format!("{path}.{k}");
                match mb.iter().find(|(mk, _)| mk == k) {
                    Some((_, vb)) => compare(va, vb, &child, child_metric, d),
                    None => d.hard.push(format!("{child}: missing from the new report")),
                }
            }
            for (k, _) in mb {
                if !ma.iter().any(|(mk, _)| mk == k) {
                    d.hard
                        .push(format!("{path}.{k}: present in the new report only"));
                }
            }
        }
        (Json::Arr(xa), Json::Arr(xb)) => {
            if xa.len() != xb.len() {
                d.hard
                    .push(format!("{path}: {} entries vs {}", xa.len(), xb.len()));
                return;
            }
            for i in 0..xa.len() {
                compare(&xa[i], &xb[i], &format!("{path}[{i}]"), metric, d);
            }
        }
        _ => {
            d.compared += 1;
            let latency_value = metric.map(is_latency).unwrap_or(false)
                && (path.ends_with(".value")
                    || path.ends_with(".ci_low")
                    || path.ends_with(".ci_high"));
            if latency_value {
                match (a.as_f64(), b.as_f64()) {
                    (Some(x), Some(y)) => {
                        if magnitude(x) == magnitude(y) {
                            d.timing
                                .push(format!("{path}: {x} vs {y} (same order of magnitude)"));
                        } else {
                            d.hard.push(format!(
                                "{path}: latency differs by orders of magnitude ({x} vs {y})"
                            ));
                        }
                    }
                    (None, None) => {}
                    _ => d.hard.push(format!(
                        "{path}: {} vs {}",
                        json::dump_compact(a, false),
                        json::dump_compact(b, false)
                    )),
                }
                return;
            }
            if path.ends_with(".storage_dir") || path.contains("raw_log_paths[") {
                let (x, y) = (
                    normalised(a.as_str().unwrap_or("")),
                    normalised(b.as_str().unwrap_or("")),
                );
                if x == y {
                    d.normalised.push(format!(
                        "{path}: run-directory prefix differs, file names equal ({x})"
                    ));
                } else {
                    d.hard.push(format!("{path}: {x} vs {y}"));
                }
                return;
            }
            if a != b {
                d.hard.push(format!(
                    "{path}: {} vs {}",
                    json::dump_compact(a, false),
                    json::dump_compact(b, false)
                ));
            }
        }
    }
}

pub fn cmd_parity_reports(args: &[String]) -> Result<i32, String> {
    let opts = parse(args, &["reference", "run", "root"])?;
    let reference = match get(&opts, "reference") {
        Some(r) => r,
        None => return Err("--reference <run dir of the Python harness> is required".to_string()),
    };
    let run = match get(&opts, "run") {
        Some(r) => r,
        None => return Err("--run <run dir of this binary> is required".to_string()),
    };
    let left = json::parse_file(&format!("{reference}/report.json"))?;
    let right = json::parse_file(&format!("{run}/report.json"))?;
    let mut d = Diff {
        hard: Vec::new(),
        timing: Vec::new(),
        normalised: Vec::new(),
        volatile: Vec::new(),
        compared: 0,
    };
    compare(&left, &right, "report", None, &mut d);

    println!("report parity: Python harness (reference) vs protorun (run)");
    println!("reference: {reference}");
    println!("run:       {run}");
    println!();

    // The disclosure text names the harness, so it is exempt as a whole; every deviation
    // item the Python report carried must still be present in the new report.
    let mut shared = 0usize;
    let mut missing: Vec<String> = Vec::new();
    let ref_devs = left
        .get("disclosure")
        .and_then(|v| v.get("known_deviations"))
        .and_then(|v| v.as_arr())
        .cloned()
        .unwrap_or_default();
    let new_devs: Vec<String> = right
        .get("disclosure")
        .and_then(|v| v.get("known_deviations"))
        .and_then(|v| v.as_arr())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    for dev in ref_devs.iter().skip(1) {
        let text = dev.as_str().unwrap_or("");
        if new_devs.iter().any(|n| n == text) {
            shared += 1;
        } else {
            missing.push(text.chars().take(80).collect());
        }
    }
    println!(
        "known_deviations: {shared} of {} reference items carried over verbatim (the harness-describing item is intentionally rewritten)",
        ref_devs.len().saturating_sub(1)
    );
    for m in &missing {
        println!("   - not carried over: {m}");
    }
    println!();

    println!("fields compared exactly:            {}", d.compared);
    println!(
        "fields equal by order of magnitude: {} (timing)",
        d.timing.len()
    );
    println!("paths equal after normalisation:    {}", d.normalised.len());
    println!("fields exempt as machine-specific:  {}", d.volatile.len());
    println!();
    if !d.hard.is_empty() {
        println!("MISMATCHES ({}):", d.hard.len());
        for m in d.hard.iter().take(60) {
            println!("   - {m}");
        }
        if d.hard.len() > 60 {
            println!("   ... {} more", d.hard.len() - 60);
        }
    }
    if !missing.is_empty() {
        println!(
            "REPORT PARITY FAILED: {missing_len} disclosure item(s) missing",
            missing_len = missing.len()
        );
        return Ok(1);
    }
    if d.hard.is_empty() {
        println!(
            "REPORT PARITY OK: every measured value, every note, every CI and the whole report structure are identical to the Python harness's report{}",
            if d.timing.is_empty() { "" } else { " (timing metrics excepted)" }
        );
        Ok(0)
    } else {
        println!("REPORT PARITY FAILED");
        Ok(1)
    }
}

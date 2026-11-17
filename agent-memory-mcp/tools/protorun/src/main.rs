//! protorun -- one utility, one binary, subcommands, standard library only.
//!
//! The protocol harness of `protocol.md`, ported from `code/run_protocol.py` +
//! `code/streams.py` (both kept, unchanged, in `evidence/pre-port-runner/`):
//!
//!   streams   generate the T1-T10 task streams from a seed
//!   servers   list the candidate servers of code/servers.json
//!   run       drive one server over MCP stdio: raw logs, metrics, report.json, summary.md
//!   parity    prove the port changed nothing: generated streams vs the recorded raw
//!             requests, and the new report vs the report the Python harness produced
//!
//! Usage: `protorun <subcommand> [options]`, `protorun help`.

mod adapter;
mod families;
// The JSON reader/writer is shared verbatim with tools/refslint/src/json.rs, where the
// rest of its API is used; the accessors this harness does not call are kept so the two
// copies stay identical.
#[allow(dead_code)]
mod json;
mod mcp;
mod parity;
mod report;
mod rng;
mod streams;
mod util;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use adapter::{Adapter, AdapterKind, Failure, Rep};
use json::Json;
use mcp::{CallLog, Session};
use report::{ReportInputs, RunArgs, FAMILY_ORDER};
use util::{ensure_dir, OrderedMap};

struct Opts {
    values: Vec<(String, String)>,
    flags: Vec<String>,
    positional: Vec<String>,
}

impl Opts {
    fn get(&self, name: &str, default: &str) -> String {
        self.values
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| default.to_string())
    }

    fn opt(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

fn parse(args: &[String], valued: &[&str]) -> Result<Opts, String> {
    let mut opts = Opts {
        values: Vec::new(),
        flags: Vec::new(),
        positional: Vec::new(),
    };
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some((name, value)) = rest.split_once('=') {
                opts.values.push((name.to_string(), value.to_string()));
                i += 1;
                continue;
            }
            if valued.contains(&rest) {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("--{rest} needs a value"))?;
                opts.values.push((rest.to_string(), value.clone()));
                i += 2;
                continue;
            }
            opts.flags.push(rest.to_string());
            i += 1;
            continue;
        }
        opts.positional.push(arg.clone());
        i += 1;
    }
    Ok(opts)
}

fn usage() {
    println!(
        "\
protorun -- the protocol harness of the agent-memory-mcp paper package.

  streams   [--root .] [--seed 1] [--out FILE] [--json]
                                            generate the T1-T10 task streams for a seed
  servers   [--root .]                      list the candidates of code/servers.json
  run       --server <name> [--root .] [--server-root <dir>] [--server-python <exe>]
            [--server-pythonpath <dir>] [--seeds 1,2,3] [--max-calls 400] [--timeout 30]
            [--wall-clock-s 900] [--out reports] [--run-name <name>] [--quiet]
                                            drive the server over MCP stdio and write
                                            reports/<run>/ with raw/, report.json, summary.md
  parity streams --reference <run dir> --server <name> [--root .] [--seeds 1,2,3]
                                            generated facts vs the recorded raw requests
  parity run-vs-report --reference <run dir> --run <run dir> [--root .]
                                            field-by-field report comparison"
    );
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = match dispatch(&argv) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    };
    std::process::exit(code);
}

fn dispatch(argv: &[String]) -> Result<i32, String> {
    let sub = argv.first().map(|s| s.as_str()).unwrap_or("help");
    let rest = &argv[1.min(argv.len())..];
    match sub {
        "help" | "--help" | "-h" => {
            usage();
            Ok(0)
        }
        "streams" => cmd_streams(rest),
        "servers" => cmd_servers(rest),
        "run" => cmd_run(rest),
        "parity" => match rest.first().map(|s| s.as_str()).unwrap_or("help") {
            "streams" => parity::cmd_parity_streams(&rest[1..]),
            "run-vs-report" => parity::cmd_parity_reports(&rest[1..]),
            _ => {
                usage();
                Ok(2)
            }
        },
        other => Err(format!("unknown subcommand {other:?}; see `protorun help`")),
    }
}

// ----------------------------------------------------------------------------- helpers

/// Absolute form of a path, without resolving symlinks: the server process is launched
/// with the data directory in USERPROFILE/HOME, and a relative path there would be
/// resolved against the *server's* working directory, not the harness's.
fn absolutize(path: &str) -> String {
    if Path::new(path).is_absolute() {
        path.trim_end_matches(['/', '\\', '.']).to_string()
    } else if path == "." || path.is_empty() {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        cwd.join(path).to_string_lossy().to_string()
    }
}

fn load_registry(root: &str) -> Result<Json, String> {
    json::parse_file(&format!(
        "{}/code/servers.json",
        root.trim_end_matches(['/', '\\'])
    ))
}

fn find_spec(registry: &Json, name: &str) -> Option<Json> {
    registry
        .get("servers")
        .and_then(|v| v.as_arr())
        .and_then(|servers| {
            servers
                .iter()
                .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(name))
                .cloned()
        })
}

fn adapter_kind(spec: &Json) -> Option<AdapterKind> {
    match spec.get("adapter").and_then(|v| v.as_str()) {
        Some("liquid") => Some(AdapterKind::Liquid),
        Some("sint_memory") => Some(AdapterKind::SintMemory),
        _ => None,
    }
}

/// `platform.python_version()` of the interpreter that launches the server.
fn server_python_version(python: &str) -> String {
    let out = std::process::Command::new(python).arg("--version").output();
    match out {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            for token in text.split_whitespace() {
                if token
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                    && token.contains('.')
                {
                    return token.to_string();
                }
            }
            "unknown".to_string()
        }
        Err(_) => "unknown".to_string(),
    }
}

fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------- streams

fn cmd_streams(args: &[String]) -> Result<i32, String> {
    let opts = parse(args, &["seed", "out", "root"])?;
    let seed: i64 = opts
        .get("seed", "1")
        .parse()
        .map_err(|_| "--seed must be an integer".to_string())?;
    let built = streams::build(seed);
    if let Some(path) = opts.opt("out") {
        ensure_dir(
            Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
                .as_str(),
        )?;
        let members: Vec<(String, Json)> =
            built.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let text = json::dump(&Json::Obj(members), 2, false);
        std::fs::write(&path, format!("{text}\n"))
            .map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("wrote {path} (seed {seed}, {} families)", built.len());
        return Ok(0);
    }
    if opts.flag("json") {
        let members: Vec<(String, Json)> =
            built.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        println!("{}", json::dump(&Json::Obj(members), 2, false));
        return Ok(0);
    }
    for fam in FAMILY_ORDER.iter() {
        if let Some(st) = built.get(fam) {
            println!(
                "{fam} tasks={} :: {}",
                st.get("tasks").and_then(|v| v.as_i64()).unwrap_or(0),
                st.get("description").and_then(|v| v.as_str()).unwrap_or("")
            );
        }
    }
    println!("{}", json::dump(&streams::sizes_report_json(), 2, false));
    Ok(0)
}

// ---------------------------------------------------------------------------- servers

fn cmd_servers(args: &[String]) -> Result<i32, String> {
    let opts = parse(args, &["root"])?;
    let root = opts.get("root", ".");
    let registry = load_registry(&root)?;
    if let Some(servers) = registry.get("servers").and_then(|v| v.as_arr()) {
        for spec in servers {
            let reason = spec.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            println!(
                "{:<28} included={:<5} adapter={:<24} {}",
                spec.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                spec.get("included")
                    .map(|v| v.truthy().to_string())
                    .unwrap_or_else(|| "false".to_string()),
                spec.get("adapter").and_then(|v| v.as_str()).unwrap_or(""),
                reason.chars().take(90).collect::<String>()
            );
        }
    }
    Ok(0)
}

// -------------------------------------------------------------------------------- run

fn cmd_run(args: &[String]) -> Result<i32, String> {
    let opts = parse(
        args,
        &[
            "root",
            "server",
            "server-root",
            "server-python",
            "server-pythonpath",
            "seeds",
            "max-calls",
            "timeout",
            "wall-clock-s",
            "out",
            "run-name",
        ],
    )?;
    let root = opts.get("root", ".");
    let root = absolutize(root.trim_end_matches(['/', '\\']));
    let registry = load_registry(&root)?;
    let candidates = registry
        .get("servers")
        .and_then(|v| v.as_arr())
        .cloned()
        .unwrap_or_default();

    let server = match opts.opt("server") {
        Some(s) => s,
        None => return Err("--server is required (or use `protorun servers`)".to_string()),
    };
    let spec = match find_spec(&registry, &server) {
        Some(s) => s,
        None => return Err(format!("unknown server {server:?}; see `protorun servers`")),
    };
    let kind = match adapter_kind(&spec) {
        Some(k) => k,
        None => {
            return Err(format!(
                "server {server} has no adapter ({}): not executable in this pass",
                spec.get("adapter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none")
            ))
        }
    };

    let seeds: Vec<i64> = opts
        .get("seeds", "1,2,3")
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<i64>())
        .collect::<Result<Vec<i64>, _>>()
        .map_err(|_| "--seeds must be a comma-separated list of integers".to_string())?;
    if seeds.is_empty() {
        return Err("--seeds must name at least one seed".to_string());
    }
    let server_python = opts.get("server-python", "python");
    let server_root = if let Some(sr) = opts.opt("server-root") {
        absolutize(&sr)
    } else {
        format!("{root}/../../refs")
    };
    let out = absolutize(&opts.get("out", &format!("{root}/reports")));
    let max_calls: usize = opts
        .get("max-calls", "400")
        .parse()
        .map_err(|_| "--max-calls must be an integer".to_string())?;
    let timeout: f64 = opts
        .get("timeout", "30")
        .parse()
        .map_err(|_| "--timeout must be a number".to_string())?;
    let wall_clock_s: u64 = opts
        .get("wall-clock-s", "900")
        .parse()
        .map_err(|_| "--wall-clock-s must be an integer".to_string())?;
    let quiet = opts.flag("quiet");

    let run_date = util::utc_date(None);
    let run_name = opts
        .opt("run-name")
        .unwrap_or_else(|| format!("{server}-{run_date}"));
    let run_dir = format!("{}/{}", out.trim_end_matches(['/', '\\']), run_name);
    let raw_dir = format!("{run_dir}/raw");
    let data_root = format!("{run_dir}/data");
    ensure_dir(&raw_dir)?;
    ensure_dir(&data_root)?;

    let py_version = server_python_version(&server_python);
    let hardware_class = format!(
        "{}, {} logical CPUs (workstation class, not a benchmark host)",
        std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| "unknown arch".to_string()),
        cpu_count()
    );
    let environment = format!(
        "{}; {}; server interpreter {}; harness protorun (Rust, standard library only, no crates)",
        std::env::consts::OS,
        std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown CPU".to_string()),
        server_python
    );
    let args = RunArgs {
        server: server.clone(),
        server_root: server_root.clone(),
        server_python: server_python.clone(),
        server_pythonpath: opts.opt("server-pythonpath"),
        seeds: seeds.clone(),
        max_calls,
        timeout,
        wall_clock_s,
        quiet,
        run_date: run_date.clone(),
        py_version: py_version.clone(),
        taxonomy_version: report::detect_taxonomy_version(&root),
        hardware_class,
        environment,
        known_deviations: known_deviations(),
        candidates,
    };

    if !quiet {
        println!("run: {run_name}");
        println!(
            "server: {} @ {}",
            spec.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            spec.get("commit_sha")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        );
        println!("server root: {server_root}");
        println!("seeds: {seeds:?}");
    }

    let outcome = run_server(&args, &root, &spec, kind, &run_dir, &raw_dir, &data_root)?;

    let input = ReportInputs {
        spec: &spec,
        per_rep_by_family: &outcome.per_rep_by_family,
        fam_tasks: &outcome.fam_tasks,
        fam_logs: &outcome.fam_logs,
        fam_failures: &outcome.fam_failures,
        skipped: &outcome.skipped,
        init_result: &outcome.init_result,
        tool_names: &outcome.tool_names,
        run_dir: &run_dir,
        call_totals: &outcome.call_totals,
    };
    let built = report::build_report(&args, &root, &input);

    let report_path = format!("{run_dir}/report.json");
    let text = json::dump(&built, 2, false);
    util::write_text_py(&report_path, &format!("{text}\n"))?;
    let summary_path = report::write_summary(&format!("{run_dir}/summary.md"), &built)?;

    let schema_path = format!("{root}/code/protocol_schema.json");
    let (errors, used) = report::validate_with_repo_validator(&root, &schema_path, &report_path);
    if !errors.is_empty() {
        println!("REPORT INVALID ({} problems):", errors.len());
        for err in errors.iter().take(25) {
            println!("   - {err}");
        }
        return Ok(1);
    }
    let families = built
        .get("families")
        .and_then(|v| v.as_arr())
        .map(|f| f.len())
        .unwrap_or(0);
    let metric_entries: usize = built
        .get("families")
        .and_then(|v| v.as_arr())
        .map(|fams| {
            fams.iter()
                .map(|f| {
                    f.get("metrics")
                        .and_then(|m| m.as_arr())
                        .map(|m| m.len())
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0);
    println!(
        "VALID   {} (subject={}, families={}, metric entries={})",
        util::relpath(&report_path, &root),
        built
            .get("subject")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        families,
        metric_entries
    );
    println!("validated with: {}", used.join(", "));
    println!("summary {}", util::relpath(&summary_path, &root));
    println!("calls per repetition: {:?}", outcome.call_totals);

    let counts = built
        .get("failures")
        .and_then(|f| f.get("counts_by_code"))
        .cloned()
        .unwrap_or(Json::Obj(vec![]));
    let count_of = |code: &str| -> i64 { counts.get(code).and_then(|v| v.as_i64()).unwrap_or(0) };
    let suspect = count_of("protocol_error") + count_of("harness_fault");
    let total_calls: usize = std::cmp::max(1, outcome.call_totals.iter().sum());
    if suspect > 0 && (suspect as f64) > 0.05 * total_calls as f64 {
        println!(
            "RUN SUSPECT: {suspect} of {total_calls} calls failed with harness-side errors (see failures.counts_by_code); the report is written for inspection but not citable"
        );
        return Ok(2);
    }
    Ok(0)
}

fn known_deviations() -> Vec<String> {
    [
        "The harness is Rust (tools/protorun, standard library only, no crates). The pre-port Python harness (code/run_protocol.py, code/streams.py) is preserved unchanged in evidence/pre-port-runner/ and its reports are the ones the port is checked against: `protorun parity run-vs-report` compares the two field by field and `protorun parity streams` compares the generated facts with the recorded raw requests.",
        "T3 (as-of-date reads) and T4 (principal-scoped reads) are not_implemented in both subjects: no tool accepts a date or a caller identity. What is measurable is still reported (timestamp exposure, lifecycle-field exposure, principal scoping across the advertised tool schemas, and the admissibility error rate over returned records).",
        "T6 consolidation is not_implemented in both subjects. For sint-memory the closest offline operation available (memory_apply_decay) was executed and its probe survival recorded as maintenance_pass_probe_survival; the consolidation-specific metrics stay null.",
        "T7 is operationalised as store-mediated cue retrieval: neither subject has an intention or cue surface, so firing is defined as a cue-only query that surfaces the stored intention after a process restart (the session boundary the family calls for).",
        "T8 confidence is operationalised as the highest confidence among returned records, 0.0 when the system abstains. No model is in the loop, so verbalised confidence cannot be elicited the way protocol.md section 3 describes; the metric notes say so.",
        "Token and USD metrics are null by construction: no model or hosted API is called, so there is no token accounting and no price table applies. Context inflation is reported as a character ratio and labelled as such.",
        "Repetitions are 3 (the protocol minimum) and the reported 95% CIs are percentile bootstrap intervals over the repetition axis; with n=3 they are coarse. Dispersion is reported rather than claimed as task sampling variability.",
        "T10 membership inference is measured with the same principal that wrote the records, because the MCP tool surface carries no caller identity. The number measures store-level inferability, not cross-principal leakage.",
        "Both subjects come from one author's reference set (SINT). The candidate list and the host constraint that reduced it (pip installs fail with Errno 28, disk full) are disclosed; this is repository-snapshot bias, reported and not corrected.",
        "Each subject's store directory is left inside the run directory (reports/<run>/data/seed<seed>) so that every number can be recomputed from the raw responses and from the store as it stood.",
        "The Rust harness validates its report with `refslint report` (the ported schema validator) instead of importing code/validate_report.py; the two validators were shown to give identical verdicts in evidence/refslint-port-2026-10-16.txt. A family that raises inside a runner is recorded as a harness_fault failure and the family falls back to family_not_measured, as in the Python harness; the difference is that the Rust harness does not abort the run on a panic.",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

struct Outcome {
    per_rep_by_family: OrderedMap<Vec<families::Metrics>>,
    fam_logs: OrderedMap<Vec<String>>,
    fam_failures: Vec<OrderedMap<OrderedMap<Failure>>>,
    skipped: OrderedMap<String>,
    fam_tasks: OrderedMap<i64>,
    call_totals: Vec<usize>,
    init_result: Json,
    tool_names: Vec<String>,
}

fn run_server(
    args: &RunArgs,
    root: &str,
    spec: &Json,
    kind: AdapterKind,
    run_dir: &str,
    raw_dir: &str,
    data_root: &str,
) -> Result<Outcome, String> {
    let _ = root;
    let mut per_rep_by_family: OrderedMap<Vec<families::Metrics>> = OrderedMap::new();
    let mut fam_logs: OrderedMap<Vec<String>> = OrderedMap::new();
    for fam in FAMILY_ORDER.iter() {
        per_rep_by_family.set(fam, Vec::new());
        fam_logs.set(fam, Vec::new());
    }
    let mut fam_failures: Vec<OrderedMap<OrderedMap<Failure>>> = Vec::new();
    let mut skipped: OrderedMap<String> = OrderedMap::new();
    let mut fam_tasks: OrderedMap<i64> = OrderedMap::new();
    let mut call_totals: Vec<usize> = Vec::new();
    let mut init_result = Json::Obj(vec![]);
    let mut tool_names: Vec<String> = Vec::new();

    for seed in args.seeds.iter() {
        let data_dir = format!("{data_root}/seed{seed}");
        // protocol.md section 4.2: each repetition runs against a fresh data directory. A
        // re-run of the same run name must not inherit the previous store, or every count
        // (record totals, duplicate rate, monitoring accuracy) silently double-counts.
        util::remove_dir_all(&data_dir)?;
        ensure_dir(&data_dir)?;
        let built = streams::build(*seed);
        for fam in FAMILY_ORDER.iter() {
            if let Some(st) = built.get(fam) {
                fam_tasks.set(fam, st.get("tasks").and_then(|v| v.as_i64()).unwrap_or(0));
            }
        }

        let process_log_path = format!("{raw_dir}/{}.seed{seed}.process.jsonl", args.server);
        let process_log = Rc::new(RefCell::new(CallLog::new(&process_log_path)?));
        let mut session = Session::new(
            spec,
            &args.server_python,
            &args.server_root,
            args.server_pythonpath.as_deref(),
            "2025-06-18",
            args.timeout,
        );
        let tools = match session.launch(&data_dir, Some(process_log.clone())) {
            Ok(t) => t,
            Err(e) => {
                process_log.borrow_mut().close();
                return Err(format!("could not start {}: {e}", args.server));
            }
        };
        init_result = session.init_result.clone();
        tool_names = tools
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        let rep = Rc::new(RefCell::new(Rep::new(*seed, args.timeout, args.max_calls)));
        rep.borrow_mut().process_log = Some(process_log.clone());
        let mut adapter = Adapter::new(kind, session, tools, rep.clone());

        for fam in FAMILY_ORDER.iter() {
            let fam_log_path = format!("{raw_dir}/{}.seed{seed}.{fam}.jsonl", args.server);
            fam_logs.get_mut(fam).unwrap().push(fam_log_path.clone());
            let fam_log = Rc::new(RefCell::new(CallLog::new(&fam_log_path)?));
            rep.borrow_mut().select(fam);
            adapter.log = Some(fam_log.clone());
            let elapsed = rep.borrow().started.elapsed().as_secs_f64();
            if elapsed > args.wall_clock_s as f64 {
                if !skipped.contains(fam) {
                    skipped.set(
                        fam,
                        format!(
                            "wall_clock_s budget ({}s) exhausted after {elapsed:.1}s",
                            args.wall_clock_s
                        ),
                    );
                }
                fam_log.borrow_mut().close();
                if !args.quiet {
                    println!(
                        "  {} seed={} {} SKIPPED (wall clock)",
                        args.server, seed, fam
                    );
                }
                continue;
            }
            let metrics = match families::runner(fam) {
                Some(runner) => {
                    let st = built.get(fam).cloned().unwrap_or(Json::Obj(vec![]));
                    runner(&rep, &st, &mut adapter)
                }
                None => families::family_not_measured(&format!(
                    "no runner is registered for family {fam}"
                )),
            };
            fam_log.borrow_mut().close();
            per_rep_by_family.get_mut(fam).unwrap().push(metrics);
            if !args.quiet {
                println!(
                    "  {} seed={} {} done ({} calls so far, {:.1}s)",
                    args.server,
                    seed,
                    fam,
                    rep.borrow().calls,
                    rep.borrow().started.elapsed().as_secs_f64()
                );
            }
        }
        fam_failures.push(rep.borrow().failures.clone());
        call_totals.push(rep.borrow().calls);
        adapter.session.stop();
        process_log.borrow_mut().event(vec![
            ("event".to_string(), Json::Str("repetition_end".to_string())),
            ("calls".to_string(), Json::Int(rep.borrow().calls as i64)),
            (
                "seconds".to_string(),
                Json::Float(util::round3(rep.borrow().started.elapsed().as_secs_f64())),
            ),
        ]);
        process_log.borrow_mut().close();
    }
    let _ = run_dir;
    Ok(Outcome {
        per_rep_by_family,
        fam_logs,
        fam_failures,
        skipped,
        fam_tasks,
        call_totals,
        init_result,
        tool_names,
    })
}

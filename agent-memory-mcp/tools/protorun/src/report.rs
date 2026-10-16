//! Aggregation and report assembly.
//!
//! Port of the `aggregate_reps`/`aggregate_failures`/`build_operations`/`build_report`/
//! `write_summary` functions of `code/run_protocol.py`, including the key order of every
//! emitted object: the ported report is diffed against the Python one field by field
//! (`protorun parity run-vs-report`).

use std::path::Path;

use crate::adapter::Failure;
use crate::families::{bootstrap_ci, mean, metric, Metrics, N_RESAMPLES};
use crate::json::{self, Json};
use crate::rng::PyRandom;
use crate::util::{basename, ensure_dir, relpath, round6, sha256_hex, OrderedMap};

pub const FAMILY_ORDER: [&str; 10] = ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10"];

/// protocol.md section 2: which task families feed which taxonomy operation, per metric.
pub const OPERATION_MAP: [(&str, &str, &[&str]); 8] = [
    (
        "O1_WRITE",
        "T1,T5,T9",
        &[
            "write_precision",
            "write_recall",
            "duplicate_rate",
            "write_latency_p50_ms",
            "write_latency_p95_ms",
            "tokens_per_write",
            "cost_per_1000_writes_usd",
            "idempotency_violations",
        ],
    ),
    (
        "O2_READ",
        "T2,T3,T4",
        &[
            "recall_at_5",
            "mrr",
            "required_record_hit_rate",
            "provenance_correctness",
            "admissibility_error_rate",
            "context_inflation",
            "read_latency_p50_ms",
            "read_latency_p95_ms",
        ],
    ),
    (
        "O3_CONSOLIDATE",
        "T6",
        &[
            "compression_ratio",
            "probe_survival_rate",
            "derived_from_completeness",
            "cost_per_consolidation_usd",
        ],
    ),
    (
        "O4_REVISE",
        "T5",
        &[
            "contradiction_detection_f1",
            "revision_correctness",
            "chain_completeness",
            "stale_read_rate",
            "collateral_change_rate",
        ],
    ),
    (
        "O5_INTEND",
        "T7",
        &[
            "cue_hit_rate",
            "false_fire_rate",
            "fire_latency_calls",
            "context_availability_at_fire",
        ],
    ),
    (
        "O6_ASSERT",
        "T8",
        &[
            "abstention_correctness",
            "in_coverage_answer_correctness",
            "key_question_correctness",
            "subject_question_correctness",
            "ece",
            "discrimination_auroc",
            "monitoring_accuracy",
        ],
    ),
    (
        "PROVENANCE",
        "T1,T2,T5,T6",
        &["provenance_correctness", "derived_from_completeness"],
    ),
    (
        "LIFECYCLE",
        "T3,T4,T5,T9",
        &[
            "lifecycle_state_exposed",
            "admissibility_error_rate",
            "stale_read_rate",
            "restart_survival",
            "snapshot_reopen_survival",
        ],
    ),
];

/// Everything the report's metadata blocks are built from.
pub struct RunArgs {
    pub server: String,
    pub server_root: String,
    pub server_python: String,
    pub server_pythonpath: Option<String>,
    pub seeds: Vec<i64>,
    pub max_calls: usize,
    pub timeout: f64,
    pub wall_clock_s: u64,
    pub quiet: bool,
    pub run_date: String,
    pub py_version: String,
    pub taxonomy_version: String,
    pub hardware_class: String,
    pub environment: String,
    pub known_deviations: Vec<String>,
    pub candidates: Vec<Json>,
}

// ------------------------------------------------------------------------ aggregation

/// Mean over repetitions with a percentile bootstrap CI over the repetition axis.
pub fn aggregate_reps(per_rep: &[Metrics], rng: &mut PyRandom, repetitions: usize) -> Vec<Json> {
    let mut names: Vec<String> = Vec::new();
    for entry in per_rep {
        for (name, _) in entry.iter() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    let mut out: Vec<Json> = Vec::new();
    for name in names {
        let entries: Vec<&Json> = per_rep.iter().filter_map(|e| e.get(&name)).collect();
        if entries.is_empty() {
            continue;
        }
        let mut template = entries[0].clone();
        let values: Vec<f64> = entries
            .iter()
            .filter_map(|e| e.get("value").and_then(|v| v.as_f64()))
            .collect();
        let mut notes = template
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if repetitions > 1 {
            if !notes.is_empty() {
                notes.push_str("; ");
            }
            notes.push_str(&format!(
                "mean over {repetitions} repetitions, percentile bootstrap 95% CI ({N_RESAMPLES} resamples) over the repetition axis"
            ));
        }
        if !values.is_empty() {
            let (lo, hi) = bootstrap_ci(&values, rng);
            set_field(
                &mut template,
                "value",
                Json::Float(round6(mean(&values).unwrap_or(0.0))),
            );
            set_field(&mut template, "ci_low", opt_float(lo));
            set_field(&mut template, "ci_high", opt_float(hi));
            set_field(&mut template, "n", Json::Int(values.len() as i64));
        } else {
            set_field(&mut template, "value", Json::Null);
            set_field(&mut template, "ci_low", Json::Null);
            set_field(&mut template, "ci_high", Json::Null);
            set_field(&mut template, "n", Json::Int(0));
        }
        set_field(&mut template, "notes", Json::Str(notes));
        out.push(template);
    }
    out
}

fn opt_float(v: Option<f64>) -> Json {
    match v {
        Some(x) => Json::Float(round6(x)),
        None => Json::Null,
    }
}

/// `d[key] = value` on a JSON object, in place when the key exists.
fn set_field(value: &mut Json, key: &str, v: Json) {
    if let Json::Obj(members) = value {
        json::set_member(members, key, v);
    }
}

pub fn aggregate_failures(fam_maps: &[&OrderedMap<Failure>]) -> Vec<Json> {
    let mut merged: OrderedMap<Json> = OrderedMap::new();
    let mut counts: OrderedMap<i64> = OrderedMap::new();
    let mut examples: OrderedMap<Vec<String>> = OrderedMap::new();
    for fam_map in fam_maps {
        for (code, entry) in fam_map.iter() {
            counts.set(code, counts.get(code).copied().unwrap_or(0) + entry.count);
            if !examples.contains(code) {
                examples.set(code, Vec::new());
            }
            let slot = examples.get_mut(code).unwrap();
            for ex in &entry.examples {
                if slot.len() < 5 && !slot.contains(ex) {
                    slot.push(ex.clone());
                }
            }
        }
    }
    for code in counts.keys() {
        let mut members: Vec<(String, Json)> = vec![
            ("code".to_string(), Json::Str(code.clone())),
            (
                "count".to_string(),
                Json::Int(counts.get(code).copied().unwrap_or(0)),
            ),
        ];
        let ex = examples.get(code).cloned().unwrap_or_default();
        members.push((
            "examples".to_string(),
            Json::Arr(ex.into_iter().map(Json::Str).collect()),
        ));
        merged.set(code, Json::Obj(members));
    }
    merged.values().cloned().collect()
}

pub fn build_operations(
    per_rep_by_family: &OrderedMap<Vec<Metrics>>,
    rng: &mut PyRandom,
) -> OrderedMap<Json> {
    let mut operations: OrderedMap<Json> = OrderedMap::new();
    for (op, families, names) in OPERATION_MAP.iter() {
        let fam_list: Vec<&str> = families.split(',').collect();
        let mut entries: Vec<Json> = Vec::new();
        let mut covered = 0usize;
        for name in names.iter() {
            let mut template: Option<Json> = None;
            let mut pooled: Vec<f64> = Vec::new();
            for fam in &fam_list {
                if let Some(reps) = per_rep_by_family.get(fam) {
                    for rep_metrics in reps {
                        let entry = match rep_metrics.get(name) {
                            Some(e) => e,
                            None => continue,
                        };
                        if template.is_none() {
                            template = Some(entry.clone());
                        }
                        if let Some(v) = entry.get("value").and_then(|v| v.as_f64()) {
                            pooled.push(v);
                        }
                    }
                }
            }
            let mut template = match template {
                Some(t) => t,
                None => continue,
            };
            if !pooled.is_empty() {
                covered += 1;
                let (lo, hi) = bootstrap_ci(&pooled, rng);
                let mut notes = template
                    .get("notes")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !notes.is_empty() {
                    notes.push_str("; ");
                }
                notes.push_str(&format!("pooled over the repetitions of {families}"));
                set_field(
                    &mut template,
                    "value",
                    Json::Float(round6(mean(&pooled).unwrap_or(0.0))),
                );
                set_field(&mut template, "ci_low", opt_float(lo));
                set_field(&mut template, "ci_high", opt_float(hi));
                set_field(&mut template, "n", Json::Int(pooled.len() as i64));
                set_field(&mut template, "notes", Json::Str(notes));
            } else {
                set_field(&mut template, "value", Json::Null);
                set_field(&mut template, "ci_low", Json::Null);
                set_field(&mut template, "ci_high", Json::Null);
                set_field(&mut template, "n", Json::Int(0));
            }
            entries.push(template);
        }
        if entries.is_empty() {
            operations.set(
                op,
                Json::Obj(vec![
                    ("supported".to_string(), Json::Str("no".to_string())),
                    ("metrics".to_string(), Json::Arr(vec![])),
                    (
                        "notes".to_string(),
                        Json::Str(
                            "no metric of this operation is computable from the raw responses"
                                .to_string(),
                        ),
                    ),
                ]),
            );
            continue;
        }
        let supported = if covered == entries.len() && covered > 0 {
            "yes"
        } else if covered > 0 {
            "partial"
        } else {
            "no"
        };
        operations.set(
            op,
            Json::Obj(vec![
                ("supported".to_string(), Json::Str(supported.to_string())),
                ("metrics".to_string(), Json::Arr(entries.clone())),
                (
                    "notes".to_string(),
                    Json::Str(format!(
                        "tasks exercised: {families} ({covered} of {} metrics carry a value)",
                        entries.len()
                    )),
                ),
            ]),
        );
    }
    operations
}

// ---------------------------------------------------------------------- report build

pub fn harness_hash(src_dir: &str, servers_file: &str) -> String {
    let mut names: Vec<String> = std::fs::read_dir(src_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".rs"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut bytes: Vec<u8> = Vec::new();
    for name in names {
        let path = format!("{}/{}", src_dir.trim_end_matches(['/', '\\']), name);
        if let Ok(data) = std::fs::read(&path) {
            bytes.extend_from_slice(&data);
        }
    }
    if let Ok(data) = std::fs::read(servers_file) {
        bytes.extend_from_slice(&data);
    }
    sha256_hex(&bytes)[..16].to_string()
}

/// Read the taxonomy version the package declares, so the report cannot drift from it.
pub fn detect_taxonomy_version(repo: &str) -> String {
    let path = format!("{}/taxonomy.md", repo.trim_end_matches(['/', '\\']));
    if let Ok(text) = std::fs::read_to_string(&path) {
        for line in text.lines() {
            let low = line.to_lowercase();
            if low.contains("version") && line.contains("0.") {
                for token in line.replace('`', " ").replace('|', " ").split_whitespace() {
                    if token.starts_with("0.") && token.matches('.').count() <= 2 {
                        return token
                            .trim_matches(|c: char| c == '.' || c == ',' || c == ';')
                            .to_string();
                    }
                }
            }
        }
    }
    "0.1".to_string()
}

pub struct ReportInputs<'a> {
    pub spec: &'a Json,
    pub per_rep_by_family: &'a OrderedMap<Vec<Metrics>>,
    pub fam_tasks: &'a OrderedMap<i64>,
    pub fam_logs: &'a OrderedMap<Vec<String>>,
    pub fam_failures: &'a [OrderedMap<OrderedMap<Failure>>],
    pub skipped: &'a OrderedMap<String>,
    pub init_result: &'a Json,
    pub tool_names: &'a [String],
    pub run_dir: &'a str,
    pub call_totals: &'a [usize],
}

pub fn build_report(args: &RunArgs, repo: &str, input: &ReportInputs) -> Json {
    let mut rng = PyRandom::new(20_261_016);
    // `fam_skip` in the Python harness was a tiny record holding these two views.
    let fam_skip_failures = input.fam_failures;
    let skipped = input.skipped;
    let seeds = args.seeds.clone();
    let mut families: Vec<Json> = Vec::new();
    for fam in FAMILY_ORDER.iter() {
        let per_rep = input
            .per_rep_by_family
            .get(fam)
            .cloned()
            .unwrap_or_default();
        let metrics = aggregate_reps(&per_rep, &mut rng, seeds.len());
        let fam_maps: Vec<&OrderedMap<Failure>> = fam_skip_failures
            .iter()
            .map(|f| f.get(fam).unwrap_or(&EMPTY_FAILURES))
            .collect();
        let failures = aggregate_failures(&fam_maps);
        let metrics = if metrics.is_empty() {
            vec![metric(
                "family_not_measured",
                "count",
                None,
                "programmatic_check",
                "no repetition produced a metric for this family",
            )]
        } else {
            metrics
        };
        let mut members: Vec<(String, Json)> = vec![
            ("id".to_string(), Json::Str(fam.to_string())),
            (
                "tasks".to_string(),
                Json::Int(input.fam_tasks.get(fam).copied().unwrap_or(0)),
            ),
            ("repetitions".to_string(), Json::Int(seeds.len() as i64)),
            ("seed".to_string(), Json::Int(seeds[0])),
            ("metrics".to_string(), Json::Arr(metrics.clone())),
            ("failures".to_string(), Json::Arr(failures.clone())),
            (
                "raw_log_paths".to_string(),
                Json::Arr(
                    input
                        .fam_logs
                        .get(fam)
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .map(|p| Json::Str(relpath(p, repo)))
                        .collect(),
                ),
            ),
        ];
        if let Some(reason) = skipped.get(fam) {
            members.push(("skipped".to_string(), Json::Bool(true)));
            members.push(("skip_reason".to_string(), Json::Str(reason.clone())));
        }
        families.push(Json::Obj(members));
    }

    let mut counts_by_code: OrderedMap<i64> = OrderedMap::new();
    let mut examples: Vec<String> = Vec::new();
    for fam_entry in &families {
        let id = fam_entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        for failure in fam_entry
            .get("failures")
            .and_then(|v| v.as_arr())
            .cloned()
            .unwrap_or_default()
        {
            let code = failure.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let count = failure.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
            counts_by_code.set(code, counts_by_code.get(code).copied().unwrap_or(0) + count);
            for ex in failure
                .get("examples")
                .and_then(|v| v.as_arr())
                .cloned()
                .unwrap_or_default()
            {
                let ex = ex.as_str().unwrap_or("").to_string();
                let line = format!("{id}: {ex}");
                if examples.len() < 10 && !examples.contains(&line) {
                    examples.push(line);
                }
            }
        }
    }

    let negotiated = input
        .init_result
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("other")
        .to_string();
    let spec_revision = if negotiated == "2025-03-26" || negotiated == "2025-06-18" {
        negotiated
    } else {
        "other".to_string()
    };
    let price_table = Json::Obj(vec![
        (
            "date".to_string(),
            Json::Str(format!(
                "n/a — run of {}; no hosted model or API is called",
                args.run_date
            )),
        ),
        (
            "source".to_string(),
            Json::Str("store-only run: the memory server is exercised directly over MCP, so no per-token price applies".to_string()),
        ),
        ("usd_per_1k_input".to_string(), Json::Null),
        ("usd_per_1k_output".to_string(), Json::Null),
    ]);
    let candidate_servers: Vec<Json> = args
        .candidates
        .iter()
        .map(|c| {
            Json::Obj(vec![
                (
                    "name".to_string(),
                    c.get("name").cloned().unwrap_or(Json::Null),
                ),
                (
                    "included".to_string(),
                    Json::Bool(c.get("included").map(|v| v.truthy()).unwrap_or(false)),
                ),
                (
                    "reason".to_string(),
                    c.get("reason").cloned().unwrap_or(Json::Null),
                ),
            ])
        })
        .collect();
    let mut n_tasks: Vec<(String, Json)> = Vec::new();
    for fam in FAMILY_ORDER.iter() {
        n_tasks.push((
            fam.to_string(),
            Json::Int(input.fam_tasks.get(fam).copied().unwrap_or(0)),
        ));
    }
    let disclosure = Json::Obj(vec![
        (
            "server_selection_rule".to_string(),
            Json::Str(
                "Candidate set = every memory-oriented MCP server in the SINT reference snapshot \
                 (six repositories listed in data/repo-facts.json, SHAs recorded there and re-verified with \
                 git rev-parse before the run), plus the second memory server inside the SINT core repository. \
                 A candidate was included when (a) it could be launched on this host with the pinned dependency \
                 set (mcp 1.9.4, the only MCP framework importable here because pip installs fail with Errno 28, \
                 disk full) and (b) its tool surface exposes a fact store (O1/O2), which T1/T2/T5 require for the \
                 profiles to be comparable. Every candidate and the ground for its decision is in \
                 code/servers.json and in candidate_servers below. The sample is a snapshot of one author's \
                 repositories: it selects for servers that run and it is not representative of the MCP ecosystem."
                    .to_string(),
            ),
        ),
        ("candidate_servers".to_string(), Json::Arr(candidate_servers)),
        ("mcp_spec_revision".to_string(), Json::Str(spec_revision.clone())),
        (
            "mcp_servers_sha".to_string(),
            input.spec.get("commit_sha").cloned().unwrap_or(Json::Null),
        ),
        ("transport".to_string(), Json::Str("stdio".to_string())),
        (
            "model_under_test".to_string(),
            Json::Str("none (store-only)".to_string()),
        ),
        (
            "model_params".to_string(),
            Json::Obj(vec![
                (
                    "note".to_string(),
                    Json::Str(
                        "no model is in the loop; the store is exercised directly over MCP stdio"
                            .to_string(),
                    ),
                ),
                ("temperature".to_string(), Json::Null),
                ("top_p".to_string(), Json::Null),
                ("max_tokens".to_string(), Json::Null),
            ]),
        ),
        (
            "embedding_model".to_string(),
            Json::Str(
                "server-internal, unspecified (neither server documents an embedding model; matching is literal: exact content for sint-liquid-mcp, substring for sint-memory-mcp)"
                    .to_string(),
            ),
        ),
        (
            "seeds".to_string(),
            Json::Arr(seeds.iter().map(|s| Json::Int(*s)).collect()),
        ),
        ("repetitions".to_string(), Json::Int(seeds.len() as i64)),
        ("n_tasks_per_family".to_string(), Json::Obj(n_tasks)),
        (
            "budgets".to_string(),
            Json::Obj(vec![
                ("max_calls".to_string(), Json::Int(args.max_calls as i64)),
                ("max_tokens".to_string(), Json::Int(0)),
                ("wall_clock_s".to_string(), Json::Int(args.wall_clock_s as i64)),
                ("price_table".to_string(), price_table.clone()),
            ]),
        ),
        ("price_table".to_string(), price_table.clone()),
        (
            "grader_info".to_string(),
            Json::Str(
                "no rubric metric was graded in this run: the only rubric row (T6 abstraction_faithfulness) carries value null because no generalisations were produced; no graders were engaged and no kappa is reported"
                    .to_string(),
            ),
        ),
        (
            "prompt_templates_hash".to_string(),
            Json::Str("not-applicable (no model prompts in a store-only run)".to_string()),
        ),
        ("environment".to_string(), Json::Str(args.environment.clone())),
        (
            "known_deviations".to_string(),
            Json::Arr(
                args.known_deviations
                    .iter()
                    .cloned()
                    .map(Json::Str)
                    .collect(),
            ),
        ),
    ]);

    let runtime = input
        .spec
        .get("runtime")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .replace("{py_version}", &args.py_version);
    let total_calls: usize = input.call_totals.iter().sum();
    let mut sorted_tools: Vec<String> = input.tool_names.to_vec();
    sorted_tools.sort();
    let tool_list: String = sorted_tools.join(", ").chars().take(400).collect();

    let report = Json::Obj(vec![
        (
            "protocol".to_string(),
            Json::Obj(vec![
                ("version".to_string(), Json::Str("0.1".to_string())),
                (
                    "taxonomy_version".to_string(),
                    Json::Str(args.taxonomy_version.clone()),
                ),
                (
                    "mcp_spec_revision".to_string(),
                    Json::Str(spec_revision.clone()),
                ),
                (
                    "schema_uri".to_string(),
                    Json::Str(
                        "https://github.com/kostyk348/agent-memory-mcp/schema/protocol-v0.1.json"
                            .to_string(),
                    ),
                ),
            ]),
        ),
        (
            "subject".to_string(),
            Json::Obj(vec![
                (
                    "name".to_string(),
                    input.spec.get("name").cloned().unwrap_or(Json::Null),
                ),
                (
                    "kind".to_string(),
                    input.spec.get("kind").cloned().unwrap_or(Json::Null),
                ),
                (
                    "repo_url".to_string(),
                    input.spec.get("repo_url").cloned().unwrap_or(Json::Null),
                ),
                (
                    "commit_sha".to_string(),
                    input.spec.get("commit_sha").cloned().unwrap_or(Json::Null),
                ),
                (
                    "version".to_string(),
                    Json::Str(
                        "MCP server as published in the repository snapshot (no release tag)"
                            .to_string(),
                    ),
                ),
                ("transport".to_string(), Json::Str("stdio".to_string())),
                ("runtime".to_string(), Json::Str(runtime)),
                (
                    "deployment".to_string(),
                    Json::Str(
                        "local process, fresh data directory per repetition (USERPROFILE/HOME redirected)"
                            .to_string(),
                    ),
                ),
                (
                    "storage_dir".to_string(),
                    Json::Str(format!(
                        "reports/{}/data/seed<seed>",
                        basename(input.run_dir)
                    )),
                ),
            ]),
        ),
        (
            "runner".to_string(),
            Json::Obj(vec![
                (
                    "harness_commit".to_string(),
                    Json::Str(format!(
                        "uncommitted working tree: sha256:{} over tools/protorun/src/*.rs + code/servers.json (this workspace has no git metadata, so there is no commit to name)",
                        harness_hash(
                            &format!("{}/tools/protorun/src", repo.trim_end_matches(['/', '\\'])),
                            &format!("{}/code/servers.json", repo.trim_end_matches(['/', '\\']))
                        )
                    )),
                ),
                (
                    "model".to_string(),
                    Json::Str("none (store-only)".to_string()),
                ),
                ("model_params".to_string(), Json::Obj(vec![])),
                (
                    "prompt_templates_hash".to_string(),
                    Json::Str("not-applicable".to_string()),
                ),
                ("repetitions".to_string(), Json::Int(seeds.len() as i64)),
                (
                    "seeds".to_string(),
                    Json::Arr(seeds.iter().map(|s| Json::Int(*s)).collect()),
                ),
                (
                    "hardware_class".to_string(),
                    Json::Str(args.hardware_class.clone()),
                ),
            ]),
        ),
        (
            "budgets".to_string(),
            Json::Obj(vec![
                ("max_calls".to_string(), Json::Int(args.max_calls as i64)),
                ("max_tokens".to_string(), Json::Int(0)),
                ("wall_clock_s".to_string(), Json::Int(args.wall_clock_s as i64)),
                ("price_table".to_string(), price_table),
            ]),
        ),
        ("families".to_string(), Json::Arr(families.clone())),
        (
            "operations".to_string(),
            Json::Obj(
                build_operations(input.per_rep_by_family, &mut rng)
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        ),
        (
            "statistics".to_string(),
            Json::Obj(vec![
                (
                    "ci_method".to_string(),
                    Json::Str("bootstrap_percentile".to_string()),
                ),
                ("n_resamples".to_string(), Json::Int(N_RESAMPLES as i64)),
                (
                    "notes".to_string(),
                    Json::Str(format!(
                        "family metrics: mean over repetitions, percentile bootstrap 95% CI over the repetition axis. operation metrics: the same procedure pooled over the repetitions of the contributing families. Tool surface: {} tools advertised ({}). Calls issued: {} over {} repetition(s).",
                        input.tool_names.len(),
                        tool_list,
                        total_calls,
                        seeds.len()
                    )),
                ),
            ]),
        ),
        (
            "failures".to_string(),
            Json::Obj(vec![
                (
                    "counts_by_code".to_string(),
                    Json::Obj(
                        counts_by_code
                            .iter()
                            .map(|(k, v)| (k.clone(), Json::Int(*v)))
                            .collect(),
                    ),
                ),
                (
                    "examples".to_string(),
                    Json::Arr(examples.into_iter().map(Json::Str).collect()),
                ),
            ]),
        ),
        ("disclosure".to_string(), disclosure),
    ]);
    report
}

static EMPTY_FAILURES: OrderedMap<Failure> = OrderedMap::new();

// --------------------------------------------------------------------------- summary

fn opt_str(v: Option<&str>) -> String {
    v.unwrap_or("").to_string()
}

pub fn write_summary(path: &str, report: &Json) -> Result<String, String> {
    let g = |p: &[&str]| -> &Json {
        let mut node = report;
        for key in p {
            node = match node.get(key) {
                Some(n) => n,
                None => return &Json::Null,
            };
        }
        node
    };
    let gs = |p: &[&str]| opt_str(g(p).as_str());
    let mut lines: Vec<String> = Vec::new();
    let subject_name = gs(&["subject", "name"]);
    lines.push(format!("# Protocol run summary — {subject_name}"));
    lines.push(String::new());
    lines.push(
        "Not a ranking. This is a per-operation profile of one memory server under the".into(),
    );
    lines.push(
        "protocol in `protocol.md`; another server's profile is a separate report and the".into(),
    );
    lines.push("two are meant to be read side by side, not scored against each other.".into());
    lines.push(String::new());
    lines.push("| field | value |".into());
    lines.push("|---|---|".into());
    lines.push(format!("| subject | {subject_name} |"));
    lines.push(format!(
        "| repository / commit | {} @ {} |",
        gs(&["subject", "repo_url"]),
        gs(&["subject", "commit_sha"])
    ));
    lines.push(format!(
        "| transport / MCP revision | {} / {} |",
        gs(&["subject", "transport"]),
        gs(&["protocol", "mcp_spec_revision"])
    ));
    lines.push(format!("| runtime | {} |", gs(&["subject", "runtime"])));
    lines.push(format!(
        "| model under test | {} |",
        gs(&["disclosure", "model_under_test"])
    ));
    lines.push(format!(
        "| seeds / repetitions | {} / {} |",
        json::py_repr(g(&["disclosure", "seeds"])),
        g(&["disclosure", "repetitions"]).as_i64().unwrap_or(0)
    ));
    lines.push(format!(
        "| max_calls per repetition | {} |",
        g(&["budgets", "max_calls"]).as_i64().unwrap_or(0)
    ));
    lines.push(format!(
        "| environment | {} |",
        gs(&["disclosure", "environment"])
    ));
    let mut raw_dirs: Vec<String> = Vec::new();
    if let Some(fams) = g(&["families"]).as_arr() {
        for fam in fams {
            if let Some(paths) = fam.get("raw_log_paths").and_then(|v| v.as_arr()) {
                for p in paths {
                    let path = p.as_str().unwrap_or("");
                    let dir = path
                        .rsplit_once('/')
                        .map(|(d, _)| d.to_string())
                        .unwrap_or_else(|| path.to_string());
                    if !dir.is_empty() && !raw_dirs.contains(&dir) {
                        raw_dirs.push(dir);
                    }
                }
            }
        }
    }
    raw_dirs.sort();
    lines.push(format!("| raw responses | {} |", raw_dirs.join(", ")));
    lines.push(String::new());
    lines.push("## Metrics by task family".into());
    lines.push(String::new());
    if let Some(fams) = g(&["families"]).as_arr() {
        for fam in fams {
            let fid = opt_str(fam.get("id").and_then(|v| v.as_str()));
            let tasks = fam.get("tasks").and_then(|v| v.as_i64()).unwrap_or(0);
            let skipped = if fam.get("skipped").map(|v| v.truthy()).unwrap_or(false) {
                format!(
                    ", skipped: {}",
                    opt_str(fam.get("skip_reason").and_then(|v| v.as_str()))
                )
            } else {
                String::new()
            };
            lines.push(format!("### {fid} ({tasks} tasks{skipped})"));
            lines.push(String::new());
            lines.push("| metric | value | 95% CI | n | computer | note |".into());
            lines.push("|---|---|---|---|---|---|".into());
            if let Some(metrics) = fam.get("metrics").and_then(|v| v.as_arr()) {
                for m in metrics {
                    let ci = match m.get("ci_low").and_then(|v| v.as_f64()) {
                        Some(lo) => format!(
                            "[{:.4}, {:.4}]",
                            lo,
                            m.get("ci_high").and_then(|v| v.as_f64()).unwrap_or(0.0)
                        ),
                        None => "—".to_string(),
                    };
                    let val = match m.get("value").and_then(|v| v.as_f64()) {
                        Some(v) => format!("{v:.4}"),
                        None => "null".to_string(),
                    };
                    let note: String = opt_str(m.get("notes").and_then(|v| v.as_str()))
                        .replace('|', "/")
                        .chars()
                        .take(160)
                        .collect();
                    lines.push(format!(
                        "| {} | {} | {} | {} | {} | {} |",
                        opt_str(m.get("name").and_then(|v| v.as_str())),
                        val,
                        ci,
                        m.get("n").and_then(|v| v.as_i64()).unwrap_or(0),
                        opt_str(m.get("computer").and_then(|v| v.as_str())),
                        note
                    ));
                }
            }
            if let Some(failures) = fam.get("failures").and_then(|v| v.as_arr()) {
                if !failures.is_empty() {
                    lines.push(String::new());
                    for f in failures {
                        let first = f
                            .get("examples")
                            .and_then(|v| v.as_arr())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        lines.push(format!(
                            "- failure `{}` ×{}: {}",
                            opt_str(f.get("code").and_then(|v| v.as_str())),
                            f.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
                            first
                        ));
                    }
                }
            }
            lines.push(String::new());
        }
    }
    lines.push("## Profile by taxonomy operation".into());
    lines.push(String::new());
    lines.push("| operation | supported | metrics with a value |".into());
    lines.push("|---|---|---|".into());
    for op in [
        "O1_WRITE",
        "O2_READ",
        "O3_CONSOLIDATE",
        "O4_REVISE",
        "O5_INTEND",
        "O6_ASSERT",
        "PROVENANCE",
        "LIFECYCLE",
    ] {
        let block = match g(&["operations", op]) {
            Json::Null => continue,
            b => b,
        };
        let with_values: Vec<String> = block
            .get("metrics")
            .and_then(|v| v.as_arr())
            .map(|ms| {
                ms.iter()
                    .filter(|m| m.get("value").map(|v| !v.is_null()).unwrap_or(false))
                    .map(|m| opt_str(m.get("name").and_then(|v| v.as_str())))
                    .collect()
            })
            .unwrap_or_default();
        let joined = if with_values.is_empty() {
            "—".to_string()
        } else {
            with_values.join(", ")
        };
        lines.push(format!(
            "| {op} | {} | {joined} |",
            opt_str(block.get("supported").and_then(|v| v.as_str()))
        ));
    }
    lines.push(String::new());
    lines.push("## Failures recorded (never smoothed, protocol.md section 6)".into());
    lines.push(String::new());
    let counts = g(&["failures", "counts_by_code"]);
    let mut any = false;
    if let Some(members) = counts.as_obj() {
        let mut sorted: Vec<&(String, Json)> = members.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (code, n) in sorted {
            lines.push(format!("- `{code}` ×{}", n.as_i64().unwrap_or(0)));
            any = true;
        }
    }
    if !any {
        lines.push("- none".into());
    }
    lines.push(String::new());
    lines.push("## Known deviations from the protocol".into());
    lines.push(String::new());
    if let Some(devs) = g(&["disclosure", "known_deviations"]).as_arr() {
        for dev in devs {
            lines.push(format!("- {}", opt_str(dev.as_str())));
        }
    }
    lines.push(String::new());
    lines.push("## Reproduce".into());
    lines.push(String::new());
    lines.push("```".into());
    lines.push(
        "# regenerate this report (paths as used for this run are in disclosure.environment)"
            .into(),
    );
    lines.push(format!(
        "tools/protorun/target/release/protorun run --server {} --seeds 1,2,3",
        subject_name
    ));
    lines.push(String::new());
    lines.push("# compare it field by field with a report produced by the Python harness".into());
    lines.push("tools/protorun/target/release/protorun parity run-vs-report --run <new-run> --reference <python-run>".into());
    lines.push(String::new());
    lines.push("# validate it with the package validator".into());
    lines.push(
        "tools/refslint/target/release/refslint report --schema code/protocol_schema.json \\"
            .into(),
    );
    lines.push("    reports/*/report.json".into());
    lines.push("```".into());
    lines.push(String::new());
    if let Some(parent) = Path::new(path).parent() {
        ensure_dir(&parent.to_string_lossy())?;
    }
    crate::util::write_text_py(path, &format!("{}\n", lines.join("\n")))?;
    Ok(path.to_string())
}

// ------------------------------------------------------------------------ validation

/// Path to the package's Rust validator, or `None` when it is not built on this host.
pub fn find_refslint(repo: &str) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(env) = std::env::var("REFSLINT") {
        if !env.is_empty() {
            candidates.push(env);
        }
    }
    for name in ["refslint.exe", "refslint"] {
        for profile in ["release", "debug"] {
            candidates.push(format!(
                "{}/tools/refslint/target/{profile}/{name}",
                repo.trim_end_matches(['/', '\\'])
            ));
        }
    }
    candidates.into_iter().find(|c| Path::new(c).is_file())
}

/// Validate the emitted report with the validator this package ships.
pub fn validate_with_repo_validator(
    repo: &str,
    schema_path: &str,
    report_path: &str,
) -> (Vec<String>, Vec<String>) {
    let mut errors: Vec<String> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    match find_refslint(repo) {
        Some(binary) => {
            let out = std::process::Command::new(&binary)
                .arg("report")
                .arg("--schema")
                .arg(schema_path)
                .arg(report_path)
                .output();
            used.push("refslint report".to_string());
            match out {
                Ok(o) if o.status.success() => {}
                Ok(o) => {
                    let mut text = String::from_utf8_lossy(&o.stdout).to_string();
                    text.push_str(&String::from_utf8_lossy(&o.stderr));
                    errors.push(format!(
                        "refslint report rejected the report: {}",
                        text.trim().chars().take(300).collect::<String>()
                    ));
                }
                Err(e) => errors.push(format!("cannot run {}: {e}", binary)),
            }
        }
        None => used.push("refslint report: binary not built, skipped".to_string()),
    }
    (errors, used)
}

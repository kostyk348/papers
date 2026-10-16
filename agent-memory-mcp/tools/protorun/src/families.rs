//! Metric computation: the statistics helpers and the ten family runners.
//!
//! Port of section "small statistics helpers" and the `run_t1`..`run_t10` functions of
//! `code/run_protocol.py`. Every number is computed from MCP responses or from the
//! harness's own write log; no model and no judge is in the loop.

use std::cell::RefCell;
use std::rc::Rc;

use crate::adapter::{answer_digest, digest, store_total, Adapter, Rep};
use crate::json::Json;
use crate::mcp::snapshot_dir_name;
use crate::rng::PyRandom;
use crate::streams::{make_fact, FactOpts};
use crate::util::{copy_tree, py_len, remove_dir_all, round6, OrderedMap};

pub const N_RESAMPLES: usize = 1000;
pub const CI_QUANTILES: (f64, f64) = (0.025, 0.975);

pub type Metrics = OrderedMap<Json>;

// ------------------------------------------------------------------------- statistics

pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}

pub fn percentile(xs: &[f64], q: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut s: Vec<f64> = xs.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if s.len() == 1 {
        return Some(s[0]);
    }
    let pos = q * (s.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        return Some(s[lo]);
    }
    Some(s[lo] + (s[hi] - s[lo]) * (pos - lo as f64))
}

/// Percentile bootstrap over the repetition axis (protocol.md section 3).
pub fn bootstrap_ci(xs: &[f64], rng: &mut PyRandom) -> (Option<f64>, Option<f64>) {
    if xs.is_empty() {
        return (None, None);
    }
    if xs.len() == 1 {
        return (Some(xs[0]), Some(xs[0]));
    }
    let n = xs.len();
    let mut stats: Vec<f64> = Vec::with_capacity(N_RESAMPLES);
    for _ in 0..N_RESAMPLES {
        let mut sum = 0.0;
        for _ in 0..n {
            sum += xs[rng.randbelow(n as u64) as usize];
        }
        stats.push(sum / n as f64);
    }
    stats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = stats[(CI_QUANTILES.0 * (stats.len() - 1) as f64).floor() as usize];
    let hi = stats[(CI_QUANTILES.1 * (stats.len() - 1) as f64).ceil() as usize];
    (Some(lo), Some(hi))
}

pub fn ratio(num: f64, den: f64) -> Option<f64> {
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

/// `ratio()` over counts.
pub fn ratio_i(num: i64, den: usize) -> Option<f64> {
    ratio(num as f64, den as f64)
}

/// AUROC with average ranks for ties; `None` when one class is empty.
pub fn auroc(scores: &[f64], labels: &[bool]) -> Option<f64> {
    let pos = labels.iter().filter(|l| **l).count();
    let neg = labels.len() - pos;
    if pos == 0 || neg == 0 {
        return None;
    }
    let mut order: Vec<usize> = (0..scores.len()).collect();
    // Python's sort is stable, and ties keep the index order; sort_by is stable too.
    order.sort_by(|a, b| {
        scores[*a]
            .partial_cmp(&scores[*b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ranks = vec![0.0f64; scores.len()];
    let mut i = 0usize;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && scores[order[j + 1]] == scores[order[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[order[k]] = avg;
        }
        i = j + 1;
    }
    let rank_sum_pos: f64 = (0..scores.len())
        .filter(|i| labels[*i])
        .map(|i| ranks[i])
        .sum();
    let n_pos = pos as f64;
    let n_neg = neg as f64;
    Some((rank_sum_pos - n_pos * (n_pos + 1.0) / 2.0) / (n_pos * n_neg))
}

/// Expected calibration error, 10 equal-width bins over reported confidence.
pub fn ece(confs: &[f64], corrects: &[bool], bins: usize) -> Option<f64> {
    if confs.is_empty() {
        return None;
    }
    let total = confs.len() as f64;
    let mut acc_err = 0.0;
    for b in 0..bins {
        let lo = b as f64 / bins as f64;
        let hi = (b + 1) as f64 / bins as f64;
        let idx: Vec<usize> = (0..confs.len())
            .filter(|i| {
                let c = confs[*i];
                (lo <= c && c < hi) || (b == bins - 1 && c == 1.0)
            })
            .collect();
        if idx.is_empty() {
            continue;
        }
        let acc = idx.iter().filter(|i| corrects[**i]).count() as f64 / idx.len() as f64;
        let conf = idx.iter().map(|i| confs[*i]).sum::<f64>() / idx.len() as f64;
        acc_err += (idx.len() as f64 / total) * (acc - conf).abs();
    }
    Some(acc_err)
}

/// One metric row, in the key order the Python harness wrote.
pub fn metric(name: &str, unit: &str, value: Option<f64>, computer: &str, notes: &str) -> Json {
    let mut members: Vec<(String, Json)> = vec![
        ("name".to_string(), Json::Str(name.to_string())),
        ("unit".to_string(), Json::Str(unit.to_string())),
        (
            "value".to_string(),
            match value {
                Some(v) => Json::Float(round6(v)),
                None => Json::Null,
            },
        ),
        ("computer".to_string(), Json::Str(computer.to_string())),
        ("notes".to_string(), Json::Str(notes.to_string())),
    ];
    if computer == "rubric_two_graders" {
        members.push(("kappa".to_string(), Json::Null));
    }
    Json::Obj(members)
}

fn harness(name: &str, unit: &str, value: Option<f64>, notes: &str) -> Json {
    metric(name, unit, value, "harness", notes)
}

fn check(name: &str, unit: &str, value: Option<f64>, notes: &str) -> Json {
    metric(name, unit, value, "programmatic_check", notes)
}

fn m(map: &mut Metrics, m: Json) {
    let name = m
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    map.set(&name, m);
}

// ------------------------------------------------------------------- stream accessors

fn arr<'a>(st: &'a Json, key: &str) -> &'a [Json] {
    st.get(key)
        .and_then(|v| v.as_arr())
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

fn s<'a>(v: &'a Json, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

fn f(v: &Json, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn truthy(v: &Json, key: &str) -> bool {
    v.get(key).map(|x| x.truthy()).unwrap_or(false)
}

fn str_vec(v: &Json) -> Vec<String> {
    v.as_arr()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

// --------------------------------------------------------------------------------- T1

pub fn run_t1(rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    let facts = arr(st, "facts");
    let before = a.stats();
    let stored_before = store_total(&before.payload);
    let mut write_latencies: Vec<f64> = Vec::new();
    for fact in facts {
        let res = a.write(fact);
        write_latencies.push(res.latency_ms);
    }
    let retry_keys = str_vec(st.get("retry_keys").unwrap_or(&Json::Null));
    for fact in facts {
        if retry_keys.contains(&s(fact, "key").to_string()) {
            let res = a.write(fact);
            write_latencies.push(res.latency_ms);
        }
    }
    let stats = a.stats();
    let stored_total = store_total(&stats.payload);
    let stored: Option<i64> = match (stored_total, stored_before) {
        (Some(t), Some(b)) => Some(t - b),
        _ => None,
    };

    let mut correct: i64 = 0;
    for fact in facts {
        let content = s(fact, "content");
        let res = a.read_content(content, 1);
        if let Some(record) = res.records.first() {
            let got_content = a.content_of(record);
            let got_tags = record.get("tags");
            let fields_ok = got_content.as_deref() == Some(content)
                && record.get("source").and_then(|v| v.as_str()) == Some(s(fact, "source"))
                && (record
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-1.0)
                    - f(fact, "confidence"))
                .abs()
                    < 1e-9
                && match got_tags {
                    None => true,
                    Some(Json::Arr(tags)) => {
                        let want = str_vec(fact.get("tags").unwrap_or(&Json::Null));
                        want.iter()
                            .all(|w| tags.iter().any(|t| t.as_str() == Some(w.as_str())))
                    }
                    Some(_) => false,
                };
            if fields_ok {
                correct += 1;
            }
        }
    }

    let dup = match stored {
        Some(v) => Some(v - facts.len() as i64),
        None => None,
    };
    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "write_precision",
            "ratio",
            match stored {
                Some(v) => ratio(correct as f64, v as f64),
                None => None,
            },
            "correct_written_records / written_records. A stored record counts as correct when it matches a generated fact and is not an extra copy of one; written_records is the store's own record count after the writes minus its count before them, so the duplication is charged here and reported separately as duplicate_rate",
        ),
    );
    m(
        &mut out,
        harness(
            "write_recall",
            "ratio",
            ratio(correct as f64, facts.len() as f64),
            "correct_written_records / generated_facts",
        ),
    );
    m(
        &mut out,
        harness(
            "duplicate_rate",
            "ratio",
            match (dup, stored) {
                (Some(d), Some(sd)) => ratio(d as f64, sd as f64),
                _ => None,
            },
            "extra_records_from_retries / written_records; a retry is the same content with the same intent",
        ),
    );
    m(
        &mut out,
        check(
            "idempotency_violations",
            "count",
            dup.map(|d| std::cmp::max(0, d) as f64),
            "retries that created an extra record (protocol.md section 3)",
        ),
    );
    m(
        &mut out,
        harness(
            "write_latency_p50_ms",
            "ms",
            percentile(&write_latencies, 0.5),
            "wall clock around tools/call",
        ),
    );
    m(
        &mut out,
        harness(
            "write_latency_p95_ms",
            "ms",
            percentile(&write_latencies, 0.95),
            "",
        ),
    );
    m(
        &mut out,
        harness(
            "tokens_per_write",
            "tokens",
            None,
            "null: no model is in the loop, so no token accounting exists; payload sizes are in the raw log",
        ),
    );
    m(
        &mut out,
        harness(
            "cost_per_1000_writes_usd",
            "usd",
            None,
            "null: store-only run, no per-token price applies (see budgets.price_table)",
        ),
    );
    let _ = rep;
    out
}

// --------------------------------------------------------------------------------- T2

pub fn run_t2(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    let corpus = arr(st, "corpus");
    for fact in corpus {
        a.write(fact);
    }
    let mut gold_source: OrderedMap<String> = OrderedMap::new();
    for fact in corpus {
        gold_source.set(s(fact, "content"), s(fact, "source").to_string());
    }
    let mut latencies: Vec<f64> = Vec::new();
    let mut hits_at_k: Vec<f64> = Vec::new();
    let mut hits_any: Vec<f64> = Vec::new();
    let mut rrs: Vec<f64> = Vec::new();
    let (mut prov_ok, mut prov_tot) = (0i64, 0i64);
    let mut inflation: Vec<f64> = Vec::new();
    let mut by_kind: OrderedMap<Vec<f64>> = OrderedMap::new();
    by_kind.set("key", Vec::new());
    by_kind.set("topic", Vec::new());

    for q in arr(st, "queries") {
        let k = q.get("k").and_then(|v| v.as_i64()).unwrap_or(5) as usize;
        let res = a.search(s(q, "query"), k);
        latencies.push(res.latency_ms);
        let returned: Vec<Option<String>> = res.records.iter().map(|r| a.content_of(r)).collect();
        let required_content = s(q, "required_content");
        let top_k: Vec<&Option<String>> = returned.iter().take(k).collect();
        let mut rank: Option<usize> = None;
        for (i, c) in returned.iter().enumerate() {
            if c.as_deref() == Some(required_content) {
                rank = Some(i + 1);
                break;
            }
        }
        let hit = if top_k.iter().any(|c| c.as_deref() == Some(required_content)) {
            1.0
        } else {
            0.0
        };
        hits_at_k.push(hit);
        hits_any.push(if rank.is_some() { 1.0 } else { 0.0 });
        let kind = s(q, "kind").to_string();
        if !by_kind.contains(&kind) {
            by_kind.set(&kind, Vec::new());
        }
        by_kind.get_mut(&kind).unwrap().push(hit);
        rrs.push(match rank {
            Some(r) => 1.0 / r as f64,
            None => 0.0,
        });
        for (record, content) in res.records.iter().zip(returned.iter()) {
            prov_tot += 1;
            let gold = content
                .as_deref()
                .and_then(|c| gold_source.get(c))
                .map(|v| v.as_str());
            let got = record.get(a.record_source_field()).and_then(|v| v.as_str());
            if gold == got {
                prov_ok += 1;
            }
        }
        let req_chars = {
            let n = py_len(required_content);
            if n == 0 {
                1
            } else {
                n
            }
        };
        let got_chars: usize = returned
            .iter()
            .map(|c| py_len(c.as_deref().unwrap_or("")))
            .sum();
        inflation.push(got_chars as f64 / req_chars as f64);
    }

    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "recall_at_5",
            "ratio",
            mean(&hits_at_k),
            "required record inside the top k returned",
        ),
    );
    m(
        &mut out,
        harness(
            "required_record_hit_rate",
            "ratio",
            mean(&hits_any),
            "required record anywhere in the returned list, ignoring k",
        ),
    );
    m(&mut out, harness("mrr", "ratio", mean(&rrs), ""));
    m(
        &mut out,
        harness(
            "key_query_hit_rate",
            "ratio",
            mean(&by_kind.get("key").cloned().unwrap_or_default()),
            "queries that repeat the record's content verbatim",
        ),
    );
    m(
        &mut out,
        harness(
            "topic_query_hit_rate",
            "ratio",
            mean(&by_kind.get("topic").cloned().unwrap_or_default()),
            "queries that name only a shared topic token, so several records match",
        ),
    );
    m(
        &mut out,
        harness(
            "provenance_correctness",
            "ratio",
            ratio(prov_ok as f64, prov_tot as f64),
            "returned records whose source field matches the generated record",
        ),
    );
    m(
        &mut out,
        harness(
            "context_inflation",
            "ratio",
            mean(&inflation),
            "characters returned / characters of the records actually required (character ratio, not tokens: no tokenizer in the loop)",
        ),
    );
    m(
        &mut out,
        harness("read_latency_p50_ms", "ms", percentile(&latencies, 0.5), ""),
    );
    m(
        &mut out,
        harness(
            "read_latency_p95_ms",
            "ms",
            percentile(&latencies, 0.95),
            "",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T3

pub fn run_t3(rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    let facts = arr(st, "facts");
    for fact in facts {
        a.write(fact);
    }
    let mut exposed: i64 = 0;
    for fact in facts {
        let res = a.read_content(s(fact, "content"), 1);
        if let Some(record) = res.records.first() {
            if truthy(record, a.record_time_field()) {
                exposed += 1;
            }
        }
    }
    for p in arr(st, "probes") {
        rep.borrow_mut().failure(
            "not_implemented",
            &format!(
                "probe 'as of {}' for {}: no tool accepts an as-of date",
                s(p, "as_of"),
                s(p, "subject")
            ),
        );
    }
    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "temporal_correctness",
            "ratio",
            None,
            "null / not_implemented: the server exposes no date-scoped read, so the as-of probe cannot be answered",
        ),
    );
    m(
        &mut out,
        check(
            "temporal_metadata_exposed",
            "ratio",
            ratio_i(exposed, facts.len()),
            "fraction of read-back records carrying a timestamp field (metadata exists, the query does not)",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T4

pub fn run_t4(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    for record in arr(st, "records") {
        a.write(record);
    }
    let (mut returned_total, mut inadmissible, mut lifecycle_exposed) = (0i64, 0i64, 0i64);
    for p in arr(st, "probes") {
        let res = a.search(s(p, "query"), 5);
        for record in &res.records {
            returned_total += 1;
            if truthy(p, "inadmissible_for_caller") {
                inadmissible += 1;
            }
            if truthy(record, "valid_until")
                || truthy(record, "expired")
                || truthy(record, "superseded")
                || s(record, "status") == "SUPERSEDED"
            {
                lifecycle_exposed += 1;
            }
        }
    }
    let scoping = a.tool_specs_principal_fraction();
    let mut out = Metrics::new();
    m(
        &mut out,
        check(
            "admissibility_error_rate",
            "ratio",
            ratio_i(inadmissible, returned_total as usize),
            "returned records that belong to a different principal than the caller; the tool surface has no caller parameter, so this is a policy check over the responses",
        ),
    );
    m(
        &mut out,
        check(
            "lifecycle_state_exposed",
            "ratio",
            ratio_i(lifecycle_exposed, returned_total as usize),
            "returned records exposing a lifecycle field (expiry/supersede marker)",
        ),
    );
    m(
        &mut out,
        check(
            "principal_scoping_exposed",
            "ratio",
            scoping,
            "fraction of tools whose input schema accepts a caller/principal argument; read from the tools/list inputSchema of this run",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T5

pub fn run_t5(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    for fact in arr(st, "wave") {
        a.write(fact);
    }
    let unrelated = arr(st, "unrelated").to_vec();
    for fact in &unrelated {
        a.write(fact);
    }
    let mut unrelated_hash_before: OrderedMap<Option<String>> = OrderedMap::new();
    for fact in &unrelated {
        let res = a.read_content(s(fact, "content"), 1);
        unrelated_hash_before.set(s(fact, "content"), digest(&res.records));
    }

    let (mut revisions_ok, mut chain_ok, mut both_returned, mut stale) = (0i64, 0i64, 0i64, 0i64);
    let mut winner_top: Vec<f64> = Vec::new();
    for c in arr(st, "conflicts") {
        let written = c
            .get("written")
            .and_then(|v| v.as_arr())
            .cloned()
            .unwrap_or_default();
        let second = written.get(1).cloned().unwrap_or(Json::Null);
        a.write(&second);
        let winner = c.get("winner").cloned().unwrap_or(Json::Null);
        let loser = c.get("loser").cloned().unwrap_or(Json::Null);
        let winner_content = s(&winner, "content").to_string();
        let confidence = f(&winner, "confidence").max(f(&loser, "confidence"));
        let res = a.revise(
            &winner_content,
            "VERIFIED",
            confidence,
            s(c, "stated_reason"),
        );
        if res.error_code.is_none() {
            let read_back = a.read_content(&winner_content, 1);
            let record = read_back.records.first().cloned().unwrap_or(Json::Null);
            if s(&record, "status") == "VERIFIED" {
                revisions_ok += 1;
            }
            let hist = a.history(&winner_content);
            let entries: Vec<Json> = match hist.payload.as_ref() {
                Some(Json::Arr(items)) => items.clone(),
                Some(Json::Obj(members)) => members
                    .iter()
                    .find(|(k, _)| k == "revision_chain")
                    .and_then(|(_, v)| v.as_arr().cloned())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            if !entries.is_empty()
                && entries.iter().all(|e| {
                    e.get("reason").map(|v| v.truthy()).unwrap_or(false)
                        && e.get("timestamp").map(|v| v.truthy()).unwrap_or(false)
                })
            {
                chain_ok += 1;
            }
        }
        // read path after the conflict: are both competing contents still served?
        let w = a.search(&winner_content, 5);
        let l = a.search(s(&loser, "content"), 5);
        if !w.records.is_empty() && !l.records.is_empty() {
            both_returned += 1;
            stale += 1;
        }
        if a.supports_ranked_search() {
            let ranked = a.search(s(c, "subject"), 5);
            if let Some(top) = ranked.records.first() {
                let top_content = a.content_of(top);
                winner_top.push(if top_content.as_deref() == Some(winner_content.as_str()) {
                    1.0
                } else {
                    0.0
                });
            }
        }
    }

    let mut collateral: i64 = 0;
    for fact in &unrelated {
        let res = a.read_content(s(fact, "content"), 1);
        let before = unrelated_hash_before
            .get(s(fact, "content"))
            .cloned()
            .flatten();
        if digest(&res.records) == before {
            collateral += 1;
        }
    }

    let anomalies = a.anomalies();
    let mut detected: Option<f64> = None;
    if let Some(Json::Obj(members)) = anomalies.payload.as_ref() {
        match members.iter().find(|(k, _)| k == "total_contradictions") {
            Some((_, v)) => detected = Some(v.as_f64().unwrap_or(0.0)),
            None => {
                let valid = members
                    .iter()
                    .find(|(k, _)| k == "valid")
                    .map(|(_, v)| matches!(v, Json::Bool(true)))
                    .unwrap_or(false);
                if valid {
                    detected = Some(0.0);
                }
            }
        }
    }
    let n = arr(st, "conflicts").len();
    let f1 = detected.map(|d| if d == 0.0 { 0.0 } else { 1.0 });

    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "contradiction_detection_f1",
            "ratio",
            f1,
            "F1 of conflicts the store flags by itself after the contradicting writes; the harness never names a conflicting pair, so this measures detection, not registration",
        ),
    );
    let supports_revise = a.supports_revise();
    m(
        &mut out,
        check(
            "revision_correctness",
            "ratio",
            if supports_revise {
                ratio_i(revisions_ok, n)
            } else {
                None
            },
            if supports_revise {
                "fraction of conflicts where the surviving record matches the stated policy after the revision call"
            } else {
                "null / not_implemented: append-only store, no operation retires or rewrites a record"
            },
        ),
    );
    m(
        &mut out,
        check(
            "chain_completeness",
            "ratio",
            if supports_revise {
                ratio_i(chain_ok, n)
            } else {
                Some(0.0)
            },
            if supports_revise {
                "fraction of revisions with a resolvable from->to edge carrying a reason and a timestamp"
            } else {
                "0.0: the append-only chain has no revision edge to resolve; every write is a new block"
            },
        ),
    );
    m(
        &mut out,
        harness(
            "stale_read_rate",
            "ratio",
            ratio_i(stale, n),
            "post-conflict probes where the superseded content is still served by the read path",
        ),
    );
    m(
        &mut out,
        harness(
            "both_competing_contents_served_rate",
            "ratio",
            ratio_i(both_returned, n),
            "fraction of conflicts where both the kept and the superseded content are returned: the store holds no resolution",
        ),
    );
    m(
        &mut out,
        harness(
            "policy_winner_top_ranked_rate",
            "ratio",
            mean(&winner_top),
            if a.supports_ranked_search() {
                "top-ranked record for the conflict subject matching the stated policy winner"
            } else {
                "null: this server has no ranked search operation (the read path is an exact-content lookup), so ranking cannot be measured"
            },
        ),
    );
    let unrelated_len = unrelated.len();
    m(
        &mut out,
        check(
            "collateral_change_rate",
            "ratio",
            ratio_i(collateral, unrelated_len).map(|r| 1.0 - r),
            "fraction of unrelated records whose content hash changed during revision",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T6

pub fn run_t6(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    for episode in arr(st, "episodes") {
        a.write(episode);
    }
    let probes = arr(st, "probes").to_vec();
    let mut before: OrderedMap<Option<String>> = OrderedMap::new();
    for p in &probes {
        let res = a.read_probe(s(p, "query"), 5);
        before.set(s(p, "key"), answer_digest(&res.records, a));
    }
    let maint = a.maintenance();
    let mut after: OrderedMap<Option<String>> = OrderedMap::new();
    for p in &probes {
        let res = a.read_probe(s(p, "query"), 5);
        after.set(s(p, "key"), answer_digest(&res.records, a));
    }
    let mut survived: i64 = 0;
    for (key, value) in before.iter() {
        if value.is_some() && after.get(key).cloned().flatten() == *value {
            survived += 1;
        }
    }
    let items = match maint.payload.as_ref() {
        Some(Json::Obj(members)) => members
            .iter()
            .find(|(k, _)| k == "applied")
            .map(|(_, v)| v.clone()),
        _ => None,
    };
    let items_after = items.clone();
    let maint_ok = maint.error_code.is_none();

    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "probe_survival_rate",
            "ratio",
            if maint_ok {
                ratio_i(survived, probes.len())
            } else {
                None
            },
            if maint_ok {
                "identical probes answered identically before and after the maintenance pass"
            } else {
                "null / not_implemented: no consolidation or maintenance operation in the tool surface"
            },
        ),
    );
    let items_after_num = items_after.as_ref().and_then(|v| v.as_f64());
    let items_before_num = items.as_ref().and_then(|v| v.as_f64());
    m(
        &mut out,
        harness(
            "compression_ratio",
            "ratio",
            match items_after_num {
                Some(after_v) if after_v != 0.0 => ratio(items_before_num.unwrap_or(0.0), after_v),
                _ => None,
            },
            if items_after_num.map(|v| v != 0.0).unwrap_or(false) {
                "items_before / items_after the pass; this pass audits records rather than merging them"
            } else {
                "null / not_implemented: nothing to consolidate"
            },
        ),
    );
    m(
        &mut out,
        check(
            "derived_from_completeness",
            "ratio",
            None,
            "null: no generalisations are produced, so there is no derived_from edge to resolve",
        ),
    );
    m(
        &mut out,
        metric(
            "abstraction_faithfulness",
            "score_1_5",
            None,
            "rubric_two_graders",
            "null: no generalisations were produced, so nothing exists for two graders to score; no graders were engaged and kappa is not reported",
        ),
    );
    m(
        &mut out,
        harness(
            "cost_per_consolidation_usd",
            "usd",
            None,
            "null: store-only run, no per-token price applies",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T7

pub fn run_t7(rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    for it in arr(st, "intentions") {
        let content = format!(
            "{} :: {} :: context {}",
            s(it, "cue"),
            s(it, "intent"),
            s(it, "context")
        );
        let fact = make_fact(
            s(it, "key"),
            &content,
            FactOpts::new()
                .tags(vec!["t7intention".to_string()])
                .confidence(0.9)
                .source("user")
                .subject(s(it, "cue"))
                .value(s(it, "intent")),
        );
        a.write(&fact);
    }
    if let Err(e) = a.restart_session() {
        rep.borrow_mut().failure(
            "harness_fault",
            &format!("session boundary restart failed: {e}"),
        );
    }
    let (mut fired, mut false_fires, mut ctx_ok) = (0i64, 0i64, 0i64);
    for it in arr(st, "intentions") {
        let res = a.search(s(it, "cue"), 5);
        if !res.records.is_empty() {
            fired += 1;
            let content = res
                .records
                .first()
                .and_then(|r| a.content_of(r))
                .unwrap_or_default();
            if content.contains(s(it, "context")) {
                ctx_ok += 1;
            }
        }
    }
    for nc in arr(st, "non_cues") {
        let res = a.search(nc.as_str().unwrap_or(""), 5);
        if !res.records.is_empty() {
            false_fires += 1;
        }
    }
    let intentions = arr(st, "intentions").len();
    let non_cues = arr(st, "non_cues").len();
    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "cue_hit_rate",
            "ratio",
            ratio_i(fired, intentions),
            "fraction of intentions surfaced by a query containing only the cue token; no dedicated intention tool exists, so firing is defined as cue retrieval across the session boundary",
        ),
    );
    m(
        &mut out,
        harness(
            "false_fire_rate",
            "ratio",
            ratio_i(false_fires, non_cues),
            "non-cues at which an intention was returned",
        ),
    );
    m(
        &mut out,
        check(
            "context_availability_at_fire",
            "ratio",
            if fired != 0 {
                ratio_i(ctx_ok, fired as usize)
            } else {
                None
            },
            "fires where the stored context string is present in the returned record",
        ),
    );
    m(
        &mut out,
        harness(
            "fire_latency_calls",
            "count",
            if fired != 0 { Some(0.0) } else { None },
            "calls between issuing the cue and receiving the intention: 0, retrieval is a single call",
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T8

pub fn run_t8(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    for item in arr(st, "in_coverage") {
        for fact in item
            .get("write")
            .and_then(|v| v.as_arr())
            .cloned()
            .unwrap_or_default()
        {
            a.write(&fact);
        }
    }
    let mut confs: Vec<f64> = Vec::new();
    let mut corrects: Vec<bool> = Vec::new();
    let mut abstain_ok: i64 = 0;
    let out_of_coverage = arr(st, "out_of_coverage").to_vec();
    for item in &out_of_coverage {
        let res = a.search(s(item, "question"), 5);
        let abstained = res.records.is_empty();
        if abstained {
            abstain_ok += 1;
        }
        confs.push(0.0);
        corrects.push(abstained);
    }
    let mut in_kind: OrderedMap<Vec<f64>> = OrderedMap::new();
    in_kind.set("key", Vec::new());
    in_kind.set("subject", Vec::new());
    let in_coverage = arr(st, "in_coverage").to_vec();
    for item in &in_coverage {
        let res = a.search(s(item, "question"), 5);
        let top = res.records.first().and_then(|r| a.content_of(r));
        let required = item.get("required").cloned().unwrap_or(Json::Null);
        let required_str = required.as_str();
        let correct = top.as_deref() == required_str;
        let kind = s(item, "kind").to_string();
        if !in_kind.contains(&kind) {
            in_kind.set(&kind, Vec::new());
        }
        in_kind
            .get_mut(&kind)
            .unwrap()
            .push(if correct { 1.0 } else { 0.0 });
        let conf = res
            .records
            .iter()
            .map(|r| r.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .fold(None::<f64>, |acc, v| Some(acc.map_or(v, |a| a.max(v))))
            .unwrap_or(0.0);
        confs.push(conf);
        corrects.push(correct);
    }
    let (mon, mon_note) = a.monitoring();

    let mut combined = in_kind.get("key").cloned().unwrap_or_default();
    combined.extend(in_kind.get("subject").cloned().unwrap_or_default());
    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "abstention_correctness",
            "ratio",
            ratio_i(abstain_ok, out_of_coverage.len()),
            "out-of-coverage questions on which the system returned no record instead of an answer",
        ),
    );
    m(
        &mut out,
        harness(
            "in_coverage_answer_correctness",
            "ratio",
            mean(&combined),
            "in-coverage questions whose top-ranked returned record is the required one (the answer the caller actually sees, not merely somewhere in the result list)",
        ),
    );
    m(
        &mut out,
        harness(
            "key_question_correctness",
            "ratio",
            mean(&in_kind.get("key").cloned().unwrap_or_default()),
            "in-coverage questions that repeat the required record's content verbatim",
        ),
    );
    m(
        &mut out,
        harness(
            "subject_question_correctness",
            "ratio",
            mean(&in_kind.get("subject").cloned().unwrap_or_default()),
            "in-coverage questions that name only the subject token; two records exist and the higher-confidence one is required",
        ),
    );
    m(
        &mut out,
        harness(
            "ece",
            "ratio",
            ece(&confs, &corrects, 10),
            "operationalisation: the reported confidence of a probe is the highest confidence among returned records (0.0 when it abstains); 10 equal-width bins over that value; correctness of an in-coverage probe is top-1 answer correctness",
        ),
    );
    m(
        &mut out,
        harness(
            "discrimination_auroc",
            "ratio",
            auroc(&confs, &corrects),
            "AUROC of the same operationalised confidence against probe correctness; null when every probe landed in one class, which makes the ROC undefined",
        ),
    );
    m(
        &mut out,
        check(
            "monitoring_accuracy",
            "ratio",
            mon,
            &format!("agreement between the store's self-reported counters and the harness's own write log; {mon_note}"),
        ),
    );
    out
}

// --------------------------------------------------------------------------------- T9

pub fn run_t9(rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    let probe_facts = arr(st, "probe_facts").to_vec();
    let mut pre: OrderedMap<Option<String>> = OrderedMap::new();
    for fact in &probe_facts {
        let res = a.read_probe(s(fact, "content"), 5);
        pre.set(s(fact, "content"), answer_digest(&res.records, a));
    }
    if let Err(e) = a.restart_session() {
        rep.borrow_mut().failure(
            "harness_fault",
            &format!("session restart in the middle of the stream failed: {e}"),
        );
    }
    let mut post: OrderedMap<Option<String>> = OrderedMap::new();
    for fact in &probe_facts {
        let res = a.read_probe(s(fact, "content"), 5);
        post.set(s(fact, "content"), answer_digest(&res.records, a));
    }
    let mut survived: i64 = 0;
    let mut diff: Vec<String> = Vec::new();
    for (key, value) in pre.iter() {
        if value.is_some() && post.get(key).cloned().flatten() == *value {
            survived += 1;
        }
        if post.get(key).cloned().flatten() != *value {
            diff.push(key.clone());
        }
    }
    for key in diff.iter().take(5) {
        rep.borrow_mut().failure(
            "nondeterministic",
            &format!("read of {key} changed across restart"),
        );
    }

    let query_set = arr(st, "query_set").to_vec();
    let mut first: OrderedMap<Option<String>> = OrderedMap::new();
    let mut second: OrderedMap<Option<String>> = OrderedMap::new();
    for q in &query_set {
        let res = a.search(
            s(q, "query"),
            q.get("k").and_then(|v| v.as_i64()).unwrap_or(5) as usize,
        );
        first.set(s(q, "query"), digest(&res.records));
    }
    for q in &query_set {
        let res = a.search(
            s(q, "query"),
            q.get("k").and_then(|v| v.as_i64()).unwrap_or(5) as usize,
        );
        second.set(s(q, "query"), digest(&res.records));
    }
    let mut identical: i64 = 0;
    for (key, value) in first.iter() {
        if second.get(key).cloned().flatten() == *value {
            identical += 1;
        }
    }

    let mut snapshot_survival: Option<f64> = None;
    match snapshot_reopen(rep, st, a) {
        Ok(v) => snapshot_survival = v,
        Err(e) => rep
            .borrow_mut()
            .failure("harness_fault", &format!("snapshot re-open failed: {e}")),
    }

    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "restart_survival",
            "ratio",
            ratio_i(survived, pre.len()),
            "probes answered identically after a process restart against the same store directory",
        ),
    );
    m(
        &mut out,
        harness(
            "snapshot_reopen_survival",
            "ratio",
            snapshot_survival,
            "probes answered identically by a second process opened on a copy of the store files",
        ),
    );
    m(
        &mut out,
        harness(
            "repeat_query_equality",
            "ratio",
            ratio_i(identical, first.len()),
            "identical-input equality of the query set repeated inside one process",
        ),
    );
    out
}

/// Second process over a copy of the store files (protocol.md section 4.8).
fn snapshot_reopen(rep: &Rc<RefCell<Rep>>, st: &Json, a: &Adapter) -> Result<Option<f64>, String> {
    let src = a.session.data_dir.clone();
    let root = std::path::Path::new(&src)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let dst = crate::mcp::join_path(&root, &snapshot_dir_name(&src));
    if std::path::Path::new(&dst).exists() {
        remove_dir_all(&dst)?;
    }
    copy_tree(&src, &dst)?;

    let mut copy = crate::mcp::Session::new(
        &a.session.spec,
        &a.session.python,
        &a.session.server_root,
        a.session.pythonpath.as_deref(),
        &a.session.protocol_version,
        a.session.timeout,
    );
    let process_log = rep.borrow().process_log.clone();
    copy.launch(&dst, process_log.clone())?;
    let mut probe_adapter = Adapter::new(a.kind, copy, a.tool_specs.clone(), rep.clone());
    probe_adapter.log = process_log;
    let mut ok: i64 = 0;
    let probe_facts = arr(st, "probe_facts").to_vec();
    for fact in &probe_facts {
        let res = probe_adapter.read_probe(s(fact, "content"), 5);
        if res
            .records
            .iter()
            .any(|r| probe_adapter.content_of(r).as_deref() == Some(s(fact, "content")))
        {
            ok += 1;
        }
    }
    probe_adapter.session.stop();
    Ok(ratio_i(ok, probe_facts.len()))
}

// -------------------------------------------------------------------------------- T10

pub fn run_t10(_rep: &Rc<RefCell<Rep>>, st: &Json, a: &mut Adapter) -> Metrics {
    let poison = arr(st, "poison").to_vec();
    for p in &poison {
        let content = format!("{} = {}", s(p, "target_subject"), s(p, "control_value"));
        let fact = make_fact(
            s(p, "target_key"),
            &content,
            FactOpts::new()
                .subject(s(p, "target_subject"))
                .value(s(p, "control_value"))
                .tags(vec!["t10target".to_string()])
                .confidence(0.8)
                .source("user"),
        );
        a.write(&fact);
    }
    let mut influenced: i64 = 0;
    for p in &poison {
        a.search(s(p, "query"), 5);
        let res = a.search(s(p, "target_subject"), 5);
        let contents: String = res
            .records
            .iter()
            .map(|r| a.content_of(r).unwrap_or_default())
            .collect::<Vec<String>>()
            .join(" ");
        if contents.contains(s(p, "intended_value")) {
            influenced += 1;
        }
    }
    for member in arr(st, "members") {
        a.write(member);
    }
    let members = arr(st, "members").to_vec();
    let non_members = arr(st, "non_members").to_vec();
    let tpr_num = members
        .iter()
        .filter(|m| !a.search(s(m, "content"), 5).records.is_empty())
        .count() as i64;
    let fpr_num = non_members
        .iter()
        .filter(|m| !a.search(s(m, "content"), 5).records.is_empty())
        .count() as i64;
    let tpr = ratio_i(tpr_num, members.len());
    let fpr = ratio_i(fpr_num, non_members.len());

    let mut out = Metrics::new();
    m(
        &mut out,
        harness(
            "poisoning_influence_rate",
            "ratio",
            ratio_i(influenced, poison.len()),
            "trials where a query-only interaction changed the later answer for the target record",
        ),
    );
    m(&mut out, harness("membership_tpr", "ratio", tpr, ""));
    m(&mut out, harness("membership_fpr", "ratio", fpr, ""));
    m(
        &mut out,
        harness(
            "membership_inference_advantage",
            "ratio",
            match (tpr, fpr) {
                (Some(t), Some(fp)) => Some(t - fp),
                _ => None,
            },
            "TPR - FPR for distinguishing a stored record; same principal as the writer, so this is store-level leakage, not cross-principal",
        ),
    );
    out
}

/// The family runners, in report order.
pub fn runner(family: &str) -> Option<fn(&Rc<RefCell<Rep>>, &Json, &mut Adapter) -> Metrics> {
    match family {
        "T1" => Some(run_t1),
        "T2" => Some(run_t2),
        "T3" => Some(run_t3),
        "T4" => Some(run_t4),
        "T5" => Some(run_t5),
        "T6" => Some(run_t6),
        "T7" => Some(run_t7),
        "T8" => Some(run_t8),
        "T9" => Some(run_t9),
        "T10" => Some(run_t10),
        _ => None,
    }
}

/// The "the harness itself failed" row a family falls back to.
pub fn family_not_measured(note: &str) -> Metrics {
    let mut out = Metrics::new();
    m(&mut out, check("family_not_measured", "count", None, note));
    out
}

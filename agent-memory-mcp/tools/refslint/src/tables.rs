//! `refslint tables` -- turn the validated protocol reports into the paper's result
//! tables, its prose-number macros, and its number ledger.
//!
//! Why this exists: the rule of the package is that no number in the paper is typed by
//! hand. Section 6 of `tex/main.tex` therefore contains no literal measurement: every
//! table is an `\input{}` of a file this subcommand writes from `reports/<run>/report.json`,
//! and every number that appears in a sentence is a macro (`\val{...}`) defined in
//! `tex/numbers.tex`, also written here. A number that has no field in a report cannot be
//! referenced, and `--check` fails if the generated files are stale or if the text
//! references a key that does not exist.
//!
//! Outputs (relative to `--root`):
//!   tex/tables/run-identity.tex   who was run, at which commit, in which environment
//!   tex/tables/family-T1.tex ...  one side-by-side profile per task family
//!   tex/tables/operations.tex     the same profile by taxonomy operation
//!   tex/numbers.tex               `\val`/`\valci`/`\valn`/`\valt` lookups
//!   data/run-numbers.tsv          the ledger: every metric, its value and its JSON path
//!
//! Display rule (stated in the paper's captions too): values are printed rounded to at
//! most four decimals; the exact value and its JSON path are in `data/run-numbers.tsv`,
//! which is written from the same read of the same file.

use crate::json::{self, Json};
use crate::util;

pub struct Options<'a> {
    /// Package root; every default path is resolved against it.
    pub root: &'a str,
    /// Explicit report paths; empty means `--root`/reports/*/report.json, sorted.
    pub reports: &'a [String],
    /// Verify that the generated files on disk are the ones these reports produce.
    pub check: bool,
}

const IDENTITY_FIELDS: &[(&str, &[&str])] = &[
    ("subject", &["subject", "name"]),
    ("repository", &["subject", "repo_url"]),
    ("commit", &["subject", "commit_sha"]),
    ("published version", &["subject", "version"]),
    ("transport / MCP revision", &["subject", "transport"]),
    ("runtime", &["subject", "runtime"]),
    ("deployment", &["subject", "deployment"]),
    ("store kept in the run dir", &["subject", "storage_dir"]),
    ("model under test", &["runner", "model"]),
    ("hardware", &["runner", "hardware_class"]),
    ("runner / harness", &["runner", "harness_commit"]),
    ("environment", &["disclosure", "environment"]),
];

// ---------------------------------------------------------------- data model

#[derive(Clone)]
struct Metric {
    name: String,
    unit: String,
    computer: String,
    value: Json,
    ci_low: Json,
    ci_high: Json,
    n: Json,
    path: String,
}

impl Metric {
    fn has_value(&self) -> bool {
        !self.value.is_null()
    }
}

struct Scope {
    id: String,
    /// Task count per repetition (families) or the `supported` verdict (operations).
    detail: String,
    metrics: Vec<Metric>,
}

struct Subject {
    alias: String,
    name: String,
    raw_files: usize,
    identity: Vec<(String, String)>,
    families: Vec<Scope>,
    operations: Vec<Scope>,
    failures: Vec<(String, i64)>,
    calls_issued: Option<i64>,
    tools_advertised: Option<i64>,
    repetitions: i64,
    seeds: String,
    ci_method: String,
    n_resamples: Option<i64>,
}

impl Subject {
    fn metrics_total(&self) -> usize {
        self.families.iter().map(|f| f.metrics.len()).sum()
    }
    fn metrics_valued(&self) -> usize {
        self.families
            .iter()
            .map(|f| f.metrics.iter().filter(|m| m.has_value()).count())
            .sum()
    }
}

// ---------------------------------------------------------------- helpers

fn child<'a>(node: &'a Json, path: &[&str]) -> Option<&'a Json> {
    let mut cur = node;
    for key in path {
        let next = cur.get(key)?;
        if next.is_null() {
            return None;
        }
        cur = next;
    }
    Some(cur)
}

fn text_of(value: &Json) -> Option<String> {
    match value {
        Json::Null => None,
        Json::Str(s) => Some(s.clone()),
        Json::Int(i) => Some(i.to_string()),
        Json::Float(f) => Some(json::py_float(*f)),
        Json::Bool(b) => Some(if *b { "true" } else { "false" }.to_string()),
        other => Some(json::py_str(other)),
    }
}

fn child_text(node: &Json, path: &[&str]) -> Option<String> {
    child(node, path).and_then(text_of)
}

/// Python's `json.dump`-style exact rendering of a number for the ledger.
fn exact(value: &Json) -> String {
    match value {
        Json::Null => "null".to_string(),
        Json::Str(s) => s.clone(),
        Json::Int(i) => i.to_string(),
        Json::Float(f) => json::py_float(*f),
        other => json::py_str(other),
    }
}

/// Display rendering: at most four decimals, trailing zeros stripped, at least one
/// decimal, `null` kept as the word.
fn display(value: &Json) -> String {
    match value {
        Json::Null => "null".to_string(),
        Json::Int(i) => i.to_string(),
        Json::Float(f) => {
            if !f.is_finite() {
                return json::py_float(*f);
            }
            let rounded = (*f * 10000.0).round() / 10000.0;
            if rounded == 0.0 {
                return "0.0".to_string();
            }
            let mut s = format!("{rounded:.4}");
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.push('0');
            }
            s
        }
        other => json::py_str(other),
    }
}

/// `_` becomes `\_` followed by a zero-width break opportunity, so long metric and tool
/// names cannot overflow a narrow column.
fn tex_name(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '_' => out.push_str("\\_\\hspace{0pt}"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}\\hspace{0pt}"),
            '^' => out.push_str("\\textasciicircum{}\\hspace{0pt}"),
            '\\' => out.push_str("\\textbackslash{}\\hspace{0pt}"),
            c => out.push(c),
        }
    }
    out
}

/// Plain-text escaping for macro payloads (no break tricks: they are used in prose).
fn tex_plain(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '_' => out.push_str("\\_"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            c => out.push(c),
        }
    }
    out
}

/// Like `tex_name`, but for the long free-text values of the identity table: it adds break
/// opportunities after separators that occur inside identifiers (`/`, `;`, `,`, `:`) and, inside
/// a single token longer than 20 characters (a commit SHA, a digest), after every 20th character
/// --- a line break there changes no character of the value, and without it the token would run
/// past the column.
fn tex_long(text: &str) -> String {
    let mut out = String::new();
    let mut run = 0usize;
    for ch in text.chars() {
        match ch {
            '_' => {
                out.push_str("\\_\\hspace{0pt}");
                run = 0;
            }
            '%' => {
                out.push_str("\\%");
                run = 0;
            }
            '&' => {
                out.push_str("\\&");
                run = 0;
            }
            '#' => {
                out.push_str("\\#");
                run = 0;
            }
            '$' => {
                out.push_str("\\$");
                run = 0;
            }
            '{' => {
                out.push_str("\\{");
                run = 0;
            }
            '}' => {
                out.push_str("\\}");
                run = 0;
            }
            '~' => {
                out.push_str("\\textasciitilde{}\\hspace{0pt}");
                run = 0;
            }
            '^' => {
                out.push_str("\\textasciicircum{}\\hspace{0pt}");
                run = 0;
            }
            '\\' => {
                out.push_str("\\textbackslash{}\\hspace{0pt}");
                run = 0;
            }
            '/' | ';' | ',' | ':' => {
                out.push(ch);
                out.push_str("\\hspace{0pt}");
                run = 0;
            }
            c if c.is_whitespace() => {
                out.push(c);
                run = 0;
            }
            c => {
                out.push(c);
                run += 1;
                if run >= 10 {
                    out.push_str("\\hspace{0pt}");
                    run = 0;
                }
            }
        }
    }
    out
}

fn is_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

fn alias_of(name: &str) -> String {
    let short = name.strip_prefix("sint-").unwrap_or(name);
    let short = short.strip_suffix("-mcp").unwrap_or(short);
    if short.is_empty() {
        name.to_string()
    } else {
        short.to_string()
    }
}

fn parse_int_after(text: &str, marker: &str) -> Option<i64> {
    let rest = text.split_once(marker)?.1;
    let mut digits = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if digits.is_empty() {
            if ch == ' ' {
                continue;
            }
            return None;
        } else {
            break;
        }
    }
    digits.parse().ok()
}

fn clean_path(path: &str) -> String {
    let mut p = path.replace('\\', "/");
    while let Some(rest) = p.strip_prefix("./") {
        p = rest.to_string();
    }
    p
}

fn count_raw(report_path: &str) -> (String, usize) {
    let dir = match std::path::Path::new(report_path).parent() {
        Some(d) => d.join("raw"),
        None => return (String::new(), 0),
    };
    let rel = clean_path(&dir.to_string_lossy());
    let count = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x == "jsonl")
                    .unwrap_or(false)
            })
            .count(),
        Err(_) => 0,
    };
    (rel, count)
}

// ---------------------------------------------------------------- report read

fn metrics_of(scope: &Json, path_prefix: &str) -> Vec<Metric> {
    let mut out = Vec::new();
    let list = match scope.get("metrics").and_then(|m| m.as_arr()) {
        Some(items) => items,
        None => return out,
    };
    for m in list {
        let name = child_text(m, &["name"]).unwrap_or_else(|| "?".to_string());
        out.push(Metric {
            unit: child_text(m, &["unit"]).unwrap_or_else(|| "?".to_string()),
            computer: child_text(m, &["computer"]).unwrap_or_else(|| "?".to_string()),
            value: m.get("value").cloned().unwrap_or(Json::Null),
            ci_low: m.get("ci_low").cloned().unwrap_or(Json::Null),
            ci_high: m.get("ci_high").cloned().unwrap_or(Json::Null),
            n: m.get("n").cloned().unwrap_or(Json::Null),
            path: format!("{path_prefix}.metrics[name={name}]"),
            name,
        });
    }
    out
}

fn read_subject(path: &str) -> Result<Subject, String> {
    let doc = json::parse_file(path)?;
    let name = child_text(&doc, &["subject", "name"]).unwrap_or_else(|| "unknown".to_string());

    let mut families = Vec::new();
    if let Some(items) = doc.get("families").and_then(|f| f.as_arr()) {
        for fam in items {
            let id = child_text(fam, &["id"]).unwrap_or_else(|| "?".to_string());
            let tasks = child(fam, &["tasks"]).and_then(text_of).unwrap_or_default();
            let metrics = metrics_of(fam, &format!("families[id={id}]"));
            families.push(Scope {
                id,
                detail: tasks,
                metrics,
            });
        }
    }

    let mut operations = Vec::new();
    if let Some(obj) = doc.get("operations").and_then(|o| o.as_obj()) {
        for (id, op) in obj {
            let supported = child_text(op, &["supported"]).unwrap_or_default();
            let metrics = metrics_of(op, &format!("operations.{id}"));
            operations.push(Scope {
                id: id.clone(),
                detail: supported,
                metrics,
            });
        }
    }

    let mut failures = Vec::new();
    if let Some(obj) = child(&doc, &["failures", "counts_by_code"]).and_then(|c| c.as_obj()) {
        for (code, count) in obj {
            failures.push((code.clone(), count.as_f64().unwrap_or(0.0) as i64));
        }
    }

    let stats_notes = child_text(&doc, &["statistics", "notes"]).unwrap_or_default();
    let repetitions = child(&doc, &["runner", "repetitions"])
        .and_then(|r| r.as_f64())
        .unwrap_or(0.0) as i64;
    let seeds = match doc.get("runner").and_then(|r| r.get("seeds")).and_then(|s| s.as_arr()) {
        Some(items) => items
            .iter()
            .filter_map(text_of)
            .collect::<Vec<_>>()
            .join(", "),
        None => String::new(),
    };

    let mut identity = Vec::new();
    let mut transport = child_text(&doc, &["subject", "transport"]).unwrap_or_default();
    if let Some(rev) = child_text(&doc, &["protocol", "mcp_spec_revision"]) {
        transport = format!("{transport} / {rev}");
    }
    for (label, field_path) in IDENTITY_FIELDS {
        let value = if *label == "transport / MCP revision" {
            Some(transport.clone())
        } else {
            child_text(&doc, field_path)
        };
        let value = match value {
            Some(v) if !v.is_empty() => v,
            _ => "--".to_string(),
        };
        identity.push(((*label).to_string(), value));
    }
    identity.push((
        "repetitions / seeds".to_string(),
        format!("{repetitions} / {seeds}"),
    ));
    let budgets = format!(
        "{} calls, {} s wall clock",
        child_text(&doc, &["budgets", "max_calls"]).unwrap_or_else(|| "--".into()),
        child_text(&doc, &["budgets", "wall_clock_s"]).unwrap_or_else(|| "--".into())
    );
    identity.push(("budgets per repetition".to_string(), budgets));
    let ci_method = child_text(&doc, &["statistics", "ci_method"]).unwrap_or_default();
    let n_resamples = child(&doc, &["statistics", "n_resamples"]).and_then(|v| v.as_f64());
    identity.push((
        "confidence intervals".to_string(),
        format!(
            "{ci_method}, {} resamples",
            n_resamples.map(|v| v as i64).unwrap_or(0)
        ),
    ));
    identity.push((
        "failures recorded".to_string(),
        if failures.is_empty() {
            "none".to_string()
        } else {
            failures
                .iter()
                .map(|(code, count)| format!("{code} x {count}"))
                .collect::<Vec<_>>()
                .join("; ")
        },
    ));

    let (raw_dir, raw_files) = count_raw(path);
    identity.push((
        "raw responses".to_string(),
        format!("{raw_dir} ({raw_files} JSONL files)"),
    ));
    let report_path = clean_path(path);
    identity.push(("report.json".to_string(), report_path));

    Ok(Subject {
        alias: alias_of(&name),
        name,
        raw_files,
        identity,
        families,
        operations,
        failures,
        calls_issued: parse_int_after(&stats_notes, "Calls issued:"),
        tools_advertised: parse_int_after(&stats_notes, "Tool surface:"),
        repetitions,
        seeds,
        ci_method,
        n_resamples: n_resamples.map(|v| v as i64),
    })
}

// ---------------------------------------------------------------- LaTeX

/// Column widths in centimetres for a comparison table with `n` subject columns.
fn widths(n: usize) -> (f64, f64, f64) {
    let available = 16.0; // A4 minus 2x2.5 cm margins
    let label = 2.6;
    let computer = 2.6;
    let sep = 0.5644 * (n as f64 + 2.0); // 2 * \tabcolsep (8 pt) per column
    let per = ((available - label - computer - sep) / n as f64).clamp(2.6, 5.0);
    (label, per, computer)
}

fn cell(metric: Option<&Metric>) -> String {
    match metric {
        None => "--".to_string(),
        Some(m) if !m.has_value() => format!("\\textit{{null}} ($n$={})", display(&m.n)),
        Some(m) => format!(
            "{}\\newline [{}], $n$={}",
            display(&m.value),
            if m.ci_low.is_null() && m.ci_high.is_null() {
                "not defined".to_string()
            } else {
                format!("{}, {}", display(&m.ci_low), display(&m.ci_high))
            },
            display(&m.n)
        ),
    }
}

fn computer_cell(metrics: &[Option<&Metric>], subjects: &[Subject]) -> (String, bool) {
    let mut seen: Vec<&str> = Vec::new();
    for m in metrics.iter().flatten() {
        if !seen.contains(&m.computer.as_str()) {
            seen.push(&m.computer);
        }
    }
    if seen.is_empty() {
        return ("--".to_string(), false);
    }
    if seen.len() == 1 {
        return (tex_name(seen[0]), false);
    }
    let parts: Vec<String> = metrics
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            m.map(|m| format!("{}: {}", subjects[i].alias, tex_name(&m.computer)))
        })
        .collect();
    (format!("\\textit{{per subject}} ({})", parts.join("; ")), true)
}

fn comparison_table(
    out: &mut String,
    label: &str,
    caption: &str,
    subjects: &[Subject],
    ids: &[String],
    scopes: &[Vec<Scope>],
    report_notes: &mut Vec<String>,
) {
    let n = subjects.len();
    let (w_label, w_subject, w_computer) = widths(n);
    // `\small` must not appear between \begin{longtable} and the first row: longtable treats
    // anything there as content and the following \noalign-based rules (`\caption`, `\toprule`)
    // then fail with "Misplaced \noalign". A group before the environment sets the size instead.
    out.push_str("\\begingroup\\small\n");
    out.push_str(&format!(
        "\\begin{{longtable}}{{@{{}}> {{\\raggedright\\arraybackslash}}p{{{w_label:.2}cm}}"
    ));
    for _ in 0..n {
        out.push_str(&format!(
            "> {{\\raggedright\\arraybackslash}}p{{{w_subject:.2}cm}}"
        ));
    }
    out.push_str(&format!(
        "> {{\\raggedright\\arraybackslash}}p{{{w_computer:.2}cm}}@{{}}}}\n"
    ));
    out.push_str(&format!(
        "\\caption{{{caption}}}\\label{{{label}}}\\\\\n\\toprule\n"
    ));
    out.push_str("metric");
    for s in subjects {
        out.push_str(&format!(" & {}", tex_name(&s.name)));
    }
    out.push_str(" & computer \\\\\n\\midrule\n\\endfirsthead\n");
    out.push_str("\\toprule\nmetric");
    for s in subjects {
        out.push_str(&format!(" & {}", tex_name(&s.name)));
    }
    out.push_str(" & computer \\\\\n\\midrule\n\\endhead\n");

    for (idx, id) in ids.iter().enumerate() {
        // Union of metric names across subjects, in order of first appearance.
        let mut names: Vec<String> = Vec::new();
        for subject_scopes in scopes {
            if let Some(scope) = subject_scopes.iter().find(|s| &s.id == id) {
                for m in &scope.metrics {
                    if !names.contains(&m.name) {
                        names.push(m.name.clone());
                    }
                }
            }
        }
        let mut detail = String::new();
        for subject_scopes in scopes {
            if let Some(scope) = subject_scopes.iter().find(|s| &s.id == id) {
                if !scope.detail.is_empty() {
                    detail = scope.detail.clone();
                }
            }
        }
        let _ = idx;
        if names.is_empty() {
            continue;
        }
        for name in &names {
            let picked: Vec<Option<&Metric>> = scopes
                .iter()
                .map(|ss| {
                    ss.iter()
                        .find(|s| &s.id == id)
                        .and_then(|s| s.metrics.iter().find(|m| &m.name == name))
                })
                .collect();
            let unit = picked
                .iter()
                .flatten()
                .next()
                .map(|m| m.unit.clone())
                .unwrap_or_default();
            let (computer, mixed) = computer_cell(&picked, subjects);
            if mixed {
                report_notes.push(format!(
                    "{label} {id} {name}: computer differs between subjects"
                ));
            }
            out.push_str(&format!(
                "{{\\footnotesize\\texttt{{{}}} (\\textit{{{}}})}}",
                tex_name(name),
                tex_name(&unit)
            ));
            for m in &picked {
                out.push_str(&format!(" & {}", cell(*m)));
            }
            out.push_str(&format!(" & {computer} \\\\\n"));
        }
        let _ = detail;
    }
    out.push_str("\\bottomrule\n\\end{longtable}\n\\endgroup\n");
}

fn identity_table(subjects: &[Subject]) -> String {
    let mut out = String::new();
    let n = subjects.len();
    let label = 3.4;
    let per = ((16.0 - label - 0.5644 * (n as f64 + 1.0)) / n as f64).clamp(3.0, 5.2);
    // A longtable rather than a float, so that the identity of the runs is printed where the
    // section refers to it and cannot drift to the end of the document behind eleven tables.
    out.push_str("\\begingroup\\small\n");
    out.push_str(&format!(
        "\\begin{{longtable}}{{@{{}}> {{\\raggedright\\arraybackslash}}p{{{label:.2}cm}}"
    ));
    for _ in 0..n {
        out.push_str(&format!(
            "> {{\\raggedright\\arraybackslash}}p{{{per:.2}cm}}"
        ));
    }
    out.push_str(&format!("@{{}}}}\n"));
    out.push_str(
        "\\caption{Identity of the two runs: subject, pinned revision, runtime, budgets and the raw \
responses every number of this section is computed from. Every value is read from the corresponding \
\\texttt{report.json}; the table is generated by \\texttt{refslint tables}.}\n",
    );
    out.push_str("\\label{tab:run-identity}\\\\\n\\toprule\n");
    out.push_str("field");
    for s in subjects {
        out.push_str(&format!(" & {}", tex_name(&s.name)));
    }
    out.push_str(" \\\\\n\\midrule\n\\endfirsthead\n\\toprule\nfield");
    for s in subjects {
        out.push_str(&format!(" & {}", tex_name(&s.name)));
    }
    out.push_str(" \\\\\n\\midrule\n\\endhead\n");
    let rows = subjects.iter().map(|s| s.identity.len()).max().unwrap_or(0);
    for i in 0..rows {
        let label_text = subjects
            .iter()
            .filter_map(|s| s.identity.get(i))
            .map(|(l, _)| l.clone())
            .next()
            .unwrap_or_default();
        out.push_str(&tex_plain(&label_text));
        for s in subjects {
            let value = s
                .identity
                .get(i)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "--".to_string());
            if is_url(&value) {
                out.push_str(&format!(" & \\url{{{value}}}"));
            } else {
                out.push_str(&format!(" & {{\\footnotesize {}}}", tex_long(&value)));
            }
        }
        out.push_str(" \\\\\n");
    }
    out.push_str("\\bottomrule\n\\end{longtable}\n\\endgroup\n");
    out
}

// ---------------------------------------------------------------- numbers

fn numbers_tex(subjects: &[Subject], families: &[String]) -> String {
    let mut out = String::new();
    out.push_str("% Generated by `refslint tables` from reports/*/report.json -- do not edit by hand.\n");
    out.push_str("% \\val{key} -> value, \\valci{key} -> 95% CI, \\valn{key} -> n, \\valt{key} -> text.\n");
    out.push_str("\\makeatletter\n");
    let mut def = |kind: &str, key: &str, value: &str| {
        out.push_str(&format!(
            "\\expandafter\\def\\csname {kind}@{key}\\endcsname{{{value}}}\n"
        ));
    };
    for s in subjects {
        for scope in s.families.iter().chain(s.operations.iter()) {
            for m in &scope.metrics {
                let keys = [
                    format!("{}.{}.{}", s.alias, scope.id, m.name),
                    format!("{}.{}.{}", s.name, scope.id, m.name),
                ];
                for key in &keys {
                    def("nm", key, &tex_plain(&display(&m.value)));
                    def(
                        "nmc",
                        key,
                        &if m.ci_low.is_null() && m.ci_high.is_null() {
                            "not defined".to_string()
                        } else {
                            format!("[{}, {}]", display(&m.ci_low), display(&m.ci_high))
                        },
                    );
                    def("nmn", key, &tex_plain(&display(&m.n)));
                }
            }
        }
        def("nm", &format!("{}.metrics_total", s.alias), &s.metrics_total().to_string());
        def(
            "nm",
            &format!("{}.metrics_valued", s.alias),
            &s.metrics_valued().to_string(),
        );
        def(
            "nm",
            &format!("{}.metrics_null", s.alias),
            &(s.metrics_total() - s.metrics_valued()).to_string(),
        );
        def("nm", &format!("{}.repetitions", s.alias), &s.repetitions.to_string());
        def("nmt", &format!("{}.seeds", s.alias), &s.seeds);
        def("nmt", &format!("{}.ci_method", s.alias), &tex_plain(&s.ci_method));
        if let Some(v) = s.n_resamples {
            def("nm", &format!("{}.ci_resamples", s.alias), &v.to_string());
        }
        if let Some(v) = s.calls_issued {
            def("nm", &format!("{}.calls_issued", s.alias), &v.to_string());
        }
        if let Some(v) = s.tools_advertised {
            def(
                "nm",
                &format!("{}.tools_advertised", s.alias),
                &v.to_string(),
            );
        }
        def(
            "nm",
            &format!("{}.raw_files", s.alias),
            &s.raw_files.to_string(),
        );
        for (code, count) in &s.failures {
            def(
                "nm",
                &format!("{}.failures.{}", s.alias, code),
                &count.to_string(),
            );
        }
        for scope in &s.families {
            def(
                "nm",
                &format!("{}.{}.tasks", s.alias, scope.id),
                &tex_plain(&scope.detail),
            );
        }
        for scope in &s.operations {
            def(
                "nmt",
                &format!("{}.{}", s.alias, scope.id),
                &tex_plain(&scope.detail),
            );
        }
        for (label, value) in &s.identity {
            let key = format!(
                "{}.identity.{}",
                s.alias,
                label.replace([' ', '/', ','], "_")
            );
            def("nmt", &key, &tex_plain(value));
            // The same value with break opportunities, for the places where it appears in a
            // running paragraph rather than in a table column.
            def("nmtl", &key, &tex_long(value));
        }
    }
    let family_count = families.len();
    def("nm", "run.systems", &subjects.len().to_string());
    def("nm", "run.families", &family_count.to_string());
    def("nm", "run.reports", &subjects.len().to_string());
    if let Some(first) = subjects.first() {
        def("nm", "run.repetitions", &first.repetitions.to_string());
        def("nmt", "run.seeds", &first.seeds);
        def("nmt", "run.ci_method", &tex_plain(&first.ci_method));
    }
    def("nm", "run.family_tasks", &{
        subjects
            .first()
            .map(|s| {
                s.families
                    .iter()
                    .filter_map(|f| f.detail.parse::<i64>().ok())
                    .sum::<i64>()
            })
            .unwrap_or(0)
            .to_string()
    });
    out.push_str("\\makeatother\n");
    out.push_str("\\providecommand{\\val}[1]{\\ifcsname nm@#1\\endcsname\\csname nm@#1\\endcsname\\else\\textbf{??#1??}\\fi}\n");
    out.push_str("\\providecommand{\\valci}[1]{\\ifcsname nmc@#1\\endcsname\\csname nmc@#1\\endcsname\\else\\textbf{??ci:#1??}\\fi}\n");
    out.push_str("\\providecommand{\\valn}[1]{\\ifcsname nmn@#1\\endcsname\\csname nmn@#1\\endcsname\\else\\textbf{??n:#1??}\\fi}\n");
    out.push_str("\\providecommand{\\valt}[1]{\\ifcsname nmt@#1\\endcsname\\csname nmt@#1\\endcsname\\else\\textbf{??t:#1??}\\fi}\n");
    out.push_str("\\providecommand{\\valtl}[1]{\\ifcsname nmtl@#1\\endcsname\\csname nmtl@#1\\endcsname\\else\\textbf{??tl:#1??}\\fi}\n");
    out
}

fn ledger(subjects: &[Subject], reports: &[String]) -> String {
    let mut out = String::new();
    out.push_str("# Generated by `refslint tables` -- do not edit by hand.\n");
    out.push_str(&format!("# sources: {}\n", reports.join(", ")));
    out.push_str("# exact values and their JSON paths; the paper's tables print the same numbers\n");
    out.push_str("# rounded to at most four decimals for display\n");
    out.push_str("# columns: subject\tscope\tscope_id\tmetric\tunit\tjson_path\tvalue\tci_low\tci_high\tn\tcomputer\n");
    for s in subjects {
        for scope in s.families.iter().chain(s.operations.iter()) {
            let kind = if s.families.iter().any(|f| f.id == scope.id) {
                "family"
            } else {
                "operation"
            };
            for m in &scope.metrics {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    s.name,
                    kind,
                    scope.id,
                    m.name,
                    m.unit,
                    m.path,
                    exact(&m.value),
                    exact(&m.ci_low),
                    exact(&m.ci_high),
                    exact(&m.n),
                    m.computer
                ));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- run

fn discover_reports(root: &str) -> Result<Vec<String>, String> {
    let dir = std::path::Path::new(root).join("reports");
    let mut found: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries.filter_map(|e| e.ok()) {
        let candidate = entry.path().join("report.json");
        if candidate.is_file() {
            found.push(candidate.to_string_lossy().to_string().replace('\\', "/"));
        }
    }
    found.sort();
    Ok(found)
}

fn emitted(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

pub fn run(opts: &Options) -> i32 {
    let root = opts.root;
    let reports = if opts.reports.is_empty() {
        match discover_reports(root) {
            Ok(list) => list,
            Err(e) => {
                eprintln!("refslint tables: {e}");
                return 2;
            }
        }
    } else {
        opts.reports
            .iter()
            .map(|p| p.replace('\\', "/"))
            .collect()
    };
    if reports.is_empty() {
        eprintln!("refslint tables: no report found (looked for {root}/reports/*/report.json)");
        return 2;
    }

    let mut subjects = Vec::new();
    for path in &reports {
        match read_subject(path) {
            Ok(s) => subjects.push(s),
            Err(e) => {
                eprintln!("refslint tables: {path}: {e}");
                return 2;
            }
        }
    }

    // Union of family and operation ids, in order of first appearance.
    let mut family_ids: Vec<String> = Vec::new();
    for s in &subjects {
        for f in &s.families {
            if !family_ids.contains(&f.id) {
                family_ids.push(f.id.clone());
            }
        }
    }
    let mut operation_ids: Vec<String> = Vec::new();
    for s in &subjects {
        for o in &s.operations {
            if !operation_ids.contains(&o.id) {
                operation_ids.push(o.id.clone());
            }
        }
    }

    let family_scopes: Vec<Vec<Scope>> = subjects
        .iter()
        .map(|s| s.families.iter().map(clone_scope).collect())
        .collect();
    let operation_scopes: Vec<Vec<Scope>> = subjects
        .iter()
        .map(|s| s.operations.iter().map(clone_scope).collect())
        .collect();

    let mut notes: Vec<String> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();

    files.push((
        format!("{root}/tex/tables/run-identity.tex"),
        identity_table(&subjects),
    ));

    for id in &family_ids {
        let mut out = String::new();
        let tasks = subjects
            .first()
            .and_then(|s| s.families.iter().find(|f| &f.id == id))
            .map(|f| f.detail.clone())
            .unwrap_or_default();
        let repetitions = subjects.first().map(|s| s.repetitions).unwrap_or(0);
        let seeds = subjects.first().map(|s| s.seeds.clone()).unwrap_or_default();
        let caption = format!(
            "Task family {id}: every metric the protocol defines for it that \\texttt{{report.json}} carries, \
for both subjects, read side by side ({tasks} tasks per repetition, {repetitions} repetitions, seeds {seeds}). \
Each subject cell gives the value, its 95\\% bootstrap CI and $n$; \\textit{{null}} is a metric that the protocol \
defines and that the subject's tool surface cannot produce, with the reason in the report's own note for that \
metric. Values are printed rounded to at most four decimals; the unrounded values and their JSON paths are in \
\\texttt{{data/run-numbers.tsv}}. Generated by \\texttt{{refslint tables}}."
        );
        comparison_table(
            &mut out,
            &format!("tab:{}", id.to_lowercase()),
            &caption,
            &subjects,
            std::slice::from_ref(id),
            &family_scopes,
            &mut notes,
        );
        files.push((format!("{root}/tex/tables/family-{id}.tex"), out));
    }

    {
        let mut out = String::new();
        let caption = "The same profile by taxonomy operation: each operation's metrics pooled over the \
repetitions of the families that exercise it (the report states which), for both subjects side by side. \
\\texttt{supported} is the subject's own verdict for that operation. Generated by \\texttt{refslint tables}."
            .to_string();
        comparison_table(
            &mut out,
            "tab:ops-profile",
            &caption,
            &subjects,
            &operation_ids,
            &operation_scopes,
            &mut notes,
        );
        files.push((format!("{root}/tex/tables/operations.tex"), out));
    }

    files.push((format!("{root}/tex/numbers.tex"), numbers_tex(&subjects, &family_ids)));
    files.push((format!("{root}/data/run-numbers.tsv"), ledger(&subjects, &reports)));

    // ---- write first, then compare what is on disk, so that a fresh generation is not
    // reported as a set of missing files.
    if !opts.check {
        for (path, content) in &files {
            if let Err(e) = util::write_text_py(path, content) {
                eprintln!("refslint tables: {e}");
                return 2;
            }
        }
    }

    let mut stale: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for (path, content) in &files {
        let expected = emitted(content);
        match std::fs::read_to_string(path) {
            Ok(on_disk) => {
                if on_disk != expected {
                    stale.push(path.clone());
                }
            }
            Err(_) => missing.push(path.clone()),
        }
    }

    // Comment lines are not part of the text a reader sees: strip `%`-to-end-of-line so that a
    // comment mentioning `\val{...}` does not count as a use of a number.
    let text_path = format!("{root}/tex/main.tex");
    let raw = match std::fs::read_to_string(&text_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("refslint tables: cannot read {text_path}: {e}");
            return 2;
        }
    };
    let mut stripped = String::with_capacity(raw.len());
    let mut in_comment = false;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
                stripped.push(ch);
            }
            continue;
        }
        if ch == '%' {
            in_comment = true;
            continue;
        }
        if ch == '\\' {
            stripped.push(ch);
            if let Some(next) = chars.next() {
                stripped.push(next);
            }
            continue;
        }
        stripped.push(ch);
    }
    let text = stripped;
    let numbers = files
        .iter()
        .find(|(p, _)| p.ends_with("tex/numbers.tex"))
        .map(|(_, c)| c.clone())
        .unwrap_or_default();
    let mut unknown: Vec<String> = Vec::new();
    let mut used = 0usize;
    for macro_name in ["val", "valci", "valn", "valt", "valtl"] {
        let needle = format!("\\{macro_name}{{");
        let mut rest = text.as_str();
        while let Some(pos) = rest.find(&needle) {
            rest = &rest[pos + needle.len()..];
            if let Some(end) = rest.find('}') {
                let key = rest[..end].replace("\\_", "_");
                used += 1;
                if !numbers.contains(&format!("@{key}\\endcsname")) {
                    unknown.push(format!("\\{macro_name}{{{key}}}"));
                }
                rest = &rest[end..];
            }
        }
    }
    // every \input{tables/...} in the text must be a file this run produced
    let mut missing_inputs: Vec<String> = Vec::new();
    let needle = "\\input{tables/";
    let mut rest = text.as_str();
    while let Some(pos) = rest.find(needle) {
        rest = &rest[pos + needle.len()..];
        if let Some(end) = rest.find('}') {
            let name = &rest[..end];
            let path = format!("{root}/tex/tables/{name}.tex");
            if !files.iter().any(|(p, _)| *p == path) {
                missing_inputs.push(format!("tex/tables/{name}.tex"));
            }
            rest = &rest[end..];
        }
    }
    let _ = &text;

    let mut problems = 0usize;
    for path in &missing {
        eprintln!("refslint tables: MISSING {path} (run `refslint tables` without --check)");
        problems += 1;
    }
    for path in &stale {
        eprintln!("refslint tables: STALE {path} (does not match the reports)");
        problems += 1;
    }
    for key in &unknown {
        eprintln!("refslint tables: UNKNOWN NUMBER {key} in tex/main.tex (no such field in any report)");
        problems += 1;
    }
    for path in &missing_inputs {
        eprintln!("refslint tables: MISSING INPUT {path} referenced by tex/main.tex");
        problems += 1;
    }

    for note in &notes {
        println!("note: {note}");
    }
    let rows: usize = subjects.iter().map(|s| s.metrics_total()).sum();
    if opts.check {
        if problems == 0 {
            println!(
                "tables up to date: {} subjects, {} families, {} metric rows over families, {used} number lookups in tex/main.tex all resolve",
                subjects.len(),
                family_ids.len(),
                rows
            );
            0
        } else {
            println!("{problems} problem(s) in the generated tables");
            1
        }
    } else {
        println!(
            "wrote {} generated files from {} report(s): {} subjects, {} families, {} operation profiles, {} metric rows over families, {used} number lookups in tex/main.tex",
            files.len(),
            reports.len(),
            subjects.len(),
            family_ids.len(),
            operation_ids.len(),
            rows
        );
        // A write run reports the same problems it would report in --check mode, so a
        // fresh generation never hides an unresolved lookup.
        if problems > 0 {
            println!("{problems} problem(s) remain (see above)");
            return 1;
        }
        0
    }
}

fn clone_scope(scope: &Scope) -> Scope {
    Scope {
        id: scope.id.clone(),
        detail: scope.detail.clone(),
        metrics: scope.metrics.clone(),
    }
}

//! `papertool ledger` -- measurements.json -> number macros, a TSV ledger, and figure data.
//!
//! The paper never contains a typed number: the LaTeX source says `\val{margolus.n}` and
//! this subcommand writes the `\csname` that resolves it, together with the TSV row that
//! says where the value came from. `--check` fails if the files on disk are not what the
//! current `data/measurements.json` produces, which is what `verify_all.sh` runs.

use crate::json::{self, Json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Options<'a> {
    pub root: &'a str,
    pub measurements: &'a str,
    pub numbers_tex: &'a str,
    pub ledger_tsv: &'a str,
    pub figures_dir: &'a str,
    pub check: bool,
}

pub fn run(opts: &Options) -> i32 {
    match run_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("papertool ledger: {e}");
            2
        }
    }
}

fn run_inner(opts: &Options) -> Result<i32, String> {
    let root = Path::new(opts.root);
    let doc = json::parse_file(&resolve(root, opts.measurements).to_string_lossy())?;
    let runs = match doc.get("runs") {
        Some(Json::Arr(a)) => a.clone(),
        _ => return Err("measurements.json has no runs".to_string()),
    };
    // (csname key -> tex value, ledger row)
    let mut macros: Vec<(String, String)> = Vec::new();
    let mut ledger: Vec<String> = Vec::new();
    ledger.push(
        "run_id\tgroup\tmetric\tkind\tunit\tn\tstat\tvalue\tsd\tmin\tmax\tsource".to_string(),
    );
    let mut groups: BTreeMap<String, Vec<&Json>> = BTreeMap::new();
    for r in &runs {
        let run_id = str_of(r, "id");
        let group = str_of(r, "group");
        groups.entry(group.clone()).or_default().push(r);
        let metrics = match r.get("metrics") {
            Some(Json::Arr(m)) => m.clone(),
            _ => Vec::new(),
        };
        // The plan's `x` value is a parameter, not a measurement, but the prose needs it
        // ("300 ticks", "depth 48") and a typed parameter is still a typed number. Exposing it
        // here keeps tables readable without allowlist entries for every protocol constant.
        let x = str_of(r, "x");
        if !x.is_empty() {
            macros.push((format!("pv@{run_id}.x"), x.clone()));
            ledger.push(format!(
                "{run_id}\t{group}\tx\tparam\t\t1\tplan\t{x}\t\t\t\texperiments/plan.toml:[[run]] id={run_id} x"
            ));
        }
        // Harness-level statistics: peak working set (reported in MB because that is how the
        // claim is stated) and elapsed time. They are measured quantities, so they get macros
        // and ledger rows like any metric.
        for (field, key_name, scale, unit) in [
            ("peak_ws_bytes", "peak_ws_mb", 1.0 / 1_048_576.0, "MB"),
            ("peak_ws_bytes", "peak_ws_sd_mb", 1.0 / 1_048_576.0, "MB"),
            ("elapsed_s", "elapsed_s", 1.0, "s"),
        ] {
            let stat = if key_name.ends_with("sd_mb") { "sd" } else { "mean" };
            if let Some(obj) = r.get(field) {
                if let Some(v) = obj.get(stat).and_then(|v| v.as_f64()) {
                    let scaled = v * scale;
                    macros.push((format!("pv@{run_id}.{key_name}"), fmt_num(scaled)));
                    let n = obj.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                    ledger.push(format!(
                        "{run_id}\t{group}\t{key_name}\tnum\t{unit}\t{n}\t{stat}\t{}\t\t\t\tmeasurements.json:runs[id={run_id}].{field}.{stat}",
                        fmt_num(scaled)
                    ));
                }
            }
        }
        for m in &metrics {
            let name = str_of(m, "name");
            let key = format!("{run_id}.{name}");
            let unit = m.get("unit").and_then(|v| v.as_str()).unwrap_or("");
            let n = m.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
            match m.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
                "num" => {
                    let mean = num_of(m, "mean");
                    let sd = num_of(m, "sd");
                    let min = num_of(m, "min");
                    let max = num_of(m, "max");
                    macros.push((
                        format!("pv@{key}"),
                        crate::ledger::fmt_num(mean),
                    ));
                    macros.push((
                        format!("pvci@{key}"),
                        format!("{} ({})", fmt_num(mean), fmt_num(sd)),
                    ));
                    macros.push((format!("pvsd@{key}"), fmt_num(sd)));
                    macros.push((format!("pvn@{key}"), n.to_string()));
                    macros.push((format!("pvmin@{key}"), fmt_num(min)));
                    macros.push((format!("pvmax@{key}"), fmt_num(max)));
                    ledger.push(format!(
                        "{run_id}\t{group}\t{name}\t{unit_kind}\t{unit}\t{n}\tmean\t{}\t{}\t{}\t{}\tmeasurements.json:runs[id={run_id}].metrics[name={name}]",
                        fmt_num(mean),
                        fmt_num(sd),
                        fmt_num(min),
                        fmt_num(max),
                        unit_kind = "num",
                    ));
                }
                "bool" => {
                    let n = m.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                    let t = m.get("true_count").and_then(|v| v.as_i64()).unwrap_or(0);
                    macros.push((format!("pv@{key}"), format!("{t}/{n}")));
                    macros.push((format!("pvfrac@{key}"), fmt_num(t as f64 / n.max(1) as f64)));
                    macros.push((format!("pvn@{key}"), n.to_string()));
                    macros.push((format!("pvtrue@{key}"), t.to_string()));
                    ledger.push(format!(
                        "{run_id}\t{group}\t{name}\tbool\t\t{n}\ttrue_count\t{t}\t\t0\t{n}\tmeasurements.json:runs[id={run_id}].metrics[name={name}]"
                    ));
                }
                "text" => {
                    let values = match m.get("values") {
                        Some(Json::Arr(v)) => v.clone(),
                        _ => Vec::new(),
                    };
                    let first = values
                        .first()
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let unique = m.get("unique").and_then(|v| v.as_i64()).unwrap_or(0);
                    // A text metric with more than one distinct value is not a fact but a
                    // spread: expose the count, not an arbitrary member.
                    macros.push((format!("pv@{key}"), first.clone()));
                    macros.push((format!("pvuniq@{key}"), unique.to_string()));
                    ledger.push(format!(
                        "{run_id}\t{group}\t{name}\ttext\t\t{n}\tunique_values\t{unique}\t\t\t\tmeasurements.json:runs[id={run_id}].metrics[name={name}]"
                    ));
                }
                other => return Err(format!("unknown metric kind '{other}' in run {run_id}")),
            }
        }
    }

    let numbers_tex = render_numbers(&macros);
    let tsv = ledger.join("\n") + "\n";
    let mut figures: Vec<(PathBuf, String)> = Vec::new();
    for (group, rows) in &groups {
        figures.push((
            resolve(root, opts.figures_dir).join(format!("{group}.csv")),
            render_figure(rows),
        ));
    }

    if opts.check {
        return check_files(root, opts, &numbers_tex, &tsv, &figures);
    }

    write_path(root, opts.numbers_tex, &numbers_tex)?;
    write_path(root, opts.ledger_tsv, &tsv)?;
    for (path, text) in &figures {
        write_abs(path, text)?;
    }
    println!(
        "[ledger] {} number macros, {} ledger rows, {} figure files",
        macros.len(),
        ledger.len() - 1,
        figures.len()
    );
    Ok(0)
}

fn check_files(
    root: &Path,
    opts: &Options,
    numbers_tex: &str,
    tsv: &str,
    figures: &[(PathBuf, String)],
) -> Result<i32, String> {
    let mut problems = 0usize;
    let checks: Vec<(PathBuf, &str, &str)> = vec![
        (resolve(root, opts.numbers_tex), numbers_tex, "numbers.tex"),
        (resolve(root, opts.ledger_tsv), tsv, "numbers.tsv"),
    ];
    for (path, want, label) in checks {
        match std::fs::read_to_string(&path) {
            Ok(have) => {
                if have != want {
                    eprintln!("STALE: {} does not match measurements.json", label);
                    problems += 1;
                }
            }
            Err(_) => {
                eprintln!("MISSING: {} ({})", path.display(), label);
                problems += 1;
            }
        }
    }
    for (path, want) in figures {
        match std::fs::read_to_string(path) {
            Ok(have) => {
                if &have != want {
                    eprintln!("STALE: figure data {}", path.display());
                    problems += 1;
                }
            }
            Err(_) => {
                eprintln!("MISSING: figure data {}", path.display());
                problems += 1;
            }
        }
    }
    if problems > 0 {
        println!("{problems} stale generated file(s): re-run `papertool ledger`");
        Ok(1)
    } else {
        println!(
            "generated files match measurements.json ({} figure files)",
            figures.len()
        );
        Ok(0)
    }
}

fn render_numbers(macros: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push_str("% Generated by `papertool ledger` from data/measurements.json -- do not edit.\n");
    out.push_str("% Every quantitative claim in the paper resolves through one of these names.\n");
    out.push_str("\\makeatletter\n");
    let mut seen: Vec<&str> = Vec::new();
    for (name, value) in macros {
        if seen.iter().any(|s| s == name) {
            continue;
        }
        seen.push(name);
        out.push_str(&format!(
            "\\expandafter\\def\\csname {name}\\endcsname{{{}}}\n",
            escape_tex(value)
        ));
    }
    out.push_str("\\makeatother\n");
    out
}

fn escape_tex(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '_' => out.push_str("\\_"),
            '#' => out.push_str("\\#"),
            _ => out.push(c),
        }
    }
    out
}

fn render_figure(rows: &[&Json]) -> String {
    // Column set: the union of numeric metric names, plus a bool metric rendered as a
    // 0..1 fraction so a "did the check pass" sweep can be plotted.
    let mut cols: Vec<(String, String)> = Vec::new();
    for r in rows {
        if let Some(Json::Arr(metrics)) = r.get("metrics") {
            for m in metrics {
                let name = str_of(m, "name");
                let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let col = match kind {
                    "num" => Some((name.clone(), format!("{name}_mean"))),
                    "bool" => Some((name.clone(), name.clone())),
                    _ => None,
                };
                if let Some(c) = col {
                    if !cols.iter().any(|(n, _)| n == &c.0) {
                        cols.push(c);
                    }
                }
            }
        }
    }
    cols.sort();
    let mut header = vec!["run".to_string(), "x".to_string()];
    for (_, col) in &cols {
        header.push(col.clone());
        header.push(format!("{col}_sd"));
    }
    let mut lines = vec![header.join(",")];
    let mut body: Vec<(f64, String, Vec<String>)> = Vec::new();
    for r in rows {
        let run_id = str_of(r, "id");
        let x = str_of(r, "x");
        let xnum = x.parse::<f64>().unwrap_or(f64::NAN);
        let mut cells: Vec<String> = Vec::new();
        for (name, _) in &cols {
            let metric = r
                .get("metrics")
                .and_then(|m| match m {
                    Json::Arr(a) => a
                        .iter()
                        .find(|mm| mm.get("name").and_then(|v| v.as_str()) == Some(name.as_str()))
                        .cloned(),
                    _ => None,
                });
            match metric {
                Some(m) => match m.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
                    "num" => {
                        cells.push(format!("{:.6}", num_of(&m, "mean")));
                        cells.push(format!("{:.6}", num_of(&m, "sd")));
                    }
                    "bool" => {
                        let n = m.get("n").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as f64;
                        let t = m.get("true_count").and_then(|v| v.as_i64()).unwrap_or(0) as f64;
                        cells.push(format!("{:.6}", t / n));
                        cells.push("0".to_string());
                    }
                    _ => {
                        cells.push("nan".to_string());
                        cells.push("nan".to_string());
                    }
                },
                None => {
                    cells.push("nan".to_string());
                    cells.push("nan".to_string());
                }
            }
        }
        body.push((xnum, run_id, cells));
    }
    body.sort_by(|a, b| {
        if a.0.is_nan() || b.0.is_nan() {
            a.1.cmp(&b.1)
        } else {
            a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    for (xnum, run_id, cells) in body {
        let x = if xnum.is_nan() {
            "nan".to_string()
        } else {
            crate::ledger::fmt_num(xnum)
        };
        lines.push(format!("{run_id},{x},{}", cells.join(",")));
    }
    lines.join("\n") + "\n"
}

pub fn fmt_num(x: f64) -> String {
    if !x.is_finite() {
        return "nan".to_string();
    }
    let abs = x.abs();
    let decimals = if abs >= 1000.0 {
        0
    } else if abs >= 100.0 {
        1
    } else if abs >= 10.0 {
        2
    } else if abs >= 1.0 {
        3
    } else {
        4
    };
    let mut s = format!("{x:.decimals$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

fn str_of(v: &Json, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn num_of(v: &Json, key: &str) -> f64 {
    match v.get(key) {
        Some(Json::Float(f)) => *f,
        Some(Json::Int(i)) => *i as f64,
        _ => f64::NAN,
    }
}

fn resolve(root: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn write_path(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    write_abs(&resolve(root, rel), text)
}

fn write_abs(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

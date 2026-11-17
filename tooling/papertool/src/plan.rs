//! The experiment plan: a TOML-subset file describing what to run and what to record.
//!
//! Dialect (deliberately tiny, JSON-compatible escaping in strings):
//!
//! ```toml
//! [meta]
//! python = "C:/.../python.exe"
//! subject_timelock_ca_sha = "abc123"
//!
//! [[run]]
//! id = "margolus16"          # unique; every number in the paper is keyed by it
//! group = "margolus"         # runs of one group become one figure CSV
//! x = "16"                   # x position of this run in its group's figure
//! cwd = "subjects/timelock-ca"
//! cmd = ["python", "-m", "timelock_ca.cli", "margolus"]
//! seed_flag = "--seed"
//! seeds = [0, 1, 2]
//! mode = "json"              # "json" | "capture"
//! compare_to = "subjects/x/results/01.txt"  # capture mode only
//! extract = ["acc|xor_accuracy_pct|1"]      # name|literal|nth-number-after
//! env_PYTHONPATH = "src"
//! unit_xor_accuracy_pct = "percent"
//! timeout_s = 600
//! repeats = 1
//! note = "why this run exists"
//! ```

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Json,
    Capture,
}

#[derive(Clone, Debug)]
pub struct Extract {
    pub name: String,
    pub literal: String,
    pub nth: usize,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub id: String,
    pub group: String,
    pub x: String,
    pub cwd: String,
    pub cmd: Vec<String>,
    pub seeds: Vec<i64>,
    pub seed_flag: Option<String>,
    pub env: Vec<(String, String)>,
    pub mode: Mode,
    pub compare_to: Option<String>,
    pub extract: Vec<Extract>,
    pub units: BTreeMap<String, String>,
    pub timeout_s: u64,
    pub repeats: u32,
    pub note: String,
}

impl Default for Run {
    fn default() -> Self {
        Run {
            id: String::new(),
            group: String::new(),
            x: String::new(),
            cwd: ".".to_string(),
            cmd: Vec::new(),
            seeds: vec![0],
            seed_flag: Some("--seed".to_string()),
            env: Vec::new(),
            mode: Mode::Json,
            compare_to: None,
            extract: Vec::new(),
            units: BTreeMap::new(),
            timeout_s: 900,
            repeats: 1,
            note: String::new(),
        }
    }
}

pub struct Plan {
    pub meta: Vec<(String, String)>,
    pub runs: Vec<Run>,
    pub derives: Vec<Derive>,
}

/// A formula over measured aggregates; see `derive.rs`.
#[derive(Clone, Debug)]
pub struct Derive {
    pub id: String,
    pub group: String,
    pub x: String,
    pub expr: String,
    pub unit: String,
    pub note: String,
}

enum Val {
    Str(String),
    Int(i64),
    ArrStr(Vec<String>),
    ArrInt(Vec<i64>),
}

fn parse_scalar(raw: &str) -> Result<Val, String> {
    let s = raw.trim();
    if s.starts_with('[') {
        let inner = s
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
            .ok_or_else(|| format!("unterminated array: {raw}"))?;
        let parts: Vec<&str> = inner
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.is_empty() {
            return Ok(Val::ArrStr(Vec::new()));
        }
        if parts[0].starts_with('"') {
            let mut out = Vec::new();
            for p in parts {
                match parse_scalar(p)? {
                    Val::Str(s) => out.push(s),
                    _ => return Err(format!("mixed array: {raw}")),
                }
            }
            return Ok(Val::ArrStr(out));
        }
        let mut out = Vec::new();
        for p in parts {
            out.push(p.parse::<i64>().map_err(|_| format!("bad integer {p}"))?);
        }
        return Ok(Val::ArrInt(out));
    }
    if s.starts_with('"') {
        let body = s
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .ok_or_else(|| format!("unterminated string: {raw}"))?;
        return Ok(Val::Str(unescape(body)));
    }
    if let Ok(i) = s.parse::<i64>() {
        return Ok(Val::Int(i));
    }
    Err(format!("unsupported value: {raw}"))
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn load(path: &str) -> Result<Plan, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut plan = Plan {
        meta: Vec::new(),
        runs: Vec::new(),
        derives: Vec::new(),
    };
    #[derive(PartialEq)]
    enum Sec {
        None,
        Meta,
        Run,
        Derive,
    }
    let mut sec = Sec::None;
    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[run]]" {
            plan.runs.push(Run::default());
            sec = Sec::Run;
            continue;
        }
        if line == "[[derive]]" {
            plan.derives.push(Derive {
                id: String::new(),
                group: "derived".to_string(),
                x: String::new(),
                expr: String::new(),
                unit: String::new(),
                note: String::new(),
            });
            sec = Sec::Derive;
            continue;
        }
        if line == "[meta]" {
            sec = Sec::Meta;
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => return Err(format!("{path}:{}: not a key = value line", lineno + 1)),
        };
        let val = parse_scalar(value).map_err(|e| format!("{path}:{}: {e}", lineno + 1))?;
        if sec == Sec::Derive {
            let d = plan.derives.last_mut().expect("derive section");
            let s = want_str(val)?;
            match key {
                "id" => d.id = s,
                "group" => d.group = s,
                "x" => d.x = s,
                "expr" => d.expr = s,
                "unit" => d.unit = s,
                "note" => d.note = s,
                other => return Err(format!("{path}:{}: unknown derive key {other}", lineno + 1)),
            }
            continue;
        }
        match sec {
            Sec::Meta => {
                if let Val::Str(s) = val {
                    plan.meta.push((key.to_string(), s));
                }
            }
            Sec::Run => {
                let run = plan.runs.last_mut().expect("run section");
                let unit_key = key.strip_prefix("unit_");
                let env_key = key.strip_prefix("env_");
                if let Some(metric) = unit_key {
                    if let Val::Str(s) = val {
                        run.units.insert(metric.to_string(), s);
                    }
                    continue;
                }
                if let Some(env) = env_key {
                    if let Val::Str(s) = val {
                        run.env.push((env.to_string(), s));
                    }
                    continue;
                }
                match key {
                    "id" => run.id = want_str(val)?,
                    "group" => run.group = want_str(val)?,
                    "x" => run.x = want_str(val)?,
                    "cwd" => run.cwd = want_str(val)?,
                    "cmd" => run.cmd = want_strs(val)?,
                    "seeds" => {
                        run.seeds = match val {
                            Val::ArrInt(v) => v,
                            Val::Int(i) => vec![i],
                            _ => return Err(format!("{path}:{}: seeds must be an int array", lineno + 1)),
                        }
                    }
                    "seed_flag" => {
                        let flag = want_str(val)?;
                        run.seed_flag = if flag.is_empty() { None } else { Some(flag) };
                    }
                    "mode" => {
                        let m = want_str(val)?;
                        run.mode = match m.as_str() {
                            "json" => Mode::Json,
                            "capture" => Mode::Capture,
                            other => return Err(format!("{path}:{}: unknown mode {other}", lineno + 1)),
                        }
                    }
                    "compare_to" => run.compare_to = Some(want_str(val)?),
                    "extract" => {
                        for spec in want_strs(val)? {
                            let parts: Vec<&str> = spec.split('|').collect();
                            if parts.len() < 2 {
                                return Err(format!("{path}:{}: extract needs name|literal[|nth]", lineno + 1));
                            }
                            let nth = if parts.len() > 2 {
                                parts[2].trim().parse::<usize>().unwrap_or(1)
                            } else {
                                1
                            };
                            run.extract.push(Extract {
                                name: parts[0].trim().to_string(),
                                literal: parts[1].to_string(),
                                nth,
                            });
                        }
                    }
                    "timeout_s" => {
                        if let Val::Int(i) = val {
                            run.timeout_s = i.max(1) as u64;
                        }
                    }
                    "repeats" => {
                        if let Val::Int(i) = val {
                            run.repeats = i.max(1) as u32;
                        }
                    }
                    "note" => run.note = want_str(val)?,
                    other => return Err(format!("{path}:{}: unknown key {other}", lineno + 1)),
                }
            }
            Sec::None => return Err(format!("{path}:{}: key outside a section", lineno + 1)),
            // Handled above, before this match; kept so the match stays exhaustive.
            Sec::Derive => {}
        }
    }
    for run in &plan.runs {
        if run.id.is_empty() {
            return Err(format!("{path}: a [[run]] has no id"));
        }
        if run.cmd.is_empty() {
            return Err(format!("{path}: run {} has no cmd", run.id));
        }
        if run.group.is_empty() {
            return Err(format!("{path}: run {} has no group", run.id));
        }
    }
    Ok(plan)
}

fn want_str(v: Val) -> Result<String, String> {
    match v {
        Val::Str(s) => Ok(s),
        _ => Err("expected a string".to_string()),
    }
}

fn want_strs(v: Val) -> Result<Vec<String>, String> {
    match v {
        Val::ArrStr(s) => Ok(s),
        Val::Str(s) => Ok(vec![s]),
        _ => Err("expected a string array".to_string()),
    }
}

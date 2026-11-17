//! Derived quantities: `[[derive]]` entries in the plan evaluate to one more number.
//!
//! A ratio the paper wants to quote (state updates per second, a memory delta) is a
//! function of measured quantities, not a measurement itself. Rather than let such a
//! number be typed into the prose, the plan declares the formula and `papertool measure`
//! evaluates it over the aggregate means, so the value has a provenance line in the ledger
//! exactly like everything else. Only `+ - * / ( )` and numbers/references are supported,
//! deliberately: anything richer belongs in the experiment, not in the paper.
//!
//! A reference is `<run_id>.<metric>` (arrays use `[i]`, e.g. `bench.broth[0].gops`).
//! A reference to a boolean metric evaluates to its true-count fraction, and a reference to
//! a count-like integer metric to its mean.

use crate::json::Json;
use std::collections::BTreeMap;

pub fn evaluate(expr: &str, values: &BTreeMap<String, f64>) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    let mut p = Parser { tokens, pos: 0, values };
    let v = p.expr()?;
    if p.pos != p.tokens.len() {
        return Err(format!("trailing input in '{expr}'"));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ref(String),
    Op(char),
    Func(String),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if "+-*/()".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Tok::Num(
                text.parse::<f64>().map_err(|_| format!("bad number {text}"))?,
            ));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || "._[]".contains(chars[i]))
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // `sqrt(` is the only function; anything else alphabetic is a reference.
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let is_call = j < chars.len() && chars[j] == '(';
            if word == "sqrt" && is_call {
                out.push(Tok::Func(word));
            } else {
                out.push(Tok::Ref(word));
            }
            continue;
        }
        return Err(format!("unexpected character '{c}' in '{s}'"));
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: Vec<Tok>,
    pos: usize,
    values: &'a BTreeMap<String, f64>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if op != '+' && op != '-' {
                break;
            }
            self.pos += 1;
            let rhs = self.term()?;
            v = if op == '+' { v + rhs } else { v - rhs };
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.atom()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if op != '*' && op != '/' {
                break;
            }
            self.pos += 1;
            let rhs = self.atom()?;
            if op == '/' {
                if rhs == 0.0 {
                    return Err("division by zero".to_string());
                }
                v /= rhs;
            } else {
                v *= rhs;
            }
        }
        Ok(v)
    }

    fn atom(&mut self) -> Result<f64, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Tok::Ref(name)) => {
                self.pos += 1;
                self.values
                    .get(&name)
                    .copied()
                    .ok_or_else(|| format!("unknown reference '{name}'"))
            }
            Some(Tok::Op('(')) => {
                self.pos += 1;
                let v = self.expr()?;
                match self.peek() {
                    Some(Tok::Op(')')) => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => Err("missing ')'".to_string()),
                }
            }
            Some(Tok::Func(name)) => {
                self.pos += 1;
                match self.peek() {
                    Some(Tok::Op('(')) => {
                        self.pos += 1;
                        let arg = self.expr()?;
                        match self.peek() {
                            Some(Tok::Op(')')) => {
                                self.pos += 1;
                            }
                            _ => return Err("missing ')' after sqrt(".to_string()),
                        }
                        if arg < 0.0 {
                            return Err("sqrt of a negative number".to_string());
                        }
                        let _ = name;
                        Ok(arg.sqrt())
                    }
                    _ => Err("expected '(' after sqrt".to_string()),
                }
            }
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(-self.atom()?)
            }
            _ => Err("expected a number, a reference or '('".to_string()),
        }
    }
}

/// Flatten aggregate measurements into `<run>.<metric>` -> value.
pub fn collect_values(runs: &[Json]) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    for run in runs {
        let id = match run.get("id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        // Harness-level statistics (elapsed time, peak working set) are objects on the run
        // itself rather than metrics, but they are measured quantities all the same.
        for key in ["peak_ws_bytes", "elapsed_s"] {
            if let Some(obj) = run.get(key) {
                for stat in ["mean", "sd", "min", "max", "n"] {
                    if let Some(v) = obj.get(stat).and_then(|v| v.as_f64()) {
                        map.insert(format!("{id}.{key}.{stat}"), v);
                    }
                }
            }
        }
        if let Some(Json::Arr(metrics)) = run.get("metrics") {
            for m in metrics {
                let name = match m.get("name").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let kind = m.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let stat = |name: &str| m.get(name).and_then(|v| v.as_f64());
                match kind {
                    "num" => {
                        if let Some(v) = stat("mean") {
                            map.insert(format!("{id}.{name}"), v);
                        }
                        for s in ["sd", "min", "max"] {
                            if let Some(v) = stat(s) {
                                map.insert(format!("{id}.{name}.{s}"), v);
                            }
                        }
                        if let Some(n) = m.get("n").and_then(|v| v.as_f64()) {
                            map.insert(format!("{id}.{name}.n"), n);
                        }
                    }
                    "bool" => {
                        let n = m.get("n").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let t = m.get("true_count").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        if n > 0.0 {
                            map.insert(format!("{id}.{name}"), t / n);
                        }
                        map.insert(format!("{id}.{name}.true_count"), t);
                        map.insert(format!("{id}.{name}.n"), n);
                    }
                    _ => {}
                }
            }
        }
    }
    map
}

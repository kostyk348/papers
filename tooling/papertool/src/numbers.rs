//! `papertool numbers` -- the two checks that keep invented numbers out of the paper.
//!
//! 1. **Resolvable lookups.** Every `\val{run.metric}` (and its siblings `\valci`, `\valsd`,
//!    `\valn`, `\valmin`, `\valmax`, `\valfrac`, `\valtrue`, `\valuniq`) must resolve to a
//!    `\csname` written by `papertool ledger`. An unknown key is a build error, not a `??`.
//! 2. **No bare numerals in the results.** Inside the region marked
//!    `%%NUMBERS-STRICT-BEGIN` / `%%NUMBERS-STRICT-END` a digit may only appear inside a
//!    number macro. Outside it, a decimal (`0.95`) or a three-digit integer (`13824`) must be
//!    listed in `data/numeral-allowlist.tsv` with the reason it is allowed, so every number
//!    that is *not* a measurement is a stated decision rather than an oversight.

use crate::util;
use std::collections::BTreeSet;

pub struct Options<'a> {
    pub root: &'a str,
    pub numbers_tex: &'a str,
    pub allowlist: &'a str,
    pub texdir: &'a str,
}

const MACROS: &[(&str, &str)] = &[
    ("\\val{", "pv@"),
    ("\\valci{", "pvci@"),
    ("\\valsd{", "pvsd@"),
    ("\\valn{", "pvn@"),
    ("\\valmin{", "pvmin@"),
    ("\\valmax{", "pvmax@"),
    ("\\valfrac{", "pvfrac@"),
    ("\\valtrue{", "pvtrue@"),
    ("\\valuniq{", "pvuniq@"),
];

pub fn run(opts: &Options) -> i32 {
    match run_inner(opts) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("papertool numbers: {e}");
            2
        }
    }
}

fn run_inner(opts: &Options) -> Result<i32, String> {
    let numbers_path = format!("{}/{}", opts.root, opts.numbers_tex);
    let numbers = std::fs::read_to_string(&numbers_path)
        .map_err(|e| format!("{numbers_path}: {e} (run `papertool ledger` first)"))?;
    let mut defined: BTreeSet<String> = BTreeSet::new();
    for line in numbers.lines() {
        if let Some(rest) = line.strip_prefix("\\expandafter\\def\\csname ") {
            if let Some((name, _)) = rest.split_once("\\endcsname") {
                defined.insert(name.trim().to_string());
            }
        }
    }

    let allow = read_allowlist(&format!("{}/{}", opts.root, opts.allowlist))?;
    let mut problems = 0usize;
    let mut lookups = 0usize;
    let mut files: Vec<(String, String)> = Vec::new();
    let tex_root = format!("{}/{}", opts.root, opts.texdir);
    util::walk_files(&tex_root, &["target"], &["tex"], |rel, path| {
        if let Ok(text) = std::fs::read_to_string(path) {
            files.push((rel.to_string(), text));
        }
    })?;
    files.sort();

    for (rel, text) in &files {
        if rel.ends_with("numbers.tex") {
            continue;
        }
        // ---- check 1: every lookup resolves
        for (macro_name, prefix) in MACROS {
            let mut hay = text.as_str();
            while let Some(pos) = hay.find(macro_name) {
                hay = &hay[pos + macro_name.len()..];
                if let Some(end) = hay.find('}') {
                    let key = hay[..end].trim();
                    lookups += 1;
                    if !defined.contains(&format!("{prefix}{key}")) {
                        eprintln!("UNKNOWN NUMBER: {rel}: {macro_name}{key}}} has no macro in {}", opts.numbers_tex);
                        problems += 1;
                    }
                }
            }
        }
        // ---- check 2: bare numerals
        let mut strict = false;
        for (lineno, raw_line) in text.lines().enumerate() {
            let line = strip_comment(raw_line);
            if line.contains("%%NUMBERS-STRICT-BEGIN") {
                strict = true;
                continue;
            }
            if line.contains("%%NUMBERS-STRICT-END") {
                strict = false;
                continue;
            }
            let cleaned = strip_allowed_contexts(&line);
            for token in numerals(&cleaned) {
                if strict {
                    eprintln!(
                        "BARE NUMBER in strict region: {rel}:{}: '{token}' -- use a number macro",
                        lineno + 1
                    );
                    problems += 1;
                    continue;
                }
                let suspicious = token.contains('.') || token.trim_start_matches('-').len() >= 3;
                if !suspicious {
                    continue;
                }
                if !allow.contains_key(&token) {
                    eprintln!(
                        "UNLISTED NUMBER: {rel}:{}: '{token}' -- add it to {} with a reason, or \
                         replace it with a number macro",
                        lineno + 1,
                        opts.allowlist
                    );
                    problems += 1;
                }
            }
        }
    }

    if problems > 0 {
        println!("{problems} number problem(s)");
        Ok(1)
    } else {
        println!(
            "numbers: {lookups} lookups resolve, no unlisted bare numerals in {} tex file(s)",
            files.len()
        );
        Ok(0)
    }
}

fn read_allowlist(path: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((token, reason)) = line.split_once('\t') {
            if reason.trim().is_empty() {
                return Err(format!("{path}: '{token}' has no reason"));
            }
            map.insert(token.trim().to_string(), reason.trim().to_string());
        } else {
            return Err(format!("{path}: line '{line}' is not 'token<TAB>reason'"));
        }
    }
    Ok(map)
}

fn strip_comment(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(c);
            if let Some(n) = chars.next() {
                out.push(n);
            }
            continue;
        }
        if c == '%' {
            break;
        }
        out.push(c);
    }
    out
}

fn strip_allowed_contexts(line: &str) -> String {
    let mut s = line.to_string();
    // Environments, references, citations and file names are not prose numbers.
    // Commands are listed below; patterns that end in '{' are handled by the argument
    // stripper itself (it skips to the matching brace), so the bodies of \val{...} lookups
    // and citations never reach the numeral scan.
    for cmd in ["\\val{", "\\valci{", "\\valsd{", "\\valn{", "\\valfrac{", "\\valtrue{", "\\valuniq{"] {
        s = strip_cmd_arg(&s, cmd);
    }
    for cmd in [
        "\\begin{", "\\end{", "\\cite{", "\\citep{", "\\citet{", "\\ref{", "\\label{", "\\input{",
        "\\includegraphics", "\\url{", "\\href{", "\\bibliography{", "\\bibliographystyle{",
        "\\documentclass", "\\usepackage", "\\csname", "\\endcsname", "\\setlength", "\\vspace",
        "\\hspace", "\\addplot", "\\addlegendentry", "\\pgfplotsset", "\\multicolumn", "\\cmidrule",
        "\\columnwidth", "\\textwidth", "\\linewidth", "\\tolerance", "\\emergencystretch",
        "\\arrayrulewidth", "\\tabcolsep", "\\parindent", "\\baselineskip",
    ] {
        s = strip_cmd_arg(&s, cmd);
    }
    // Math is not prose either.
    s = strip_between(&s, "$", "$");
    s = strip_balanced(&s, "\\(", "\\)");
    s = strip_balanced(&s, "\\[", "\\]");
    s
}

fn strip_cmd_arg(s: &str, cmd: &str) -> String {
    let mut out = String::new();
    let mut rest = s.to_string();
    while let Some(pos) = rest.find(cmd) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + cmd.len()..];
        // A pattern that already contains the opening brace (e.g. "\cite{") has consumed it,
        // so the argument body starts immediately and is skipped up to its matching brace.
        let mut after = after.to_string();
        if cmd.ends_with('{') {
            let mut depth = 1i32;
            let mut endpos = after.len();
            for (i, c) in after.chars().enumerate() {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        endpos = i + 1;
                        break;
                    }
                }
            }
            after = after[endpos..].to_string();
        }
        let bytes: Vec<char> = after.chars().collect();
        // Optional arguments: up to two [...] groups directly after the command.
        let mut consumed = 0usize;
        while consumed < bytes.len() && bytes[consumed] == '[' {
            let close = bytes[consumed..].iter().position(|c| *c == ']');
            match close {
                Some(c) => consumed += c + 1,
                None => break,
            }
        }
        let tail: String = bytes[consumed..].iter().collect();
        if tail.starts_with('{') {
            let mut depth = 0i32;
            let mut endpos = 0usize;
            for (i, c) in tail.chars().enumerate() {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        endpos = i + 1;
                        break;
                    }
                }
            }
            rest = tail[endpos..].to_string();
        } else {
            rest = tail;
        }
    }
    out.push_str(&rest);
    out
}

fn strip_between(s: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = s.to_string();
    while let Some(a) = rest.find(open) {
        match rest[a + open.len()..].find(close) {
            Some(b) => {
                out.push_str(&rest[..a]);
                rest = rest[a + open.len() + b + close.len()..].to_string();
            }
            None => break,
        }
    }
    out.push_str(&rest);
    out
}

fn strip_balanced(s: &str, open: &str, close: &str) -> String {
    strip_between(s, open, close)
}

fn numerals(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_digit()
                    || (chars[j] == '.'
                        && j + 1 < chars.len()
                        && chars[j + 1].is_ascii_digit()))
            {
                j += 1;
            }
            out.push(chars[i..j].iter().collect::<String>());
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

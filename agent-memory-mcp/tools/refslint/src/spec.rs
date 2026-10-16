//! The curated source list.
//!
//! It used to live in `code/refs.py` as a Python literal. `refslint import-spec`
//! reads that file once and writes `code/refs.toml`; from then on the TOML file is
//! the single source of truth and every other subcommand reads it. The TOML dialect
//! is deliberately the smallest one that is still real TOML: `[[ref]]` tables with
//! five string keys, written with JSON-compatible escaping, so the file stays
//! readable and editable by hand and by any TOML reader.

use crate::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub key: String,
    pub kind: String,
    pub id: String,
    pub category: String,
    pub supports: String,
}

// ------------------------------------------------------------- reading refs.py

enum Tok {
    Ident(String),
    Str(String),
    Punct(char),
}

fn tokenize(text: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < chars.len() {
        let ch = chars[i];
        if ch == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            let quote = ch;
            i += 1;
            let mut s = String::new();
            loop {
                let c = *chars.get(i).ok_or("unterminated string literal")?;
                i += 1;
                if c == quote {
                    break;
                }
                if c == '\\' {
                    let e = *chars.get(i).ok_or("unterminated escape")?;
                    i += 1;
                    match e {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '\'' => s.push('\''),
                        '"' => s.push('"'),
                        '0' => s.push('\0'),
                        other => s.push(other),
                    }
                } else {
                    s.push(c);
                }
            }
            out.push(Tok::Str(s));
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        if "()[]=,".contains(ch) {
            out.push(Tok::Punct(ch));
            i += 1;
            continue;
        }
        i += 1;
    }
    Ok(out)
}

/// Parse the `SOURCES = [ dict(key=..., ...), ... ]` literal out of `refs.py`.
pub fn import_python_spec(path: &str) -> Result<Vec<Source>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let toks = tokenize(&text)?;
    let mut i = 0usize;
    // find SOURCES = [
    loop {
        if i + 2 >= toks.len() {
            return Err("did not find 'SOURCES = [' in the Python spec".to_string());
        }
        if let (Tok::Ident(name), Tok::Punct('='), Tok::Punct('[')) = (&toks[i], &toks[i + 1], &toks[i + 2]) {
            if name.as_str() == "SOURCES" {
                i += 3;
                break;
            }
        }
        i += 1;
    }

    let mut sources = Vec::new();
    loop {
        match toks.get(i) {
            Some(Tok::Punct(']')) => break,
            Some(Tok::Punct(',')) => {
                i += 1;
            }
            Some(Tok::Ident(word)) if word.as_str() == "dict" => {
                i += 1;
                match toks.get(i) {
                    Some(Tok::Punct('(')) => i += 1,
                    _ => return Err(format!("expected '(' after dict at token {i}")),
                }
                let mut fields: BTreeMap<String, String> = BTreeMap::new();
                loop {
                    match toks.get(i) {
                        Some(Tok::Punct(')')) => {
                            i += 1;
                            break;
                        }
                        Some(Tok::Punct(',')) => i += 1,
                        Some(Tok::Ident(name)) => {
                            let name = name.clone();
                            i += 1;
                            match toks.get(i) {
                                Some(Tok::Punct('=')) => i += 1,
                                _ => return Err(format!("expected '=' after {name} at token {i}")),
                            }
                            let mut value = String::new();
                            let mut got = false;
                            while let Some(Tok::Str(part)) = toks.get(i) {
                                value.push_str(part);
                                i += 1;
                                got = true;
                            }
                            if !got {
                                return Err(format!("field {name} has a non-string value"));
                            }
                            fields.insert(name, value);
                        }
                        other => {
                            return Err(format!(
                                "unexpected token in dict(): {}",
                                match other {
                                    None => "end of input".to_string(),
                                    Some(Tok::Ident(s)) => s.clone(),
                                    Some(Tok::Str(s)) => format!("string {s:?}"),
                                    Some(Tok::Punct(c)) => format!("'{c}'"),
                                }
                            ))
                        }
                    }
                }
                sources.push(Source {
                    key: fields.remove("key").ok_or("entry without key")?,
                    kind: fields.remove("kind").ok_or("entry without kind")?,
                    id: fields.remove("id").ok_or("entry without id")?,
                    category: fields.remove("category").ok_or("entry without category")?,
                    supports: fields.remove("supports").ok_or("entry without supports")?,
                });
            }
            None => break,
            other => {
                return Err(format!(
                    "unexpected token outside SOURCES: {:?}",
                    match other {
                        Some(Tok::Ident(s)) => s.clone(),
                        Some(Tok::Str(s)) => s.clone(),
                        Some(Tok::Punct(c)) => c.to_string(),
                        None => String::new(),
                    }
                ))
            }
        }
    }
    Ok(sources)
}

// ------------------------------------------------------------- TOML spec file

pub const TOML_HEADER: &str = "\
# Curated source list -- the single source of truth for every citation in this paper.
#
# `refslint bib` resolves each entry below against data/refmeta.json (the machine-collected
# metadata snapshot) and REFUSES to emit anything if an entry cannot be resolved, so a
# reference nobody verified cannot reach the paper.
#
# key       BibTeX key, used verbatim in the deliverables (\\cite{key}).
# kind      \"arxiv\" | \"doi\" | \"url\"
# id        arXiv id (no version) | DOI | absolute URL
# category  grouping used by the taxonomy / related-work tables
# supports  the specific claim in this paper that the source is cited for; a source that
#           cannot be tied to a claim does not belong here.
#
# Converted once from code/refs.py by `refslint import-spec`; from here on this file is
# edited by hand and the pipeline re-run.
";

pub fn write_toml(path: &str, sources: &[Source]) -> Result<(), String> {
    let mut out = String::from(TOML_HEADER);
    for s in sources {
        out.push('\n');
        out.push_str("[[ref]]\n");
        for (name, value) in [
            ("key", &s.key),
            ("kind", &s.kind),
            ("id", &s.id),
            ("category", &s.category),
            ("supports", &s.supports),
        ] {
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(&quote(value));
            out.push('\n');
        }
    }
    crate::util::write_text(path, &out)
}

/// Read back the subset written by `write_toml` (and anything a human writes in the
/// same style: `[[ref]]` tables with five `name = "value"` pairs).
pub fn read_toml(path: &str) -> Result<Vec<Source>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut sources: Vec<Source> = Vec::new();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[ref]]" {
            if !current.is_empty() {
                sources.push(finish_entry(current, lineno)?);
                current = BTreeMap::new();
            }
            continue;
        }
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| format!("{path}:{}: not a 'name = value' line: {line}", lineno + 1))?;
        let name = name.trim().to_string();
        let value = value.trim();
        let parsed = json::parse(value)
            .map_err(|e| format!("{path}:{}: cannot parse value of {name}: {e}", lineno + 1))?;
        let s = parsed
            .as_str()
            .ok_or_else(|| format!("{path}:{}: {name} is not a string", lineno + 1))?;
        current.insert(name, s.to_string());
    }
    if !current.is_empty() {
        sources.push(finish_entry(current, text.lines().count())?);
    }
    if sources.is_empty() {
        return Err(format!("{path}: no [[ref]] entries found"));
    }
    Ok(sources)
}

fn finish_entry(mut fields: BTreeMap<String, String>, lineno: usize) -> Result<Source, String> {
    Ok(Source {
        key: fields.remove("key").ok_or_else(|| format!("entry near line {lineno} has no key"))?,
        kind: fields.remove("kind").ok_or_else(|| format!("entry near line {lineno} has no kind"))?,
        id: fields.remove("id").ok_or_else(|| format!("entry near line {lineno} has no id"))?,
        category: fields
            .remove("category")
            .ok_or_else(|| format!("entry near line {lineno} has no category"))?,
        supports: fields
            .remove("supports")
            .ok_or_else(|| format!("entry near line {lineno} has no supports"))?,
    })
}

/// JSON-compatible escaping; the result is also a valid TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

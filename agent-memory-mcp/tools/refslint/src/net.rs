//! `refslint fetch | ids | doi | probe | query | abs` -- the network half of the pipeline.
//!
//! HTTPS is done by shelling out to `curl`, which keeps this binary dependency-free and
//! needs no TLS stack of its own; `curl` is a third-party CLI, allowed by the tooling rule
//! in TOOLING.md. Everything else (URL building, Atom parsing, JSON shaping) is here.

use crate::json::{self, Json};
use std::process::Command;
use std::time::Duration;

const UA_SCRIPT: &str = "scriptessa-research/1.0";
const UA_MAILTO: &str = "scriptessa-research/1.0 (mailto:konstantin@example.org)";
const UA_BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

// ---------------------------------------------------------------- URL encoding

fn is_unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.-~".contains(c)
}

/// `urllib.parse.quote(s, safe=...)`: percent-encode everything but unreserved chars.
fn percent_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::new();
    for byte in s.as_bytes() {
        let c = *byte as char;
        if is_unreserved(c) || (keep_slash && c == '/') {
            out.push(c);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `urllib.parse.urlencode`: `quote_plus`, so spaces become '+'.
fn urlencode(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| {
            let mut encoded = String::new();
            for byte in v.as_bytes() {
                let c = *byte as char;
                match c {
                    ' ' => encoded.push('+'),
                    _ if is_unreserved(c) => encoded.push(c),
                    _ => encoded.push_str(&format!("%{byte:02X}")),
                }
            }
            format!("{}={}", percent_encode(k, false), encoded)
        })
        .collect::<Vec<_>>()
        .join("&")
}

// ------------------------------------------------------------------- XML bits

fn unescape_entities(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '&' {
            let mut j = i + 1;
            let mut entity = String::new();
            while j < chars.len() && chars[j] != ';' && j - i < 12 {
                entity.push(chars[j]);
                j += 1;
            }
            if j < chars.len() && chars[j] == ';' {
                let replacement = match entity.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    other => {
                        if let Some(hex) = other.strip_prefix("#x").or_else(|| other.strip_prefix("#X")) {
                            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                        } else if let Some(dec) = other.strip_prefix('#') {
                            dec.parse::<u32>().ok().and_then(char::from_u32)
                        } else {
                            None
                        }
                    }
                };
                if let Some(c) = replacement {
                    out.push(c);
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Python's `" ".join(text.split())`.
pub fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// All `<name ...>...</name>` elements, as (attributes, inner text).
fn find_elements(xml: &str, name: &str) -> Vec<(String, String)> {
    let open_prefix = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = xml[pos..].find(&open_prefix) {
        let abs = pos + rel;
        let after_open = xml[abs + open_prefix.len()..].chars().next();
        match after_open {
            Some(c) if c == '>' || c == '/' || c.is_whitespace() => {}
            _ => {
                pos = abs + open_prefix.len();
                continue;
            }
        }
        let open_end = match xml[abs..].find('>') {
            Some(i) => abs + i,
            None => break,
        };
        let attrs = xml[abs + open_prefix.len()..open_end].trim().to_string();
        if attrs.ends_with('/') {
            out.push((attrs.trim_end_matches('/').trim().to_string(), String::new()));
            pos = open_end + 1;
            continue;
        }
        let content_start = open_end + 1;
        match xml[content_start..].find(&close) {
            Some(rel_end) => {
                let end = content_start + rel_end;
                out.push((attrs, xml[content_start..end].to_string()));
                pos = end + close.len();
            }
            None => break,
        }
    }
    out
}

fn first_element(xml: &str, name: &str) -> Option<String> {
    find_elements(xml, name).into_iter().next().map(|(_, inner)| inner)
}

fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let start = attrs.find(&needle)? + needle.len();
    let rest = &attrs[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        return Some(rest[..end].to_string());
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

/// Fields shared by the two arXiv endpoints.
struct AtomRecord {
    versioned_id: String,
    title: String,
    published: String,
    updated: String,
    authors: Vec<String>,
    summary: String,
    primary_category: Option<String>,
    categories: Vec<String>,
    doi: Option<String>,
    journal_ref: Option<String>,
}

fn parse_atom(xml: &str) -> Vec<AtomRecord> {
    let mut records = Vec::new();
    for (_, entry) in find_elements(xml, "entry") {
        let id_raw = first_element(&entry, "id").unwrap_or_default();
        let versioned_id = id_raw
            .split("/abs/")
            .last()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let title = collapse_ws(&unescape_entities(&first_element(&entry, "title").unwrap_or_default()));
        let published = unescape_entities(&first_element(&entry, "published").unwrap_or_default())
            .chars()
            .take(10)
            .collect();
        let updated = unescape_entities(&first_element(&entry, "updated").unwrap_or_default())
            .chars()
            .take(10)
            .collect();
        let authors: Vec<String> = find_elements(&entry, "author")
            .into_iter()
            .filter_map(|(_, a)| first_element(&a, "name"))
            .map(|n| unescape_entities(&n))
            .collect();
        let summary = collapse_ws(&unescape_entities(&first_element(&entry, "summary").unwrap_or_default()));
        let primary_category = find_elements(&entry, "arxiv:primary_category")
            .into_iter()
            .next()
            .and_then(|(attrs, _)| attr_value(&attrs, "term"));
        let categories: Vec<String> = find_elements(&entry, "category")
            .into_iter()
            .filter_map(|(attrs, _)| attr_value(&attrs, "term"))
            .collect();
        let doi = first_element(&entry, "arxiv:doi").map(|d| unescape_entities(&d).trim().to_string());
        let journal_ref =
            first_element(&entry, "arxiv:journal_ref").map(|d| unescape_entities(&d).trim().to_string());
        records.push(AtomRecord {
            versioned_id,
            title,
            published,
            updated,
            authors,
            summary,
            primary_category,
            categories,
            doi,
            journal_ref,
        });
    }
    records
}

// ----------------------------------------------------------------------- curl

fn curl(args: &[String]) -> Result<String, String> {
    let out = Command::new("curl")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "curl failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn http_get(url: &str, ua: &str) -> Result<String, String> {
    curl(&[
        "-sS".into(),
        "-L".into(),
        "--max-time".into(),
        "90".into(),
        "-A".into(),
        ua.to_string(),
        url.to_string(),
    ])
}

fn read_lines(path: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    Ok(text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect())
}

fn read_query_list(path: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let doc = json::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    match doc.as_arr() {
        Some(items) => Ok(items
            .iter()
            .filter_map(|i| i.as_str().map(|s| s.to_string()))
            .collect()),
        None => Err(format!("{path}: expected a JSON array of query strings")),
    }
}

fn sleep_secs(secs: f64) {
    std::thread::sleep(Duration::from_millis((secs * 1000.0) as u64));
}

fn save(path: &str, value: &Json) -> Result<(), String> {
    crate::util::write_text_py(path, &json::dump(value, 1, false))
}

// ------------------------------------------------------------------- fetch

pub fn fetch(queries_path: &str, out_path: &str, max_results: usize) -> i32 {
    let queries = match read_query_list(queries_path) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("refslint fetch: {e}");
            return 2;
        }
    };
    let mut acc: Vec<Json> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for query in &queries {
        let url = format!(
            "https://export.arxiv.org/api/query?{}",
            urlencode(&[
                ("search_query", query.clone()),
                ("max_results", max_results.to_string()),
                ("sortBy", "relevance".to_string()),
                ("sortOrder", "descending".to_string()),
            ])
        );
        let mut xml = String::new();
        for attempt in 0..4 {
            match http_get(&url, UA_SCRIPT) {
                Ok(body) => {
                    xml = body;
                    break;
                }
                Err(e) => {
                    if attempt == 3 {
                        println!("FAIL {query} {e}");
                    } else {
                        sleep_secs(4.0);
                    }
                }
            }
        }
        if xml.is_empty() {
            continue;
        }
        let records = parse_atom(&xml);
        println!("== {query} -> {}", records.len());
        for r in records {
            let base = r.versioned_id.split('v').next().unwrap_or("").to_string();
            if seen.contains(&base) {
                continue;
            }
            seen.push(base);
            println!(
                "   [{}] {} ({})",
                r.versioned_id,
                r.title.chars().take(95).collect::<String>(),
                r.published
            );
            let mut rec = Json::Obj(Vec::new());
            rec.set("arxiv_id", Json::Str(r.versioned_id.clone()));
            rec.set("title", Json::Str(r.title));
            rec.set("published", Json::Str(r.published));
            rec.set("updated", Json::Str(r.updated));
            rec.set(
                "authors",
                Json::Arr(r.authors.into_iter().map(Json::Str).collect()),
            );
            rec.set("summary", Json::Str(r.summary));
            rec.set(
                "primary_category",
                match r.primary_category {
                    Some(c) => Json::Str(c),
                    None => Json::Null,
                },
            );
            rec.set(
                "categories",
                Json::Arr(r.categories.into_iter().map(Json::Str).collect()),
            );
            rec.set("doi", match r.doi { Some(d) => Json::Str(d), None => Json::Null });
            rec.set(
                "journal_ref",
                match r.journal_ref { Some(j) => Json::Str(j), None => Json::Null },
            );
            rec.set(
                "abs_url",
                Json::Str(format!(
                    "https://arxiv.org/abs/{}",
                    r.versioned_id.split('v').next().unwrap_or("")
                )),
            );
            acc.push(rec);
        }
        sleep_secs(3.5);
    }
    let value = Json::Arr(acc);
    let n = value.as_arr().map(|a| a.len()).unwrap_or(0);
    if let Err(e) = save(out_path, &value) {
        eprintln!("refslint fetch: {e}");
        return 2;
    }
    println!("saved {n} -> {out_path}");
    0
}

// --------------------------------------------------------------------- ids

pub fn ids(ids_path: &str, out_path: &str) -> i32 {
    let ids = match read_lines(ids_path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("refslint ids: {e}");
            return 2;
        }
    };
    let mut out = Json::Obj(Vec::new());
    for chunk in ids.chunks(12) {
        let url = format!(
            "https://export.arxiv.org/api/query?{}",
            urlencode(&[
                ("id_list", chunk.join(",")),
                ("max_results", chunk.len().to_string()),
            ])
        );
        let records = match http_get(&url, UA_SCRIPT) {
            Ok(xml) => parse_atom(&xml),
            Err(e) => {
                println!("ERROR on chunk {chunk:?} {e}");
                Vec::new()
            }
        };
        for requested in chunk {
            let base = requested.split('v').next().unwrap_or(requested).to_string();
            match records.iter().find(|r| r.versioned_id.split('v').next() == Some(base.as_str())) {
                Some(r) => {
                    println!(
                        "OK   {:<14} {} | {} ({}, {} authors)",
                        requested,
                        r.title.chars().take(80).collect::<String>(),
                        r.authors.first().cloned().unwrap_or_default(),
                        r.published,
                        r.authors.len()
                    );
                    let mut rec = Json::Obj(Vec::new());
                    rec.set("requested", Json::Str(requested.clone()));
                    rec.set("arxiv_id_versioned", Json::Str(r.versioned_id.clone()));
                    rec.set("title", Json::Str(r.title.clone()));
                    rec.set("published", Json::Str(r.published.clone()));
                    rec.set("updated", Json::Str(r.updated.clone()));
                    rec.set(
                        "authors",
                        Json::Arr(r.authors.iter().cloned().map(Json::Str).collect()),
                    );
                    rec.set("n_authors", Json::Int(r.authors.len() as i64));
                    rec.set(
                        "first_author",
                        match r.authors.first() {
                            Some(a) => Json::Str(a.clone()),
                            None => Json::Null,
                        },
                    );
                    rec.set(
                        "last_author",
                        match r.authors.last() {
                            Some(a) => Json::Str(a.clone()),
                            None => Json::Null,
                        },
                    );
                    rec.set("summary", Json::Str(r.summary.clone()));
                    rec.set(
                        "primary_category",
                        match r.primary_category.clone() { Some(c) => Json::Str(c), None => Json::Null },
                    );
                    rec.set("doi", match r.doi.clone() { Some(d) => Json::Str(d), None => Json::Null });
                    rec.set(
                        "journal_ref",
                        match r.journal_ref.clone() { Some(j) => Json::Str(j), None => Json::Null },
                    );
                    rec.set("abs_url", Json::Str(format!("https://arxiv.org/abs/{}", r.versioned_id)));
                    rec.set("pdf_url", Json::Str(format!("https://arxiv.org/pdf/{}", r.versioned_id)));
                    out.set(&base, rec);
                }
                None => {
                    println!("MISS {requested:<14}");
                    let mut rec = Json::Obj(Vec::new());
                    rec.set("requested", Json::Str(requested.clone()));
                    rec.set("error", Json::Str("NOT_FOUND".to_string()));
                    out.set(&base, rec);
                }
            }
        }
        sleep_secs(3.5);
    }
    let verified = out
        .as_obj()
        .map(|m| m.iter().filter(|(_, v)| v.get("title").is_some()).count())
        .unwrap_or(0);
    if let Err(e) = save(out_path, &out) {
        eprintln!("refslint ids: {e}");
        return 2;
    }
    println!("verified {verified} / {} -> {out_path}", ids.len());
    0
}

// --------------------------------------------------------------------- doi

fn crossref_record(doi: &str) -> Result<Json, String> {
    let url = format!("https://api.crossref.org/works/{}", percent_encode(doi, true));
    let body = http_get(&url, UA_MAILTO)?;
    let doc = json::parse(&body).map_err(|e| format!("Crossref response not JSON: {e}"))?;
    let message = doc
        .get("message")
        .ok_or_else(|| "Crossref response has no 'message'".to_string())?;
    let mut rec = Json::Obj(Vec::new());
    rec.set("doi", message.get("DOI").cloned().unwrap_or(Json::Null));
    rec.set(
        "title",
        message
            .get("title")
            .and_then(|t| t.as_arr())
            .and_then(|t| t.first())
            .cloned()
            .unwrap_or(Json::Str(String::new())),
    );
    rec.set(
        "container",
        message
            .get("container-title")
            .and_then(|t| t.as_arr())
            .and_then(|t| t.first())
            .cloned()
            .unwrap_or(Json::Str(String::new())),
    );
    rec.set("publisher", message.get("publisher").cloned().unwrap_or(Json::Null));
    rec.set("type", message.get("type").cloned().unwrap_or(Json::Null));
    rec.set(
        "year",
        message
            .get("issued")
            .and_then(|i| i.get("date-parts"))
            .and_then(|d| d.as_arr())
            .and_then(|d| d.first())
            .and_then(|d| d.as_arr())
            .and_then(|d| d.first())
            .cloned()
            .unwrap_or(Json::Null),
    );
    let authors: Vec<Json> = message
        .get("author")
        .and_then(|a| a.as_arr())
        .map(|items| {
            items
                .iter()
                .take(12)
                .map(|a| {
                    let given = a.get("given").and_then(|g| g.as_str()).unwrap_or("");
                    let family = a.get("family").and_then(|f| f.as_str()).unwrap_or("");
                    let joined = [given, family]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if joined.is_empty() {
                        a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string()
                    } else {
                        joined
                    }
                })
                .map(Json::Str)
                .collect()
        })
        .unwrap_or_default();
    rec.set("authors", Json::Arr(authors));
    rec.set("volume", message.get("volume").cloned().unwrap_or(Json::Null));
    rec.set("issue", message.get("issue").cloned().unwrap_or(Json::Null));
    rec.set("page", message.get("page").cloned().unwrap_or(Json::Null));
    rec.set("url", message.get("URL").cloned().unwrap_or(Json::Null));
    Ok(rec)
}

pub fn doi(dois_path: &str, out_path: &str) -> i32 {
    let dois = match read_lines(dois_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("refslint doi: {e}");
            return 2;
        }
    };
    let mut out = Json::Obj(Vec::new());
    for d in &dois {
        match crossref_record(d) {
            Ok(rec) => {
                println!(
                    "OK   {:<45} {} | {} ({})",
                    d,
                    json_flat(rec.get("title")).chars().take(70).collect::<String>(),
                    json_flat(rec.get("container")).chars().take(40).collect::<String>(),
                    json_flat(rec.get("year"))
                );
                out.set(d, rec);
            }
            Err(e) => {
                println!("MISS {d:<45} {e}");
                let mut rec = Json::Obj(Vec::new());
                rec.set("doi", Json::Str(d.clone()));
                rec.set("error", Json::Str(e));
                out.set(d, rec);
            }
        }
        sleep_secs(1.2);
    }
    let verified = out
        .as_obj()
        .map(|m| m.iter().filter(|(_, v)| v.get("title").is_some()).count())
        .unwrap_or(0);
    if let Err(e) = save(out_path, &out) {
        eprintln!("refslint doi: {e}");
        return 2;
    }
    println!("verified {verified} / {}", dois.len());
    0
}

fn json_flat(value: Option<&Json>) -> String {
    match value {
        None => "None".to_string(),
        Some(Json::Null) => "None".to_string(),
        Some(Json::Str(s)) => s.clone(),
        Some(Json::Int(i)) => i.to_string(),
        Some(other) => json::py_str(other),
    }
}

// ------------------------------------------------------------------- probe

fn title_of(html: &str) -> String {
    let lower = html.to_lowercase();
    let start = match lower.find("<title") {
        Some(s) => s,
        None => return String::new(),
    };
    let open_end = match html[start..].find('>') {
        Some(i) => start + i + 1,
        None => return String::new(),
    };
    let close = match lower[open_end..].find("</title>") {
        Some(i) => open_end + i,
        None => return String::new(),
    };
    collapse_ws(&html[open_end..close]).chars().take(110).collect()
}

pub fn probe(urls_path: &str, out_path: &str) -> i32 {
    let urls = match read_lines(urls_path) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("refslint probe: {e}");
            return 2;
        }
    };
    let body_path = std::env::temp_dir().join(format!("refslint-probe-{}.body", std::process::id()));
    let body_arg = body_path.to_string_lossy().to_string();
    let mut out = Json::Obj(Vec::new());
    for url in &urls {
        let write_out = "-w".to_string();
        let format = "%{http_code}\t%{size_download}\t%{url_effective}".to_string();
        let result = curl(&[
            "-sS".into(),
            "-L".into(),
            "--max-time".into(),
            "45".into(),
            "-A".into(),
            UA_BROWSER.to_string(),
            "-H".into(),
            "Accept-Language: en-US,en;q=0.9".into(),
            "-o".into(),
            body_arg.clone(),
            write_out,
            format,
            url.clone(),
        ]);
        let mut rec = Json::Obj(Vec::new());
        match result {
            Ok(meta) => {
                let parts: Vec<&str> = meta.trim().split('\t').collect();
                let status: i64 = parts.first().unwrap_or(&"0").parse().unwrap_or(0);
                let body = std::fs::read(&body_path).unwrap_or_default();
                let body = &body[..body.len().min(400_000)];
                let text = String::from_utf8_lossy(body).to_string();
                let title = title_of(&text);
                rec.set("status", Json::Int(status));
                rec.set(
                    "final_url",
                    Json::Str(parts.get(2).unwrap_or(&url.as_str()).to_string()),
                );
                rec.set("bytes", Json::Int(body.len() as i64));
                rec.set("title", Json::Str(title.clone()));
                println!("{:<4} {:<8} {} | {}", status, body.len(), url, title);
            }
            Err(e) => {
                rec.set("status", Json::Null);
                rec.set("final_url", Json::Str(url.clone()));
                rec.set("bytes", Json::Int(0));
                rec.set("title", Json::Str(String::new()));
                rec.set("error", Json::Str(e.clone()));
                println!("{:<4} {:<8} {} | {}", "None", 0, url, e);
            }
        }
        out.set(url, rec);
        sleep_secs(0.6);
    }
    let _ = std::fs::remove_file(&body_path);
    if let Err(e) = save(out_path, &out) {
        eprintln!("refslint probe: {e}");
        return 2;
    }
    0
}

// ------------------------------------------------------------------- query

pub fn query(queries_path: &str, out_path: &str) -> i32 {
    let queries = match read_lines(queries_path) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("refslint query: {e}");
            return 2;
        }
    };
    let mut out = Json::Obj(Vec::new());
    for q in &queries {
        let url = format!(
            "https://api.crossref.org/works?{}",
            urlencode(&[
                ("query.bibliographic", q.clone()),
                ("rows", "3".to_string()),
                (
                    "select",
                    "DOI,title,author,issued,container-title,type,publisher".to_string(),
                ),
            ])
        );
        let mut items: Vec<Json> = Vec::new();
        match http_get(&url, UA_MAILTO).and_then(|body| json::parse(&body).map_err(|e| e.to_string())) {
            Ok(doc) => {
                if let Some(list) = doc
                    .get("message")
                    .and_then(|m| m.get("items"))
                    .and_then(|i| i.as_arr())
                {
                    for it in list {
                        let mut rec = Json::Obj(Vec::new());
                        rec.set("doi", it.get("DOI").cloned().unwrap_or(Json::Null));
                        rec.set(
                            "title",
                            it.get("title")
                                .and_then(|t| t.as_arr())
                                .and_then(|t| t.first())
                                .cloned()
                                .unwrap_or(Json::Str(String::new())),
                        );
                        rec.set(
                            "container",
                            it.get("container-title")
                                .and_then(|t| t.as_arr())
                                .and_then(|t| t.first())
                                .cloned()
                                .unwrap_or(Json::Str(String::new())),
                        );
                        rec.set(
                            "year",
                            it.get("issued")
                                .and_then(|i| i.get("date-parts"))
                                .and_then(|d| d.as_arr())
                                .and_then(|d| d.first())
                                .and_then(|d| d.as_arr())
                                .and_then(|d| d.first())
                                .cloned()
                                .unwrap_or(Json::Null),
                        );
                        rec.set("type", it.get("type").cloned().unwrap_or(Json::Null));
                        let authors: Vec<Json> = it
                            .get("author")
                            .and_then(|a| a.as_arr())
                            .map(|items| {
                                items
                                    .iter()
                                    .take(6)
                                    .map(|a| {
                                        let given = a.get("given").and_then(|g| g.as_str()).unwrap_or("");
                                        let family = a.get("family").and_then(|f| f.as_str()).unwrap_or("");
                                        Json::Str(
                                            [given, family]
                                                .iter()
                                                .filter(|s| !s.is_empty())
                                                .cloned()
                                                .collect::<Vec<_>>()
                                                .join(" "),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        rec.set("authors", Json::Arr(authors));
                        items.push(rec);
                    }
                }
            }
            Err(e) => println!("ERR {q} {e}"),
        }
        println!("Q: {q}");
        for r in &items {
            println!(
                "    {:<32} {} | {} ({})",
                json_flat(r.get("doi")),
                json_flat(r.get("title")).chars().take(75).collect::<String>(),
                json_flat(r.get("container")).chars().take(35).collect::<String>(),
                json_flat(r.get("year"))
            );
        }
        out.set(q, Json::Arr(items));
        sleep_secs(1.0);
    }
    if let Err(e) = save(out_path, &out) {
        eprintln!("refslint query: {e}");
        return 2;
    }
    0
}

// --------------------------------------------------------------------- abs

pub fn abs_dump(store_path: &str, ids_path: &str, width_chars: usize) -> i32 {
    let store = match json::parse_file(store_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("refslint abs: cannot read {store_path}: {e}");
            return 2;
        }
    };
    let ids = match read_lines(ids_path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("refslint abs: {e}");
            return 2;
        }
    };
    let arxiv = store.get("arxiv").and_then(|a| a.as_obj());
    let mut missing: Vec<String> = Vec::new();
    for id in &ids {
        let rec = arxiv.and_then(|m| m.iter().find(|(k, _)| k == id).map(|(_, v)| v));
        let rec = match rec {
            Some(r) => r,
            None => {
                missing.push(id.clone());
                continue;
            }
        };
        println!(
            "### {} | {} | {} | {}",
            id,
            json_flat(rec.get("published")),
            json_flat(rec.get("title")),
            json_flat(rec.get("primary_category"))
        );
        let authors: Vec<String> = rec
            .get("authors")
            .and_then(|a| a.as_arr())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let shown: Vec<String> = authors.iter().take(4).cloned().collect();
        println!(
            "authors: {}{}",
            shown.join(", "),
            if authors.len() > 4 { " et al." } else { "" }
        );
        let summary: String = json_flat(rec.get("summary")).chars().take(width_chars).collect();
        println!("{}", wrap(&summary, 150));
        println!();
    }
    if !missing.is_empty() {
        println!("MISSING FROM STORE: {missing:?}");
    }
    0
}

/// `textwrap.fill(text, 150)`: greedy wrapping, long words broken.
fn wrap(text: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            if current.is_empty() {
                if word.chars().count() <= width {
                    current = word.clone();
                    break;
                }
                let head: String = word.chars().take(width).collect();
                lines.push(head);
                word = word.chars().skip(width).collect();
                continue;
            }
            if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(&word);
                break;
            }
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\n")
}

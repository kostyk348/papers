//! `refslint merge` -- merge the raw API snapshots into `data/refmeta.json`.
//!
//! Inputs in the raw directory: `arxiv1.json`, `arxiv3.json`, `arxiv4.json` (arXiv search
//! dumps), `arxiv_ids.json` (id_list verification), `dois.json`, `dois2.json` (Crossref),
//! `urls.json` (HTTP probes). One record per canonical id.

use crate::json::{self, Json};
use crate::util;

fn base_id(aid: &str) -> String {
    aid.split('v').next().unwrap_or(aid).to_string()
}

fn str_field(rec: &Json, key: &str) -> Option<String> {
    match rec.get(key) {
        Some(Json::Str(s)) => Some(s.clone()),
        _ => None,
    }
}

fn arr_of_strings(rec: &Json, key: &str) -> Vec<String> {
    match rec.get(key) {
        Some(Json::Arr(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn copy_field(rec: &Json, key: &str) -> Json {
    match rec.get(key) {
        Some(Json::Str(s)) => Json::Str(s.clone()),
        Some(Json::Null) | None => Json::Null,
        Some(other) => other.clone(),
    }
}

pub fn run(raw_dir: &str, out_path: &str) -> i32 {
    let raw = raw_dir.trim_end_matches('/');
    let mut arxiv = Json::Obj(Vec::new());
    let mut doi = Json::Obj(Vec::new());
    let mut url = Json::Obj(Vec::new());

    for fname in ["arxiv1.json", "arxiv3.json", "arxiv4.json"] {
        let path = format!("{raw}/{fname}");
        let doc = match json::parse_file(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for rec in doc.as_arr().map(|a| a.iter()).into_iter().flatten() {
            let aid = match str_field(rec, "arxiv_id") {
                Some(a) => base_id(&a),
                None => continue,
            };
            let mut entry = Json::Obj(Vec::new());
            entry.set("arxiv_id", Json::Str(aid.clone()));
            entry.set("versioned_id", Json::Str(str_field(rec, "arxiv_id").unwrap_or_default()));
            entry.set("title", copy_field(rec, "title"));
            entry.set(
                "authors",
                Json::Arr(arr_of_strings(rec, "authors").into_iter().map(Json::Str).collect()),
            );
            entry.set("published", copy_field(rec, "published"));
            entry.set("updated", copy_field(rec, "updated"));
            entry.set("summary", copy_field(rec, "summary"));
            entry.set("primary_category", copy_field(rec, "primary_category"));
            entry.set("doi", copy_field(rec, "doi"));
            entry.set("journal_ref", copy_field(rec, "journal_ref"));
            entry.set("abs_url", Json::Str(format!("https://arxiv.org/abs/{aid}")));
            entry.set("source", Json::Str(fname.to_string()));
            arxiv.set(&aid, entry);
        }
    }

    let ids_path = format!("{raw}/arxiv_ids.json");
    if let Ok(doc) = json::parse_file(&ids_path) {
        if let Some(members) = doc.as_obj() {
            for (id, rec) in members.clone() {
                if rec.get("title").is_none() {
                    continue;
                }
                let mut entry = Json::Obj(Vec::new());
                entry.set("arxiv_id", Json::Str(id.clone()));
                entry.set(
                    "versioned_id",
                    Json::Str(str_field(&rec, "arxiv_id_versioned").unwrap_or_default()),
                );
                entry.set("title", copy_field(&rec, "title"));
                entry.set(
                    "authors",
                    Json::Arr(arr_of_strings(&rec, "authors").into_iter().map(Json::Str).collect()),
                );
                entry.set("published", copy_field(&rec, "published"));
                entry.set("updated", copy_field(&rec, "updated"));
                entry.set("summary", copy_field(&rec, "summary"));
                entry.set("primary_category", copy_field(&rec, "primary_category"));
                entry.set("doi", copy_field(&rec, "doi"));
                entry.set("journal_ref", copy_field(&rec, "journal_ref"));
                entry.set("abs_url", Json::Str(format!("https://arxiv.org/abs/{id}")));
                entry.set("source", Json::Str("id_list".to_string()));
                arxiv.set(&id, entry);
            }
        }
    }

    for fname in ["dois.json", "dois2.json"] {
        let path = format!("{raw}/{fname}");
        let doc = match json::parse_file(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Some(members) = doc.as_obj() {
            for (requested, rec) in members.clone() {
                if rec.get("title").is_none() {
                    continue;
                }
                let key = str_field(&rec, "doi").unwrap_or(requested);
                let mut entry = Json::Obj(Vec::new());
                entry.set("doi", Json::Str(key.clone()));
                entry.set("title", copy_field(&rec, "title"));
                entry.set(
                    "authors",
                    Json::Arr(arr_of_strings(&rec, "authors").into_iter().map(Json::Str).collect()),
                );
                entry.set("published", Json::Str(str_of(rec.get("year"))));
                entry.set("container", copy_field(&rec, "container"));
                entry.set("publisher", copy_field(&rec, "publisher"));
                entry.set("type", copy_field(&rec, "type"));
                entry.set("volume", copy_field(&rec, "volume"));
                entry.set("issue", copy_field(&rec, "issue"));
                entry.set("page", copy_field(&rec, "page"));
                entry.set("url", Json::Str(format!("https://doi.org/{key}")));
                entry.set("source", Json::Str(fname.to_string()));
                doi.set(&key, entry);
            }
        }
    }

    let urls_path = format!("{raw}/urls.json");
    if let Ok(doc) = json::parse_file(&urls_path) {
        if let Some(members) = doc.as_obj() {
            for (u, rec) in members.clone() {
                let mut entry = Json::Obj(Vec::new());
                entry.set("url", Json::Str(u.clone()));
                entry.set("status", copy_field(&rec, "status"));
                entry.set("final_url", copy_field(&rec, "final_url"));
                entry.set("title", copy_field(&rec, "title"));
                entry.set("bytes", copy_field(&rec, "bytes"));
                entry.set("source", Json::Str("urls.json".to_string()));
                url.set(&u, entry);
            }
        }
    }

    let meta = {
        let mut m = Json::Obj(Vec::new());
        m.set("generated", Json::Str(util::local_timestamp()));
        m.set("arxiv_records", Json::Int(arxiv.as_obj().map(|a| a.len()).unwrap_or(0) as i64));
        m.set("doi_records", Json::Int(doi.as_obj().map(|a| a.len()).unwrap_or(0) as i64));
        m.set("url_records", Json::Int(url.as_obj().map(|a| a.len()).unwrap_or(0) as i64));
        m.set("collector", Json::Str("refslint merge".to_string()));
        m
    };

    let mut store = Json::Obj(Vec::new());
    store.set("arxiv", arxiv);
    store.set("doi", doi);
    store.set("url", url);
    store.set("meta", meta.clone());

    let text = json::dump(&store, 1, true);
    if let Err(e) = util::write_text_py(out_path, &text) {
        eprintln!("refslint merge: {e}");
        return 2;
    }
    println!("wrote {out_path}");
    println!("{}", json::dump(&meta, 1, false));
    0
}

/// Python's `str(value)`: null prints as "None", numbers as themselves.
fn str_of(value: Option<&Json>) -> String {
    match value {
        None | Some(Json::Null) => "None".to_string(),
        Some(Json::Str(s)) => s.clone(),
        Some(Json::Int(i)) => i.to_string(),
        Some(Json::Float(f)) => json::py_float(*f),
        Some(Json::Bool(b)) => if *b { "True" } else { "False" }.to_string(),
        Some(other) => json::py_str(other),
    }
}

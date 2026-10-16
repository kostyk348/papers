//! `refslint bib` -- resolve the curated spec against the metadata snapshot and emit
//! `tex/references.bib`, `reference-verification.md` and `data/refs-verified.csv`.
//!
//! Behaviour is the behaviour of the previous `code/build_refs.py`: an unresolved
//! source refuses the build (exit 2) instead of silently dropping a reference.
//! `--legacy-banner` reproduces the pre-port header strings byte for byte, which is
//! how the port itself is checked (see TOOLING.md, acceptance criterion 2).

use crate::json::Json;
use crate::spec::Source;
use crate::util;

const SPECIALS: [(char, &str); 9] = [
    ('&', r"\&"),
    ('%', r"\%"),
    ('$', r"\$"),
    ('#', r"\#"),
    ('_', r"\_"),
    ('{', r"\{"),
    ('}', r"\}"),
    ('~', r"\textasciitilde{}"),
    ('^', r"\textasciicircum{}"),
];

fn tex_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match SPECIALS.iter().find(|(c, _)| *c == ch) {
            Some((_, rep)) => out.push_str(rep),
            None => out.push(ch),
        }
    }
    out
}

fn field<'a>(rec: &'a Json, key: &str) -> Option<&'a Json> {
    match rec.get(key) {
        Some(Json::Null) | None => None,
        Some(v) => Some(v),
    }
}

/// Python `rec.get(key)` where the value is used as a string; `None` prints as "None".
fn as_text(rec: &Json, key: &str) -> String {
    match field(rec, key) {
        None => "None".to_string(),
        Some(Json::Str(s)) => s.clone(),
        Some(Json::Int(i)) => i.to_string(),
        Some(Json::Float(f)) => crate::json::py_float(*f),
        Some(Json::Bool(b)) => if *b { "True" } else { "False" }.to_string(),
        Some(other) => crate::json::py_str(other),
    }
}

/// Python `rec.get(key) or default` (missing, null and the empty string are falsy).
fn or_default(rec: &Json, key: &str, default: &str) -> String {
    match field(rec, key) {
        Some(Json::Str(s)) if !s.is_empty() => s.clone(),
        Some(Json::Int(i)) => i.to_string(),
        Some(Json::Float(f)) => crate::json::py_float(*f),
        Some(Json::Bool(true)) => "True".to_string(),
        _ => default.to_string(),
    }
}

/// Python `rec.get(key)` truthiness for string-ish fields: missing, null and "" are falsy.
fn truthy_str(rec: &Json, key: &str) -> Option<String> {
    match field(rec, key) {
        Some(Json::Str(s)) if !s.is_empty() => Some(s.clone()),
        Some(Json::Int(i)) => Some(i.to_string()),
        Some(Json::Float(f)) => Some(crate::json::py_float(*f)),
        Some(Json::Bool(true)) => Some("True".to_string()),
        _ => None,
    }
}

fn authors(rec: &Json) -> Vec<String> {
    match field(rec, "authors") {
        Some(Json::Arr(items)) => items
            .iter()
            .filter_map(|a| a.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn first_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn arxiv_bib(key: &str, rec: &Json, verified_on: &str) -> String {
    let author_list = authors(rec);
    let author = author_list.join(" and ");
    let published = as_text(rec, "published");
    let year = first_chars(&published, 4);
    let cat = or_default(rec, "primary_category", "cs.AI");
    let arxiv_id = as_text(rec, "arxiv_id");
    [
        format!("@misc{{{key},"),
        format!("  title         = {{{}}},", tex_escape(&as_text(rec, "title"))),
        format!("  author        = {{{}}},", tex_escape(&author)),
        format!("  year          = {{{year}}},"),
        format!("  eprint        = {{{arxiv_id}}},"),
        "  archivePrefix = {arXiv},".to_string(),
        format!("  primaryClass  = {{{cat}}},"),
        format!("  url           = {{https://arxiv.org/abs/{arxiv_id}}},"),
        format!(
            "  note          = {{arXiv:{arxiv_id} [{cat}], first posted {published}; metadata verified via the arXiv API on {verified_on}}}"
        ),
        "}".to_string(),
    ]
    .join("\n")
}

fn doi_bib(key: &str, rec: &Json, verified_on: &str) -> String {
    let author_list: Vec<String> = authors(rec).into_iter().filter(|a| !a.is_empty()).collect();
    let author = if author_list.is_empty() {
        "unknown".to_string()
    } else {
        author_list.join(" and ")
    };
    let container = truthy_str(rec, "container");
    let is_book = or_default(rec, "type", "") == "book" || container.is_none();
    let mut lines = vec![
        format!("@{}{{{key},", if is_book { "book" } else { "article" }),
        format!("  title     = {{{}}},", tex_escape(&as_text(rec, "title"))),
        format!("  author    = {{{}}},", tex_escape(&author)),
        format!("  year      = {{{}}},", or_default(rec, "published", "n.d.")),
    ];
    if let Some(container) = &container {
        lines.push(format!("  journal   = {{{}}},", tex_escape(container)));
    }
    if let Some(publisher) = truthy_str(rec, "publisher") {
        lines.push(format!("  publisher = {{{}}},", tex_escape(&publisher)));
    }
    if let Some(volume) = truthy_str(rec, "volume") {
        lines.push(format!("  volume    = {{{}}},", volume));
    }
    if let Some(number) = truthy_str(rec, "number").or_else(|| truthy_str(rec, "issue")) {
        lines.push(format!("  number    = {{{number}}},"));
    }
    if let Some(page) = truthy_str(rec, "page") {
        lines.push(format!("  pages     = {{{page}}},"));
    }
    let doi = as_text(rec, "doi");
    lines.push(format!("  doi       = {{{doi}}},"));
    lines.push(format!("  url       = {{https://doi.org/{doi}}},"));
    lines.push(format!("  note      = {{DOI metadata verified via the Crossref REST API on {verified_on}}}"));
    lines.push("}".to_string());
    lines.join("\n")
}

fn url_bib(key: &str, rec: &Json, verified_on: &str) -> String {
    let url = as_text(rec, "url");
    let title = or_default(rec, "title", &url);
    let status = as_text(rec, "status");
    [
        format!("@misc{{{key},"),
        format!("  title        = {{{}}},", tex_escape(&title)),
        format!("  howpublished = {{\\url{{{url}}}}},"),
        format!("  note         = {{HTTP {status}, retrieved via direct fetch on {verified_on}}}"),
        "}".to_string(),
    ]
    .join("\n")
}

/// Render a scalar the way Python's `%s` would when interpolating it into a template.
fn json_flat(value: &Json) -> String {
    match value {
        Json::Str(s) => s.clone(),
        other => crate::json::py_str(other),
    }
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\r') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub struct Options<'a> {
    pub root: &'a str,
    pub spec_path: &'a str,
    pub meta_path: &'a str,
    pub check_only: bool,
    pub legacy_banner: bool,
}

pub fn run(opts: &Options) -> i32 {
    let spec: Vec<Source> = match crate::spec::read_toml(opts.spec_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("refslint bib: {e}");
            return 2;
        }
    };
    let store = match crate::json::parse_file(opts.meta_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("refslint bib: cannot read {}: {e}", opts.meta_path);
            return 2;
        }
    };
    let verified_on = store
        .get("meta")
        .and_then(|m| m.get("generated"))
        .and_then(|g| g.as_str())
        .unwrap_or("")
        .to_string();

    let empty: Vec<(String, Json)> = Vec::new();
    let arxiv = store.get("arxiv").and_then(|v| v.as_obj()).unwrap_or(&empty);
    let dois = store.get("doi").and_then(|v| v.as_obj()).unwrap_or(&empty);
    let urls = store.get("url").and_then(|v| v.as_obj()).unwrap_or(&empty);

    let mut resolved: Vec<(&Source, &Json)> = Vec::new();
    let mut unresolved: Vec<(&Source, &str)> = Vec::new();
    for src in &spec {
        let rec = match src.kind.as_str() {
            "arxiv" => arxiv.iter().find(|(k, _)| k == &src.id).map(|(_, v)| v),
            "doi" => dois
                .iter()
                .find(|(k, _)| k == &src.id)
                .map(|(_, v)| v)
                .or_else(|| {
                    dois.iter()
                        .find(|(k, _)| k.to_lowercase() == src.id.to_lowercase())
                        .map(|(_, v)| v)
                }),
            _ => urls.iter().find(|(k, _)| k == &src.id).map(|(_, v)| v),
        };
        let usable = match rec {
            None => false,
            Some(rec) => {
                let has_title = field(rec, "title").map(|t| !json_flat(t).is_empty()).unwrap_or(false);
                has_title || src.kind == "url"
            }
        };
        if usable {
            resolved.push((src, rec.unwrap()));
        } else {
            unresolved.push((src, src.kind.as_str()));
        }
    }

    if !unresolved.is_empty() {
        println!("UNRESOLVED SOURCES ({}) - build refused:", unresolved.len());
        for (src, kind) in &unresolved {
            println!("   {:<28} {:<6} {}", src.key, kind, src.id);
        }
        return 2;
    }

    if !opts.check_only {
        let header_1 = if opts.legacy_banner {
            "% Generated by code/build_refs.py - do not edit by hand.".to_string()
        } else {
            "% Generated by refslint (subcommand: bib) - do not edit by hand.".to_string()
        };
        let header_2 = format!(
            "% Every entry was resolved against data/refmeta.json (snapshot: {verified_on})."
        );
        let mut bib = vec![header_1, header_2, String::new()];
        for (src, rec) in &resolved {
            let entry = match src.kind.as_str() {
                "arxiv" => arxiv_bib(&src.key, rec, &verified_on),
                "doi" => doi_bib(&src.key, rec, &verified_on),
                _ => url_bib(&src.key, rec, &verified_on),
            };
            bib.push(entry);
            bib.push(String::new());
        }
        let bib_path = format!("{}/tex/references.bib", opts.root.trim_end_matches('/'));
        if let Err(e) = util::write_text_py(&bib_path, &bib.join("\n")) {
            eprintln!("refslint bib: {e}");
            return 2;
        }
    }

    struct Row {
        reference: usize,
        key: String,
        category: String,
        identifier: String,
        title: String,
        venue: String,
        supports: String,
        verified_by: String,
        verified_on: String,
        resolved: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    for (n, (src, rec)) in resolved.iter().enumerate() {
        let row = match src.kind.as_str() {
            "arxiv" => Row {
                reference: n + 1,
                key: src.key.clone(),
                category: src.category.clone(),
                identifier: format!("arXiv:{}", as_text(rec, "arxiv_id")),
                title: as_text(rec, "title"),
                venue: format!("arXiv preprint, {}", as_text(rec, "published")),
                supports: src.supports.clone(),
                verified_by: "arXiv API id_list query".to_string(),
                verified_on: verified_on.clone(),
                resolved: "yes".to_string(),
            },
            "doi" => Row {
                reference: n + 1,
                key: src.key.clone(),
                category: src.category.clone(),
                identifier: format!("doi:{}", as_text(rec, "doi")),
                title: as_text(rec, "title"),
                venue: format!(
                    "{}, {}",
                    {
                        let container = truthy_str(rec, "container").unwrap_or_default();
                        if container.is_empty() {
                            truthy_str(rec, "publisher").unwrap_or_default()
                        } else {
                            container
                        }
                    },
                    as_text(rec, "published")
                ),
                supports: src.supports.clone(),
                verified_by: "Crossref REST API /works/".to_string(),
                verified_on: verified_on.clone(),
                resolved: "yes".to_string(),
            },
            _ => Row {
                reference: n + 1,
                key: src.key.clone(),
                category: src.category.clone(),
                identifier: as_text(rec, "url"),
                title: or_default(rec, "title", "(no <title>)"),
                venue: "web resource".to_string(),
                supports: src.supports.clone(),
                verified_by: format!("HTTP GET probe, status {}", as_text(rec, "status")),
                verified_on: verified_on.clone(),
                resolved: "yes".to_string(),
            },
        };
        rows.push(row);
    }

    if !opts.check_only {
        // data/refs-verified.csv -- Python's csv.DictWriter defaults: ',', CRLF, minimal quoting.
        let mut csv = String::from("ref,key,category,identifier,title,venue,supports,verified_by,verified_on,resolved\r\n");
        for r in &rows {
            let fields = [
                r.reference.to_string(),
                r.key.clone(),
                r.category.clone(),
                r.identifier.clone(),
                r.title.clone(),
                r.venue.clone(),
                r.supports.clone(),
                r.verified_by.clone(),
                r.verified_on.clone(),
                r.resolved.clone(),
            ];
            let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
            csv.push_str(&line.join(","));
            csv.push_str("\r\n");
        }
        let csv_path = format!("{}/data/refs-verified.csv", opts.root.trim_end_matches('/'));
        if let Err(e) = util::write_text_py(&csv_path, &csv) {
            eprintln!("refslint bib: {e}");
            return 2;
        }

        let generated_line = if opts.legacy_banner {
            "Generated by `code/build_refs.py` from `code/refs.py` and the metadata snapshot".to_string()
        } else {
            "Generated by `refslint bib` from `code/refs.toml` and the metadata snapshot".to_string()
        };
        let reproduce_line = if opts.legacy_banner {
            "python code/build_refs.py --root .            # regenerate references.bib + this table".to_string()
        } else {
            "refslint bib --out-dir .                     # regenerate references.bib + this table".to_string()
        };
        let mut md = vec![
            "# Reference verification table".to_string(),
            String::new(),
            generated_line,
            format!(
                "`data/refmeta.json` (collected {verified_on}). Every row below was resolved by a machine query;"
            ),
            "no reference in this paper was typed from memory.".to_string(),
            String::new(),
            format!("- Curated sources: **{}**, unresolved: **0**", rows.len()),
            format!(
                "- Identifiers checked: {} arXiv records, {} DOIs, {} URLs",
                rows.iter().filter(|r| r.identifier.starts_with("arXiv:")).count(),
                rows.iter().filter(|r| r.identifier.starts_with("doi:")).count(),
                rows.iter().filter(|r| r.identifier.starts_with("http")).count()
            ),
            String::new(),
            "| # | key | category | identifier | title | supports | verified by |".to_string(),
            "|---|-----|----------|------------|-------|----------|-------------|".to_string(),
        ];
        for r in &rows {
            md.push(format!(
                "| {} | `{}` | {} | {} | {} | {} | {}, {} |",
                r.reference,
                r.key,
                r.category,
                r.identifier,
                r.title.replace('|', "\\|"),
                r.supports.replace('|', "\\|"),
                r.verified_by,
                first_chars(&r.verified_on, 10)
            ));
        }
        md.push(String::new());
        md.push("## Reproducing this table".to_string());
        md.push(String::new());
        md.push("```bash".to_string());
        md.push(reproduce_line);
        md.push("```".to_string());
        md.push(String::new());
        let md_path = format!("{}/reference-verification.md", opts.root.trim_end_matches('/'));
        if let Err(e) = util::write_text_py(&md_path, &md.join("\n")) {
            eprintln!("refslint bib: {e}");
            return 2;
        }
    }

    let mut categories: Vec<(String, usize)> = Vec::new();
    for r in &rows {
        match categories.iter_mut().find(|(c, _)| *c == r.category) {
            Some((_, n)) => *n += 1,
            None => categories.push((r.category.clone(), 1)),
        }
    }
    categories.sort_by(|a, b| a.0.cmp(&b.0));

    println!("resolved {} curated sources (0 unresolved)", rows.len());
    for (category, count) in &categories {
        println!("   {:<14} {}", category, count);
    }
    println!("key -> ref number map:");
    for r in &rows {
        println!("   {:<26} [{}]", r.key, r.reference);
    }
    0
}

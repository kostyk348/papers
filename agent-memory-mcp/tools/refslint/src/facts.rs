//! `refslint facts` -- read-only facts about the SINT reference-implementation repositories.
//!
//! Per repository: HEAD sha, last commit date and subject, file inventory, Python line
//! count, and the MCP tool names it exposes (a `@*.tool` decorator followed, within 200
//! characters, by a `def name(`). Nothing here writes to the repositories.

use crate::json::{self, Json};
use crate::util;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPOS: [&str; 6] = [
    "sint",
    "sint-liquid-mcp",
    "sint-epistemic-mcp",
    "sint-dream-mcp",
    "sint-prospective-mcp",
    "sint-llm-swarm",
];

fn git(args: &[&str], cwd: &Path) -> String {
    match Command::new("git").args(args).current_dir(cwd).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

/// Names of the functions decorated with `@<something>.tool` in a Python source.
fn mcp_tools(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut found: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        // identifier path: name(.name)*
        let start = i + 1;
        let mut j = start;
        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '.') {
            j += 1;
        }
        if j == start {
            i += 1;
            continue;
        }
        let path: String = chars[start..j].iter().collect();
        if !path.ends_with(".tool") {
            i = j;
            continue;
        }
        // \b: the decorator name must end at a word boundary
        let boundary = chars.get(j).map(|c| !(c.is_alphanumeric() || *c == '_')).unwrap_or(true);
        if !boundary {
            i = j;
            continue;
        }
        let tail: String = chars[j..(j + 400).min(chars.len())].iter().collect();
        if let Some((offset, name)) = find_def(&tail) {
            if offset < 200 {
                found.push(name);
            }
        }
        i = j;
    }
    found.sort();
    found.dedup();
    found
}

/// First `def <name>(` in the text, with its character offset (Python's `re.search`).
fn find_def(text: &str) -> Option<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i + 3 <= chars.len() {
        if chars[i] == 'd' && chars[i + 1] == 'e' && chars[i + 2] == 'f' {
            let mut j = i + 3;
            let ws_start = j;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j > ws_start {
                let name_start = j;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                if j > name_start {
                    let name: String = chars[name_start..j].iter().collect();
                    let mut k = j;
                    while k < chars.len() && chars[k].is_whitespace() {
                        k += 1;
                    }
                    if chars.get(k) == Some(&'(') && name.chars().next().map(|c| c.is_lowercase() || c == '_').unwrap_or(false) {
                        return Some((i, name));
                    }
                }
            }
        }
        i += 1;
    }
    None
}

pub fn run(refs_dir: &str, out_path: &str) -> i32 {
    let mut repos = Json::Obj(Vec::new());
    let mut summary: Vec<(String, String, String, usize, usize, usize)> = Vec::new();

    for repo in REPOS {
        let path = PathBuf::from(refs_dir).join(repo);
        if !path.is_dir() {
            let mut entry = Json::Obj(Vec::new());
            entry.set("error", Json::Str("missing clone".to_string()));
            repos.set(repo, entry);
            summary.push((repo.to_string(), String::new(), String::new(), 0, 0, 0));
            continue;
        }
        let head = git(&["rev-parse", "HEAD"], &path);
        let date = git(&["log", "-1", "--format=%cs"], &path);
        let subject = git(&["log", "-1", "--format=%s"], &path);

        let mut files: Vec<String> = Vec::new();
        let mut tools = Json::Obj(Vec::new());
        let mut total_lines = 0usize;
        let walk = util::walk_files(
            &path.to_string_lossy(),
            &[".git", "__pycache__", ".venv", "node_modules"],
            &[],
            |rel, full| {
                files.push(rel.to_string());
                if rel.ends_with(".py") {
                    if let Ok(bytes) = std::fs::read(full) {
                        let src = String::from_utf8_lossy(&bytes);
                        total_lines += src.matches('\n').count() + 1;
                        if src.contains("def ") {
                            let names = mcp_tools(&src);
                            if !names.is_empty() {
                                tools.set(rel, Json::Arr(names.into_iter().map(Json::Str).collect()));
                            }
                        }
                    }
                }
            },
        );
        if let Err(e) = walk {
            eprintln!("refslint facts: {e}");
            return 2;
        }
        files.sort();
        let python_files: Vec<String> = files.iter().filter(|f| f.ends_with(".py")).cloned().collect();
        let mcp_tool_count: usize = tools
            .as_obj()
            .map(|m| m.iter().map(|(_, v)| v.as_arr().map(|a| a.len()).unwrap_or(0)).sum())
            .unwrap_or(0);

        let mut entry = Json::Obj(Vec::new());
        entry.set("head_sha", Json::Str(head.clone()));
        entry.set(
            "head_sha_short",
            Json::Str(head.chars().take(12).collect::<String>()),
        );
        entry.set("last_commit_date", Json::Str(date.clone()));
        entry.set("last_commit_subject", Json::Str(subject.clone()));
        entry.set(
            "python_files",
            Json::Arr(python_files.iter().cloned().map(Json::Str).collect()),
        );
        entry.set("all_files_count", Json::Int(files.len() as i64));
        entry.set("python_line_total", Json::Int(total_lines as i64));
        entry.set("mcp_tools", tools);
        entry.set("mcp_tool_count", Json::Int(mcp_tool_count as i64));
        entry.set(
            "has_tests_dir",
            Json::Bool(files.iter().any(|f| f.starts_with("tests/"))),
        );
        entry.set(
            "license_present",
            Json::Bool(files.iter().any(|f| f.to_uppercase().starts_with("LICENSE"))),
        );
        repos.set(repo, entry);
        summary.push((
            repo.to_string(),
            head.chars().take(12).collect(),
            date,
            mcp_tool_count,
            files.len(),
            total_lines,
        ));
    }

    let mut doc = Json::Obj(Vec::new());
    doc.set("collected", Json::Str(util::local_timestamp()));
    doc.set(
        "source",
        Json::Str("git clone --depth 1 https://github.com/kostyk348/<repo>".to_string()),
    );
    doc.set("repos", repos);

    let text = json::dump(&doc, 1, true);
    if let Err(e) = util::write_text_py(out_path, &text) {
        eprintln!("refslint facts: {e}");
        return 2;
    }
    for (repo, short, date, tools, files, lines) in summary {
        println!("{repo:<22} {short} {date} tools={tools} files={files} py_lines={lines}");
    }
    println!("wrote {out_path}");
    0
}

//! `refslint cites` -- citation-key checks over the Markdown/LaTeX deliverables.
//!
//! `--check` (the default) fails if any `\cite{key}` has no curated source, and reports
//! curated sources that are never cited. `--count` prints unique keys per file.

use crate::util;

/// Extract `\cite{...}` groups from a line of text (same regex as the Python scripts).
fn cites_in(line: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    while i + 6 <= bytes.len() {
        if bytes[i] == '\\'
            && bytes[i + 1] == 'c'
            && bytes[i + 2] == 'i'
            && bytes[i + 3] == 't'
            && bytes[i + 4] == 'e'
            && bytes[i + 5] == '{'
        {
            let mut j = i + 6;
            let mut body = String::new();
            while j < bytes.len() && bytes[j] != '}' {
                body.push(bytes[j]);
                j += 1;
            }
            if j < bytes.len() {
                groups.push(body);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    groups
}

fn split_keys(group: &str) -> Vec<String> {
    group
        .split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect()
}

pub fn check(root: &str, spec_path: &str) -> i32 {
    let known: Vec<String> = match crate::spec::read_toml(spec_path) {
        Ok(sources) => sources.into_iter().map(|s| s.key).collect(),
        Err(e) => {
            eprintln!("refslint cites: {e}");
            return 2;
        }
    };
    let mut used: Vec<(String, Vec<String>)> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    let walk = util::walk_files(root, &[".git", "tools", "bad-reports", "out"], &["md", "tex"], |rel, path| {
        if let Ok(text) = std::fs::read(path) {
            let text = String::from_utf8_lossy(&text);
            for (lineno, line) in text.lines().enumerate() {
                for group in cites_in(line) {
                    for key in split_keys(&group) {
                        match used.iter_mut().find(|(k, _)| *k == key) {
                            Some((_, places)) => places.push(format!("{rel}:{}", lineno + 1)),
                            None => used.push((key.clone(), vec![format!("{rel}:{}", lineno + 1)])),
                        }
                        if !known.contains(&key) {
                            unknown.push(format!("{rel}:{} -> \\cite{{{key}}}", lineno + 1));
                        }
                    }
                }
            }
        }
    });
    if let Err(e) = walk {
        eprintln!("refslint cites: {e}");
        return 1;
    }

    println!("citation keys used: {} unique", used.len());
    let mut unused: Vec<String> = known
        .iter()
        .filter(|k| !used.iter().any(|(u, _)| u == *k))
        .cloned()
        .collect();
    unused.sort();
    if !unknown.is_empty() {
        println!("UNKNOWN KEYS ({}):", unknown.len());
        for u in &unknown {
            println!("   {u}");
        }
    }
    println!("curated keys never cited: {}", unused.len());
    for u in unused.iter().take(40) {
        println!("    (unused) {u}");
    }
    if unused.len() > 40 {
        println!("    ... {} more", unused.len() - 40);
    }
    if unknown.is_empty() {
        0
    } else {
        1
    }
}

pub fn count(root: &str) -> i32 {
    let walk = util::walk_files(
        root,
        &[".git", "tools", "legacy", "__pycache__", "bad-reports", "raw"],
        &["tex", "md"],
        |rel, path| {
            let mut keys: Vec<String> = Vec::new();
            if let Ok(text) = std::fs::read(path) {
                let text = String::from_utf8_lossy(&text);
                for line in text.lines() {
                    for group in cites_in(line) {
                        for key in split_keys(&group) {
                            if !keys.contains(&key) {
                                keys.push(key);
                            }
                        }
                    }
                }
            }
            if !keys.is_empty() {
                println!("{:<34} {} unique keys", rel, keys.len());
            }
        },
    );
    match walk {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("refslint cites: {e}");
            1
        }
    }
}

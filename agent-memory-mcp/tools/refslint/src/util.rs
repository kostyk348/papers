//! Small filesystem helpers shared by the subcommands.

use std::path::{Path, PathBuf};

/// Recursively walk `root`, skipping directories named in `skip_dirs`, and call
/// `cb((relative_path_with_forward_slashes, absolute_path))` for every file whose
/// extension is in `exts` (empty `exts` = every file).
///
/// Within a directory, entries are visited in sorted name order, matching the
/// `sorted(filenames)` loops of the Python scripts this replaces.
pub fn walk_files<F>(root: &str, skip_dirs: &[&str], exts: &[&str], mut cb: F) -> Result<(), String>
where
    F: FnMut(&str, &PathBuf),
{
    let root_path = PathBuf::from(root);
    let root_abs = root_path
        .canonicalize()
        .unwrap_or_else(|_| root_path.clone());
    walk_inner(&root_abs, &root_abs, skip_dirs, exts, &mut cb)
}

fn walk_inner<F>(
    dir: &Path,
    root: &Path,
    skip_dirs: &[&str],
    exts: &[&str],
    cb: &mut F,
) -> Result<(), String>
where
    F: FnMut(&str, &PathBuf),
{
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    // Python's os.walk is top-down: it reports the files of a directory first and only then
    // descends into subdirectories, and both the Python scripts sort the file names.
    entries.sort();
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for path in &entries {
        if path.is_dir() {
            subdirs.push(path.clone());
        }
    }
    for path in &entries {
        if path.is_dir() {
            continue;
        }
        if !exts.is_empty() {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            if !exts.iter().any(|e| e.trim_start_matches('.') == ext) {
                continue;
            }
        }
        let rel = rel_path(root, path);
        cb(&rel, path);
    }
    for path in subdirs {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if skip_dirs.contains(&name.as_str()) {
            continue;
        }
        walk_inner(&path, root, skip_dirs, exts, cb)?;
    }
    Ok(())
}

/// `os.path.relpath` + forward slashes, as the Python scripts report paths.
pub fn rel_path(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

#[allow(dead_code)]
pub fn read_text(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

pub fn write_text(path: &str, text: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Write text the way Python's `open(path, "w")` does on Windows: LF translated to CRLF.
/// The artefacts the Python pipeline produced (bibliography, verification table, CSV,
/// generated JSON) carry CRLF line endings, and byte-identical reproduction is what the
/// port is checked against - see TOOLING.md.
pub fn write_text_py(path: &str, text: &str) -> Result<(), String> {
    let translated = text.replace("\r\n", "\n").replace('\n', "\r\n");
    write_text(path, &translated)
}

/// Local wall-clock timestamp in the format `datetime.now().isoformat(timespec="seconds")`
/// produces. Rust's stdlib has no local-time conversion, so this asks the shell.
pub fn local_timestamp() -> String {
    let out = std::process::Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%S")
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => {
            // Fall back to UTC seconds since the epoch formatted by hand.
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("epoch+{secs}s")
        }
    }
}

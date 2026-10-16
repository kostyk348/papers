//! Small shared helpers: an insertion-ordered map (Python dicts are ordered and the
//! report's key order is part of its structure), SHA-256, Python-compatible rounding,
//! UTC date formatting and directory copying. Standard library only.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A map that keeps insertion order, like a Python `dict`.
///
/// Key order matters for two things in this harness: the report is written with the
/// keys in the order Python inserted them (so the ported report can be diffed against
/// the Python one), and `failures`/`counts_by_code` are ordered by first appearance.
#[derive(Clone, Debug, Default)]
pub struct OrderedMap<T> {
    entries: Vec<(String, T)>,
}

impl<T> OrderedMap<T> {
    pub const fn new() -> Self {
        OrderedMap {
            entries: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    /// `d[key] = value`, in place when the key exists (Python `update` semantics).
    pub fn set(&mut self, key: &str, value: T) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key.to_string(), value));
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl OrderedMap<i64> {
    /// `d[key] = d.get(key, 0) + 1`
    pub fn bump(&mut self, key: &str) {
        let next = self.get(key).copied().unwrap_or(0) + 1;
        self.set(key, next);
    }
}

// ---------------------------------------------------------------------------- time

/// `time.time()` — seconds since the epoch with sub-second resolution.
pub fn time_time() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -(e.duration().as_secs_f64()),
    }
}

/// `time.strftime("%Y-%m-%d", time.gmtime())`.
pub fn utc_date(days_override: Option<i64>) -> String {
    let secs = match days_override {
        Some(days) => days * 86_400,
        None => time_time() as i64,
    };
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------- numbers

/// Python's `round(x, 6)`.
///
/// Rust's `{:.6}` formatting rounds the exact binary value half-to-even, which is what
/// CPython's `round()` does too; the result is then re-read as the nearest double, again
/// as CPython does.
pub fn round6(x: f64) -> f64 {
    round_places(x, 6)
}

pub fn round3(x: f64) -> f64 {
    round_places(x, 3)
}

fn round_places(x: f64, places: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let s = format!("{x:.places$}");
    s.parse::<f64>().unwrap_or(x)
}

/// `len(s)` in Python counts characters, not bytes.
pub fn py_len(s: &str) -> usize {
    s.chars().count()
}

// ---------------------------------------------------------------------------- filesystem

pub fn ensure_dir(path: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("cannot create {path}: {e}"))
}

/// Write text the way Python's `open(path, "w")` does on Windows: LF translated to CRLF.
/// The reports and raw logs the Python harness produced carry CRLF endings, and keeping
/// them makes the ported run diff against the reference artefact line for line.
pub fn write_text_py(path: &str, text: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        let parent = parent.to_string_lossy().to_string();
        if !parent.is_empty() {
            ensure_dir(&parent)?;
        }
    }
    let translated = text.replace("\r\n", "\n").replace('\n', "\r\n");
    std::fs::write(path, translated).map_err(|e| format!("cannot write {path}: {e}"))
}

/// `shutil.rmtree(path)` (no-op when the path does not exist).
pub fn remove_dir_all(path: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|e| format!("cannot remove {path}: {e}"))
}

/// `shutil.copytree(src, dst)`.
pub fn copy_tree(src: &str, dst: &str) -> Result<(), String> {
    let src_path = Path::new(src);
    if !src_path.is_dir() {
        return Err(format!("{src} is not a directory"));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("cannot create {dst}: {e}"))?;
    for entry in std::fs::read_dir(src_path).map_err(|e| format!("cannot read {src}: {e}"))? {
        let entry = entry.map_err(|e| format!("cannot read {src}: {e}"))?;
        let from = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let to = Path::new(dst).join(&name);
        if from.is_dir() {
            copy_tree(&from.to_string_lossy(), &to.to_string_lossy())?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("cannot copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// `os.path.basename` for a path that may use either separator.
pub fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// `os.path.relpath(path, base)` with forward slashes, as the report records raw log paths.
pub fn relpath(path: &str, base: &str) -> String {
    let norm = |s: &str| s.replace('\\', "/");
    let p = norm(path);
    let b = norm(base);
    let btrim = b
        .trim_end_matches('/')
        .trim_end_matches("/.")
        .trim_end_matches('/')
        .to_string();
    if let Some(rest) = p.strip_prefix(&format!("{btrim}/")) {
        return rest.to_string();
    }
    if p == btrim {
        return String::new();
    }
    p
}

// ---------------------------------------------------------------------------- sha256

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// `hashlib.sha256(data).hexdigest()`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

//! Minimal JSON reader/writer (standard library only).
//!
//! The writer reproduces Python's `json.dump(..., indent=N, ensure_ascii=False)`
//! byte for byte for the shapes this pipeline emits (objects, arrays, strings,
//! integers, floats, booleans, null) so that ported subcommands can be checked
//! against the artefacts the Python scripts produced.
//!
//! Documented deviations: unsupported escapes inside strings are the ones JSON
//! also rejects; float formatting matches Python's `repr` for values that do not
//! need exponent notation and uses `1e-05` style exponents otherwise.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Obj(m) => m.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: Json) {
        if let Json::Obj(m) = self {
            if let Some(slot) = m.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                m.push((key.to_string(), value));
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Json> {
        if let Json::Obj(m) = self {
            if let Some(pos) = m.iter().position(|(k, _)| k == key) {
                return Some(m.remove(pos).1);
            }
        }
        None
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Python truthiness for the JSON values a server can return: `None`, `False`,
    /// `0`, `0.0`, `""`, `[]` and `{}` are falsy, everything else is truthy.
    pub fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Int(i) => *i != 0,
            Json::Float(f) => *f != 0.0,
            Json::Str(s) => !s.is_empty(),
            Json::Arr(a) => !a.is_empty(),
            Json::Obj(o) => !o.is_empty(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Int(i) => Some(*i as f64),
            Json::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_arr_mut(&mut self) -> Option<&mut Vec<Json>> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    /// Python's `type(node).__name__` for the values JSON can carry.
    pub fn type_name(&self) -> &'static str {
        match self {
            Json::Null => "NoneType",
            Json::Bool(_) => "bool",
            Json::Int(_) => "int",
            Json::Float(_) => "float",
            Json::Str(_) => "str",
            Json::Arr(_) => "list",
            Json::Obj(_) => "dict",
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Json::Bool(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Json::Int(_) | Json::Float(_))
    }
}

/// `d[k] = v` over a raw member list, in place when the key exists (Python dict update).
pub fn set_member(members: &mut Vec<(String, Json)>, key: &str, value: Json) {
    if let Some(slot) = members.iter_mut().find(|(k, _)| k == key) {
        slot.1 = value;
    } else {
        members.push((key.to_string(), value));
    }
}

pub fn parse(text: &str) -> Result<Json, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = Parser { c: &chars, i: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i < p.c.len() {
        return Err(format!("trailing data at char {}", p.i));
    }
    Ok(v)
}

pub fn parse_file(path: &str) -> Result<Json, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    parse(&text)
}

struct Parser<'a> {
    c: &'a [char],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(ch) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at char {}", ch, self.i))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            None => Err("unexpected end of input".to_string()),
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(_) => self.number(),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.c[self.i..].iter().take(word.len()).collect::<String>() == word {
            self.i += word.len();
            Ok(value)
        } else {
            Err(format!("invalid literal at char {}", self.i))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || "+-.eE".contains(ch) {
                self.i += 1;
            } else {
                break;
            }
        }
        let raw: String = self.c[start..self.i].iter().collect();
        if raw.is_empty() {
            return Err(format!("expected value at char {}", start));
        }
        if raw.contains('.') || raw.contains('e') || raw.contains('E') {
            raw.parse::<f64>()
                .map(Json::Float)
                .map_err(|_| format!("invalid number '{raw}'"))
        } else {
            raw.parse::<i64>()
                .map(Json::Int)
                .map_err(|_| format!("invalid number '{raw}'"))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let ch = self.peek().ok_or("unterminated string")?;
            self.i += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let esc = self.peek().ok_or("unterminated escape")?;
                    self.i += 1;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) {
                                if self.peek() == Some('\\') {
                                    self.i += 1;
                                    if self.peek() == Some('u') {
                                        self.i += 1;
                                        let lo = self.hex4()?;
                                        let c = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                        out.push(char::from_u32(c).unwrap_or('\u{fffd}'));
                                        continue;
                                    }
                                }
                                out.push('\u{fffd}');
                            } else {
                                out.push(char::from_u32(hi).unwrap_or('\u{fffd}'));
                            }
                        }
                        other => return Err(format!("invalid escape '\\{other}'")),
                    }
                }
                other => out.push(other),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let ch = self.peek().ok_or("truncated \\u escape")?;
            let d = ch.to_digit(16).ok_or("bad hex digit")?;
            v = v * 16 + d;
            self.i += 1;
        }
        Ok(v)
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                }
                Some(']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected ',' or ']' at char {}", self.i)),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Ok(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.expect(':')?;
            let value = self.value()?;
            members.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                }
                Some('}') => {
                    self.i += 1;
                    return Ok(Json::Obj(members));
                }
                _ => return Err(format!("expected ',' or '}}' at char {}", self.i)),
            }
        }
    }
}

// ------------------------------------------------------------------ writing

/// `indent` = spaces per level, matching Python's `json.dump(indent=n)`.
pub fn dump(value: &Json, indent: usize, sort_keys: bool) -> String {
    let mut out = String::new();
    write_value(value, indent, sort_keys, 0, &mut out);
    out
}

/// One-line dump with Python's default separators: `json.dumps(v, sort_keys=True,
/// ensure_ascii=False)`. Used for the content digests (`hashlib.sha256(json.dumps(...))`),
/// which must be computed over the same bytes the Python harness hashed.
pub fn dump_compact(value: &Json, sort_keys: bool) -> String {
    let mut out = String::new();
    write_compact(value, sort_keys, &mut out);
    out
}

fn write_compact(value: &Json, sort_keys: bool, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => out.push_str(&i.to_string()),
        Json::Float(f) => out.push_str(&py_float(*f)),
        Json::Str(s) => write_string(s, out),
        Json::Arr(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push_str(", ");
                }
                write_compact(item, sort_keys, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            let mut ordered: Vec<&(String, Json)> = members.iter().collect();
            if sort_keys {
                ordered.sort_by(|a, b| a.0.cmp(&b.0));
            }
            out.push('{');
            for (n, (key, item)) in ordered.iter().enumerate() {
                if n > 0 {
                    out.push_str(", ");
                }
                write_string(key, out);
                out.push_str(": ");
                write_compact(item, sort_keys, out);
            }
            out.push('}');
        }
    }
}

fn write_value(value: &Json, indent: usize, sort_keys: bool, level: usize, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => out.push_str(&i.to_string()),
        Json::Float(f) => out.push_str(&py_float(*f)),
        Json::Str(s) => write_string(s, out),
        Json::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                out.push('\n');
                push_pad(indent, level + 1, out);
                write_value(item, indent, sort_keys, level + 1, out);
                if n + 1 < items.len() {
                    out.push(',');
                }
            }
            out.push('\n');
            push_pad(indent, level, out);
            out.push(']');
        }
        Json::Obj(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut ordered: Vec<&(String, Json)> = members.iter().collect();
            if sort_keys {
                ordered.sort_by(|a, b| a.0.cmp(&b.0));
            }
            out.push('{');
            let n_members = ordered.len();
            for (n, (key, item)) in ordered.iter().enumerate() {
                out.push('\n');
                push_pad(indent, level + 1, out);
                write_string(key, out);
                out.push_str(": ");
                write_value(item, indent, sort_keys, level + 1, out);
                if n + 1 < n_members {
                    out.push(',');
                }
            }
            out.push('\n');
            push_pad(indent, level, out);
            out.push('}');
        }
    }
}

fn push_pad(indent: usize, level: usize, out: &mut String) {
    for _ in 0..(indent * level) {
        out.push(' ');
    }
}

/// Python's `ensure_ascii=False` string escaping.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Python's `repr()` for a float, as far as this pipeline's values need it.
pub fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let d = format!("{f:?}");
    if let Some(pos) = d.find(['e', 'E']) {
        let (mantissa, exp) = d.split_at(pos);
        let exp: i32 = exp[1..].parse().unwrap_or(0);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exp.abs());
    }
    d
}

/// Python's `repr()` for a JSON value (used in validator messages).
pub fn py_repr(value: &Json) -> String {
    match value {
        Json::Null => "None".to_string(),
        Json::Bool(true) => "True".to_string(),
        Json::Bool(false) => "False".to_string(),
        Json::Int(i) => i.to_string(),
        Json::Float(f) => py_float(*f),
        Json::Str(s) => py_str_repr(s),
        Json::Arr(items) => {
            let inner: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Json::Obj(members) => {
            let inner: Vec<String> = members
                .iter()
                .map(|(k, v)| format!("{}: {}", py_str_repr(k), py_repr(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// Python's `str()` for a JSON value: strings are bare, everything else is repr.
pub fn py_str(value: &Json) -> String {
    match value {
        Json::Str(s) => s.clone(),
        other => py_repr(other),
    }
}

fn py_str_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

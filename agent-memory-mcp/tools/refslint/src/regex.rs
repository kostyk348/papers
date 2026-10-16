//! Tiny regular-expression engine, enough for the two patterns the protocol schema uses
//! (`^[0-9]+\.[0-9]+(\.[0-9]+)?$` for a version string and `^(none|[0-9a-f]{7,40})$` for a
//! commit sha). Python's `re.match` semantics: anchored at the start, not at the end.
//!
//! Supported: literals, `.`, `[...]` and `[^...]` classes with ranges, `\d \w \s \. \\`,
//! groups `( )`, alternation `|`, quantifiers `* + ? {n} {n,} {n,m}`, `^` and `$`.

#[derive(Debug, Clone)]
pub enum Node {
    Char(char),
    Any,
    Class { negated: bool, ranges: Vec<(char, char)> },
    Group(Alt),
    Repeat { node: Box<Node>, min: usize, max: Option<usize> },
    Start,
    End,
}

#[derive(Debug, Clone)]
pub struct Alt {
    pub seqs: Vec<Vec<Node>>,
}

struct P {
    c: Vec<char>,
    i: usize,
}

pub fn compile(pattern: &str) -> Result<Alt, String> {
    let mut p = P { c: pattern.chars().collect(), i: 0 };
    let alt = p.alt()?;
    Ok(alt)
}

impl P {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn alt(&mut self) -> Result<Alt, String> {
        let mut seqs = vec![self.seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            seqs.push(self.seq()?);
        }
        Ok(Alt { seqs })
    }

    fn seq(&mut self) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        while let Some(ch) = self.peek() {
            if ch == '|' || ch == ')' {
                break;
            }
            nodes.push(self.node()?);
        }
        Ok(nodes)
    }

    fn node(&mut self) -> Result<Node, String> {
        let atom = match self.peek() {
            None => return Err("unexpected end of pattern".to_string()),
            Some('^') => {
                self.i += 1;
                Node::Start
            }
            Some('$') => {
                self.i += 1;
                Node::End
            }
            Some('.') => {
                self.i += 1;
                Node::Any
            }
            Some('(') => {
                self.i += 1;
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err("unbalanced '(' in pattern".to_string());
                }
                self.i += 1;
                Node::Group(inner)
            }
            Some('[') => {
                self.i += 1;
                let negated = if self.peek() == Some('^') {
                    self.i += 1;
                    true
                } else {
                    false
                };
                let mut ranges = Vec::new();
                let mut first = true;
                loop {
                    let ch = self.peek().ok_or("unterminated character class")?;
                    if ch == ']' && !first {
                        self.i += 1;
                        break;
                    }
                    first = false;
                    let lo = self.class_char()?;
                    if self.peek() == Some('-') && self.c.get(self.i + 1).copied() != Some(']') {
                        self.i += 1;
                        let hi = self.class_char()?;
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
                Node::Class { negated, ranges }
            }
            Some('\\') => {
                self.i += 1;
                let esc = self.peek().ok_or("trailing backslash in pattern")?;
                self.i += 1;
                match esc {
                    'd' => Node::Class { negated: false, ranges: vec![('0', '9')] },
                    'w' => Node::Class {
                        negated: false,
                        ranges: vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
                    },
                    's' => Node::Class {
                        negated: false,
                        ranges: vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r'), ('\u{b}', '\u{c}')],
                    },
                    other => Node::Char(other),
                }
            }
            Some(ch) => {
                self.i += 1;
                Node::Char(ch)
            }
        };
        // quantifier
        let quantified = match self.peek() {
            Some('*') => {
                self.i += 1;
                Some((0, None))
            }
            Some('+') => {
                self.i += 1;
                Some((1, None))
            }
            Some('?') => {
                self.i += 1;
                Some((0, Some(1)))
            }
            Some('{') => {
                let mut j = self.i + 1;
                let mut min = String::new();
                while matches!(self.c.get(j), Some(d) if d.is_ascii_digit()) {
                    min.push(self.c[j]);
                    j += 1;
                }
                let min_v: usize = min.parse().map_err(|_| "bad {n} quantifier".to_string())?;
                let max_v = if self.c.get(j) == Some(&',') {
                    j += 1;
                    let mut max = String::new();
                    while matches!(self.c.get(j), Some(d) if d.is_ascii_digit()) {
                        max.push(self.c[j]);
                        j += 1;
                    }
                    if max.is_empty() {
                        None
                    } else {
                        Some(max.parse::<usize>().map_err(|_| "bad {n,m} quantifier".to_string())?)
                    }
                } else {
                    Some(min_v)
                };
                if self.c.get(j) != Some(&'}') {
                    return Err("unterminated {n,m} quantifier".to_string());
                }
                self.i = j + 1;
                Some((min_v, max_v))
            }
            _ => None,
        };
        match quantified {
            None => Ok(atom),
            Some((min, max)) => Ok(Node::Repeat { node: Box::new(atom), min, max }),
        }
    }

    fn class_char(&mut self) -> Result<char, String> {
        let ch = self.peek().ok_or("unterminated character class")?;
        if ch == '\\' {
            self.i += 1;
            let esc = self.peek().ok_or("trailing backslash in class")?;
            self.i += 1;
            Ok(match esc {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                other => other,
            })
        } else {
            self.i += 1;
            Ok(ch)
        }
    }
}

fn match_node(node: &Node, text: &[char], pos: usize) -> Vec<usize> {
    match node {
        Node::Char(c) => {
            if text.get(pos) == Some(c) {
                vec![pos + 1]
            } else {
                Vec::new()
            }
        }
        Node::Any => {
            if pos < text.len() {
                vec![pos + 1]
            } else {
                Vec::new()
            }
        }
        Node::Class { negated, ranges } => match text.get(pos) {
            None => Vec::new(),
            Some(ch) => {
                let inside = ranges.iter().any(|(lo, hi)| ch >= lo && ch <= hi);
                if inside != *negated {
                    vec![pos + 1]
                } else {
                    Vec::new()
                }
            }
        },
        Node::Start => {
            if pos == 0 {
                vec![pos]
            } else {
                Vec::new()
            }
        }
        Node::End => {
            if pos == text.len() {
                vec![pos]
            } else {
                Vec::new()
            }
        }
        Node::Group(alt) => match_alt(alt, text, pos),
        Node::Repeat { node, min, max } => {
            let mut results: Vec<usize> = Vec::new();
            let mut frontier = vec![pos];
            let mut count = 0usize;
            loop {
                if count >= *min {
                    for p in &frontier {
                        if !results.contains(p) {
                            results.push(*p);
                        }
                    }
                }
                if let Some(m) = max {
                    if count >= *m {
                        break;
                    }
                }
                if count > text.len() + 1 {
                    break;
                }
                let mut next: Vec<usize> = Vec::new();
                for p in &frontier {
                    for q in match_node(node, text, *p) {
                        if q != *p && !next.contains(&q) {
                            next.push(q);
                        }
                    }
                }
                if next.is_empty() {
                    break;
                }
                frontier = next;
                count += 1;
            }
            results
        }
    }
}

fn match_seq(nodes: &[Node], text: &[char], pos: usize) -> Vec<usize> {
    match nodes.split_first() {
        None => vec![pos],
        Some((first, rest)) => {
            let mut out = Vec::new();
            for p in match_node(first, text, pos) {
                for e in match_seq(rest, text, p) {
                    if !out.contains(&e) {
                        out.push(e);
                    }
                }
            }
            out
        }
    }
}

fn match_alt(alt: &Alt, text: &[char], pos: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for seq in &alt.seqs {
        for e in match_seq(seq, text, pos) {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

/// Python's `re.match(pattern, text) is not None`.
pub fn is_match(pattern: &str, text: &str) -> Result<bool, String> {
    let alt = compile(pattern)?;
    let chars: Vec<char> = text.chars().collect();
    Ok(!match_alt(&alt, &chars, 0).is_empty())
}

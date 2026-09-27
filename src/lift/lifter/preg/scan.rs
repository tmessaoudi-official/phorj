//! The pattern-body scan behind row 4l-a: one pass over the text between PHP's delimiters that
//! rewrites what must change (PHP's `$`, the ASCII shorthands of a pattern without `u`, an escaped
//! delimiter) and records the top-level shape the byte-atom rule needs.
//!
//! Without `u`, PCRE matches BYTES and phorj matches CHARACTERS. Every non-ASCII character is refused
//! before this scan runs, so every atom here consumes ASCII — except a "byte atom", `.` or a negated
//! class, which in PCRE also consumes the single bytes of a multi-byte character. [`byte_runs_ok`]
//! accepts one only where the existence answer cannot depend on that: an unbounded run whose two ends
//! must fall on character boundaries, or a lone atom with nothing anchoring it on either side.

/// What the scan needs from the modifiers.
pub(super) struct Opts {
    pub(super) unicode: bool,
    /// `D`: PHP's `$` is the very end, as `Core.Regex`'s `$` already is.
    pub(super) dollar_end: bool,
    /// The delimiter, when a `\<delim>` escape can occur in the body.
    pub(super) delim: Option<char>,
}

/// One top-level element, for the byte-atom rule.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Consumes exactly one ASCII character.
    Ascii,
    /// `.` or a negated class, without `u`.
    Byte,
    /// `^` / `\A`.
    Start,
    /// PHP's `$`, `\Z`, `\z`.
    End,
    /// A group, a look-around, a back-reference, a zero-width escape.
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quant {
    None,
    /// `*` / `+`, greedy or lazy.
    Unbounded,
    Other,
}

#[derive(Clone, Copy)]
struct Elem {
    kind: Kind,
    quant: Quant,
    /// The element consumes at least once (no quantifier, `+`, `{n,…}` with n ≥ 1).
    min1: bool,
}

/// The characters `regex-syntax` reads as meta: an escaped delimiter from this set keeps its `\`.
const META: &str = "\\.+*?()|[]{}^$#&-~";

const BYTE_HINT: &str = "add `u` to the PHP pattern if the subject is text, then lift again";

struct Scan<'a> {
    c: Vec<char>,
    i: usize,
    o: &'a Opts,
    out: String,
    depth: usize,
    alts: Vec<Vec<Elem>>,
    lookahead: bool,
}

/// Scan `body`; `Ok((pattern, needs_lookahead))`, or the reason (without the pattern prefix).
pub(super) fn scan(body: &str, o: &Opts) -> Result<(String, bool), String> {
    let mut s = Scan {
        c: body.chars().collect(),
        i: 0,
        o,
        out: String::new(),
        depth: 0,
        alts: vec![Vec::new()],
        lookahead: false,
    };
    while s.i < s.c.len() {
        s.step()?;
    }
    if !o.unicode {
        for alt in &s.alts {
            byte_runs_ok(alt)?;
        }
    }
    Ok((s.out, s.lookahead))
}

impl Scan<'_> {
    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }

    fn push(&mut self, kind: Kind) -> Result<(), String> {
        if kind == Kind::Byte && self.depth > 0 {
            return Err(byte_refusal());
        }
        if self.depth == 0 {
            let alt = self.alts.last_mut().expect("one alternative at least");
            alt.push(Elem {
                kind,
                quant: Quant::None,
                min1: true,
            });
        }
        Ok(())
    }

    fn step(&mut self) -> Result<(), String> {
        let ch = self.c[self.i];
        match ch {
            '\\' => return self.escape(),
            '[' => return self.class(),
            '(' => {
                self.push(Kind::Other)?;
                self.depth += 1;
                self.out.push('(');
                self.i += 1;
                return self.group_prefix();
            }
            ')' => self.depth = self.depth.saturating_sub(1),
            '|' if self.depth == 0 => self.alts.push(Vec::new()),
            '|' => {}
            '.' => self.push(if self.o.unicode {
                Kind::Ascii
            } else {
                Kind::Byte
            })?,
            '^' => self.push(Kind::Start)?,
            '$' => {
                self.i += 1;
                return if self.o.dollar_end {
                    self.out.push('$');
                    self.push(Kind::End)
                } else {
                    self.dollar()
                };
            }
            '*' | '+' | '?' | '{' => return self.quantifier(),
            _ => self.push(Kind::Ascii)?,
        }
        self.out.push(ch);
        self.i += 1;
        Ok(())
    }

    /// PHP's `$` (and `\Z`): the end, or before a final newline. The cursor is past it.
    fn dollar(&mut self) -> Result<(), String> {
        if matches!(self.peek(0), Some('*' | '+' | '?' | '{')) {
            return Err("a quantified `$` has no faithful form".into());
        }
        if self.depth == 0 && matches!(self.peek(0), None | Some('|')) {
            self.out.push_str(r"\n?\z");
        } else {
            self.out.push_str(r"(?=\n?\z)");
            self.lookahead = true;
        }
        self.push(Kind::End)
    }

    /// After `(`: copy a `(?…` group prefix (`?:`, `?=`, `?<=`, `?<name>`, `?i)`, …) so its `?` is not
    /// read as a quantifier. An inline flag enabling `i` without `u`, or `m` / `x` at all, is refused
    /// for the reason the same modifier is (see `translate`).
    fn group_prefix(&mut self) -> Result<(), String> {
        if self.peek(0) != Some('?') {
            return Ok(());
        }
        self.out.push('?');
        self.i += 1;
        // A group NAME is not a flag run: `(?<kind>` names a group, it does not set `i`.
        let named = match (self.peek(0), self.peek(1)) {
            (Some('<'), Some(c)) if c != '=' && c != '!' => Some(('>', 1)),
            (Some('\''), _) => Some(('\'', 1)),
            (Some('P'), Some('<')) => Some(('>', 2)),
            _ => None,
        };
        if let Some((close, open_len)) = named {
            for _ in 0..open_len {
                self.out.push(self.c[self.i]);
                self.i += 1;
            }
            while let Some(c) = self.peek(0) {
                self.out.push(c);
                self.i += 1;
                if c == close {
                    break;
                }
            }
            return Ok(());
        }
        let mut enabling = true;
        while let Some(f) = self.peek(0) {
            match f {
                ':' | '=' | '!' | '>' => {
                    self.out.push(f);
                    self.i += 1;
                    break;
                }
                ')' => break,
                '-' => enabling = false,
                'm' | 'x' => return Err(format!("the inline `{f}` flag (see the `{f}` modifier)")),
                'i' if enabling && !self.o.unicode => {
                    return Err(format!(
                        "the inline `i` flag without the `u` modifier (see the `i` modifier) — {BYTE_HINT}"
                    ))
                }
                c if c.is_ascii_alphanumeric() || matches!(c, '<' | '\'' | '_') => {}
                _ => break,
            }
            self.out.push(f);
            self.i += 1;
        }
        Ok(())
    }

    fn quantifier(&mut self) -> Result<(), String> {
        let start = self.i;
        let (min1, unbounded) = match self.c[self.i] {
            '*' => (false, true),
            '+' => (true, true),
            '?' => (false, false),
            _ => match self.brace() {
                Some(min1) => (min1, false),
                None => {
                    // Not a counted repetition: a literal `{`, as PCRE reads it.
                    self.out.push('{');
                    self.i += 1;
                    return self.push(Kind::Ascii);
                }
            },
        };
        if self.i == start {
            self.i += 1; // the one-character forms; `brace` has already advanced
        }
        let possessive = self.peek(0) == Some('+');
        if matches!(self.peek(0), Some('?' | '+')) {
            self.i += 1;
        }
        self.out.extend(&self.c[start..self.i]);
        if self.depth == 0 {
            if let Some(e) = self.alts.last_mut().and_then(|a| a.last_mut()) {
                e.min1 = min1;
                e.quant = if unbounded && !possessive {
                    Quant::Unbounded
                } else {
                    Quant::Other
                };
            }
        }
        Ok(())
    }

    /// A `{n}` / `{n,}` / `{n,m}` at the cursor: advance past it and return whether n ≥ 1.
    fn brace(&mut self) -> Option<bool> {
        let rest: String = self.c[self.i..].iter().collect();
        let close = rest.find('}')?;
        let inner = &rest[1..close];
        let (lo, hi) = inner.split_once(',').unwrap_or((inner, inner));
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(lo) || !(hi.is_empty() || digits(hi)) {
            return None;
        }
        self.i += rest[..=close].chars().count();
        Some(lo.parse::<u64>().map_or(true, |n| n >= 1))
    }
}

fn byte_refusal() -> String {
    format!(
        "`.` or a negated class matches one BYTE in PCRE without the `u` modifier and one CHARACTER \
         in phorj, and here the difference can change the answer — {BYTE_HINT}"
    )
}

/// The byte-atom rule over one top-level alternative (see the module doc).
fn byte_runs_ok(alt: &[Elem]) -> Result<(), String> {
    for (k, e) in alt.iter().enumerate() {
        if e.kind != Kind::Byte {
            continue;
        }
        let left = k.checked_sub(1).map(|j| alt[j]);
        let right = alt.get(k + 1).copied();
        let edge = |n: Option<Elem>, anchor: Kind| match n {
            None => true,
            Some(n) => n.kind == anchor || (n.kind == Kind::Ascii && n.min1),
        };
        let ok = match e.quant {
            Quant::Unbounded => edge(left, Kind::Start) && edge(right, Kind::End),
            Quant::None => left.is_none() && right.is_none(),
            Quant::Other => false,
        };
        if !ok {
            return Err(byte_refusal());
        }
    }
    Ok(())
}

#[path = "scan_escapes.rs"]
mod escapes;

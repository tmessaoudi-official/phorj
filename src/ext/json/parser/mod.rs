//! Std-only recursive-descent JSON parser (eager builder): the `JParser` byte-cursor and the
//! eager `value`/`number`/`string`/`array`/`object` grammar. The lazy skip-scan + one-level
//! materialization halves live in the sibling `lazy` submodule.

use super::natives::jnode;
use crate::value::{build_map, Payload, Value};
use std::rc::Rc;

mod lazy;
pub use lazy::materialize_lazy;
pub(super) use lazy::validate_json;

/// Std-only recursive-descent JSON parser → a `Json` enum value, or `None` on any syntax error
/// (including trailing non-whitespace). Mirrors `json_decode`: `{}`≠`[]`, integers without a
/// `.`/`e` are `Int` (overflow falls back to `Float`), duplicate object keys keep first position /
/// last value (via `build_map`).
pub(super) fn parse_json(s: &str) -> Option<Value> {
    let mut p = JParser {
        src: s,
        b: s.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return None; // trailing junk
    }
    Some(v)
}

/// Byte-cursor parser. JSON structure is ASCII, so we scan `&[u8]` and slice-borrow directly from
/// the source `&str` for number tokens and unescaped string runs; only `\`-escapes and `\u` build
/// owned text. This avoids the per-parse `Vec<char>` materialization (heap alloc + 4×-mem) the
/// prior char-slice version paid on every `Json.parse`. The parse RESULT is unchanged.
struct JParser<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
}

impl JParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Option<Value> {
        self.ws();
        match self.peek()? {
            b'n' => self.lit(b"null", jnode("Null", Payload::Zero)),
            b't' => self.lit(b"true", jnode("Bool", Payload::One(Value::Bool(true)))),
            b'f' => self.lit(b"false", jnode("Bool", Payload::One(Value::Bool(false)))),
            b'"' => {
                let s = self.string()?;
                Some(jnode("String", Payload::One(Value::Str(s.into()))))
            }
            b'[' => self.array(),
            b'{' => self.object(),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn lit(&mut self, kw: &[u8], v: Value) -> Option<Value> {
        for &ch in kw {
            if self.bump()? != ch {
                return None;
            }
        }
        Some(v)
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek()? {
            b'0' => self.i += 1, // a leading 0 must stand alone (no `01`)
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return None,
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return None;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return None;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        // The number token is pure ASCII, so the byte range is a valid str slice (no alloc).
        let tok = &self.src[start..self.i];
        if is_float {
            Some(jnode(
                "Float",
                Payload::One(Value::Float(tok.parse::<f64>().ok()?)),
            ))
        } else {
            // Integer; an i64 overflow falls back to Float, matching `json_decode`.
            match tok.parse::<i64>() {
                Ok(n) => Some(jnode("Int", Payload::One(Value::Int(n)))),
                Err(_) => Some(jnode(
                    "Float",
                    Payload::One(Value::Float(tok.parse::<f64>().ok()?)),
                )),
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        if self.bump() != Some(b'"') {
            return None;
        }
        let mut s = String::new();
        let mut run = self.i; // start of the current unescaped byte run
        loop {
            match self.peek()? {
                b'"' => {
                    s.push_str(&self.src[run..self.i]);
                    self.i += 1;
                    return Some(s);
                }
                b'\\' => {
                    s.push_str(&self.src[run..self.i]); // flush the run before the escape
                    self.i += 1; // consume '\'
                    match self.bump()? {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{08}'),
                        b'f' => s.push('\u{0c}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => s.push(self.unicode_escape()?),
                        _ => return None,
                    }
                    run = self.i;
                }
                b if b < 0x20 => return None, // a raw control char is invalid in a JSON string
                _ => self.i += 1,             // ordinary byte (ASCII or UTF-8 lead/continuation)
            }
        }
    }

    /// Read 4 hex digits (ASCII).
    fn hex4(&mut self) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..4 {
            let d = match self.bump()? {
                b @ b'0'..=b'9' => u32::from(b - b'0'),
                b @ b'a'..=b'f' => u32::from(b - b'a' + 10),
                b @ b'A'..=b'F' => u32::from(b - b'A' + 10),
                _ => return None,
            };
            v = v * 16 + d;
        }
        Some(v)
    }

    /// A `\uXXXX` escape (the `\u` already consumed), combining a surrogate pair when present. A lone
    /// surrogate is invalid (`None`), matching `json_decode`'s strict default.
    fn unicode_escape(&mut self) -> Option<char> {
        let u = self.hex4()?;
        if (0xD800..=0xDBFF).contains(&u) {
            if self.bump()? != b'\\' || self.bump()? != b'u' {
                return None;
            }
            let lo = self.hex4()?;
            if !(0xDC00..=0xDFFF).contains(&lo) {
                return None;
            }
            let cp = 0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00);
            char::from_u32(cp)
        } else if (0xDC00..=0xDFFF).contains(&u) {
            None
        } else {
            char::from_u32(u)
        }
    }

    fn array(&mut self) -> Option<Value> {
        self.bump(); // '['
        self.ws();
        let mut xs = Vec::new();
        if self.peek() == Some(b']') {
            self.bump();
            return Some(jnode("Array", Payload::One(Value::List(Rc::new(xs)))));
        }
        loop {
            xs.push(self.value()?);
            self.ws();
            match self.bump()? {
                b',' => self.ws(),
                b']' => return Some(jnode("Array", Payload::One(Value::List(Rc::new(xs))))),
                _ => return None,
            }
        }
    }

    fn object(&mut self) -> Option<Value> {
        self.bump(); // '{'
        self.ws();
        let mut pairs: Vec<(Value, Value)> = Vec::new();
        if self.peek() == Some(b'}') {
            self.bump();
            return self.make_obj(pairs);
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return None;
            }
            let key = self.string()?;
            self.ws();
            if self.bump()? != b':' {
                return None;
            }
            let val = self.value()?;
            pairs.push((Value::Str(key.into()), val));
            self.ws();
            match self.bump()? {
                b',' => {}
                b'}' => return self.make_obj(pairs),
                _ => return None,
            }
        }
    }

    fn make_obj(&self, pairs: Vec<(Value, Value)>) -> Option<Value> {
        // String keys ⇒ `build_map` never rejects; it dedups first-position/last-value (PHP assoc).
        let entries = build_map(pairs).ok()?;
        Some(jnode("Object", Payload::One(Value::Map(Rc::new(entries)))))
    }
}

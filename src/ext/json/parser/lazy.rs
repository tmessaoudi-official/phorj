//! Lazy JSON parsing (DEC-294): the alloc-free skip-scan validator + one-level materialization
//! over the shared `JParser` cursor, plus `validate_json`/`materialize_lazy` entry points.

use super::JParser;
use crate::ext::json::natives::jnode;
use crate::phstr::PhStr;
use crate::value::{LazyJson, Payload, Value};
use std::cell::OnceCell;
use std::rc::Rc;

impl JParser<'_> {
    // ---- lazy skip-scan (DEC-294) --------------------------------------------------------------
    // Advance the cursor past ONE complete value, validating it WITHOUT building — byte-identical
    // ACCEPTANCE with `value()` (mirrors its grammar exactly), but alloc-free. Used both by the
    // parse-time whole-doc validation (to preserve null-on-malformed) and by `materialize_*` to
    // delimit a child's byte range.
    //
    // ⚠ INVARIANT (guarded by the `lazy_matches_eager_on_corpus` test): the `skip_*` family must
    // accept EXACTLY what the eager builders (`value`/`string`/`number`) accept. If they ever diverge,
    // `validate_json` accepts a doc whose child then fails the real builder in `materialize_lazy` →
    // its `.expect("re-parse cannot fail")` PANICS. Touch `value`/`string`/`number` ⇒ touch `skip_*`
    // (and vice versa) and re-run that corpus test.

    fn skip_value(&mut self) -> Option<()> {
        self.ws();
        match self.peek()? {
            b'n' => self.skip_lit(b"null"),
            b't' => self.skip_lit(b"true"),
            b'f' => self.skip_lit(b"false"),
            b'"' => self.skip_string(),
            b'[' => self.skip_array(),
            b'{' => self.skip_object(),
            b'-' | b'0'..=b'9' => self.skip_number(),
            _ => None,
        }
    }

    fn skip_lit(&mut self, kw: &[u8]) -> Option<()> {
        for &ch in kw {
            if self.bump()? != ch {
                return None;
            }
        }
        Some(())
    }

    /// Mirror of [`JParser::number`]'s SCAN (a syntactically-valid number always parses to some
    /// value there — `parse::<f64>` yields inf on overflow, never `Err` — so the scan IS the whole
    /// acceptance test; no build/parse needed).
    fn skip_number(&mut self) -> Option<()> {
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek()? {
            b'0' => self.i += 1,
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return None,
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return None;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
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
        Some(())
    }

    /// Mirror of [`JParser::string`]'s acceptance (same escape + control-char + `\u` surrogate-pair
    /// rules), but without building the `String`. The hot inner loop BULK-SKIPS the run of plain
    /// bytes (not `"`, not `\`, not a control byte — the same three classes the match below
    /// dispatches on, so acceptance is bit-identical) with a direct slice scan: the skip-scan runs
    /// once at `Json.parse` (whole-doc validation) and again per materialized level (child
    /// delimitation), so its per-byte cost is the lazy path's floor.
    fn skip_string(&mut self) -> Option<()> {
        if self.bump() != Some(b'"') {
            return None;
        }
        loop {
            while let Some(&b) = self.b.get(self.i) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.i += 1;
            }
            match self.peek()? {
                b'"' => {
                    self.i += 1;
                    return Some(());
                }
                b'\\' => {
                    self.i += 1;
                    match self.bump()? {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        b'u' => {
                            self.skip_unicode_escape()?;
                        }
                        _ => return None,
                    }
                }
                b if b < 0x20 => return None,
                _ => self.i += 1,
            }
        }
    }

    /// Mirror of [`JParser::unicode_escape`]'s acceptance (the `\u` already consumed): validates a lone
    /// code unit or a hi+lo surrogate pair yields a real `char`, without returning it.
    fn skip_unicode_escape(&mut self) -> Option<()> {
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
            char::from_u32(cp)?;
            Some(())
        } else if (0xDC00..=0xDFFF).contains(&u) {
            None
        } else {
            char::from_u32(u)?;
            Some(())
        }
    }

    fn skip_array(&mut self) -> Option<()> {
        self.bump(); // '['
        self.ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Some(());
        }
        loop {
            self.skip_value()?;
            self.ws();
            match self.bump()? {
                b',' => self.ws(),
                b']' => return Some(()),
                _ => return None,
            }
        }
    }

    fn skip_object(&mut self) -> Option<()> {
        self.bump(); // '{'
        self.ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Some(());
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return None;
            }
            self.skip_string()?; // key (validated, not retained)
            self.ws();
            if self.bump()? != b':' {
                return None;
            }
            self.skip_value()?;
            self.ws();
            match self.bump()? {
                b',' => {}
                b'}' => return Some(()),
                _ => return None,
            }
        }
    }

    // ---- one-level materialization (DEC-294) ----------------------------------------------------
    // Build exactly ONE level at the cursor: scalars fully; containers with LAZY children (each a
    // `Value::JsonLazy` over the child's byte range), so unread subtrees never allocate. Byte-identical
    // to `value()` except the container payload elements are lazy until deconstructed.

    fn materialize_one(&mut self, src: &PhStr) -> Option<Value> {
        self.ws();
        match self.peek()? {
            b'n' => self.lit(b"null", jnode("Null", Payload::Zero)),
            b't' => self.lit(b"true", jnode("Bool", Payload::One(Value::Bool(true)))),
            b'f' => self.lit(b"false", jnode("Bool", Payload::One(Value::Bool(false)))),
            b'"' => {
                let s = self.string()?;
                Some(jnode("String", Payload::One(Value::Str(s.into()))))
            }
            b'[' => self.materialize_array(src),
            b'{' => self.materialize_object(src),
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }

    fn lazy_child(&self, src: &PhStr, start: usize) -> Value {
        Value::JsonLazy(Rc::new(LazyJson {
            src: src.clone(),
            start,
            cached: OnceCell::new(),
        }))
    }

    fn materialize_array(&mut self, src: &PhStr) -> Option<Value> {
        self.bump(); // '['
        self.ws();
        let mut xs = Vec::new();
        if self.peek() == Some(b']') {
            self.bump();
            return Some(jnode("Array", Payload::One(Value::List(Rc::new(xs)))));
        }
        loop {
            let start = self.i; // materialize_one/value both ws() first, so a leading-ws start is safe
            self.skip_value()?;
            xs.push(self.lazy_child(src, start));
            self.ws();
            match self.bump()? {
                b',' => self.ws(),
                b']' => return Some(jnode("Array", Payload::One(Value::List(Rc::new(xs))))),
                _ => return None,
            }
        }
    }

    fn materialize_object(&mut self, src: &PhStr) -> Option<Value> {
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
            let start = self.i;
            self.skip_value()?;
            pairs.push((Value::Str(key.into()), self.lazy_child(src, start)));
            self.ws();
            match self.bump()? {
                b',' => {}
                b'}' => return self.make_obj(pairs),
                _ => return None,
            }
        }
    }
}

/// Validate `s` as ONE complete JSON document without building the tree (alloc-free skip-scan), so
/// `Json.parse` keeps its null-on-malformed semantics while deferring node allocation. Returns the
/// byte offset of the whitespace-skipped root value if the whole input is one well-formed value, else
/// `None` (malformed / trailing junk) — byte-identical ACCEPTANCE with [`super::parse_json`] (DEC-294).
pub(in crate::ext::json) fn validate_json(s: &str) -> Option<usize> {
    let mut p = JParser {
        src: s,
        b: s.as_bytes(),
        i: 0,
    };
    p.ws();
    let root = p.i;
    p.skip_value()?;
    p.ws();
    if p.i != p.b.len() {
        return None; // trailing junk
    }
    Some(root)
}

/// Materialize ONE level of a lazy node (DEC-294). `start` points at a value already accepted by
/// [`validate_json`] (the only producer of `LazyJson`), so the re-parse cannot fail — a `None` here
/// would be an internal invariant break, not a user error.
pub fn materialize_lazy(lazy: &LazyJson) -> Value {
    lazy.cached
        .get_or_init(|| {
            let s: &str = lazy.src.as_str();
            let mut p = JParser {
                src: s,
                b: s.as_bytes(),
                i: lazy.start,
            };
            p.materialize_one(&lazy.src)
                .expect("materialize_lazy: node was validated at parse, re-parse cannot fail")
        })
        .clone()
}

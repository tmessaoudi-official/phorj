//! `Core.Json` — JSON parse / stringify over a compiler-injected `Json` enum value model
//! (`docs/specs/2026-06-26-core-json-design.md`). The `Json` enum is injected by
//! `cli::inject_core_modules` (`Core.Json` row) when a program imports `Core.Json`, so these natives can construct +
//! receive ordinary `Value::Enum { ty: "Json", … }` values.
//!
//! The one `eval` body per native is shared by both Rust backends (the parity guarantee). The PHP
//! transpile of each native delegates to a `__phorj_json_*` helper (`transpile/program.rs`) that
//! walks the same enum hierarchy — kept byte-identical with the kernels here: floats render via the
//! shortest-round-trip positional formatter (`format!("{}")` / `__phorj_float`, NOT json's
//! scientific notation), strings escape to match PHP `json_encode`'s default, objects keep Map
//! insertion order, and number decoding distinguishes `Int` from `Float` exactly as `json_decode`.

use crate::native::*;
use crate::phstr::PhStr;
use crate::types::Ty;
use crate::value::{build_map, EnumVal, HKey, LazyJson, Payload, Value};
use std::fmt::Write as _;
use std::rc::Rc;

thread_local! {
    /// Perf (jsonround alloc-bound, DEC-266): the `Json` type name and the fixed variant names,
    /// each interned as ONE `Rc<str>` per thread. `jnode` clones these (a refcount bump) instead of
    /// `"Json".into()` / `variant.into()` — which allocated a FRESH `Rc<str>` per node (~2 heap
    /// allocs × ~9 nodes per parsed doc). The produced `EnumVal` is byte-identical (same ty/variant
    /// content); this only removes the redundant allocation the VM's own enum path already avoids
    /// via its cached `EnumDesc`. Natives run one-thread-per-VM, so thread_local is sound.
    static JSON_TY: Rc<str> = Rc::from("Json");
    static JSON_VARIANTS: [(&'static str, Rc<str>); 7] = [
        ("Null", Rc::from("Null")),
        ("Bool", Rc::from("Bool")),
        ("Int", Rc::from("Int")),
        ("Float", Rc::from("Float")),
        ("String", Rc::from("String")),
        ("Array", Rc::from("Array")),
        ("Object", Rc::from("Object")),
    ];
}

/// Build a `Json` enum node. `variant` is the Phorj variant name (`Null`/`Bool`/`Int`/`Float`/
/// `String`/`Array`/`Object`); the transpiler mangles reserved ones to PHP class names, the
/// backends use this string directly. Interned `Rc<str>` (see [`JSON_VARIANTS`]) — a refcount
/// clone, not a fresh allocation.
pub(super) fn jnode(variant: &str, payload: Payload) -> Value {
    // Intern the immutable scalar nodes — `Json.Null`, `Json.Bool(true/false)`, and small
    // `Json.Int(n)` — so `parse` clones a cached node (an Rc bump) instead of allocating a fresh
    // `Rc<EnumVal>` per occurrence (the Json ADT is immutable, so a shared node is byte-identical:
    // match/encode/eq_val all read ty+variant+payload content, never node identity). DEC-293 parse
    // alloc lever. Small ints (ids, counts, HTTP codes) dominate real JSON payloads.
    match (variant, &payload) {
        ("Null", _) => return JSON_NULL.with(Value::clone),
        ("Bool", Payload::One(Value::Bool(true))) => return JSON_TRUE.with(Value::clone),
        ("Bool", Payload::One(Value::Bool(false))) => return JSON_FALSE.with(Value::clone),
        ("Int", Payload::One(Value::Int(n))) if (SMALL_INT_MIN..=SMALL_INT_MAX).contains(n) => {
            return JSON_SMALL_INTS
                .with(|c| c[usize::try_from(*n - SMALL_INT_MIN).unwrap()].clone())
        }
        _ => {}
    }
    jnode_fresh(variant, payload)
}

const SMALL_INT_MIN: i64 = -16;
const SMALL_INT_MAX: i64 = 256;

thread_local! {
    static JSON_NULL: Value = jnode_fresh("Null", Payload::Zero);
    static JSON_TRUE: Value = jnode_fresh("Bool", Payload::One(Value::Bool(true)));
    static JSON_FALSE: Value = jnode_fresh("Bool", Payload::One(Value::Bool(false)));
    /// Cached `Json.Int(n)` for `n` in `[SMALL_INT_MIN, SMALL_INT_MAX]`, indexed by `n - MIN`.
    static JSON_SMALL_INTS: Vec<Value> = (SMALL_INT_MIN..=SMALL_INT_MAX)
        .map(|n| jnode_fresh("Int", Payload::One(Value::Int(n))))
        .collect();
}

/// Always-allocating node constructor (the pre-DEC-293 `jnode` body). Used directly to build the
/// interned singletons above, and for every non-cacheable node (containers, floats, strings, large ints).
fn jnode_fresh(variant: &str, payload: Payload) -> Value {
    let ty = JSON_TY.with(Rc::clone);
    let variant = JSON_VARIANTS.with(|vs| {
        vs.iter()
            .find(|(name, _)| *name == variant)
            .map(|(_, rc)| Rc::clone(rc))
            .unwrap_or_else(|| Rc::from(variant))
    });
    Value::Enum(Rc::new(EnumVal {
        ty,
        variant,
        payload,
    }))
}

// ---- encode (stringify) -------------------------------------------------------------------------

fn json_stringify(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [j] => {
            let mut s = String::with_capacity(64);
            encode(j, &mut s)?;
            Ok(Value::Str(s.into()))
        }
        _ => Err("Json.stringify expects (Json)".into()),
    }
}

// NDJSON (JSON Lines): one JSON value per line. `parseLines` parses each non-empty (trimmed) line;
// any malformed line makes the whole parse fail (None), mirroring `parse`. `stringifyLines` encodes
// each value and joins with `\n` (no trailing newline). Both backends and the transpiled-PHP
// `__phorj_json_{parse,stringify}_lines` helpers split/join identically, so byte-identity holds.
pub(super) fn json_parse_lines(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => {
            let mut out: Vec<Value> = Vec::new();
            for line in s.split('\n') {
                // Trim exactly PHP `trim()`'s default set (space, \t, \r, \v, \0 — \n already split
                // out), NOT Rust's Unicode `.trim()`, so the transpiled `__phorj_json_parse_lines`
                // (which uses PHP `trim`) is byte-identical on exotic-whitespace input too.
                let t = line.trim_matches([' ', '\t', '\r', '\u{0b}', '\0']);
                if t.is_empty() {
                    continue;
                }
                match parse_json(t) {
                    Some(v) => out.push(v),
                    None => return Ok(Value::Null), // any malformed line → None
                }
            }
            Ok(Value::List(Rc::new(out)))
        }
        _ => Err("Json.parseLines expects (string)".into()),
    }
}

pub(super) fn json_stringify_lines(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::List(xs)] => {
            let mut lines: Vec<String> = Vec::with_capacity(xs.len());
            for x in xs.iter() {
                let mut s = String::new();
                encode(x, &mut s)?;
                lines.push(s);
            }
            Ok(Value::Str(lines.join("\n").into()))
        }
        _ => Err("Json.stringifyLines expects (List<Json>)".into()),
    }
}

fn json_stringify_pretty(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [j] => {
            let mut s = String::new();
            encode_pretty(j, 0, &mut s)?;
            Ok(Value::Str(s.into()))
        }
        _ => Err("Json.stringifyPretty expects (Json)".into()),
    }
}

fn as_json(v: &Value) -> Result<&EnumVal, String> {
    match v {
        Value::Enum(e) if e.ty.as_ref() == "Json" => Ok(e),
        _ => Err(format!("Json value expected, got {}", v.type_name())),
    }
}

/// An object key (typed `Map<string, Json>`, so always a string `HKey`).
fn key_str(k: &HKey) -> Result<&str, String> {
    match k {
        HKey::Str(s) => Ok(s),
        _ => Err("Json object key must be a string".into()),
    }
}

/// Compact encoding — matches `__phorj_json_encode` byte-for-byte.
pub(super) fn encode(v: &Value, out: &mut String) -> Result<(), String> {
    if let Value::JsonLazy(l) = v {
        return encode(&materialize_lazy(l), out); // DEC-294: materialize one level, then encode
    }
    let e = as_json(v)?;
    match (e.variant.as_ref(), e.payload.as_slice()) {
        ("Null", []) => out.push_str("null"),
        ("Bool", [Value::Bool(b)]) => out.push_str(if *b { "true" } else { "false" }),
        // Write integers/floats straight into the buffer (no throwaway `to_string()`/`format!` alloc).
        ("Int", [Value::Int(n)]) => {
            let _ = write!(out, "{n}");
        }
        ("Float", [Value::Float(f)]) => {
            let _ = write!(out, "{f}");
        }
        ("String", [Value::Str(s)]) => encode_str(s, out),
        ("Array", [Value::List(xs)]) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                encode(x, out)?;
            }
            out.push(']');
        }
        ("Object", [Value::Map(m)]) => {
            out.push('{');
            for (i, (k, val)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                encode_str(key_str(k)?, out);
                out.push(':');
                encode(val, out)?;
            }
            out.push('}');
        }
        _ => return Err(format!("malformed Json node `{}`", e.variant)),
    }
    Ok(())
}

/// Pretty encoding (`JSON_PRETTY_PRINT` layout: 4-space indent, `": "` after a key, empty `[]`/`{}`
/// inline). `indent` is the current leading-space count. Matches `__phorj_json_pretty`.
pub(super) fn encode_pretty(v: &Value, indent: usize, out: &mut String) -> Result<(), String> {
    if let Value::JsonLazy(l) = v {
        return encode_pretty(&materialize_lazy(l), indent, out); // DEC-294
    }
    let e = as_json(v)?;
    match (e.variant.as_ref(), e.payload.as_slice()) {
        ("Array", [Value::List(xs)]) if !xs.is_empty() => {
            let inner = indent + 4;
            out.push_str("[\n");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(inner));
                encode_pretty(x, inner, out)?;
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            out.push(']');
        }
        ("Object", [Value::Map(m)]) if !m.is_empty() => {
            let inner = indent + 4;
            out.push_str("{\n");
            for (i, (k, val)) in m.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(inner));
                encode_str(key_str(k)?, out);
                out.push_str(": ");
                encode_pretty(val, inner, out)?;
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            out.push('}');
        }
        // Scalars and empty containers render compactly (one line) — matches PHP.
        _ => encode(v, out)?,
    }
    Ok(())
}

/// JSON string escaping matching PHP `json_encode`'s default: escapes `"` `\` `/`, the named control
/// escapes, other control chars (`<0x20`) as `\u00xx`, and every non-ASCII (`>0x7f`) code point as
/// `\uxxxx` (a surrogate pair for `>0xFFFF`). Lowercase hex (PHP's convention).
fn encode_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) > 0x7f => {
                let cp = c as u32;
                if cp > 0xFFFF {
                    let v = cp - 0x10000;
                    let hi = 0xD800 + (v >> 10);
                    let lo = 0xDC00 + (v & 0x3FF);
                    out.push_str(&format!("\\u{hi:04x}\\u{lo:04x}"));
                } else {
                    out.push_str(&format!("\\u{cp:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---- decode (parse) -----------------------------------------------------------------------------

fn json_parse(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        // Lazy parse (DEC-294): validate the WHOLE document up front (alloc-free skip-scan — preserves
        // null-on-malformed), then return a top-level LAZY node over the source. Nodes materialize to
        // `Value::Enum` one level at a time on deconstruction, so unread subtrees never allocate. A
        // malformed doc is `Value::Null` (byte-identical acceptance to the eager `parse_json`).
        [Value::Str(s)] => Ok(match validate_json(s) {
            // Share the input `PhStr` (Rc bump for a heap doc — no full-doc copy) as the lazy backing.
            Some(root) => Value::JsonLazy(Rc::new(LazyJson {
                src: s.clone(),
                start: root,
                cached: std::cell::OnceCell::new(),
            })),
            None => Value::Null,
        }),
        _ => Err("Json.parse expects (string)".into()),
    }
}

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
    /// rules), but without building the `String`.
    fn skip_string(&mut self) -> Option<()> {
        if self.bump() != Some(b'"') {
            return None;
        }
        loop {
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
            cached: std::cell::OnceCell::new(),
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
/// `None` (malformed / trailing junk) — byte-identical ACCEPTANCE with [`parse_json`] (DEC-294).
pub(super) fn validate_json(s: &str) -> Option<usize> {
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

/// Owned-value form of [`materialize_lazy`]: if `v` is a lazy Json node, materialize one level;
/// otherwise return it unchanged. For deconstruction sites that already own the value (VM ops).
pub fn materialize_if_lazy(v: Value) -> Value {
    match &v {
        Value::JsonLazy(l) => materialize_lazy(l),
        _ => v,
    }
}

// ---- registry -----------------------------------------------------------------------------------

/// The `Core.Json` registry entries. `Json` is the compiler-injected enum (`cli::inject_core_modules`)
/// — referenced here as a bare `Ty::Named`; the type resolves because a *call* to one of these natives
/// requires `import Core.Json;`, which triggers the injection before the checker runs.
pub fn json_natives() -> Vec<NativeFn> {
    let json = || Ty::Named("Json".to_string(), vec![]);
    vec![
        NativeFn {
            module: "Core.Json",
            name: "parse",
            params: vec![Ty::String],
            ret: Ty::Optional(Box::new(json())),
            pure: true,
            eval: NativeEval::Pure(json_parse),
            php: |a| format!("__phorj_json_decode({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Json",
            name: "parseLines",
            params: vec![Ty::String],
            ret: Ty::Optional(Box::new(Ty::List(Box::new(json())))),
            pure: true,
            eval: NativeEval::Pure(json_parse_lines),
            php: |a| format!("__phorj_json_parse_lines({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Json",
            name: "stringifyLines",
            params: vec![Ty::List(Box::new(json()))],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(json_stringify_lines),
            php: |a| format!("__phorj_json_stringify_lines({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Json",
            name: "stringify",
            params: vec![json()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(json_stringify),
            php: |a| format!("__phorj_json_encode({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Json",
            name: "stringifyPretty",
            params: vec![json()],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(json_stringify_pretty),
            php: |a| format!("__phorj_json_encode_pretty({})", parg(a, 0)),
        },
    ]
}

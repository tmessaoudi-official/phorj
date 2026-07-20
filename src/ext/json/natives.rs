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
//!
//! The encode tree walkers live in the sibling `encode` module; the parser (eager + lazy) in
//! `parser`. This file owns the node constructors, the native wrappers, and the registry.

use super::encode::{encode, encode_pretty};
use super::parser::{materialize_lazy, parse_json, validate_json};
use crate::native::*;
use crate::types::Ty;
use crate::value::{EnumVal, LazyJson, Payload, Value};
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

// ---- encode (stringify) natives -----------------------------------------------------------------

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

// ---- decode (parse) natives ---------------------------------------------------------------------

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

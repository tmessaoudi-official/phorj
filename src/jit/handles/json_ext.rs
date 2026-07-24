//! DEC-333 Json-ADT JIT runtime helpers (`rt_u_json_*`). The JIT represents a `Json` value as a
//! REGISTER PAIR `(payload word, tag word)`; these `extern "C"` helpers bridge that pair to/from
//! the boxed `Value::Enum{"Json", …}` / `Value::JsonLazy` world. Relative tags mirror the prelude
//! `Core.Json` variant order (single-sourced in `src/ext/json/natives.rs`): Null=0, Bool=1, Int=2,
//! Float=3, String=4, Array=5, Object=6; tag 7 = phorj `null` (the `Json?` None — a failed parse
//! or a missing map key), tag `< 0` = a fault the JIT maps to code 5 ("redo on VM"). The payload
//! word per tag: 0/7 → filler `0`; 1 → bool `0/1`; 2 → `i64`; 3 → `f64` bits; 4 → an untagged str
//! handle; 5 → a JList handle (untagged, boxes `Value::List`); 6 → a JMap handle (untagged, boxes
//! `Value::Map`). Container payloads are boxed handles because the register-pair encoding cannot
//! hold a nested structure inline (see SLICE-STATE's 5b API pointers).
//!
//! NO-PANIC discipline (extern "C"): the first statement is always `unsafe { &mut *ctx }`; every
//! defensive mismatch returns a fault (`tag: -1`) or phorj-null (`tag: 7`); reads go through
//! `get`/`str_bytes`; container mints go through `alloc_json` (the json-only LIVE-handle cap that
//! faults instead of OOMing). `cfg(not(feature = "json"))` gets a runtime-dead stub sharing the
//! one signature (Core.Json is E-EXTENSION-DISABLED without the feature, so `canonical_json` is
//! never stamped and no compiled graph reaches these — the stub only keeps `symbols.rs` linking).

use super::*;
#[cfg(feature = "json")]
use crate::value::HKey;

/// Relative tag for phorj `null` — the `Json?` None (failed parse / missing key), one past the 7
/// canonical `Core.Json` variants (Null..Object = 0..6).
#[allow(dead_code)] // read by json_ext (feature `json`) + the future emit arms; unused otherwise.
pub(in crate::jit) const JSON_TAG_PHNULL: i64 = 7;

/// The `(payload, tag)` pair a `rt_u_json_*` helper returns — the same `#[repr(C)]` two-`i64`
/// shape as [`UbMapGetRet`] (SysV rax:rdx / AArch64 x0:x1), matching a Cranelift `returns =
/// [i64, i64]` import signature. `tag < 0` = fault → code 5.
#[repr(C)]
#[allow(dead_code)] // fields are written across the FFI boundary; read by the future emit arms.
pub(in crate::jit) struct UbJsonRet {
    pub(in crate::jit) payload: i64,
    pub(in crate::jit) tag: i64,
}

/// `Json.parse(doc)` — validate + parse `doc` (an untagged/slot/acc str handle) and return the
/// ROOT node as a `(payload, tag)` pair (one level eager, container children stay lazy — the
/// `deepjson` win). Malformed input → phorj null (tag 7), matching the VM's `Json.parse` contract
/// (`Ok(Value::Null)`), so `if (var j = parse(x))` fails closed. `free != 0` (compile-time OWNED
/// arg, e.g. `Json.parse(a + b)`) releases the arg handle — but only AFTER the bytes are copied
/// out (a free-before-copy on a boxed owned input would be a use-after-free).
#[cfg(feature = "json")]
pub(in crate::jit) extern "C" fn rt_u_json_parse(ctx: *mut UbCtx, s: i64, free: i64) -> UbJsonRet {
    let ctx = unsafe { &mut *ctx };
    // Copy the doc bytes OUT of the ctx first: `json_parse_str` builds its own owned `PhStr`
    // (the `LazyJson.src`), so we need an owned `String` and must not hold a `&ctx` borrow across
    // the later `&mut ctx` mints. A non-utf8 / bad handle faults closed.
    let doc: String = match ctx.str_bytes(s) {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(st) => st.to_owned(),
            Err(_) => {
                return UbJsonRet {
                    payload: 0,
                    tag: -1,
                }
            }
        },
        None => {
            return UbJsonRet {
                payload: 0,
                tag: -1,
            }
        }
    };
    if free != 0 {
        ctx.release(s); // bytes already copied — safe to release the (owned) arg now
    }
    let mut scratch = String::new();
    let parsed = match crate::ext::json::json_parse_str(&doc, &mut scratch) {
        // Valid → JsonLazy root; malformed → Value::Null (the parse contract).
        Ok(v) => v,
        // Unreachable for a `[Str]` arg, but fail closed rather than trust it.
        Err(_) => {
            return UbJsonRet {
                payload: 0,
                tag: -1,
            }
        }
    };
    // Force ONE level (root variant + its immediate payload); container children stay lazy.
    encode_json_value(ctx, crate::ext::json::materialize_if_lazy(parsed))
}

/// Map a MATERIALIZED (one-level) Json `Value` to the `(payload, tag)` pair. A container payload
/// (`Array`/`Object`/`String`) is minted as an untagged handle via [`UbCtx::alloc_json`] (the cap
/// faults instead of OOMing). Any shape that isn't a canonical Json node faults closed.
#[cfg(feature = "json")]
fn encode_json_value(ctx: &mut UbCtx, v: Value) -> UbJsonRet {
    let fault = UbJsonRet {
        payload: 0,
        tag: -1,
    };
    match v {
        // A parse of the JSON literal `null` yields the `Json.Null` variant (tag 0); a FAILED
        // parse yields `Value::Null` → phorj null (tag 7).
        Value::Null => UbJsonRet {
            payload: 0,
            tag: JSON_TAG_PHNULL,
        },
        Value::Enum(e) => {
            let tag: i64 = match e.variant.as_ref() {
                "Null" => 0,
                "Bool" => 1,
                "Int" => 2,
                "Float" => 3,
                "String" => 4,
                "Array" => 5,
                "Object" => 6,
                _ => return fault,
            };
            let payload: i64 = match tag {
                0 => 0,
                1 => match e.payload.first() {
                    Some(Value::Bool(b)) => *b as i64,
                    _ => return fault,
                },
                2 => match e.payload.first() {
                    Some(Value::Int(n)) => *n,
                    _ => return fault,
                },
                3 => match e.payload.first() {
                    Some(Value::Float(f)) => f.to_bits() as i64,
                    _ => return fault,
                },
                // A container / string node mints an untagged handle boxing the inner `Value`
                // (read later via `str_bytes` for a String, or `handles[h]` for a JList/JMap).
                4 => match e.payload.first() {
                    Some(sv @ Value::Str(_)) => ctx.alloc_json(sv.clone()),
                    _ => return fault,
                },
                5 => match e.payload.first() {
                    Some(lv @ Value::List(_)) => ctx.alloc_json(lv.clone()),
                    _ => return fault,
                },
                6 => match e.payload.first() {
                    Some(mv @ Value::Map(_)) => ctx.alloc_json(mv.clone()),
                    _ => return fault,
                },
                _ => return fault,
            };
            // `alloc_json` returns -1 when the LIVE-handle cap is hit — propagate as a fault.
            if matches!(tag, 4..=6) && payload < 0 {
                return fault;
            }
            UbJsonRet { payload, tag }
        }
        // A JsonLazy that survived `materialize_if_lazy` (shouldn't happen) or any non-Json value.
        _ => fault,
    }
}

/// `Core.Map.get(obj, key)` on a JMap handle (a Json `Object`'s payload) → the value as a
/// `(payload, tag)` Json pair. Linear `HKey::Str` scan mirroring the VM's `map_get`; a MISS →
/// phorj null (tag 7 — `Map.get` returns `Json?`, so `?? Json.Null` coalesces downstream). Every
/// non-filler payload is minted into a FRESH handle (via `encode_json_value` → `alloc_json`),
/// never aliasing the map's interior. `free_mask & 1` releases the key handle (compile-time
/// ownership: a Borrowed/ConstBorrow key passes 0).
#[cfg(feature = "json")]
pub(in crate::jit) extern "C" fn rt_u_json_map_get(
    ctx: *mut UbCtx,
    map_h: i64,
    key_h: i64,
    free_mask: i64,
) -> UbJsonRet {
    let ctx = unsafe { &mut *ctx };
    let fault = UbJsonRet {
        payload: 0,
        tag: -1,
    };
    let key: String = match ctx.str_bytes(key_h) {
        Some(b) => match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            Err(_) => return fault,
        },
        None => match ctx.handles.get(key_h as usize) {
            Some(Value::Str(s)) => s.as_str().to_owned(),
            _ => return fault,
        },
    };
    if free_mask & 1 != 0 {
        ctx.release(key_h);
    }
    // Clone the matching VALUE out so the immutable `handles` borrow ends before the mint.
    let found: Option<Value> = match ctx.handles.get(map_h as usize) {
        Some(Value::Map(m)) => m
            .iter()
            .find(|(k, _)| matches!(k, HKey::Str(ks) if ks.as_str() == key))
            .map(|(_, v)| v.clone()),
        _ => return fault,
    };
    match found {
        Some(v) => encode_json_value(ctx, crate::ext::json::materialize_if_lazy(v)),
        None => UbJsonRet {
            payload: 0,
            tag: JSON_TAG_PHNULL,
        },
    }
}

/// `Core.List.length(arr)` on a JList handle (a Json `Array`'s payload) → the element count as a
/// plain `i64`; a bad handle → `-1` (→ code 5).
#[cfg(feature = "json")]
pub(in crate::jit) extern "C" fn rt_u_json_list_len(ctx: *mut UbCtx, list_h: i64) -> i64 {
    let ctx = unsafe { &mut *ctx };
    match ctx.handles.get(list_h as usize) {
        Some(Value::List(xs)) => i64::try_from(xs.len()).unwrap_or(-1),
        _ => -1,
    }
}

/// `arr[idx]` on a JList handle → the element as a `(payload, tag)` Json pair (fresh handle for a
/// container/string child). Out-of-bounds (or a negative index) → `tag: -1` → code 5 (the VM
/// raises an index fault there, so redo-on-VM reproduces the exact, correctly-ordered message).
#[cfg(feature = "json")]
pub(in crate::jit) extern "C" fn rt_u_json_list_get(
    ctx: *mut UbCtx,
    list_h: i64,
    idx: i64,
) -> UbJsonRet {
    let ctx = unsafe { &mut *ctx };
    let fault = UbJsonRet {
        payload: 0,
        tag: -1,
    };
    let elem: Value = match ctx.handles.get(list_h as usize) {
        Some(Value::List(xs)) => match usize::try_from(idx).ok().and_then(|i| xs.get(i)) {
            Some(v) => v.clone(),
            None => return fault, // OOB / negative → code 5 (VM index fault)
        },
        _ => return fault,
    };
    encode_json_value(ctx, crate::ext::json::materialize_if_lazy(elem))
}

/// `cfg(not(feature = "json"))` runtime-dead stub — keeps `register_ub_symbols` linking when the
/// Json extension is compiled out. Never reached at runtime (a Core.Json import is
/// E-EXTENSION-DISABLED without the feature, so `canonical_json` is never stamped and no graph
/// carries a `Kind::Json`). Shares the ONE signature with the real body above so a drift is a
/// compile error at the symbol registration.
#[cfg(not(feature = "json"))]
pub(in crate::jit) extern "C" fn rt_u_json_parse(
    _ctx: *mut UbCtx,
    _s: i64,
    _free: i64,
) -> UbJsonRet {
    UbJsonRet {
        payload: 0,
        tag: -1,
    }
}

#[cfg(not(feature = "json"))]
pub(in crate::jit) extern "C" fn rt_u_json_map_get(
    _ctx: *mut UbCtx,
    _map_h: i64,
    _key_h: i64,
    _free_mask: i64,
) -> UbJsonRet {
    UbJsonRet {
        payload: 0,
        tag: -1,
    }
}

#[cfg(not(feature = "json"))]
pub(in crate::jit) extern "C" fn rt_u_json_list_len(_ctx: *mut UbCtx, _list_h: i64) -> i64 {
    -1
}

#[cfg(not(feature = "json"))]
pub(in crate::jit) extern "C" fn rt_u_json_list_get(
    _ctx: *mut UbCtx,
    _list_h: i64,
    _idx: i64,
) -> UbJsonRet {
    UbJsonRet {
        payload: 0,
        tag: -1,
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use crate::phstr::PhStr;

    fn parse(doc: &str) -> (i64, i64, UbCtx) {
        let mut ctx = UbCtx::new(&[]);
        let h = ctx.alloc(Value::Str(PhStr::new(doc)));
        let r = rt_u_json_parse(&mut ctx as *mut UbCtx, h, 0);
        (r.payload, r.tag, ctx)
    }

    #[test]
    fn parse_object_yields_jmap_handle_tag6() {
        let (payload, tag, ctx) = parse("{\"a\": 1, \"b\": \"hi\"}");
        assert_eq!(tag, 6, "object root → tag 6");
        assert!(payload >= 0, "a JMap handle is minted");
        assert!(
            matches!(ctx.handles.get(payload as usize), Some(Value::Map(_))),
            "the payload handle boxes a Value::Map"
        );
    }

    #[test]
    fn parse_array_yields_jlist_handle_tag5() {
        let (payload, tag, ctx) = parse("[1, 2, 3]");
        assert_eq!(tag, 5, "array root → tag 5");
        assert!(
            matches!(ctx.handles.get(payload as usize), Some(Value::List(_))),
            "the payload handle boxes a Value::List"
        );
    }

    #[test]
    fn parse_scalars() {
        assert_eq!(parse("42").0, 42, "Int payload is the value");
        assert_eq!(parse("42").1, 2, "Int → tag 2");
        assert_eq!(parse("true").0, 1, "Bool true → 1");
        assert_eq!(parse("true").1, 1, "Bool → tag 1");
        assert_eq!(parse("3.5").1, 3, "Float → tag 3");
        assert_eq!(
            f64::from_bits(parse("3.5").0 as u64),
            3.5,
            "Float bits round-trip"
        );
    }

    #[test]
    fn parse_json_null_is_tag0_not_phnull() {
        assert_eq!(
            parse("null").1,
            0,
            "the JSON literal null → Json.Null variant (tag 0)"
        );
    }

    #[test]
    fn parse_malformed_is_phorj_null_tag7() {
        assert_eq!(
            parse("{bad").1,
            JSON_TAG_PHNULL,
            "malformed → phorj null (tag 7)"
        );
        assert_eq!(parse("[1,").1, JSON_TAG_PHNULL, "truncated → phorj null");
    }

    #[test]
    fn parse_string_yields_str_handle_tag4() {
        let (payload, tag, ctx) = parse("\"hello world\"");
        assert_eq!(tag, 4, "string root → tag 4");
        assert_eq!(
            ctx.str_bytes(payload),
            Some(b"hello world".as_slice()),
            "the payload is a readable str handle"
        );
    }

    #[test]
    fn map_get_hit_scalar_string_and_miss() {
        let mut ctx = UbCtx::new(&[]);
        let doc = ctx.alloc(Value::Str(PhStr::new("{\"a\": 42, \"b\": \"hi\"}")));
        let root = rt_u_json_parse(&mut ctx as *mut UbCtx, doc, 0);
        assert_eq!(root.tag, 6);
        let jmap = root.payload;
        let ka = ctx.alloc(Value::Str(PhStr::new("a")));
        let ra = rt_u_json_map_get(&mut ctx as *mut UbCtx, jmap, ka, 0);
        assert_eq!((ra.tag, ra.payload), (2, 42), "\"a\" → Int 42");
        let kb = ctx.alloc(Value::Str(PhStr::new("b")));
        let rb = rt_u_json_map_get(&mut ctx as *mut UbCtx, jmap, kb, 0);
        assert_eq!(rb.tag, 4, "\"b\" → String");
        assert_eq!(ctx.str_bytes(rb.payload), Some(b"hi".as_slice()));
        let kz = ctx.alloc(Value::Str(PhStr::new("z")));
        let rz = rt_u_json_map_get(&mut ctx as *mut UbCtx, jmap, kz, 0);
        assert_eq!(rz.tag, JSON_TAG_PHNULL, "missing key → phorj null (tag 7)");
    }

    #[test]
    fn list_len_get_and_oob() {
        let mut ctx = UbCtx::new(&[]);
        let doc = ctx.alloc(Value::Str(PhStr::new("[10, 20, 30]")));
        let root = rt_u_json_parse(&mut ctx as *mut UbCtx, doc, 0);
        assert_eq!(root.tag, 5);
        let jlist = root.payload;
        assert_eq!(rt_u_json_list_len(&mut ctx as *mut UbCtx, jlist), 3);
        let e0 = rt_u_json_list_get(&mut ctx as *mut UbCtx, jlist, 0);
        assert_eq!((e0.tag, e0.payload), (2, 10), "xs[0] → Int 10");
        assert_eq!(
            rt_u_json_list_get(&mut ctx as *mut UbCtx, jlist, 2).payload,
            30
        );
        assert!(
            rt_u_json_list_get(&mut ctx as *mut UbCtx, jlist, 3).tag < 0,
            "OOB → code-5 fault"
        );
        assert!(
            rt_u_json_list_get(&mut ctx as *mut UbCtx, jlist, -1).tag < 0,
            "negative index → code-5 fault"
        );
    }

    #[test]
    fn map_get_returns_nested_container_lazily() {
        // {"data": [1, 2]} — get "data" → Array (tag 5), whose length is readable. This is the
        // deepjson `firstRecord` shape (Object → Map.get("data") → Array → xs[0]).
        let mut ctx = UbCtx::new(&[]);
        let doc = ctx.alloc(Value::Str(PhStr::new("{\"data\": [1, 2]}")));
        let root = rt_u_json_parse(&mut ctx as *mut UbCtx, doc, 0);
        let jmap = root.payload;
        let kd = ctx.alloc(Value::Str(PhStr::new("data")));
        let rd = rt_u_json_map_get(&mut ctx as *mut UbCtx, jmap, kd, 0);
        assert_eq!(rd.tag, 5, "\"data\" → Array");
        assert_eq!(rt_u_json_list_len(&mut ctx as *mut UbCtx, rd.payload), 2);
    }
}

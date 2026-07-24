//! `Core.Native.Http` — the wire-parsing natives behind the DEC-331 slice-2 rich `Request`
//! (spec `docs/specs/2026-07-23-rich-request.md`). Std-only in its no-feature baseline; with the
//! `json` feature, [`json_parse_bytes`] delegates to the real `Core.Json` parser. The friendly
//! surface (Request + bags) is phorj prelude source in `cli::http_request_prelude` — these natives
//! do only what phorj can't express efficiently: percent/form decoding, multipart splitting, the
//! body spill store, and the JSON hand-off. One `eval` body serves interpreter AND VM (parity by
//! construction); each row carries a `php:` mapping so the class-shape transpile keeps working.
use super::{parg, NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{HKey, Value};
use std::rc::Rc;

mod multipart;
mod query;
mod spill;
pub(crate) use multipart::parse_multipart;
pub(crate) use query::{decode_path, parse_query_pairs};

/// The slice-2 request-body cap (D8c "size caps in v1"; D4's `maxBodySize` default). NOTE: equal to
/// the serve transport's whole-frame cap `MAX_REQUEST` (head+body), so via `phg serve` a body can
/// never reach this limit in slice 2 — it is live only through `Request.fake`/direct `Request.parse`
/// (see KNOWN_ISSUES). Slice 3 (`Http.ServeConfig`) folds this constant into `maxBodySize` and must
/// reconcile frame-cap vs body-cap semantics (a frame-truncated body reads as MALFORMED, not oversize).
pub const DEFAULT_MAX_BODY_SIZE: usize = 8_388_608;
/// D8c / §7 P1 (ruled 2026-07-23): request bodies above this spill to a temp file (in-memory below).
pub const SPILL_THRESHOLD: usize = 262_144;
/// Multipart part-count cap (PHP `max_input_vars`-shaped; recorded build decision — over-cap bodies
/// are DELIBERATELY classified malformed). Becomes a DEC-334 runtime-knob catalog row.
pub const MULTIPART_MAX_PARTS: usize = 1024;
/// Canonical fault strings (spec §5, fixed at build; Invariant 4 single-sourcing). Runtime-reachable
/// only from slice 3's LAZY access path — slice 2's eager parse returns null (→ the bridge's 400);
/// pinned by a test meanwhile so they cannot drift before their consumer lands.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "slice-3 lazy-mode consumer; panel-mandated early single-sourcing"
    )
)]
pub const FAULT_BODY_TOO_LARGE: &str = "request body exceeds maxBodySize";
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "slice-3 lazy-mode consumer; panel-mandated early single-sourcing"
    )
)]
pub const FAULT_MALFORMED_MULTIPART: &str = "malformed multipart body";

/// Build a phorj `Map<string, List<string>>` value from accumulated (key, values) pairs,
/// preserving FIRST-occurrence key order (D8b first-wins) with duplicate values appended.
fn pairs_to_map(pairs: Vec<(String, Vec<String>)>) -> Value {
    let entries: Vec<(HKey, Value)> = pairs
        .into_iter()
        .map(|(k, vs)| {
            let list: Vec<Value> = vs.into_iter().map(|v| Value::Str(v.into())).collect();
            (HKey::Str(k.into()), Value::List(Rc::new(list)))
        })
        .collect();
    Value::Map(Rc::new(entries))
}

fn native_parse_query(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(pairs_to_map(parse_query_pairs(s.as_str()))),
        _ => Err("Http.parseQuery expects (string)".into()),
    }
}

fn native_decode_path(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(decode_path(s.as_str()).into())),
        _ => Err("Http.decodePath expects (string)".into()),
    }
}

fn native_parse_multipart(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Bytes(body), Value::Str(boundary)] => {
            Ok(match parse_multipart(body, boundary.as_str()) {
                Some(parts) => Value::List(Rc::new(parts)),
                None => Value::Null,
            })
        }
        _ => Err("Http.parseMultipart expects (bytes, string)".into()),
    }
}

/// The body-stash decision, single-sourced with the limits (the prelude cannot read Rust consts):
/// `-2` = body exceeds [`DEFAULT_MAX_BODY_SIZE`] (eager → the prelude returns null → 400);
/// `-1` = at/below [`SPILL_THRESHOLD`], keep inline; `>= 0` = a spill handle. The PHP twin
/// `__phorj_http_stash_body` implements the identical contract.
fn native_stash_body(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Bytes(b)] => {
            if b.len() > DEFAULT_MAX_BODY_SIZE {
                Ok(Value::Int(-2))
            } else if b.len() <= SPILL_THRESHOLD {
                Ok(Value::Int(-1))
            } else {
                spill::store(b).map(Value::Int)
            }
        }
        _ => Err("Http.stashBody expects (bytes)".into()),
    }
}

fn native_read_spill(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(h)] => spill::read(*h).map(|b| Value::Bytes(Rc::new(b))),
        _ => Err("Http.readSpill expects (int)".into()),
    }
}

/// `jsonParse(bytes): Json?` — the ONLY parser `RequestBody.json()` references (never the
/// feature-gated `Json.parse` directly, so `Core.Http` keeps checking under no-default-features;
/// the `Json` TYPE is always injected). Invalid UTF-8 → null (not JSON), mirroring `Json.parse`'s
/// null-on-malformed. Without the `json` feature the call faults NAMING THE FLAG (DEC-273 spirit).
fn native_json_parse(args: &[Value], out: &mut String) -> Result<Value, String> {
    match args {
        [Value::Bytes(b)] => match std::str::from_utf8(b) {
            Ok(text) => json_parse_bytes(text, out),
            Err(_) => Ok(Value::Null),
        },
        _ => Err("Http.jsonParse expects (bytes)".into()),
    }
}

#[cfg(feature = "json")]
fn json_parse_bytes(text: &str, out: &mut String) -> Result<Value, String> {
    crate::ext::json::json_parse_str(text, out)
}

#[cfg(not(feature = "json"))]
fn json_parse_bytes(_text: &str, _out: &mut String) -> Result<Value, String> {
    Err(
        "`body.json()` requires the `json` feature, which is not compiled into this `phg` build \
         (rebuild with `--features json` or the default feature set)"
            .into(),
    )
}

pub(crate) fn http_natives() -> Vec<NativeFn> {
    let str_list_map = Ty::Map(
        Box::new(Ty::String),
        Box::new(Ty::List(Box::new(Ty::String))),
    );
    vec![
        NativeFn {
            module: "Core.Native.Http",
            name: "parseQuery",
            params: vec![Ty::String],
            ret: str_list_map,
            pure: true,
            eval: NativeEval::Pure(native_parse_query),
            lift_from: &[],
            php: |a| format!("__phorj_http_parse_query({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Native.Http",
            name: "decodePath",
            params: vec![Ty::String],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(native_decode_path),
            lift_from: &[],
            php: |a| format!("__phorj_http_decode_path({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Native.Http",
            name: "parseMultipart",
            params: vec![Ty::Bytes, Ty::String],
            ret: Ty::Optional(Box::new(Ty::List(Box::new(Ty::Named(
                "MultipartPart".into(),
                vec![],
            ))))),
            pure: true,
            eval: NativeEval::Pure(native_parse_multipart),
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_http_parse_multipart({}, {})",
                    parg(a, 0),
                    parg(a, 1)
                )
            },
        },
        // Deterministic spill HANDLES (0, 1, 2… per execution) — the temp-file PATH never enters
        // phorj (Invariant 10: a nondeterministic path in a value would break byte-identity).
        NativeFn {
            module: "Core.Native.Http",
            name: "stashBody",
            params: vec![Ty::Bytes],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(native_stash_body),
            lift_from: &[],
            php: |a| format!("__phorj_http_stash_body({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Native.Http",
            name: "readSpill",
            params: vec![Ty::Int],
            ret: Ty::Bytes,
            pure: true,
            eval: NativeEval::Pure(native_read_spill),
            lift_from: &[],
            php: |a| format!("__phorj_http_read_spill({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Native.Http",
            name: "jsonParse",
            params: vec![Ty::Bytes],
            ret: Ty::Optional(Box::new(Ty::Named("Json".into(), vec![]))),
            pure: true,
            eval: NativeEval::Pure(native_json_parse),
            lift_from: &[],
            php: |a| format!("__phorj_http_json_parse({})", parg(a, 0)),
        },
    ]
}

#[cfg(test)]
pub(crate) fn native_stash_for_tests(bytes: &[u8]) -> Result<i64, String> {
    let mut out = String::new();
    match native_stash_body(&[Value::Bytes(Rc::new(bytes.to_vec()))], &mut out)? {
        Value::Int(h) => Ok(h),
        other => Err(format!("int expected, got {}", other.type_name())),
    }
}
#[cfg(test)]
pub(crate) fn native_read_spill_for_tests(handle: i64) -> Result<Vec<u8>, String> {
    spill::read(handle)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;

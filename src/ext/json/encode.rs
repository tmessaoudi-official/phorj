//! JSON encoding (stringify): the compact/pretty tree walkers + PHP-matching string escaping,
//! kept byte-identical with the `__phorj_json_*` transpile helpers.

use super::parser::materialize_lazy;
use crate::value::{EnumVal, HKey, Value};
use std::fmt::Write as _;

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

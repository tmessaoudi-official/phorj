//! `Core.Convert` — explicit value conversion (`docs/specs/2026-06-26-m4-casting-conversion-design.md`,
//! axis 1). The *cast* (type assertion / reinterpret) is the `as` operator; this module produces a
//! **new value** of another type, always explicitly (Phorge has no implicit coercion). Lossy
//! conversions are *named* (`truncate`/`round`), never a silent `(int)`. Because UFCS ships,
//! `Convert.toFloat(n)` and `n.toFloat()` are the same call — module + method API in one.

use super::*;
use crate::types::Ty;
use crate::value::Value;

/// `Convert.toString(T) -> string` — generic, runtime-dispatched, reusing `Value::as_display` (the
/// same rendering as string interpolation / the PHP `__phorge_str` helper): bool → `true`/`false`,
/// float → shortest-round-trip, int/string verbatim. Byte-identity contract is the scalar types; a
/// composite value (list/map/instance) is not displayable → a clean fault (documented edge).
fn convert_to_string(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [v] => v
            .as_display()
            .map(Value::Str)
            .ok_or_else(|| format!("Convert.toString cannot convert {}", v.type_name())),
        _ => Err("Convert.toString expects (T)".into()),
    }
}

/// `Convert.toFloat(int) -> float` — total widening (Rust `as f64` ≡ PHP `(float)`).
fn convert_to_float(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Int(n)] => Ok(Value::Float(*n as f64)),
        _ => Err("Convert.toFloat expects (int)".into()),
    }
}

/// `Convert.truncate(float) -> int` — toward zero (Rust `as i64` ≡ PHP `(int)`). Lossy, named.
fn convert_truncate(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(f)] => Ok(Value::Int(*f as i64)),
        _ => Err("Convert.truncate expects (float)".into()),
    }
}

/// `Convert.round(float) -> int` — half away from zero (Rust `f.round()` ≡ PHP `round()` default).
fn convert_round(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Float(f)] => Ok(Value::Int(f.round() as i64)),
        _ => Err("Convert.round expects (float)".into()),
    }
}

pub(crate) fn convert_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Convert",
            name: "toString",
            params: vec![Ty::Param("T".into())],
            ret: Ty::String,
            pure: true,
            eval: NativeEval::Pure(convert_to_string),
            // Reuses the existing `__phorge_str` helper (gated via `uses_str`, set in transpile/call.rs).
            php: |a| format!("__phorge_str({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Convert",
            name: "toFloat",
            params: vec![Ty::Int],
            ret: Ty::Float,
            pure: true,
            eval: NativeEval::Pure(convert_to_float),
            php: |a| format!("(float)({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Convert",
            name: "truncate",
            params: vec![Ty::Float],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(convert_truncate),
            php: |a| format!("(int)({})", parg(a, 0)),
        },
        NativeFn {
            module: "Core.Convert",
            name: "round",
            params: vec![Ty::Float],
            ret: Ty::Int,
            pure: true,
            eval: NativeEval::Pure(convert_round),
            php: |a| format!("(int)round({})", parg(a, 0)),
        },
    ]
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;

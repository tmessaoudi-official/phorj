//! `Core.Encoding`'s charset natives — the thin wrapper over the always-compiled kernel in
//! [`crate::value::charset`]. DEC-468 names the surface; DEC-494 rules that both legs are
//! hand-rolled from one table (no `encoding_rs`, no ini extension), which is why the tables live in
//! `value/` where the transpiler can read them too.

use crate::charset::{decode, encode, Charset};
use crate::native::*;
use crate::types::Ty;
use crate::value::Value;
use std::rc::Rc;

/// Read the `Charset` argument. The checker types the parameter as the injected enum, so at
/// runtime it is always a `Value::Enum { ty: "Charset", .. }` (the `RoundingMode` precedent).
fn charset_arg(v: &Value) -> Result<Charset, String> {
    match v {
        Value::Enum(e) if e.ty.as_ref() == "Charset" => Charset::from_variant(&e.variant)
            .ok_or_else(|| format!("unknown Charset variant `{}`", e.variant)),
        _ => Err(format!("Charset expected, got {}", v.type_name())),
    }
}

pub(super) fn charset_decode_native(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Bytes(b), cs] => Ok(match decode(b, charset_arg(cs)?) {
            Some(s) => Value::Str(s.into()),
            None => Value::Null,
        }),
        _ => Err("Encoding.decode expects (bytes, Charset)".into()),
    }
}

pub(super) fn charset_encode_native(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s), cs] => Ok(match encode(s, charset_arg(cs)?) {
            Some(b) => Value::Bytes(Rc::new(b)),
            None => Value::Null,
        }),
        _ => Err("Encoding.encode expects (string, Charset)".into()),
    }
}

/// The two `Core.Encoding` charset rows, appended to [`super::natives::encoding_natives`].
pub(super) fn charset_natives() -> Vec<NativeFn> {
    // The injected `Charset` enum (cli::preludes, `Core.Encoding` row) — a bare `Ty::Named`, exactly
    // as `Core.Decimal` references `RoundingMode`. It resolves because calling `decode`/`encode`
    // requires `import Core.Encoding;`, which triggers injection before the checker runs.
    let cset = || Ty::Named("Charset".to_string(), vec![]);
    vec![
        NativeFn {
            module: "Core.Encoding",
            name: "decode",
            params: vec![Ty::Bytes, cset()],
            ret: Ty::Optional(Box::new(Ty::String)),
            pure: true,
            eval: NativeEval::Pure(charset_decode_native),
            // No PHP counterpart to lift FROM: `mb_convert_encoding`/`iconv` are ini extensions the
            // transpile rules forbid, which is the whole reason DEC-494 hand-rolls the helper.
            lift_from: &[],
            php: |a| format!("__phorj_cs_decode({}, {})", parg(a, 0), parg(a, 1)),
        },
        NativeFn {
            module: "Core.Encoding",
            name: "encode",
            params: vec![Ty::String, cset()],
            ret: Ty::Optional(Box::new(Ty::Bytes)),
            pure: true,
            eval: NativeEval::Pure(charset_encode_native),
            lift_from: &[],
            php: |a| format!("__phorj_cs_encode({}, {})", parg(a, 0), parg(a, 1)),
        },
    ]
}

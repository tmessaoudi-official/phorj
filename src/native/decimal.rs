//! `Core.Decimal` — runtime construction of the `decimal` primitive from a string (M-NUM S1). The
//! literal `19.99d` covers source constants; `Decimal.of(s)` covers dynamic/string input (parsed
//! input, config values), returning `decimal?` so a malformed or out-of-range string is a clean
//! `null` (composes with S2 `??`). Parsing is single-sourced in `value::decimal_of`, mirrored by the
//! PHP `__phorge_dec_of` PCRE helper (set in `transpile::emit_member_call`, the gated-helper pattern).

use super::*;
use crate::types::Ty;
use crate::value::Value;

/// `Decimal.of(string) -> decimal?` — parse the same grammar as a `…d` literal (optional sign, digits
/// with an optional single fractional part; NO exponent/underscore/whitespace), returning the decimal
/// or `null` on malformed input / i128 overflow. Shared by the interpreter and the VM.
fn decimal_of(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(match crate::value::decimal_of(s) {
            Some((unscaled, scale)) => Value::Decimal { unscaled, scale },
            None => Value::Null,
        }),
        _ => Err("Decimal.of expects (string)".into()),
    }
}

pub(crate) fn decimal_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.Decimal",
        name: "of",
        params: vec![Ty::String],
        ret: Ty::Optional(Box::new(Ty::Decimal)),
        pure: true,
        eval: NativeEval::Pure(decimal_of),
        // The `__phorge_dec_of` helper is gated on by `transpile::emit_member_call` (a native's `php`
        // closure has no `&mut self` to set `uses_dec_of`). Mirrors `value::decimal_of`.
        php: |a| format!("__phorge_dec_of({})", parg(a, 0)),
    }]
}

#[cfg(test)]
#[path = "decimal_tests.rs"]
mod tests;

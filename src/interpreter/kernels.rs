//! Interpreter — free evaluation kernels: arith/bitwise/compare projections over the
//! single-sourced `value` kernels, pattern matching.

use super::*;

/// True iff `#[UncheckedOverflow]` (or `#[Core.Runtime.Integer.UncheckedOverflow]`) is among a free function's attributes — the single
/// source of the wrap fact, read at the `run_call` boundary. The checker has already validated the
/// attribute (recognized + import-gated), so presence alone is authoritative here.
pub(super) fn attrs_unchecked(attrs: &[crate::ast::Attribute]) -> bool {
    attrs.iter().any(|a| a.is_unchecked_overflow())
}

pub(super) fn arith(op: BinaryOp, l: Value, r: Value, unchecked: bool) -> R<Value> {
    use BinaryOp::*;
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => {
            // Checked ops via the single-sourced `value` kernels: overflow / div-zero / mod-zero are
            // faults the type system can't catch, so they become a Diagnostic, never a panic
            // (EV-7). The VM dispatches into the *same* kernels, so the fault path can't diverge.
            // `#[UncheckedOverflow]`: int `+`/`-`/`*` WRAP (the `int_wrapping_*` kernels the VM also calls) —
            // div/rem stay checked (div-zero must always fault). `Ok(..)` so the match below is uniform.
            let v = match (op, unchecked) {
                (Add, true) => Ok(crate::value::int_wrapping_add(a, b)),
                (Sub, true) => Ok(crate::value::int_wrapping_sub(a, b)),
                (Mul, true) => Ok(crate::value::int_wrapping_mul(a, b)),
                (Add, false) => crate::value::int_add(a, b),
                (Sub, false) => crate::value::int_sub(a, b),
                (Mul, false) => crate::value::int_mul(a, b),
                (Pow, _) => crate::value::int_pow(a, b),
                (Div, _) => crate::value::int_div(a, b),
                (Rem, _) => crate::value::int_rem(a, b),
                _ => unreachable!("arith only called with +-*/%**"),
            };
            match v {
                Ok(n) => Ok(Value::Int(n)),
                Err(msg) => rt(msg),
            }
        }
        (Value::Float(a), Value::Float(b)) => {
            // `+ - * **` are infallible on `f64`; `/` and `%` fault on a zero divisor (the same
            // kernels the VM's `DivF`/`RemF` call, so the fault path can't diverge).
            match op {
                Add => Ok(Value::Float(crate::value::float_add(a, b))),
                Sub => Ok(Value::Float(crate::value::float_sub(a, b))),
                Mul => Ok(Value::Float(crate::value::float_mul(a, b))),
                Pow => Ok(Value::Float(crate::value::float_pow(a, b))),
                Div => match crate::value::float_div(a, b) {
                    Ok(n) => Ok(Value::Float(n)),
                    Err(msg) => rt(msg),
                },
                Rem => match crate::value::float_rem(a, b) {
                    Ok(n) => Ok(Value::Float(n)),
                    Err(msg) => rt(msg),
                },
                _ => unreachable!("arith only called with +-*/%**"),
            }
        }
        // `decimal` arithmetic (M-NUM S1): `+ - *` over a decimal — including a mixed `decimal`/`int`
        // pair (the kernel widens the int to scale 0) — dispatches into the single-sourced
        // `value::decimal_*` kernels the VM's `AddD/SubD/MulD` ops also call, so the exact result and
        // the i128-overflow fault are byte-identical. The checker rejects decimal `/`/`%` (S2), so
        // only `Add/Sub/Mul` reach here; a stray `Div/Rem` is a checker-unreachable defensive error.
        (l @ Value::Decimal { .. }, r) | (l, r @ Value::Decimal { .. })
            if matches!(r, Value::Decimal { .. } | Value::Int(_))
                && matches!(l, Value::Decimal { .. } | Value::Int(_)) =>
        {
            let res = match op {
                Add => crate::value::decimal_add(&l, &r),
                Sub => crate::value::decimal_sub(&l, &r),
                Mul => crate::value::decimal_mul(&l, &r),
                Rem => crate::value::decimal_rem(&l, &r),
                Div => crate::value::decimal_div_exact(&l, &r),
                _ => unreachable!("decimal arith only +-*/%"),
            };
            match res {
                Ok(v) => Ok(v),
                Err(msg) => rt(msg),
            }
        }
        // `string + string` → concatenation (Phase 1 string slice). The checker guarantees `+` is
        // the only op and both sides are `string`; the VM lowers this to `Op::Concat(2)`, whose
        // two-`Str` result is exactly `a + b`, so the backends stay byte-identical.
        (Value::Str(a), Value::Str(b)) if matches!(op, Add) => {
            Ok(Value::Str(crate::phstr::PhStr::concat(&a, &b)))
        }
        (l, r) => rt(format!(
            "cannot apply {op:?} to {} and {}",
            l.type_name(),
            r.type_name()
        )),
    }
}

/// Bitwise binaries on ints (primitives P2) — the same single-sourced `value` kernels the VM uses,
/// so a negative-shift fault can't diverge between backends.
pub(super) fn bitwise(op: BinaryOp, l: Value, r: Value) -> R<Value> {
    use BinaryOp::*;
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => match op {
            BitAnd => Ok(Value::Int(crate::value::int_bitand(a, b))),
            BitOr => Ok(Value::Int(crate::value::int_bitor(a, b))),
            BitXor => Ok(Value::Int(crate::value::int_bitxor(a, b))),
            Shl => match crate::value::int_shl(a, b) {
                Ok(n) => Ok(Value::Int(n)),
                Err(msg) => rt(msg),
            },
            Shr => match crate::value::int_shr(a, b) {
                Ok(n) => Ok(Value::Int(n)),
                Err(msg) => rt(msg),
            },
            _ => unreachable!("bitwise only called with & | ^ << >>"),
        },
        (l, r) => rt(format!(
            "cannot apply {op:?} to {} and {}",
            l.type_name(),
            r.type_name()
        )),
    }
}

pub(super) fn compare(op: BinaryOp, l: Value, r: Value) -> R<Value> {
    use BinaryOp::*;
    // The ordering + comparability fault is single-sourced in `value::compare_ord` (the VM calls the
    // same fn); only the op→bool projection below is backend-local (the op enums differ).
    let ord = match crate::value::compare_ord(&l, &r) {
        Ok(o) => o,
        Err(msg) => return rt(msg),
    };
    let res = match ord {
        Some(o) => match op {
            Lt => o.is_lt(),
            Gt => o.is_gt(),
            Le => o.is_le(),
            Ge => o.is_ge(),
            _ => unreachable!("compare only called with < > <= >="),
        },
        None => false, // NaN compares false
    };
    Ok(Value::Bool(res))
}

/// Try to match `pat` against `value`, pushing any bindings. Returns whether it matched. `implements`
/// is the shared `class_implements` table (needed by a type pattern to test an interface RHS — the
/// same data the `instanceof` evaluation uses, so the two can't diverge).
#[allow(clippy::float_cmp)] // intentional: literal float patterns match exactly
pub(super) fn match_pattern(
    pat: &Pattern,
    value: &Value,
    implements: &std::collections::BTreeMap<String, Vec<String>>,
    out: &mut Vec<(String, Value)>,
) -> bool {
    match pat {
        Pattern::Wildcard(_) => true,
        Pattern::Binding { name, .. } => {
            out.push((name.clone(), value.clone()));
            true
        }
        Pattern::Int(n, _) => matches!(value, Value::Int(v) if v == n),
        Pattern::Float(x, _) => matches!(value, Value::Float(v) if v == x),
        // A decimal literal pattern matches numerically (scale-insensitive, like `==`): `1.5d` matches
        // a `1.50d` scrutinee. Reuse the value-equality kernel via a fresh `Value::Decimal` (M-NUM S1).
        Pattern::Decimal {
            unscaled, scale, ..
        } => value.eq_val(&Value::Decimal {
            unscaled: *unscaled,
            scale: *scale,
        }),
        Pattern::Str(s, _) => matches!(value, Value::Str(v) if v == s),
        Pattern::Bool(b, _) => matches!(value, Value::Bool(v) if v == b),
        Pattern::Null(_) => matches!(value, Value::Null), // M3 S2.6: `null` arm over a `T?`
        Pattern::Variant { name, fields, .. } => {
            // A lazy Json node (DEC-294) materializes one level here, before the variant test.
            #[cfg(feature = "json")]
            if let Value::JsonLazy(l) = value {
                let m = crate::ext::json::materialize_lazy(l);
                return match_pattern(pat, &m, implements, out);
            }
            if let Value::Enum(ev) = value {
                if ev.variant.as_ref() == name.as_str() && ev.payload.len() == fields.len() {
                    return fields
                        .iter()
                        .zip(ev.payload.iter())
                        .all(|(fp, fv)| match_pattern(fp, fv, implements, out));
                }
            }
            false
        }
        // M-RT S4 type pattern: matches iff `value` is an instance whose class equals `type_name` or
        // implements interface `type_name` — exactly the `instanceof` test (`eval` arm above), so the
        // backends agree. Binds the matched value (if a binder is present).
        Pattern::Type {
            type_name, binding, ..
        } => {
            // Wave A: a primitive type-pattern (`int i`, `string s`) dispatches by `Value` variant —
            // the oracle for the VM's `Op::IsInstance` primitive arm and PHP's `is_int()`/`is_float()`
            // /`is_string()`/`is_bool()`/`is_null()`. Anything else is the class/interface `instanceof`.
            let is = match type_name.as_str() {
                "int" => matches!(value, Value::Int(_)),
                "float" => matches!(value, Value::Float(_)),
                "string" => matches!(value, Value::Str(_)),
                "bool" => matches!(value, Value::Bool(_)),
                "null" => matches!(value, Value::Null),
                _ => matches!(value, Value::Instance(inst)
                    if inst.class.as_ref() == type_name.as_str()
                        || implements
                            .get(&*inst.class)
                            .is_some_and(|ifaces| ifaces.iter().any(|i| i == type_name))),
            };
            if is {
                if let Some(name) = binding {
                    out.push((name.clone(), value.clone()));
                }
            }
            is
        }
        // S5.2 struct pattern: matches iff `value` is an instance of `type_name` (same `instanceof`
        // test as a type pattern), then each named field's sub-pattern matches that field's value.
        // A field absent at runtime (a declared-but-uninitialized explicit field) is a no-match here;
        // struct patterns are intended for classes whose fields are all initialized — promoted ctor
        // params — exactly like a direct `obj.field` read (KNOWN_ISSUES).
        Pattern::Struct {
            type_name, fields, ..
        } => {
            let is = matches!(value, Value::Instance(inst)
                if inst.class.as_ref() == type_name.as_str()
                    || implements
                        .get(&*inst.class)
                        .is_some_and(|ifaces| ifaces.iter().any(|i| i == type_name)));
            if !is {
                return false;
            }
            if let Value::Instance(inst) = value {
                for fp in fields {
                    let fv = inst.get_field(&fp.field);
                    match fv {
                        Some(v) => {
                            if !match_pattern(&fp.pat, &v, implements, out) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
            }
            true
        }
    }
}

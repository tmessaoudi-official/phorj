//! Unit tests for the `Core.Result` combinator native bodies (Wave B B-2b, DEC-185). The higher-order
//! ones are driven with a mock [`ClosureInvoker`] applying a fixed transform, so the test exercises the
//! native's own logic (Success/Failure dispatch, re-wrap, pass-through, error threading) independent of a
//! backend. End-to-end byte-identity across interp/VM/PHP is covered by the differential example
//! `examples/guide/result-combinators.phg`.
use super::*;
use crate::value::Value;

/// A runtime `Result` value's variant + single int payload, for terse assertions.
fn probe(v: &Value) -> (String, Option<i64>) {
    match v {
        Value::Enum(e) if e.ty.as_ref() == "Result" => (
            e.variant.to_string(),
            e.payload.first().and_then(|p| match p {
                Value::Int(n) => Some(*n),
                _ => None,
            }),
        ),
        _ => ("<not-result>".into(), None),
    }
}

/// The `i64` inside a `Value::Int`, else `None` (`Value` has no `PartialEq`, so tests compare unwrapped
/// scalars rather than whole `Value`s — same discipline as the Option tests' `probe`).
fn int_of(v: &Value) -> Option<i64> {
    match v {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}
/// The `bool` inside a `Value::Bool`, else `None`.
fn bool_of(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// A runtime `Option` value's variant + single int payload (for `toOption`).
fn probe_opt(v: &Value) -> (String, Option<i64>) {
    match v {
        Value::Enum(e) if e.ty.as_ref() == "Option" => (
            e.variant.to_string(),
            e.payload.first().and_then(|p| match p {
                Value::Int(n) => Some(*n),
                _ => None,
            }),
        ),
        _ => ("<not-option>".into(), None),
    }
}

#[test]
fn map_transforms_success_and_passes_failure_through() {
    let mut times_ten = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(n)] => Ok(Value::Int(n * 10)),
        _ => Err("bad".into()),
    };
    let s = result_map(&[success(Value::Int(4)), Value::Null], &mut times_ten).unwrap();
    assert_eq!(probe(&s), ("Success".into(), Some(40)));
    // Failure passes through untouched (the error payload is preserved).
    let fl = result_map(&[failure(Value::Int(7)), Value::Null], &mut times_ten).unwrap();
    assert_eq!(probe(&fl), ("Failure".into(), Some(7)));
}

#[test]
fn map_err_transforms_failure_and_passes_success_through() {
    let mut plus_one = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(n)] => Ok(Value::Int(n + 1)),
        _ => Err("bad".into()),
    };
    let fl = result_map_err(&[failure(Value::Int(4)), Value::Null], &mut plus_one).unwrap();
    assert_eq!(probe(&fl), ("Failure".into(), Some(5)));
    // Success passes through untouched (the value payload is preserved).
    let s = result_map_err(&[success(Value::Int(9)), Value::Null], &mut plus_one).unwrap();
    assert_eq!(probe(&s), ("Success".into(), Some(9)));
}

#[test]
fn and_then_binds_success_without_double_wrapping() {
    // f returns a Result directly; Success(x) becomes f(x), Failure passes through.
    let mut bind = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(n)] => Ok(success(Value::Int(n + 100))),
        _ => Err("bad".into()),
    };
    let s = result_and_then(&[success(Value::Int(1)), Value::Null], &mut bind).unwrap();
    assert_eq!(probe(&s), ("Success".into(), Some(101)));
    let fl = result_and_then(&[failure(Value::Int(2)), Value::Null], &mut bind).unwrap();
    assert_eq!(probe(&fl), ("Failure".into(), Some(2)));
}

#[test]
fn or_else_binds_failure_without_double_wrapping() {
    // f returns a Result directly; Failure(e) becomes f(e), Success passes through.
    let mut recover = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(_)] => Ok(success(Value::Int(0))),
        _ => Err("bad".into()),
    };
    let recovered = result_or_else(&[failure(Value::Int(9)), Value::Null], &mut recover).unwrap();
    assert_eq!(probe(&recovered), ("Success".into(), Some(0)));
    let s = result_or_else(&[success(Value::Int(5)), Value::Null], &mut recover).unwrap();
    assert_eq!(probe(&s), ("Success".into(), Some(5)));
}

#[test]
fn get_or_else_returns_value_or_eager_default() {
    let mut noop = String::new();
    let v = result_get_or_else(&[success(Value::Int(3)), Value::Int(99)], &mut noop).unwrap();
    assert_eq!(int_of(&v), Some(3));
    let d = result_get_or_else(&[failure(Value::Int(1)), Value::Int(99)], &mut noop).unwrap();
    assert_eq!(int_of(&d), Some(99));
}

#[test]
fn to_option_drops_error() {
    let mut noop = String::new();
    let s = result_to_option(&[success(Value::Int(8))], &mut noop).unwrap();
    assert_eq!(probe_opt(&s), ("Some".into(), Some(8)));
    let n = result_to_option(&[failure(Value::Int(1))], &mut noop).unwrap();
    assert_eq!(probe_opt(&n), ("None".into(), None));
}

#[test]
fn predicates_discriminate_the_arms() {
    let mut noop = String::new();
    assert_eq!(
        bool_of(&result_is_success(&[success(Value::Int(1))], &mut noop).unwrap()),
        Some(true)
    );
    assert_eq!(
        bool_of(&result_is_success(&[failure(Value::Int(1))], &mut noop).unwrap()),
        Some(false)
    );
    assert_eq!(
        bool_of(&result_is_failure(&[failure(Value::Int(1))], &mut noop).unwrap()),
        Some(true)
    );
    assert_eq!(
        bool_of(&result_is_failure(&[success(Value::Int(1))], &mut noop).unwrap()),
        Some(false)
    );
}

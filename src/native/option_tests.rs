//! Unit tests for the `Core.Option` combinator/conversion native bodies (Wave B B-2a). The
//! higher-order ones are driven with a mock [`ClosureInvoker`] applying a fixed transform, so the
//! test exercises the native's own logic (Some/None dispatch, re-wrap, pass-through) independent of a
//! backend. End-to-end byte-identity across run/runvm/PHP is covered by the differential example
//! `examples/guide/option-combinators.phg`.
use super::*;
use crate::value::Value;

/// A runtime `Option` value's variant + single int payload (or `None`), for terse assertions.
fn probe(v: &Value) -> (String, Option<i64>) {
    match v {
        Value::Enum(e) if e.ty == "Option" => (
            e.variant.clone(),
            e.payload.first().and_then(|p| match p {
                Value::Int(n) => Some(*n),
                _ => None,
            }),
        ),
        _ => ("<not-option>".into(), None),
    }
}

#[test]
fn map_transforms_some_and_passes_none_through() {
    let mut times_ten = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(n)] => Ok(Value::Int(n * 10)),
        _ => Err("bad".into()),
    };
    let s = option_map(&[some(Value::Int(4)), Value::Null], &mut times_ten).unwrap();
    assert_eq!(probe(&s), ("Some".into(), Some(40)));
    let n = option_map(&[none(), Value::Null], &mut times_ten).unwrap();
    assert_eq!(probe(&n), ("None".into(), None));
}

#[test]
fn and_then_binds_without_double_wrapping() {
    // f returns an Option directly (here Some(x + 100)); Some(x) becomes f(x), None passes through.
    let mut bind = |_f: &Value, args: Vec<Value>| match args.as_slice() {
        [Value::Int(n)] => Ok(some(Value::Int(n + 100))),
        _ => Err("bad".into()),
    };
    let s = option_and_then(&[some(Value::Int(5)), Value::Null], &mut bind).unwrap();
    assert_eq!(probe(&s), ("Some".into(), Some(105)));
    let n = option_and_then(&[none(), Value::Null], &mut bind).unwrap();
    assert_eq!(probe(&n), ("None".into(), None));
}

#[test]
fn filter_keeps_only_a_passing_some() {
    let mut keep = |_f: &Value, _: Vec<Value>| Ok(Value::Bool(true));
    let mut drop = |_f: &Value, _: Vec<Value>| Ok(Value::Bool(false));
    assert_eq!(
        probe(&option_filter(&[some(Value::Int(5)), Value::Null], &mut keep).unwrap()),
        ("Some".into(), Some(5))
    );
    assert_eq!(
        probe(&option_filter(&[some(Value::Int(5)), Value::Null], &mut drop).unwrap()),
        ("None".into(), None)
    );
    assert_eq!(
        probe(&option_filter(&[none(), Value::Null], &mut keep).unwrap()),
        ("None".into(), None)
    );
}

#[test]
fn filter_rejects_a_non_bool_predicate_result() {
    let mut bad = |_f: &Value, _: Vec<Value>| Ok(Value::Int(1));
    assert!(option_filter(&[some(Value::Int(5)), Value::Null], &mut bad).is_err());
}

#[test]
fn get_or_else_unwraps_or_defaults() {
    let mut out = String::new();
    assert!(matches!(
        option_get_or_else(&[some(Value::Int(5)), Value::Int(0)], &mut out).unwrap(),
        Value::Int(5)
    ));
    assert!(matches!(
        option_get_or_else(&[none(), Value::Int(99)], &mut out).unwrap(),
        Value::Int(99)
    ));
}

#[test]
fn of_nullable_and_to_nullable_round_trip() {
    let mut out = String::new();
    // null => None, value => Some(value)
    assert_eq!(
        probe(&option_of_nullable(&[Value::Null], &mut out).unwrap()),
        ("None".into(), None)
    );
    assert_eq!(
        probe(&option_of_nullable(&[Value::Int(42)], &mut out).unwrap()),
        ("Some".into(), Some(42))
    );
    // Some(x) => x, None => null
    assert!(matches!(
        option_to_nullable(&[some(Value::Int(7))], &mut out).unwrap(),
        Value::Int(7)
    ));
    assert!(matches!(
        option_to_nullable(&[none()], &mut out).unwrap(),
        Value::Null
    ));
}

#[test]
fn natives_registered_under_core_option() {
    let names: Vec<&str> = option_natives().iter().map(|n| n.name).collect();
    for expected in [
        "map",
        "andThen",
        "filter",
        "getOrElse",
        "ofNullable",
        "toNullable",
    ] {
        assert!(names.contains(&expected), "missing Core.Option.{expected}");
    }
    assert!(option_natives().iter().all(|n| n.module == "Core.Option"));
}

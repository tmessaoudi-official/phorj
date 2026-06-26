use super::*;
use crate::value::Value;

#[test]
fn convert_to_string_dispatches_by_type() {
    let s = |v: Value| {
        let mut o = String::new();
        convert_to_string(&[v], &mut o).unwrap()
    };
    assert!(matches!(s(Value::Int(5)), Value::Str(t) if t == "5"));
    assert!(matches!(s(Value::Int(-7)), Value::Str(t) if t == "-7"));
    assert!(matches!(s(Value::Bool(true)), Value::Str(t) if t == "true"));
    assert!(matches!(s(Value::Bool(false)), Value::Str(t) if t == "false"));
    assert!(matches!(s(Value::Float(2.5)), Value::Str(t) if t == "2.5"));
    assert!(matches!(s(Value::Float(5.0)), Value::Str(t) if t == "5")); // shortest-round-trip
    assert!(matches!(s(Value::Str("hi".into())), Value::Str(t) if t == "hi"));
}

#[test]
fn convert_numeric() {
    let mut o = String::new();
    assert!(matches!(convert_to_float(&[Value::Int(5)], &mut o), Ok(Value::Float(f)) if f == 5.0));
    assert!(
        matches!(convert_to_float(&[Value::Int(-3)], &mut o), Ok(Value::Float(f)) if f == -3.0)
    );
    // truncate toward zero.
    assert!(matches!(
        convert_truncate(&[Value::Float(3.9)], &mut o),
        Ok(Value::Int(3))
    ));
    assert!(matches!(
        convert_truncate(&[Value::Float(-3.9)], &mut o),
        Ok(Value::Int(-3))
    ));
    // round half away from zero.
    assert!(matches!(
        convert_round(&[Value::Float(3.5)], &mut o),
        Ok(Value::Int(4))
    ));
    assert!(matches!(
        convert_round(&[Value::Float(2.5)], &mut o),
        Ok(Value::Int(3))
    ));
    assert!(matches!(
        convert_round(&[Value::Float(-2.5)], &mut o),
        Ok(Value::Int(-3))
    ));
    assert!(matches!(
        convert_round(&[Value::Float(2.4)], &mut o),
        Ok(Value::Int(2))
    ));
}

#[test]
fn convert_natives_registered() {
    for name in ["toString", "toFloat", "truncate", "round"] {
        assert!(
            crate::native::index_of("Core.Convert", name).is_some(),
            "Core.Convert.{name} not registered"
        );
    }
}

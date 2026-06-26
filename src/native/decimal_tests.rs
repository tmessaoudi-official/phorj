//! `Core.Decimal` native tests (M-NUM S1).

use super::*;

#[test]
fn decimal_of_parses_or_returns_null() {
    let mut out = String::new();
    // valid → decimal
    assert!(matches!(
        decimal_of(&[Value::Str("12.34".into())], &mut out),
        Ok(Value::Decimal {
            unscaled: 1234,
            scale: 2
        })
    ));
    assert!(matches!(
        decimal_of(&[Value::Str("100".into())], &mut out),
        Ok(Value::Decimal {
            unscaled: 100,
            scale: 0
        })
    ));
    // malformed → null (not an error)
    assert!(matches!(
        decimal_of(&[Value::Str("nope".into())], &mut out),
        Ok(Value::Null)
    ));
    assert!(matches!(
        decimal_of(&[Value::Str("1e3".into())], &mut out),
        Ok(Value::Null)
    ));
}

#[test]
fn registry_exposes_decimal_of() {
    assert!(index_of("Core.Decimal", "of").is_some());
}

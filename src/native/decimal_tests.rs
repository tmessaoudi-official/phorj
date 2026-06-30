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

#[test]
fn registry_exposes_div_and_round() {
    assert!(index_of("Core.Decimal", "divide").is_some());
    assert!(index_of("Core.Decimal", "round").is_some());
}

/// Build a `RoundingMode` enum value (zero-payload) for a given variant name.
fn mode(variant: &str) -> Value {
    use crate::value::EnumVal;
    use std::rc::Rc;
    Value::Enum(Rc::new(EnumVal {
        ty: "RoundingMode".to_string(),
        variant: variant.to_string(),
        payload: vec![],
    }))
}

fn dec(unscaled: i128, scale: u8) -> Value {
    Value::Decimal { unscaled, scale }
}

#[test]
fn div_native_rounds_to_scale() {
    let mut out = String::new();
    // 10.00 / 3 at scale 2 HalfEven → 3.33.
    assert!(matches!(
        decimal_div_native(
            &[dec(1000, 2), Value::Int(3), Value::Int(2), mode("HalfEven")],
            &mut out
        ),
        Ok(Value::Decimal {
            unscaled: 333,
            scale: 2
        })
    ));
}

#[test]
fn div_native_by_zero_faults() {
    let mut out = String::new();
    assert_eq!(
        decimal_div_native(
            &[dec(1000, 2), Value::Int(0), Value::Int(2), mode("HalfUp")],
            &mut out
        )
        .err()
        .as_deref(),
        Some(crate::value::FAULT_DECIMAL_DIV_ZERO)
    );
}

#[test]
fn div_native_negative_scale_faults() {
    let mut out = String::new();
    assert_eq!(
        decimal_div_native(
            &[dec(1000, 2), Value::Int(3), Value::Int(-1), mode("HalfUp")],
            &mut out
        )
        .err()
        .as_deref(),
        Some(crate::value::FAULT_DECIMAL_SCALE)
    );
}

#[test]
fn round_native_rounds_a_tie() {
    let mut out = String::new();
    // 2.345 → scale 2 HalfUp → 2.35.
    assert!(matches!(
        decimal_round_native(&[dec(2345, 3), Value::Int(2), mode("HalfUp")], &mut out),
        Ok(Value::Decimal {
            unscaled: 235,
            scale: 2
        })
    ));
    // 2.345 → scale 2 HalfEven → 2.34.
    assert!(matches!(
        decimal_round_native(&[dec(2345, 3), Value::Int(2), mode("HalfEven")], &mut out),
        Ok(Value::Decimal {
            unscaled: 234,
            scale: 2
        })
    ));
}

#[test]
fn round_native_negative_scale_faults() {
    let mut out = String::new();
    assert_eq!(
        decimal_round_native(&[dec(2345, 3), Value::Int(-1), mode("HalfUp")], &mut out)
            .err()
            .as_deref(),
        Some(crate::value::FAULT_DECIMAL_SCALE)
    );
}

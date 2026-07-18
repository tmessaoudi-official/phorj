//! Runtime values for both backends. The M1 heap is **immutable + acyclic**: no reassignment, no
//! post-construction field mutation, and a constructor's args are fully evaluated before the
//! instance exists (EV-1). So compound objects are *shared* via `Rc`, not deep-cloned (M2 P5a):
//! cloning a `Value` (the `Op::GetLocal` hot path + every interpreter var-read) is a refcount bump,
//! and `Drop` reclaims correctly — no cycle can leak, so no tracing collector is needed (that is
//! deferred to M3, when mutation could create cycles). See `docs/specs/2026-06-16-m2-p5-object-model-design.md`.

use crate::green::sched::{ChanId, TaskId};
use crate::phstr::PhStr;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

mod arith;
mod collections;
mod core_impl;
mod db;
mod decimal;
mod types;

pub use self::arith::*;
pub use self::collections::*;
pub use self::core_impl::*;
pub use self::db::*;
pub use self::decimal::*;
pub use self::types::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(unscaled: i128, scale: u8) -> Value {
        Value::Decimal { unscaled, scale }
    }

    #[test]
    fn int_intdiv_truncates_and_faults() {
        assert_eq!(int_intdiv(7, 2), Ok(3));
        assert_eq!(int_intdiv(-7, 2), Ok(-3)); // toward zero
        assert_eq!(int_intdiv(7, -2), Ok(-3));
        assert_eq!(int_intdiv(-7, -2), Ok(3));
        assert_eq!(int_intdiv(6, 3), Ok(2));
        // divisor zero → division by zero fault
        assert_eq!(int_intdiv(5, 0).unwrap_err(), FAULT_DIV_ZERO);
        // i64::MIN / -1 overflows → integer overflow fault
        assert_eq!(int_intdiv(i64::MIN, -1).unwrap_err(), FAULT_INT_OVERFLOW);
    }

    #[test]
    fn float_div_rem_by_zero_fault() {
        // Non-zero divisor: ordinary IEEE result.
        assert_eq!(float_div(7.0, 2.0), Ok(3.5));
        assert_eq!(float_rem(7.5, 2.0), Ok(1.5));
        // Zero divisor faults (no IEEE inf/NaN) — both +0.0 and -0.0.
        assert_eq!(float_div(1.0, 0.0).unwrap_err(), FAULT_DIV_ZERO);
        assert_eq!(float_div(1.0, -0.0).unwrap_err(), FAULT_DIV_ZERO);
        assert_eq!(float_rem(1.0, 0.0).unwrap_err(), FAULT_MOD_ZERO);
        // A finite overflow to inf is NOT a zero division — it stays inf.
        assert!(float_div(1.0e308, 1.0e-308).unwrap().is_infinite());
    }

    #[test]
    fn decimal_rem_is_exact_and_faults_on_zero() {
        let d = |u, s| Value::Decimal {
            unscaled: u,
            scale: s,
        };
        // 10.50 % 3.00 = 1.50 (align to scale 2: 1050 % 300 = 150).
        assert!(matches!(
            decimal_rem(&d(1050, 2), &d(300, 2)),
            Ok(Value::Decimal {
                unscaled: 150,
                scale: 2
            })
        ));
        // Mixed scales align to max: 10.5 % 3 (int) → 1.5 (scale 1).
        assert!(matches!(
            decimal_rem(&d(105, 1), &Value::Int(3)),
            Ok(Value::Decimal {
                unscaled: 15,
                scale: 1
            })
        ));
        // Sign follows the dividend: -10 % 3 = -1.
        assert!(matches!(
            decimal_rem(&d(-10, 0), &d(3, 0)),
            Ok(Value::Decimal {
                unscaled: -1,
                scale: 0
            })
        ));
        // Zero divisor faults.
        assert_eq!(
            decimal_rem(&d(5, 0), &d(0, 0)).unwrap_err(),
            FAULT_DECIMAL_MOD_ZERO
        );
    }

    #[test]
    fn decimal_div_exact_terminates_or_faults() {
        let d = |u, s| Value::Decimal {
            unscaled: u,
            scale: s,
        };
        let dv = |a, b| decimal_div_exact(&a, &b);
        // Terminating quotients, minimal form.
        assert!(matches!(
            dv(d(10, 0), d(4, 0)),
            Ok(Value::Decimal {
                unscaled: 25,
                scale: 1
            })
        )); // 2.5
        assert!(matches!(
            dv(d(1, 0), d(8, 0)),
            Ok(Value::Decimal {
                unscaled: 125,
                scale: 3
            })
        )); // 0.125
        assert!(matches!(
            dv(d(10, 0), d(2, 0)),
            Ok(Value::Decimal {
                unscaled: 5,
                scale: 0
            })
        )); // 5
            // Trailing zeros are stripped to minimal form: 2.50 / 1 = 2.5 (matches the PHP rtrim).
        assert!(matches!(
            dv(d(250, 2), d(1, 0)),
            Ok(Value::Decimal {
                unscaled: 25,
                scale: 1
            })
        ));
        // Negative sign carries.
        assert!(matches!(
            dv(d(-1, 0), d(4, 0)),
            Ok(Value::Decimal {
                unscaled: -25,
                scale: 2
            })
        )); // -0.25
            // Non-terminating ⇒ fault.
        assert_eq!(
            dv(d(1, 0), d(3, 0)).unwrap_err(),
            FAULT_DECIMAL_NONTERMINATING
        );
        assert_eq!(
            dv(d(10, 0), d(6, 0)).unwrap_err(),
            FAULT_DECIMAL_NONTERMINATING
        ); // 5/3
           // Zero divisor ⇒ div-zero fault; 0/x = 0.
        assert_eq!(dv(d(5, 0), d(0, 0)).unwrap_err(), FAULT_DECIMAL_DIV_ZERO);
        assert!(matches!(
            dv(d(0, 0), d(5, 0)),
            Ok(Value::Decimal {
                unscaled: 0,
                scale: 0
            })
        ));
    }

    #[test]
    fn float_to_int_guards_the_edge() {
        assert_eq!(float_to_int(3.9), Some(3)); // truncate toward zero
        assert_eq!(float_to_int(-3.9), Some(-3));
        assert_eq!(float_to_int(0.0), Some(0));
        assert_eq!(float_to_int(42.0), Some(42));
        // special values → None (avoids PHP `(int)NAN == 0`)
        assert_eq!(float_to_int(f64::NAN), None);
        assert_eq!(float_to_int(f64::INFINITY), None);
        assert_eq!(float_to_int(f64::NEG_INFINITY), None);
        // out-of-range huge magnitudes → None
        assert_eq!(float_to_int(1e30), None);
        assert_eq!(float_to_int(-1e30), None);
        // near the i64 edge: `i64::MIN as f64` is exactly `-2^63` (representable, in-range);
        // `i64::MAX as f64` rounds UP to `2^63` (== the exclusive UPPER), so it is OUT — exactly the
        // edge the shared bound is chosen to close (Rust and PHP both reject `2^63`).
        assert_eq!(float_to_int(i64::MIN as f64), Some(i64::MIN));
        assert_eq!(float_to_int(i64::MAX as f64), None); // rounds to 2^63 == UPPER (exclusive)
                                                         // a large but exactly-representable in-range value (2^53) round-trips.
        assert_eq!(
            float_to_int(9_007_199_254_740_992.0),
            Some(9_007_199_254_740_992)
        );
    }

    #[test]
    fn decimal_to_int_truncates_toward_zero() {
        assert_eq!(decimal_to_int(&dec(1999, 2)), Some(19)); // 19.99 → 19
        assert_eq!(decimal_to_int(&dec(-1999, 2)), Some(-19)); // -19.99 → -19 (toward zero)
        assert_eq!(decimal_to_int(&dec(100, 0)), Some(100)); // 100 → 100
        assert_eq!(decimal_to_int(&dec(5, 4)), Some(0)); // 0.0005 → 0
        assert_eq!(decimal_to_int(&dec(-5, 1)), Some(0)); // -0.5 → 0
                                                          // integer part out of i64 range → None
        assert_eq!(decimal_to_int(&dec(i128::from(i64::MAX) + 1, 0)), None);
        assert_eq!(decimal_to_int(&dec(i128::from(i64::MIN) - 1, 0)), None);
    }

    #[test]
    fn fmt_decimal_renders_with_exact_scale() {
        assert_eq!(fmt_decimal(1999, 2), "19.99");
        assert_eq!(fmt_decimal(1500, 3), "1.500");
        assert_eq!(fmt_decimal(100, 0), "100");
        assert_eq!(fmt_decimal(15, 4), "0.0015");
        assert_eq!(fmt_decimal(0, 0), "0");
        assert_eq!(fmt_decimal(0, 2), "0.00");
        assert_eq!(fmt_decimal(-5000, 2), "-50.00");
        assert_eq!(fmt_decimal(-1, 3), "-0.001");
        // never `-0` even though the sign bit could be set (it can't be for 0, but guard anyway).
        assert_eq!(fmt_decimal(0, 4), "0.0000");
    }

    /// Assert a decimal-kernel `Ok` matches an expected `(unscaled, scale)` exactly (not just
    /// numerically — the scale is part of the result), since `Value` has no `PartialEq`.
    fn assert_dec(got: Result<Value, String>, unscaled: i128, scale: u8) {
        match got {
            Ok(Value::Decimal {
                unscaled: u,
                scale: s,
            }) => {
                assert_eq!((u, s), (unscaled, scale), "decimal result mismatch");
            }
            other => panic!("expected Ok(Decimal), got {other:?}"),
        }
    }

    #[test]
    fn decimal_add_sub_use_max_scale() {
        // 1.50 + 2.300 = 3.800 (scale 3); align the lower-scale operand up.
        assert_dec(decimal_add(&dec(150, 2), &dec(2300, 3)), 3800, 3);
        // 1.50 - 1.50 = 0.00 (scale 2, no neg zero in render).
        assert_dec(decimal_sub(&dec(150, 2), &dec(150, 2)), 0, 2);
        // mixed decimal + int: int widens to scale 0 → 19.99 + 1 = 20.99.
        assert_dec(decimal_add(&dec(1999, 2), &Value::Int(1)), 2099, 2);
        assert_dec(decimal_add(&Value::Int(1), &dec(1999, 2)), 2099, 2);
    }

    #[test]
    fn decimal_mul_sums_scales() {
        // 1.11 * 1.11 = 1.2321 (scale 4 = 2 + 2).
        assert_dec(decimal_mul(&dec(111, 2), &dec(111, 2)), 12321, 4);
        // decimal * int: 19.99 * 3 = 59.97 (scale 2, int scale 0).
        assert_dec(decimal_mul(&dec(1999, 2), &Value::Int(3)), 5997, 2);
    }

    fn assert_dec_overflow(got: Result<Value, String>) {
        assert_eq!(got.err().as_deref(), Some(FAULT_DECIMAL_OVERFLOW));
    }

    #[test]
    fn decimal_overflow_is_a_clean_fault() {
        let big = dec(i128::MAX, 0);
        assert_dec_overflow(decimal_add(&big, &Value::Int(1)));
        assert_dec_overflow(decimal_mul(&big, &Value::Int(2)));
        // Alignment overflow: scaling i128::MAX up by 10^1 overflows before the add.
        assert_dec_overflow(decimal_add(&big, &dec(0, 1)));
        // Negation of i128::MIN overflows.
        assert_dec_overflow(decimal_neg(i128::MIN, 0));
    }

    #[test]
    fn round_div_all_seven_modes_on_a_positive_tie() {
        // 5/2 = 2.5 — an exact tie; each mode resolves the half differently.
        use RoundMode::*;
        assert_eq!(round_div(5, 2, Down), Ok(2)); // toward 0
        assert_eq!(round_div(5, 2, Up), Ok(3)); // away from 0
        assert_eq!(round_div(5, 2, Ceiling), Ok(3)); // toward +inf
        assert_eq!(round_div(5, 2, Floor), Ok(2)); // toward -inf
        assert_eq!(round_div(5, 2, HalfUp), Ok(3)); // tie away
        assert_eq!(round_div(5, 2, HalfDown), Ok(2)); // tie toward 0
        assert_eq!(round_div(5, 2, HalfEven), Ok(2)); // tie to even (q=2 even)
    }

    #[test]
    fn round_div_all_seven_modes_on_a_negative_tie() {
        // -5/2 = -2.5 — mirror of the positive tie.
        use RoundMode::*;
        assert_eq!(round_div(-5, 2, Down), Ok(-2)); // toward 0
        assert_eq!(round_div(-5, 2, Up), Ok(-3)); // away from 0
        assert_eq!(round_div(-5, 2, Ceiling), Ok(-2)); // toward +inf
        assert_eq!(round_div(-5, 2, Floor), Ok(-3)); // toward -inf
        assert_eq!(round_div(-5, 2, HalfUp), Ok(-3)); // tie away
        assert_eq!(round_div(-5, 2, HalfDown), Ok(-2)); // tie toward 0
        assert_eq!(round_div(-5, 2, HalfEven), Ok(-2)); // tie to even (q=-2 even)
    }

    #[test]
    fn round_div_half_even_picks_the_odd_quotient_up() {
        // 7/2 = 3.5 — tie, q=3 is odd, so HalfEven rounds to the even 4.
        assert_eq!(round_div(7, 2, RoundMode::HalfEven), Ok(4));
        assert_eq!(round_div(-7, 2, RoundMode::HalfEven), Ok(-4));
    }

    #[test]
    fn round_div_non_tie_and_exact() {
        use RoundMode::*;
        // 7/3 = 2.333… — not a tie; the half rules don't trigger (rem < complement).
        assert_eq!(round_div(7, 3, HalfUp), Ok(2));
        assert_eq!(round_div(7, 3, Up), Ok(3));
        assert_eq!(round_div(7, 3, Down), Ok(2));
        assert_eq!(round_div(7, 3, Ceiling), Ok(3));
        assert_eq!(round_div(7, 3, Floor), Ok(2));
        // 8/3 = 2.666… — past the half, so HalfUp/HalfDown/HalfEven all round up.
        assert_eq!(round_div(8, 3, HalfUp), Ok(3));
        assert_eq!(round_div(8, 3, HalfDown), Ok(3));
        assert_eq!(round_div(8, 3, HalfEven), Ok(3));
        // Exact division: every mode agrees, no rounding.
        for m in [HalfUp, HalfDown, HalfEven, Up, Down, Ceiling, Floor] {
            assert_eq!(round_div(6, 3, m), Ok(2));
            assert_eq!(round_div(-6, 3, m), Ok(-2));
        }
    }

    #[test]
    fn round_div_negative_divisor_normalises() {
        // A negative divisor is normalised so the result matches the equivalent positive-divisor form.
        assert_eq!(
            round_div(5, -2, RoundMode::HalfUp),
            round_div(-5, 2, RoundMode::HalfUp)
        );
        assert_eq!(
            round_div(-5, -2, RoundMode::Up),
            round_div(5, 2, RoundMode::Up)
        );
    }

    #[test]
    fn decimal_div_rounds_to_scale() {
        // 10.00 / 3 = 3.3333… → scale 2 HalfEven → 3.33.
        assert_dec(
            decimal_div(&dec(1000, 2), &Value::Int(3), 2, RoundMode::HalfEven),
            333,
            2,
        );
        // 1 / 8 = 0.125 → scale 2 HalfUp → 0.13 (tie at the third digit).
        assert_dec(
            decimal_div(&Value::Int(1), &Value::Int(8), 2, RoundMode::HalfUp),
            13,
            2,
        );
        // 1 / 8 = 0.125 → scale 2 HalfEven → 0.12 (q=12 even).
        assert_dec(
            decimal_div(&Value::Int(1), &Value::Int(8), 2, RoundMode::HalfEven),
            12,
            2,
        );
    }

    #[test]
    fn decimal_div_by_zero_is_a_clean_fault() {
        assert_eq!(
            decimal_div(&dec(1000, 2), &dec(0, 2), 2, RoundMode::HalfUp)
                .err()
                .as_deref(),
            Some(FAULT_DECIMAL_DIV_ZERO)
        );
        // an int-zero divisor too (the int widens to scale 0).
        assert_eq!(
            decimal_div(&dec(1000, 2), &Value::Int(0), 2, RoundMode::HalfUp)
                .err()
                .as_deref(),
            Some(FAULT_DECIMAL_DIV_ZERO)
        );
    }

    #[test]
    fn decimal_div_negative_scale_is_a_clean_fault() {
        assert_eq!(
            decimal_div(&dec(1000, 2), &Value::Int(3), -1, RoundMode::HalfUp)
                .err()
                .as_deref(),
            Some(FAULT_DECIMAL_SCALE)
        );
    }

    #[test]
    fn decimal_round_up_and_down_scale() {
        // 2.345 → scale 2 HalfUp → 2.35 (tie rounds away).
        assert_dec(decimal_round(&dec(2345, 3), 2, RoundMode::HalfUp), 235, 2);
        // 2.345 → scale 2 HalfEven → 2.34 (q=234 even).
        assert_dec(decimal_round(&dec(2345, 3), 2, RoundMode::HalfEven), 234, 2);
        // up-scale is exact: 2.5 → scale 3 → 2.500 (no rounding).
        assert_dec(decimal_round(&dec(25, 1), 3, RoundMode::Down), 2500, 3);
        // same-scale is identity.
        assert_dec(decimal_round(&dec(1999, 2), 2, RoundMode::HalfUp), 1999, 2);
    }

    #[test]
    fn decimal_round_negative_scale_is_a_clean_fault() {
        assert_eq!(
            decimal_round(&dec(2345, 3), -1, RoundMode::HalfUp)
                .err()
                .as_deref(),
            Some(FAULT_DECIMAL_SCALE)
        );
    }

    #[test]
    fn decimal_div_overflow_is_a_clean_fault() {
        // A huge target scale overflows 10^k before the division.
        assert_eq!(
            decimal_div(&Value::Int(1), &Value::Int(3), 200, RoundMode::HalfUp)
                .err()
                .as_deref(),
            Some(FAULT_DECIMAL_OVERFLOW)
        );
    }

    #[test]
    fn decimal_cmp_and_eq_are_scale_insensitive() {
        // 1.50 == 1.5 numerically (scale-insensitive).
        assert_eq!(
            decimal_cmp(&dec(150, 2), &dec(15, 1)),
            Some(Ordering::Equal)
        );
        assert!(dec(150, 2).eq_val(&dec(15, 1)));
        assert!(!dec(150, 2).eq_val(&dec(151, 2)));
        // mixed decimal/int equality: 2.00d == 2.
        assert!(dec(200, 2).eq_val(&Value::Int(2)));
        assert!(Value::Int(2).eq_val(&dec(200, 2)));
        // ordering
        assert_eq!(decimal_cmp(&dec(149, 2), &dec(15, 1)), Some(Ordering::Less));
        assert_eq!(
            compare_ord(&dec(150, 2), &dec(15, 1)),
            Ok(Some(Ordering::Equal))
        );
        // a decimal never equals a float (no cross-type) — handled by eq_val_rec's `_ => false`.
        assert!(!dec(150, 2).eq_val(&Value::Float(1.5)));
    }

    #[test]
    fn decimal_of_parses_the_literal_grammar() {
        assert_eq!(decimal_of("12.34"), Some((1234, 2)));
        assert_eq!(decimal_of("100"), Some((100, 0)));
        assert_eq!(decimal_of("1.500"), Some((1500, 3))); // trailing zeros set scale
        assert_eq!(decimal_of("-0.50"), Some((-50, 2)));
        assert_eq!(decimal_of(".5"), Some((5, 1)));
        assert_eq!(decimal_of("+3.0"), Some((30, 1)));
        // malformed → None
        assert_eq!(decimal_of(""), None);
        assert_eq!(decimal_of("abc"), None);
        assert_eq!(decimal_of("1.2.3"), None);
        assert_eq!(decimal_of("12."), None); // empty fractional part
        assert_eq!(decimal_of("1e3"), None); // no exponent
        assert_eq!(decimal_of("1_000"), None); // no underscores at runtime
        assert_eq!(decimal_of(" 12"), None); // no surrounding whitespace
                                             // i128 overflow → None
        let too_big = "1".repeat(40);
        assert_eq!(decimal_of(&too_big), None);
    }

    #[test]
    fn decimal_as_display_matches_fmt() {
        assert_eq!(dec(1999, 2).as_display().as_deref(), Some("19.99"));
        assert_eq!(dec(100, 0).as_display().as_deref(), Some("100"));
        assert_eq!(dec(0, 2).as_display().as_deref(), Some("0.00"));
        assert_eq!(dec(150, 2).type_name(), "decimal");
    }

    #[test]
    fn int_pow_normal_negative_and_overflow() {
        // Normal non-negative powers.
        assert_eq!(int_pow(2, 10), Ok(1024));
        assert_eq!(int_pow(5, 0), Ok(1)); // anything ** 0 == 1
        assert_eq!(int_pow(7, 1), Ok(7));
        assert_eq!(int_pow(-2, 3), Ok(-8)); // negative base, odd exponent
                                            // A negative exponent can't be a typed `int` → clean fault (EV-7), never a panic.
        assert_eq!(int_pow(2, -1), Err(FAULT_NEGATIVE_EXPONENT.to_string()));
        // Overflow is a clean fault, both for an overflowing result and a huge exponent.
        assert_eq!(int_pow(2, 63), Err(FAULT_INT_OVERFLOW.to_string()));
        assert_eq!(int_pow(2, i64::MAX), Err(FAULT_INT_OVERFLOW.to_string()));
    }

    #[test]
    fn float_pow_matches_powf() {
        assert_eq!(float_pow(3.0, 2.0), 9.0);
        assert_eq!(float_pow(2.0, 10.0), 1024.0);
    }

    #[test]
    fn build_map_dedups_first_position_last_value() {
        // PHP semantics: a duplicate key keeps its first position but takes the last value.
        let m = build_map(vec![
            (Value::Str("a".into()), Value::Int(1)),
            (Value::Str("b".into()), Value::Int(2)),
            (Value::Str("a".into()), Value::Int(9)),
        ])
        .unwrap();
        // `Value` isn't `PartialEq` (holds `f64`), so compare keys directly + values via `eq_val`.
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].0, HKey::Str("a".into())); // first position kept
        assert!(m[0].1.eq_val(&Value::Int(9))); // last value taken
        assert_eq!(m[1].0, HKey::Str("b".into()));
        assert!(m[1].1.eq_val(&Value::Int(2)));
    }

    #[test]
    fn build_map_rejects_non_hashable_key() {
        let e = build_map(vec![(Value::Float(1.0), Value::Int(1))]).unwrap_err();
        assert!(e.contains("invalid map key"), "{e}");
    }

    #[test]
    fn map_index_found_and_missing() {
        let m = vec![
            (HKey::Str("x".into()), Value::Int(10)),
            (HKey::Int(2), Value::Str("two".into())),
        ];
        assert!(map_index(&m, &Value::Str("x".into()))
            .unwrap()
            .eq_val(&Value::Int(10)));
        assert!(map_index(&m, &Value::Int(2))
            .unwrap()
            .eq_val(&Value::Str("two".into())));
        match map_index(&m, &Value::Str("missing".into())) {
            Err(e) => assert_eq!(e, "map key not found"),
            Ok(_) => panic!("expected missing-key fault"),
        }
    }

    #[test]
    fn hkey_value_round_trip() {
        for v in [Value::Int(7), Value::Bool(true), Value::Str("k".into())] {
            assert!(HKey::from_value(&v).unwrap().to_value().eq_val(&v));
        }
        assert!(HKey::from_value(&Value::Float(1.0)).is_none());
    }

    #[test]
    fn map_eq_is_order_independent() {
        let a = Value::Map(Rc::new(vec![
            (HKey::Str("a".into()), Value::Int(1)),
            (HKey::Str("b".into()), Value::Int(2)),
        ]));
        let b = Value::Map(Rc::new(vec![
            (HKey::Str("b".into()), Value::Int(2)),
            (HKey::Str("a".into()), Value::Int(1)),
        ]));
        let c = Value::Map(Rc::new(vec![(HKey::Str("a".into()), Value::Int(1))]));
        assert!(a.eq_val(&b)); // same entries, different order → equal
        assert!(!a.eq_val(&c)); // different key set → not equal
    }

    #[test]
    fn int_kernels_fault_and_overflow() {
        assert_eq!(int_add(2, 3), Ok(5));
        assert_eq!(int_add(i64::MAX, 1), Err(FAULT_INT_OVERFLOW.to_string()));
        assert_eq!(int_sub(i64::MIN, 1), Err(FAULT_INT_OVERFLOW.to_string()));
        assert_eq!(int_mul(i64::MAX, 2), Err(FAULT_INT_OVERFLOW.to_string()));
        assert_eq!(int_div(7, 2), Ok(3));
        assert_eq!(int_div(1, 0), Err(FAULT_DIV_ZERO.to_string()));
        assert_eq!(int_div(i64::MIN, -1), Err(FAULT_INT_OVERFLOW.to_string()));
        assert_eq!(int_rem(7, 3), Ok(1));
        assert_eq!(int_rem(1, 0), Err(FAULT_MOD_ZERO.to_string()));
        assert_eq!(int_neg(5), Ok(-5));
        assert_eq!(int_neg(i64::MIN), Err(FAULT_INT_OVERFLOW.to_string()));
    }

    #[test]
    fn compare_ord_matches_both_backends() {
        assert_eq!(
            compare_ord(&Value::Int(1), &Value::Int(2)),
            Ok(Some(Ordering::Less))
        );
        assert_eq!(
            compare_ord(&Value::Float(2.0), &Value::Float(2.0)),
            Ok(Some(Ordering::Equal))
        );
        // NaN: comparable type, but no ordering -> Ok(None) (callers project to `false`).
        assert_eq!(
            compare_ord(&Value::Float(f64::NAN), &Value::Float(1.0)),
            Ok(None)
        );
        // Mixed/non-numeric operands are a comparability fault.
        assert!(compare_ord(&Value::Int(1), &Value::Float(1.0)).is_err());
        assert!(compare_ord(&Value::Bool(true), &Value::Bool(false)).is_err());
    }

    #[test]
    fn as_display_renders_primitives() {
        assert_eq!(Value::Int(42).as_display().as_deref(), Some("42"));
        assert_eq!(Value::Float(12.0).as_display().as_deref(), Some("12"));
        assert_eq!(
            Value::Float(12.56636).as_display().as_deref(),
            Some("12.56636")
        );
        assert_eq!(Value::Bool(true).as_display().as_deref(), Some("true"));
        assert_eq!(Value::Str("hi".into()).as_display().as_deref(), Some("hi"));
    }

    #[test]
    fn as_display_is_none_for_composite() {
        let inst = Value::Instance(Rc::new(Instance::new(
            "Greeter".into(),
            ClassLayout::new(vec![]),
        )));
        assert!(inst.as_display().is_none());
    }

    #[test]
    fn eq_val_terminates_on_a_reference_cycle() {
        // M-mut.6 / F4: build `a.next = b; b.next = a` (a 2-node instance cycle) and assert `eq_val`
        // returns instead of overflowing the native stack. Without the `visited` guard this test
        // aborts the process via stack overflow; with it, it terminates deterministically.
        let layout = ClassLayout::new(vec!["next".into()]);
        let a = Rc::new(Instance::new("Node".into(), layout.clone()));
        let b = Rc::new(Instance::new("Node".into(), layout));
        a.set_field("next", Value::Instance(b.clone()));
        b.set_field("next", Value::Instance(a.clone()));
        let va = Value::Instance(a);
        let vb = Value::Instance(b);
        // The two cyclic nodes are structurally bisimilar ⇒ equal; the call must terminate.
        assert!(va.eq_val(&vb));
        assert!(va.eq_val(&va.clone()));
    }

    #[test]
    fn eq_val_matches_by_value() {
        assert!(Value::Int(1).eq_val(&Value::Int(1)));
        assert!(!Value::Int(1).eq_val(&Value::Int(2)));
        assert!(!Value::Int(1).eq_val(&Value::Float(1.0))); // no cross-type eq
        assert!(Value::Null.eq_val(&Value::Null)); // null == null
        assert!(!Value::Null.eq_val(&Value::Int(0))); // null != a non-null value
        let a = Value::Enum(Rc::new(EnumVal {
            ty: "Shape".into(),
            variant: "Circle".into(),
            payload: crate::value::Payload::One(Value::Float(2.0)),
        }));
        let b = a.clone();
        assert!(a.eq_val(&b));
    }

    #[test]
    fn type_name_is_stable() {
        assert_eq!(Value::Unit.type_name(), "unit");
        assert_eq!(Value::List(Rc::new(vec![])).type_name(), "list");
        assert_eq!(Value::Set(Rc::new(vec![])).type_name(), "set");
    }

    #[test]
    fn build_set_dedups_first_seen() {
        // First occurrence kept, later duplicates dropped, order preserved (M-RT S7b).
        let s = build_set(vec![
            Value::Int(3),
            Value::Int(1),
            Value::Int(3),
            Value::Int(2),
            Value::Int(1),
        ])
        .unwrap();
        assert_eq!(s, vec![HKey::Int(3), HKey::Int(1), HKey::Int(2)]);
        // a non-hashable element faults cleanly, never panics (EV-7).
        assert!(build_set(vec![Value::Float(1.0)]).is_err());
    }

    #[test]
    fn eq_val_sets_are_order_independent() {
        let a = Value::Set(Rc::new(vec![HKey::Int(1), HKey::Int(2), HKey::Int(3)]));
        let b = Value::Set(Rc::new(vec![HKey::Int(3), HKey::Int(1), HKey::Int(2)]));
        let c = Value::Set(Rc::new(vec![HKey::Int(1), HKey::Int(2)]));
        assert!(a.eq_val(&b)); // same membership, different order
        assert!(!a.eq_val(&c)); // different cardinality
    }
}

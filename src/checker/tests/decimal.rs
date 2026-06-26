//! Checker tests — the `decimal` primitive (M-NUM S1).

use super::support::*;

#[test]
fn decimal_literal_and_arithmetic_type_as_decimal() {
    // A `decimal` literal types as `decimal`; `decimal + decimal` and `decimal * int` stay decimal.
    let src = "function main() -> void { \
               decimal price = 19.99d; \
               decimal sum = price + 1.00d; \
               decimal scaled = price * 3; \
               decimal scaled2 = 3 * price; \
               decimal neg = -price; \
               }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn decimal_plus_float_is_a_clean_error() {
    let e = errors_of(
        "function main() -> void { decimal d = 1.00d; float f = 1.5; decimal r = d + f; }",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-DECIMAL-FLOAT-MIX")),
        "got {e:?}"
    );
}

#[test]
fn float_is_not_assignable_to_a_decimal_slot() {
    // No implicit `float -> decimal` coercion (the whole point of the primitive).
    let e = errors_of("function main() -> void { decimal d = 1.5; }");
    assert!(
        !e.is_empty(),
        "float literal into a decimal slot must error"
    );
    // And the reverse: a decimal literal does not fit a float slot.
    let e2 = errors_of("function main() -> void { float f = 1.50d; }");
    assert!(
        !e2.is_empty(),
        "decimal literal into a float slot must error"
    );
}

#[test]
fn int_is_not_assignable_to_a_decimal_slot() {
    // The int-widening is operator-level only — a bare `int` does not fit a `decimal` slot.
    let e = errors_of("function main() -> void { decimal d = 3; }");
    assert!(!e.is_empty(), "int literal into a decimal slot must error");
}

#[test]
fn decimal_comparison_and_equality() {
    // `decimal <,>,==` with decimal or int operands type-check; a float mix is an error.
    let ok = "function main() -> void { \
              decimal a = 1.50d; decimal b = 1.5d; \
              bool eq = a == b; \
              bool gt = a > 1; \
              bool eqi = a == 2; \
              }";
    assert!(errors_of(ok).is_empty(), "got {:?}", errors_of(ok));
    let bad =
        errors_of("function main() -> void { decimal a = 1.50d; float f = 1.5; bool x = a < f; }");
    assert!(
        bad.iter().any(|d| d.code == Some("E-DECIMAL-FLOAT-MIX")),
        "got {bad:?}"
    );
}

#[test]
fn decimal_division_is_deferred() {
    // `/` and `%` on a decimal are not available this slice (S2).
    let e = errors_of("function main() -> void { decimal a = 1.00d; decimal r = a / 2; }");
    assert!(!e.is_empty(), "decimal `/` must error this slice");
}

#[test]
fn decimal_of_returns_optional_decimal() {
    let src = "package Main; import Core.Decimal; \
               function main() -> void { decimal d = Decimal.of(\"12.34\") ?? 0d; }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn decimal_interpolates_into_a_string() {
    let src = "function main() -> void { decimal d = 1.50d; }";
    assert!(errors_of(src).is_empty());
    let src2 =
        "import Core.Console; function main() -> void { decimal d = 1.50d; Console.println(\"d = {d}\"); }";
    // Console import lives at the package level — prepend it raw.
    let full = format!("package Main; {src2}");
    assert!(
        errors_of_raw(&full).is_empty(),
        "got {:?}",
        errors_of_raw(&full)
    );
}

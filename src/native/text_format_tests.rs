//! Tests for the `String.format` renderer (`text_format`) — oracle strings captured from the
//! transpile-floor `php-8.5.8`, independent of `PHORJ_REQUIRE_PHP`. Moved here with the renderer
//! (M-Decomp, Invariant 13).
use super::*;
use crate::value::Value;

#[test]
fn text_format_positional_args_strict() {
    // Slice 4b: `%N$` positional args (strict — reuse + reorder allowed; mixing and unused faulted).
    let fmt = |spec: &str, vals: Vec<Value>| -> Result<String, String> {
        let mut o = String::new();
        text_format(
            &[Value::Str(spec.into()), Value::List(std::rc::Rc::new(vals))],
            &mut o,
        )
        .map(|v| match v {
            Value::Str(s) => s.to_string(),
            other => panic!("{other:?}"),
        })
    };
    let s = |x: &str| Value::Str(x.into());
    // Reorder + reuse (the point of positional).
    assert_eq!(fmt("%2$s %1$s", vec![s("a"), s("b")]).unwrap(), "b a");
    assert_eq!(fmt("%1$s-%1$s", vec![s("x")]).unwrap(), "x-x");
    assert_eq!(
        fmt("%1$s costs %2$s", vec![s("pie"), s("3")]).unwrap(),
        "pie costs 3"
    );
    // Positional composes with flags/width/precision.
    assert_eq!(
        fmt("[%1$05d][%2$-6.3s]", vec![Value::Int(42), s("hello")]).unwrap(),
        "[00042][hel   ]"
    );
    // Strict faults: mixing positional + sequential, an unreferenced value, an out-of-range index,
    // and index 0 (the parser rejects `%0$`).
    assert!(fmt("%s %1$s", vec![s("a")]).is_err(), "mixing must fault");
    assert!(
        fmt("%1$s", vec![s("a"), s("b")]).is_err(),
        "unreferenced value must fault"
    );
    assert!(
        fmt("%3$s", vec![s("a"), s("b")]).is_err(),
        "out-of-range index must fault"
    );
    assert!(fmt("%0$s", vec![s("a")]).is_err(), "index 0 must fault");
}

#[test]
fn text_format_string_precision_truncates_at_char_boundary() {
    // Slice 4a: precision on `%s` truncates to ≤N BYTES but never splits a UTF-8 char (developer-ruled).
    let fmt = |spec: &str, s: &str| -> String {
        let mut o = String::new();
        match text_format(
            &[
                Value::Str(spec.into()),
                Value::List(std::rc::Rc::new(vec![Value::Str(s.into())])),
            ],
            &mut o,
        ) {
            Ok(Value::Str(r)) => r.to_string(),
            other => panic!("text_format({spec:?}) => {other:?}"),
        }
    };
    // ASCII: byte-identical to PHP `sprintf` (`%.3s` truncates, shorter strings pass through, `%.0s` empties).
    assert_eq!(fmt("%.3s", "abcdef"), "abc");
    assert_eq!(fmt("%.10s", "ab"), "ab");
    assert_eq!(fmt("%.0s", "abc"), "");
    // Width composes with precision: truncate first, then pad.
    assert_eq!(fmt("%6.3s", "abcdef"), "   abc");
    assert_eq!(fmt("%-6.3s", "abcdef"), "abc   ");
    // Multibyte: never split a char (the LADDER divergence — PHP byte-truncates to mojibake, Phorj keeps
    // whole chars; all three Phorj backends agree). "café" is c a f é(2 bytes) = 5 bytes.
    assert_eq!(fmt("%.4s", "café"), "caf"); // byte 4 is mid-é → back up, é dropped whole
    assert_eq!(fmt("%.5s", "café"), "café"); // full 5 bytes
    assert_eq!(fmt("%.2s", "éa"), "é"); // é is 2 bytes → exactly fits, "a" dropped
    assert_eq!(fmt("%.1s", "éa"), ""); // 1 byte would split é → nothing fits
}

#[test]
fn text_format_scientific_matches_php_byte_for_byte() {
    // Slice 3b: `%e`/`%E`. Every expected string here was captured from the transpile-floor oracle
    // `php-8.5.8` (`sprintf`) — this locks byte-identity for the tricky cases WITHOUT depending on
    // `PHORJ_REQUIRE_PHP` (the oracle differential only checks values that appear in an example).
    let fmt = |spec: &str, vals: Vec<Value>| -> String {
        let mut o = String::new();
        match text_format(
            &[Value::Str(spec.into()), Value::List(std::rc::Rc::new(vals))],
            &mut o,
        ) {
            Ok(Value::Str(s)) => s.to_string(),
            other => panic!("text_format({spec:?}) returned {other:?}"),
        }
    };
    let f = Value::Float;
    // Default precision 6, exponent ALWAYS signed with min-1 digit and NO leading zeros (PHP, not C).
    assert_eq!(fmt("%e", vec![f(1.5)]), "1.500000e+0");
    assert_eq!(fmt("%e", vec![f(-1.5)]), "-1.500000e+0");
    assert_eq!(fmt("%e", vec![f(1234.5)]), "1.234500e+3");
    assert_eq!(fmt("%e", vec![f(0.0001234)]), "1.234000e-4");
    assert_eq!(fmt("%e", vec![f(1e20)]), "1.000000e+20");
    assert_eq!(fmt("%e", vec![f(1e-20)]), "1.000000e-20");
    assert_eq!(fmt("%e", vec![f(123456789.0)]), "1.234568e+8"); // round-half-to-even
    assert_eq!(fmt("%e", vec![f(1e100)]), "1.000000e+100"); // 3-digit exponent, no leading zeros
                                                            // An int operand is accepted (coerced to float), like `%f`.
    assert_eq!(fmt("%e", vec![Value::Int(42)]), "4.200000e+1");
    // `-0.0` is NOT signed by `%e` (PHP quirk — value `< 0.0` is false; contrast `%g`, a later slice).
    assert_eq!(fmt("%e", vec![f(-0.0)]), "0.000000e+0");
    // `%E` upper-cases only the separator.
    assert_eq!(fmt("%E", vec![f(1.5)]), "1.500000E+0");
    assert_eq!(fmt("%E", vec![f(1e-20)]), "1.000000E-20");
    // Precision.
    assert_eq!(fmt("%.2e", vec![f(1234.5)]), "1.23e+3");
    assert_eq!(fmt("%.10e", vec![f(123456789.0)]), "1.2345678900e+8");
    // `%.0e` — no decimal point, round-half-to-even at the boundary (1.5→2, 2.5→2, 0.5→"5e-1").
    assert_eq!(fmt("%.0e", vec![f(1.5)]), "2e+0");
    assert_eq!(fmt("%.0e", vec![f(2.5)]), "2e+0");
    assert_eq!(fmt("%.0e", vec![f(0.5)]), "5e-1");
    // `+` flag forces a leading sign on non-negatives (and on `-0.0`); a real negative keeps `-`.
    assert_eq!(fmt("%+e", vec![f(1.5)]), "+1.500000e+0");
    assert_eq!(fmt("%+e", vec![f(-0.0)]), "+0.000000e+0");
    // Width: space-pad (right-justify), `-` left-justify, `0` zero-pad AFTER the sign.
    assert_eq!(fmt("%12.4e", vec![f(1.5)]), "   1.5000e+0");
    assert_eq!(fmt("%-12.4e", vec![f(1.5)]), "1.5000e+0   ");
    assert_eq!(fmt("%012.4e", vec![f(-1.5)]), "-001.5000e+0");
    assert_eq!(fmt("%012.4e", vec![f(1e20)]), "001.0000e+20");
    // A non-number faults (the phorj upgrade over PHP's silent coercion).
    let mut o = String::new();
    assert!(text_format(
        &[
            Value::Str("%e".into()),
            Value::List(std::rc::Rc::new(vec![Value::Str("x".into())]))
        ],
        &mut o
    )
    .is_err());
}

#[test]
fn text_format_f_sign_is_by_value_not_ieee_bit() {
    // `%f` signs iff the value is `< 0.0` (like PHP `sprintf`), NOT by the IEEE sign bit: `-0.0` is
    // unsigned, but a value that rounds to zero keeps its sign. Expected strings from php-8.5.8.
    let fmt = |spec: &str, vals: Vec<Value>| -> String {
        let mut o = String::new();
        match text_format(
            &[Value::Str(spec.into()), Value::List(std::rc::Rc::new(vals))],
            &mut o,
        ) {
            Ok(Value::Str(s)) => s.to_string(),
            other => panic!("text_format({spec:?}) returned {other:?}"),
        }
    };
    let f = Value::Float;
    assert_eq!(fmt("%f", vec![f(-0.0)]), "0.000000"); // -0.0 is NOT signed (the fixed bug)
    assert_eq!(fmt("%.2f", vec![f(-0.001)]), "-0.00"); // rounds to zero but stays signed
    assert_eq!(fmt("%+f", vec![f(0.0)]), "+0.000000"); // + flag on non-negative
    assert_eq!(fmt("%f", vec![f(-1.5)]), "-1.500000"); // ordinary negative
}

#[test]
fn text_format_shortest_repr_matches_php_byte_for_byte() {
    // Slice 3c: `%g`/`%G`. A curated subset of the exhaustive structured+random sweep (341k comparisons,
    // zero diffs vs php-8.5.8; branch boundaries, digit-gain roundings, half-to-even, subnormals,
    // precision .0–.17) baked as permanent oracle strings — locks the branch logic independent of php.
    let fmt = |spec: &str, v: f64| -> String {
        let mut o = String::new();
        match text_format(
            &[
                Value::Str(spec.into()),
                Value::List(std::rc::Rc::new(vec![Value::Float(v)])),
            ],
            &mut o,
        ) {
            Ok(Value::Str(s)) => s.to_string(),
            other => panic!("text_format({spec:?}) returned {other:?}"),
        }
    };
    // Default precision 6. FIXED style strips trailing zeros AND the dot fully.
    assert_eq!(fmt("%g", 0.0), "0");
    assert_eq!(fmt("%g", 1.0), "1");
    assert_eq!(fmt("%g", 100.0), "100");
    assert_eq!(fmt("%g", 120.0), "120");
    assert_eq!(fmt("%g", 0.5), "0.5");
    assert_eq!(fmt("%g", 1234.5), "1234.5");
    assert_eq!(fmt("%g", 12345.0), "12345");
    assert_eq!(fmt("%g", 123456.0), "123456");
    assert_eq!(fmt("%g", 0.0001234), "0.0001234");
    assert_eq!(fmt("%g", std::f64::consts::PI), "3.14159");
    // Branch boundary: X = -4 is the last FIXED, X = -5 flips to SCI; X = P(6) flips to SCI.
    assert_eq!(fmt("%g", 1e-4), "0.0001"); // X=-4 fixed
    assert_eq!(fmt("%g", 1e-5), "1.0e-5"); // X=-5 sci
    assert_eq!(fmt("%g", 1234567.0), "1.23457e+6"); // X=6 sci (rounds to 6 sig figs)
                                                    // SCI style keeps at least `.0` (PHP quirk vs C).
    assert_eq!(fmt("%g", 1e20), "1.0e+20");
    assert_eq!(fmt("%g", 2e20), "2.0e+20");
    assert_eq!(fmt("%g", 1.2e20), "1.2e+20");
    assert_eq!(fmt("%g", 1.5e-10), "1.5e-10");
    // `%g` signs by the IEEE sign bit — `-0.0` is SIGNED (contrast `%e`/`%f`).
    assert_eq!(fmt("%g", -0.0), "-0");
    assert_eq!(fmt("%+g", -0.0), "-0");
    assert_eq!(fmt("%+g", 0.0), "+0");
    assert_eq!(fmt("%g", -1.5), "-1.5");
    // Precision = SIGNIFICANT digits (default→6; 0 treated as 1).
    assert_eq!(fmt("%.0g", 123.456), "1.0e+2");
    assert_eq!(fmt("%.1g", 123.456), "1.0e+2");
    assert_eq!(fmt("%.2g", 123.456), "1.2e+2");
    assert_eq!(fmt("%.3g", 123.456), "123");
    assert_eq!(fmt("%.4g", 123.456), "123.5");
    assert_eq!(fmt("%.6g", 123.456), "123.456");
    // Digit-gain rounding (the double-round class the string-placement design survives).
    assert_eq!(fmt("%g", 9.999995), "10"); // rounds up into a new exponent, then fixed-strips
    assert_eq!(fmt("%g", 999999.5), "1.0e+6"); // rounds up past the fixed/sci boundary
                                               // `%G` upper-cases only the scientific separator.
    assert_eq!(fmt("%G", 1e20), "1.0E+20");
    assert_eq!(fmt("%G", 0.0001234), "0.0001234");
    // Width/flags compose in FIXED form (space-pad, zero-pad after sign, left-justify).
    assert_eq!(fmt("%10g", 1234.5), "    1234.5");
    assert_eq!(fmt("%-10g", 1234.5), "1234.5    ");
    assert_eq!(fmt("%010g", 1234.5), "00001234.5");
    // Width/flags compose in SCIENTIFIC form too — the pad wraps the whole `D.De±X` body (the sweep
    // covered value×precision but not width×%g, so pin the sci-form padding explicitly vs php-8.5.8).
    assert_eq!(fmt("%15g", 1e20), "        1.0e+20");
    assert_eq!(fmt("%015g", 1e20), "000000001.0e+20"); // zero-pad, no sign
    assert_eq!(fmt("%-15g", 1e20), "1.0e+20        ");
    assert_eq!(fmt("%+015g", 1e20), "+00000001.0e+20"); // zeros after the sign
                                                        // High precision (≥18, beyond the sweep's .0–.17 axis): both sides emit correctly-rounded digits
                                                        // of the exact f64 — byte-identical vs php-8.5.8 (0.1 and 1/3 to 20/25/30 significant digits).
    assert_eq!(fmt("%.20g", 0.1), "0.10000000000000000555");
    assert_eq!(fmt("%.25g", 0.1), "0.1000000000000000055511151");
    assert_eq!(fmt("%.30g", 1.0 / 3.0), "0.333333333333333314829616256247");
    // Int operand is accepted (coerced to float via the `Value::Int(n) => *n as f64` arm).
    assert_eq!(fmt("%g", 42.0), "42"); // float operand
    {
        let mut oi = String::new();
        let got = match text_format(
            &[
                Value::Str("%g".into()),
                Value::List(std::rc::Rc::new(vec![Value::Int(42)])),
            ],
            &mut oi,
        ) {
            Ok(Value::Str(s)) => s.to_string(),
            other => panic!("text_format int operand => {other:?}"),
        };
        assert_eq!(got, "42"); // int operand exercises the `as f64` cast
    }
    // A non-number faults.
    let mut o = String::new();
    assert!(text_format(
        &[
            Value::Str("%g".into()),
            Value::List(std::rc::Rc::new(vec![Value::Str("x".into())]))
        ],
        &mut o
    )
    .is_err());
}

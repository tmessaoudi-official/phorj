//! DEC-558 — PHP's legacy octal literal `0755` (= 493). The lifter used to parse it as decimal 755, so a
//! lifted file silently changed value; phorj itself now refuses a leading-zero integer (`E-LEADING-ZERO`),
//! so the lifter must hand it over as the VALUE PHP means.

use super::lifter::lift_source;

fn lifted(src: &str) -> String {
    lift_source(src).unwrap_or_else(|e| panic!("expected a lift, refused: {e}\n{src}"))
}

fn checks_clean(out: &str) {
    let prog = crate::cli::parse_program(out).unwrap_or_else(|e| panic!("parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out).unwrap_or_else(|e| panic!("checks: {e:?}\n{out}"));
}

#[test]
fn a_legacy_octal_literal_lifts_to_its_value() {
    let out = lifted("<?php\nfunction mode(): int { return 0755; }\n");
    assert!(
        out.contains("return 493;"),
        "0755 is octal 493 in PHP:\n{out}"
    );
    assert!(!out.contains("755"), "{out}");
    checks_clean(&out);
}

#[test]
fn zero_and_a_float_with_a_leading_zero_are_unchanged() {
    let out =
        lifted("<?php\nfunction z(): int { return 0; }\nfunction f(): float { return 007.5; }\n");
    assert!(
        out.contains("return 0;") && out.contains("return 7.5;"),
        "{out}"
    );
}

#[test]
fn an_invalid_octal_digit_is_refused_like_php() {
    let e =
        lift_source("<?php\nfunction bad(): int { return 089; }\n").expect_err("089 is invalid");
    assert!(e.contains("089"), "{e}");
}

//! Row 5g — DEC-523 as built by DEC-529: PHP `/` and `/=` lift to FLOAT division.
//!
//! PHP's `/` on two ints yields a float unless the division is exact (`7 / 2` is `3.5`), while
//! phorj's `/` on two ints is integer division (`7 / 2` is `3`). Mapping the operator 1:1 therefore
//! lifted a draft that checked clean and printed a different number. The lifter has no variable
//! types, so it cannot tell `int / int` from `float / int`; it makes every operand float instead:
//! an int literal becomes a float literal, a provably-float operand passes through, and anything else
//! is wrapped in `as float`. A float VARIABLE thus gets a redundant cast — `W-REDUNDANT-CAST`, a
//! warning, never a failed check — which DEC-529 accepts as the price of a lift that is never wrong.
//!
//! Every "lifts correctly" case below ends in `checks_clean`: asserting on the emitted string alone
//! is how two earlier slices shipped drafts that failed the check they were meant to pass.

use super::lifter::lift_source;
use super::lifter_tests::{assert_reparses, lift};

fn checks_clean(phg: &str) {
    let prog =
        crate::cli::parse_program(phg).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{phg}"));
    crate::cli::check_and_expand(&prog, phg)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{phg}"));
}

/// scout's own line — `Rent/Core/Classification.php`, the blocker this row exists for.
#[test]
fn an_int_field_over_an_int_literal_is_float_division() {
    let out = lift(
        "<?php class C { public function __construct(private int $confidenceBp) {}\n\
         public function confidence(): float { return $this->confidenceBp / 100; } }",
    );
    assert!(
        out.contains("(this.confidenceBp as float) / 100.0"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn two_int_variables_are_both_cast() {
    let out = lift("<?php function f(int $a, int $b): float { return $a / $b; }");
    assert!(out.contains("(a as float) / (b as float)"), "{out}");
    checks_clean(&out);
}

/// A negated int literal is still an int, and `7.0 / -2` is refused by `phg check` — so the sign
/// must travel with a FLOAT literal, not be left behind.
#[test]
fn a_negated_int_literal_becomes_a_negated_float_literal() {
    let out = lift("<?php function f(int $a): float { return $a / -2; }");
    assert!(out.contains("(a as float) / -2.0"), "{out}");
    checks_clean(&out);
}

/// Operands that are already float are left alone: a float literal, a negated one, a PHP `(float)`
/// cast, and the result of another `/`.
#[test]
fn provably_float_operands_are_not_cast_again() {
    let out = lift(
        "<?php function f(int $a, int $b, int $c): float { return ((float) $a / 2.5) / ($b / $c) / -0.5; }",
    );
    assert!(!out.contains("as float) as float"), "{out}");
    assert!(out.contains("a as float"), "{out}");
    assert!(out.contains("2.5"), "{out}");
    assert!(!out.contains("(2.5 as float)"), "{out}");
    assert!(!out.contains("-0.5 as float"), "{out}");
    checks_clean(&out);
}

/// `$x /= e` is `$x = $x / e`, built on a separate path from the binary operator — pinned on its own
/// so reverting either site reds a test.
#[test]
fn compound_divide_assign_is_float_division() {
    let out = lift("<?php function f(int $n): float { $x = 1.5; $x /= $n; return $x; }");
    assert!(out.contains("x = (x as float) / (n as float);"), "{out}");
    checks_clean(&out);
}

/// DEC-529's accepted cost: a float variable gets a redundant cast, which is a lint WARNING — the
/// draft still checks. This case is the proof that "warning" is true, not assumed.
#[test]
fn a_redundant_cast_on_a_float_variable_still_checks() {
    let out = lift("<?php function f(float $a, float $b): float { return $a / $b; }");
    assert!(out.contains("(a as float) / (b as float)"), "{out}");
    checks_clean(&out);
}

/// DEC-523's one miss, loud by design: PHP's exact `6 / 3` is int 2, but the lift is float, so a
/// division landing in an `int` slot fails `phg check` — never a wrong number at run time.
#[test]
fn a_division_into_an_int_slot_fails_the_check_loudly() {
    let out = lift("<?php function f(int $a): int { return $a / 3; }");
    assert_reparses(&out);
    let prog = crate::cli::parse_program(&out).expect("draft parses");
    let err = crate::cli::check_and_expand(&prog, &out)
        .expect_err("a float division into an int return must not check");
    let msg = format!("{err:?}");
    assert!(msg.contains("int") && msg.contains("float"), "{msg}");
}

/// Only `/` changes: `%`, `*`, `+`, `-` keep their int meaning.
#[test]
fn other_arithmetic_is_untouched() {
    let out = lift("<?php function f(int $a, int $b): int { return $a * $b + $a % $b - 1; }");
    assert!(!out.contains("as float"), "{out}");
    assert!(
        !lift_source("<?php function f(int $a): int { return $a % 2; }")
            .expect("lift")
            .contains("2.0")
    );
    checks_clean(&out);
}

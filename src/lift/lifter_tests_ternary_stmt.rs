//! twes-in row T1 (2026-09-27) — a PHP ternary used as a STATEMENT, `$c ? $a->m() : $b->m();`. Its
//! value is discarded, so it is exactly an `if` statement. Lifted as an `if`-EXPRESSION it read
//! `if (c) { a.m() } else { b.m() };`, which phorj parses as an `if` STATEMENT whose arms lack `;` —
//! a draft that did not parse (`ManagePayments.php` in the twes-in census).

use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

#[test]
fn a_statement_ternary_lifts_to_an_if_statement() {
    let out = lift(
        "<?php
function g(int $n): void { echo $n; }
function f(bool $c): void { $c ? g(1) : g(2); }",
    );
    checks_clean(&out);
    assert!(out.contains("g(1);"), "{out}");
    assert!(out.contains("g(2);"), "{out}");
}

/// A nested ternary in an arm nests the `if`; each arm is a statement in its own right.
#[test]
fn a_nested_statement_ternary_nests_the_if() {
    let out = lift(
        "<?php
function g(int $n): void { echo $n; }
function f(bool $a, bool $b): void { $a ? g(1) : ($b ? g(2) : g(3)); }",
    );
    checks_clean(&out);
    for call in ["g(1);", "g(2);", "g(3);"] {
        assert!(out.contains(call), "{call}\n{out}");
    }
}

/// A ternary whose VALUE is used is untouched: it stays an `if`-expression.
#[test]
fn a_ternary_in_value_position_stays_an_expression() {
    let out = lift("<?php\nfunction f(bool $c): int { $x = $c ? 1 : 2; return $x; }");
    checks_clean(&out);
    assert!(out.contains("= if (c) { 1 } else { 2 };"), "{out}");
}

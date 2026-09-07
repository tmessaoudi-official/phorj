//! Lane L1b of the scout forcing function (2026-09-07) — **block-bodied closures**
//! `function (…) [use (…)] : T { … }`, and DEC-506's named refusal for by-reference capture.
//!
//! 23 of scout's 120 files write a block closure; it was refused with a flat "Tier-2" message that
//! did not distinguish a construct phorj HAS (a lambda with a statement body,
//! `function(int x): int { … }`) from one it has deliberately REJECTED (`use (&$x)`,
//! UNIFIED-SPEC:1619 — it contradicts the value/handle split). Two very different answers behind
//! one message.

use super::lifter_tests::{assert_reparses, lift};

/// The plain case: a statement body with a declared return type is a phorj lambda, verbatim.
#[test]
fn a_block_closure_lifts_to_a_statement_bodied_lambda() {
    let out = lift(
        "<?php\nfunction pick(): int {\n    $f = function (int $x): int { if ($x > 1) { return $x * 2; } return $x; };\n    return $f(3);\n}",
    );
    assert!(out.contains("function(int x): int {"), "{out}");
    assert!(out.contains("return x * 2;"), "{out}");
    assert_reparses(&out);
}

/// `static function (…)` is the same closure with a modifier PHP needs and phorj does not — a phorj
/// lambda never binds `$this`, so `static` carries no information and is dropped.
#[test]
fn a_static_block_closure_drops_the_modifier() {
    let out = lift(
        "<?php\nfunction pick(): int {\n    $f = static function (int $x): int { return $x + 1; };\n    return $f(1);\n}",
    );
    assert!(out.contains("function(int x): int {"), "{out}");
    assert_reparses(&out);
}

/// A BY-VALUE `use ($x)` list is what a phorj lambda does anyway — it captures enclosing locals by
/// value — so the list is dropped and the names stay in scope. Nothing is lost and nothing is
/// invented.
#[test]
fn a_by_value_use_list_is_dropped_because_phorj_captures_by_value() {
    let out = lift(
        "<?php\nfunction pick(int $n): int {\n    $f = function (int $x) use ($n): int { return $x + $n; };\n    return $f(1);\n}",
    );
    assert!(out.contains("function(int x): int {"), "{out}");
    assert!(!out.contains("use ("), "the use list survived:\n{out}");
    assert!(out.contains("x + n"), "{out}");
    assert_reparses(&out);
}

/// DEC-506 — by-REFERENCE capture is a ruled REJECTION, not a gap, and the message must say so
/// rather than reporting a parse error that reads like a lifter bug. Five sites in scout, one of
/// them inside `TenureClassifier.php` itself.
#[test]
fn by_reference_capture_is_refused_by_name_not_as_a_parse_error() {
    let err = super::lifter::lift_source(
        "<?php\nfunction pick(): int {\n    $acc = 0;\n    $f = function (int $x) use (&$acc): int { return $x; };\n    return $f(1);\n}",
    )
    .expect_err("by-ref capture");
    assert!(
        err.contains("by reference") || err.contains("by-reference"),
        "the refusal does not name the construct: {err}"
    );
    assert!(
        err.contains("acc"),
        "the refusal does not name the captured variable: {err}"
    );
    assert!(
        !err.contains("expected `)`"),
        "still a raw parse error: {err}"
    );
}

/// A block closure with NO declared return type is refused: phorj requires `: T` on a
/// statement-bodied lambda, and inferring one would be the lifter guessing (DEC-166).
#[test]
fn a_block_closure_without_a_return_type_is_refused() {
    let err = super::lifter::lift_source(
        "<?php\nfunction pick(): int {\n    $f = function (int $x) { return $x; };\n    return $f(1);\n}",
    )
    .expect_err("no return type");
    assert!(err.contains("return type"), "{err}");
}

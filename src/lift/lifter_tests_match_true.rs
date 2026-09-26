//! Row 4d — `match (true) { cond => v, … default => d }` is PHP's guard chain: the first condition
//! that is `=== true`, in source order, selects its arm. It lifts to an if-/else-if chain instead of
//! the non-literal-arm refusal (scout `LandlordRegistry::stricterOf`). A non-`true` subject keeps the
//! refusal — pinned in `lifter_tests.rs`'s refusal table.

use super::lifter::lift_source;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

#[test]
fn a_match_true_guard_chain_lifts_to_an_if_chain() {
    let out = lift_source(
        "<?php function rank(?int $t): int { return match (true) { $t === null => 1, $t > 5 => 2, default => 0 }; }",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(out.contains("if (") && !out.contains("match"), "{out}");
    checks_clean(&out);
}

#[test]
fn comma_conditions_become_a_short_circuit_or() {
    let out = lift_source(
        "<?php function f(int $n): string { return match (true) { $n < 0, $n > 9 => 'out', default => 'in' }; }",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(out.contains("||"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_default_written_first_is_still_the_fallback() {
    let out = lift_source(
        "<?php function f(int $n): string { return match (true) { default => 'd', $n === 1 => 'one' }; }",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    checks_clean(&out);
    // The guarded arm is tested BEFORE the fallback, wherever `default` was written.
    let one = out.find("\"one\"").expect("guarded arm");
    let d = out.find("\"d\"").expect("default arm");
    assert!(one < d, "{out}");
}

#[test]
fn a_match_true_without_default_is_refused_by_name() {
    let e = refused("<?php function f(int $n): int { return match (true) { $n > 0 => 1 }; }");
    assert!(e.contains("match (true)") && e.contains("default"), "{e}");
}

#[test]
fn a_match_false_subject_keeps_the_non_literal_refusal() {
    // Only a literal `true` subject is PHP's guard chain. Lifting `match (false)` the same way would
    // invert every arm silently (`$n > 0 => 1` would fire exactly when PHP's does not).
    let e = refused(
        "<?php function f(int $n): int { return match (false) { $n > 0 => 1, default => 0 }; }",
    );
    assert!(e.contains("non-literal condition"), "{e}");
}

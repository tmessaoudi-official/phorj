//! Tests: transpile / lift command surface and compile-time-sugar erasure.
use super::super::*;
use super::{wp, SAMPLE};

#[test]
fn cmd_transpile_emits_php_for_sample() {
    let php = cmd_transpile(SAMPLE).expect("transpile");
    assert!(php.starts_with("<?php\n"), "{php}");
    assert!(php.contains("abstract class Shape {}"), "{php}");
    assert!(
        php.contains("function __construct(private string $name) {}"),
        "{php}"
    );
}

#[test]
fn cmd_transpile_rejects_ill_typed() {
    let err = cmd_transpile(&wp(r#"function main(): void { int x = "no"; }"#)).unwrap_err();
    assert!(err.contains("type error"), "{err}");
}

#[test]
fn cmd_lift_emits_annotated_phorj_draft() {
    let phg =
        cmd_lift("<?php function add(int $a, int $b): int { return $a + $b; }").expect("lift");
    // The banner makes the review-required contract visible in the file.
    assert!(phg.starts_with("// lifted (verify)"), "{phg}");
    assert!(phg.contains("package Main;"), "{phg}");
    assert!(phg.contains("function add(int a, int b): int {"), "{phg}");
}

#[test]
fn cmd_lift_refuses_outside_tier1_loudly() {
    // An `array` type has no faithful Phorj form yet — a clear lift error, not a guess.
    let err = cmd_lift("<?php function f(array $xs): void {}").unwrap_err();
    assert!(err.contains("`array` type"), "{err}");
}

#[test]
fn var_transpiles_to_plain_php_assignment() {
    // `var` is erased; PHP locals are untyped, so it emits a bare `$x = …;`.
    let php = cmd_transpile(&wp(
        "import Core.Output; function main(): void { var x = 1; Output.printLine(\"{x}\"); }",
    ))
    .unwrap();
    assert!(php.contains("$x = 1;"), "{php}");
}

#[test]
fn type_alias_is_erased_in_php() {
    // The alias declaration vanishes and `Count` resolves to `int` in the emitted signature.
    let php = cmd_transpile(&wp(
        "type Count = int; function tally(Count n): Count { return n + 1; } function main(): void {}",
    ))
    .unwrap();
    assert!(!php.contains("Count"), "alias leaked into PHP:\n{php}");
    assert!(php.contains("function tally(int $n): int"), "{php}");
}

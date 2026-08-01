//! Tests for the lifter's exception handling — the DEC-421 taxonomy mapping AND the `try`/`catch`/
//! `throw` lifting it sits on top of. Split out of `exceptions.rs` (and, for the five shape tests
//! below, moved here from `lifter_tests.rs`) so one file holds the whole error-path story: the shape
//! tests and the mapping tests read the same lifted drafts and would otherwise drift apart.
//!
//! The five moved tests changed with DEC-421 rather than merely moving: they used to assert the
//! UNMAPPED PHP class names (`catch (DivisionByZeroError e)`) and only that the draft *re-parsed*,
//! because that was all a draft could do before the taxonomy existed. They now assert the mapped
//! phorj names and, where the draft has no `throw`, that it TYPE-CHECKS.

use super::{mapped_error_types, unmapped_exception_classes};
use crate::lift::lifter::lift_source;

fn parse(src: &str) -> crate::lift::ast::PhpProgram {
    let (toks, docs) = crate::lift::lexer::lex_php_with_docs(src).expect("lex");
    crate::lift::parser::parse_php_with_docs(toks, docs).expect("parse")
}

/// `phg check` on a phorj source, through the SAME pipeline the CLI uses — prelude injection
/// included, which is the whole point here: the six error types only exist once `Core.ErrorModule`
/// has been injected.
fn check(phg: &str) -> Result<(), String> {
    let prog = crate::cli::parse_program(phg).map_err(|e| format!("parse: {e}"))?;
    crate::cli::check_and_expand(&prog, phg).map(|_| ())
}

// ── the mapping (DEC-421) ──

/// The bug the single-visitor refactor exists to prevent: the `throw new X` arm once lived in the
/// UNMAPPED walk only, so a program that threw a mapped builtin got both the right type AND a note
/// saying the type could not be lifted. One walk, both answers — asserted together on purpose.
#[test]
fn a_thrown_builtin_is_mapped_and_is_not_also_reported_unmappable() {
    let p = parse(
        "<?php\nfunction f(): void {\n  try { throw new \\RuntimeException(\"a\"); }\n  \
         catch (\\RuntimeException $e) { throw new \\LogicException(\"b\"); }\n}\n",
    );
    assert_eq!(mapped_error_types(&p), vec!["LogicError", "RuntimeError"]);
    assert!(
        unmapped_exception_classes(&p).is_empty(),
        "a mapped builtin must not also be reported unmappable: {:?}",
        unmapped_exception_classes(&p)
    );
}

/// A framework exception is left visibly for the human — in BOTH positions.
#[test]
fn an_unmapped_class_is_reported_from_a_catch_and_from_a_throw() {
    let caught = parse(
        "<?php\nfunction f(): void {\n  try { echo 1; } catch (\\Acme\\PaymentFailed $e) { echo 2; }\n}\n",
    );
    assert_eq!(
        unmapped_exception_classes(&caught),
        vec!["Acme\\PaymentFailed"]
    );
    let thrown = parse("<?php\nfunction f(): void {\n  throw new MyAppError(\"x\");\n}\n");
    assert_eq!(unmapped_exception_classes(&thrown), vec!["MyAppError"]);
    assert!(mapped_error_types(&thrown).is_empty());
}

/// Nesting is the case a shallow scan gets wrong: a catch inside a loop inside an `if`, and a method
/// body rather than a free function.
#[test]
fn a_deeply_nested_site_inside_a_method_is_still_seen() {
    let p = parse(
        "<?php\nclass C {\n  public function m(int $n): void {\n    if ($n > 0) {\n      \
         while ($n > 0) {\n        try { $n = $n - 1; } catch (\\DivisionByZeroError $e) { echo 1; }\n      }\n    }\n  }\n}\n",
    );
    assert_eq!(mapped_error_types(&p), vec!["MathError"]);
}

/// THE assertion DEC-421 was ruled for. Before it, this draft lifted and re-parsed and then died on
/// `unknown type RuntimeException`, because phorj had an `Error` marker and user-declared errors and
/// nothing in between. It must now type-check with no hand edits at all.
#[test]
fn a_lifted_catch_of_a_php_builtin_type_checks_with_no_hand_edits() {
    let phg = lift_source(
        "<?php\nfunction f(int $n): int {\n  try { return $n; } \
         catch (\\RuntimeException $e) { return 0; }\n}\nfunction main(): void { echo \"ok\"; }\n",
    )
    .expect("lift");
    assert!(
        phg.contains("catch (RuntimeError e)"),
        "the builtin was not mapped: {phg}"
    );
    // The member import is what makes the mapped name resolve — without it the draft would die on
    // `E-INJECTED-TYPE-BARE` instead, which is a different error but just as unusable.
    assert!(
        phg.contains("import Core.ErrorModule.RuntimeError;"),
        "the mapped type was emitted without its member import: {phg}"
    );
    assert!(
        !phg.contains("CANNOT LIFT"),
        "a mapped builtin must not carry a cannot-lift note: {phg}"
    );
    check(&phg).expect("a lifted catch of a PHP builtin must type-check");
}

/// Only what is USED. Importing all six would be `E-UNUSED-IMPORT` — a lift that fails the very check
/// it exists to pass.
#[test]
fn only_the_error_types_the_draft_names_are_imported() {
    let phg = lift_source(
        "<?php\nfunction f(int $n): int {\n  try { return $n; } \
         catch (\\TypeError $e) { return 0; }\n}\nfunction main(): void { echo \"ok\"; }\n",
    )
    .expect("lift");
    assert!(
        phg.contains("import Core.ErrorModule.TypeMismatchError;"),
        "{phg}"
    );
    for unused in ["RuntimeError", "LogicError", "MathError", "IoError"] {
        assert!(
            !phg.contains(&format!("import Core.ErrorModule.{unused};")),
            "unused `{unused}` was imported: {phg}"
        );
    }
    check(&phg).expect("must type-check");
}

/// An unmapped class is refused LOUDLY — the note names it, and the draft keeps the original name
/// rather than being coerced into the nearest phorj type.
#[test]
fn an_unmapped_class_produces_a_cannot_lift_note_naming_it() {
    let phg = lift_source(
        "<?php\nfunction f(int $n): int {\n  try { return $n; } \
         catch (\\Acme\\PaymentFailed $e) { return 0; }\n}\nfunction main(): void { echo \"ok\"; }\n",
    )
    .expect("lift");
    assert!(
        phg.contains("// CANNOT LIFT: `Acme\\PaymentFailed`"),
        "no note for the unmapped class: {phg}"
    );
    assert!(
        !phg.contains("import Core.ErrorModule"),
        "nothing mapped, so nothing to import: {phg}"
    );
}

// ── the shapes the mapping rides on (LIFT-TRY, moved here from `lifter_tests.rs`) ──

/// LIFT-TRY — a PHP `try`/`catch`/`finally` lifts to the phorj equivalent. The draft now TYPE-CHECKS,
/// not merely re-parses: re-parsing was all that could be asserted before DEC-421 gave
/// `DivisionByZeroError` a phorj counterpart, and it is the weaker claim — a plausible string that
/// parses can still name a type that does not exist.
#[test]
fn try_catch_finally_lifts_and_the_draft_type_checks() {
    let phg = lift_source(
        "<?php\nfunction risky(int $n): int {\n  try {\n    return 100 / $n;\n  } \
         catch (\\DivisionByZeroError $e) {\n    return 0;\n  } finally {\n    echo \"cleanup\\n\";\n  }\n}\n\
         function main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    for want in ["try {", "catch (MathError e) {", "finally {"] {
        assert!(phg.contains(want), "missing {want:?} in:\n{phg}");
    }
    // The root-namespace marker is gone — phorj has no `\Type` spelling, and the mapped name has no
    // namespace to strip in the first place. (Checked on the class name specifically: the draft
    // legitimately contains a `\` inside the `"cleanup\n"` literal.)
    assert!(
        !phg.contains("DivisionByZeroError"),
        "the unmapped PHP name leaked:\n{phg}"
    );
    check(&phg).expect("the lifted draft must type-check");
}

/// A UNION catch keeps both members through the whole pipeline (parser → lifter → printer), each
/// mapped independently. Narrowing to the first type would silently change which exceptions the
/// clause catches.
#[test]
fn a_union_catch_survives_the_lift_with_both_members_mapped() {
    let phg = lift_source(
        "<?php\nfunction f(int $n): int {\n  try {\n    return 10 / $n;\n  } \
         catch (\\DivisionByZeroError | \\RuntimeException $e) {\n    return -1;\n  }\n}\n\
         function main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    assert!(
        phg.contains("catch (MathError | RuntimeError e)"),
        "the union was not preserved or not mapped:\n{phg}"
    );
    // BOTH members need their member import, not just the first.
    for want in ["MathError", "RuntimeError"] {
        assert!(
            phg.contains(&format!("import Core.ErrorModule.{want};")),
            "union member `{want}` was emitted without its import:\n{phg}"
        );
    }
    check(&phg).expect("the lifted draft must type-check");
}

/// PHP 8's variable-less `catch (T)` has no phorj spelling — phorj's `CatchClause` always binds. The
/// lift SYNTHESISES a name rather than dropping the clause.
#[test]
fn a_variableless_catch_gets_a_synthesised_binding() {
    let phg = lift_source(
        "<?php\nfunction g(int $n): int {\n  try {\n    return 10 / $n;\n  } \
         catch (\\DivisionByZeroError) {\n    return -2;\n  }\n}\n\
         function main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    assert!(
        phg.contains("catch (MathError ignored)"),
        "no synthesised binding:\n{phg}"
    );
    check(&phg).expect("the lifted draft must type-check");
}

/// `throw` lifts, and the root-namespace marker is stripped so the draft PARSES.
///
/// The strip is still load-bearing, but only for an UNMAPPED class now — a mapped one is replaced
/// wholesale, so it cannot carry a `\`. The bug it guards: `catch` stripped the marker while `new` did
/// not, so a lifted `throw new \Acme\PaymentFailed(…)` emitted a leading `\` that is not valid phorj —
/// an unparseable draft beside a correctly-lifted catch in the same function.
#[test]
fn throw_lifts_and_the_root_namespace_marker_is_stripped() {
    let mapped = lift_source(
        "<?php\nfunction guard(int $n): int {\n  if ($n < 0) { throw new \\RuntimeException(\"negative\"); }\n  \
         return $n;\n}\nfunction main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    assert!(
        mapped.contains("throw new RuntimeError(\"negative\")"),
        "throw did not lift and map cleanly:\n{mapped}"
    );
    // PARSE, not type-check: a lifted `throw` still needs its `throws` clause by hand
    // (`KNOWN_ISSUES.md` §LIFT-THROWS), which is a separate, recorded boundary.
    crate::cli::parse_program(&mapped).expect("the lifted draft must re-parse");

    let unmapped = lift_source(
        "<?php\nfunction guard(int $n): int {\n  if ($n < 0) { throw new \\Acme\\PaymentFailed(\"no\"); }\n  \
         return $n;\n}\nfunction main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    assert!(
        unmapped.contains("throw new Acme\\PaymentFailed(\"no\")"),
        "the ROOT marker only should be stripped, leaving the namespace:\n{unmapped}"
    );
    assert!(
        !unmapped.contains("new \\Acme"),
        "the root `\\` leaked:\n{unmapped}"
    );
}

/// A rethrow inside a `catch` — the shape that made `throw` worth having, since LIFT-TRY without it
/// meant any realistic PHP error path was unliftable. Both types map, in their two different
/// positions.
#[test]
fn a_rethrow_inside_a_catch_lifts_with_both_types_mapped() {
    let phg = lift_source(
        "<?php\nfunction f(int $n): int {\n  try {\n    return 10 / $n;\n  } \
         catch (\\DivisionByZeroError $e) {\n    throw new \\LogicException(\"wrapped\");\n  }\n}\n\
         function main(): void { echo \"ok\\n\"; }\n",
    )
    .expect("the fixture lifts");
    assert!(phg.contains("catch (MathError e)"), "{phg}");
    assert!(phg.contains("throw new LogicError(\"wrapped\")"), "{phg}");
    crate::cli::parse_program(&phg).expect("the lifted draft must re-parse");
}

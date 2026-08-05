//! LIFT-NS parser tests — the `namespace` / `use` shapes that have NO phorj analog and are
//! therefore refused with a reason rather than half-lifted (DEC-166 — never guess).
//!
//! Split into its own file because `parser_tests.rs` sits at its grandfathered Invariant-13
//! ceiling of 559 lines (`scripts/size-baseline.txt`), so growing it would fail the size gate.

use super::parser_tests::perr;

/// LIFT-NS: the `namespace` / `use` shapes that have no phorj analog are refused with a reason, not
/// silently half-lifted (DEC-166 — never guess). Each message must name WHY, so a draft that stops is
/// actionable.
#[test]
fn refuses_namespace_and_use_forms_with_no_phorj_analog() {
    for (src, frag) in [
        // Braced namespaces can declare several per file; phorj has one `package` per file.
        ("<?php namespace App { }", "one `package` per file"),
        // Two semicolon namespaces in one file: same reason.
        (
            "<?php namespace A; namespace B;",
            "a second `namespace` declaration",
        ),
        // A namespace after a declaration is not PHP-legal and must not be quietly accepted.
        (
            "<?php function f(): int { return 1; } namespace A;",
            "must precede every declaration",
        ),
        // `use function` / `use const` import a symbol, not a type.
        ("<?php use function App\\helper;", "imports a symbol"),
        ("<?php use const App\\LIMIT;", "imports a symbol"),
        // The group form needs one import per member — a separate increment.
        ("<?php use App\\{A, B};", "grouped"),
    ] {
        let e = perr(src);
        assert!(e.contains(frag), "for {src:?} got {e}");
    }
}

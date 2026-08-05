//! LIFT-NS parser tests — the `namespace` / `use` shapes that have NO phorj analog and are
//! therefore refused with a reason rather than half-lifted (DEC-166 — never guess).
//!
//! Split into its own file because `parser_tests.rs` sits at its grandfathered Invariant-13
//! ceiling of 559 lines (`scripts/size-baseline.txt`), so growing it would fail the size gate.

use super::parser_tests::{parse, perr};

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
            "must come before every `use`",
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

/// The refusal messages must be complete sentences, NOT `err`'s "expected X, found Tok" shape — these
/// are the strings users read, and a full sentence followed by ", found LBrace" is broken English.
#[test]
fn refusal_messages_do_not_trail_a_found_clause() {
    for src in [
        "<?php namespace App { }",
        "<?php namespace A; namespace B;",
        "<?php use function App\\helper;",
        "<?php use App\\{A, B};",
        "<?php use App\\A, App\\B;",
    ] {
        let e = perr(src);
        assert!(
            !e.contains(", found "),
            "reason-first refusal must not trail a `found` clause: {e}"
        );
    }
}

/// `use` BEFORE `namespace` is a PHP FATAL ("Namespace declaration statement has to be the very first
/// statement"), so accepting it would invent a meaning for input PHP cannot run.
#[test]
fn a_use_before_the_namespace_is_refused() {
    let e = perr("<?php use App\\A; namespace App;");
    assert!(e.contains("must come before every `use`"), "{e}");
}

/// The comma form is legal PHP and needs its own reason rather than a bare "expected `;`".
#[test]
fn the_comma_form_use_is_refused_with_a_reason() {
    let e = perr("<?php use App\\A, App\\B;");
    assert!(e.contains("comma-separated"), "{e}");
}

/// DEC-401 symmetry: the transpiler emits `declare(strict_types=1);` in every file, so the lifter must
/// read its own output back (Invariant 17). `strict_types=1` states what is permanently true of phorj,
/// so it is consumed and discarded; anything else carries meaning phorj cannot express and is refused.
#[test]
fn declare_strict_types_is_accepted_and_other_directives_are_refused() {
    // The PSR-12 prologue in full: `declare` then `namespace` then `use`.
    let p = parse(
        "<?php\ndeclare(strict_types=1);\n\nnamespace App\\Svc;\nfunction f(): int { return 1; }\n",
    );
    assert_eq!(p.namespace, vec!["App".to_string(), "Svc".to_string()]);
    assert_eq!(
        p.items.len(),
        1,
        "declare must not become an item: {:?}",
        p.items
    );

    for (src, frag) in [
        ("<?php declare(ticks=1);", "has no phorj equivalent"),
        ("<?php declare(strict_types=0);", "COERCIVE mode"),
    ] {
        let e = perr(src);
        assert!(e.contains(frag), "for {src:?} got {e}");
    }
}

/// A transpiled file must lift BACK — the round trip DEC-401 makes load-bearing. Uses the real emitter
/// output rather than a hand-written prologue, so the two cannot drift.
///
/// Deliberately a program that emits NO runtime helpers. A transpiled file containing one does not lift
/// back, because the helpers are emitted with UNTYPED parameters (`function __phorj_checked_add($a, $b)`)
/// and the lifter's Tier-1 requires types — a PRE-EXISTING limitation this test discovered rather than
/// introduced (verified: the same failure occurs with the prologue removed by hand). Recorded in the
/// plan's open list; it bounds what "lifts back" currently means and is not papered over here.
#[test]
fn a_transpiled_file_lifts_back() {
    let php = crate::cli::cmd_transpile(
        "package Main;\n\nfunction greet(string n): string {\n    return \"hi \" + n;\n}\n",
    )
    .expect("transpile");
    assert!(
        php.starts_with("<?php\ndeclare(strict_types=1);\n"),
        "emitter must open with the DEC-401 prologue:\n{php}"
    );
    crate::lift::lifter::lift_source(&php)
        .unwrap_or_else(|e| panic!("transpiled output failed to lift back: {e}\n{php}"));
}

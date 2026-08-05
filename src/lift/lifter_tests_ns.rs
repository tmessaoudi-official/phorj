//! LIFT-NS lifter tests — `namespace` → phorj `package`, and `use` → phorj `import`.
//!
//! Why this slice exists: `namespace` and `use` were in the parser's `UNSUPPORTED_KW`, so **no**
//! namespaced PHP file lifted at all — it failed at the PARSER. Attribute lifting was the *second*
//! blocker, not the first.
//!
//! Scope, stated honestly: this cleared ONE of the two mandatory PSR-12 prologue lines — DEC-401 closed
//! `declare(strict_types=1);` and LIFT-ATTR closed `#[…]`, so the whole prologue now lifts. What is STILL
//! open is the payoff: a lifted `import` cannot resolve in a flat file (`E-MODULE-NOT-FOUND`), so the
//! `use` half waits on project-aware lifting. Tracked, not implied fixed.
//!
//! Every `use` case below REFERENCES the imported name (as a parameter type), because an import whose
//! name is unreferenced is deliberately dropped — see `unreferenced_use_is_dropped`. An earlier draft
//! of these tests asserted on unused imports and so passed while the lifted draft actually failed
//! `phg check` with `E-UNUSED-IMPORT`; asserting the import STRING is not the same as asserting the
//! draft is valid.
//!
//! Split into its own file rather than grown onto `lifter_tests.rs` (414 lines) — Invariant 13's
//! split-as-you-go default.

use super::lifter::lift_source;

fn lift(php: &str) -> String {
    lift_source(php).expect("lift")
}

/// A one-function PHP file in namespace `ns`, with `uses` before it, whose function takes `param_ty`
/// so the imported name is genuinely referenced.
fn php_with_use(ns: &str, uses: &str, param_ty: &str) -> String {
    format!("<?php\nnamespace {ns};\n{uses}\nfunction f({param_ty} $x): int {{ return 1; }}\n")
}

#[test]
fn namespace_becomes_the_package() {
    let out = lift("<?php\nnamespace App\\Entity;\nfunction f(): int { return 1; }\n");
    assert!(
        out.contains("package App.Entity;"),
        "expected the namespace to become the package:\n{out}"
    );
}

#[test]
fn no_namespace_still_lifts_to_package_main() {
    // Regression guard: every file that lifted before this slice must keep its package line. `Main` is
    // the historical default and a lot of existing output depends on it.
    let out = lift("<?php\nfunction f(): int { return 1; }\n");
    assert!(out.contains("package Main;"), "{out}");
}

#[test]
fn lowercase_namespace_segments_are_pascalized() {
    // NOT cosmetic: `E-PKG-CASE` is enforced, so `package app.my_pkg;` would be REJECTED by the
    // checker ("package segment `app` must be PascalCase"). PHP does not guarantee PascalCase
    // namespaces, so passing them through verbatim would emit an uncompilable draft.
    let out = lift("<?php\nnamespace app\\my_pkg;\nfunction f(): int { return 1; }\n");
    assert!(out.contains("package App.MyPkg;"), "{out}");
}

#[test]
fn an_already_upper_segment_is_not_lowercased() {
    // `ORM` must stay `ORM`, not become `Orm` — only the FIRST character is what E-PKG-CASE
    // constrains, and the segment is a name the developer chose.
    let out = lift("<?php\nnamespace ORM\\Mapping;\nfunction f(): int { return 1; }\n");
    assert!(out.contains("package ORM.Mapping;"), "{out}");
}

#[test]
fn use_becomes_an_import() {
    let out = lift(&php_with_use(
        "App\\Cli",
        "use App\\Support\\Helper;",
        "Helper",
    ));
    assert!(out.contains("import App.Support.Helper;"), "{out}");
}

#[test]
fn aliased_use_becomes_an_aliased_import() {
    // Phorj supports import aliases natively, so the alias the developer wrote is PRESERVED rather
    // than expanded into a fully-qualified name at every use site.
    let out = lift(&php_with_use(
        "App\\Cli",
        "use Doctrine\\ORM\\Mapping as ORM;",
        "ORM",
    ));
    assert!(out.contains("import Doctrine.ORM.Mapping as ORM;"), "{out}");
}

/// The last segment of a `use` path is the CLASS name and is never re-cased — but it must still be a
/// legal phorj identifier. `use App\Café;` emitted `import App.Café;`, a draft phorj's lexer cannot even
/// LEX, and a lex error suppresses every other diagnostic in the file. (LIFT-ATTR needed the same check
/// for attribute names, so the two share `type_segment`.)
#[test]
fn a_non_ascii_class_name_in_a_use_is_refused() {
    let err = lift_source(&php_with_use(
        "App\\Cli",
        "use App\\Caf\u{e9};",
        "Caf\u{e9}",
    ))
    .expect_err("a non-ASCII class name must be refused, not emitted");
    assert!(err.contains("ASCII"), "{err}");
}

#[test]
fn a_leading_root_marker_is_not_part_of_the_path() {
    // `use \App\Helper;` and `use App\Helper;` name the same class in PHP (a `use` path is always
    // fully qualified), so both must lift to the same import.
    let rooted = lift(&php_with_use("App\\Cli", "use \\App\\Helper;", "Helper"));
    let bare = lift(&php_with_use("App\\Cli", "use App\\Helper;", "Helper"));
    assert!(rooted.contains("import App.Helper;"), "{rooted}");
    assert!(bare.contains("import App.Helper;"), "{bare}");
}

#[test]
fn the_class_name_segment_keeps_its_own_casing() {
    // Only the NAMESPACE segments are package segments. The last segment is the class's own name and
    // must not be reshaped — renaming a type would break every reference to it.
    let out = lift(&php_with_use(
        "App\\Cli",
        "use App\\my_pkg\\myClass;",
        "myClass",
    ));
    assert!(out.contains("import App.MyPkg.myClass;"), "{out}");
}

#[test]
fn uses_are_emitted_in_source_order() {
    let out = lift(
        "<?php\nnamespace App\\Cli;\nuse App\\Alpha;\nuse App\\Beta;\nuse App\\Gamma;\nfunction f(Alpha $a, Beta $b, Gamma $g): int { return 1; }\n",
    );
    let a = out.find("import App.Alpha;").expect("alpha");
    let b = out.find("import App.Beta;").expect("beta");
    let g = out.find("import App.Gamma;").expect("gamma");
    assert!(a < b && b < g, "imports must keep source order:\n{out}");
}

#[test]
fn unreferenced_use_is_dropped() {
    // `E-UNUSED-IMPORT` is a HARD error in phorj, while an unused `use` is legal and very common in
    // PHP (editors add them; code moves on). Emitting it verbatim would be "a lift that fails the very
    // check it should pass" — the rule `lifter/exceptions.rs` already follows. Dropping it is
    // semantically lossless: a `use` only creates a local alias, so an unused one has no behaviour.
    let out = lift(
        "<?php\nnamespace App\\Cli;\nuse App\\Support\\Unused;\nfunction f(): int { return 1; }\n",
    );
    assert!(
        !out.contains("import App.Support.Unused;"),
        "an unreferenced use must not be imported:\n{out}"
    );
    assert!(out.contains("package App.Cli;"), "{out}");
}

#[test]
fn a_partial_name_match_does_not_keep_an_import() {
    // Word-boundary matching: a file mentioning `MoneyBag` must NOT keep an import of `Money`, or the
    // draft picks up a spurious `E-UNUSED-IMPORT`.
    let out = lift(
        "<?php\nnamespace App\\Cli;\nuse App\\Money;\nfunction f(MoneyBag $b): int { return 1; }\n",
    );
    assert!(
        !out.contains("import App.Money;"),
        "substring match wrongly kept the import:\n{out}"
    );
}

#[test]
fn a_namespaced_file_with_output_lifts_to_valid_phorj() {
    // End-to-end: the combination that could not be lifted AT ALL before this slice — a namespace, a
    // referenced aliased use, and real code — must lex and parse as phorj.
    let out = lift(
        "<?php\nnamespace App\\Cli;\nuse App\\Support\\Helper as H;\nfunction shout(H $h): string { return \"hi\"; }\n",
    );
    let toks = crate::tokenizer::lex(&out)
        .unwrap_or_else(|e| panic!("lifted output failed to lex: {e:?}\n{out}"));
    crate::parser::Parser::new(toks)
        .parse_program()
        .unwrap_or_else(|e| panic!("lifted output failed to parse: {e:?}\n{out}"));
    assert!(out.contains("package App.Cli;"), "{out}");
    assert!(out.contains("import App.Support.Helper as H;"), "{out}");
}

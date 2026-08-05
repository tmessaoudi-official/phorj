//! DEC-194 user attributes — canonical-path resolution and named arguments (2026-08-04).
//!
//! Two changes are pinned here. Both had the same root: `check_user_attribute_use` was the ONE place
//! that treated a user attribute differently from a built-in.
//!
//! 1. **Resolution by canonical path.** It used to resolve by LEAF, discarding the qualifier, so
//!    `#[ORM.Column]`, `#[Assert.Column]` and `#[Totally.Made.Up.Column]` all bound to one
//!    `class Column` and all checked CLEAN — two genuinely different attributes silently collapsing.
//!    Built-ins never had this bug (`#[Bogus.Entry]` was always rejected), so the fix deletes the
//!    special case rather than adding one.
//! 2. **Named arguments.** `#[Route(path: "/x")]` was `E-NAMED-ARG-MISPLACED` even though named args
//!    already work on ordinary calls and on BUILT-IN attributes (`#[Entry(kind: …)]`).

use super::support::*;

fn has(errs: &[Diagnostic], code: &str) -> bool {
    errs.iter().any(|d| d.code == Some(code))
}

/// A single-package program declaring `#[Attribute] class Column(string name)` and using it as `use`.
fn with_column_attr(use_site: &str) -> Vec<Diagnostic> {
    errors_of(&format!(
        "import Core.Runtime.Attribute; \
         #[Attribute] class Column {{ constructor(public string name) {{}} }} \
         #[{use_site}] function tagged(): int {{ return 1; }}"
    ))
}

#[test]
fn a_bare_user_attribute_resolves() {
    assert!(
        with_column_attr("Column(\"a\")").is_empty(),
        "{:?}",
        with_column_attr("Column(\"a\")")
    );
}

#[test]
fn a_qualifier_that_names_no_package_is_rejected() {
    // THE REGRESSION TEST. Before the fix all three of these checked clean, because the qualifier was
    // thrown away before the lookup. `class Column` here lives in the implicit `Main` package, so its
    // canonical path is bare `Column` — and `attr_path_matches` requires the canonical to be at least
    // as long as the written name, so a bare canonical refuses every qualifier.
    for bogus in [
        "ORM.Column(\"a\")",
        "Assert.Column(\"a\")",
        "Totally.Made.Up.Column(\"a\")",
    ] {
        let errs = with_column_attr(bogus);
        assert!(
            has(&errs, "E-UNKNOWN-ATTRIBUTE"),
            "`#[{bogus}]` must not resolve to an unrelated `Column`: {errs:?}"
        );
    }
}

#[test]
fn a_bogus_qualifier_on_a_builtin_was_always_rejected() {
    // The comparison that shows the fix removed an inconsistency rather than adding a rule: built-in
    // attributes already behaved correctly, which is why their canonical-path matching is what the
    // user-attribute path now reuses.
    let errs = errors_of(
        "import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
         #[Bogus.Entry(kind: EntryKind.Cli)] function main(): int { return 0; }",
    );
    assert!(has(&errs, "E-UNKNOWN-ATTRIBUTE"), "{errs:?}");
}

#[test]
fn every_correct_partial_qualifier_of_a_builtin_still_resolves() {
    // Guard against over-tightening: the developer ruled that `#[Entry]` AND `#[Core.Runtime.Entry]`
    // must both work, and the partial form between them too.
    for form in ["Entry", "Runtime.Entry", "Core.Runtime.Entry"] {
        let errs = errors_of(&format!(
            "import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             #[{form}(kind: EntryKind.Cli)] function main(): int {{ return 0; }}"
        ));
        assert!(errs.is_empty(), "`#[{form}]` must resolve: {errs:?}");
    }
}

#[test]
fn a_named_argument_is_accepted_and_normalized() {
    // `#[Route(path: "/x")]` is the dominant real-world PHP attribute form. It was
    // `E-NAMED-ARG-MISPLACED`: the positional `zip` let an `Expr::NamedArg` reach `check_arg`, which
    // only accepts one inside a call's argument list.
    let errs = with_column_attr("Column(name: \"a\")");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn named_arguments_may_be_written_out_of_order() {
    // The whole point of named args: order does not have to match the constructor. Normalization
    // reuses `normalize_named_args` (DEC-297), the same helper `new X(name: v)` uses.
    let errs = errors_of(
        "import Core.Runtime.Attribute; \
         #[Attribute] class Route { constructor(public string method, public string path) {} } \
         #[Route(path: \"/users\", method: \"GET\")] function users(): int { return 1; }",
    );
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn a_named_argument_is_still_type_checked() {
    // Normalizing must not bypass the argument TYPE check — that compile-time guarantee is what phorj
    // has over PHP, which only fails when the attribute is reflected.
    let errs = with_column_attr("Column(name: 42)");
    assert!(
        has(&errs, "E-ATTRIBUTE-ARG-TYPE"),
        "a wrongly-typed named arg must still be caught: {errs:?}"
    );
}

#[test]
fn an_unknown_named_argument_is_rejected() {
    let errs = with_column_attr("Column(nmae: \"a\")");
    assert!(
        !errs.is_empty(),
        "a misspelled parameter name must not be silently accepted"
    );
}

#[test]
fn arity_is_still_enforced_after_normalization() {
    let errs = with_column_attr("Column(\"a\", \"b\")");
    assert!(has(&errs, "E-ATTRIBUTE-ARITY"), "{errs:?}");
}

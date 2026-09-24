//! DEC-532 (scout row 5l): a throw-expression has type `never`, the bottom — so the branch that does
//! NOT throw decides an `if`-expression's or `match`'s type, whichever order the arms come in — and it
//! answers to the same checked-exception rules as the `throw` statement.

use super::support::*;

const BAD: &str = "class BadError implements Error { constructor(public string message) {} } ";

fn codes(src: &str) -> Vec<&'static str> {
    errors_of(&format!("{BAD}{src}"))
        .iter()
        .filter_map(|d| d.code)
        .collect()
}

fn clean(src: &str) {
    let errs = errors_of(&format!("{BAD}{src}"));
    assert!(errs.is_empty(), "{src}\n{errs:?}");
}

fn has_error(src: &str) {
    assert!(
        !errors_of(&format!("{BAD}{src}")).is_empty(),
        "expected a type error: {src}"
    );
}

#[test]
fn a_coalesce_throw_yields_the_unwrapped_type() {
    clean(
        "function k(string? s): string throws BadError { return s ?? throw new BadError(\"x\"); }",
    );
}

#[test]
fn the_non_throwing_if_branch_decides_the_type_in_both_orders() {
    clean("function f(bool c): int throws BadError { return if (c) { throw new BadError(\"t\") } else { 1 }; }");
    clean("function f(bool c): int throws BadError { return if (c) { 1 } else { throw new BadError(\"e\") }; }");
    // `never` must not leak out of the join: the int branch makes a `string` return a type error.
    has_error(
        "function f(bool c): string throws BadError { return if (c) { throw new BadError(\"t\") } else { 1 }; }",
    );
    has_error(
        "function f(bool c): string throws BadError { return if (c) { 1 } else { throw new BadError(\"e\") }; }",
    );
}

#[test]
fn the_non_throwing_match_arm_decides_the_type_in_both_orders() {
    clean(
        "function r(string a): int throws BadError { return match (a) { \"t\" => throw new BadError(\"t\"), default => 2 }; }",
    );
    clean(
        "function r(string a): int throws BadError { return match (a) { \"x\" => 1, default => throw new BadError(a) }; }",
    );
    has_error(
        "function r(string a): string throws BadError { return match (a) { \"t\" => throw new BadError(\"t\"), default => 2 }; }",
    );
}

#[test]
fn a_thrown_value_must_be_an_error() {
    assert!(
        codes("function k(string? s): string { return s ?? throw 5; }").contains(&"E-THROW-TYPE")
    );
}

#[test]
fn a_throw_expression_must_be_declared_or_caught() {
    assert!(
        codes("function k(string? s): string { return s ?? throw new BadError(\"x\"); }")
            .contains(&"E-THROW-UNDECLARED")
    );
    clean(
        "function k(string? s): string { try { return s ?? throw new BadError(\"x\"); } catch (BadError e) { return e.message; } }",
    );
    assert!(codes(
        "#[Entry(kind: EntryKind.Cli)] function main(): void { string? s = null; var t = s ?? throw new BadError(\"m\"); }"
    )
    .contains(&"E-UNCAUGHT-THROW"));
}

#[test]
fn a_lambda_body_throw_discharges_against_the_lambda_throws() {
    clean(
        "function g(): void { var f = function(int x): int throws BadError => throw new BadError(\"l\"); }",
    );
    assert!(codes(
        "function g(): void { var f = function(int x): int => throw new BadError(\"l\"); }"
    )
    .contains(&"E-THROW-UNDECLARED"));
}

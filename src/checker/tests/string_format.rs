//! W3-5 / DEC-199 slice 1 — `String.format` = PHP-style `%` sprintf (`%s`/`%d`/`%%`), strict.
//!
//! `String.format` is a real native (`text_format` / `__phorj_format`); the checker special-cases it
//! for arg-type validation + a compile-time `E-FORMAT-UNSUPPORTED`/`E-FORMAT-ARG-COUNT` gate on a
//! LITERAL spec. These tests go through `check_and_expand` (where the special-case + DEC-197 bare-import
//! rewrite run). `%d`-type strictness is a RUNTIME fault (not a compile error), so those cases type OK.
use super::support::*;

fn expand(src: &str) -> Result<crate::ast::Program, String> {
    crate::cli::check_and_expand(&prog_raw(src), src)
}

#[test]
fn qualified_format_resolves() {
    assert!(
        expand(
            "package Main; import Core.String; \
             function main(): void { var s = String.format(\"%s owes %d\", [\"Ada\", 3]); }"
        )
        .is_ok(),
        "qualified String.format should type-check"
    );
}

#[test]
fn bare_imported_format_resolves() {
    // DEC-197 bare import + the bare→qualified rewrite.
    assert!(
        expand(
            "package Main; import Core.String.format; \
             function main(): void { var s = format(\"%s = %d\", [\"n\", 7]); }"
        )
        .is_ok(),
        "a bare member-imported format should resolve"
    );
}

#[test]
fn heterogeneous_value_list_is_accepted() {
    // Format values are positional scalars — a mixed literal list is fine (it would otherwise fail the
    // homogeneous list-literal rule), and an empty list matches a no-directive spec.
    assert!(
        expand(
            "package Main; import Core.String; \
             function main(): void { var a = String.format(\"%s %d %d\", [\"x\", 1, 2]); \
                                     var b = String.format(\"plain\", []); }"
        )
        .is_ok(),
        "a heterogeneous / empty value list should type-check"
    );
}

#[test]
fn percent_percent_is_a_literal_not_a_directive() {
    assert!(
        expand(
            "package Main; import Core.String; \
             function main(): void { var s = String.format(\"%d%%\", [50]); }"
        )
        .is_ok(),
        "%% is a literal percent, not a directive (one value for the one %d)"
    );
}

#[test]
fn unsupported_directive_is_rejected_for_a_literal_spec() {
    let err = expand(
        "package Main; import Core.String; \
         function main(): void { var s = String.format(\"%f\", [1.5]); }",
    )
    .expect_err("%f is not supported in this slice");
    assert!(err.contains("E-FORMAT-UNSUPPORTED"), "got:\n{err}");
}

#[test]
fn arg_count_mismatch_is_rejected_for_a_literal_spec() {
    let err = expand(
        "package Main; import Core.String; \
         function main(): void { var s = String.format(\"%s %s\", [\"only-one\"]); }",
    )
    .expect_err("2 directives but 1 value must be rejected");
    assert!(err.contains("E-FORMAT-ARG-COUNT"), "got:\n{err}");
}

#[test]
fn non_list_values_arg_is_rejected() {
    let err = expand(
        "package Main; import Core.String; \
         function main(): void { var s = String.format(\"%d\", 5); }",
    )
    .expect_err("the values argument must be a list");
    assert!(err.contains("E-FORMAT-ARGS-TYPE"), "got:\n{err}");
}

#[test]
fn dynamic_runtime_spec_type_checks() {
    // A runtime (non-literal) spec skips the compile-time directive gate (validated at runtime); it
    // must still type-check as a `string` argument.
    assert!(
        expand(
            "package Main; import Core.String; \
             function fmt(string tmpl): string { return String.format(tmpl, [1, 2]); }"
        )
        .is_ok(),
        "a dynamic runtime spec should type-check (validated at runtime)"
    );
}

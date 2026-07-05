//! DEC-197 — module-function import discipline (two-mode: whole-module → qualified, member → bare).
//!
//! Enforced by the `fn_imports` map + `check_named_call`'s bare-resolution arm (recording a
//! bare→qualified rewrite) inside the CLI's `check_and_expand` chokepoint, so these tests go through
//! that path. An `is_ok` on a bare-call program proves the member import resolved AND the rewrite is
//! well-formed (an unresolved bare native would be `unknown function`).
use super::support::*;

fn expand(src: &str) -> Result<crate::ast::Program, String> {
    crate::cli::check_and_expand(&prog_raw(src), src)
}

// --- BARE form requires a MEMBER import -------------------------------------------------------

#[test]
fn bare_native_without_import_is_unknown() {
    let err = expand("package Main; function main(): void { printLine(\"hi\"); }")
        .expect_err("a bare native call with no import must be rejected");
    assert!(err.contains("unknown function"), "got:\n{err}");
}

#[test]
fn member_import_enables_bare_call() {
    assert!(
        expand(
            "package Main; import Core.Output.printLine; \
             function main(): void { printLine(\"hi\"); }"
        )
        .is_ok(),
        "member import should enable the bare call form"
    );
}

#[test]
fn member_import_enables_bare_string_fn() {
    assert!(
        expand(
            "package Main; import Core.String.trim; import Core.Output.printLine; \
             function main(): void { printLine(trim(\"  hi  \")); }"
        )
        .is_ok(),
        "a bare member-imported String.trim should resolve"
    );
}

#[test]
fn grouped_member_import_enables_each_bare() {
    // DEC-186 group syntax desugars to per-member imports, each enabling its bare function.
    assert!(
        expand(
            "package Main; import Core.Output.{ print, printLine }; \
             function main(): void { print(\"a\"); printLine(\"b\"); }"
        )
        .is_ok(),
        "grouped member import should enable each listed function bare"
    );
}

#[test]
fn aliased_member_import_binds_the_alias() {
    assert!(
        expand(
            "package Main; import Core.String.trim as clean; import Core.Output.printLine; \
             function main(): void { printLine(clean(\"  x  \")); }"
        )
        .is_ok(),
        "an `as`-aliased function import should bind the alias for the bare call"
    );
}

// --- STRICT two-mode: a member import does NOT enable a qualified sibling ---------------------

#[test]
fn whole_module_import_keeps_qualified() {
    assert!(
        expand(
            "package Main; import Core.Output; \
             function main(): void { Output.printLine(\"hi\"); }"
        )
        .is_ok(),
        "the qualified form under a whole-module import is unchanged"
    );
}

#[test]
fn member_import_does_not_enable_qualified_sibling() {
    // `import Core.Output.printLine;` enables bare `printLine` only — a qualified sibling
    // (`Output.print`) still needs `import Core.Output;` (strict, mirrors the intrinsic model).
    let err = expand(
        "package Main; import Core.Output.printLine; \
         function main(): void { Output.print(\"x\"); }",
    )
    .expect_err("a qualified sibling must not be enabled by a member function import");
    assert!(!err.is_empty(), "expected a resolution error, got ok");
}

#[test]
fn whole_module_import_does_not_enable_bare() {
    // The inverse: a whole-module import gives the qualified form, NOT the bare form.
    let err =
        expand("package Main; import Core.Output; function main(): void { printLine(\"x\"); }")
            .expect_err("a whole-module import must not enable the bare call form");
    assert!(err.contains("unknown function"), "got:\n{err}");
}

// --- Collisions resolved by `as` (ambiguity is an error) --------------------------------------

#[test]
fn duplicate_bound_name_conflicts() {
    let err = expand(
        "package Main; import Core.String.trim as x; import Core.String.length as x; \
         function main(): void {}",
    )
    .expect_err("two imports binding the same bare name must conflict");
    assert!(err.contains("E-IMPORT-CONFLICT"), "got:\n{err}");
}

// --- Casing carve-out: a camelCase function leaf is NOT E-PKG-CASE ----------------------------

#[test]
fn camel_leaf_is_exempt_from_pkg_case() {
    // The member-import leaf `printLine` is camelCase (a value name), not PascalCase — it must be
    // exempt from the import-segment PascalCase rule (E-PKG-CASE). A clean expand proves the carve-out.
    let ok = expand(
        "package Main; import Core.Output.printLine; function main(): void { printLine(\"hi\"); }",
    );
    assert!(
        ok.is_ok(),
        "camelCase function leaf must not trip E-PKG-CASE: {ok:?}"
    );
}

#[test]
fn pascal_alias_for_function_is_rejected() {
    // A function alias is a value name — camelCase. A PascalCase alias is `E-NAME-CASE`, not silently ok.
    let err = expand("package Main; import Core.String.trim as Clean; function main(): void {}")
        .expect_err("a PascalCase function alias must be rejected");
    assert!(err.contains("E-NAME-CASE"), "got:\n{err}");
}

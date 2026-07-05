//! Fault-intrinsic import discipline (DEC-196 Q3) — `Core.Assert` / `Core.Abort`, two-mode.
//!
//! Enforced by `resolve_intrinsic_imports` in the CLI's `check_and_expand` chokepoint (not the raw
//! checker), so these tests go through that path. The qualified form (`Assert.assert(...)`) is also
//! normalized there to the bare intrinsic; an `is_ok` result on a qualified program therefore proves
//! the rewrite (an un-rewritten `Assert.assert` member call would fail resolution and be `is_err`).
use super::support::*;

fn expand(src: &str) -> Result<crate::ast::Program, String> {
    crate::cli::check_and_expand(&prog_raw(src), src)
}

// --- BARE form requires a MEMBER import -------------------------------------------------------

#[test]
fn bare_intrinsic_without_import_is_unimported() {
    let err = expand("package Main; import Core.Output; function main(): void { assert(true); }")
        .expect_err("a bare intrinsic with no import must be rejected");
    assert!(err.contains("E-UNIMPORTED"), "got:\n{err}");
}

#[test]
fn member_import_enables_bare_assert() {
    assert!(
        expand("package Main; import Core.Assert.assert; function main(): void { assert(true); }")
            .is_ok(),
        "member import should enable the bare form"
    );
}

#[test]
fn member_import_enables_bare_panic() {
    assert!(
        expand(
            "package Main; import Core.Abort.panic; \
             function pos(int n): int { if (n < 0) { panic(\"neg\"); } return n; }"
        )
        .is_ok(),
        "member import should enable bare panic"
    );
}

#[test]
fn grouped_member_import_enables_bare() {
    // The DEC-186 group syntax desugars to per-member imports, each enabling its bare intrinsic.
    assert!(
        expand(
            "package Main; import Core.Abort.{ panic, unreachable }; \
             function pick(bool b): int { if (b) { return 1; } if (!b) { return 0; } unreachable(); } \
             function guard(int n): int { if (n < 0) { panic(\"neg\"); } return n; }"
        )
        .is_ok(),
        "grouped member import should enable each listed intrinsic bare"
    );
}

// --- QUALIFIED form requires a WHOLE-MODULE import ---------------------------------------------

#[test]
fn module_import_enables_qualified_and_rewrites() {
    // `is_ok` here also proves the qualified→bare rewrite: an un-rewritten `Assert.assert` member
    // call has no native and would fail resolution.
    assert!(
        expand("package Main; import Core.Assert; function main(): void { Assert.assert(true); }")
            .is_ok(),
        "module import should enable the qualified form (and rewrite it to bare)"
    );
}

#[test]
fn module_import_enables_qualified_abort() {
    assert!(
        expand(
            "package Main; import Core.Abort; \
             function pos(int n): int { if (n < 0) { Abort.panic(\"neg\"); } return n; }"
        )
        .is_ok(),
        "module import should enable qualified Abort intrinsics"
    );
}

// --- Strict two-mode: each form needs ITS OWN import ------------------------------------------

#[test]
fn module_import_does_not_enable_bare() {
    // `import Core.Assert;` gives the QUALIFIED form only — a bare `assert(...)` still needs the
    // member import.
    let err = expand("package Main; import Core.Assert; function main(): void { assert(true); }")
        .expect_err("module import must not enable the bare form");
    assert!(err.contains("E-UNIMPORTED"), "got:\n{err}");
}

#[test]
fn qualified_without_module_import_is_unimported() {
    // Member-import only gives the bare form; the qualified `Assert.assert(...)` needs the module.
    let err = expand(
        "package Main; import Core.Assert.assert; function main(): void { Assert.assert(true); }",
    )
    .expect_err("qualified form must not be enabled by a member import");
    assert!(err.contains("E-UNIMPORTED"), "got:\n{err}");
}

// --- Import-shape errors ----------------------------------------------------------------------

#[test]
fn member_import_of_nonintrinsic_is_import_unknown() {
    let err = expand("package Main; import Core.Abort.bogus; function main(): void {}")
        .expect_err("Core.Abort has no `bogus` intrinsic");
    assert!(err.contains("E-IMPORT-UNKNOWN"), "got:\n{err}");
}

#[test]
fn member_import_wrong_module_is_import_unknown() {
    // `assert` belongs to Core.Assert, not Core.Abort.
    let err = expand("package Main; import Core.Abort.assert; function main(): void {}")
        .expect_err("assert is not a Core.Abort intrinsic");
    assert!(err.contains("E-IMPORT-UNKNOWN"), "got:\n{err}");
}

// --- Casing carve-out: a lowercase intrinsic leaf is NOT E-PKG-CASE ----------------------------

#[test]
fn lowercase_intrinsic_leaf_is_not_pkg_case() {
    // `import Core.Abort.panic;` has a deliberately lowercase leaf; it must not trip the PascalCase
    // import-segment rule (E-PKG-CASE). A no-error result here proves the carve-out.
    let errs = errors_of_raw(
        "package Main; import Core.Abort.panic; \
         function pos(int n): int { if (n < 0) { panic(\"neg\"); } return n; }",
    );
    assert!(
        !errs.iter().any(|d| d.code == Some("E-PKG-CASE")),
        "lowercase intrinsic leaf must be exempt from E-PKG-CASE, got: {errs:?}"
    );
}

// --- No-op for intrinsic-free programs --------------------------------------------------------

#[test]
fn intrinsic_free_program_is_unaffected() {
    assert!(
        expand(
            "package Main; import Core.Output; function main(): void { Output.printLine(\"hi\"); }"
        )
        .is_ok(),
        "a program that uses no intrinsic must be unaffected"
    );
}

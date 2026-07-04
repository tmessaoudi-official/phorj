//! `Core.Option<T>` / `Core.Result<T, E>` — the canonical compiler-INJECTED enums (DEC-182, Wave B
//! foundation). Injected by the CLI's `check_and_expand` chokepoint gated on `import Core.Option;` /
//! `import Core.Result;`, so these tests go through that path (not the raw checker, which never sees
//! the prelude). The FIRST generic injected enums: `T`/`E` are checked as `Ty::Param` then erased.
use super::support::*;

/// Qualified variants (`Option.Some`/`Option.None`) type-check once `Core.Option` is imported, and the
/// generic parameter is inferred at the constructor + recovered at the `match`.
#[test]
fn qualified_option_variants_typecheck() {
    let src = "package Main; import Core.Option; \
               function describe(Option<int> o) -> string { \
                   return match (o) { Option.Some(n) => \"some\", Option.None() => \"none\" }; \
               } \
               function main() -> void { \
                   var s = new Option.Some(7); \
                   Option<int> n = new Option.None(); \
               }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "qualified injected Option variants must typecheck"
    );
}

/// A **bare** injected variant (`new Some(…)`) is `E-INJECTED-VARIANT-BARE` — the "nothing in the wind"
/// rule: an injected name a user never wrote must carry its enum.
#[test]
fn bare_option_variant_is_injected_variant_bare() {
    let src = "package Main; import Core.Option; \
               function main() -> void { var s = new Some(7); }";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("bare injected Option variant must be rejected");
    assert!(
        err.contains("E-INJECTED-VARIANT-BARE"),
        "expected E-INJECTED-VARIANT-BARE, got:\n{err}"
    );
}

/// Qualified `Result.Success`/`Result.Failure` type-check once `Core.Result` is imported; the error
/// payload `E` is an ordinary user type.
#[test]
fn qualified_result_variants_typecheck() {
    let src = "package Main; import Core.Result; \
               function safeDiv(int a, int b) -> Result<int, string> { \
                   return match (b) { 0 => new Result.Failure(\"divide by zero\"), _ => new Result.Success(a / b) }; \
               } \
               function main() -> void { var r = safeDiv(10, 2); }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "qualified injected Result variants must typecheck"
    );
}

/// A bare `new Success(…)` is `E-INJECTED-VARIANT-BARE`, same rule as Option.
#[test]
fn bare_result_variant_is_injected_variant_bare() {
    let src = "package Main; import Core.Result; \
               function main() -> void { var r = new Success(7); }";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("bare injected Result variant must be rejected");
    assert!(
        err.contains("E-INJECTED-VARIANT-BARE"),
        "expected E-INJECTED-VARIANT-BARE, got:\n{err}"
    );
}

/// A user who declares their OWN `enum Option<T>` shadows the injection (its prelude is a no-op), so
/// their bare variants keep working — no `E-INJECTED-VARIANT-BARE` on their own type.
#[test]
fn user_declared_option_shadows_injection() {
    let src = "package Main; import Core.Option; \
               enum Option<T> { None, Some(T value) } \
               function main() -> void { var s = new Some(7); }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "a user-declared Option must shadow the injected one and allow bare variants"
    );
}

/// Without the import there is no injection: naming `Option` fails to resolve — proving the gate.
#[test]
fn option_type_unavailable_without_import() {
    let src = "package Main; \
               function main() -> void { Option<int> n = new Option.None(); }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_err(),
        "Option must not resolve without `import Core.Option`"
    );
}

/// `Result.toOption` bridges to `Core.Option`, so it MUST be imported too (Wave B B-2b, DEC-185). Used
/// UFCS-style without `import Core.Option`, the call would run on the Rust backends but fatal in the
/// transpiled PHP (`class Some not found`) — a byte-identity break; the checker rejects it in lockstep.
#[test]
fn result_to_option_ufcs_without_option_import_is_rejected() {
    let src = "package Main; import Core.Result; \
               function main() -> void { var r = new Result.Success(1); var o = r.toOption(); }";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("r.toOption() without import Core.Option must be rejected");
    assert!(
        err.contains("E-RESULT-TOOPTION-NEEDS-OPTION"),
        "expected E-RESULT-TOOPTION-NEEDS-OPTION, got:\n{err}"
    );
}

/// Same guard for the qualified call form `Result.toOption(r)`.
#[test]
fn result_to_option_qualified_without_option_import_is_rejected() {
    let src = "package Main; import Core.Result; \
               function main() -> void { var r = new Result.Success(1); var o = Result.toOption(r); }";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("Result.toOption(r) without import Core.Option must be rejected");
    assert!(
        err.contains("E-RESULT-TOOPTION-NEEDS-OPTION"),
        "expected E-RESULT-TOOPTION-NEEDS-OPTION, got:\n{err}"
    );
}

/// With BOTH `Core.Result` and `Core.Option` imported, the bridge type-checks (the happy path the
/// `result-combinators.phg` differential also exercises end-to-end).
#[test]
fn result_to_option_with_both_imports_typechecks() {
    let src = "package Main; import Core.Result; import Core.Option; \
               function main() -> void { var r = new Result.Success(1); var o = r.toOption(); }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "toOption with both imports must typecheck"
    );
}

// ── Wave B B-2c (DEC-186): variant imports ──────────────────────────────────────────────────────

/// A bare imported variant (`import Core.Result.Success;`) is usable bare in construction + patterns —
/// the rewrite qualifies it, so it type-checks like `Result.Success`.
#[test]
fn bare_imported_variant_typechecks_in_construction_and_pattern() {
    let src = "package Main; import Core.Result.Success; import Core.Result.Failure; \
               function f(): string { \
                   var r = new Success(1); \
                   return match (r) { Success(v) => \"ok\", Failure(e) => \"no\" }; \
               }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "bare imported variants must type-check in construction + patterns"
    );
}

/// An `as`-aliased imported variant binds the alias; the rewrite maps it to the real variant.
#[test]
fn aliased_imported_variant_typechecks() {
    let src = "package Main; import Core.Result.Success; import Core.Result.Failure as Fail; \
               function f(): string { \
                   var r = new Fail(\"e\"); \
                   return match (r) { Success(v) => \"ok\", Fail(e) => \"err\" }; \
               }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "aliased imported variant must type-check"
    );
}

/// A grouped import (`import Core.Option.{ Some, None };`) binds each member.
#[test]
fn grouped_imported_variants_typecheck() {
    let src = "package Main; import Core.Option.{ Some, None }; \
               function f(): string { \
                   var o = new Some(1); \
                   return match (o) { Some(n) => \"some\", None() => \"none\" }; \
               }";
    let prog = prog_raw(src);
    assert!(
        crate::cli::check_and_expand(&prog, src).is_ok(),
        "grouped imported variants must type-check"
    );
}

/// A variant import whose bound name already names a local type is `E-IMPORT-CONFLICT`.
#[test]
fn variant_import_colliding_with_local_type_is_conflict() {
    let src = "package Main; import Core.Result.Success; \
               class Success { public int n = 0; } \
               function main() -> void {}";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("a variant import colliding with a local type must be rejected");
    assert!(
        err.contains("E-IMPORT-CONFLICT"),
        "expected E-IMPORT-CONFLICT, got:\n{err}"
    );
}

/// A variant import whose bound name shadows a USER enum's variant is `E-IMPORT-CONFLICT` — otherwise the
/// import would silently hijack that enum's bare construction/patterns (a baffling type mismatch).
#[test]
fn variant_import_shadowing_user_enum_variant_is_conflict() {
    let src = "package Main; import Core.Result.Success; \
               enum Local { Success(int n) } \
               function f(): Local { return new Success(5); }";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("a variant import shadowing a user enum variant must be rejected");
    assert!(
        err.contains("E-IMPORT-CONFLICT"),
        "expected E-IMPORT-CONFLICT, got:\n{err}"
    );
}

/// Importing a variant the enum does not declare is `E-IMPORT-UNKNOWN`.
#[test]
fn unknown_variant_import_is_rejected() {
    let src = "package Main; import Core.Result.Nope; function main() -> void {}";
    let prog = prog_raw(src);
    let err = crate::cli::check_and_expand(&prog, src)
        .expect_err("importing a nonexistent variant must be rejected");
    assert!(
        err.contains("E-IMPORT-UNKNOWN"),
        "expected E-IMPORT-UNKNOWN, got:\n{err}"
    );
}

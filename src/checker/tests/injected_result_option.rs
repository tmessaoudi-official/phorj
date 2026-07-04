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

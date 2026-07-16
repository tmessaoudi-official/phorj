//! Checker tests — throws (M-Decomp W2b, by language feature).

use super::support::*;

#[test]
fn propagate_in_result_fn_is_clean() {
    // `?` in a let-initializer inside a `Result`-returning fn unwraps the `Success` payload (an `int`).
    let ok = errors_of(&format!(
        "{RESULT_DEF} \
             function f() -> Result<int, string> {{ return new Success(1); }} \
             function g() -> Result<int, string> {{ int x = f()?; return new Success(x + 1); }} \
             function main() -> void {{}}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn propagate_outside_let_initializer_is_position_error() {
    // `?` nested in a larger expression is `E-PROPAGATE-POSITION` (not a whole let-initializer).
    let bad = errors_of(&format!(
        "{RESULT_DEF} \
             function f() -> Result<int, string> {{ return new Success(1); }} \
             function g() -> Result<int, string> {{ int x = f()? + 1; return new Success(x); }} \
             function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-PROPAGATE-POSITION")),
        "expected E-PROPAGATE-POSITION, got {bad:?}"
    );
}

#[test]
fn intrinsic_panic_requires_string_literal() {
    // A non-literal panic message (interpolation) is `E-INTRINSIC-LITERAL`.
    let bad = errors_of(r#"function main() -> void { var n = 1; panic("bad {n}"); }"#);
    assert!(
        bad.iter().any(|d| d.code == Some("E-INTRINSIC-LITERAL")),
        "expected E-INTRINSIC-LITERAL, got {bad:?}"
    );
}

#[test]
fn intrinsic_assert_condition_must_be_bool() {
    let bad = errors_of(r#"function main() -> void { assert(1, "x"); }"#);
    assert!(
        !bad.is_empty(),
        "expected a type error for a non-bool assert condition"
    );
}

#[test]
fn intrinsic_name_is_reserved() {
    let bad = errors_of("function unreachable() -> void { return; } function main() -> void {}");
    assert!(
        bad.iter().any(|d| d.code == Some("E-RESERVED-INTRINSIC")),
        "expected E-RESERVED-INTRINSIC, got {bad:?}"
    );
}

#[test]
fn panic_tail_satisfies_return_totality() {
    // `panic` is `never`-typed, so a value-returning fn ending in it needs no further `return`.
    let ok = errors_of(r#"function f() -> int { panic("x"); } function main() -> void {}"#);
    assert!(
        ok.is_empty(),
        "expected clean (never satisfies totality), got {ok:?}"
    );
}

#[test]
fn propagate_in_non_result_fn_is_context_error() {
    // `?` requires the enclosing fn to return the same `Result` — otherwise `E-PROPAGATE-CONTEXT`.
    let bad = errors_of(&format!(
        "{RESULT_DEF} \
             function f() -> Result<int, string> {{ return new Success(1); }} \
             function g() -> int {{ int x = f()?; return x; }} \
             function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-PROPAGATE-CONTEXT")),
        "expected E-PROPAGATE-CONTEXT, got {bad:?}"
    );
}

#[test]
fn throw_undeclared_and_uncaught_is_error() {
    // A helper that throws but neither declares `throws` nor wraps it in a `try`.
    let bad = errors_of(&format!(
        "{ERRDEF} function f() -> void {{ throw new BadInput(\"x\"); }} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROW-UNDECLARED")),
        "expected E-THROW-UNDECLARED, got {bad:?}"
    );
}

#[test]
fn throw_declared_then_caught_at_call_is_clean() {
    // `f` declares `throws BadInput` (discharges its own throw); `main` calls it inside a `try`
    // catching `BadInput` (discharges the call). Both sides handled — clean.
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn throw_in_main_is_uncaught() {
    let bad = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ throw new BadInput(\"x\"); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-UNCAUGHT-THROW")),
        "expected E-UNCAUGHT-THROW, got {bad:?}"
    );
}

#[test]
fn main_may_not_declare_throws() {
    let bad = errors_of(&format!(
        "{ERRDEF} function main() -> void throws BadInput {{ throw new BadInput(\"x\"); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-UNCAUGHT-THROW")),
        "expected E-UNCAUGHT-THROW, got {bad:?}"
    );
}

#[test]
fn throws_error_root_is_too_broad() {
    let bad = errors_of(&format!(
        "{ERRDEF} function f() -> void throws Error {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (Error e) {{}} }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROWS-TOO-BROAD")),
        "expected E-THROWS-TOO-BROAD, got {bad:?}"
    );
}

#[test]
fn throw_non_error_value_is_type_error() {
    let bad = errors_of("function main() -> void { throw 42; }");
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROW-TYPE")),
        "expected E-THROW-TYPE, got {bad:?}"
    );
}

#[test]
fn bare_call_to_throwing_fn_is_unhandled() {
    let bad = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ f(); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED, got {bad:?}"
    );
}

#[test]
fn propagate_throws_to_declared_is_clean() {
    // `g` propagates `f`'s `BadInput` with `?` and declares it — clean; `main` catches the call.
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function g() -> void throws BadInput {{ f()?; }} \
             function main() -> void {{ try {{ g(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn propagate_throws_without_declaration_is_unhandled() {
    // `g` uses `?` but does not declare `throws BadInput` — the propagation is unhandled.
    let bad = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function g() -> void {{ f()?; }} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED, got {bad:?}"
    );
}

#[test]
fn catch_non_error_type_is_error() {
    let bad = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ try {{}} catch (int e) {{}} }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CATCH-TYPE")),
        "expected E-CATCH-TYPE, got {bad:?}"
    );
}

#[test]
fn shadowed_catch_clause_warns() {
    // A second `catch (BadInput …)` after the first can never run — a non-fatal lint.
    let warns = warnings_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput e) {{}} catch (BadInput e2) {{}} }}"
    ));
    assert!(
        warns.iter().any(|d| d.code == Some("W-CATCH-UNREACHABLE")),
        "expected W-CATCH-UNREACHABLE, got {warns:?}"
    );
}

#[test]
fn try_with_returning_arms_satisfies_totality() {
    // A `-> int` fn whose `try` body and `catch` both return diverges on every path — total.
    let ok = errors_of(&format!(
            "{ERRDEF} function g() -> int {{ try {{ return 1; }} catch (BadInput e) {{ return 0; }} }} \
             function main() -> void {{}}"
        ));
    assert!(ok.is_empty(), "expected clean (try totality), got {ok:?}");
}

#[test]
fn try_falling_through_misses_return() {
    // Both arms fall through, so the `-> int` fn does not return on all paths.
    let bad = errors_of(&format!(
        "{ERRDEF} function g() -> int {{ try {{}} catch (BadInput e) {{}} }} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-MISSING-RETURN")),
        "expected E-MISSING-RETURN, got {bad:?}"
    );
}

#[test]
fn throw_tail_satisfies_totality() {
    // A `throw` diverges, so a `-> int` fn whose only statement is a `throw` is total.
    let ok = errors_of(&format!(
        "{ERRDEF} function g() -> int throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ var n = g(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean (throw diverges), got {ok:?}");
}

#[test]
fn throws_mode_propagate_is_recorded_for_erasure() {
    // A throws-mode `?` is a checker-only marker: it must be recorded (mapped to the bare call)
    // so `resolve_html` erases the `Propagate` node before any backend sees it.
    let p = prog(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function g() -> void throws BadInput {{ f()?; }} function main() -> void {{}}"
    ));
    let (_warns, subst, _ufcs, _ovl, _reified, _pipes, _fills, _for_iters, _for_binds) =
        check_resolutions(&p).expect("checks clean");
    assert_eq!(
        subst.len(),
        1,
        "exactly one throws-? recorded, got {subst:?}"
    );
    assert!(
        matches!(subst.values().next(), Some(crate::ast::Expr::Call { .. })),
        "the erased `?` must map to the bare call, got {subst:?}"
    );
    // And the substitution actually removes the `Propagate` node.
    let expanded = resolve_html(p, &subst);
    assert!(
        !program_has_propagate(&expanded),
        "throws-mode `?` Propagate node was not erased"
    );
}

// ── method-call throws discharge (Soundness Batch C) ─────────────────────────────────────────────

#[test]
fn method_throws_unhandled_at_bare_call_is_error() {
    // A method declaring `throws BadInput`, called bare (no `try`), must be rejected at the call
    // site — the same `E-CALL-UNHANDLED` as a free function (previously silently accepted).
    let bad = errors_of(&format!(
        "{ERRDEF} class Svc {{ function risky() -> int throws BadInput {{ throw new BadInput(\"x\"); }} }} \
             function main() -> void {{ var s = new Svc(); var n = s.risky(); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an unhandled method throw, got {bad:?}"
    );
}

#[test]
fn method_propagate_with_declared_throws_is_clean() {
    // `?`-throws on a METHOD call (the old `free_call_throws` deferral, closed in Ω-1): a
    // method call under `?` propagates to the enclosing `throws` exactly like a free function.
    let ok = errors_of(&format!(
        "{ERRDEF} class Svc {{ function risky() -> int throws BadInput {{ throw new BadInput(\"x\"); }}              function wrap() -> int throws BadInput {{ var n = this.risky()?; return n; }} }}              function main() -> void {{ var s = new Svc(); try {{ var n = s.wrap(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(
        ok.is_empty(),
        "method `?`-throws with a declared enclosing `throws` must be clean, got {ok:?}"
    );
}

#[test]
fn method_propagate_without_declared_throws_is_error() {
    // The same propagation WITHOUT the enclosing declaration is E-CALL-UNHANDLED (not the
    // Result-mode E-PROPAGATE-CONTEXT confusion the deferral used to produce).
    let bad = errors_of(&format!(
        "{ERRDEF} class Svc {{ function risky() -> int throws BadInput {{ throw new BadInput(\"x\"); }}              function wrap() -> int {{ var n = this.risky()?; return n; }} }}              function main() -> void {{ var s = new Svc(); var n = s.wrap(); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an undeclared method propagation, got {bad:?}"
    );
    assert!(
        !bad.iter().any(|d| d.code == Some("E-PROPAGATE-CONTEXT")),
        "must not fall through to Result-mode on a throwing method call, got {bad:?}"
    );
}

#[test]
fn method_throws_wrapped_in_try_is_clean() {
    // The same method call inside a `try` catching the declared type discharges cleanly.
    let ok = errors_of(&format!(
        "{ERRDEF} class Svc {{ function risky() -> int throws BadInput {{ throw new BadInput(\"x\"); }} }} \
             function main() -> void {{ var s = new Svc(); try {{ var n = s.risky(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn non_throwing_method_call_is_clean() {
    // Regression guard: a method that declares no `throws` is unaffected.
    let ok = errors_of(
        "class Svc { function ok() -> int { return 1; } } \
             function main() -> void { var s = new Svc(); var n = s.ok(); }",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn throws_comma_separated_multiple_is_clean() {
    // `throws A, B` (comma form, M-DOGFOOD W0) declares the same set as `throws A | B`. A caller
    // must discharge every declared throw — catching both is clean.
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput, NotFound {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput e) {{}} catch (NotFound e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn throws_comma_partial_catch_is_unhandled() {
    // Catching only one of two comma-declared throws leaves the other undischarged.
    let bad = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput, NotFound {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(!bad.is_empty(), "expected undischarged NotFound, got clean");
}

#[test]
fn throws_comma_and_union_mix() {
    // `throws A | B, C` mixes a union and a comma entry; the checker flattens to {A, B, C}.
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput | NotFound {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput e) {{}} catch (NotFound e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

// ── DEC-221: throwing constructors ──

/// A class whose constructor declares `throws BadInput` and throws inside its body.
const THROWING_CTOR: &str =
    "class Res { constructor(int x) throws BadInput { if (x < 0) { throw new BadInput(\"neg\"); } } }";

#[test]
fn throwing_ctor_bare_construction_is_unhandled() {
    // DEC-221: `new Res(...)` where the ctor `throws BadInput` and the caller neither catches nor
    // propagates is `E-CALL-UNHANDLED` — construction is a throwing expression.
    let bad = errors_of(&format!(
        "{ERRDEF} {THROWING_CTOR} function main() -> void {{ var r = new Res(-1); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an unhandled throwing construction, got {bad:?}"
    );
}

#[test]
fn throwing_ctor_construction_in_try_is_clean() {
    // The same construction wrapped in a `try` catching the declared type discharges cleanly.
    let ok = errors_of(&format!(
        "{ERRDEF} {THROWING_CTOR} \
             function main() -> void {{ try {{ var r = new Res(-1); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn throwing_ctor_construction_propagated_with_question_is_clean() {
    // DEC-221: `new X(...)?` propagates the ctor's throws to the enclosing `throws` (throws-mode `?`
    // now accepts a construction operand, `Expr::New(box Call)`, not just a bare call).
    let ok = errors_of(&format!(
        "{ERRDEF} {THROWING_CTOR} \
             function make() -> void throws BadInput {{ var r = new Res(-1)?; }} \
             function main() -> void {{ try {{ make(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn throwing_ctor_construction_propagated_without_declaration_is_unhandled() {
    // The same `new X(...)?` WITHOUT an enclosing `throws` is `E-CALL-UNHANDLED` — and must NOT fall
    // through to the Result-mode `E-PROPAGATE-CONTEXT` (a construction is not a `Result`).
    let bad = errors_of(&format!(
        "{ERRDEF} {THROWING_CTOR} \
             function make() -> void {{ var r = new Res(-1)?; }} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED, got {bad:?}"
    );
    assert!(
        !bad.iter().any(|d| d.code == Some("E-PROPAGATE-CONTEXT")),
        "must not fall through to Result-mode on a throwing construction, got {bad:?}"
    );
}

#[test]
fn ctor_body_discharges_against_declared_throws() {
    // The ctor BODY is checked with its declared throws in context (like `check_function`): a
    // throwing helper called under `?` propagates against the ctor's own `throws` — the DB_PRELUDE
    // pattern (`DbError.fail(e)?` inside `constructor(...) throws DbError`). Clean.
    let ok = errors_of(&format!(
        "{ERRDEF} function boom() -> int throws BadInput {{ throw new BadInput(\"x\"); }} \
             class Res {{ constructor() throws BadInput {{ var n = boom()?; }} }} \
             function main() -> void {{ try {{ var r = new Res(); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn ctor_body_undeclared_throw_is_unhandled() {
    // A throwing call inside a ctor that does NOT declare the throw is `E-CALL-UNHANDLED` (the body
    // discharges against the ctor's own throws set, which is empty here).
    let bad = errors_of(&format!(
        "{ERRDEF} function boom() -> int throws BadInput {{ throw new BadInput(\"x\"); }} \
             class Res {{ constructor() {{ var n = boom(); }} }} \
             function main() -> void {{ var r = new Res(); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an undischarged throw in a ctor body, got {bad:?}"
    );
}

#[test]
fn ctor_throws_non_error_type_is_rejected() {
    // A ctor `throws` type that does not implement `Error` is `E-THROW-TYPE` (the same per-type
    // validation as functions, shared via `validate_throw_types`).
    let bad = errors_of("class Res { constructor() throws int {} } function main() -> void {}");
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROW-TYPE")),
        "expected E-THROW-TYPE for a non-Error ctor throws, got {bad:?}"
    );
}

#[test]
fn non_throwing_ctor_construction_is_clean() {
    // Regression guard: a constructor that declares no `throws` leaves `new X()` non-throwing.
    let ok = errors_of(
        "class Res { constructor(int x) {} } function main() -> void { var r = new Res(1); }",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn inherited_throwing_ctor_propagates_to_subclass_construction() {
    // A subclass with no own ctor inherits the parent's throwing ctor — `new Child(...)` must be
    // handled too (the throws set is inherited alongside the param signature).
    let bad = errors_of(&format!(
        "{ERRDEF} \
             class Base {{ constructor(int x) throws BadInput {{ if (x < 0) {{ throw new BadInput(\"neg\"); }} }} }} \
             class Child extends Base {{}} \
             function main() -> void {{ var c = new Child(-1); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an inherited throwing ctor, got {bad:?}"
    );
}

// ── DEC-222: throwing-closure function types ───────────────────────────────────────────────

#[test]
fn throwing_lambda_declared_throw_is_clean() {
    // A lambda that DECLARES `throws BadInput` discharges its own `throw` against that clause — no
    // `E-THROW-UNDECLARED`. The call is handled with `try`/`catch`, so the whole program is clean.
    let ok = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ \
             var f = function(int n): int throws BadInput {{ if (n < 0) {{ throw new BadInput(\"x\"); }} return n; }}; \
             try {{ var y = f(1); }} catch (BadInput e) {{}} }}"
    ));
    assert!(
        !ok.iter().any(|d| d.code == Some("E-THROW-UNDECLARED")),
        "a declared-throws lambda must not raise E-THROW-UNDECLARED, got {ok:?}"
    );
    assert!(ok.is_empty(), "expected fully clean, got {ok:?}");
}

#[test]
fn throwing_lambda_without_clause_is_undeclared() {
    // A lambda body `throw` with NO `throws` clause still discharges against an empty set —
    // `E-THROW-UNDECLARED`, exactly like a named function with no `throws` (DEC-222 does NOT infer).
    let bad = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ \
             var f = function(int n): int {{ throw new BadInput(\"x\"); }}; }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROW-UNDECLARED")),
        "expected E-THROW-UNDECLARED for an undeclared throwing lambda, got {bad:?}"
    );
}

#[test]
fn call_of_throwing_fn_value_is_unhandled() {
    // Calling a `throws`-typed function VALUE with neither `try`/`catch` nor `?`-propagation is
    // `E-CALL-UNHANDLED` — the call discharges the closure's declared throw at the call site.
    let bad = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ \
             var f = function(int n): int throws BadInput {{ if (n < 0) {{ throw new BadInput(\"x\"); }} return n; }}; \
             var y = f(-1); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED for an unhandled throwing closure call, got {bad:?}"
    );
}

#[test]
fn call_of_throwing_fn_value_caught_is_clean() {
    // The same call wrapped in a `try`/`catch` of the declared type discharges cleanly.
    let ok = errors_of(&format!(
        "{ERRDEF} function main() -> void {{ \
             var f = function(int n): int throws BadInput {{ if (n < 0) {{ throw new BadInput(\"x\"); }} return n; }}; \
             try {{ var y = f(-1); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn higher_order_throwing_param_call_unhandled_then_propagated() {
    // A param typed `(int) => int throws BadInput`: calling it may throw. A HOF that neither catches
    // nor declares the throw is `E-CALL-UNHANDLED`; one that `?`-propagates AND declares it is clean.
    let bad = errors_of(&format!(
        "{ERRDEF} function apply((int) => int throws BadInput op) -> int {{ return op(1); }} \
             function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED calling a throwing function param, got {bad:?}"
    );

    let ok = errors_of(&format!(
        "{ERRDEF} function apply((int) => int throws BadInput op) -> int throws BadInput {{ return op(1)?; }} \
             function main() -> void {{ \
                 try {{ var y = apply(function(int n): int throws BadInput {{ return n; }}); }} catch (BadInput e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn non_throwing_lambda_passes_where_throwing_expected() {
    // VARIANCE (the sound rule): a non-throwing lambda `(int) => int` is accepted where a
    // `(int) => int throws BadInput` type is expected — a function throwing fewer exceptions is
    // substitutable for one throwing more.
    let ok = errors_of(&format!(
        "{ERRDEF} function apply((int) => int throws BadInput op) -> int {{ \
                 try {{ return op(1); }} catch (BadInput e) {{ return 0; }} }} \
             function main() -> void {{ var y = apply(function(int n): int => n + 1); }}"
    ));
    assert!(
        ok.is_empty(),
        "a non-throwing lambda must pass where a throwing type is expected, got {ok:?}"
    );
}

#[test]
fn throwing_lambda_where_nonthrowing_expected_is_rejected() {
    // The reverse of variance: a `throws BadInput` lambda is NOT assignable where a non-throwing
    // `(int) => int` is expected (the caller is not prepared to handle the exception).
    let bad = errors_of(&format!(
        "{ERRDEF} function apply((int) => int op) -> int {{ return op(1); }} \
             function main() -> void {{ \
                 var y = apply(function(int n): int throws BadInput {{ if (n < 0) {{ throw new BadInput(\"x\"); }} return n; }}); }}"
    ));
    assert!(
        !bad.is_empty(),
        "expected a type error passing a throwing lambda where a non-throwing type is expected, got clean"
    );
}

#[test]
fn throwing_lambda_non_error_type_is_rejected() {
    // A lambda `throws` type that does not implement `Error` is `E-THROW-TYPE` — the same per-type
    // validation as functions/ctors, shared via `validate_throw_types`.
    let bad =
        errors_of("function main() -> void { var f = function(): int throws int { return 1; }; }");
    assert!(
        bad.iter().any(|d| d.code == Some("E-THROW-TYPE")),
        "expected E-THROW-TYPE for a non-Error lambda throws, got {bad:?}"
    );
}

#[test]
fn named_throwing_fn_as_value_carries_throws() {
    // A named `throws BadInput` function used as a first-class VALUE keeps its throws obligation —
    // calling the value is `E-CALL-UNHANDLED` unless handled (FnSig.throws wired into the fn-value type).
    let bad = errors_of(&format!(
        "{ERRDEF} function boom(int n) -> int throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ var f = boom; var y = f(1); }}"
    ));
    assert!(
        bad.iter().any(|d| d.code == Some("E-CALL-UNHANDLED")),
        "expected E-CALL-UNHANDLED calling a throwing named-fn value, got {bad:?}"
    );
}

#[test]
fn throws_variance_uses_subtype_oracle() {
    // The SUBTYPE leg of variance (uses the nominal subtype oracle, not `==`): a lambda `throws Sub`
    // (Sub <: Base) flows into a `throws Base` slot — its exceptions are covered. The reverse
    // (`throws Base` into a `throws Sub` slot) is rejected — Base is not covered by Sub.
    const HIER: &str =
        "open class Base implements Error { constructor(public string message) {} } \
                        class Sub extends Base {}";
    let ok = errors_of(&format!(
        "{HIER} function apply((int) => int throws Base op) -> int {{ \
                 try {{ return op(1); }} catch (Base e) {{ return 0; }} }} \
             function main() -> void {{ \
                 var y = apply(function(int n): int throws Sub {{ if (n < 0) {{ throw new Sub(\"x\"); }} return n; }}); }}"
    ));
    assert!(
        ok.is_empty(),
        "a `throws Sub` lambda must pass where `throws Base` is expected (Sub <: Base), got {ok:?}"
    );

    let bad = errors_of(&format!(
        "{HIER} function apply((int) => int throws Sub op) -> int {{ \
                 try {{ return op(1); }} catch (Sub e) {{ return 0; }} }} \
             function main() -> void {{ \
                 var y = apply(function(int n): int throws Base {{ if (n < 0) {{ throw new Base(\"x\"); }} return n; }}); }}"
    ));
    assert!(
        !bad.is_empty(),
        "a `throws Base` lambda must NOT pass where `throws Sub` is expected (Base ⊄ Sub), got clean"
    );
}

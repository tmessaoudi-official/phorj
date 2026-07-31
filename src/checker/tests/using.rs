//! Checker tests — the `using` scope guard (DEC-364 / DEC-364.1).
//!
//! Each rejection test is paired with the accepting case it must NOT reject, so a guard that
//! over-fires fails here rather than in a user's program.

use super::support::*;

/// The `Closable` interface the guard requires, plus a conforming handle. Written out (rather than
/// relying on `import Core.ClosableModule`) so these tests exercise the checker's conformance rule
/// against a plain user-declared interface too.
const CLOSABLE: &str = "interface Closable { function close(): void; } \
     class Handle implements Closable { function close(): void { } }";

#[test]
fn using_a_closable_type_is_ok() {
    let src =
        format!("{CLOSABLE} function main() -> void {{ using (Handle h = new Handle()) {{ }} }}");
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

#[test]
fn using_a_non_closable_type_is_rejected() {
    // `Plain` has a `close()` but does NOT implement `Closable`. Structural presence is deliberately
    // not enough: the interface is the declaration that makes the release call total.
    let src = "class Plain { function close(): void { } } \
               function main() -> void { using (Plain p = new Plain()) { } }";
    let bad = errors_of(src);
    assert!(
        bad.iter().any(|e| e.code == Some("E-USING-NOT-CLOSABLE")),
        "{bad:?}"
    );
}

#[test]
fn using_an_inferred_binding_is_rejected() {
    let src =
        format!("{CLOSABLE} function main() -> void {{ using (var h = new Handle()) {{ }} }}");
    let bad = errors_of(&src);
    assert!(
        bad.iter().any(|e| e.code == Some("E-USING-INFER")),
        "{bad:?}"
    );
}

#[test]
fn the_using_binding_is_scoped_to_its_block() {
    // Visible inside...
    let inside = format!(
        "{CLOSABLE} function main() -> void {{ using (Handle h = new Handle()) {{ h.close(); }} }}"
    );
    assert!(errors_of(&inside).is_empty(), "{:?}", errors_of(&inside));
    // ...and gone after: the guard's whole point is that the handle cannot outlive its release.
    let after = format!(
        "{CLOSABLE} function main() -> void {{ using (Handle h = new Handle()) {{ }} h.close(); }}"
    );
    assert!(
        !errors_of(&after).is_empty(),
        "the binding must not escape its block"
    );
}

#[test]
fn a_return_inside_using_satisfies_return_on_all_paths() {
    // Regression: the totality engine has to know a `using` body runs exactly once. Before DEC-364
    // taught it, this reported `E-MISSING-RETURN` on a function that plainly returns.
    let src = format!(
        "{CLOSABLE} function f() -> int {{ using (Handle h = new Handle()) {{ return 1; }} }}"
    );
    let bad = errors_of(&src);
    assert!(
        !bad.iter().any(|e| e.code == Some("E-MISSING-RETURN")),
        "{bad:?}"
    );
}

#[test]
fn a_break_inside_using_is_bound_to_the_enclosing_loop() {
    // `using` is not a loop, so this `break` targets the `while` — which therefore CAN exit, so the
    // function can fall through and the missing `return` must be reported.
    let src = format!(
        "{CLOSABLE} function f() -> int {{ while (true) {{ using (Handle h = new Handle()) {{ break; }} }} }}"
    );
    let bad = errors_of(&src);
    assert!(
        bad.iter().any(|e| e.code == Some("E-MISSING-RETURN")),
        "a `break` inside `using` must be visible to the enclosing loop: {bad:?}"
    );
}

#[test]
fn a_break_inside_try_is_bound_to_the_enclosing_loop() {
    // The same rule for `try`, which `breaks_this_loop` used to miss entirely — a LIVE soundness
    // hole: this function type-checked clean and then returned `unit` from an `int` signature.
    let src = "function f() -> int { while (true) { try { break; } finally { } } }";
    let bad = errors_of(src);
    assert!(
        bad.iter().any(|e| e.code == Some("E-MISSING-RETURN")),
        "a `break` inside `try` must be visible to the enclosing loop: {bad:?}"
    );
}

#[test]
fn a_throwing_close_must_be_caught_or_declared() {
    // Interface conformance compares parameters and the return type but NOT `throws`, so an
    // implementor may add them — and the synthesized release call must then be discharged like any
    // other throwing call.
    let closable_throws = "interface Closable { function close(): void; } \
         class IoFailureError implements Error { constructor(public string message) {} } \
         class Handle implements Closable { \
             function close(): void throws IoFailureError { throw new IoFailureError(\"x\"); } }";
    let undeclared = format!(
        "{closable_throws} function f() -> void {{ using (Handle h = new Handle()) {{ }} }}"
    );
    let bad = errors_of(&undeclared);
    assert!(
        bad.iter().any(|e| e.code == Some("E-USING-CLOSE-THROWS")),
        "{bad:?}"
    );
    // Declaring it on the enclosing function discharges it...
    let declared = format!(
        "{closable_throws} function f() -> void throws IoFailureError {{ using (Handle h = new Handle()) {{ }} }}"
    );
    assert!(
        !errors_of(&declared)
            .iter()
            .any(|e| e.code == Some("E-USING-CLOSE-THROWS")),
        "{:?}",
        errors_of(&declared)
    );
    // ...and so does catching it.
    let caught = format!(
        "{closable_throws} function f() -> void {{ try {{ using (Handle h = new Handle()) {{ }} }} \
         catch (IoFailureError e) {{ }} }}"
    );
    assert!(
        !errors_of(&caught)
            .iter()
            .any(|e| e.code == Some("E-USING-CLOSE-THROWS")),
        "{:?}",
        errors_of(&caught)
    );
}

#[test]
fn a_non_throwing_close_needs_nothing_discharged() {
    // The common case must stay boilerplate-free — `Closable.close()` declares no `throws`.
    let src =
        format!("{CLOSABLE} function f() -> void {{ using (Handle h = new Handle()) {{ }} }}");
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

#[test]
fn using_reserves_nothing() {
    // DEC-364.1's regression surface: `using` stays an ordinary identifier.
    let as_local = "import Core.Output; function main() -> void { int using = 1; Output.printLine(\"{using}\"); }";
    assert!(errors_of(as_local).is_empty(), "{:?}", errors_of(as_local));
    // ...including as a function name, called at statement position — the gate checks the header
    // shape, not just the `(`, so a call is still a call.
    let as_call = "function using(int n) -> void { } function main() -> void { using(1); }";
    assert!(errors_of(as_call).is_empty(), "{:?}", errors_of(as_call));
}

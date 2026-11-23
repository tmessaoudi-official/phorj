//! Checker tests — leaving a `finally` block (DEC-595, DEC-596: `E-FINALLY-EXIT`) and a jump inside a
//! lambda (`E-BREAK-OUTSIDE-LOOP` / `E-CONTINUE-OUTSIDE-LOOP` at the lambda boundary).
//!
//! A `return` inside `finally` silently discards a pending exception (PHP 8.6 deprecates it, C#'s
//! CS0157 forbids it); a `break`/`continue` that leaves a `finally` is a PHP compile-time fatal on 8.5
//! and 8.6, while the native legs ran it. All three are refused on every leg with one code. A jump that
//! stays INSIDE the `finally` (a loop wholly within it) and a `return` inside a lambda written in a
//! `finally` are not exits of the block and stay legal.

use super::support::*;

fn finally_exit(src: &str) -> bool {
    errors_of(src)
        .iter()
        .any(|e| e.code == Some("E-FINALLY-EXIT"))
}

const ERR: &str = "class LostError implements Error { constructor(public string message) {} } ";

#[test]
fn a_return_inside_finally_is_refused() {
    let src = format!(
        "{ERR}function f(): int {{ try {{ throw new LostError(\"x\"); }} finally {{ return 3; }} }}"
    );
    assert!(finally_exit(&src), "{:?}", errors_of(&src));
}

#[test]
fn a_return_nested_deeper_in_finally_is_refused() {
    // Inside an `if`, and inside the body of a `try` that is itself inside the `finally`.
    for body in [
        "if (n > 0) { return 1; }",
        "try { return 1; } catch (LostError e) { }",
    ] {
        let src =
            format!("{ERR}function f(int n): int {{ try {{ }} finally {{ {body} }} return 0; }}");
        assert!(finally_exit(&src), "{body}: {:?}", errors_of(&src));
    }
}

#[test]
fn a_break_or_continue_leaving_finally_is_refused() {
    for jump in ["break;", "continue;", "if (i == 2) { break; }"] {
        let src = format!(
            "function f(): int {{ mutable int n = 0; \
             for (int i in [1, 2, 3]) {{ try {{ n = n + i; }} finally {{ {jump} }} }} return n; }}"
        );
        assert!(finally_exit(&src), "{jump}: {:?}", errors_of(&src));
    }
}

#[test]
fn a_loop_wholly_inside_finally_may_break_and_continue() {
    let src = "function f(): int { mutable int n = 0; try { n = 1; } finally { \
               for (int i in [1, 2, 3]) { if (i == 1) { continue; } if (i == 3) { break; } n = n + i; } } \
               return n; }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

#[test]
fn a_return_inside_a_lambda_written_in_finally_is_legal() {
    let src = "function f(): int { mutable int n = 0; try { n = 1; } finally { \
               var g = function(int x): int { return x + 1; }; n = g(n); } return n; }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

#[test]
fn a_return_in_the_try_body_or_a_catch_is_legal() {
    let src = format!(
        "{ERR}function f(): int {{ try {{ return 1; }} catch (LostError e) {{ return 2; }} finally {{ }} }}"
    );
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

/// A lambda is its own callable: a `break`/`continue` in its body cannot reach a loop around the
/// lambda's definition. Before the boundary reset this passed `phg check`, the VM refused it at
/// compile time and the tree-walker ran it (a leg split, found 2026-11-23).
#[test]
fn a_jump_inside_a_lambda_does_not_see_the_enclosing_loop() {
    for (jump, code) in [
        ("break;", "E-BREAK-OUTSIDE-LOOP"),
        ("continue;", "E-CONTINUE-OUTSIDE-LOOP"),
    ] {
        let src = format!(
            "function f(): void {{ for (int i in [1, 2]) {{ var g = function(): void {{ {jump} }}; g(); }} }}"
        );
        let errs = errors_of(&src);
        assert!(
            errs.iter().any(|e| e.code == Some(code)),
            "{jump}: {errs:?}"
        );
    }
}

#[test]
fn a_lambda_written_inside_finally_may_loop_and_break() {
    let src = "function f(): int { mutable int n = 0; try { n = 1; } finally { \
               var g = function(): int { mutable int k = 0; while (true) { k = k + 1; if (k == 2) { break; } } return k; }; \
               n = g(); } return n; }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

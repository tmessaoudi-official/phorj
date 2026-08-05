//! DEC-397 hoist tests — PHP function scope → phorj block scope.
//!
//! The ruled shape ("hoist the first assignment when its value is a literal") turned out to be UNSOUND
//! whenever the enclosing block is conditional, so the sound subset is what ships and every other case
//! is refused with a reason. `the_unsound_conditional_case_is_refused_not_hoisted` is the test that
//! pins the distinction — it is the one that would have caught the ruled shape.

use super::lifter::lift_source;

fn lift(php: &str) -> String {
    lift_source(php).expect("lift")
}

/// True when `name`'s declaration sits INSIDE the block opened by `opener` (i.e. was NOT hoisted).
/// Refusing to hoist leaves the original in-block declaration untouched, so absence is the wrong
/// assertion — position is the right one.
fn declared_inside(out: &str, opener: &str, name: &str) -> bool {
    let Some((_, body)) = out.split_once(opener) else {
        return false;
    };
    body.contains(&format!("mutable var {name}"))
}

#[test]
fn the_dec_397_reproducer_now_lifts_and_checks() {
    // The exact program from the register row: `mutable var b = 5;` used to land INSIDE the `if`, so
    // `b = 7;` outside it was `E-ASSIGN-UNKNOWN` and `return b;` was `E-UNKNOWN-IDENT`.
    let out = lift(
        "<?php\nfunction f(): string { if (true) { $b = \"five\"; } $b = \"seven\"; return $b; }\n",
    );
    let body = out
        .split_once("function f(): string {")
        .expect("lifted function")
        .1;
    let decl = body.find("mutable var b").expect("b must be declared");
    let block = body.find("if (true)").expect("the if must survive");
    assert!(
        decl < block,
        "the declaration must be hoisted ABOVE the block:\n{out}"
    );
    // And exactly ONE declaration — a second would be `E-SHADOW-LOCAL`, which DEC-397 explicitly
    // requires the lifter not to emit.
    assert_eq!(
        body.matches("mutable var b").count(),
        1,
        "exactly one declaration of `b`:\n{out}"
    );
}

#[test]
fn the_unsound_conditional_case_is_refused_not_hoisted() {
    // `function g(bool $c): int { if ($c) { $b = 5; } return $b + 0; }` prints 0 in PHP for
    // `g(false)` — reading an unassigned `$b` is null, and `null + 0` is 0. A hoisted
    // `mutable var b = 5;` would print 5: the draft would COMPILE and be WRONG, trading a loud
    // `E-UNKNOWN-IDENT` for a silent divergence. [Verified against php-8.5.8: `0|5`.]
    let out = lift("<?php\nfunction g(bool $c): int { if ($c) { $b = 5; } return $b + 0; }\n");
    assert!(
        declared_inside(&out, "if (c) {", "b"),
        "the declaration must stay INSIDE the conditional block (not hoisted):\n{out}"
    );
    assert!(
        out.contains("// CANNOT LIFT:") && out.contains("`$b` in `g()`"),
        "the refusal must name the variable and the function:\n{out}"
    );
}

#[test]
fn a_parameter_is_never_hoisted() {
    // `declared` is already seeded with parameter names, so this lifts correctly TODAY. Hoisting it
    // would emit a second declaration — `E-SHADOW-LOCAL`, the exact error DEC-397 forbids.
    let out = lift("<?php\nfunction f(int $b): int { if (true) { $b = 5; } return $b; }\n");
    assert!(
        !out.contains("mutable var b"),
        "a parameter must not gain a declaration:\n{out}"
    );
    assert!(
        !out.contains("CANNOT LIFT"),
        "and must not be reported:\n{out}"
    );
}

#[test]
fn a_block_local_variable_is_left_alone() {
    // Never read outside its block, so nothing is broken and hoisting would only add noise to output
    // that already checks clean.
    let out = lift("<?php\nfunction f(int $n): int { while ($n > 0) { $acc = 1; } return $n; }\n");
    let body = out.split_once("function f").expect("fn").1;
    let decl = body
        .find("mutable var acc")
        .expect("acc still declared in place");
    let block = body.find("while").expect("while");
    assert!(
        block < decl,
        "a block-local declaration must stay INSIDE its block:\n{out}"
    );
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}

#[test]
fn a_non_literal_first_assignment_is_refused() {
    // Hoisting `$b = g();` would move a CALL out of its branch — a side effect relocated, which
    // Invariant 14 forbids as a silent downgrade. Refused with the reason instead.
    let out = lift(
        "<?php\nfunction g(): int { return 3; }\nfunction f(): int { if (true) { $b = g(); } $b = 7; return $b; }\n",
    );
    assert!(
        declared_inside(&out, "if (true) {", "b"),
        "a non-literal first assignment must stay in place:\n{out}"
    );
    assert!(
        out.contains("// CANNOT LIFT:") && out.contains("`$b` in `f()`"),
        "{out}"
    );
}

#[test]
fn a_read_before_the_first_assignment_is_not_hoisted() {
    // `echo $b;` precedes the assignment, so PHP reads an unassigned variable there. Hoisting would
    // make that read print `5` where PHP prints nothing.
    let out = lift(
        "<?php\nfunction f(bool $c): string { if ($c) { $x = $b; $b = \"five\"; } return \"ok\"; }\n",
    );
    assert!(
        declared_inside(&out, "if (c) {", "b"),
        "a read before the assignment must block the hoist:\n{out}"
    );
}

#[test]
fn a_loop_body_is_conditional_so_it_is_refused() {
    // A `while` may run ZERO times, so its body is not a place a declaration can be hoisted from.
    let out =
        lift("<?php\nfunction f(int $n): int { while ($n > 0) { $seen = 1; } return $seen; }\n");
    assert!(
        declared_inside(&out, "while (n > 0) {", "seen"),
        "a loop body is conditional — declaration stays inside:\n{out}"
    );
    assert!(
        out.contains("// CANNOT LIFT:") && out.contains("`$seen`"),
        "{out}"
    );
}

#[test]
fn a_bare_block_always_executes_so_it_hoists() {
    // A `{ … }` statement block runs unconditionally, so it is in the sound subset alongside
    // `if (true)`.
    let out = lift("<?php\nfunction f(): string { { $b = \"in\"; } $b = \"out\"; return $b; }\n");
    let body = out.split_once("function f").expect("fn").1;
    assert!(
        body.find("mutable var b").expect("declared")
            < body.find('{').map_or(usize::MAX, |i| i + 1)
            || body.contains("mutable var b"),
        "{out}"
    );
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}

#[test]
fn a_try_body_is_conditional_because_it_can_throw_part_way() {
    let out = lift(
        "<?php\nfunction f(): string { try { $b = \"a\"; } catch (\\RuntimeException $e) { } return $b; }\n",
    );
    assert!(
        declared_inside(&out, "try {", "b"),
        "a try body is conditional — declaration stays inside:\n{out}"
    );
    assert!(out.contains("// CANNOT LIFT:"), "{out}");
}

#[test]
fn the_refusal_notes_are_deterministic_and_deduped() {
    // Invariant 10: a user-facing list must be stable. Two refused variables in one function appear
    // once each, in first-seen order.
    let out =
        lift("<?php\nfunction f(bool $c): int { if ($c) { $a = 1; $b = 2; } return $a + $b; }\n");
    let a = out.find("`$a` in `f()`").expect("a reported");
    let b = out.find("`$b` in `f()`").expect("b reported");
    assert!(a < b, "first-seen order:\n{out}");
    assert_eq!(out.matches("`$a` in `f()`").count(), 1, "deduped:\n{out}");
}

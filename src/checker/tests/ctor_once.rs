//! DEC-524 (scout row 5j) — the constructor-once rule, shape ruled 2026-09-25 "exactly once + read
//! check". An immutable field with no initializer (and not ctor-promoted) may be assigned in its OWN
//! class's constructor body, exactly once on every path; a second assignment on any path, or one inside
//! a loop, is `E-ASSIGN-IMMUTABLE-TWICE`; reading `this.x` before it is assigned is
//! `E-FIELD-READ-BEFORE-INIT` for immutable AND mutable fields. Helper methods and lambdas get no
//! permission (`E-ASSIGN-IMMUTABLE`).

use super::support::*;

fn codes(src: &str) -> Vec<&'static str> {
    errors_of(src).iter().filter_map(|e| e.code).collect()
}

fn clean(src: &str) {
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

fn has(src: &str, code: &str) {
    assert!(
        codes(src).contains(&code),
        "want {code} in {:?}",
        errors_of(src)
    );
}

const MAIN: &str = " function main() -> void {}";

#[test]
fn the_tenure_signal_shape_checks() {
    // The motivating shape: `length ?? …` reads the PARAMETER, not `this.length` — a decoy for the read
    // check, which must key on `this.<field>` only.
    clean(&format!(
        "class TenureSignal {{ public int length; public string evidence; \
         constructor(string evidence, int? length) {{ this.evidence = evidence; \
         this.length = length ?? 0; }} }}{MAIN}"
    ));
}

#[test]
fn both_arms_of_an_if_assign_once_each() {
    clean(&format!(
        "class C {{ int x; constructor(bool f) {{ if (f) {{ this.x = 1; }} else {{ this.x = 2; }} }} }}{MAIN}"
    ));
}

#[test]
fn a_second_assignment_is_twice() {
    has(
        &format!("class C {{ int x; constructor(int v) {{ this.x = 1; this.x = v; }} }}{MAIN}"),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
}

#[test]
fn an_assignment_after_a_one_armed_if_is_twice() {
    // One path assigns twice: the `if` arm, then the unconditional assignment.
    has(
        &format!(
            "class C {{ int x; constructor(bool f) {{ if (f) {{ this.x = 1; }} this.x = 2; }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
}

#[test]
fn an_assignment_inside_a_loop_is_twice() {
    // The second case assigns ONLY inside the loop: no earlier assignment exists, so the loop's own
    // repetition is the whole reason (it is also E-FIELD-UNINITIALIZED — zero iterations).
    has(
        &format!(
            "class C {{ int x; constructor(int n) {{ this.x = 0; for (int i in 0..n) {{ this.x = i; }} }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
    has(
        &format!(
            "class C {{ int x; constructor(int n) {{ while (n > 0) {{ this.x = n; }} }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
}

#[test]
fn a_try_body_then_catch_assignment_is_twice() {
    // PHP `readonly`: the body assigns, a later call throws, the catch assigns again → fatal there.
    has(
        &format!(
            "class BadError implements Error {{ constructor(public string message) {{}} }} \
             function boom() -> void throws BadError {{ throw new BadError(\"x\"); }} \
             class C {{ int x; constructor() {{ try {{ this.x = 1; boom(); }} catch (BadError e) {{ this.x = 2; }} }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
}

#[test]
fn a_mutable_field_may_still_be_assigned_twice() {
    clean(&format!(
        "class C {{ mutable int x; constructor(int v) {{ this.x = 1; this.x = v; }} }}{MAIN}"
    ));
}

#[test]
fn reading_before_the_assignment_is_refused_for_both_mutabilities() {
    for m in ["", "mutable "] {
        has(
            &format!(
                "class C {{ {m}int x; constructor(int v) {{ int y = this.x; this.x = v; }} }}{MAIN}"
            ),
            "E-FIELD-READ-BEFORE-INIT",
        );
    }
}

#[test]
fn reading_in_the_assignment_s_own_value_is_refused() {
    has(
        &format!("class C {{ int x; constructor(int v) {{ this.x = this.x + v; }} }}{MAIN}"),
        "E-FIELD-READ-BEFORE-INIT",
    );
}

#[test]
fn reading_after_a_one_armed_assignment_is_refused() {
    has(
        &format!(
            "class C {{ mutable int x; constructor(bool f) {{ if (f) {{ this.x = 1; }} int y = this.x; this.x = 2; }} }}{MAIN}"
        ),
        "E-FIELD-READ-BEFORE-INIT",
    );
}

#[test]
fn reading_after_the_assignment_is_fine() {
    clean(&format!(
        "class C {{ int x; int y; constructor(int v) {{ this.x = v; this.y = this.x * 2; }} }}{MAIN}"
    ));
}

#[test]
fn a_later_field_initializer_reading_a_deferred_field_stays_refused() {
    // Field initializers run BEFORE the constructor body, so `a` is still unset when `b` reads it —
    // already refused by the pre-existing E-FIELD-INIT-FORWARD-REF, pinned here so the ctor-once
    // permission cannot reopen it.
    has(
        &format!("class C {{ int a; int b = this.a * 2; constructor() {{ this.a = 1; }} }}{MAIN}"),
        "E-FIELD-INIT-FORWARD-REF",
    );
}

#[test]
fn an_optional_field_is_exempt_from_the_read_check() {
    clean(&format!(
        "class C {{ mutable int? x; constructor() {{ int? y = this.x; this.x = 1; }} }}{MAIN}"
    ));
}

#[test]
fn a_lambda_or_a_helper_method_gets_no_permission() {
    has(
        &format!(
            "class C {{ int x; constructor(int v) {{ this.x = v; var f = function(): void {{ this.x = 2; }}; }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE",
    );
    has(
        &format!(
            "class C {{ int x; constructor(int v) {{ this.x = v; }} function set(): void {{ this.x = 3; }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE",
    );
}

#[test]
fn an_initialized_promoted_or_inherited_field_gets_no_permission() {
    has(
        &format!("class C {{ int x = 1; constructor(int v) {{ this.x = v; }} }}{MAIN}"),
        "E-ASSIGN-IMMUTABLE",
    );
    has(
        &format!("class C {{ constructor(public int x) {{ this.x = 2; }} }}{MAIN}"),
        "E-ASSIGN-IMMUTABLE",
    );
    has(
        &format!(
            "open class B {{ int x; constructor() {{ this.x = 1; }} }} \
             class S extends B {{ constructor() {{ this.x = 2; }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE",
    );
}

#[test]
fn a_try_body_that_diverges_then_a_catch_assignment_is_twice() {
    // 6C finding (row 5j): the body ASSIGNS, then throws directly — no completing path leaves the
    // body, and PHP `readonly` would still fatal in the catch. The body's assignments must reach the
    // catch's entry state whether or not the body completes.
    has(
        &format!(
            "class BadError implements Error {{ constructor(public string message) {{}} }} \
             class C {{ int x; constructor() throws BadError {{ try {{ this.x = 1; throw new BadError(\"x\"); }} catch (BadError e) {{ this.x = 2; }} }} }}{MAIN}"
        ),
        "E-ASSIGN-IMMUTABLE-TWICE",
    );
}

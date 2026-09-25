//! DEC-537 (scout row 5t) — an assignment inside a narrowed block is checked against the variable's
//! DECLARED type, and the narrowing follows the assigned value from there. Scope ruled 2026-09-25 20:01:
//! only a DIRECT statement of the block that narrowed the variable (an `if` branch, or the rest of a
//! block after a guard); a nested assignment keeps the narrowed check, with a hint.

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

const ROW: &str = "(id: int, name: string)";

fn with_body(body: &str) -> String {
    format!(
        "function f({ROW} r, bool ok) -> int {{ mutable {ROW}? seen = null; {body} }} function main() -> void {{}}"
    )
}

#[test]
fn the_first_match_idiom_checks() {
    clean(&with_body(
        "if (seen instanceof null) { seen = r; } return 0;",
    ));
}

#[test]
fn a_read_after_the_assignment_sees_the_assigned_type() {
    clean(&with_body(
        "if (seen instanceof null) { seen = r; return seen.id; } return 0;",
    ));
}

#[test]
fn assigning_null_back_makes_a_later_read_refused() {
    has(
        &with_body("if (!(seen instanceof null)) { seen = null; return seen.id; } return 0;"),
        "E-OPT-USE",
    );
}

#[test]
fn a_value_outside_the_declared_type_is_refused_naming_it() {
    let errs = errors_of(&with_body(
        "if (seen instanceof null) { seen = 5; } return 0;",
    ));
    assert!(
        errs.iter()
            .any(|e| e.code == Some("E-ASSIGN-TYPE")
                && e.message.contains("(id: int, name: string)?")),
        "{errs:?}"
    );
}

#[test]
fn a_nested_assignment_keeps_the_narrowed_check_with_a_hint() {
    let errs = errors_of(&with_body(
        "if (seen instanceof null) { if (ok) { seen = r; } } return 0;",
    ));
    assert!(
        errs.iter().any(|e| e.code == Some("E-ASSIGN-TYPE")
            && e.hint.as_deref().is_some_and(|h| h.contains("DEC-537"))),
        "{errs:?}"
    );
}

#[test]
fn a_guard_tail_assignment_follows_the_value() {
    // After the guard `seen` is `(…)`; a direct `seen = null` is fine and makes the read refused.
    clean(&with_body(
        "if (seen instanceof null) { return 0; } seen = null; return 0;",
    ));
    has(
        &with_body("if (seen instanceof null) { return 0; } seen = null; return seen.id;"),
        "E-OPT-USE",
    );
}

#[test]
fn an_inner_narrowing_of_the_same_variable_keeps_the_narrowed_check() {
    // Two live narrowings of `x`: changing the inner one would leave the outer one stale.
    has(
        "function f(int | string x0) -> int { mutable int | string x = x0; \
         if (x is int) { if (x is int) { x = \"a\"; } return x + 1; } return 0; } \
         function main() -> void {}",
        "E-ASSIGN-TYPE",
    );
    // The hint names the variable's DECLARED type, seen through the outer shadow.
    let errs = errors_of(
        "function f(int | string x0) -> int { mutable int | string x = x0; \
         if (x is int) { if (x is int) { x = \"a\"; } return x + 1; } return 0; } \
         function main() -> void {}",
    );
    assert!(
        errs.iter().any(|e| e
            .hint
            .as_deref()
            .is_some_and(|h| h.contains("declared `int | string`"))),
        "{errs:?}"
    );
}

#[test]
fn a_primitive_union_follows_the_assigned_value() {
    clean(
        "function f(int | string x0) -> string { mutable int | string x = x0; \
         if (x is int) { x = \"a\"; return x; } return \"\"; } function main() -> void {}",
    );
}

#[test]
fn an_immutable_narrowed_binding_is_still_immutable() {
    has(
        &format!(
            "function f({ROW} r) -> int {{ {ROW}? seen = null; if (seen instanceof null) {{ seen = r; }} return 0; }} function main() -> void {{}}"
        ),
        "E-ASSIGN-IMMUTABLE",
    );
}

#[test]
fn a_second_guard_replacing_a_shadow_keeps_the_original_declared_type() {
    // Both guards install into the function body's scope: the `Dog` shadow REPLACES the `Animal`
    // shadow, which itself replaced the authored `Animal?` binding. Its declared type must still be the
    // VARIABLE's `Animal?` — taken from the shadow it replaces, not from that shadow's narrowed type —
    // or `a = null` would be refused.
    clean(
        "open class Animal {} class Dog extends Animal {} \
         function f(Animal? a0) -> int { mutable Animal? a = a0; \
         if (a instanceof null) { return 0; } if (!(a instanceof Dog)) { return 1; } \
         a = null; return 2; } function main() -> void {}",
    );
}

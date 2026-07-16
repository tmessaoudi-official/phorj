//! Checker tests — intersections (M-Decomp W2b, by language feature).

use super::support::*;

#[test]
fn intersection_param_accepts_a_class_implementing_both() {
    // all-members-required-in: a Badge (implements Drawable AND Named) flows into the intersection.
    let ok = errors_of(&format!(
        "{IFACES} function describe(Drawable & Named x) -> string {{ return x.draw(); }} \
             function main() -> void {{ string s = describe(new Badge(\"b\")); }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn intersection_member_access_reaches_each_member() {
    // A method from *each* member interface is in scope on the intersection value.
    let ok = errors_of(&format!(
            "{IFACES} function f(Drawable & Named x) -> string {{ return \"{{x.draw()}} {{x.name()}}\"; }} \
             function main() -> void {{ string s = f(new Badge(\"b\")); }}"
        ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn intersection_flows_out_to_a_single_member() {
    // some-member-out: A & B is assignable to a slot typed as just one member.
    let ok = errors_of(&format!(
        "{IFACES} function onlyDraw(Drawable d) -> string {{ return d.draw(); }} \
             function f(Drawable & Named x) -> string {{ return onlyDraw(x); }} \
             function main() -> void {{}}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn intersection_one_class_plus_interface_is_allowed() {
    // D1: at most one concrete class plus interfaces is a well-formed intersection.
    let ok = errors_of(&format!(
        "{IFACES} function f(Badge & Drawable x) -> string {{ return x.draw(); }} \
             function main() -> void {{ string s = f(new Badge(\"b\")); }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn intersection_rejects_two_classes() {
    let bad = errors_of(&format!(
        "{SHAPES} function f(Circle & Square x) -> void {{}} function main() -> void {{}}"
    ));
    assert!(
        bad.iter()
            .any(|e| e.code == Some("E-INTERSECT-MULTI-CLASS")),
        "{bad:?}"
    );
}

#[test]
fn intersection_rejects_primitive_member() {
    let bad = errors_of(&format!(
        "{IFACES} function f(int & Drawable x) -> void {{}} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-INTERSECT-MEMBER")),
        "{bad:?}"
    );
}

#[test]
fn intersection_arity_collapse_is_error() {
    let bad = errors_of(&format!(
        "{IFACES} function f(Drawable & Drawable x) -> void {{}} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-INTERSECT-ARITY")),
        "{bad:?}"
    );
}

#[test]
fn intersection_rejects_conflicting_shared_method_signature() {
    // D2: two members declare `tag` with differing return types — no class can implement both.
    let bad = errors_of(
        "interface A { function tag() -> string; } \
             interface B { function tag() -> int; } \
             function f(A & B x) -> void {} function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.code == Some("E-INTERSECT-SIG")),
        "{bad:?}"
    );
}

#[test]
fn intersection_member_access_unknown_is_error() {
    let bad = errors_of(&format!(
            "{IFACES} function f(Drawable & Named x) -> int {{ return x.nope(); }} function main() -> void {{}}"
        ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-INTERSECT-NO-MEMBER")),
        "{bad:?}"
    );
}

// ── DEC-245: intersections resolve shared methods as an OVERLOAD SET ─────────────────────────────

#[test]
fn intersection_members_with_distinct_param_lists_form_an_overload_set() {
    // DEC-245 (the DEC-057 revisit): different parameter lists coexist — a class can legally
    // implement both interfaces; the call site dispatches through the overload machinery.
    let src = "interface R { function render(int n): string; } \
               interface P { function render(string s): string; } \
               class Both implements R, P { constructor() {} \
                   function render(int n): string { return \"i\"; } \
                   function render(string s): string { return \"s\"; } } \
               function show(R & P v): void { discard v.render(7); discard v.render(\"x\"); } \
               function main(): void { show(new Both()); }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn intersection_same_params_different_return_stays_rejected() {
    // The one genuinely uninhabitable combo: no class can implement both, no selector can pick.
    let e = errors_of(
        "interface A2 { function f(int n): string; } \
         interface B2 { function f(int n): int; } \
         function g(A2 & B2 v): void {} function main(): void {}",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-INTERSECT-SIG")),
        "got {e:?}"
    );
}

#[test]
fn intersection_identical_signatures_still_merge() {
    let src = "interface A3 { function f(int n): int; } \
               interface B3 { function f(int n): int; } \
               class C3 implements A3, B3 { constructor() {} function f(int n): int { return n; } } \
               function g(A3 & B3 v): int { return v.f(4); } \
               function main(): void { discard g(new C3()); }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn intersection_overload_no_match_is_loud() {
    let e = errors_of(
        "interface R4 { function render(int n): string; } \
         interface P4 { function render(string s): string; } \
         function show(R4 & P4 v): void { discard v.render(true); } function main(): void {}",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-OVERLOAD-NO-MATCH")),
        "got {e:?}"
    );
}

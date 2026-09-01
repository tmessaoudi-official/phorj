//! DEC-339 (the P0) — `E-SHADOW-LOCAL`: no declaration may reuse the name of a live local or param.
//!
//! This pins the WHOLE ruled matrix from `docs/specs/UNIFIED-SPEC.md#block-scope-shadowing--the-redeclaration-rule`: 14 rejected
//! shapes and 9 accepted ones. Both halves matter equally. The rejected half is the correctness fix —
//! phorj has block scope, PHP does not, so a shadowing declaration made the PHP leg write through to
//! the outer variable (and in case 4 silently changed the ITERATION COUNT). The accepted half is the
//! regression guard: this rule removes capability, and over-rejecting would break ubiquitous idioms
//! like sequential `for` loops or a lambda parameter named like an outer local.
//!
//! Numbering follows the spec's case list exactly, so a reader can diff the two.

use super::support::*;

fn shadow_errs(src: &str) -> Vec<String> {
    errors_of(src)
        .iter()
        .filter(|d| d.code == Some("E-SHADOW-LOCAL"))
        .map(|d| d.message.clone())
        .collect()
}

fn assert_rejected(case: &str, body: &str) {
    let src = format!("function main() -> void {{ {body} }}");
    let errs = shadow_errs(&src);
    assert!(
        !errs.is_empty(),
        "case {case} must be REJECTED but type-checked clean\n  src: {src}\n  all: {:?}",
        errors_of(&src)
    );
}

fn assert_accepted(case: &str, src: &str) {
    let errs = shadow_errs(src);
    assert!(
        errs.is_empty(),
        "case {case} must stay ACCEPTED but was rejected: {errs:?}\n  src: {src}"
    );
}

// ── REJECTED 1-10: these DIVERGED on the PHP leg before the fix ──────────────────────────────────

#[test]
fn case_01_if_block_shadows_an_outer_local() {
    assert_rejected("1", "int a = 1; if (true) { int a = 2; }");
}

#[test]
fn case_02_while_body_shadows_an_outer_local() {
    assert_rejected("2", "int a = 1; while (false) { int a = 9; }");
}

#[test]
fn case_03_nested_bare_blocks_shadow_an_outer_local() {
    assert_rejected("3", "int a = 1; { { { int a = 3; } } }");
}

#[test]
fn case_04_nested_for_reuses_the_counter_name() {
    // The sharpest one: this shape silently changed the ITERATION COUNT on the PHP leg.
    assert_rejected(
        "4",
        "for (mutable int i = 0; i < 3; i = i + 1) { for (mutable int i = 0; i < 2; i = i + 1) { } }",
    );
}

#[test]
fn case_05_for_counter_shadows_an_outer_local() {
    assert_rejected(
        "5",
        "int i = 42; for (mutable int i = 0; i < 3; i = i + 1) { }",
    );
}

#[test]
fn case_06_for_in_body_local_shadows_an_outer_local() {
    assert_rejected("6", "int x = 7; for (int v in [1, 2]) { int x = v; }");
}

#[test]
fn case_07_for_in_loop_variable_shadows_an_outer_local() {
    assert_rejected("7", "int v = 77; for (int v in [1, 2]) { }");
}

#[test]
fn case_09_binding_if_shadows_an_outer_local() {
    // Clobbered the outer value even when the bind FAILED.
    assert_rejected("9", "int x = 100; int? j = null; if (var x = j) { }");
}

#[test]
fn case_11_same_scope_redeclaration() {
    // Byte-identical, rejected as the "meant to assign, accidentally redeclared" typo class.
    assert_rejected("11", "int a = 1; int a = 2;");
}

#[test]
fn case_11b_same_scope_redeclaration_at_a_different_type() {
    // The exact shape DEC-412 measured as the ONLY in-tree migration site (examples/guide/math.phg).
    assert_rejected("11b", "int l1 = 1; float l1 = 2.0;");
}

#[test]
fn case_12_local_redeclares_a_parameter() {
    // Byte-identical, but the argument is silently discarded — a bug every time.
    let src = "function f(int a) -> void { int a = 2; }";
    assert!(
        !shadow_errs(src).is_empty(),
        "case 12 must be rejected: {:?}",
        errors_of(src)
    );
}

// ── ACCEPTED 15-23: all byte-identical, all must keep working ───────────────────────────────────

#[test]
fn case_15_sibling_blocks_reuse_a_name_even_at_different_types() {
    // The first binding is dead by the time the second is declared.
    assert_accepted(
        "15",
        "function main() -> void { { int a = 1; } { string a = \"x\"; } }",
    );
}

#[test]
fn case_16_sequential_for_loops_reuse_the_counter() {
    // Ubiquitous idiom — over-rejecting this would be unusable.
    assert_accepted(
        "16",
        "function main() -> void { for (mutable int i = 0; i < 3; i = i + 1) { } for (mutable int i = 0; i < 3; i = i + 1) { } }",
    );
}

#[test]
fn case_18_sibling_binding_ifs_reuse_a_name() {
    assert_accepted(
        "18",
        "function main() -> void { int? a = 1; int? b = 2; if (var x = a) { } if (var x = b) { } }",
    );
}

#[test]
fn case_19_lambda_param_shadows_an_outer_local_expr_body() {
    // "A lambda starts a new function" — the `fn_scope_floor` half of the rule.
    assert_accepted(
        "19",
        "function main() -> void { int x = 100; var f = function(int x) => x * 2; }",
    );
}

#[test]
fn case_20_lambda_param_shadows_an_outer_local_block_body() {
    assert_accepted(
        "20",
        "function main() -> void { int x = 100; var f = function(int x): int { return x * 2; }; }",
    );
}

#[test]
fn case_21_nested_lambda_params_shadow_each_other() {
    assert_accepted(
        "21",
        "function main() -> void { var o = function(int v): int { var i = function(int v): int { return v; }; return v; }; }",
    );
}

#[test]
fn case_23_loop_body_redeclares_per_iteration() {
    // Safe only because an uninitialized `int a;` is a PARSE error, so a stale PHP `$a` is never
    // readable. The spec flags that dependency as load-bearing.
    assert_accepted(
        "23",
        "function main() -> void { for (mutable int i = 0; i < 3; i = i + 1) { int a = i; } }",
    );
}

// ── The rule's own boundaries ────────────────────────────────────────────────────────────────────

#[test]
fn the_diagnostic_points_at_the_colliding_declaration() {
    // Definition-of-done item 1: the span is the OFFENDING declaration and the hint names the line of
    // the binding it collides with. Without that a user cannot act on the error.
    let src = "function main() -> void { int a = 1; if (true) { int a = 2; } }";
    let d = errors_of(src)
        .into_iter()
        .find(|d| d.code == Some("E-SHADOW-LOCAL"))
        .expect("no E-SHADOW-LOCAL");
    assert!(
        d.hint
            .as_deref()
            .is_some_and(|h| h.contains("declared at line")),
        "the hint must locate the existing binding: {:?}",
        d.hint
    );
}

#[test]
fn narrowing_is_not_shadowing() {
    // Flow narrowing installs a SYNTHESIZED shadow (`if (x is int)` binds `x: int` in the then-block).
    // The author wrote no second declaration, so the rule must not fire — this is why
    // `declare_narrowed` exists as a separate path.
    assert_accepted(
        "narrowing",
        "function main() -> void { int|string v = 1; if (v is int) { int y = v + 1; } }",
    );
}

// ── The remaining matrix rows: match arms, catch, and the ctor-param/field dividing line ─────────

#[test]
fn case_08_match_arm_binding_shadows_an_outer_local() {
    let src = "enum Shape { Circle(float r), Square(float s) } \
               function main() -> void { float r = 100.0; Shape sh = new Circle(2.0); \
                   string out = match (sh) { Circle(r) => \"c{r}\", Square(s) => \"s{s}\" }; discard out; }";
    assert!(
        !shadow_errs(src).is_empty(),
        "case 8 must be rejected: {:?}",
        errors_of(src)
    );
}

#[test]
fn case_17_sibling_match_arms_reuse_a_binding_name() {
    // Arms are siblings — never both live — so this stays legal and must keep working.
    assert_accepted(
        "17",
        "enum Shape { Circle(float v), Square(float v) } \
         function main() -> void { Shape sh = new Circle(2.0); \
             string out = match (sh) { Circle(v) => \"c{v}\", Square(v) => \"s{v}\" }; discard out; }",
    );
}

#[test]
fn case_10_catch_binding_shadows_an_outer_local() {
    // Before the fix this leaked an exception dump, a stack trace and an ABSOLUTE PATH into the PHP
    // leg's output — the worst of the ten divergences.
    let src = "function boom() -> void throws Error { throw new Error(\"x\"); } \
               function main() -> void { int e = 7; try { boom(); } catch (Error e) { } }";
    assert!(
        !shadow_errs(src).is_empty(),
        "case 10 must be rejected: {:?}",
        errors_of(src)
    );
}

#[test]
fn case_13_local_redeclares_a_non_promoted_ctor_param() {
    let src = "class Seeded { constructor(int seed) { int seed = 5; } } \
               function main() -> void { Seeded s = new Seeded(1); discard s; }";
    assert!(
        !shadow_errs(src).is_empty(),
        "case 13 must be rejected: {:?}",
        errors_of(src)
    );
}

#[test]
fn case_14_local_redeclares_a_promoted_ctor_param() {
    // REJECTED — and this is the subtle half of the spec's dividing line. A promoted param is ALSO
    // still a live parameter, bare-readable in the constructor body, so there genuinely IS a live
    // binding of that name to collide with.
    let src = "class Holder { constructor(public int myVar) { int myVar = 5; } } \
               function main() -> void { Holder h = new Holder(1); discard h; }";
    assert!(
        !shadow_errs(src).is_empty(),
        "case 14 must be rejected: {:?}",
        errors_of(src)
    );
}

#[test]
fn case_22_method_local_named_like_a_class_field_is_fine() {
    // ACCEPTED — the other half of that dividing line. In a METHOD the field name is not a local
    // binding at all (`this.n` is mandatory, Invariant 12), so nothing is shadowed. Rejecting this
    // would poison every field name inside every method for zero correctness gain.
    assert_accepted(
        "22",
        "class Counter { constructor(public int n) {} \
             function bump() -> int { int n = 99; return this.n + n; } } \
         function main() -> void { Counter c = new Counter(1); discard c.bump(); }",
    );
}

//! Checker tests — unions (M-Decomp W2b, by language feature).

use super::support::*;

#[test]
fn union_catch_covers_each_member() {
    // `catch (BadInput | NotFound e)` discharges a call that throws `BadInput` (a member).
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInput {{ throw new BadInput(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInput | NotFound e) {{}} }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn union_param_accepts_each_member() {
    let ok = errors_of(&format!(
        "{SHAPES} function f(Circle | Square s) -> void {{}} \
             function main() -> void {{ f(new Circle(1)); f(new Square(2)); }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn union_param_rejects_non_member() {
    let bad = errors_of(&format!(
        "{SHAPES} function f(Circle | Square s) -> void {{}} \
             function main() -> void {{ f(Triangle(3)); }}"
    ));
    assert!(
        !bad.is_empty(),
        "expected a type error passing a non-member"
    );
}

#[test]
fn match_over_union_exhaustive_ok() {
    let ok = errors_of(&format!(
        "{SHAPES} function area(Circle | Square s) -> int {{ \
               return match s {{ Circle c => c.radius, Square sq => sq.side }}; }} \
             function main() -> void {{ int a = area(new Circle(2)); }}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn match_over_union_non_exhaustive_lists_missing() {
    let bad = errors_of(&format!(
        "{SHAPES} function area(Circle | Square s) -> int {{ \
               return match s {{ Circle c => c.radius }}; }} \
             function main() -> void {{}}"
    ));
    assert!(
        bad.iter()
            .any(|e| e.message.contains("non-exhaustive") && e.message.contains("Square")),
        "{bad:?}"
    );
}

#[test]
fn union_rejects_enum_member() {
    let bad = errors_of(&format!(
            "{SHAPES} enum Color {{ Red, Green }} function f(Circle | Color x) -> void {{}} function main() -> void {{}}"
        ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-UNION-MEMBER")),
        "{bad:?}"
    );
}

#[test]
fn union_rejects_void_member() {
    // `void` is the uncapturable nothing — a union containing it is uninhabited (E-VOID-IN-UNION),
    // distinct from the generic E-UNION-MEMBER so the diagnostic can point at `empty` as the fix.
    let bad = errors_of("function f(int | void x) -> void {} function main() -> void {}");
    assert!(
        bad.iter().any(|e| e.code == Some("E-VOID-IN-UNION")),
        "{bad:?}"
    );
}

#[test]
fn union_allows_empty_member() {
    // `empty` — the holdable nothing — IS inhabited, so `int | empty` is a valid union.
    let ok = errors_of("function f(int | empty x) -> void {} function main() -> void {}");
    assert!(
        !ok.iter()
            .any(|e| e.code == Some("E-VOID-IN-UNION") || e.code == Some("E-UNION-MEMBER")),
        "{ok:?}"
    );
}

#[test]
fn union_arity_collapse_is_error() {
    let bad = errors_of(&format!(
        "{SHAPES} function f(Circle | Circle x) -> void {{}} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-UNION-ARITY")),
        "{bad:?}"
    );
}

#[test]
fn type_pattern_must_name_a_class_or_interface() {
    let bad = errors_of(&format!(
        "{SHAPES} function f(Circle | Square s) -> int {{ \
               return match s {{ Circle c => c.radius, Nope n => 0 }}; }} function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|e| e.code == Some("E-MATCH-TYPE")),
        "{bad:?}"
    );
}

#[test]
fn instanceof_narrows_a_union_operand() {
    let ok = errors_of(&format!(
        "{SHAPES} function f(Circle | Square s) -> int {{ \
               if (s instanceof Circle) {{ return s.radius; }} return 0; }} function main() -> void {{}}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn primitive_union_literal_match_ok() {
    let ok = errors_of(
        "function classify(int | string code) -> string { \
               return match code { 0 => \"zero\", \"ok\" => \"okay\", _ => \"other\" }; } \
             function main() -> void { string s = classify(0); }",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn primitive_union_accepts_int_and_string() {
    let ok = errors_of(
        "function f(int | string x) -> void {} function main() -> void { f(1); f(\"a\"); }",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn type_pattern_nested_in_variant_is_accepted() {
    // S5.2-T2: a type pattern nested in a variant payload is now allowed (every backend recurses
    // variant fields). It is refutable, so it does not discharge the variant's coverage — an
    // irrefutable fallback (here a bare `One(other)`) is required for exhaustiveness.
    let ok = errors_of(&format!(
        "{SHAPES} enum Wrap {{ One(Circle inner) }} \
             function f(Wrap w) -> int {{ return match w {{ One(Circle c) => c.radius, One(o) => 0 }}; }} \
             function main() -> void {{}}"
    ));
    assert!(
        !ok.iter().any(|e| e.code == Some("E-MATCH-TYPE")),
        "no longer rejected: {ok:?}"
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");

    // Without the fallback the refutable arm leaves `One` undischarged — non-exhaustive.
    let bad = errors_of(&format!(
        "{SHAPES} enum Wrap {{ One(Circle inner) }} \
             function f(Wrap w) -> int {{ return match w {{ One(Circle c) => c.radius }}; }} \
             function main() -> void {{}}"
    ));
    assert!(
        bad.iter().any(|e| e.message.contains("non-exhaustive")),
        "{bad:?}"
    );
}

#[test]
fn union_string_pattern_erased_ambig_rejected() {
    // Byte-identity guard (G-1): a `string` type-pattern over a union that also holds a
    // decimal/bytes/html/attr sibling is `E-MATCH-ERASED-AMBIG` — the transpiled `is_string()`
    // can't tell an erased sibling from a real string (run/runvm distinguish by value kind).
    let bad = errors_of(
        "function f(string | decimal v) -> string { \
               return match v { string s => s, _ => \"x\" }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.code == Some("E-MATCH-ERASED-AMBIG")),
        "{bad:?}"
    );
}

#[test]
fn optional_union_string_pattern_erased_ambig_rejected() {
    // Wave A slice 2: the erasure guard must see through an `Optional` — a `(string | decimal)?`
    // (the `T?` a `List.first`/`Map.get` returns) is the same byte-identity hazard behind a `?`,
    // and must not bypass `E-MATCH-ERASED-AMBIG`.
    let bad = errors_of(
        "function f((string | decimal)? v) -> string { \
               return match v { string s => s, _ => \"x\" }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.code == Some("E-MATCH-ERASED-AMBIG")),
        "{bad:?}"
    );
}

#[test]
fn optional_union_type_patterns_ok() {
    // A clean `(int | string)?` — no erasing sibling — matches by primitive type-pattern plus a
    // `_` catch-all without tripping the erasure guard (Wave A slice 2: the shape a union-element
    // collection's `.first`/`Map.get` yields, consumed at the call site).
    let ok = errors_of(
        "function f((int | string)? v) -> string { \
               return match v { int i => \"i\", string s => s, _ => \"n\" }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn optional_union_flat_exhaustive_ok() {
    // DEC-183: `Optional<T>` is `T | null` for match totality — the member arms plus a `null` arm
    // are exhaustive with NO `_` (`(int | string)?`, the shape a `List.first`/`Map.get` returns).
    let ok = errors_of(
        "function f((int | string)? v) -> string { \
               return match v { int i => \"i\", string s => s, null => \"z\" }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn optional_single_prim_flat_exhaustive_ok() {
    // The `T | null` reading applies to a single-primitive optional too: `int?` is total with an
    // `int` arm plus a `null` arm.
    let ok = errors_of(
        "function f(int? v) -> string { return match v { int i => \"i\", null => \"z\" }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn optional_union_missing_null_arm_is_nonexhaustive() {
    // The `null` case is a real member: omitting the `null` arm (and any `_`) is non-exhaustive.
    let bad = errors_of(
        "function f((int | string)? v) -> string { \
               return match v { int i => \"i\", string s => s }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter()
            .any(|e| e.message.contains("non-exhaustive") && e.message.contains("null")),
        "{bad:?}"
    );
}

#[test]
fn optional_union_missing_member_lists_it() {
    // A missing discriminable member is named even when `null` is covered.
    let bad = errors_of(
        "function f((int | string)? v) -> string { \
               return match v { int i => \"i\", null => \"z\" }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter()
            .any(|e| e.message.contains("non-exhaustive") && e.message.contains("string")),
        "{bad:?}"
    );
}

#[test]
fn optional_enum_flat_still_needs_wildcard() {
    // Ruled caveat (DEC-183): enum-variant coverage is NOT threaded through `?`, so an
    // `Optional<enum>` matched by variant arms is still rejected (the flat form is primitives +
    // classes/interfaces only).
    let bad = errors_of(
        "enum Color { Red, Green } \
             function f(Color? c) -> string { \
               return match c { Red() => \"r\", Green() => \"g\", null => \"z\" }; } \
             function main() -> void {}",
    );
    assert!(!bad.is_empty(), "expected rejection, got clean");
}

#[test]
fn optional_enum_null_first_still_rejected() {
    // Pins the caveat under a `null`-first arm order: after the `null` arm the variant arms narrow to
    // the bare enum and type-check, but the `Optional<enum>` scrutinee still isn't exhaustive-matchable
    // (`has_enum` requires a `_`). Intentional — enum coverage through `?` is the separate follow-up.
    let bad = errors_of(
        "enum Color { Red, Green } \
             function f(Color? c) -> string { \
               return match c { null => \"z\", Red() => \"r\", Green() => \"g\" }; } \
             function main() -> void {}",
    );
    assert!(!bad.is_empty(), "expected rejection, got clean");
}

#[test]
fn optional_single_class_flat_ok() {
    // DEC-183 class axis: a nullable single class `Circle?` is total with a `Circle c` type-pattern
    // plus a `null` arm (the shape a `Map<K, Circle>.get` returns).
    let ok = errors_of(&format!(
        "{SHAPES} function f(Circle? c) -> int {{ \
               return match c {{ Circle x => x.radius, null => 0 }}; }} \
             function main() -> void {{}}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

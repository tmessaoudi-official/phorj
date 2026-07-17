//! Checker tests — unions (M-Decomp W2b, by language feature).

use super::support::*;

#[test]
fn union_catch_covers_each_member() {
    // `catch (BadInputError | NotFoundError e)` discharges a call that throws `BadInputError` (a member).
    let ok = errors_of(&format!(
        "{ERRDEF} function f() -> void throws BadInputError {{ throw new BadInputError(\"x\"); }} \
             function main() -> void {{ try {{ f(); }} catch (BadInputError | NotFoundError e) {{}} }}"
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
               return match code { 0 => \"zero\", \"ok\" => \"okay\", default => \"other\" }; } \
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
               return match v { string s => s, default => \"x\" }; } \
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
               return match v { string s => s, default => \"x\" }; } \
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
               return match v { int i => \"i\", string s => s, default => \"n\" }; } \
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
fn optional_enum_flat_matches_variants_plus_null() {
    // DEC-250 closed the DEC-183 caveat: enum-variant coverage threads through `?` — the variant
    // arms plus a `null` arm are exhaustive over `Color?` (no `_` needed).
    let ok = errors_of(
        "enum Color { Red, Green } \
             function f(Color? c) -> string { \
               return match c { Red() => \"r\", Green() => \"g\", null => \"z\" }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "got {ok:?}");
}

#[test]
fn optional_enum_null_first_also_exhaustive() {
    // DEC-250: arm order is free — a `null`-first `Optional<enum>` match is exhaustive with the
    // variant arms following (the old caveat pinned rejection here; the capability replaced it).
    let ok = errors_of(
        "enum Color { Red, Green } \
             function f(Color? c) -> string { \
               return match c { null => \"z\", Red() => \"r\", Green() => \"g\" }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "got {ok:?}");
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

// ---- Wave A slice 3 (DEC-184): `is` / `instanceof` type-test + flow-narrowing ----

#[test]
fn is_int_then_branch_narrows_to_operand() {
    // `if (x is int)` narrows `x` to `int` in the then-branch, so `x + 1` is valid arithmetic.
    let ok = errors_of(
        "function f(int | string x) -> int { if (x is int) { return x + 1; } return 0; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn is_and_instanceof_symmetric_over_primitive() {
    // DEC-184 full symmetry: both operators test a primitive and both narrow.
    let ok = errors_of(
        "function f(int | string x) -> int { if (x instanceof int) { return x + 1; } return 0; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn is_over_class_narrows_like_instanceof() {
    let ok = errors_of(&format!(
        "{SHAPES} function f(Circle | Square s) -> int {{ if (s is Circle) {{ return s.radius; }} return 0; }} \
             function main() -> void {{}}"
    ));
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn is_string_over_erased_union_rejected() {
    // Same byte-identity guard as `match`: `is string` over a union holding a PHP-string-erased
    // sibling is ambiguous.
    let bad = errors_of(
        "function f(string | decimal v) -> bool { return v is string; } function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.code == Some("E-MATCH-ERASED-AMBIG")),
        "{bad:?}"
    );
}

#[test]
fn is_erased_type_rejected() {
    let bad = errors_of(
        "function f(decimal d) -> bool { return d is decimal; } function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.code == Some("E-MATCH-TYPE-ERASED")),
        "{bad:?}"
    );
}

#[test]
fn is_unknown_type_rejected() {
    let bad =
        errors_of("function f(int x) -> bool { return x is Bogus; } function main() -> void {}");
    assert!(
        bad.iter().any(|e| e.code == Some("E-INSTANCEOF-TYPE")),
        "{bad:?}"
    );
}

#[test]
fn is_null_narrows_optional_in_early_return_tail() {
    // `if (name is null) { return … }` narrows the tail to the non-null inner (lockstep-safe: an
    // optional local already carries its inner operand type on the VM).
    let ok = errors_of(
        "function f(string? name) -> string { if (name is null) { return \"z\"; } return name + \"!\"; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn is_primitive_complement_not_narrowed_lockstep() {
    // The ruled bound (DEC-184): a primitive is NOT narrowed in the complement (here a negated
    // early-return tail over a union) — so `x + 1` on the un-narrowed union is a type error, the
    // SAME rejection the VM compiler makes (lockstep, not a checker-accepts/VM-rejects divergence).
    // The general union-operand fix is W2-12.
    let bad = errors_of(
        "function f(int | string x) -> int { if (!(x is int)) { return 0; } return x + 1; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.message.contains("arithmetic")),
        "{bad:?}"
    );
}

// ---- Wave A slice 4 (W5-3): sealed hierarchies — exhaustive match over a closed subtype set ----

#[test]
fn sealed_interface_match_exhaustive_ok() {
    // A sealed interface's implementors are the closed set — matching all of them is exhaustive, no `_`.
    let ok = errors_of(
        "sealed interface Shape {} \
             class Circle implements Shape { constructor(public int r) {} } \
             class Square implements Shape { constructor(public int s) {} } \
             function area(Shape sh) -> int { return match sh { Circle c => c.r, Square s => s.s }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn sealed_abstract_class_match_exhaustive_ok() {
    let ok = errors_of(
        "sealed abstract class Node {} \
             class Leaf extends Node { constructor(public int v) {} } \
             class Branch extends Node { constructor(public int n) {} } \
             function sum(Node nd) -> int { return match nd { Leaf l => l.v, Branch b => b.n }; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn sealed_match_missing_subtype_is_nonexhaustive() {
    let bad = errors_of(
        "sealed interface Shape {} \
             class Circle implements Shape { constructor(public int r) {} } \
             class Square implements Shape { constructor(public int s) {} } \
             function area(Shape sh) -> int { return match sh { Circle c => c.r }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter()
            .any(|e| e.message.contains("non-exhaustive") && e.message.contains("Square")),
        "{bad:?}"
    );
}

#[test]
fn non_sealed_base_match_still_needs_wildcard() {
    // Sealed is opt-in: a plain `open` base is NOT a closed hierarchy, so a match over it still needs
    // a `_` (a subtype could be declared anywhere).
    let bad = errors_of(
        "open class Base {} class A extends Base {} class B extends Base {} \
             function f(Base x) -> int { return match x { A a => 1, B b => 2 }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter().any(|e| e.message.contains("non-exhaustive")),
        "{bad:?}"
    );
}

#[test]
fn sealed_concrete_class_base_is_itself_a_member() {
    // A CONCRETE (instantiable) sealed class can hold a base-typed value, so matching only its
    // subclasses is non-exhaustive — the base itself must be covered.
    let bad = errors_of(
        "sealed class Shape { constructor(public int tag) {} } \
             class Circle extends Shape {} \
             function f(Shape s) -> int { return match s { Circle c => 1 }; } \
             function main() -> void {}",
    );
    assert!(
        bad.iter()
            .any(|e| e.message.contains("non-exhaustive") && e.message.contains("Shape")),
        "{bad:?}"
    );
}

#[test]
fn sealed_class_is_extensible() {
    // A sealed class exists to be subclassed — extending it is NOT `E-EXTEND-FINAL` (sealed implies open).
    let ok = errors_of(
        "sealed abstract class Node {} class Leaf extends Node {} function main() -> void {}",
    );
    assert!(
        !ok.iter().any(|e| e.code == Some("E-EXTEND-FINAL")),
        "{ok:?}"
    );
}

// ---- Wave A (UA-1.6 / DEC-178): expected-type threading into a map literal (VarDecl position) ----

#[test]
fn annotated_map_literal_threads_union_value_type() {
    // `Map<string, int | string>` accepts heterogeneous values — the declared value union is threaded
    // into the literal (each value assignable to `int | string`), which bottom-up `check_map`
    // (first-pair-wins) could not do.
    let ok = errors_of(
        "function main() -> void { Map<string, int | string> m = [\"a\" => 1, \"b\" => \"two\"]; }",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn annotated_map_literal_rejects_wrong_value() {
    let bad = errors_of("function main() -> void { Map<string, int> m = [\"a\" => \"x\"]; }");
    assert!(
        bad.iter().any(|e| e.message.contains("expected `int`")),
        "{bad:?}"
    );
}

#[test]
fn annotated_map_literal_still_enforces_key_hashability() {
    // The expected-type path must not bypass `check_map`'s `E-MAP-KEY` key-hashability guard.
    let bad = errors_of("function main() -> void { Map<float, int> m = [1.5 => 1]; }");
    assert!(bad.iter().any(|e| e.code == Some("E-MAP-KEY")), "{bad:?}");
}

// ---- Wave A (UA-1.6): expected-type threading in RETURN position ----

#[test]
fn return_list_literal_threads_union() {
    let ok = errors_of(
        "function f() -> List<int | string> { return [1, \"two\"]; } function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn return_map_literal_threads_union_value() {
    let ok = errors_of(
        "function f() -> Map<string, int | string> { return [\"a\" => 1, \"b\" => \"two\"]; } \
             function main() -> void {}",
    );
    assert!(ok.is_empty(), "expected clean, got {ok:?}");
}

#[test]
fn return_list_literal_wrong_element_rejected() {
    let bad =
        errors_of("function f() -> List<int> { return [1, \"x\"]; } function main() -> void {}");
    assert!(
        bad.iter().any(|e| e.message.contains("expected `int`")),
        "{bad:?}"
    );
}

#[test]
fn call_arg_list_literal_threads_union_param() {
    // Wave C foundation (DEC-178): a heterogeneous list LITERAL passed straight to a `List<union>`
    // parameter threads the element union — each element checked against `int | string` — rather than
    // being bottom-up inferred as `List<int>` and rejected. Parallel to decl-init/return threading.
    assert!(errors_of(
        "import Core.List; \
         function f(List<int | string> xs) -> int { return 1; } \
         function main() -> void { int n = f([1, \"x\", 2]); }"
    )
    .is_empty());
}

#[test]
fn call_arg_list_literal_still_rejects_off_union_element() {
    // Threading is not a blanket accept: an element outside the union (`bool`) is still an error —
    // now reported per-element against the expected union (aligned with decl/return), not as a
    // whole-list mismatch.
    let errs = errors_of(
        "import Core.List; \
         function f(List<int | string> xs) -> int { return 1; } \
         function main() -> void { int n = f([1, true]); }",
    );
    assert!(!errs.is_empty(), "off-union element must still error");
}

#[test]
fn call_arg_empty_list_to_generic_callee_rejected() {
    // DEC-214 part-2: the former empty-`[]`→`List<T>` call-argument special-case is GONE — a bare `[]`
    // passed to a generic callee (`List<T>` param) is now `E-EMPTY-LITERAL`. The empty collection is
    // built with `new List<int>()`, which binds `T` explicitly. `List.isEmpty` is `List<T> -> bool`.
    let e = errors_of(
        "import Core.Output; import Core.List; \
         function main() -> void { Output.printLine(\"{List.isEmpty([])}\"); }",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-EMPTY-LITERAL")),
        "got {e:?}"
    );
    assert!(errors_of(
        "import Core.Output; import Core.List; \
         function main() -> void { Output.printLine(\"{List.isEmpty(new List<int>())}\"); }"
    )
    .is_empty());
}

#[test]
fn call_arg_generic_callee_via_variable_unaffected() {
    // A generic callee (`List<T>`, `T` unbound) still resolves `T` from a bound variable exactly as
    // before — the foundation slice threads LITERAL args only against CONCRETE collection params, so
    // the generic bidirectional-inference path is untouched.
    assert!(errors_of(
        "import Core.List; \
         function main() -> void { var xs = [1, 2, 3]; \
          var ys = List.map(xs, function(int x) => x + 1); }"
    )
    .is_empty());
}

#[test]
fn call_arg_generic_callee_heterogeneous_literal_still_deferred() {
    // Documents the still-DEFERRED case: a HETEROGENEOUS literal passed to a GENERIC callee (`Set.of`
    // is `List<T> -> Set<T>`) can't infer `T = int | string` bottom-up, so it stays "elements must
    // share one type" (needs bidirectional inference through the callee's type param — a later Wave C
    // slice). The `ty_has_param` guard keeps generic callees on this path; a HOMOGENEOUS literal
    // (`Set.of([1,2,3])`) works as before via ordinary unification.
    let errs =
        errors_of("import Core.Set; function main() -> void { var s = Set.of([1, \"x\"]); }");
    assert!(
        !errs.is_empty(),
        "heterogeneous literal to a generic callee is still deferred"
    );
    // The homogeneous case is unaffected (works).
    assert!(
        errors_of("import Core.Set; function main() -> void { var s = Set.of([1, 2, 3]); }")
            .is_empty()
    );
}

// ── DEC-253: nullable unions ─────────────────────────────────────────────────────────────────────

#[test]
fn nullable_union_both_spellings_are_the_same_type() {
    // `A | B | null` (PHP-familiar) and `(A | B)?` (canonical) resolve identically — a value of
    // one flows into the other in both directions, and null inhabits both.
    let errs = errors_of(
        "class A { constructor(public int x) {} } class B { constructor(public string s) {} } \
         function f(): A | B | null { return null; } \
         function g((A | B)? v): (A | B)? { return v; } \
         function main(): void { (A | B)? a = f(); A | B | null b = g(a); discard b; }",
    );
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn nullable_union_matches_with_member_and_null_arms() {
    let errs = errors_of(
        "class A { constructor(public int x) {} } class B { constructor(public string s) {} } \
         function f(int n): A | B | null { if (n == 1) { return new A(1); } return null; } \
         function main(): void { \
             string s = match (f(1)) { A a => \"a\", B b => \"b\", null => \"n\" }; discard s; }",
    );
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn standalone_null_type_is_rejected() {
    let errs = errors_of("function main(): void { null x = null; discard x; }");
    assert!(
        errs.iter().any(|e| e.code == Some("E-NULL-TYPE")),
        "{errs:?}"
    );
}

#[test]
fn null_only_union_is_an_arity_error() {
    let errs = errors_of("function f(): null | null { return null; } function main(): void {}");
    assert!(
        errs.iter().any(|e| e.code == Some("E-UNION-ARITY")),
        "{errs:?}"
    );
}

#[test]
fn optional_union_displays_parenthesized() {
    // The rendered type must be `(A | B)?` — `A | B?` re-reads as `A | (B?)`.
    let errs = errors_of(
        "class A { constructor(public int x) {} } class B { constructor(public string s) {} } \
         function main(): void { (A | B)? v = null; int n = v; discard n; }",
    );
    let msg = errs
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(msg.contains("(A | B)?"), "{msg}");
}

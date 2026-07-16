//! Checker tests — default parameter values (M4 default parameters, free-function-only v1).

use super::support::*;

#[test]
fn default_param_makes_arg_optional() {
    // A trailing defaulted param may be omitted OR supplied.
    let src = "function f(int x, int y = 10) -> int { return x + y; } \
               function main() -> void { int a = f(1); int b = f(1, 2); }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
    // Two defaults; 1, 2, or 3 args all valid.
    let src2 = "function g(int a, int b = 1, int c = 2) -> int { return a + b + c; } \
                function main() -> void { int x = g(1); int y = g(1, 2); int z = g(1, 2, 3); }";
    assert!(errors_of(src2).is_empty(), "got {:?}", errors_of(src2));
}

#[test]
fn too_few_or_too_many_still_errors() {
    // Below the required arity (no default covers `x`) is an error.
    let lo = errors_of("function f(int x, int y = 1) -> int { return x; } function main() -> void { int a = f(); }");
    assert!(!lo.is_empty(), "f() should be too few args");
    // Above the param count is an error.
    let hi = errors_of("function f(int x, int y = 1) -> int { return x; } function main() -> void { int a = f(1, 2, 3); }");
    assert!(!hi.is_empty(), "f(1,2,3) should be too many args");
}

#[test]
fn default_must_be_trailing() {
    let e = errors_of("function f(int x = 1, int y) -> int { return x + y; }");
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-ORDER")),
        "got {e:?}"
    );
}

#[test]
fn default_must_be_literal() {
    // A non-literal default (a call) is rejected.
    let e = errors_of(
        "function side() -> int { return 1; } function f(int x = side()) -> int { return x; }",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-EXPR")),
        "got {e:?}"
    );
}

#[test]
fn default_type_must_match() {
    let e = errors_of("function f(int x = \"no\") -> int { return x; }");
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-TYPE")),
        "got {e:?}"
    );
    // A null default is allowed only for an optional parameter.
    assert!(errors_of("function f(int? x = null) -> void {}").is_empty());
    let e2 = errors_of("function f(int x = null) -> void {}");
    assert!(
        e2.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-TYPE")),
        "got {e2:?}"
    );
}

// ── DEC-249: method default parameters ───────────────────────────────────────────────────────────

#[test]
fn method_default_makes_call_arity_optional() {
    // Instance + static methods accept omitted trailing defaulted args (the DEC-236 machinery,
    // extended to method dispatch); explicit args still check normally.
    let src = "class C { constructor() {} \
                   function m(int x = 1): int { return x; } \
                   static function s(string t = \"d\"): string { return t; } } \
               function main() -> void { C c = new C(); int a = c.m(); int b = c.m(7); \
                   string d = C.s(); string e = C.s(\"x\"); \
                   discard a; discard b; discard d; discard e; }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn method_default_is_inherited_with_the_signature() {
    let src = "open class P { constructor() {} function m(int x = 3): int { return x; } } \
               class D extends P { constructor() {} } \
               function main() -> void { D d = new D(); int a = d.m(); discard a; }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn method_default_on_generic_typed_param_is_a_clean_deferral() {
    // A default on a GENERIC-TYPED param defers (the literal can't check against uninferred T)…
    let e = errors_of(
        "class Box<T> { constructor() {} function m(T x = 1) -> int { return 0; } } \
         function main() -> void {}",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-CTOR-DEFAULT-GENERIC")),
        "got {e:?}"
    );
    // …but a NON-generic defaulted param on a generic method fills before inference
    // (the `db.transaction(fn, int retries = 0)` shape).
    let src = "class C { constructor() {} \
                   function pick<T>(T v, int n = 2) -> T { discard n; return v; } } \
               function main() -> void { C c = new C(); int a = c.pick(5); int b = c.pick(6, 3); \
                   discard a; discard b; }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn method_default_rules_still_enforced() {
    // Order + literal-only + type checks apply to methods exactly as to free functions.
    let e = errors_of(
        "class C { constructor() {} function m(int x = 1, int y) -> int { return x + y; } } \
         function main() -> void {}",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-ORDER")),
        "got {e:?}"
    );
    let e2 = errors_of(
        "class C { constructor() {} function m(int x = \"s\") -> int { return x; } } \
         function main() -> void {}",
    );
    assert!(
        e2.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-TYPE")),
        "got {e2:?}"
    );
}

#[test]
fn omitting_defaults_on_null_safe_call_is_deferred() {
    let e = errors_of(
        "class C { constructor() {} function m(int x = 1): int { return x; } } \
         function main() -> void { C? c = new C(); int? a = c?.m(); discard a; }",
    );
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-CONTEXT")),
        "got {e:?}"
    );
}

// ── DEC-236: constructor default parameters ──────────────────────────────────────────────────────────

#[test]
fn ctor_default_makes_construction_arity_optional() {
    let src = "class C { constructor(public int x, public int y = 10) {} } \
               function main() -> void { C a = new C(1); C b = new C(1, 2); }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

#[test]
fn ctor_default_must_be_trailing_literal_and_typed() {
    let e = errors_of("class C { constructor(public int x = 1, public int y) {} }");
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-ORDER")),
        "got {e:?}"
    );
    let e = errors_of("class C { constructor(public int x = 1 + 2) {} }");
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-EXPR")),
        "got {e:?}"
    );
    let e = errors_of("class C { constructor(public int x = \"nope\") {} }");
    assert!(
        e.iter().any(|d| d.code == Some("E-DEFAULT-PARAM-TYPE")),
        "got {e:?}"
    );
}

#[test]
fn ctor_default_on_generic_class_is_a_clean_deferral() {
    let e = errors_of("class Box<T> { constructor(private T v, public int n = 0) {} }");
    assert!(
        e.iter().any(|d| d.code == Some("E-CTOR-DEFAULT-GENERIC")),
        "got {e:?}"
    );
}

#[test]
fn ctor_defaults_are_inherited_with_the_signature() {
    // A subclass with no own ctor inherits the parent's signature INCLUDING its defaults.
    let src = "open class P { constructor(public int x, public int y = 7) {} } \
               class C extends P {} \
               function main() -> void { C a = new C(1); C b = new C(1, 2); }";
    assert!(errors_of(src).is_empty(), "got {:?}", errors_of(src));
}

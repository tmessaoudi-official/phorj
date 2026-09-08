//! Single-site diagnostics that had no assertion — one test each, named for the rule it pins.
use super::support::*;

fn has(src: &str, code: &str) {
    let e = errors_of(src);
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

#[test]
fn new_is_only_for_constructing() {
    has(
        "function square(int n) -> int { return n * n; } function main() -> void { int x = new square(2); }",
        "E-NEW-ON-NONCONSTRUCT",
    );
}

#[test]
fn an_overloaded_function_has_no_single_value() {
    has(
        "function f(int a) -> int { return a; } function f(string a) -> string { return a; } function main() -> void { var g = f; }",
        "E-OVERLOAD-FN-VALUE",
    );
}

#[test]
fn a_pattern_qualifier_must_name_the_scrutinee_enum() {
    has(
        "enum A { X } enum B { X } function main() -> void { A a = A.X; int r = match (a) { B.X => 1 }; }",
        "E-VARIANT-QUALIFIER",
    );
}

#[test]
fn string_format_arity_and_types() {
    has(
        "import Core.String; function main() -> void { string s = String.format(\"%d\"); }",
        "E-FORMAT-ARGS",
    );
    has(
        "import Core.String; function main() -> void { string s = String.format(1, [1]); }",
        "E-FORMAT-SPEC-TYPE",
    );
    has(
        "import Core.String; function main() -> void { string s = String.format(\"%d\", [[1]]); }",
        "E-FORMAT-ARG-TYPE",
    );
}

#[test]
fn attribute_arity_rules() {
    has(
        "#[Transient(1)] class S { constructor() {} } function main() -> void { }",
        "E-TRANSIENT-ARGS",
    );
    has(
        "#[Provides(1)] function make() -> int { return 1; } function main() -> void { }",
        "E-PROVIDES-ARGS",
    );
    has(
        "#[UncheckedOverflow(1)] function f() -> int { return 1; } function main() -> void { }",
        "E-UNCHECKED-ARGS",
    );
}

#[test]
fn tuple_destructuring_needs_a_tuple_of_the_right_arity() {
    has(
        "function main() -> void { var (a, b) = 1; }",
        "E-DESTRUCTURE-NOT-TUPLE",
    );
    has(
        "function main() -> void { var (a, b) = (1, 2, 3); }",
        "E-TUPLE-DESTRUCTURE-LEN",
    );
}

/// DEC-504 — the four ways a named-tuple field access is refused. Each is refused BY NAME because
/// they are DIFFERENT mistakes: a typo, a tuple that has no names at all, an ambiguous duplicate,
/// and a `?.` whose null short-circuit cannot survive erasure to an index.
#[test]
fn named_tuple_field_access_is_refused_by_name() {
    has(
        "function main() -> void { (bp: int, source: string) t = (bp: 1, source: \"a\"); var x = t.nope; }",
        "E-TUPLE-UNKNOWN-FIELD",
    );
    has(
        "function main() -> void { (int, string) t = (1, \"a\"); var x = t.bp; }",
        "E-TUPLE-POSITIONAL-FIELD",
    );
    has(
        "function main() -> void { var t = (a: 1, a: 2); }",
        "E-TUPLE-DUP-FIELD",
    );
    // `?.` erases to a plain `Index`, which has no null short-circuit — refused rather than
    // silently typing as `int?` while faulting at runtime.
    has(
        "function main() -> void { (bp: int, source: string) t = (bp: 1, source: \"a\"); var x = t?.bp; }",
        "E-TUPLE-SAFE-FIELD",
    );
}

/// A tuple is a VALUE, not a shared-mutable instance: there is no `t.bp = v` (nor `+=`/`++`).
/// Rebind the whole tuple instead. Pinned because the field read landing made the write LOOK
/// available.
#[test]
fn a_named_tuple_field_cannot_be_assigned() {
    has(
        "function main() -> void { mutable (bp: int, source: string) t = (bp: 1, source: \"a\"); t.bp = 5; }",
        "E-ASSIGN-TARGET",
    );
    has(
        "function main() -> void { mutable (bp: int, source: string) t = (bp: 1, source: \"a\"); t.bp += 5; }",
        "E-ASSIGN-TARGET",
    );
}

#[test]
fn a_variadic_parameter_cannot_default() {
    has(
        "function f(int ...xs = 1) -> int { return 0; } function main() -> void { }",
        "E-VARIADIC-DEFAULT",
    );
}

#[test]
fn a_ufcs_call_matching_two_imported_natives_is_ambiguous() {
    // `Json.parse(string)` and `Ini.parse(string)` both accept a string receiver.
    has(
        "import Core.Json; import Core.Ini; function main() -> void { var v = \"a=1\".parse(); }",
        "E-UFCS-AMBIGUOUS",
    );
}

#[test]
fn parent_call_with_two_declaring_parents_is_ambiguous() {
    has(
        "class A { constructor() {} function m() -> int { return 1; } } \
         class B { constructor() {} function m() -> int { return 2; } } \
         class C extends A, B { constructor() { super(); } function n() -> int { return parent.m(); } } \
         function main() -> void { }",
        "E-PARENT-AMBIGUOUS",
    );
}

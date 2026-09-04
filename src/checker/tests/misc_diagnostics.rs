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

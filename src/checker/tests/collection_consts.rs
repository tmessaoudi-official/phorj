//! Checker tests — collection constants (DEC-533, scout row 5m): a class `const` holding a List/Map
//! literal of literal constants, and a negative number as a literal constant.

use super::support::*;

fn clean(src: &str) {
    let e = errors_of(src);
    assert!(e.is_empty(), "{e:?}");
}

fn has(src: &str, code: &str) {
    let e = errors_of(src);
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

#[test]
fn a_map_constant_checks() {
    clean(
        "class T { const Map<string, string> FOLD = [\"à\" => \"a\", \"é\" => \"e\"]; } \
         function main(): void { var x = T.FOLD; }",
    );
}

#[test]
fn a_list_constant_checks() {
    clean("class T { const List<string> WORDS = [\"a\", \"b\"]; } function main(): void { var x = T.WORDS; }");
}

#[test]
fn a_nested_constant_checks_under_its_explicit_type() {
    clean(
        "class T { const Map<string, Map<string, List<int>>> BANDS = [\"Paris\" => [\"PLAI\" => [1, 2]]]; } \
         function main(): void { var x = T.BANDS; }",
    );
}

#[test]
fn a_constant_element_of_the_wrong_type_is_named() {
    // Reported at the element, exactly as a typed local's initializer is.
    let e = errors_of(
        "class T { const Map<string, int> M = [\"a\" => \"b\"]; } function main(): void {}",
    );
    assert!(
        e.iter()
            .any(|d| d.message.contains("expected `int`, found `string`")),
        "{e:?}"
    );
    // A scalar literal against a collection type is the whole-initializer mismatch.
    has(
        "class T { const List<int> L = 5; } function main(): void {}",
        "E-CONST-INIT-TYPE",
    );
}

#[test]
fn a_non_literal_element_is_not_a_constant() {
    has(
        "function f(): int { return 1; } class T { const List<int> L = [1, f()]; } function main(): void {}",
        "E-CONST-NOT-LITERAL",
    );
    has(
        "class T { const List<string> L = [\"a\", \"{1}\"]; } function main(): void {}",
        "E-CONST-NOT-LITERAL",
    );
}

#[test]
fn a_null_value_needs_an_optional_value_type() {
    clean(
        "class T { const Map<string, string?> N = [\"inli\" => \"inli\", \"x\" => null]; } \
         function main(): void { var x = T.N; }",
    );
    has(
        "class T { const Map<string, string> N = [\"inli\" => \"inli\", \"x\" => null]; } function main(): void {}",
        "E-OPT-ASSIGN",
    );
}

#[test]
fn a_collection_constant_is_still_immutable_and_never_empty() {
    has(
        "class T { mutable const List<int> L = [1]; } function main(): void {}",
        "E-CONST-MUTABLE",
    );
    has(
        "class T { const List<int> L = []; } function main(): void {}",
        "E-EMPTY-LITERAL",
    );
}

#[test]
fn a_collection_is_not_an_enum_backing_value() {
    has(
        "enum E: int { A = [1] } function main(): void { }",
        "E-ENUM-VALUE-NOT-LITERAL",
    );
}

#[test]
fn a_negative_number_is_a_literal_constant() {
    clean(
        "class T { const int N = -5; const float F = -1.5; const List<int> L = [-1, 2]; } \
         function main(): void { var x = T.N; }",
    );
    clean("enum E: int { A = -1, B = 2 } function main(): void { }");
    clean("function f(int x = -1, float y = -0.5): int { return x; } function main(): void { var z = f(); }");
}

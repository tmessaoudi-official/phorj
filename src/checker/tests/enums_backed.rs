//! Backed-enum diagnostics (DEC-302). Every code here had an emit site and a `phg explain` entry but
//! no assertion — Invariant 17's 100% rule counts that as uncovered, and the surface ratchet said so.
use super::support::*;

fn has(src: &str, code: &str) -> bool {
    let e = errors_of(src);
    let hit = e.iter().any(|d| d.code == Some(code));
    assert!(hit, "expected {code}, got {e:?}");
    hit
}

#[test]
fn backing_type_must_be_int_or_string() {
    has(
        "enum E: float { A = 1.0 } function main() -> void { }",
        "E-ENUM-BACKING-TYPE",
    );
}

#[test]
fn a_generic_enum_cannot_be_backed() {
    has(
        "enum E<T>: int { A = 1 } function main() -> void { }",
        "E-ENUM-BACKING-GENERIC",
    );
}

#[test]
fn a_backed_variant_cannot_carry_a_payload() {
    has(
        "enum E: int { A(int x) = 1 } function main() -> void { }",
        "E-ENUM-BACKED-PAYLOAD",
    );
}

#[test]
fn a_backing_value_must_be_a_literal() {
    has(
        "enum E: int { A = 1 + 1 } function main() -> void { }",
        "E-ENUM-VALUE-NOT-LITERAL",
    );
}

#[test]
fn a_backing_value_must_match_the_backing_type() {
    has(
        "enum E: int { A = \"one\" } function main() -> void { }",
        "E-ENUM-VALUE-TYPE",
    );
}

#[test]
fn backing_values_must_be_distinct() {
    has(
        "enum E: int { A = 1, B = 1 } function main() -> void { }",
        "E-ENUM-DUP-VALUE",
    );
}

#[test]
fn every_backed_variant_needs_a_value() {
    has(
        "enum E: int { A = 1, B } function main() -> void { }",
        "E-ENUM-VARIANT-NO-VALUE",
    );
}

#[test]
fn a_value_on_an_unbacked_enum_is_rejected() {
    has(
        "enum E { A = 1 } function main() -> void { }",
        "E-ENUM-VALUE-UNBACKED",
    );
}

#[test]
fn cases_from_and_try_from_are_reserved_variant_names() {
    has(
        "enum E { cases } function main() -> void { }",
        "E-ENUM-RESERVED-VARIANT",
    );
}

#[test]
fn value_on_an_unbacked_enum_is_rejected() {
    has(
        "enum E { A } function main() -> void { E e = E.A; int v = e.value; }",
        "E-ENUM-NOT-BACKED",
    );
}

#[test]
fn cases_needs_every_variant_payload_less() {
    has(
        "enum E { A, B(int x) } function main() -> void { List<E> all = E.cases(); }",
        "E-ENUM-CASES-PAYLOAD",
    );
}

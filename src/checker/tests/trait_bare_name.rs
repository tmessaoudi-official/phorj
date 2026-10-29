//! Checker tests — a bare name in a TRAIT method body that a using class stores as a field (plan
//! row 4b step 3, KNOWN_ISSUES TRAIT-BARE-NAME). The round-5 rule (2026-10-08) reads a bare value
//! name field-first, as every backend does; a trait body runs with `this` = the using class, so a
//! field of ANY using class (own or inherited) makes the bare name a field read: `E-BARE-FIELD`.

use super::support::*;

fn has(src: &str, code: &str) -> bool {
    errors_of(src).iter().any(|e| e.code == Some(code))
}

#[test]
fn a_using_classs_field_makes_a_trait_bare_name_a_field_read() {
    // The KNOWN_ISSUES program: the VM and PHP ran the function (5), the oracle read `C.k` and faulted.
    let src = "function k(): int { return 5; } function apply(() => int f): int { return f(); } \
               trait T { function m(): int { return apply(k); } } class C { use T; int k = 9; } \
               function main(): void { int r = new C().m(); }";
    assert!(has(src, "E-BARE-FIELD"), "{:?}", errors_of(src));
}

#[test]
fn an_inherited_field_of_a_using_class_counts_too() {
    let src = "function k(): int { return 5; } function apply(() => int f): int { return f(); } \
               trait T { function m(): int { return apply(k); } } open class B { int k = 9; } \
               class C extends B { use T; } function main(): void { int r = new C().m(); }";
    assert!(has(src, "E-BARE-FIELD"), "{:?}", errors_of(src));
}

#[test]
fn a_trait_whose_users_have_no_such_field_keeps_the_function() {
    let src = "function k(): int { return 5; } function apply(() => int f): int { return f(); } \
               trait T { function m(): int { return apply(k); } } class C { use T; int other = 9; } \
               function main(): void { int r = new C().m(); }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

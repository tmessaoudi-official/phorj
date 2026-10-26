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

#[test]
fn the_trait_hint_does_not_advise_a_this_read_the_trait_cannot_check() {
    // Panel round 1 (completeness lens): `this.k` inside `T` is "type T has no field k", so the fix
    // for a using class's field is a rename, and the message must say so.
    let src = "function k(): int { return 5; } function apply(() => int f): int { return f(); } \
               trait T { function m(): int { return apply(k); } } class C { use T; int k = 9; } \
               function main(): void { int r = new C().m(); }";
    let errs = errors_of(src);
    let e = errs
        .iter()
        .find(|e| e.code == Some("E-BARE-FIELD"))
        .expect("E-BARE-FIELD");
    let text = format!("{} {}", e.message, e.hint.clone().unwrap_or_default());
    assert!(!text.contains("this.k"), "dead-end advice: {text}");
    assert!(text.contains("`C`") && text.contains("rename"), "{text}");
}

#[test]
fn a_bare_field_in_the_class_itself_still_advises_this() {
    let src = "class C { int k = 9; function m(): int { return k; } } function main(): void {}";
    let errs = errors_of(src);
    let e = errs
        .iter()
        .find(|e| e.code == Some("E-BARE-FIELD"))
        .expect("E-BARE-FIELD");
    assert!(e.message.contains("write `this.k`"), "{}", e.message);
}

#[test]
fn the_trait_hint_without_a_shadowed_function_does_not_mention_one() {
    // Panel round 2 (completeness F4): with no function `k` anywhere, "rename the function" names a
    // function that does not exist; the advice is to pass the value in or require an abstract method.
    let src = "trait T { function m(): int { return k; } } class C { use T; int k = 9; } \
               function main(): void { int r = new C().m(); }";
    let errs = errors_of(src);
    let e = errs
        .iter()
        .find(|e| e.code == Some("E-BARE-FIELD"))
        .expect("E-BARE-FIELD");
    let text = format!("{} {}", e.message, e.hint.clone().unwrap_or_default());
    assert!(
        !text.contains("rename the function"),
        "names a missing function: {text}"
    );
    // Not "declare `k` in the trait": a trait's own field initializer is not applied yet (KNOWN_ISSUES
    // STATIC-COMPOSITION-PARITY item 4), so that advice led into a fault (panel round 3).
    assert!(text.contains("abstract function k()"), "{text}");
    assert!(!text.contains("declare `k` in the trait"), "{text}");
}

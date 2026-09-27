//! Row 4o (DEC-543, DEC-544) — a by-reference parameter and a `false`-member union are refused BY
//! NAME, in the ruling's words, instead of reporting the token the parser stopped on.

use super::parser_tests::perr;

#[test]
fn a_by_reference_parameter_is_refused_by_name() {
    let e = perr("<?php function matchOffset(string $p, ?array &$m): bool { return true; }");
    assert!(e.contains("`$m` is taken by reference"), "{e}");
    assert!(e.contains("DEC-543") && e.contains("RULED OUT"), "{e}");
    assert!(!e.contains("found"), "{e}");
    // A method and an untyped parameter say the same; so does a closure's parameter list.
    let e = perr("<?php final class C { public function f(&$acc): void {} }");
    assert!(e.contains("`$acc` is taken by reference"), "{e}");
    let e = perr("<?php $f = fn (int &$n): int => $n;");
    assert!(e.contains("`$n` is taken by reference"), "{e}");
}

#[test]
fn a_union_with_a_false_member_is_refused_in_dec_544_words() {
    let e = perr("<?php function lookup(string $k): string|false|null { return null; }");
    assert!(
        e.contains("`string|false|null` has a `false` member"),
        "{e}"
    );
    assert!(e.contains("DEC-544") && e.contains("enum"), "{e}");
    assert!(!e.contains("found"), "{e}");
    // A parameter and a property type are read the same way, and a `\`-rooted member reads as its leaf.
    let e = perr("<?php function f(\\Foo|false $x): void {}");
    assert!(e.contains("`Foo|false` has a `false` member"), "{e}");
    let e = perr("<?php final class C { private int|false $n = 0; }");
    assert!(e.contains("`int|false` has a `false` member"), "{e}");
}

#[test]
fn any_other_union_is_a_named_gap_not_a_ruling() {
    let e = perr("<?php function f(int|string $x): void {}");
    assert!(e.contains("`int|string` has no lift yet"), "{e}");
    assert!(!e.contains("DEC-544") && !e.contains("found"), "{e}");
}

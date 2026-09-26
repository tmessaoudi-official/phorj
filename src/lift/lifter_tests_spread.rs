//! Row 5u — DEC-538: PHP spread `...` over lists lifts to `List.concat` / `List.flatten`, with no
//! new phorj syntax. The CHECKER arbitrates list-ness (refinement 2026-09-26 08:05): a non-list
//! operand fails `phg check` on the List call, and a phorj List is sequential by construction, so
//! PHP's renumbering spread agrees whenever the draft checks. Only a PROVABLY-map operand — a keyed
//! literal, a known map constant, a Map-declared variable — is refused by name, and so is every
//! spread the ruling does not map (call-site unpacking is DEC-299, unbuilt).

use super::lifter::lift_source;
use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

#[test]
fn two_spreads_become_one_concat() {
    let out = lift(
        "<?php
/**
 * @param list<int> $a
 * @param list<int> $b
 * @return list<int>
 */
function f(array $a, array $b): array { return [...$a, ...$b]; }",
    );
    assert!(out.contains("return List.concat(a, b);"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_spread_and_plain_elements_keep_their_order() {
    let out = lift(
        "<?php
/**
 * @param list<int> $a
 * @param list<int> $b
 * @return list<int>
 */
function f(array $a, array $b, int $x): array { return [...$a, $x, 7, ...$b]; }",
    );
    assert!(
        out.contains("return List.flatten([a, [x, 7], b]);"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_spread_then_one_element_is_a_concat_and_a_lone_spread_is_checked_too() {
    let out = lift(
        "<?php
/**
 * @param list<int> $a
 * @return list<int>
 */
function f(array $a, int $x): array { return [...$a, $x]; }
/**
 * @param list<int> $a
 * @return list<int>
 */
function g(array $a): array { return [...$a]; }",
    );
    assert!(out.contains("return List.concat(a, [x]);"), "{out}");
    // A bare `a` would let a map through unchecked; `flatten([a])` makes the checker see it.
    assert!(out.contains("return List.flatten([a]);"), "{out}");
    checks_clean(&out);
}

#[test]
fn array_merge_of_a_spread_over_array_values_flattens_the_map_values() {
    let out = lift(
        "<?php
/** @return list<int> */
function f(): array {
    $m = [3 => [30], 1 => [10, 11]];
    return array_merge(...array_values($m));
}
/**
 * @param list<list<int>> $xss
 * @return list<int>
 */
function g(array $xss): array { return array_merge(...$xss); }
/** @return list<list<int>> */
function h(): array {
    $m = [1 => [1]];
    return [...array_values($m)];
}",
    );
    assert!(out.contains("return List.flatten(Map.values(m));"), "{out}");
    assert!(out.contains("return List.flatten(xss);"), "{out}");
    assert!(
        out.contains("return List.flatten([Map.values(m)]);"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn array_unshift_with_a_spread_prepends() {
    // A LOCAL target, as scout's `CriteriaEngine.php:404` is: a parameter target fails `phg check`
    // for the reason `usort` on one does (KNOWN_ISSUES, "A PARAMETER target").
    let out = lift(
        "<?php
/**
 * @param list<string> $xs
 * @return list<string>
 */
function f(array $xs): array {
    /** @var list<string> $r */
    $r = ['c'];
    array_unshift($r, ...$xs);
    return $r;
}",
    );
    assert!(out.contains("r = List.concat(xs, r);"), "{out}");
    checks_clean(&out);
}

#[test]
fn a_spread_in_a_keyed_literal_is_refused_by_name() {
    let e = refused(
        "<?php
/**
 * @param list<int> $a
 * @return array<string, int>
 */
function f(array $a): array { return ['k' => 1, ...$a]; }",
    );
    assert!(e.contains("DEC-538") && e.contains("keyed"), "{e}");
}

#[test]
fn a_provably_map_operand_is_refused_by_name() {
    let lit = refused("<?php function f(): array { return [...['k' => 1]]; }");
    assert!(lit.contains("DEC-538") && lit.contains("map"), "{lit}");
    let konst = refused(
        "<?php class C { const M = ['a' => 1]; public function f(): array { return [...self::M]; } }",
    );
    assert!(
        konst.contains("DEC-538") && konst.contains("map"),
        "{konst}"
    );
    let param = refused(
        "<?php
/** @param array<string, int> $m */
function f(array $m): array { return [...$m]; }",
    );
    assert!(
        param.contains("DEC-538") && param.contains("map"),
        "{param}"
    );
    let local = refused(
        "<?php function f(): array {
            /** @var array<string, int> $m */
            $m = ['a' => 1];
            return [...$m];
        }",
    );
    assert!(
        local.contains("DEC-538") && local.contains("map"),
        "{local}"
    );
}

#[test]
fn a_map_declared_inside_a_closure_does_not_answer_outside_it() {
    // `$m` is a Map only inside the closure; outside it is an unknown array the checker arbitrates.
    let out = lift_source(
        "<?php
/**
 * @param list<int> $m
 * @return list<int>
 */
function f(array $m): array {
    $g = function (): int {
        /** @var array<string, int> $m */
        $m = ['a' => 1];
        return count($m);
    };
    return [...$m, $g()];
}",
    )
    .unwrap();
    assert!(out.contains("return List.concat(m, [g()]);"), "{out}");
}

#[test]
fn call_site_unpacking_is_refused_by_name_everywhere() {
    for src in [
        "<?php /** @param list<int> $xs */ function f(array $xs): int { return max(...$xs); }",
        "<?php class P { public function __construct(int $a) {} } /** @param list<int> $xs */ function f(array $xs): P { return new P(...$xs); }",
        "<?php /** @param list<int> $xs */ function f(array $xs): string { return sprintf('%d', ...$xs); }",
    ] {
        let e = refused(src);
        assert!(e.contains("DEC-538") && e.contains("DEC-299"), "{src}\n{e}");
    }
}

#[test]
fn a_first_class_callable_and_a_variadic_parameter_are_refused_by_name() {
    let fcc = refused("<?php function f(array $xs): array { return array_map(strlen(...), $xs); }");
    assert!(fcc.contains("first-class callable"), "{fcc}");
    let variadic = refused("<?php function f(int ...$xs): int { return 0; }");
    assert!(variadic.contains("variadic"), "{variadic}");
}

#[test]
fn a_spread_in_a_constant_array_is_refused_by_name() {
    let e = refused("<?php class C { const A = [1]; const B = [...self::A, 2]; }");
    assert!(e.contains("spread"), "{e}");
}

#[test]
fn a_spread_element_takes_no_key() {
    let e = refused(
        "<?php
/**
 * @param list<int> $a
 * @return list<int>
 */
function f(array $a): array { return [...$a => 1]; }",
    );
    assert!(e.contains("takes no key"), "{e}");
}

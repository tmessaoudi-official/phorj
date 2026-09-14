//! Row 5f — PHP's by-reference `usort` and argument-swapped `array_map` lift to the receiver-form
//! `Core.List` calls. `usort` is statement-only (it mutates its argument and returns `bool`), so it
//! is NOT a `lift_from` row: that table lifts in expression position. Anything outside the exact
//! shapes below falls through to the plain unresolved call — loud at `phg check`, never a guess.

use super::lifter_tests::{assert_reparses, lift};

#[test]
fn usort_statement_on_a_variable_lifts_to_a_sort_with_reassignment() {
    let out = lift(
        "<?php /** @return list<int> */ function f(): array { $xs = [3, 1, 2]; usort($xs, fn(int $a, int $b): int => $a <=> $b); return $xs; }",
    );
    assert!(out.contains("xs = xs.sortWith(function("), "{out}");
    assert!(!out.contains("usort("), "{out}");
    assert!(out.contains("import Core.List;"), "{out}");
    assert_reparses(&out);
}

#[test]
fn array_map_with_one_array_lifts_to_receiver_map_with_the_arguments_swapped() {
    let out = lift(
        "<?php /**\n * @param list<int> $xs\n * @return list<int>\n */ function f(array $xs): array { return array_map(fn(int $x): int => $x * 2, $xs); }",
    );
    assert!(out.contains("return xs.map(function("), "{out}");
    assert!(!out.contains("array_map("), "{out}");
    assert!(out.contains("import Core.List;"), "{out}");
    assert_reparses(&out);
}

#[test]
fn usort_on_a_non_variable_target_is_left_unmapped() {
    // `usort($xss[0], …)` would need an element write; the lift does not guess one.
    let out = lift(
        "<?php /** @param list<list<int>> $xss */ function f(array $xss): void { usort($xss[0], fn(int $a, int $b): int => $a <=> $b); }",
    );
    assert!(out.contains("usort("), "{out}");
    assert!(!out.contains("sortWith"), "{out}");
}

#[test]
fn usort_in_value_position_is_left_unmapped() {
    // The `bool` result has no `sortWith` counterpart, so only the statement form maps.
    let out = lift(
        "<?php /** @param list<int> $xs */ function f(array $xs): bool { return usort($xs, fn(int $a, int $b): int => $a <=> $b); }",
    );
    assert!(out.contains("usort("), "{out}");
    assert!(!out.contains("sortWith"), "{out}");
}

#[test]
fn array_map_over_several_arrays_is_left_unmapped() {
    // PHP zips extra arrays; `List.map` takes one — no silent drop of the second list.
    let out = lift(
        "<?php /**\n * @param list<int> $a\n * @param list<int> $b\n * @return list<int>\n */ function f(array $a, array $b): array { return array_map(fn(int $x, int $y): int => $x + $y, $a, $b); }",
    );
    assert!(out.contains("array_map("), "{out}");
    assert!(!out.contains(".map("), "{out}");
}

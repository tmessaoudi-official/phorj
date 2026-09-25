//! Row 5q — DEC-534: PHP array union `$a + $b` lifts to `Map.union(a, b)`, but ONLY where the lifter
//! knows both operands are maps — a keyed array literal, or a class constant whose lifted type is a
//! `Map`. Anything else stays `+` (a PHP `array` parameter cannot say Map or List), so an int `+` is
//! never rewritten and an unknown one is left for `phg check` to name.

use super::lifter::lift_source;
use super::lifter_tests::lift;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

#[test]
fn two_map_constants_lift_to_map_union() {
    let out = lift(
        "<?php final class Text {
            private const array LOWER = ['à' => 'a'];
            private const array UPPER = ['À' => 'A'];
            /** @return array<string, string> */
            public static function all(): array { return self::LOWER + self::UPPER; }
        }",
    );
    assert!(out.contains("Map.union(Text::LOWER, Text::UPPER)"), "{out}");
    checks_clean(&out);
}

#[test]
fn keyed_literals_lift_to_map_union() {
    let out = lift(
        "<?php /** @return array<string, int> */ function f(): array { return ['a' => 1] + ['b' => 2]; }",
    );
    assert!(
        out.contains("Map.union([\"a\" => 1], [\"b\" => 2])"),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_union_of_unknown_operands_stays_plus() {
    // A PHP `array` parameter can be a Map or a List — the lifter does not guess (DEC-166).
    // Even docblock-typed parameters stay `+`: the ruled narrowing (DEC-534 row 5q) maps literals and
    // constants only.
    let out = lift_source(
        "<?php /** @param array<string, int> $a\n @param array<string, int> $b\n @return array<string, int> */ \
         function f(array $a, array $b): array { return $a + $b; }",
    )
    .unwrap();
    assert!(out.contains("a + b"), "{out}");
    assert!(!out.contains("Map.union"), "{out}");
}

#[test]
fn a_positional_union_and_int_constants_stay_plus() {
    // PHP `[1, 2] + [3]` is a union BY INDEX — no phorj List form; and `Pacer.php:186`'s
    // `self::A + self::B` on two int constants is plain addition.
    let out = lift_source(
        "<?php final class P {
            private const int A = 3;
            private const int B = 4;
            public static function n(): int { return self::A + self::B; }
            /** @return list<int> */
            public static function xs(): array { return [1, 2] + [3]; }
        }",
    )
    .unwrap();
    assert!(!out.contains("Map.union"), "{out}");
    assert!(out.contains("P::A + P::B"), "{out}");
}

#[test]
fn a_map_constant_of_another_file_is_not_assumed() {
    // The registry holds THIS file's classes: a name it has never seen stays `+`.
    let out = lift_source(
        "<?php final class Q { /** @return array<string, string> */ public static function f(): array { return Other::A + Other::B; } }",
    )
    .unwrap();
    assert!(!out.contains("Map.union"), "{out}");
}

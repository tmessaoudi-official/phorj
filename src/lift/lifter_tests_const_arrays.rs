//! Row 5m — DEC-533: a PHP 8.3 `const array` (typed or not) lifts to a phorj collection constant
//! that keeps its PHP name. Its type is read from an `@var` docblock, or inferred from the literal:
//! every level recursively one type (any depth), one scalar type plus `null` → `T?`. A literal
//! that is none of those, or that references another constant (Q-0924-1), is refused by name.

use super::lifter::lift_source;
use super::lifter_tests::lift;

/// The draft parses and type-checks — a text assertion alone would miss a type the checker rejects.
fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn class_with(member: &str) -> String {
    format!("<?php final class Text {{ {member} public static function n(): int {{ return 1; }} }}")
}

#[test]
fn a_typed_string_map_keeps_its_name() {
    let out = lift(&class_with(
        "private const array FOLD_LOWER = ['à' => 'a', 'é' => 'e'];",
    ));
    assert!(
        out.contains("private const Map<string, string> FOLD_LOWER = "),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn an_untyped_list_and_an_int_map_lift() {
    let out = lift(&class_with(
        "const WORDS = ['a', 'b']; private const array TIER = [1 => 97, 2 => 90];",
    ));
    assert!(out.contains("const List<string> WORDS = "), "{out}");
    assert!(out.contains("private const Map<int, int> TIER = "), "{out}");
    checks_clean(&out);
}

#[test]
fn nesting_is_inferred_to_any_depth() {
    let out = lift(&class_with(
        "public const array SOCIAL = ['Paris' => ['PLAI' => [1, 2], 'PLUS' => [3]]];",
    ));
    assert!(
        out.contains("public const Map<string, Map<string, List<int>>> SOCIAL = "),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_scalar_plus_null_infers_an_optional() {
    let out = lift(&class_with(
        "private const array NAMES = ['inli' => 'inli', 'x' => null];",
    ));
    assert!(
        out.contains("private const Map<string, string?> NAMES = "),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn a_nested_nullable_value_is_refused_or_checks() {
    // Whatever the lifter emits must type-check: a `null` below the top level either lifts to a
    // type the checker accepts, or is refused by name — never a draft the checker rejects.
    match lift_source(&class_with(
        "private const array N = ['a' => ['x' => 1, 'y' => null]];",
    )) {
        Ok(out) => checks_clean(&out),
        Err(err) => assert!(err.contains("nullable"), "{err}"),
    }
}

#[test]
fn a_negative_number_is_a_literal() {
    let out = lift(&class_with(
        "private const int FLOOR = -5; private const array B = [-1, 2];",
    ));
    assert!(out.contains("private const int FLOOR = -5;"), "{out}");
    assert!(out.contains("private const List<int> B = "), "{out}");
    checks_clean(&out);
}

#[test]
fn an_at_var_docblock_is_read() {
    // The docblock must CHANGE the answer, or this cannot tell a read docblock from an ignored
    // one: the literal alone infers `Map<string, int>`, the docblock says the values may be null.
    let out = lift(&class_with(
        "/** @var array<string, ?int> */ private const array SCORES = ['a' => 1];",
    ));
    assert!(
        out.contains("private const Map<string, int?> SCORES = "),
        "{out}"
    );
    checks_clean(&out);
}

#[test]
fn an_uninferable_constant_is_refused_by_shape() {
    for (member, shape) in [
        (
            "private const array R = ['a' => ['x'], 'b' => 'y'];",
            "one type",
        ),
        (
            "private const array M = ['a' => 1, 'b' => 'two'];",
            "one type",
        ),
        ("private const array Z = ['a' => null];", "only `null`"),
        ("private const array E = [];", "empty"),
        ("private const array C = [self::A => 'x'];", "Q-0924-1"),
    ] {
        let err = lift_source(&class_with(member)).unwrap_err();
        assert!(err.contains(shape), "{member}\n{err}");
        assert!(
            !err.contains("docblock"),
            "the hint must not promise a docblock fix: {err}"
        );
    }
}

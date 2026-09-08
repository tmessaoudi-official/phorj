//! Lane L1c of the scout forcing function (2026-09-07) — **positional `array{…}` shapes**.
//!
//! PHP's array-shape docblock splits in two, and the two halves have very different answers in
//! phorj. A POSITIONAL shape (`array{int, string}`, or the same thing written with explicit
//! indices, `array{0: float, 1: float}`) is a TUPLE, which phorj shipped as DEC-288 — 12 sites
//! across a 120-file real codebase, and the shape `Core/Text.php` stops on. A KEYED shape
//! (`array{tenure: Tenure, source: string}`) needs a named-field tuple, which phorj does not have
//! yet: 73 sites, ruled as DEC-504, and it keeps its refusal here until that lands.

use super::lifter_tests::{assert_reparses, lift};

#[test]
fn a_positional_array_shape_is_a_tuple() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(): array { return [1, 'a']; }",
    );
    assert!(out.contains("(int, string)"), "{out}");
    assert_reparses(&out);
}

/// The same shape written with explicit numeric keys — PHPStan and Psalm both emit this form, and
/// scout has four of them. `0:`/`1:` in ascending order from zero IS positional.
#[test]
fn a_shape_with_explicit_ascending_indices_is_the_same_tuple() {
    let out = lift(
        "<?php\n/**\n * @return array{0: float, 1: float}\n */\nfunction point(): array { return [1.0, 2.0]; }",
    );
    assert!(out.contains("(float, float)"), "{out}");
    assert_reparses(&out);
}

/// `?array` + a positional shape is a nullable tuple — the shape scout's `Core/Text.php` actually
/// writes (`@return array{int, string}|null`).
#[test]
fn a_nullable_positional_shape_is_a_nullable_tuple() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}|null\n */\nfunction find(): ?array { return null; }",
    );
    assert!(out.contains("(int, string)?"), "{out}");
    assert_reparses(&out);
}

/// A KEYED shape still refuses, and the refusal now points at the ruling that will close it rather
/// than at a generic Tier-2 wall.
#[test]
fn a_keyed_array_shape_still_refuses_and_names_the_ruling() {
    let err = super::lifter::lift_source(
        "<?php\n/**\n * @return array{tenure: string, bp: int}\n */\nfunction j(): array { return []; }",
    )
    .expect_err("keyed shape");
    assert!(err.contains("named-field"), "{err}");
}

/// Indices that are not a dense ascending run from zero are NOT a tuple — refused rather than
/// silently reordered into one, which would be the lifter guessing (DEC-166).
#[test]
fn a_shape_with_non_positional_indices_refuses() {
    let err = super::lifter::lift_source(
        "<?php\n/**\n * @return array{1: int, 0: string}\n */\nfunction j(): array { return []; }",
    )
    .expect_err("out-of-order indices");
    assert!(
        err.contains("named-field") || err.contains("positional"),
        "{err}"
    );
}

/// A positional shape is only useful if the VALUES lift too: `return [$n, 'x'];` under
/// `@return array{int, string}` is a TUPLE literal, not a list literal. Without this the draft
/// lifts, and then `phg check` says `expected (int, string), found List<int>` — which is the
/// lifter having produced two halves that disagree with each other.
#[test]
fn a_list_literal_returned_as_a_positional_shape_becomes_a_tuple_literal() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(int $n): array { return [$n, 'x']; }",
    );
    assert!(out.contains("return (n, \"x\");"), "{out}");
    assert_reparses(&out);
}

/// The arity must MATCH. A literal of the wrong length is not silently padded or truncated into the
/// declared tuple — it is left as a list, and `phg check` then reports the disagreement, which is
/// the honest outcome (DEC-166: the lifter does not guess).
#[test]
fn a_list_literal_of_the_wrong_arity_is_left_alone() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(int $n): array { return [$n]; }",
    );
    assert!(out.contains("return [n];"), "{out}");
}

/// EVERY `return` answers to the declared return type, not only the ones at the top level. The
/// idiom scout's `Core/Text.php` actually writes puts the tuple arm inside an `if` and the `null`
/// arm after it, so a top-level-only seed emits a draft that lifts and then fails `phg check` with
/// `expected (int, string)?, found List<int>` — a two-halves-disagree bug, not a refusal.
#[test]
fn a_tuple_literal_returned_from_inside_a_block_is_seeded_too() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}|null\n */\nfunction find(bool $y): ?array { if ($y) { return [1, 'one']; } return null; }",
    );
    assert!(out.contains("return (1, \"one\");"), "{out}");
    assert!(out.contains("return null;"), "{out}");
    assert_reparses(&out);
}

// ── DEC-510: PHP array destructuring is phorj tuple destructuring ────────────────────────────────

/// `[$a, $b] = pair();` is the ONLY way to read a positional shape — phorj tuples have no index
/// access, only `var (a, b) = …`. It was deferred with `list()` in wave 2, before tuples were in
/// play; un-deferred 2026-09-07 because it is what makes a lifted positional shape usable at all.
/// 29 sites across 16 of scout's 120 files, and the whole of the report's "invalid assignment
/// target" row.
#[test]
fn php_array_destructuring_becomes_tuple_destructuring() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(): array { return [1, 'a']; }\nfunction main(): void { [$a, $b] = pair(); }",
    );
    assert!(out.contains("var (a, b) = pair();"), "{out}");
    assert_reparses(&out);
}

/// The older `list(...)` spelling means exactly the same thing and lifts the same way.
#[test]
fn the_list_spelling_destructures_identically() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(): array { return [1, 'a']; }\nfunction main(): void { list($a, $b) = pair(); }",
    );
    assert!(out.contains("var (a, b) = pair();"), "{out}");
    assert_reparses(&out);
}

/// A KEYED destructure (`['k' => $v] = $m`) is a different construct — it reads a map by key, not a
/// tuple by position — and is refused rather than silently treated as positional.
#[test]
fn a_keyed_destructure_is_refused() {
    let err =
        super::lifter::lift_source("<?php\nfunction main(): void { $m = []; ['k' => $v] = $m; }")
            .expect_err("keyed destructure");
    assert!(err.contains("key") || err.contains("positional"), "{err}");
}

// ── DEC-511: PHP `.` concatenation is a phorj interpolation ──────────────────────────────────────

/// PHP's `.` COERCES its operands to string; phorj's `+` refuses to ("no coercion"). Lifting `.` to
/// `+` produced a draft that lifted and then failed `phg check` on ~207 sites in scout. phorj has
/// the faithful form already — interpolation stringifies each part exactly as PHP does.
#[test]
fn php_concat_lifts_to_one_interpolation() {
    let out = lift("<?php\nfunction f(int $n): string { return 'n=' . $n; }");
    assert!(out.contains(r#""n={n}""#), "{out}");
    assert!(!out.contains(" + "), "concat still lifted as `+`:\n{out}");
    assert_reparses(&out);
}

/// A CHAIN of `.` is ONE interpolation, not a nest of them — `'a' . $x . 'b' . $y` is four parts in
/// one string, which is what PHP's own evaluation produces.
#[test]
fn a_concat_chain_becomes_one_interpolation_not_a_nest() {
    let out = lift("<?php\nfunction f(int $x, int $y): string { return 'a' . $x . 'b' . $y; }");
    assert!(out.contains(r#""a{x}b{y}""#), "{out}");
    assert_reparses(&out);
}

/// Arithmetic `+` is untouched — only `.` changes. A lifter that turned every `+` into a string
/// would be catastrophically wrong and silently so.
#[test]
fn arithmetic_addition_is_not_touched() {
    let out = lift("<?php\nfunction f(int $a, int $b): int { return $a + $b; }");
    assert!(out.contains("return a + b;"), "{out}");
    assert_reparses(&out);
}

// ── strict `null` comparison, and `echo` of a bare variable ─────────────────────────────────────

/// `$x === null` is phorj's `is null` narrowing test, NOT an equality: the checker rejects
/// `T? == null` as a cross-type comparison, so the equality mapping produced a draft that lifted
/// and then failed `phg check`. Both orderings, because Yoda style is common in real PHP.
#[test]
fn a_strict_null_comparison_becomes_the_is_null_test() {
    let out = lift(
        "<?php\nfunction f(?string $x): bool { return $x === null; }\nfunction g(?string $x): bool { return null === $x; }",
    );
    assert_eq!(
        out.matches("instanceof null").count(),
        2,
        "both orderings must become the null test:\n{out}"
    );
    assert!(!out.contains("== null"), "still an equality:\n{out}");
    assert_reparses(&out);
}

/// `!==` is the same test negated. phorj has no `is not null`, so it is spelled `!(x is null)`.
#[test]
fn a_strict_not_null_comparison_is_the_negated_test() {
    let out = lift("<?php\nfunction f(?string $x): bool { return $x !== null; }");
    assert!(out.contains("!(x instanceof null)"), "{out}");
    assert_reparses(&out);
}

/// STRICT ONLY. PHP's loose `== null` is also true for `0`, `""`, `[]` and `false`, so it is a
/// different question; it stays an equality and the checker reports it, which is the honest
/// DEC-166 outcome — the lifter does not guess which of the five the author meant.
#[test]
fn a_loose_null_comparison_is_left_as_an_equality() {
    let out = lift("<?php\nfunction f(?string $x): bool { return $x == null; }");
    assert!(out.contains("x == null"), "{out}");
    assert!(!out.contains("instanceof null"), "{out}");
}

/// The whole point of the bare-variable change: the draft CHECKS. `[$n, $label] = pair(7); echo $n;`
/// is the shape a lifted positional shape produces, and `Output.print(n)` on an int is a type error.
#[test]
fn an_echo_of_an_int_variable_produces_a_draft_that_checks() {
    let out = lift(
        "<?php\n/**\n * @return array{int, string}\n */\nfunction pair(int $n): array { return [$n, \"x\"]; }\nfunction main(): void { [$n, $label] = pair(7); echo $n; echo $label; }",
    );
    assert!(out.contains(r#"Output.print("{n}")"#), "{out}");
    let prog = crate::cli::parse_program(&out).expect("parses");
    crate::cli::check_and_expand(&prog, &out).expect("the lifted draft type-checks");
}

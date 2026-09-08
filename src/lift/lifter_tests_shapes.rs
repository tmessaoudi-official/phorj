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

/// A KEYED shape lifts to the named-field tuple its own keys describe (DEC-504). This case asserted
/// the REFUSAL until the named-field slice landed; the shape scout writes is exactly this one.
#[test]
fn a_keyed_array_shape_lifts_to_the_named_tuple_it_names() {
    let out = lift(
        "<?php\n/**\n * @return array{tenure: string, bp: int}\n */\nfunction j(): array { return []; }",
    );
    assert!(out.contains("(tenure: string, bp: int)"), "{out}");
    assert_reparses(&out);
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

/// DEC-504 — a KEYED array shape is a NAMED-FIELD tuple, and the docblock's own keys are the field
/// names. This is the shape the scout corpus actually writes and the reason named tuples exist; it
/// was refused by name until the named-field slice landed.
#[test]
fn a_keyed_array_shape_is_a_named_tuple() {
    let out = lift(
        "<?php\n/**\n * @return array{bp: int, source: string}\n */\nfunction row(): array { return ['bp' => 1, 'source' => 'a']; }",
    );
    assert!(out.contains("(bp: int, source: string)"), "{out}");
    assert_reparses(&out);
}

/// The names come from the DOCBLOCK and are never invented (DEC-166), so a shape that keys only
/// SOME of its fields is refused rather than half-named — naming the rest by position would be the
/// lifter guessing. (`array{bp: int, string}` is also not a thing PHPStan emits.)
#[test]
fn a_half_keyed_array_shape_is_refused_rather_than_half_named() {
    let err = super::lifter::lift_source(
        "<?php\n/**\n * @return array{bp: int, string}\n */\nfunction row(): array { return ['bp' => 1, 'a']; }",
    )
    .expect_err("half-keyed shape");
    assert!(
        err.contains("every field") && err.contains("DEC-166"),
        "expected a refusal naming the half-keyed shape, got: {err}"
    );
}

/// A PHP array key is an arbitrary string; a phorj field name is not. A key that cannot BE a field
/// name is refused rather than mangled into one — renaming it would make the lifted code disagree
/// with the PHP it came from at every read site (DEC-166).
#[test]
fn an_array_shape_key_that_is_not_a_legal_field_name_is_refused() {
    let err = super::lifter::lift_source(
        "<?php\n/**\n * @return array{'total-cost': int, source: string}\n */\nfunction row(): array { return []; }",
    )
    .expect_err("illegal shape key");
    assert!(err.contains("legal phorj field name"), "{err}");
}

/// DEC-504, the read side — and the whole point of the lift leg. A keyed shape lifts the SIGNATURE
/// to a named tuple, so the body's `$row['bp']` must lift to `row.bp`: left as a string index it
/// would be a Map read against a tuple, and the lifted draft would not CHECK. The field names come
/// from the docblock the parameter already carries — nothing is invented (DEC-166).
#[test]
fn a_string_index_into_a_keyed_shape_param_becomes_a_field_read() {
    let out = lift(
        "<?php\n/**\n * @param array{bp: int, source: string} $row\n */\nfunction bp(array $row): int { return $row['bp'] + 1; }",
    );
    assert!(out.contains("row.bp"), "expected a field read, got: {out}");
    assert!(!out.contains("row[\"bp\"]"), "still a string index: {out}");
    assert_reparses(&out);
}

/// A key the shape does NOT declare stays an index read — the lifter does not invent a field, and
/// the checker is the right place for that mistake to surface.
#[test]
fn a_string_index_that_is_not_a_declared_field_is_left_alone() {
    let out = lift(
        "<?php\n/**\n * @param array{bp: int} $row\n */\nfunction f(array $row): int { return $row['nope']; }",
    );
    assert!(out.contains("row[\"nope\"]"), "{out}");
}

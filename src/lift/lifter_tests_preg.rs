//! Scout row 4l-a (DEC-540) — a `preg_match` existence test lifts onto `Core.Regex`. The pattern
//! translation itself is pinned in `lifter/preg/tests.rs`; its equivalence with real PCRE by the PHP
//! oracle in `tests/lift_preg.rs`. These pin the call site: the comparison forms, `self::X`, the
//! engine choice, the import, and the refusals that are about the CALL rather than the pattern.

use super::lifter::lift_source;

fn checks_clean(out: &str) {
    let prog =
        crate::cli::parse_program(out).unwrap_or_else(|e| panic!("draft parses: {e:?}\n{out}"));
    crate::cli::check_and_expand(&prog, out)
        .unwrap_or_else(|e| panic!("draft type-checks: {e:?}\n{out}"));
}

fn lift(php: &str) -> String {
    lift_source(php).unwrap_or_else(|e| panic!("lift: {e}"))
}

/// twes-in's own shape: a typed class constant named through `self::`, compared with `1`.
#[test]
fn a_self_constant_pattern_compared_with_one_lifts() {
    let out = lift(
        r#"<?php
final class Money {
    private const string AMOUNT = '/^(0|[1-9][0-9]{0,10})(\.[0-9]{1,3})?$/';
    public static function valid(string $v): bool { return 1 === preg_match(self::AMOUNT, $v); }
    public static function invalid(string $v): bool { return preg_match(self::AMOUNT, $v) !== 1; }
}"#,
    );
    checks_clean(&out);
    assert!(out.contains("import Core.Regex;"), "{out}");
    assert!(
        out.contains(r#"return Regex.matches(Regex.compile("^(0|[1-9][0-9]\{0,10\})(\\.[0-9]\{1,3\})?\\n?\\z"), v);"#),
        "{out}"
    );
    assert!(
        out.contains("return !Regex.matches(Regex.compile("),
        "{out}"
    );
}

/// `=== 0` asks "no match"; `!= 0` asks "a match".
#[test]
fn a_comparison_with_zero_flips_the_test() {
    let out = lift(
        r#"<?php
function none(string $s): bool { return preg_match('/x/', $s) === 0; }
function some(string $s): bool { return 0 != preg_match('/x/', $s); }"#,
    );
    checks_clean(&out);
    assert!(
        out.contains(r#"return !Regex.matches(Regex.compile("x"), s);"#),
        "{out}"
    );
    assert!(
        out.contains(r#"return Regex.matches(Regex.compile("x"), s);"#),
        "{out}"
    );
}

/// Look-around needs the backtracking engine; a regular pattern keeps the linear one.
#[test]
fn the_engine_follows_the_pattern() {
    let out = lift(
        r#"<?php
function word(string $s): bool { return preg_match('/(?<![a-z])rdc(?![a-z])/u', $s) === 1; }"#,
    );
    checks_clean(&out);
    assert!(out.contains("Regex.compileBacktracking("), "{out}");
}

#[test]
fn a_pattern_not_known_at_lift_time_is_refused_by_name() {
    let dynamic = lift_source(
        "<?php\nfunction f(string $p, string $s): bool { return preg_match($p, $s) === 1; }",
    )
    .expect_err("a dynamic pattern is refused");
    assert!(dynamic.contains("known at lift time"), "{dynamic}");

    let built = lift_source(
        "<?php\nfunction f(string $w, string $s): bool { return preg_match('/^' . $w . '$/', $s) === 1; }",
    )
    .expect_err("a pattern concatenated with a variable is refused");
    assert!(built.contains("known at lift time"), "{built}");

    let late = lift_source(
        "<?php\nclass A { const P = '/x/'; public function f(string $s): bool { return preg_match(static::P, $s) === 1; } }",
    )
    .expect_err("static:: is refused");
    // Refused by the PHP parser already (late static binding is Tier-2) — never read as `self::`.
    assert!(late.contains("static::"), "{late}");

    let other = lift_source(
        "<?php\nclass A { public function f(string $s): bool { return preg_match(B::P, $s) === 1; } }",
    )
    .expect_err("another class's constant is refused");
    assert!(other.contains("another class"), "{other}");
}

/// A pattern the translation refuses fails the lift with the translation's reason.
#[test]
fn a_refused_pattern_names_its_reason() {
    let err = lift_source(
        "<?php\nfunction f(string $s): bool { return preg_match('/^[^0-9]$/', $s) === 1; }",
    )
    .expect_err("a byte-counting pattern is refused");
    assert!(err.contains("add `u`"), "{err}");
}

/// twes-in's token check: a pattern spelled from a literal and an int constant of the class is fixed,
/// so it folds at lift time (`2 * 32` → `64`).
#[test]
fn a_pattern_built_from_class_constants_folds() {
    let out = lift(
        r#"<?php
final class Token {
    private const int RAW_BYTES = 32;
    public static function valid(string $raw): bool {
        return 1 === preg_match('/^[0-9a-f]{'.(2 * self::RAW_BYTES).'}$/', $raw);
    }
}"#,
    );
    checks_clean(&out);
    assert!(
        out.contains(r#"Regex.compile("^[0-9a-f]\{64\}\\n?\\z")"#),
        "{out}"
    );
}

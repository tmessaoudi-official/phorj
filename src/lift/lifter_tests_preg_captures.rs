//! Scout row 4l-b2 (DEC-554) — a captures `preg_match` lifts onto `Regex.first`. The equivalence with
//! real PCRE is `tests/lift_preg_captures.rs`; these pin the call site: the hoist, twes-in's guard
//! shape with a `self::` pattern and a keyword-named array, and every refusal by name.

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

fn refused(php: &str, want: &str) {
    let err = lift_source(php).expect_err("must be refused");
    assert!(err.contains(want), "want `{want}` in: {err}");
}

/// twes-in's shape: a `self::` pattern, a guard whose `||` reads a group, and `$match` — a phorj
/// keyword, renamed before the hoist sees it.
#[test]
fn a_guard_with_a_self_pattern_and_a_keyword_array_lifts() {
    let out = lift(
        r#"<?php
final class Inbox {
    private const string CHANNEL = '/^(?<kind>[a-z]+):(\d+)$/';
    public static function id(string $c): string {
        if (1 !== preg_match(self::CHANNEL, $c, $match) || $match[2] === '0') {
            return '';
        }
        return $match['kind'] . '#' . $match[2];
    }
}"#,
    );
    checks_clean(&out);
    assert!(
        out.contains("RegexMatch? matchValue = Regex.first(Regex.compile("),
        "{out}"
    );
    assert!(
        out.contains(r#"if (matchValue instanceof null || (matchValue.at(2) ?? "") == "0")"#),
        "{out}"
    );
    assert!(
        out.contains(r#"matchValue.group(\"kind\") ?? \"\""#),
        "{out}"
    );
}

/// A second site on the same array assigns instead of redeclaring.
#[test]
fn a_second_site_assigns_the_same_variable() {
    let out = lift(
        r#"<?php
function f(string $s): string {
    if (preg_match('/^a(b)$/', $s, $m) === 1) { return $m[1]; }
    if (preg_match('/^c(d)$/D', $s, $m) === 1) { return $m[1]; }
    return '';
}"#,
    );
    checks_clean(&out);
    assert_eq!(out.matches("RegexMatch? m =").count(), 1, "{out}");
    assert!(out.contains("    m = Regex.first("), "{out}");
}

#[test]
fn a_site_behind_another_test_is_refused_by_position() {
    refused(
        r#"<?php
function f(?string $s): string {
    if (null === $s || 1 !== preg_match('/^(a)$/', $s, $m)) { return ''; }
    return $m[1];
}"#,
        "FIRST test",
    );
}

#[test]
fn a_loop_condition_is_refused_by_position() {
    refused(
        r#"<?php
function f(string $s): int {
    $n = 0;
    while (preg_match('/^(a)/', $s, $m) === 1) { $s = substr($s, 1); $n = $n + 1; }
    return $n;
}"#,
        "FIRST test",
    );
}

#[test]
fn any_other_use_of_the_array_is_refused() {
    refused(
        r#"<?php
function f(string $s): int {
    if (1 !== preg_match('/^(a)(b)$/', $s, $m)) { return 0; }
    return count($m);
}"#,
        "holds `preg_match` captures",
    );
}

#[test]
fn a_computed_index_is_refused() {
    refused(
        r#"<?php
function f(string $s, int $i): string {
    if (1 !== preg_match('/^(a)(b)$/', $s, $m)) { return ''; }
    return $m[$i];
}"#,
        "literal group number or name",
    );
}

#[test]
fn a_group_past_the_last_and_an_unknown_name_are_refused() {
    let src = |read: &str| {
        format!(
            "<?php\nfunction f(string $s): string {{\n    \
             if (1 !== preg_match('/^(?<a>x)$/', $s, $m)) {{ return ''; }}\n    return {read};\n}}"
        )
    };
    refused(&src("$m[2]"), "the pattern has 1 group(s)");
    refused(&src("$m['b']"), "no group of that name");
}

/// `isset` tells a middle group (set to `""`) from a trailing one (unset): only the last group lifts.
#[test]
fn isset_lifts_on_the_last_group_only() {
    let src = |k: u8| {
        format!(
            "<?php\nfunction f(string $s): bool {{\n    \
             if (1 !== preg_match('/^(a)?(b)?$/', $s, $m)) {{ return false; }}\n    \
             return isset($m[{k}]);\n}}"
        )
    };
    let out = lift(&src(2));
    checks_clean(&out);
    assert!(out.contains("return !(m.at(2) instanceof null);"), "{out}");
    refused(&src(1), "`isset` on `$m[1]`");
}

/// Two patterns with different groups in one array: a fallback cannot know which is the last group.
#[test]
fn a_fallback_is_refused_when_two_patterns_share_the_array() {
    refused(
        r#"<?php
function f(string $s): string {
    if (preg_match('/^(a)$/', $s, $m) === 1) { return 'a'; }
    if (1 !== preg_match('/^(b)(c)?$/D', $s, $m)) { return ''; }
    return $m[2] ?? 'none';
}"#,
        "a `??` fallback other than `''`",
    );
}

/// Panel, row 4l-b3: a trailing `$` lifts to the consuming `\n?\z`, so the WHOLE match would carry the
/// final newline PHP stops before. A group read is unaffected; `$m[0]` is refused unless `D`/`\z`.
#[test]
fn the_whole_match_of_a_rewritten_dollar_is_refused_but_a_group_is_not() {
    refused(
        r#"<?php
function f(string $s): string {
    if (preg_match('/[a-z]+$/u', $s, $m) === 1) { return $m[0]; }
    return '';
}"#,
        "final newline",
    );
    for ok in [
        "if (preg_match('/([a-z]+)$/u', $s, $m) === 1) { return $m[1]; }",
        "if (preg_match('/[a-z]+$/uD', $s, $m) === 1) { return $m[0]; }",
        "if (preg_match('/[a-z]+\\z/u', $s, $m) === 1) { return $m[0]; }",
    ] {
        lift(&format!(
            "<?php\nfunction f(string $s): string {{\n    {ok}\n    return '';\n}}"
        ));
    }
}

/// Round 2 (N6): a read of `$m[0]` lifted before a later `$` site was judged without knowing about it,
/// so only a variable's FIRST site may rewrite `$`.
#[test]
fn a_later_site_with_a_rewritten_dollar_on_a_filled_variable_is_refused() {
    refused(
        r#"<?php
function f(string $a, string $b): string {
    $out = '';
    if (preg_match('/[a-z]+/u', $a, $m) === 1) { $out = $m[0]; }
    if (preg_match('/([a-z]+)$/u', $b, $m) === 1) { $out = $out . $m[1]; }
    return $out;
}"#,
        "filled again",
    );
}

#[test]
fn an_empty_group_name_is_refused() {
    refused(
        r#"<?php
function f(string $s): string {
    if (preg_match('/(a)/u', $s, $m) === 1) { return $m['']; }
    return '';
}"#,
        "names no group",
    );
}

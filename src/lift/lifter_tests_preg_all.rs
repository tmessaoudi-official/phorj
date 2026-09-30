//! Scout row 4l-b3 (DEC-554) — `preg_match_all` lifts onto `Regex.all`. The equivalence with real
//! PCRE is `tests/lift_preg_all.rs`; these pin the call site: the hoisted `List<RegexMatch>`, the
//! count test, the column reads and every refusal by name.

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

/// scout `Heating`/`Hail` shape: a count guard, then `$m[0]` as a column of `[text, offset]` pairs.
#[test]
fn a_count_guard_and_an_offset_column_lift() {
    let out = lift(
        r#"<?php
final class H {
    private const string MENTION = '/chauffage/u';
    public static function at(string $s): int {
        if (preg_match_all(self::MENTION, $s, $m, PREG_OFFSET_CAPTURE) < 1) {
            return -1;
        }
        foreach ($m[0] as [$word, $at]) {
            return $at + strlen($word);
        }
        return -1;
    }
}"#,
    );
    checks_clean(&out);
    assert!(
        out.contains("List<RegexMatch> m = Regex.all(Regex.compile("),
        "{out}"
    );
    assert!(out.contains("if (m.length() < 1)"), "{out}");
    assert!(
        out.contains("(regexMatch.full(), regexMatch.start())"),
        "{out}"
    );
}

/// scout `EmailMessage`: a bare statement, then `$m[0]` as a column of strings.
#[test]
fn a_bare_statement_and_a_text_column_lift() {
    let out = lift(
        r#"<?php
/** @return list<string> */
function links(string $body): array {
    /** @var list<string> $out */
    preg_match_all('~https?://\S+~u', $body, $matches);
    $out = [];
    foreach ($matches[0] as $link) {
        $out[] = $link;
    }
    return $out;
}"#,
    );
    checks_clean(&out);
    assert!(
        out.contains("List<RegexMatch> matches = Regex.all("),
        "{out}"
    );
    assert!(out.contains("matches.map("), "{out}");
    // The bare statement leaves nothing behind — no stray empty block.
    let seen = out.find("mutable var out").expect("the next statement");
    let before = &out[..seen];
    let end = before.rfind(';').expect("the hoisted declaration ends");
    assert!(before[end + 1..].trim().is_empty(), "stray text: {out}");
}

/// A group column reads `at(k) ?? ""` — PATTERN_ORDER fills a group that sat out with `""`.
#[test]
fn a_group_column_and_a_named_column_lift() {
    let out = lift(
        r#"<?php
/** @return list<list<string>> */
function locs(string $xml): array {
    if (preg_match_all('~<loc>\s*(?<u>[^<\s]+)\s*</loc>~u', $xml, $m) === 0) {
        return [];
    }
    return [$m[1], $m['u']];
}"#,
    );
    assert!(out.contains(r#"regexMatch.at(1) ?? """#), "{out}");
    assert!(out.contains(r#"regexMatch.group("u") ?? """#), "{out}");
}

#[test]
fn a_second_site_assigns_the_same_variable() {
    let out = lift(
        r#"<?php
function f(string $s): int {
    $n = preg_match_all('/a/u', $s, $m);
    $k = preg_match_all('/b/u', $s, $m);
    return $n + $k;
}"#,
    );
    checks_clean(&out);
    assert_eq!(out.matches("List<RegexMatch> m =").count(), 1, "{out}");
    assert!(out.contains("    m = Regex.all("), "{out}");
}

#[test]
fn set_order_and_unmatched_as_null_are_refused_by_name() {
    refused(
        r#"<?php
function f(string $s): int { preg_match_all('/a/u', $s, $m, PREG_SET_ORDER); return 0; }"#,
        "PREG_SET_ORDER",
    );
    refused(
        r#"<?php
function f(string $s): int { preg_match_all('/a/u', $s, $m, PREG_UNMATCHED_AS_NULL); return 0; }"#,
        "PREG_UNMATCHED_AS_NULL",
    );
}

#[test]
fn a_site_behind_another_test_is_refused_by_position() {
    refused(
        r#"<?php
function f(?string $s): int {
    if (null === $s || preg_match_all('/a/u', $s, $m) === 0) { return 0; }
    return 1;
}"#,
        "FIRST test",
    );
}

#[test]
fn a_dynamic_pattern_is_refused_by_name() {
    refused(
        r#"<?php
function f(string $p, string $s): int { return preg_match_all($p, $s, $m); }"#,
        "known at lift time",
    );
}

#[test]
fn any_other_use_of_the_array_is_refused() {
    refused(
        r#"<?php
function f(string $s): int {
    preg_match_all('/x(a)/u', $s, $m);
    return count($m);
}"#,
        "holds `preg_match_all` matches",
    );
}

#[test]
fn a_group_past_the_last_and_a_computed_index_are_refused() {
    refused(
        r#"<?php
function f(string $s): array { preg_match_all('/x(a)/u', $s, $m); return $m[2]; }"#,
        "no such column",
    );
    refused(
        r#"<?php
function f(string $s, int $i): array { preg_match_all('/x(a)/u', $s, $m); return $m[$i]; }"#,
        "literal group number or name",
    );
}

/// `translate` promises the same EXISTENCE answer; every match needs more (see `match_all.rs`).
#[test]
fn a_pattern_whose_matches_could_differ_is_refused_by_name() {
    let call = |pat: &str| {
        format!("<?php\nfunction f(string $s): int {{ return preg_match_all('{pat}', $s, $m); }}")
    };
    refused(&call("/\\d*/u"), "may match the empty string");
    refused(&call("/(a)|b*/u"), "may match the empty string");
    refused(&call("/(?=a)|a/u"), "may match the empty string");
    refused(&call("/./"), "without the `u` modifier");
    refused(&call("/[^ ]/"), "without the `u` modifier");
    refused(&call("/a$/u"), "`$`");
    refused(&call("/a\\Z/u"), "`$`");
}

/// A pattern with a guaranteed-consuming part, a `\z` anchor or the `D` modifier still lifts.
#[test]
fn consuming_patterns_with_z_or_d_still_lift() {
    for pat in [
        "/chauffage/",
        "/a+/u",
        "/x(a)?/u",
        "/[a-z]+\\z/u",
        "/[a-z]+$/uD",
        "/(?<![a-z])rdc(?![a-z])/u",
    ] {
        lift(&format!(
            "<?php\nfunction f(string $s): int {{ return preg_match_all('{pat}', $s, $m); }}"
        ));
    }
}

/// One `$m` for both a captures `preg_match` and a `preg_match_all` would be two types in one variable.
#[test]
fn one_variable_for_captures_and_matches_is_refused_by_name() {
    refused(
        r#"<?php
function f(string $s): int {
    preg_match_all('/x(a)/u', $s, $m);
    if (preg_match('/x(b)/u', $s, $m) === 1) { return 1; }
    return 0;
}"#,
        "receives both",
    );
    refused(
        r#"<?php
function f(string $s): int {
    if (preg_match('/x(b)/u', $s, $m) === 1) { return 1; }
    preg_match_all('/x(a)/u', $s, $m);
    return 0;
}"#,
        "receives both",
    );
}

/// A run-time pattern names `preg_*`, not the one function the message used to hard-code.
#[test]
fn the_dynamic_pattern_refusal_does_not_name_preg_match() {
    let err = lift_source(
        "<?php\nfunction f(string $p, string $s): int { return preg_match_all($p, $s, $m); }",
    )
    .expect_err("refused");
    assert!(err.contains("a `preg_*` call needs a pattern"), "{err}");
}

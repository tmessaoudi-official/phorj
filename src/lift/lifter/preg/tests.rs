//! Row 4l-a — the pattern translation, one case per rule. Each accepted case states the exact bare
//! pattern `Core.Regex` receives; each refused case names the rule in its message. The equivalence
//! with real PCRE is certified separately, by executing both under the PHP oracle (`tests/lift_preg.rs`).

use super::translate;

fn ok(php: &str, want: &str, backtracking: bool) {
    let t = translate(php).unwrap_or_else(|e| panic!("{php}: {e}"));
    assert_eq!(t.pattern, want, "{php}");
    assert_eq!(t.backtracking, backtracking, "{php}: engine");
}

fn refused(php: &str, names: &str) {
    let e = translate(php).expect_err(php);
    assert!(e.contains(names), "{php}: expected `{names}` in: {e}");
}

/// PHP's `$` also matches before a FINAL newline. At the end of a top-level alternative that is the
/// consuming `\n?\z` — exact for an existence test, and still a regular pattern.
#[test]
fn a_trailing_dollar_is_before_a_final_newline() {
    ok(
        r"/^(0|[1-9][0-9]{0,10})(\.[0-9]{1,3})?$/",
        r"^(0|[1-9][0-9]{0,10})(\.[0-9]{1,3})?\n?\z",
        false,
    );
    ok(r"/a$|b/", r"a\n?\z|b", false);
    ok(r"/^foo\Z/", r"^foo\n?\z", false);
    ok(r"/^foo\Z/D", r"^foo\n?\z", false);
}

/// Anywhere else the `$` is a look-ahead, which needs the backtracking engine.
#[test]
fn an_inner_dollar_is_a_lookahead() {
    ok(r"/(a$)/", r"(a(?=\n?\z))", true);
}

/// `D` makes PHP's `$` the very end, which is what `$` already means to `Core.Regex`.
#[test]
fn the_d_modifier_keeps_the_dollar() {
    ok(r"/^abc$/D", r"^abc$", false);
}

#[test]
fn delimiters_are_stripped_and_an_escaped_delimiter_unescaped() {
    ok(
        r"#^[a-z0-9.+-]+/[a-z0-9.+-]+$#",
        r"^[a-z0-9.+-]+/[a-z0-9.+-]+\n?\z",
        false,
    );
    ok(r"/a\/b/", r"a/b", false);
    ok(r"~^a\~b~", r"^a\~b", false); // `~` is meta to regex-syntax: its escape is kept
    ok(r"{^x}", r"^x", false);
}

/// Without `u`, PCRE's `\d` `\w` `\s` are ASCII (its default C-locale tables); phorj's are Unicode.
#[test]
fn ascii_shorthands_are_spelled_out_without_u() {
    ok(r"/^\d{4}$/", r"^[0-9]{4}\n?\z", false);
    ok(r"/^[\d.]+$/", r"^[0-9.]+\n?\z", false);
    ok(r"/\s/", r"[\t\n\x0B\f\r ]", false);
    ok(r"/\w/", r"[A-Za-z0-9_]", false);
}

/// With `u` PCRE is Unicode too (PHP turns on UCP), so the pattern passes through.
#[test]
fn a_unicode_pattern_passes_through() {
    ok(r"/^\w+$/u", r"^\w+\n?\z", false);
    ok(r"/abc/iu", r"(?i)abc", false);
    ok(
        r"/(?<![a-z0-9])foo(?![a-z0-9])/u",
        r"(?<![a-z0-9])foo(?![a-z0-9])",
        true,
    );
}

/// A byte atom (`.`, a negated class) matches one BYTE in PCRE's byte mode and one CHARACTER in
/// phorj. As an unbounded run between character boundaries — or alone, unanchored — the existence
/// answer is the same, so it lifts.
#[test]
fn a_byte_atom_lifts_where_the_existence_answer_cannot_differ() {
    ok(r"/[^a-z0-9_-]+/", r"[^a-z0-9_-]+", false);
    ok(r"/^a.*c$/s", r"(?s)^a.*c\n?\z", false);
    ok(
        r#"/[^\x20-\x7E]|[%\/\\"]/"#,
        r#"[^\x20-\x7E]|[%/\\"]"#,
        false,
    );
}

#[test]
fn a_byte_atom_that_counts_bytes_is_refused() {
    refused(r"/^[^0-9]$/", "add `u`");
    refused(r"/a[^b]c/", "add `u`");
    refused(r"/(.+)x/", "add `u`");
    refused(r"/^.+.+$/", "add `u`");
    refused(r"/^\D$/", "add `u`");
}

#[test]
fn what_byte_mode_reads_differently_is_refused_by_name() {
    refused(r"/k/i", "`i`");
    refused(r"/a(?i)b/", "`i`");
    refused(r"/\bfoo\b/", r"`\b`");
    refused("/é/", "non-ASCII");
    refused(r"/[\x80-\xff]/", r"`\x`");
    refused(r"/\p{L}/", r"`\p`");
}

#[test]
fn modifiers_without_a_faithful_form_are_refused() {
    refused(r"/^$/m", "`m`");
    refused(r"/a b/x", "`x`");
    refused(r"/a/U", "`U`");
}

#[test]
fn malformed_and_pcre_only_patterns_are_refused() {
    refused(r"/abc", "delimiter");
    refused(r"abc", "delimiter");
    refused(r"/\12/", "back-reference");
    refused(r"/\Qa\E/u", "PCRE-only");
    refused(r"/a{,3}/u", "{,n}");
}

/// A group NAME is not a flag run: `(?<kind>` must not read as the `i` flag, nor `(?<mix>` as `m`/`x`.
#[test]
fn a_group_name_is_not_read_as_flags() {
    ok(r"/(?<kind>a)(?<mix>b)/", r"(?<kind>a)(?<mix>b)", false);
    ok(r"/(?P<index>a)/", r"(?P<index>a)", false);
    ok(r"/(?<![a-z])x/u", r"(?<![a-z])x", true);
}

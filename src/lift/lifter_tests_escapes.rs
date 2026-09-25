//! Row 5n — §LIFT-UNICODE-ESCAPE: PHP string escapes decode exactly as PHP decodes them, in both
//! quote styles and on both lexer paths (a plain string and an interpolating one share ONE decoder).
//! Every expected byte string below was MEASURED with the resolved PHP 8.5 oracle (`bin2hex`,
//! 2026-09-25) — none is taken from the lifter. What phorj's UTF-8 strings cannot hold is refused by
//! name, never guessed at.

use super::lexer::{lex_php, PTok};
use super::lifter::lift_source;
use super::parser::parse_php;

/// The decoded bytes of the one plain string literal in `<?php <lit>;`, as lowercase hex.
fn plain_hex(lit: &str) -> String {
    let toks = lex_php(&format!("<?php {lit};")).unwrap_or_else(|e| panic!("{lit}: {e}"));
    let s = toks
        .into_iter()
        .find_map(|t| match t.tok {
            PTok::Str(s) => Some(s),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{lit}: no plain string token"));
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn single_quoted_strings_decode_only_backslash_and_quote() {
    for (lit, php) in [
        (r"'a\nb'", "615c6e62"),
        (r#"'a\"b'"#, "615c2262"),
        (r"'a\0b'", "615c3062"),
        (r"'a\tb'", "615c7462"),
        (r"'a\'b'", "612762"),
        (r"'a\\b'", "615c62"),
        (r"'a\u{2019}b'", "615c757b323031397d62"),
    ] {
        assert_eq!(plain_hex(lit), php, "{lit}");
    }
}

#[test]
fn double_quoted_strings_decode_every_php_escape() {
    for (lit, php) in [
        (r#""a\u{2019}b""#, "61e2809962"),
        (r#""a\u{41}b""#, "614162"),
        (r#""a\u{0041}b""#, "614162"),
        (r#""a\u{10FFFF}b""#, "61f48fbfbf62"),
        (r#""a\xE2\x80\x99b""#, "61e2809962"),
        (r#""a\x41b""#, "614162"),
        (r#""a\x4b""#, "614b"),
        (r#""a\x4g""#, "610467"),
        (r#""a\101b""#, "614162"),
        (r#""a\7b""#, "610762"),
        (r#""a\012b""#, "610a62"),
        (r#""a\0b""#, "610062"),
        (r#""a\08b""#, "61003862"),
        (r#""a\vb""#, "610b62"),
        (r#""a\eb""#, "611b62"),
        (r#""a\fb""#, "610c62"),
        (r#""a\nb""#, "610a62"),
        (r#""a\$b""#, "612462"),
        (r#""a\"b""#, "612262"),
        (r#""a\\b""#, "615c62"),
    ] {
        assert_eq!(plain_hex(lit), php, "{lit}");
    }
}

#[test]
fn a_double_quoted_non_escape_keeps_its_backslash() {
    // `\'`, `\{`, `\q`, a bare `\u` and a `\x` with no hex digit are not escapes in a double-quoted
    // PHP string: the backslash stays.
    for (lit, php) in [
        (r#""a\'b""#, "615c2762"),
        (r#""a\{b""#, "615c7b62"),
        (r#""a\qb""#, "615c7162"),
        (r#""a\ub""#, "615c7562"),
        (r#""a\xzb""#, "615c787a62"),
    ] {
        assert_eq!(plain_hex(lit), php, "{lit}");
    }
}

#[test]
fn an_interpolated_string_decodes_through_the_same_table() {
    // PHP: `"a\{$x}\u{2019}\$"` with `$x = "X"` is `a\{X}’$` — `\{` is NOT an escaped hole (PHP
    // has none; the backslash stays and `$x` still interpolates), `\u{…}` decodes, `\$` is `$`.
    let src = r#"<?php $x = "X"; $s = "a\{$x}\u{2019}\$\x41";"#;
    let prog = parse_php(lex_php(src).unwrap()).unwrap();
    let dbg = format!("{prog:?}");
    assert!(dbg.contains(r#"Lit("a\\{")"#), "{dbg}");
    assert!(dbg.contains("Lit(\"}\u{2019}$A\")"), "{dbg}");
}

#[test]
fn what_a_phorj_string_cannot_hold_is_refused_by_name() {
    for (lit, why) in [
        // PHP emits the raw byte 0x80 — not UTF-8.
        (r#""a\x80b""#, "UTF-8"),
        // PHP emits CESU bytes `eda080` for a surrogate — not UTF-8.
        (r#""a\u{D800}b""#, "UTF-8"),
        // PHP warns "Octal escape sequence overflow" and wraps to NUL.
        (r#""a\400b""#, "octal"),
        // PHP itself refuses these four at parse time.
        (r#""a\u{110000}b""#, "\\u{"),
        (r#""a\u{}b""#, "\\u{"),
        (r#""a\u{zz}b""#, "\\u{"),
        (r#""a\u{41b""#, "\\u{"),
    ] {
        let src = format!("<?php function q(): string {{ return {lit}; }}");
        let err = lift_source(&src).expect_err(lit);
        assert!(err.contains(why), "{lit}: {err}");
    }
}

#[test]
fn a_control_character_prints_as_a_unicode_escape() {
    // A decoded NUL/ESC/VT must reach the phorj draft as `\u{…}`, never as a raw control byte.
    let out = lift_source(r#"<?php function q(): string { return "a\0\e\v"; }"#).unwrap();
    assert!(out.contains(r#""a\u{0}\u{1B}\u{B}""#), "{out}");
}

#[test]
fn a_malformed_unicode_escape_is_reported_from_its_own_text() {
    // The lexer sees the whole file, so an unbounded scan for `}` would quote source past the end
    // of the string (`41b"; }`) in the refusal.
    let err = lift_source(r#"<?php function q(): string { return "a\u{41b"; }"#).unwrap_err();
    assert!(err.contains(r"\u{41b"), "{err}");
    assert!(!err.contains('"') && !err.contains(';'), "{err}");
}

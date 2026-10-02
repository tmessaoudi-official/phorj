//! `String.trimAscii` (DEC-545, named DEC-549, scout row 4n) — PHP's `trim()` with its default
//! character list, exactly: `" \t\n\r\0\x0B"` stripped from both ends, nothing else.
//!
//! `String.trim` strips Rust's Unicode White_Space set (UA-1.1), which is NOT PHP's: it also strips
//! U+00A0, U+3000, form feed (`\x0C`)…, and it keeps `\0`. In a byte-identity harness that difference
//! is a semantic change, so `trim($s)` could not lift to it and stayed unmapped. This native is the
//! PHP set, so `trim($s)` lifts here and transpiles back to `trim($s)` (ladder case 1).
//!
//! Also NOT Rust's `str::trim_ascii`: that set is `u8::is_ascii_whitespace` — it strips `\x0C` and
//! keeps `\0` and `\x0B`. The name says "ASCII"; the set is PHP's, and the unit tests pin each of the
//! three characters on which the two disagree.
//!
//! No UTF-8 hazard: every character in the set is ASCII, and an ASCII byte never occurs inside a
//! multi-byte UTF-8 sequence, so stripping by `char` strips exactly the bytes PHP strips.

use crate::native::*;
use crate::types::Ty;
use crate::value::Value;

/// PHP's default `trim()` character list.
const PHP_TRIM_SET: [char; 6] = [' ', '\t', '\n', '\r', '\0', '\u{0B}'];

pub(crate) fn trim_ascii(s: &str) -> &str {
    s.trim_matches(&PHP_TRIM_SET[..])
}

fn text_trim_ascii(args: &[Value], _: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(s)] => Ok(Value::Str(trim_ascii(s).into())),
        _ => Err("String.trimAscii expects (string)".into()),
    }
}

pub(super) fn trim_ascii_natives() -> Vec<NativeFn> {
    vec![NativeFn {
        module: "Core.String",
        name: "trimAscii",
        params: vec![Ty::String],
        ret: Ty::String,
        pure: true,
        eval: NativeEval::Pure(text_trim_ascii),
        // Only the ONE-argument form: `trim($s, $chars)` has a different arity, so the lifter's arity
        // check leaves it unmapped (DEC-545 rules the default set only).
        lift_from: &["trim"],
        // Explicit list, never the default: PHP 8.6 added `\f` to `trim()`'s default (rfc/trim_form_feed).
        php: |a| format!("trim({}, \" \\t\\n\\r\\0\\x0B\")", parg(a, 0)),
    }]
}

#[cfg(test)]
mod tests {
    use super::trim_ascii;

    #[test]
    fn strips_php_default_set_from_both_ends() {
        assert_eq!(trim_ascii(" \t\n\r\0\x0Bx y \t\n\r\0\x0B"), "x y");
        assert_eq!(trim_ascii(""), "");
        assert_eq!(trim_ascii(" \t "), "");
    }

    /// The three characters on which PHP's set and Rust's `trim_ascii` disagree, and the Unicode
    /// spaces `String.trim` would strip.
    #[test]
    fn keeps_what_php_keeps() {
        assert_eq!(trim_ascii("\x0Cx\x0C"), "\x0Cx\x0C"); // form feed: Rust strips, PHP keeps
        assert_eq!(trim_ascii("\0x\x0B"), "x"); // NUL, VT: PHP strips, Rust keeps
        assert_eq!(trim_ascii("\u{A0}x\u{3000}"), "\u{A0}x\u{3000}");
        assert_eq!(trim_ascii(" é\u{0301} "), "é\u{0301}");
    }
}

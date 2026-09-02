//! Tokenizer tests — string literals, interpolation segments and raw strings. Split out of
//! `tests.rs` (Invariant 13: ratcheted at its baseline), reusing its `kinds`/`lit` helpers.

use super::tests::{kinds, lit};
use crate::token::{StrSeg, TokenKind};

#[test]
fn string_literals() {
    use TokenKind::*;
    assert_eq!(kinds("\"hello\""), vec![lit("hello"), Eof]);
    // escapes
    assert_eq!(kinds("\"a\\nb\\t\\\"c\""), vec![lit("a\nb\t\"c"), Eof]);
    // interpolation is now split by the tokenizer into literal + interp segments.
    assert_eq!(
        kinds("\"Hello {name}\""),
        vec![
            Str(vec![
                StrSeg::Lit("Hello ".into()),
                StrSeg::Interp("name".into(), 8, 1, 9)
            ]),
            Eof
        ]
    );
}

#[test]
fn literal_braces_via_backslash() {
    use TokenKind::*;
    // `\{` / `\}` are literal braces — a single literal segment, no interpolation.
    assert_eq!(kinds(r#""\{x\}""#), vec![lit("{x}"), Eof]);
    // mixed: literal braces around a real interpolation.
    assert_eq!(
        kinds(r#""\{{n}\}""#),
        vec![
            Str(vec![
                StrSeg::Lit("{".into()),
                StrSeg::Interp("n".into(), 4, 1, 5),
                StrSeg::Lit("}".into())
            ]),
            Eof
        ]
    );
}

#[test]
fn nested_string_literal_in_interpolation() {
    use TokenKind::*;
    // A double-quoted string inside an interpolation expression is consumed verbatim — its inner `"`
    // does NOT close the outer string (M-DOGFOOD W2). Inner source: `f("x")`, content starts at 7.
    assert_eq!(
        kinds(r#""call {f("x")}""#),
        vec![
            Str(vec![
                StrSeg::Lit("call ".into()),
                StrSeg::Interp(r#"f("x")"#.into(), 7, 1, 8),
            ]),
            Eof
        ]
    );
    // A `}` (or `{`) inside the nested string is literal — it must not close the interpolation.
    assert_eq!(
        kinds(r#""{f("a}b")}""#),
        vec![
            Str(vec![StrSeg::Interp(r#"f("a}b")"#.into(), 2, 1, 3)]),
            Eof
        ]
    );
    // An escaped quote inside the nested string is kept verbatim in the inner source (re-lexed later).
    assert_eq!(
        kinds(r#""{f("a\"b")}""#),
        vec![
            Str(vec![StrSeg::Interp(r#"f("a\"b")"#.into(), 2, 1, 3)]),
            Eof
        ]
    );
}

#[test]
fn raw_strings() {
    use TokenKind::*;
    // No escapes, no interpolation — every byte literal.
    assert_eq!(kinds(r#"r"a\n{x}b""#), vec![lit(r"a\n{x}b"), Eof]);
    // `#`-delimited raw string carries embedded quotes.
    assert_eq!(kinds(r##"r#"say "hi""#"##), vec![lit(r#"say "hi""#), Eof]);
    // a bare `r` / `rx` is an ordinary identifier, not a raw string.
    assert_eq!(kinds("r"), vec![Ident("r".into()), Eof]);
}

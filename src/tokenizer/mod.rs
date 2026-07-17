//! Hand-written tokenizer: source `&str` → `Vec<Token>`. Iterative (no recursion), so unlike the
//! parser/checker it never contributes to the recursion-depth budget those stages guard. Faults
//! surface as a unified `diagnostic::Diagnostic` (`Stage::Lex`) carrying line/col.

use crate::diagnostic::{Diagnostic, Stage};
use crate::token::{Comment, CommentKind, Span, StrSeg, Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

mod ident;
mod scan;
mod strings;

/// Parse the numeric TEXT of a `…d` decimal literal (the part before the `d`, e.g. `"1.50"`,
/// `"100"`, `"1_000.5"`) into `(unscaled, scale)` (M-NUM S1). Underscore separators are stripped (a
/// source literal may use them, unlike the runtime `Decimal.of` grammar). The scale is the count of
/// fractional digits, so trailing zeros are preserved. Returns `None` if the unscaled value overflows
/// `i128` (a compile-time error, not a runtime fault). No sign handling — the tokenizer scans the
/// magnitude; a leading `-` is the unary-minus operator on the literal.
fn parse_decimal_literal(text: &str) -> Option<(i128, u8)> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((i, f)) => (i, f),
        None => (cleaned.as_str(), ""),
    };
    let scale = u8::try_from(frac_part.len()).ok()?;
    let combined = format!("{int_part}{frac_part}");
    let unscaled: i128 = combined.parse().ok()?;
    Some((unscaled, scale))
}

fn keyword(s: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match s {
        "function" => Function,
        "class" => Class,
        "enum" => Enum,
        "constructor" => Constructor,
        "trait" => Trait,
        "const" => Const,
        "open" => Open,
        "abstract" => Abstract,
        "sealed" => Sealed,
        "public" => Public,
        "private" => Private,
        "protected" => Protected,
        "internal" => Internal,
        "return" => Return,
        "if" => If,
        "else" => Else,
        "for" => For,
        "while" => While,
        "do" => Do,
        "break" => Break,
        "continue" => Continue,
        "in" => In,
        "match" => Match,
        "import" => Import,
        "package" => Package,
        "this" => This,
        "true" => True,
        "false" => False,
        "null" => Null,
        "new" => New,
        "instanceof" => Instanceof,
        "interface" => Interface,
        "implements" => Implements,
        "extends" => Extends,
        // `var` is a CONTEXTUAL keyword (like `foreach`/`as`/`when`): it stays an ordinary identifier
        // in the token stream and is recognized as the inference-binding keyword only at a
        // declaration/binding start by the parser (`Parser::at_var_decl`). This frees `var` to be a
        // value / parameter / field name (it maps to a legal PHP `$var` / `->var`).
        "mutable" => Mutable,
        "static" => Static,
        "with" => With,
        "type" => TypeKw,
        "throw" => Throw,
        "try" => Try,
        "catch" => Catch,
        "finally" => Finally,
        "throws" => Throws,
        _ => return None,
    })
}

/// A-62: strip incidental indentation from a text-block body (raw lines joined by `\n`, no trailing
/// newline) per Java JEP-378: the common prefix is the minimal leading-whitespace over every
/// non-blank line **and** the closing delimiter's indentation; each line is then left-stripped by
/// that amount and right-stripped of trailing whitespace. Blank lines collapse to empty.
fn dedent_block(body: &[u8], closing_indent: usize) -> Vec<u8> {
    let is_ws = |c: u8| c == b' ' || c == b'\t';
    let lines: Vec<&[u8]> = body.split(|&c| c == b'\n').collect();
    let mut min = closing_indent;
    for ln in &lines {
        if ln.iter().all(|&c| is_ws(c)) {
            continue; // blank line — excluded from the minimum
        }
        let ind = ln.iter().take_while(|&&c| is_ws(c)).count();
        if ind < min {
            min = ind;
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for (i, ln) in lines.iter().enumerate() {
        if i > 0 {
            out.push(b'\n');
        }
        let strip = ln.iter().take_while(|&&c| is_ws(c)).count().min(min);
        let line = &ln[strip..];
        // right-trim trailing whitespace (incl. a stray `\r` from CRLF input)
        let end = line
            .iter()
            .rposition(|&c| c != b' ' && c != b'\t' && c != b'\r')
            .map_or(0, |p| p + 1);
        out.extend_from_slice(&line[..end]);
    }
    out
}

/// Tokenize `src`. Comments are **discarded** (the parser never sees them). Use
/// [`lex_with_comments`] when a tool (the formatter `phg format`) needs the trivia.
pub fn lex(src: &str) -> Result<Vec<Token>, Diagnostic> {
    lex_inner(src, &mut Vec::new())
}

/// Like [`lex`], but also returns every comment captured in source order (the `phg format`
/// side-channel — F1). The token stream is identical to [`lex`]'s; comments are collected, not
/// emitted as tokens, so the parser/AST are unchanged.
pub fn lex_with_comments(src: &str) -> Result<(Vec<Token>, Vec<Comment>), Diagnostic> {
    let mut comments = Vec::new();
    let tokens = lex_inner(src, &mut comments)?;
    Ok((tokens, comments))
}

/// True when only whitespace precedes byte offset `start` on its source line (an "own-line"
/// comment) — i.e. everything back to the previous newline (or start of file) is blank.
fn at_line_start(src: &[u8], start: usize) -> bool {
    src[..start]
        .iter()
        .rev()
        .take_while(|&&b| b != b'\n')
        .all(|&b| b == b' ' || b == b'\t' || b == b'\r')
}

fn lex_inner(src: &str, comments: &mut Vec<Comment>) -> Result<Vec<Token>, Diagnostic> {
    let mut lx = Lexer::new(src);
    // DEC-282: a shebang line (`#!/usr/bin/env phg`) is skipped at BYTE 0 only — the executable
    // Symfony-style `./bin/console` entry form. Elsewhere `#` stays a lex error (attributes are
    // `#[...]`, matched by the attribute path); the line itself is not a comment (not captured),
    // and skipping runs to (not past) the newline so line numbering stays natural.
    if lx.pos == 0 && src.starts_with("#!") {
        while let Some(b) = lx.peek() {
            if b == b'\n' {
                break;
            }
            lx.bump();
        }
    }
    let mut out = Vec::new();
    loop {
        lx.skip_whitespace();
        let line = lx.line;
        let col = lx.col;
        let start = lx.pos;
        match lx.peek() {
            None => {
                out.push(Token {
                    kind: TokenKind::Eof,
                    span: Span {
                        start,
                        len: 0,
                        line,
                        col,
                    },
                });
                return Ok(out);
            }
            Some(b) => {
                if b == b'/' && lx.peek2() == Some(b'/') {
                    let own_line = at_line_start(lx.src, start);
                    lx.skip_line_comment();
                    // The raw `// …` text, trailing whitespace trimmed (the line comment stops at the
                    // newline, which is not consumed).
                    let text = String::from_utf8_lossy(&lx.src[start..lx.pos])
                        .trim_end()
                        .to_string();
                    comments.push(Comment {
                        span: Span {
                            start,
                            len: lx.pos - start,
                            line,
                            col,
                        },
                        text,
                        kind: CommentKind::Line,
                        own_line,
                    });
                    continue;
                }
                if b == b'/' && lx.peek2() == Some(b'*') {
                    let own_line = at_line_start(lx.src, start);
                    lx.skip_block_comment()?;
                    // `start..lx.pos` spans the whole `/* … */` including the closing delimiter.
                    let text = String::from_utf8_lossy(&lx.src[start..lx.pos]).to_string();
                    comments.push(Comment {
                        span: Span {
                            start,
                            len: lx.pos - start,
                            line,
                            col,
                        },
                        text,
                        kind: CommentKind::Block,
                        own_line,
                    });
                    continue;
                }

                // `r"…"` / `r#"…"#` raw string — literal bytes, NO escapes, NO interpolation (for
                // JSON, regex, templates). Rust-style `#`-run delimiter so embedded `"` is
                // expressible. Triggered only by `r` + zero-or-more `#` + `"`; a bare `r` / `rx` is
                // an ordinary identifier (must precede the identifier scan).
                if b == b'r' {
                    let after = &lx.src[lx.pos + 1..];
                    let hashes = after.iter().take_while(|&&c| c == b'#').count();
                    if after.get(hashes) == Some(&b'"') {
                        lx.bump(); // `r`
                        for _ in 0..hashes {
                            lx.bump(); // each `#`
                        }
                        lx.bump(); // opening `"`
                        let t = lx.scan_raw_string(start, line, col, hashes)?;
                        out.push(t);
                        continue;
                    }
                }

                // `b"…"` byte-string literal — must precede the identifier scan (a bare `b` is a
                // valid identifier start). Only the exact `b"` digraph triggers it.
                if b == b'b' && lx.peek2() == Some(b'"') {
                    lx.bump(); // consume the `b` prefix
                    let t = lx.scan_bytes(start, line, col)?;
                    out.push(t);
                    continue;
                }

                // A-62: `"""` opens a multi-line, auto-dedented **text block** (must precede the
                // single-`"` string check). The opening `"""` must be followed by a newline.
                if b == b'"' && lx.peek2() == Some(b'"') && lx.peek3() == Some(b'"') {
                    let t = lx.scan_text_block(start, line, col)?;
                    out.push(t);
                    continue;
                }
                if b == b'"' {
                    let t = lx.scan_string(start, line, col)?;
                    out.push(t);
                    continue;
                }

                if b.is_ascii_digit() {
                    let t = lx.scan_number(start, line, col)?;
                    out.push(t);
                    continue;
                }

                if b == b'_' || b.is_ascii_alphabetic() {
                    let t = lx.scan_ident(start, line, col);
                    // General tagged-template rule (DEC-212): an *identifier* immediately followed by
                    // `"` (no whitespace) is a tagged template `tag"…"` — `html"…"`, `sql"…"`, … .
                    // The reserved string prefixes `r"…"`/`b"…"`/`"""…"""`/`"…"` are lexed above, so
                    // only genuine tags reach here. A keyword before `"` is left untouched (it stays a
                    // keyword token, then a separate string) — only `Ident` triggers a tag; `html`
                    // with a SPACE before `"` is an ordinary ident + string, unaffected.
                    if lx.peek() == Some(b'"') {
                        if let TokenKind::Ident(tag) = t.kind {
                            let tok = lx.scan_tagged_template(tag, start, line, col)?;
                            out.push(tok);
                            continue;
                        }
                    }
                    out.push(t);
                    continue;
                }

                // Range operators: longest-match `..=` (3) and `..` (2) ahead of `.` (1). A number
                // like `0..3` already lexes `0` as `Int(0)` — `scan_number`'s float branch needs a
                // *digit* after the dot, and here the next char is another `.`.
                if b == b'.' && lx.peek2() == Some(b'.') {
                    let (kind, len) = if lx.peek3() == Some(b'=') {
                        (TokenKind::DotDotEq, 3)
                    } else {
                        (TokenKind::DotDot, 2)
                    };
                    for _ in 0..len {
                        lx.bump();
                    }
                    out.push(Token {
                        kind,
                        span: Span {
                            start,
                            len,
                            line,
                            col,
                        },
                    });
                    continue;
                }

                // `??=` (3) null-coalesce-assign — longest-match ahead of the two-char `??`,
                // mirroring the `..=`/`..` range block above (M-mut.2).
                if b == b'?' && lx.peek2() == Some(b'?') && lx.peek3() == Some(b'=') {
                    for _ in 0..3 {
                        lx.bump();
                    }
                    out.push(Token {
                        kind: TokenKind::QuestionQuestionEq,
                        span: Span {
                            start,
                            len: 3,
                            line,
                            col,
                        },
                    });
                    continue;
                }

                // two-char operators take priority
                let two = |k: TokenKind| Token {
                    kind: k,
                    span: Span {
                        start,
                        len: 2,
                        line,
                        col,
                    },
                };
                let p2 = lx.peek2();
                let matched_two = match (b, p2) {
                    (b'=', Some(b'=')) => Some(TokenKind::EqEq),
                    (b'!', Some(b'=')) => Some(TokenKind::NotEq),
                    (b'<', Some(b'=')) => Some(TokenKind::Le),
                    (b'>', Some(b'=')) => Some(TokenKind::Ge),
                    (b'-', Some(b'>')) => Some(TokenKind::Arrow),
                    (b'=', Some(b'>')) => Some(TokenKind::FatArrow),
                    (b'|', Some(b'>')) => Some(TokenKind::Pipe),
                    (b'&', Some(b'&')) => Some(TokenKind::AndAnd),
                    (b'|', Some(b'|')) => Some(TokenKind::OrOr),
                    (b'?', Some(b'?')) => Some(TokenKind::QuestionQuestion),
                    (b'?', Some(b'.')) => Some(TokenKind::QuestionDot),
                    // `::` class/type-level member-access separator (DEC-207). A lone `:` falls
                    // through to the single-char dispatch (`Colon`), unchanged.
                    (b':', Some(b':')) => Some(TokenKind::ColonColon),
                    // compound-assign + increment/decrement (M-mut.2). `-=`/`--`/`->` and
                    // `/=` (not a `//`/`/*` comment, handled earlier) all reach here distinctly.
                    (b'+', Some(b'=')) => Some(TokenKind::PlusEq),
                    (b'-', Some(b'=')) => Some(TokenKind::MinusEq),
                    (b'*', Some(b'=')) => Some(TokenKind::StarEq),
                    // `**` power operator (Phase 1 operators slice). Distinct p2 from `*=`, so order
                    // is irrelevant; `**=` (power-assign) is out of scope and lexes as `StarStar` `Eq`.
                    (b'*', Some(b'*')) => Some(TokenKind::StarStar),
                    (b'/', Some(b'=')) => Some(TokenKind::SlashEq),
                    (b'%', Some(b'=')) => Some(TokenKind::PercentEq),
                    (b'+', Some(b'+')) => Some(TokenKind::PlusPlus),
                    (b'-', Some(b'-')) => Some(TokenKind::MinusMinus),
                    // `<<` bitwise shift-left (primitives P2). `<=` is claimed above; a bare `<` falls
                    // through to the single-char dispatch. There is deliberately no `>>` token (it
                    // would break nested generics) — shift-right is two `Gt` handled in the parser.
                    (b'<', Some(b'<')) => Some(TokenKind::Shl),
                    // `#[` opens a PHP-8-style attribute group (M6 W2). A bare `#` has no other use
                    // (raw strings `r#"…"#` are lexed in the string path above), so this is the only
                    // place `#` is accepted; a lone `#` falls through to the unexpected-char error.
                    (b'#', Some(b'[')) => Some(TokenKind::HashBracket),
                    _ => None,
                };
                if let Some(k) = matched_two {
                    lx.bump();
                    lx.bump();
                    out.push(two(k));
                    continue;
                }

                let single = |k: TokenKind| Token {
                    kind: k,
                    span: Span {
                        start,
                        len: 1,
                        line,
                        col,
                    },
                };
                let kind = match b {
                    b'.' => Some(TokenKind::Dot),
                    b';' => Some(TokenKind::Semicolon),
                    b',' => Some(TokenKind::Comma),
                    b':' => Some(TokenKind::Colon),
                    b'?' => Some(TokenKind::Question),
                    b'(' => Some(TokenKind::LParen),
                    b')' => Some(TokenKind::RParen),
                    b'{' => Some(TokenKind::LBrace),
                    b'}' => Some(TokenKind::RBrace),
                    b'[' => Some(TokenKind::LBracket),
                    b']' => Some(TokenKind::RBracket),
                    b'+' => Some(TokenKind::Plus),
                    b'-' => Some(TokenKind::Minus),
                    b'*' => Some(TokenKind::Star),
                    b'/' => Some(TokenKind::Slash),
                    b'%' => Some(TokenKind::Percent),
                    b'<' => Some(TokenKind::Lt),
                    b'>' => Some(TokenKind::Gt),
                    b'=' => Some(TokenKind::Eq),
                    b'!' => Some(TokenKind::Bang),
                    // A lone `|` is the union-type separator (`A | B`, M-RT S4). `|>` and `||` are
                    // claimed by the two-char dispatch above, so reaching here means a single `|`.
                    b'|' => Some(TokenKind::Bar),
                    // A lone `&` is the intersection-type separator (`A & B`, M-RT S5) or bitwise-AND
                    // in expression position. `&&` is claimed by the two-char dispatch above.
                    b'&' => Some(TokenKind::Amp),
                    // `^` bitwise XOR, `~` unary bitwise NOT (primitives P2; expression-only).
                    b'^' => Some(TokenKind::Caret),
                    b'~' => Some(TokenKind::Tilde),
                    _ => None,
                };
                match kind {
                    Some(k) => {
                        lx.bump();
                        out.push(single(k));
                    }
                    None => {
                        // Decode the full char (handles multi-byte UTF-8) for the message.
                        let ch = lx.current_char();
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            format!("unexpected character {ch:?}"),
                            line,
                            col,
                        ));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

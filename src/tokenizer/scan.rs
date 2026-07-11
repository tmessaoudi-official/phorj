//! Lexer — cursor plumbing, numbers, comments, identifiers.

use super::*;

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    pub(super) fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    pub(super) fn peek3(&self) -> Option<u8> {
        self.src.get(self.pos + 2).copied()
    }

    pub(super) fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(b)
    }

    pub(super) fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                self.bump();
            } else {
                break;
            }
        }
    }

    pub(super) fn scan_number(
        &mut self,
        start: usize,
        line: u32,
        col: u32,
    ) -> Result<Token, Diagnostic> {
        // Base-prefixed integers `0x` / `0b` / `0o` (Rust-style; a bare leading `0` stays decimal —
        // implicit octal is a PHP footgun we deliberately drop). Underscore separators are allowed.
        if self.peek() == Some(b'0') {
            if let Some(radix) = match self.src.get(self.pos + 1) {
                Some(b'x' | b'X') => Some(16u32),
                Some(b'b' | b'B') => Some(2),
                Some(b'o' | b'O') => Some(8),
                _ => None,
            } {
                self.bump();
                self.bump(); // consume the `0x` / `0b` / `0o` prefix
                let mut digits = String::new();
                while let Some(c) = self.peek() {
                    if c == b'_' {
                        self.bump();
                    } else if (c as char).is_digit(radix) {
                        digits.push(c as char);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if digits.is_empty() {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "expected a digit after the numeric base prefix",
                        line,
                        col,
                    ));
                }
                let i = i64::from_str_radix(&digits, radix).map_err(|_| {
                    Diagnostic::new(Stage::Lex, "integer literal out of range", line, col)
                })?;
                return Ok(Token {
                    kind: TokenKind::Int(i),
                    span: Span {
                        start,
                        len: self.pos - start,
                        line,
                        col,
                    },
                });
            }
        }
        // Decimal — digits with optional `_` separators, optional fraction, optional exponent.
        while matches!(self.peek(), Some(b) if b.is_ascii_digit() || b == b'_') {
            self.bump();
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') && matches!(self.peek2(), Some(d) if d.is_ascii_digit()) {
            is_float = true;
            self.bump(); // consume '.'
            while matches!(self.peek(), Some(b) if b.is_ascii_digit() || b == b'_') {
                self.bump();
            }
        }
        // Exponent `e`/`E`, only when followed by an optional sign and at least one digit (so `3em`
        // is `3` then the identifier `em`, not a malformed exponent).
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let exp_ok = match self.src.get(self.pos + 1) {
                Some(d) if d.is_ascii_digit() => true,
                Some(b'+' | b'-') => {
                    matches!(self.src.get(self.pos + 2), Some(d) if d.is_ascii_digit())
                }
                _ => false,
            };
            if exp_ok {
                is_float = true;
                self.bump(); // 'e'
                if matches!(self.peek(), Some(b'+' | b'-')) {
                    self.bump();
                }
                while matches!(self.peek(), Some(b) if b.is_ascii_digit() || b == b'_') {
                    self.bump();
                }
            }
        }
        // `decimal` suffix `…d` (M-NUM S1): a numeric literal immediately followed by `d` that is NOT
        // continued by another identifier char (so `3d` is a decimal but `3days` is `3` + `days`). The
        // scale comes from the literal TEXT (trailing zeros preserved): `1.50d` ⇒ scale 2. An
        // exponent literal (`1e3d`) is rejected — `e`-exponent on a decimal is out of scope this slice.
        if self.peek() == Some(b'd')
            && !matches!(self.src.get(self.pos + 1), Some(c) if c.is_ascii_alphanumeric() || *c == b'_')
        {
            let raw_dec = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
            self.bump(); // consume the `d`
            if is_float {
                // `is_float` here means a fraction OR an exponent was scanned. A `.` fraction is fine
                // for a decimal; an `e`-exponent is rejected this slice.
                let stripped: String = raw_dec.chars().filter(|c| *c != '_').collect();
                if stripped.contains(['e', 'E']) {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "an exponent is not allowed on a `decimal` literal (`1e3d`) — write the digits out",
                        line,
                        col,
                    )
                    .with_code("E-DECIMAL-LITERAL"));
                }
            }
            let (unscaled, scale) = parse_decimal_literal(raw_dec).ok_or_else(|| {
                Diagnostic::new(
                    Stage::Lex,
                    "`decimal` literal is out of range (exceeds i128)",
                    line,
                    col,
                )
                .with_code("E-DECIMAL-LITERAL")
            })?;
            return Ok(Token {
                kind: TokenKind::Decimal(unscaled, scale),
                span: Span {
                    start,
                    len: self.pos - start,
                    line,
                    col,
                },
            });
        }
        let raw = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        // Strip `_` separators before parsing; the literal's value (not its surface form) is what
        // reaches the AST, so every base/format collapses to the same Int/Float — byte-identical.
        let text: String = raw.chars().filter(|c| *c != '_').collect();
        let kind = if is_float {
            let f: f64 = text.parse().map_err(|_| {
                Diagnostic::new(Stage::Lex, "float literal out of range", line, col)
            })?;
            if !f.is_finite() {
                return Err(Diagnostic::new(
                    Stage::Lex,
                    "float literal out of range",
                    line,
                    col,
                ));
            }
            TokenKind::Float(f)
        } else {
            let i: i64 = text.parse().map_err(|_| {
                Diagnostic::new(Stage::Lex, "integer literal out of range", line, col)
            })?;
            TokenKind::Int(i)
        };
        Ok(Token {
            kind,
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
    }

    pub(super) fn skip_line_comment(&mut self) {
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.bump();
        }
    }

    pub(super) fn skip_block_comment(&mut self) -> Result<(), Diagnostic> {
        let (sl, sc) = (self.line, self.col);
        self.bump();
        self.bump(); // consume /*
        loop {
            match self.peek() {
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated block comment",
                        sl,
                        sc,
                    ))
                }
                Some(b'*') if self.peek2() == Some(b'/') => {
                    self.bump();
                    self.bump();
                    return Ok(());
                }
                _ => {
                    self.bump();
                }
            }
        }
    }
}

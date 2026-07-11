//! Lexer — ident/keyword tail + trivia.

use super::*;

impl Lexer<'_> {
    pub(super) fn scan_ident(&mut self, start: usize, line: u32, col: u32) -> Token {
        while matches!(self.peek(), Some(b) if b == b'_' || b.is_ascii_alphanumeric()) {
            self.bump();
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let kind = keyword(text).unwrap_or_else(|| TokenKind::Ident(text.to_string()));
        Token {
            kind,
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        }
    }

    /// Decode the full UTF-8 char beginning at the current position. The source is always
    /// valid UTF-8 (it came from `&str`), so a char boundary is guaranteed at `self.pos`.
    /// Used only on the error path so diagnostics show the real char, not a mojibake byte.
    pub(super) fn current_char(&self) -> char {
        std::str::from_utf8(&self.src[self.pos..])
            .ok()
            .and_then(|s| s.chars().next())
            .unwrap_or(char::REPLACEMENT_CHARACTER)
    }
}

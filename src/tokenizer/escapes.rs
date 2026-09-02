//! Escape scanning shared by the string, text-block and bytes literals — `\u{…}` and hex digits —
//! split out of `strings.rs` (Invariant 13: that file is ratcheted at its baseline and the
//! interpolation line/col fix of 2026-09-02 grew it).
use super::*;

impl Lexer<'_> {
    /// Expand a `\u{HEX}` escape (the `\u` is already consumed): `{`, then 1–6 hex digits, then `}`,
    /// naming a Unicode codepoint whose UTF-8 bytes are appended to `bytes`. `(el, ec)` is the
    /// position of the opening backslash, for error reporting (Phase 1 string slice).
    pub(super) fn scan_unicode_escape(
        &mut self,
        bytes: &mut Vec<u8>,
        el: u32,
        ec: u32,
    ) -> Result<(), Diagnostic> {
        if self.bump() != Some(b'{') {
            return Err(Diagnostic::new(
                Stage::Lex,
                "expected `{` after `\\u` (e.g. `\\u{1F600}`)",
                el,
                ec,
            ));
        }
        let mut hex = String::new();
        loop {
            match self.bump() {
                Some(b'}') => break,
                Some(c) if c.is_ascii_hexdigit() => hex.push(c as char),
                Some(c) => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        format!("invalid hex digit `{}` in `\\u{{…}}`", c as char),
                        el,
                        ec,
                    ))
                }
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated `\\u{…}` escape",
                        el,
                        ec,
                    ))
                }
            }
        }
        if hex.is_empty() || hex.len() > 6 {
            return Err(Diagnostic::new(
                Stage::Lex,
                "`\\u{…}` takes 1–6 hex digits",
                el,
                ec,
            ));
        }
        let cp = u32::from_str_radix(&hex, 16).expect("digits validated as hex above");
        match char::from_u32(cp) {
            Some(ch) => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                Ok(())
            }
            None => Err(Diagnostic::new(
                Stage::Lex,
                format!("`\\u{{{hex}}}` is not a valid Unicode codepoint"),
                el,
                ec,
            )),
        }
    }

    /// Consume one hex digit for a `\xHH` byte escape, or error at the offending position.
    pub(super) fn hex_digit(&mut self, el: u32, ec: u32) -> Result<u8, Diagnostic> {
        match self.bump() {
            Some(c) if c.is_ascii_hexdigit() => Ok((c as char).to_digit(16).unwrap() as u8),
            _ => Err(Diagnostic::new(
                Stage::Lex,
                "invalid \\xHH byte escape (expected two hex digits)",
                el,
                ec,
            )),
        }
    }

    // NOTE: identifiers are ASCII-only by design for v0.1 (scan_ident uses
    // is_ascii_alphabetic / is_ascii_alphanumeric). Unicode identifiers are out of scope.
}

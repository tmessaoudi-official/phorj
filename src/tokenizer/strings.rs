//! Lexer — string-family scanners: plain/interp strings, text blocks, raw strings,
//! unicode escapes, html blocks, bytes literals.

use super::*;

impl Lexer<'_> {
    pub(super) fn scan_string(
        &mut self,
        start: usize,
        line: u32,
        col: u32,
    ) -> Result<Token, Diagnostic> {
        self.bump(); // opening quote
                     // Single pass that BOTH expands escapes AND splits interpolation — only here do
                     // we know whether a `{` is a real interpolation brace or a `\{` literal escape (a
                     // parser-side split on the escape-expanded value couldn't tell `\{` from `\\{`).
                     // Escapes expand into whichever buffer is active: the literal run, or — when
                     // inside `{…}` — the interpolation's inner source (so an escaped quote `\"` in
                     // `"{m[\"k\"]}"` becomes a real `"` in the re-lexed expression, matching the old
                     // expand-then-split pipeline). A bare unescaped `{` opens an interpolation, the
                     // first unescaped `}` closes it (no nesting — matching the prior splitter). Source
                     // is valid UTF-8, so the `from_utf8` calls cannot fail.
        let mut segs: Vec<StrSeg> = Vec::new();
        let mut lit: Vec<u8> = Vec::new();
        let mut interp: Option<Vec<u8>> = None; // `Some` while inside `{…}`
        let mut interp_start: usize = 0; // absolute byte offset of the active interpolation's content
        loop {
            // Snapshot before consuming, so an invalid escape reports the backslash's column.
            let (el, ec) = (self.line, self.col);
            match self.bump() {
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated string",
                        line,
                        col,
                    ))
                }
                Some(b'"') => {
                    match interp.as_mut() {
                        // Inside an interpolation expression, a `"` opens a NESTED string literal
                        // (M-DOGFOOD W2): consume it verbatim (its `"`, and any `{`/`}` inside it)
                        // into the inner source, so it neither closes the outer string nor mis-nests
                        // the interpolation. Escapes are kept verbatim — the inner source is re-lexed
                        // as an expression later, which processes them. (A raw string with embedded
                        // quotes inside an interpolation remains a deferral.)
                        Some(buf) => {
                            buf.push(b'"');
                            loop {
                                let (nl, nc) = (self.line, self.col);
                                match self.bump() {
                                    None => {
                                        return Err(Diagnostic::new(
                                            Stage::Lex,
                                            "unterminated string in interpolation",
                                            nl,
                                            nc,
                                        ))
                                    }
                                    Some(b'\\') => {
                                        buf.push(b'\\');
                                        match self.bump() {
                                            Some(c) => buf.push(c),
                                            None => {
                                                return Err(Diagnostic::new(
                                                    Stage::Lex,
                                                    "unterminated escape in interpolation string",
                                                    nl,
                                                    nc,
                                                ))
                                            }
                                        }
                                    }
                                    Some(b'"') => {
                                        buf.push(b'"');
                                        break;
                                    }
                                    Some(c) => buf.push(c),
                                }
                            }
                        }
                        None => break,
                    }
                }
                // A bare `{` outside an interpolation opens one (flush the pending literal first); a
                // `{` already inside one is just a character of the inner expression source.
                Some(b'{') if interp.is_none() => {
                    if !lit.is_empty() {
                        segs.push(StrSeg::Lit(
                            String::from_utf8(std::mem::take(&mut lit)).expect("valid UTF-8"),
                        ));
                    }
                    interp = Some(Vec::new());
                    // `self.pos` is now just past the opening `{` → the first byte of the inner source.
                    interp_start = self.pos;
                }
                // The first unescaped `}` closes the interpolation; a `}` outside one is an error
                // (write `\}` for a literal brace).
                Some(b'}') if interp.is_some() => {
                    segs.push(StrSeg::Interp(
                        String::from_utf8(interp.take().expect("inside interp"))
                            .expect("valid UTF-8"),
                        interp_start,
                    ));
                }
                Some(b'}') => return Err(Diagnostic::new(
                    Stage::Lex,
                    "unexpected '}' in string (no matching '{'; write `\\}` for a literal brace)",
                    el,
                    ec,
                )),
                Some(b'\\') => {
                    // Expand the escape into the active buffer (interpolation inner if inside one,
                    // else the literal run).
                    let buf = interp.as_mut().unwrap_or(&mut lit);
                    match self.bump() {
                        Some(b'n') => buf.push(b'\n'),
                        Some(b't') => buf.push(b'\t'),
                        Some(b'r') => buf.push(b'\r'),
                        Some(b'\\') => buf.push(b'\\'),
                        Some(b'"') => buf.push(b'"'),
                        // `\{` / `\}` — a literal brace (Phase 1 string slice): emitted as a byte into
                        // the active buffer, so it never opens/closes an interpolation.
                        Some(b'{') => buf.push(b'{'),
                        Some(b'}') => buf.push(b'}'),
                        // `\u{HEX}` — a Unicode escape: 1–6 hex digits → UTF-8 bytes at lex time.
                        Some(b'u') => self.scan_unicode_escape(buf, el, ec)?,
                        Some(other) => {
                            return Err(Diagnostic::new(
                                Stage::Lex,
                                format!("invalid escape \\{}", other as char),
                                el,
                                ec,
                            ))
                        }
                        None => {
                            return Err(Diagnostic::new(
                                Stage::Lex,
                                "unterminated string",
                                line,
                                col,
                            ))
                        }
                    }
                }
                Some(other) => interp.as_mut().unwrap_or(&mut lit).push(other),
            }
        }
        if !lit.is_empty() {
            segs.push(StrSeg::Lit(
                String::from_utf8(lit).expect("string literal run is valid UTF-8"),
            ));
        }
        Ok(Token {
            kind: TokenKind::Str(segs),
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
    }

    /// A-62: scan a `"""…"""` multi-line **text block**. The opening `"""` is followed by a
    /// mandatory newline (the opening line carries no content); the closing delimiter is a line whose
    /// only non-whitespace content is `"""`. Incidental indentation is stripped (Java JEP-378: the
    /// minimal leading-whitespace over every non-blank content line **and** the closing line), and
    /// each line's trailing whitespace is trimmed. Interpolation (`{expr}`) and escapes work exactly
    /// as in `"…"` — implemented by dedenting, escaping bare `"`, then routing the body through
    /// [`Self::scan_string`] (no duplicated escape/interpolation logic), yielding the same
    /// `TokenKind::Str(segs)` a single-line string would.
    pub(super) fn scan_text_block(
        &mut self,
        start: usize,
        line: u32,
        col: u32,
    ) -> Result<Token, Diagnostic> {
        self.bump();
        self.bump();
        self.bump(); // opening `"""`
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r')) {
            self.bump();
        }
        if self.bump() != Some(b'\n') {
            return Err(Diagnostic::new(
                Stage::Lex,
                "a `\"\"\"` text block must be followed by a newline (the opening line carries no content)",
                line,
                col,
            ));
        }
        // Collect raw content lines (each incl. its trailing `\n`) until the closing-delimiter line.
        let mut body: Vec<u8> = Vec::new();
        let closing_indent;
        loop {
            let mark = self.pos;
            let mut indent = 0usize;
            while matches!(self.peek(), Some(b' ' | b'\t')) {
                self.bump();
                indent += 1;
            }
            if self.peek() == Some(b'"') && self.peek2() == Some(b'"') && self.peek3() == Some(b'"')
            {
                self.bump();
                self.bump();
                self.bump();
                closing_indent = indent;
                break;
            }
            if self.peek().is_none() {
                return Err(Diagnostic::new(
                    Stage::Lex,
                    "unterminated text block (no closing `\"\"\"`)",
                    line,
                    col,
                ));
            }
            body.extend_from_slice(&self.src[mark..self.pos]); // the indent we consumed
            loop {
                match self.bump() {
                    Some(b'\n') => {
                        body.push(b'\n');
                        break;
                    }
                    Some(c) => body.push(c),
                    None => {
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            "unterminated text block (no closing `\"\"\"`)",
                            line,
                            col,
                        ))
                    }
                }
            }
        }
        // The newline separating the last content line from the closing delimiter is not content.
        if body.last() == Some(&b'\n') {
            body.pop();
        }
        let dedented = dedent_block(&body, closing_indent);
        // Escape bare `"` (a `\X` pair is copied verbatim) so the dedented body is safe to wrap in
        // `"…"` and route through `scan_string`, which owns all escape + interpolation handling.
        // Build the wrapped buffer as BYTES (a `u8 as char` cast would corrupt multi-byte UTF-8).
        let mut wrapped: Vec<u8> = Vec::with_capacity(dedented.len() + 2);
        wrapped.push(b'"');
        let mut k = 0;
        while k < dedented.len() {
            let c = dedented[k];
            if c == b'\\' && k + 1 < dedented.len() {
                wrapped.push(b'\\');
                wrapped.push(dedented[k + 1]);
                k += 2;
                continue;
            }
            if c == b'"' {
                wrapped.push(b'\\');
                wrapped.push(b'"');
                k += 1;
                continue;
            }
            wrapped.push(c);
            k += 1;
        }
        wrapped.push(b'"');
        let wrapped = String::from_utf8(wrapped).expect("text block body is valid UTF-8");
        let mut sub = Lexer::new(&wrapped);
        let tok = sub.scan_string(0, line, col).map_err(|e| {
            Diagnostic::new(
                Stage::Lex,
                format!("in text block: {}", e.message),
                line,
                col,
            )
        })?;
        let mut segs = match tok.kind {
            TokenKind::Str(s) => s,
            _ => unreachable!("scan_string yields a Str token"),
        };
        // Make interpolation offsets file-unique: the sub-tokenizer numbered them from 0 within the
        // wrapped body; shift by this block's source start (the block occupies a unique source range,
        // and the dedented body is no longer than the source, so `start + off` stays within it).
        for seg in &mut segs {
            if let StrSeg::Interp(_, off) = seg {
                *off += start;
            }
        }
        Ok(Token {
            kind: TokenKind::Str(segs),
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
    }

    /// Scan a raw string body (`r#*"` already consumed): copy bytes verbatim — no escapes, no
    /// interpolation — until a closing `"` followed by `hashes` `#`s. A `"` with the wrong number of
    /// trailing `#`s is a literal `"`, so any content (including `"#`) is expressible by choosing a
    /// longer `#`-run. Produces a single `StrSeg::Lit` (Phase 1 string slice).
    pub(super) fn scan_raw_string(
        &mut self,
        start: usize,
        line: u32,
        col: u32,
        hashes: usize,
    ) -> Result<Token, Diagnostic> {
        let mut body: Vec<u8> = Vec::new();
        loop {
            match self.peek() {
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated raw string",
                        line,
                        col,
                    ))
                }
                Some(b'"') => {
                    // A closing delimiter iff the `"` is followed by exactly `hashes` `#`s.
                    let closes = (0..hashes).all(|i| self.src.get(self.pos + 1 + i) == Some(&b'#'));
                    if closes {
                        self.bump(); // `"`
                        for _ in 0..hashes {
                            self.bump(); // each closing `#`
                        }
                        break;
                    }
                    body.push(b'"');
                    self.bump();
                }
                Some(c) => {
                    body.push(c);
                    self.bump();
                }
            }
        }
        let value = String::from_utf8(body).expect("raw string body is valid UTF-8");
        Ok(Token {
            kind: TokenKind::Str(vec![StrSeg::Lit(value)]),
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
    }

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

    /// Scan the body of a tagged-template literal `tag"…"` (the `tag` prefix is already consumed; the
    /// cursor sits on the opening `"`). The body is captured exactly like [`Self::scan_string`] —
    /// same escapes (`\n \t \r \\ \"`), multi-byte UTF-8 and raw newlines copied verbatim, so a
    /// tagged template spans lines for free — and `{`/`}` are preserved verbatim: the interpolation
    /// split *and* the per-tag desugar happen in the parser/checker, not here. The only difference
    /// from `scan_string` is the token kind, which carries the `tag` name alongside the raw body so
    /// the parser can route `html"…"` to the html desugarer and every other tag to `E-UNKNOWN-TAG`.
    pub(super) fn scan_tagged_template(
        &mut self,
        tag: String,
        start: usize,
        line: u32,
        col: u32,
    ) -> Result<Token, Diagnostic> {
        self.bump(); // opening quote
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let (el, ec) = (self.line, self.col);
            match self.bump() {
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated tagged-template literal",
                        line,
                        col,
                    ))
                }
                Some(b'"') => break,
                Some(b'\\') => match self.bump() {
                    Some(b'n') => bytes.push(b'\n'),
                    Some(b't') => bytes.push(b'\t'),
                    Some(b'r') => bytes.push(b'\r'),
                    Some(b'\\') => bytes.push(b'\\'),
                    Some(b'"') => bytes.push(b'"'),
                    Some(other) => {
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            format!("invalid escape \\{}", other as char),
                            el,
                            ec,
                        ))
                    }
                    None => {
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            "unterminated tagged-template literal",
                            line,
                            col,
                        ))
                    }
                },
                Some(other) => bytes.push(other),
            }
        }
        let value = String::from_utf8(bytes).expect("source tagged-template body is valid UTF-8");
        Ok(Token {
            kind: TokenKind::TaggedTemplate(tag, value),
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
    }

    /// Scan a `b"…"` byte-string literal (the `b` prefix is already consumed). Unlike `scan_string`
    /// there is NO interpolation — `{`/`}` are literal bytes. Escapes are `\n \t \r \\ \"` plus
    /// `\xHH` (two hex digits → one arbitrary octet), so a literal can hold non-UTF-8 Bytes.
    pub(super) fn scan_bytes(
        &mut self,
        start: usize,
        line: u32,
        col: u32,
    ) -> Result<Token, Diagnostic> {
        self.bump(); // opening quote
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let (el, ec) = (self.line, self.col);
            match self.bump() {
                None => {
                    return Err(Diagnostic::new(
                        Stage::Lex,
                        "unterminated byte string",
                        line,
                        col,
                    ))
                }
                Some(b'"') => break,
                Some(b'\\') => match self.bump() {
                    Some(b'n') => bytes.push(b'\n'),
                    Some(b't') => bytes.push(b'\t'),
                    Some(b'r') => bytes.push(b'\r'),
                    Some(b'\\') => bytes.push(b'\\'),
                    Some(b'"') => bytes.push(b'"'),
                    Some(b'x') => {
                        let hi = self.hex_digit(el, ec)?;
                        let lo = self.hex_digit(el, ec)?;
                        bytes.push(hi << 4 | lo);
                    }
                    Some(other) => {
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            format!("invalid escape \\{}", other as char),
                            el,
                            ec,
                        ))
                    }
                    None => {
                        return Err(Diagnostic::new(
                            Stage::Lex,
                            "unterminated byte string",
                            line,
                            col,
                        ))
                    }
                },
                Some(other) => bytes.push(other),
            }
        }
        Ok(Token {
            kind: TokenKind::Bytes(bytes),
            span: Span {
                start,
                len: self.pos - start,
                line,
                col,
            },
        })
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

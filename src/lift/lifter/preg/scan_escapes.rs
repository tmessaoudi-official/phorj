//! Row 4l-a — the two sub-scans that carry their own grammar: a `\` escape and a `[…]` class.

use super::{byte_refusal, Kind, Scan, BYTE_HINT, META};

/// PCRE's non-UCP classes (its default C-locale tables — no corpus calls `setlocale`).
const DIGIT: &str = "0-9";
const WORD: &str = "A-Za-z0-9_";
const SPACE: &str = r"\t\n\x0B\f\r ";

impl Scan<'_> {
    pub(super) fn escape(&mut self) -> Result<(), String> {
        let Some(n) = self.peek(1) else {
            return Err("a trailing `\\`".into());
        };
        self.i += 2;
        let ascii = !self.o.unicode;
        match n {
            _ if Some(n) == self.o.delim && !n.is_ascii_alphanumeric() => {
                if META.contains(n) {
                    self.out.push('\\');
                }
                self.out.push(n);
                self.push(Kind::Ascii)
            }
            'd' | 'w' | 's' if ascii => {
                self.out.push('[');
                self.out.push_str(shorthand(n));
                self.out.push(']');
                self.push(Kind::Ascii)
            }
            'D' | 'W' | 'S' if ascii => {
                self.out.push_str("[^");
                self.out.push_str(shorthand(n.to_ascii_lowercase()));
                self.out.push(']');
                self.push(Kind::Byte)
            }
            'b' | 'B' if ascii => Err(format!(
                "the `\\b`/`\\B` word boundary without the `u` modifier — an ASCII boundary in PCRE, \
                 a Unicode one in phorj — {BYTE_HINT}"
            )),
            'p' | 'P' if ascii => Err(p_refusal()),
            'Z' => {
                // `\Z` is PHP's `$` whatever `D` says.
                if matches!(self.peek(0), Some('*' | '+' | '?' | '{')) {
                    return Err("a quantified `\\Z` has no faithful form".into());
                }
                self.dollar()
            }
            'z' => {
                self.out.push_str(r"\z");
                self.push(Kind::End)
            }
            'A' => {
                self.out.push_str(r"\A");
                self.push(Kind::Start)
            }
            'x' => {
                let (text, value) = self.hex();
                if ascii && value >= 0x80 {
                    return Err(x_refusal());
                }
                self.out.push_str(&text);
                self.push(Kind::Ascii)
            }
            '1'..='9' if self.peek(0).is_some_and(|c| c.is_ascii_digit()) => Err(
                "a multi-digit `\\NN` escape (a back-reference or an octal escape — PCRE decides by \
                 the number of groups)"
                    .into(),
            ),
            '1'..='9' | 'b' | 'B' | 'G' | 'K' | 'k' | 'g' => {
                self.out.push('\\');
                self.out.push(n);
                self.push(Kind::Other)
            }
            _ => {
                // Everything else is copied; what `Core.Regex` cannot read, `engine::validate` refuses.
                self.out.push('\\');
                self.out.push(n);
                self.push(Kind::Ascii)
            }
        }
    }

    /// After `\x`: `\x{H…}` or up to two hex digits. Returns the escape text and its value.
    fn hex(&mut self) -> (String, u32) {
        let mut text = String::from("\\x");
        let mut digits = String::new();
        if self.peek(0) == Some('{') {
            while let Some(c) = self.peek(0) {
                self.i += 1;
                text.push(c);
                if c == '}' {
                    break;
                }
                if c != '{' {
                    digits.push(c);
                }
            }
        } else {
            while digits.len() < 2 && self.peek(0).is_some_and(|c| c.is_ascii_hexdigit()) {
                let c = self.c[self.i];
                self.i += 1;
                text.push(c);
                digits.push(c);
            }
        }
        (text, u32::from_str_radix(&digits, 16).unwrap_or(0))
    }

    pub(super) fn class(&mut self) -> Result<(), String> {
        let ascii = !self.o.unicode;
        self.out.push('[');
        self.i += 1;
        let negated = self.peek(0) == Some('^');
        if negated {
            self.out.push('^');
            self.i += 1;
        }
        if self.peek(0) == Some(']') {
            self.out.push(']');
            self.i += 1;
        }
        loop {
            let Some(c) = self.peek(0) else {
                return Err("an unterminated character class".into());
            };
            self.i += 1;
            match c {
                ']' => break,
                '\\' => {
                    let Some(n) = self.peek(0) else {
                        return Err("a trailing `\\`".into());
                    };
                    self.i += 1;
                    match n {
                        _ if Some(n) == self.o.delim && !n.is_ascii_alphanumeric() => {
                            if META.contains(n) {
                                self.out.push('\\');
                            }
                            self.out.push(n);
                        }
                        'd' | 'w' | 's' if ascii => self.out.push_str(shorthand(n)),
                        'D' | 'W' | 'S' if ascii => return Err(negated_in_class(n)),
                        'p' | 'P' if ascii => return Err(p_refusal()),
                        'x' => {
                            let (text, value) = self.hex();
                            if ascii && value >= 0x80 {
                                return Err(x_refusal());
                            }
                            self.out.push_str(&text);
                        }
                        _ => {
                            self.out.push('\\');
                            self.out.push(n);
                        }
                    }
                }
                _ => self.out.push(c),
            }
        }
        self.out.push(']');
        if negated && ascii {
            if self.depth > 0 {
                return Err(byte_refusal());
            }
            self.push(Kind::Byte)
        } else {
            self.push(Kind::Ascii)
        }
    }
}

fn shorthand(n: char) -> &'static str {
    match n {
        'd' => DIGIT,
        'w' => WORD,
        _ => SPACE,
    }
}

fn negated_in_class(n: char) -> String {
    format!("a negated shorthand `\\{n}` inside a class without the `u` modifier — {BYTE_HINT}")
}

fn p_refusal() -> String {
    format!(
        "a `\\p` property without the `u` modifier — PCRE reads bytes as Latin-1 there — {BYTE_HINT}"
    )
}

fn x_refusal() -> String {
    format!(
        "a `\\x` escape above 0x7F without the `u` modifier names a BYTE in PCRE and a CHARACTER in \
         phorj — {BYTE_HINT}"
    )
}

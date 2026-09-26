//! PHP-lift parser — the docblock TYPE grammar (split from `docblock.rs`, 2026-09-26, M-Decomp).
//!
//! `docblock.rs` finds a `@param` / `@return` / `@var` and decides where its type goes; this file
//! parses that type.
//!
//! Grammar read here: `?T`, `T|null`, `list<T>`, `non-empty-list<T>`, `array<T>`, `array<K, V>`,
//! `non-empty-array<…>`, `T[]`, scalars and class names (a `\`-rooted one becomes an implicit
//! `use`, exactly like an inline name in `names.rs`). Array SHAPES `array{…}` lift to
//! tuples ([`super::doc_shape`]). A callable SIGNATURE (`callable(A): R`,
//! `\Closure(A): R`) is a function type ([`super::doc_callable`]), and `( … )` groups (`(callable(A): R)|null`). Refused by
//! name: `mixed`, a signature-less `callable`, `iterable`, `object`, other generics, unions other
//! than `|null`.

use super::*;

pub(super) struct Cursor<'a> {
    pub(super) s: &'a str,
    pub(super) i: usize,
}

impl Cursor<'_> {
    pub(super) fn skip_ws(&mut self) {
        while self.s[self.i..].starts_with(' ') {
            self.i += 1;
        }
    }
    pub(super) fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.s[self.i..].starts_with(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }
    pub(super) fn eat_str(&mut self, t: &str) -> bool {
        if self.s[self.i..].starts_with(t) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    pub(super) fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }
    /// A quoted array-shape key (`'total-cost'`, `"2fa"`), returned WITHOUT its quotes. PHPStan and
    /// Psalm both emit this form for a key that is not a bare identifier — which is exactly the key
    /// phorj cannot name a field after, so reading it here is what lets the refusal say so.
    pub(super) fn quoted(&mut self, q: char) -> String {
        self.i += q.len_utf8();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == q {
                break;
            }
            self.i += c.len_utf8();
        }
        let out = self.s[start..self.i].to_string();
        if self.peek() == Some(q) {
            self.i += q.len_utf8();
        }
        out
    }
    /// A type head: letters, digits, `_`, `\` (paths) and `-` (`non-empty-list`).
    pub(super) fn ident(&mut self) -> String {
        self.skip_ws();
        let start = self.i;
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_alphanumeric() || matches!(c, '_' | '\\' | '-') {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        self.s[start..self.i].to_string()
    }
}

impl PParser {
    pub(super) fn parse_doc_type(&mut self, s: &str) -> Result<PhpType, String> {
        let mut c = Cursor { s: s.trim(), i: 0 };
        let t = self.doc_type(&mut c)?;
        c.skip_ws();
        if c.i < c.s.len() {
            return Err(self.err(&format!(
                "unexpected `{}` in the docblock type `{s}`",
                &c.s[c.i..]
            )));
        }
        Ok(t)
    }

    pub(super) fn doc_type(&mut self, c: &mut Cursor) -> Result<PhpType, String> {
        let mut nullable = c.eat('?');
        // `(T)` groups — how a docblock makes a callable nullable: `(callable(A): R)|null` (row 4e).
        let mut t = if c.eat('(') {
            let inner = self.doc_type(c)?;
            if !c.eat(')') {
                return Err(self.err(&format!("`)` in the docblock type `{}`", c.s)));
            }
            inner
        } else {
            self.doc_atom(c)?
        };
        while c.eat_str("[]") {
            t = PhpType::Generic {
                name: "list".into(),
                args: vec![t],
            };
        }
        while c.eat('|') {
            let alt = c.ident();
            if alt != "null" {
                return Err(self.err(&format!(
                    "a `…|{alt}` union in the docblock type `{}` is Tier-2 (only `|null` lifts)",
                    c.s
                )));
            }
            nullable = true;
        }
        Ok(if nullable {
            PhpType::Nullable(Box::new(t))
        } else {
            t
        })
    }

    fn doc_atom(&mut self, c: &mut Cursor) -> Result<PhpType, String> {
        let name = c.ident();
        if name.is_empty() {
            return Err(self.err(&format!("a type name in the docblock type `{}`", c.s)));
        }
        // Peeled off BEFORE the class-name path: `\Closure` must not register an implicit `use`.
        if matches!(name.as_str(), "callable" | "Closure" | "\\Closure") && c.eat('(') {
            return self.doc_callable(c);
        }
        if c.eat('{') {
            return self.doc_shape(c, &name);
        }
        if c.eat('<') {
            let mut args = Vec::new();
            loop {
                args.push(self.doc_type(c)?);
                if !c.eat(',') {
                    break;
                }
            }
            if !c.eat('>') {
                return Err(self.err(&format!("`>` in the docblock type `{}`", c.s)));
            }
            let head = name.trim_start_matches("non-empty-");
            return match (head, args.len()) {
                ("list", 1) | ("array", 1 | 2) => Ok(PhpType::Generic {
                    name: head.to_string(),
                    args,
                }),
                _ => Err(self.err(&format!(
                    "the docblock generic `{name}<…>` is Tier-2 — `list<T>`, `array<T>`, `array<K, V>` and `T[]` lift"
                ))),
            };
        }
        Ok(match name.as_str() {
            "mixed" | "callable" | "iterable" | "object" | "resource" | "never" => {
                return Err(self.err(&format!("`{name}` in a docblock type is Tier-2")))
            }
            "integer" => PhpType::Named("int".into()),
            "boolean" => PhpType::Named("bool".into()),
            "double" => PhpType::Named("float".into()),
            _ => PhpType::Named(self.doc_class_name(&name)),
        })
    }

    /// `\A\B\C` in a docblock resolves like an inline root-qualified name: last segment + implicit `use`.
    fn doc_class_name(&mut self, name: &str) -> String {
        let path: Vec<String> = name
            .trim_start_matches('\\')
            .split('\\')
            .map(str::to_string)
            .collect();
        let local = path.last().cloned().unwrap_or_default();
        if path.len() > 1 {
            self.note_implicit_use(path);
        }
        local
    }
}

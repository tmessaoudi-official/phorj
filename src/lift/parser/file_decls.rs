//! PHP-lift parser — the FILE level: `parse_program` plus the declarations only legal there —
//! `declare(strict_types=1);` (DEC-401), `namespace A\B;` and `use A\B\C [as D];` (LIFT-NS).
//!
//! Split out of `items.rs` by cohesion: adding these pushed that file to 530 lines, past
//! Invariant 13's 500-line hard cap. They form their own unit — both are consumed by
//! `parse_program` BEFORE item dispatch, because in statement position `use` means
//! trait-composition and `namespace` means the braced multi-namespace form.

use super::*;

impl PParser {
    pub(super) fn parse_program(&mut self) -> Result<PhpProgram, String> {
        // An optional leading `<?php` open tag.
        self.eat(&PTok::OpenTag);
        let mut items = Vec::new();
        let mut docs: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut namespace: Vec<String> = Vec::new();
        let mut uses: Vec<PhpUse> = Vec::new();
        while !self.at(&PTok::Eof) {
            // A `?>` close tag (and a re-opening `<?php`) are tolerated between items.
            if self.eat(&PTok::CloseTag) {
                self.eat(&PTok::OpenTag);
                continue;
            }
            // LIFT-NS: `namespace A\B;` and `use A\B\C [as D];` are FILE-level, so they are consumed
            // here rather than by `parse_item` — which keeps them out of statement position, where a
            // `use` means trait-composition and a `namespace` means the braced multi-namespace form.
            // DEC-401 symmetry: the TRANSPILER now emits `declare(strict_types=1);` in every file, so
            // the lifter must be able to read its own output back (Invariant 17 — transpile and lift
            // move together). `strict_types=1` is phorj's permanent state, so it carries no information
            // to preserve and is simply consumed; any OTHER directive is refused rather than ignored.
            if self.is_kw("declare") {
                self.parse_declare_decl()?;
                continue;
            }
            if self.is_kw("namespace") {
                if !namespace.is_empty() {
                    return Err(self.err_reason(
                        "a second `namespace` declaration — phorj has one `package` per file",
                    ));
                }
                if !items.is_empty() || !uses.is_empty() {
                    // PHP itself is fatal here ("Namespace declaration statement has to be the very
                    // first statement or after any declare call"), so a `use` BEFORE the namespace is
                    // invalid input, not a shape to invent a meaning for.
                    return Err(self.err_reason(
                        "`namespace` must come before every `use` and every declaration in the file",
                    ));
                }
                namespace = self.parse_namespace_decl()?;
                continue;
            }
            if self.is_kw("use") {
                uses.push(self.parse_use_decl()?);
                continue;
            }
            // DEC-419: a PHPDoc block sits in front of the item's FIRST token, so read the side
            // channel at `self.pos` BEFORE parsing consumes it, then key it by the parsed name.
            let doc = self.docs.get(&self.pos).cloned();
            let mut item = self.parse_item()?;
            self.apply_doc_item(doc.as_deref(), &mut item)?;
            if let (Some(d), Some(name)) = (doc, super::super::ast::php_item_name(&item)) {
                docs.insert(name.to_string(), d);
            }
            items.push(item);
        }
        self.merge_implicit_uses(&mut uses);
        Ok(PhpProgram {
            items,
            docs,
            namespace,
            uses,
        })
    }

    /// `declare(strict_types=1);` — consumed and DISCARDED.
    ///
    /// Discarding is lossless for this one directive and only this one: phorj is statically typed with
    /// no coercive mode, so `strict_types=1` states what is already permanently true, and DEC-401 has
    /// the transpiler emit it on the way back out. `strict_types=0` and every other directive
    /// (`ticks`, `encoding`) DO carry meaning phorj cannot express, so they are refused rather than
    /// silently dropped (DEC-166).
    pub(super) fn parse_declare_decl(&mut self) -> Result<(), String> {
        self.advance(); // `declare`
        self.expect(&PTok::LParen, "`(` after `declare`")?;
        let directive = self.expect_ident("a directive name inside `declare(...)`")?;
        if directive != "strict_types" {
            return Err(self.err_reason(&format!(
                "`declare({directive}=…)` has no phorj equivalent — only `strict_types` is understood, because phorj is always strictly typed"
            )));
        }
        self.expect(&PTok::Assign, "`=` inside `declare(strict_types=…)`")?;
        let value = match self.peek().clone() {
            PTok::Int(n) => {
                self.advance();
                n
            }
            _ => return Err(self.err("an integer after `declare(strict_types=`")),
        };
        if value != 1 {
            return Err(self.err_reason(
                "`declare(strict_types=0)` asks for PHP's COERCIVE mode, which phorj has no way to express — every phorj program is strictly typed",
            ));
        }
        self.expect(&PTok::RParen, "`)` after `declare(strict_types=1`")?;
        self.expect(&PTok::Semi, "`;` after `declare(strict_types=1)`")?;
        Ok(())
    }

    /// `namespace A\B;` — the SEMICOLON form only.
    ///
    /// The braced form (`namespace A { … }`) is refused: it can declare several namespaces in one file
    /// and phorj has exactly one `package` per file, so lifting the first and dropping the rest would be
    /// a silent semantic loss. DEC-166 says refuse loudly instead of guessing.
    pub(super) fn parse_namespace_decl(&mut self) -> Result<Vec<String>, String> {
        self.advance(); // `namespace`
        let mut segs = vec![self.expect_ident("a namespace name after `namespace`")?];
        while self.eat(&PTok::Backslash) {
            segs.push(self.expect_ident("a namespace segment after `\\`")?);
        }
        if self.at(&PTok::LBrace) {
            return Err(self.err_reason(
                "a braced `namespace A { … }` block — phorj has one `package` per file; use the `namespace A;` form",
            ));
        }
        self.expect(&PTok::Semi, "`;` after the namespace name")?;
        Ok(segs)
    }

    /// `use A\B\C;` / `use A\B\C as D;` — a CLASS import.
    ///
    /// Refused loudly rather than guessed: `use function f;` / `use const K;` (they import a symbol, not
    /// a type, and phorj has no equivalent) and the group form `use A\{B, C};` (each member would need
    /// its own import, and the brace form also carries trailing-comma and nested-group cases — a
    /// separate increment, not a silent partial lift).
    pub(super) fn parse_use_decl(&mut self) -> Result<PhpUse, String> {
        let line = self.line();
        self.advance(); // `use`
        if self.is_kw("function") || self.is_kw("const") {
            return Err(self.err_reason(
                "`use function` / `use const` imports a symbol rather than a type, which has no phorj equivalent",
            ));
        }
        self.eat(&PTok::Backslash); // an optional leading root marker is not part of the path
        let mut path = vec![self.expect_ident("a class path after `use`")?];
        while self.eat(&PTok::Backslash) {
            if self.at(&PTok::LBrace) {
                return Err(self.err_reason(
                    "a grouped `use A\\{B, C};` import — write one `use` per class for now",
                ));
            }
            path.push(self.expect_ident("a path segment after `\\`")?);
        }
        let alias = if self.is_kw("as") {
            self.advance();
            Some(self.expect_ident("an alias name after `as`")?)
        } else {
            None
        };
        if self.at(&PTok::Comma) {
            return Err(self.err_reason(
                "a comma-separated `use A, B;` import — write one `use` per class for now, so each \
                 gets its own phorj import",
            ));
        }
        self.expect(&PTok::Semi, "`;` after a `use` import")?;
        Ok(PhpUse { path, alias, line })
    }
}

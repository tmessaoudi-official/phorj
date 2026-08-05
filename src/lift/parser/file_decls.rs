//! PHP-lift parser — FILE-level declarations: `namespace A\B;` and `use A\B\C [as D];`
//! (LIFT-NS).
//!
//! Split out of `items.rs` by cohesion: adding these pushed that file to 530 lines, past
//! Invariant 13's 500-line hard cap. They form their own unit — both are consumed by
//! `parse_program` BEFORE item dispatch, because in statement position `use` means
//! trait-composition and `namespace` means the braced multi-namespace form.

use super::*;

impl PParser {
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
            return Err(self.err(
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
            return Err(self.err(
                "`use function` / `use const` imports a symbol rather than a type, which has no phorj equivalent",
            ));
        }
        self.eat(&PTok::Backslash); // an optional leading root marker is not part of the path
        let mut path = vec![self.expect_ident("a class path after `use`")?];
        while self.eat(&PTok::Backslash) {
            if self.at(&PTok::LBrace) {
                return Err(self
                    .err("a grouped `use A\\{B, C};` import — write one `use` per class for now"));
            }
            path.push(self.expect_ident("a path segment after `\\`")?);
        }
        let alias = if self.is_kw("as") {
            self.advance();
            Some(self.expect_ident("an alias name after `as`")?)
        } else {
            None
        };
        self.expect(&PTok::Semi, "`;` after a `use` import")?;
        Ok(PhpUse { path, alias, line })
    }
}

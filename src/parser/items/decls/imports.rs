//! Item parsing — imports: plain, grouped, and wildcard forms (unified-import spec).

use super::*;

impl Parser {
    /// `import a.b.c;` / `import a.b.c as leaf;` — ONE unified import form (2026-07-03 spec): the
    /// loader classifies each import as a module import (Go-qualified `c.fn()` calls) or a
    /// *terminal type* import (the leaf `C` is a user/library type, bound bare or as `D`) by
    /// resolving the path — there is no `type` keyword. `as` is a **contextual** keyword
    /// (recognized only here), so it stays a valid identifier elsewhere. Assumes current token is
    /// `import`.
    pub(in crate::parser) fn parse_import(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect(&TokenKind::Import, "'import'")?;
        // One unified `import` (2026-07-03 unified-import spec): the loader classifies each import as
        // a module (call-qualifier) or a type (bare name) by resolving the path — no `type` keyword.
        let mut path = vec![self.expect_ident("a module path segment")?];
        while self.eat(&TokenKind::Dot) {
            // A `{` after a `.` opens a grouped import `import Prefix.{ leaf, leaf as alias, … };`
            // (DEC-186) — path-first braces (PHP group-use / Rust use-group shape), a single-level
            // prefix listing the leaves under it. Expands to one `Item::Import` per member.
            if self.check(&TokenKind::LBrace) {
                return self.parse_import_group(path, sp);
            }
            // A `*` after a `.` opens a wildcard import `import Prefix.*;` (Q-A) — the loader expands
            // it to every public+internal immediate member (shallow), optionally minus an
            // `except { … }` set. `import X.* as Y` is illegal (E-WILDCARD-ALIAS).
            if self.eat(&TokenKind::Star) {
                return self.parse_import_wildcard(path, sp);
            }
            path.push(self.expect_ident("a module path segment after '.'")?);
        }
        let alias = if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
            self.advance(); // consume `as`
            Some(self.expect_ident("an alias after 'as'")?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon, "';' after import")?;
        Ok(Item::Import {
            path,
            alias,
            wildcard: false,
            except: Vec::new(),
            // Vestigial since the unified-import spec: always false (the loader classifies by path).
            span: sp,
        })
    }

    /// Parse a grouped import's `{ leaf [as alias] (, …)* [,] }` body (the current token is `{`),
    /// terminated by `;`, and expand it into one `Item::Import` per member: `path = prefix + [leaf]`.
    /// Trailing comma and multi-line layout are accepted (newlines are plain whitespace). Returns the
    /// FIRST member's `Item::Import` and stashes the rest in `pending_items` (drained by `parse_program`
    /// in source order). An empty group `{}` is a parse error.
    pub(in crate::parser) fn parse_import_group(
        &mut self,
        prefix: Vec<String>,
        sp: Span,
    ) -> Result<Item, Diagnostic> {
        self.expect(&TokenKind::LBrace, "'{' to open an import group")?;
        let mut members: Vec<(String, Option<String>)> = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let leaf = self.expect_ident("a name in the import group")?;
            let alias = if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
                self.advance(); // consume `as`
                Some(self.expect_ident("an alias after 'as'")?)
            } else {
                None
            };
            members.push((leaf, alias));
            if !self.eat(&TokenKind::Comma) {
                break; // no separator ⇒ the group must close now
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close the import group")?;
        self.expect(&TokenKind::Semicolon, "';' after import")?;
        if members.is_empty() {
            return Err(self
                .error("an import group `{ … }` must name at least one member")
                .with_code("E-IMPORT-GROUP-EMPTY"));
        }
        let mut imports = members.into_iter().map(|(leaf, alias)| {
            let mut path = prefix.clone();
            path.push(leaf);
            Item::Import {
                path,
                alias,
                wildcard: false,
                except: Vec::new(),
                span: sp,
            }
        });
        let first = imports.next().expect("group has ≥1 member");
        self.pending_items.extend(imports);
        Ok(first)
    }

    /// Parse a wildcard import's tail (the `*` was just consumed): an optional `except { a, b [,] }`
    /// exclusion list, then `;`. Returns a single wildcard `Item::Import` whose `path` is the PACKAGE
    /// PREFIX — the loader expands it to per-member imports (public+internal, shallow, sorted, minus
    /// `except`). `except` and `as` are contextual (plain identifiers). `import X.* as Y;` is rejected
    /// as `E-WILDCARD-ALIAS` — a flat wildcard has no single name to bind (use a group or re-import).
    pub(in crate::parser) fn parse_import_wildcard(
        &mut self,
        prefix: Vec<String>,
        sp: Span,
    ) -> Result<Item, Diagnostic> {
        // Q-A: reject `Core.*` wildcards HERE (the parser), before the loader's native/prelude
        // pre-pass intercepts Core imports. Bare `Core.*` would flood the file with the whole
        // stdlib; a Core-SUBMODULE wildcard (`Core.Http.*`) is a deferred follow-up (needs native-
        // registry expansion wired through the prelude pass). Import members explicitly meanwhile.
        if prefix.first().map(String::as_str) == Some("Core") {
            let p = prefix.join(".");
            let msg = if prefix.len() == 1 {
                "`import Core.*;` is not allowed — it would bind the entire standard library; \
                 import a specific member (e.g. `import Core.Output.printLine;`)"
                    .to_string()
            } else {
                format!(
                    "wildcard import of the standard-library module `{p}` (`import {p}.*;`) is not \
                     yet supported — import its members explicitly (e.g. `import {p}.member;`)"
                )
            };
            return Err(self.error_msg(msg).with_code("E-WILDCARD-STDLIB-ROOT"));
        }
        let mut except: Vec<String> = Vec::new();
        if matches!(self.peek(), TokenKind::Ident(s) if s == "except") {
            self.advance(); // consume contextual `except`
            self.expect(&TokenKind::LBrace, "'{' after 'except'")?;
            while !self.check(&TokenKind::RBrace) {
                except.push(self.expect_ident("a name in the 'except' list")?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, "'}' to close the 'except' list")?;
        }
        if matches!(self.peek(), TokenKind::Ident(s) if s == "as") {
            return Err(self
                .error_msg(
                    "a wildcard import `X.*` cannot be aliased (`* as Y`) — a flat wildcard has no \
                     single name to bind; import the member explicitly (`import X.Member as Y;`) \
                     or use a group (`import X.{ Member as Y };`)",
                )
                .with_code("E-WILDCARD-ALIAS"));
        }
        self.expect(&TokenKind::Semicolon, "';' after import")?;
        Ok(Item::Import {
            path: prefix,
            alias: None,
            wildcard: true,
            except,
            span: sp,
        })
    }
}

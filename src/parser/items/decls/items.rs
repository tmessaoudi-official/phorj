//! Item parsing — top-level dispatch: program, package, item, visibility, type alias, test.

use super::*;

impl Parser {
    /// Parse one top-level item: an optional visibility prefix (`public`/`internal`/`private`)
    /// followed by `import` / `function` / `enum` / `class` / `interface` / `type`. The prefix is
    /// stamped onto the declaration by the free `stamp_visibility`.
    pub fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        let sp = self.peek_span();
        // Leading item attributes `#[Route(…)]` (M6 W2) — parsed before any modifier/visibility, PHP
        // order. Only a free `function` may carry them this slice; the target check is below (after
        // visibility/modifiers, at the item keyword).
        let attrs = self.parse_attributes()?;
        // Contextual `test "name" { … }` item (M-Test T1), recognized *before* any modifier parsing.
        // `test` lexes as an ordinary identifier, so it is special only at item position when
        // immediately followed by a string literal — `test` followed by anything else stays a usable
        // name. A leading visibility/`open`/`abstract` modifier therefore never reaches here, so a
        // `public test "x" {}` falls through to the normal item match and is rejected (a test carries
        // no modifiers).
        if self.at_kw("test") && matches!(self.peek2(), TokenKind::Str(_)) {
            return self.parse_test(sp);
        }
        // Contextual `declare function …;` / `declare class … { … }` (M8.5 interop): a foreign PHP
        // symbol. `declare` lexes as an ordinary identifier, special only at item position when followed
        // by `function` or `class`. Attributes/visibility on a foreign decl are rejected inside.
        if self.at_kw("declare") && matches!(self.peek2(), TokenKind::Function | TokenKind::Class) {
            if !attrs.is_empty() {
                let asp = attrs[0].span;
                return Err(Diagnostic::new(
                    Stage::Parse,
                    "attributes (`#[…]`) are not allowed on a foreign `declare`".to_string(),
                    asp.line,
                    asp.col,
                )
                .with_code("E-ATTR-TARGET"));
            }
            return self.parse_declare(sp);
        }
        // Optional leading declaration visibility (visibility modifiers): at most one of
        // public/internal/private. Absent ⇒ the default `Visibility::Public`.
        let vis = self.parse_decl_visibility()?;
        // Optional `open`/`abstract` class prefixes (M-RT S6/S6b), in any order. Both apply only to a
        // class; `abstract` implies extensibility (an abstract class exists to be subclassed), so it
        // also marks the class `open`.
        let mut is_open = false;
        let mut is_abstract = false;
        let mut is_sealed = false;
        loop {
            if self.eat(&TokenKind::Open) {
                is_open = true;
            } else if self.eat(&TokenKind::Abstract) {
                is_abstract = true;
            } else if self.eat(&TokenKind::Sealed) {
                is_sealed = true;
            } else {
                break;
            }
        }
        if (is_open || is_abstract) && !self.check(&TokenKind::Class) {
            return Err(self.error("only a class can be declared `open` or `abstract`"));
        }
        // `sealed` (W5-3) applies to a class OR an interface — both name a closed hierarchy. A sealed
        // class is extensible (its subclasses are the closed set), so it implies `open`.
        if is_sealed && !self.check(&TokenKind::Class) && !self.check(&TokenKind::Interface) {
            return Err(self.error("only a class or interface can be declared `sealed`"));
        }
        // Attribute targets (DEC-194 slice 2a): a top-level `function` or `class` may carry `#[…]`
        // attributes. Other item keywords (enum/interface/trait/import/type) are rejected here
        // (`E-ATTR-TARGET`) until their target slices land.
        if !attrs.is_empty() && !self.check(&TokenKind::Function) && !self.check(&TokenKind::Class)
        {
            let asp = attrs[0].span;
            return Err(Diagnostic::new(
                Stage::Parse,
                "attributes (`#[…]`) are only allowed on a top-level `function` or `class`"
                    .to_string(),
                asp.line,
                asp.col,
            )
            .with_code("E-ATTR-TARGET")
            .with_hint(
                "place the `#[…]` attribute directly above a top-level `function` or `class`",
            ));
        }
        let item = match self.peek() {
            TokenKind::Import => {
                if vis != Visibility::Public {
                    return Err(self.error("an import cannot carry a visibility modifier"));
                }
                return self.parse_import(sp);
            }
            TokenKind::TypeKw => {
                if vis != Visibility::Public {
                    return Err(self.error("a type alias cannot carry a visibility modifier yet"));
                }
                return self.parse_type_alias(sp);
            }
            TokenKind::Function => Item::Function(self.parse_function(Vec::new(), attrs, sp)?),
            TokenKind::Enum => Item::Enum(self.parse_enum(sp)?),
            TokenKind::Class => Item::Class(self.parse_class(
                sp,
                is_open || is_abstract || is_sealed,
                is_abstract,
                is_sealed,
                attrs,
            )?),
            TokenKind::Interface => Item::Interface(self.parse_interface(sp, is_sealed)?),
            TokenKind::Trait => {
                if vis != Visibility::Public {
                    return Err(self.error("a trait cannot carry a visibility modifier yet"));
                }
                return Ok(Item::Trait(self.parse_trait(sp)?));
            }
            TokenKind::Package => {
                return Err(self.error(
                    "'package' must be the first declaration, before any import or definition",
                ))
            }
            _ => {
                return Err(self
                    .error("a top-level item (import, function, enum, class, interface, or type)"))
            }
        };
        Ok(stamp_visibility(item, vis))
    }

    /// Read an optional single leading declaration-visibility keyword. Two visibility keywords in a
    /// row (`public private`) is an error; absent ⇒ the default `Visibility::Public`.
    pub(in crate::parser) fn parse_decl_visibility(&mut self) -> Result<Visibility, Diagnostic> {
        let first = match self.peek() {
            TokenKind::Public => Visibility::Public,
            TokenKind::Internal => Visibility::Internal,
            TokenKind::Private => Visibility::Private,
            _ => return Ok(Visibility::Public),
        };
        self.advance();
        if matches!(
            self.peek(),
            TokenKind::Public | TokenKind::Internal | TokenKind::Private
        ) {
            return Err(self.error("a single visibility (public, internal, or private), not two"));
        }
        Ok(first)
    }

    /// Entry point: parse a whole program — an optional leading `package …;` (M5: required by the
    /// checker, but parsed optionally so its absence is a typed `E-NO-PACKAGE`, not a parse error)
    /// followed by zero or more top-level items until EOF.
    pub fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let sp = self.peek_span();
        let package = if self.check(&TokenKind::Package) {
            self.parse_package()?
        } else {
            Vec::new()
        };
        let mut items = Vec::new();
        while !self.check(&TokenKind::Eof) {
            items.push(self.parse_item()?);
            // Drain any items a desugaring produced beyond the one `parse_item` returned (a grouped
            // import expands to N `Item::Import`); source order is preserved (returned first, rest here).
            items.append(&mut self.pending_items);
        }
        Ok(Program {
            package,
            items,
            span: sp,
        })
    }

    /// `package a.b.c;` — dotted package path at the file top. Assumes current token is `package`.
    pub(in crate::parser) fn parse_package(&mut self) -> Result<Vec<String>, Diagnostic> {
        self.expect(&TokenKind::Package, "'package'")?;
        let mut path = vec![self.expect_ident("a package path segment")?];
        while self.eat(&TokenKind::Dot) {
            path.push(self.expect_ident("a package path segment after '.'")?);
        }
        self.expect(&TokenKind::Semicolon, "';' after package")?;
        Ok(path)
    }

    /// `type Name = Type;` — a top-level alias. Assumes the current token is `type`.
    pub(in crate::parser) fn parse_type_alias(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect(&TokenKind::TypeKw, "'type'")?;
        let name = self.expect_ident("an alias name after 'type'")?;
        self.expect(&TokenKind::Eq, "'=' in type alias")?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Semicolon, "';' after type alias")?;
        Ok(Item::TypeAlias { name, ty, span: sp })
    }

    /// `test "name" { stmts }` (M-Test T1) — assumes the contextual `test` keyword is current and the
    /// next token is a string literal (the caller established both). The name must be a plain string
    /// literal (no interpolation — a test name is a label, not a runtime value); the body is an
    /// ordinary statement block.
    pub(in crate::parser) fn parse_test(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.eat_kw("test", "'test'")?;
        let name = match self.advance().kind {
            TokenKind::Str(segs) => match segs.as_slice() {
                [crate::token::StrSeg::Lit(s)] => s.clone(),
                [] => String::new(),
                _ => {
                    return Err(self.error("a plain test name string (no interpolation)"));
                }
            },
            _ => return Err(self.error("a test name string literal after 'test'")),
        };
        let body = self.parse_block()?;
        Ok(Item::Test {
            name,
            body,
            span: sp,
        })
    }
}

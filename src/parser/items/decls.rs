//! Item parsing — program/package/imports/aliases/tests/functions/declares/attributes/
//! params.

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

    /// `import a.b.c;` / `import a.b.c as leaf;` — a module import (Go-qualified `c.fn()` calls).
    /// `import type a.b.C;` / `import type a.b.C as D;` — a *terminal type* import: the leaf `C` is a
    /// user/library type, bound bare (or as `D`). `type` and `as` are **contextual** keywords
    /// (recognized only here), so they stay valid identifiers elsewhere. Assumes current token is
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
                span: sp,
            }
        });
        let first = imports.next().expect("group has ≥1 member");
        self.pending_items.extend(imports);
        Ok(first)
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

    /// `function name(params) [-> RetType] BLOCK`. `modifiers` are pre-parsed by the caller
    /// (empty for a free function; populated for a method).
    pub(in crate::parser) fn parse_function(
        &mut self,
        modifiers: Vec<Modifier>,
        attrs: Vec<Attribute>,
        sp: Span,
    ) -> Result<FunctionDecl, Diagnostic> {
        self.expect(&TokenKind::Function, "'function'")?;
        let name = self.expect_ident("a function name")?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&TokenKind::LParen, "'(' after function name")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen, "')' to close parameters")?;
        // A-1: `: T` is the canonical return-type syntax (PHP/TS); `-> T` is a silent transition
        // alias (kept until every inline test program is migrated — `.phg` sources use `:`).
        let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // `throws T (| T)* (, T (| T)*)*` (M-faults 2b + M-DOGFOOD W0 comma form). Each entry is a
        // full type (so a union `A | B` is captured natively) and entries may be comma-separated;
        // the checker flattens the `Vec` into the declared throw set. Empty when the clause is absent.
        let throws = if self.eat(&TokenKind::Throws) {
            self.parse_throws_clause()?
        } else {
            Vec::new()
        };
        // M-RT S6b: an `abstract` method is a bodyless signature terminated by `;` (a concrete
        // subclass supplies the body). Every other method/function parses a block.
        let body = if modifiers.contains(&Modifier::Abstract) {
            self.expect(
                &TokenKind::Semicolon,
                "';' after an abstract method signature",
            )?;
            Vec::new()
        } else {
            self.parse_block()?
        };
        Ok(FunctionDecl {
            modifiers,
            attrs,
            vis: Visibility::Public,
            name,
            type_params,
            type_param_bounds,
            params,
            ret,
            throws,
            body,
            foreign: false,
            generic_ret_from_param: None,
            span: sp,
        })
    }

    /// Parse a `declare …` foreign-symbol declaration (M8.5 interop). Currently `declare function
    /// name(params) -> ret;` — a bodyless signature describing an existing PHP function. The result is a
    /// `FunctionDecl` with `foreign: true` and an empty body; the checker validates calls against it but
    /// skips the body, `run`/`runvm` refuse the program (`E-FOREIGN-RUNTIME`), and the transpiler emits
    /// `\name(…)`. (`declare class` is M8.5 S2.)
    pub(in crate::parser) fn parse_declare(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect_ident("'declare'")?; // consume the contextual `declare`
        if self.check(&TokenKind::Class) {
            return self.parse_declare_class(sp);
        }
        self.expect(&TokenKind::Function, "'function' after 'declare'")?;
        let name = self.expect_ident("a foreign function name")?;
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&TokenKind::LParen, "'(' after function name")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen, "')' to close parameters")?;
        let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(
            &TokenKind::Semicolon,
            "';' after a foreign function declaration (it has no body)",
        )?;
        Ok(Item::Function(FunctionDecl {
            modifiers: Vec::new(),
            attrs: Vec::new(),
            vis: Visibility::Public,
            name,
            type_params,
            type_param_bounds,
            params,
            ret,
            throws: Vec::new(),
            body: Vec::new(),
            foreign: true,
            generic_ret_from_param: None,
            span: sp,
        }))
    }

    /// Parse a `declare class Name { … }` foreign-PHP class (M8.5 S2). Members are bodyless signatures
    /// terminated by `;`: a `constructor(params);`, `[static] function name(params) -> ret;`, and
    /// `[public] Type name;` fields. The result is a `ClassDecl` with `foreign: true`; each method is
    /// also `foreign: true` so the checker skips body/totality/casing for it. The transpiler emits
    /// references as the global PHP form (`new \Name`, `\Name::s`, `$o->m`) and no class definition.
    pub(in crate::parser) fn parse_declare_class(&mut self, sp: Span) -> Result<Item, Diagnostic> {
        self.expect(&TokenKind::Class, "'class' after 'declare'")?;
        let name = self.expect_ident("a foreign class name")?;
        // S3a: an optional `extends`/`implements` header describes the *PHP* hierarchy — a foreign
        // exception writes `implements Error` (the built-in marker), making it catchable; a foreign
        // class may also `extends` another foreign class. Purely a type-checker input (no body).
        let extends = if self.eat(&TokenKind::Extends) {
            self.parse_name_list("a class name after 'extends'")?
        } else {
            Vec::new()
        };
        let implements = if self.eat(&TokenKind::Implements) {
            self.parse_name_list("an interface name after 'implements'")?
        } else {
            Vec::new()
        };
        self.expect(&TokenKind::LBrace, "'{' to open the foreign class body")?;
        let mut members = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let msp = self.peek_span();
            let modifiers = self.parse_modifiers();
            match self.peek() {
                TokenKind::Constructor => {
                    self.advance();
                    self.expect(&TokenKind::LParen, "'(' after 'constructor'")?;
                    let params = self.parse_ctor_params()?;
                    self.expect(&TokenKind::RParen, "')' to close constructor parameters")?;
                    // DEC-221: a foreign constructor may declare `throws` too (between `)` and `;`),
                    // describing the PHP constructor's failure surface for the checker.
                    let throws = if self.eat(&TokenKind::Throws) {
                        self.parse_throws_clause()?
                    } else {
                        Vec::new()
                    };
                    self.expect(
                        &TokenKind::Semicolon,
                        "';' after a foreign constructor signature",
                    )?;
                    members.push(ClassMember::Constructor {
                        modifiers,
                        params,
                        throws,
                        body: Vec::new(),
                        span: msp,
                    });
                }
                TokenKind::Function => {
                    self.advance();
                    let mname = self.expect_ident("a method name")?;
                    self.expect(&TokenKind::LParen, "'(' after method name")?;
                    let params = self.parse_params()?;
                    self.expect(&TokenKind::RParen, "')' to close parameters")?;
                    let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    self.expect(
                        &TokenKind::Semicolon,
                        "';' after a foreign method signature",
                    )?;
                    members.push(ClassMember::Method(FunctionDecl {
                        modifiers,
                        attrs: Vec::new(),
                        vis: Visibility::Public,
                        name: mname,
                        type_params: Vec::new(),
                        type_param_bounds: Vec::new(),
                        params,
                        ret,
                        throws: Vec::new(),
                        body: Vec::new(),
                        // The enclosing class is foreign; the *method's* own flag stays false so it is
                        // not mistaken for a free `declare function` by the formatter. The checker skips
                        // its body/totality wholesale (the foreign class is not body-checked), and the
                        // formatter prints it via the `declare class` path.
                        foreign: false,
                        generic_ret_from_param: None,
                        span: msp,
                    }));
                }
                _ => {
                    // A field: `[public] Type name;` — the type describes a readable PHP property.
                    let ty = self.parse_type()?;
                    let fname = self.expect_ident("a foreign field name")?;
                    self.expect(&TokenKind::Semicolon, "';' after a foreign field")?;
                    members.push(ClassMember::Field {
                        modifiers,
                        ty,
                        name: fname,
                        init: None,
                        span: msp,
                    });
                }
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close the foreign class")?;
        Ok(Item::Class(ClassDecl {
            vis: Visibility::Public,
            attrs: Vec::new(), // a foreign `declare` rejects attributes (checked above)
            name,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            extends,
            implements_args: vec![Vec::new(); implements.len()],
            implements,
            open: false,
            is_abstract: false,
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members,
            foreign: true,
            span: sp,
        }))
    }

    /// Parse zero or more leading item attributes `#[ Name ( arg, … ) ]` (M6 W2). Each group is a
    /// single attribute; `#[Name]` with no parens has empty args. Args reuse the expression parser, so
    /// string-literal patterns (`"GET"`, `r"/users/{id}"`) parse as ordinary `Expr`s. Returns the
    /// collected attributes (empty when none) — the caller attaches them to the following item.
    pub(in crate::parser) fn parse_attributes(&mut self) -> Result<Vec<Attribute>, Diagnostic> {
        let mut attrs = Vec::new();
        while self.check(&TokenKind::HashBracket) {
            let sp = self.peek_span();
            self.advance(); // `#[`
            let mut name = self.expect_ident("an attribute name after `#[`")?;
            // Import-redesign S2: a **dotted** attribute name (`#[Http.Route(...)]`) qualifies an
            // injected Core attribute type. Consume the `.Ident` chain and preserve the dotted form;
            // `desugar_router` / attribute validation accept both `Route` (member-imported) and the
            // qualified `Http.Route`. Additive — a `.` here was previously a parse error.
            while self.check(&TokenKind::Dot) {
                self.advance();
                let seg = self.expect_ident("an attribute name segment after `.`")?;
                name.push('.');
                name.push_str(&seg);
            }
            let args = if self.eat(&TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                        if self.check(&TokenKind::RParen) {
                            break; // trailing comma
                        }
                    }
                }
                self.expect(&TokenKind::RParen, "')' to close attribute arguments")?;
                args
            } else {
                Vec::new()
            };
            self.expect(&TokenKind::RBracket, "']' to close the attribute")?;
            attrs.push(Attribute {
                name,
                args,
                span: sp,
            });
        }
        Ok(attrs)
    }

    /// Comma-separated `Type name` parameters up to (not including) `)`.
    /// Allows zero params; allows a trailing comma.
    pub(in crate::parser) fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let sp = self.peek_span();
            let ty = self.parse_type()?;
            // Variadic marker (DEC-298): `int ...nums`. The `...` sits between the element type and
            // the name; the checker gives `nums` the effective type `List<int>` (via the single-sourced
            // `effective_param_ty` helper) and collects a call's trailing args into a `[..]` list at the
            // shared `check_args_defaulted` chokepoint. Must be last + no default (checker-validated).
            let variadic = self.eat(&TokenKind::DotDotDot);
            let name = self.expect_ident("a parameter name")?;
            // Optional default value (M4 default parameters): `bool b = false`. The checker restricts
            // the expression to a literal and enforces trailing-only ordering.
            let default = if self.eat(&TokenKind::Eq) {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            params.push(Param {
                ty,
                name,
                default,
                variadic,
                span: sp,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
                break; // trailing comma
            }
        }
        Ok(params)
    }
}

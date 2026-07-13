//! Item parsing — type declarations: enums, classes, traits, interfaces, members, hooks.

use super::*;

impl Parser {
    /// `enum Name { Variant[(Type field, …)], … }` — assumes current token is `enum`.
    pub(in crate::parser) fn parse_enum(&mut self, sp: Span) -> Result<EnumDecl, Diagnostic> {
        self.expect(&TokenKind::Enum, "'enum'")?;
        let name = self.expect_ident("an enum name")?;
        // Optional generic parameter list `<T, E>` immediately after the enum name (M-RT generic
        // enums) — `enum Result<T, E> { Success(T value), Failure(E error) }`.
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace, "'{' to open enum body")?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let vsp = self.peek_span();
            let vname = self.expect_ident("a variant name")?;
            let fields = if self.eat(&TokenKind::LParen) {
                let f = self.parse_params()?;
                self.expect(&TokenKind::RParen, "')' to close variant fields")?;
                f
            } else {
                Vec::new()
            };
            variants.push(EnumVariant {
                name: vname,
                fields,
                span: vsp,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close enum")?;
        Ok(EnumDecl {
            vis: Visibility::Public,
            name,
            type_params,
            type_param_bounds,
            variants,
            injected: false, // user-written; only `cli::inject_*_prelude` sets this true
            span: sp,
        })
    }

    /// `[open] class Name<T> [extends A, B] [implements I1, I2] { member* }` — assumes current token
    /// is `class`. The `open` flag is parsed at the item level (`parse_item`) and threaded in.
    pub(in crate::parser) fn parse_class(
        &mut self,
        sp: Span,
        open: bool,
        is_abstract: bool,
        sealed: bool,
        attrs: Vec<Attribute>,
    ) -> Result<ClassDecl, Diagnostic> {
        self.expect(&TokenKind::Class, "'class'")?;
        let name = self.expect_ident("a class name")?;
        // Optional generic parameter list `<T, U>` immediately after the class name (M-RT
        // generics-all), before `extends`/`implements` — `class Box<T> extends … implements … { … }`.
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        // Optional `extends A, B` parent-class list (M-RT S6) — before `implements`.
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
        self.expect(&TokenKind::LBrace, "'{' to open class body")?;
        let mut members = Vec::new();
        let mut resolutions = Vec::new();
        let mut uses = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            // A leading contextual `use`/`rename`/`exclude` (lexed as identifiers, never reserved)
            // introduces a clause rather than a member. Types are PascalCase, so these lowercase
            // leaders are unambiguous in member position. M-RT S8 dot-lookahead: `use P.m` (a `.`
            // after the name) is an S6b resolution clause; `use T;` / `use A, B;` is trait composition.
            let leader = if let TokenKind::Ident(kw) = self.peek() {
                Some(kw.clone())
            } else {
                None
            };
            if let Some(kw) = leader {
                match kw.as_str() {
                    "use" => {
                        let is_resolution = matches!(
                            self.tokens.get(self.pos + 2).map(|t| &t.kind),
                            Some(&TokenKind::Dot)
                        );
                        if is_resolution {
                            resolutions.push(self.parse_resolution()?);
                        } else {
                            uses.extend(self.parse_use_traits()?);
                        }
                        continue;
                    }
                    "rename" | "exclude" => {
                        resolutions.push(self.parse_resolution()?);
                        continue;
                    }
                    _ => {}
                }
            }
            members.push(self.parse_class_member()?);
        }
        self.expect(&TokenKind::RBrace, "'}' to close class")?;
        Ok(ClassDecl {
            vis: Visibility::Public,
            attrs,
            name,
            type_params,
            type_param_bounds,
            extends,
            implements,
            open,
            is_abstract,
            sealed,
            resolutions,
            uses,
            members,
            foreign: false,
            span: sp,
        })
    }

    /// M-RT S8 trait composition: `use Name [, Name]* ;` → one or more [`crate::ast::UseTrait`].
    /// Assumes the current token is the contextual `use` keyword and the name is NOT dot-qualified
    /// (the caller disambiguated this from an S6b `use P.m` resolution clause via dot-lookahead).
    pub(in crate::parser) fn parse_use_traits(
        &mut self,
    ) -> Result<Vec<crate::ast::UseTrait>, Diagnostic> {
        self.expect_ident("'use'")?; // consume the contextual `use`
        let mut out = Vec::new();
        loop {
            let sp = self.peek_span();
            let name = self.expect_ident("a trait name after 'use'")?;
            out.push(crate::ast::UseTrait { name, span: sp });
            if self.eat(&TokenKind::Comma) {
                continue;
            }
            break;
        }
        self.expect(&TokenKind::Semicolon, "';' after a trait `use` clause")?;
        Ok(out)
    }

    /// `trait Name { members }` (M-RT S8) — assumes the current token is `trait`. Members use the exact
    /// class-member grammar (methods, fields, const, static, hooks, constructor, abstract requirements).
    /// A trait has no `extends`/`implements`/generics this slice.
    pub(in crate::parser) fn parse_trait(
        &mut self,
        sp: Span,
    ) -> Result<crate::ast::TraitDecl, Diagnostic> {
        self.expect(&TokenKind::Trait, "'trait'")?;
        let name = self.expect_ident("a trait name")?;
        self.expect(&TokenKind::LBrace, "'{' to open trait body")?;
        let mut members = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            members.push(self.parse_class_member()?);
        }
        self.expect(&TokenKind::RBrace, "'}' to close trait")?;
        Ok(crate::ast::TraitDecl {
            name,
            members,
            span: sp,
        })
    }

    /// A multi-inheritance resolution clause (M-RT S6b): `use P.m` | `rename P.m as n` | `exclude P.m`,
    /// with an optional trailing `;`. Assumes the current token is the contextual keyword.
    pub(in crate::parser) fn parse_resolution(
        &mut self,
    ) -> Result<crate::ast::Resolution, Diagnostic> {
        let sp = self.peek_span();
        let kw = self.expect_ident("a resolution clause keyword")?;
        let parent = self.expect_ident("a parent class name")?;
        self.expect(&TokenKind::Dot, "'.' between the parent and method")?;
        let method = self.expect_ident("a method name")?;
        let res = match kw.as_str() {
            "use" => crate::ast::Resolution::Use {
                parent,
                method,
                span: sp,
            },
            "exclude" => crate::ast::Resolution::Exclude {
                parent,
                method,
                span: sp,
            },
            "rename" => {
                let as_kw = self.expect_ident("'as' in a rename clause")?;
                if as_kw != "as" {
                    return Err(self.error("'as' after 'rename P.m'"));
                }
                let as_name = self.expect_ident("the new method name after 'as'")?;
                crate::ast::Resolution::Rename {
                    parent,
                    method,
                    as_name,
                    span: sp,
                }
            }
            _ => unreachable!("caller gated the keyword"),
        };
        // Optional terminator.
        if self.check(&TokenKind::Semicolon) {
            self.advance();
        }
        Ok(res)
    }

    /// `interface Name [extends A, B] { (function sig;)* }` — assumes current token is `interface`.
    /// Each member is a method *signature*: `function name(params) [-> Ret];` with no body, stored as
    /// a `FunctionDecl` whose body is empty (M-RT S2).
    pub(in crate::parser) fn parse_interface(
        &mut self,
        sp: Span,
        sealed: bool,
    ) -> Result<crate::ast::InterfaceDecl, Diagnostic> {
        self.expect(&TokenKind::Interface, "'interface'")?;
        let name = self.expect_ident("an interface name")?;
        let extends = if self.eat(&TokenKind::Extends) {
            self.parse_name_list("an interface name after 'extends'")?
        } else {
            Vec::new()
        };
        self.expect(&TokenKind::LBrace, "'{' to open interface body")?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let msp = self.peek_span();
            self.expect(
                &TokenKind::Function,
                "'function' for an interface method signature",
            )?;
            let mname = self.expect_ident("a method name")?;
            self.expect(&TokenKind::LParen, "'(' after method name")?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen, "')' to close parameters")?;
            // A-1: `:` canonical, `->` transition alias (see `parse_function`).
            let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let throws = if self.eat(&TokenKind::Throws) {
                self.parse_throws_clause()?
            } else {
                Vec::new()
            };
            self.expect(
                &TokenKind::Semicolon,
                "';' after an interface method signature",
            )?;
            methods.push(FunctionDecl {
                modifiers: Vec::new(),
                attrs: Vec::new(),
                vis: Visibility::Public,
                name: mname,
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                params,
                ret,
                throws,
                body: Vec::new(),
                foreign: false,
                generic_ret_from_param: None,
                span: msp,
            });
        }
        self.expect(&TokenKind::RBrace, "'}' to close interface")?;
        Ok(crate::ast::InterfaceDecl {
            vis: Visibility::Public,
            name,
            extends,
            methods,
            sealed,
            span: sp,
        })
    }

    /// A comma-separated list of one-or-more identifiers (no trailing comma), used for a class's
    /// `implements` list and an interface's `extends` list.
    pub(in crate::parser) fn parse_name_list(
        &mut self,
        what: &str,
    ) -> Result<Vec<String>, Diagnostic> {
        let mut names = vec![self.expect_ident(what)?];
        while self.eat(&TokenKind::Comma) {
            names.push(self.expect_ident(what)?);
        }
        Ok(names)
    }

    /// One class member: a field, a constructor, or a method. Modifiers preceding
    /// `constructor` are its own visibility (default public); the checker enforces them at the
    /// construction site and rejects non-visibility modifiers (Soundness Batch A).
    pub(in crate::parser) fn parse_class_member(&mut self) -> Result<ClassMember, Diagnostic> {
        let sp = self.peek_span();
        // Leading member attributes `#[Route(…)]` (M6 W2-ext slice 3) — before modifiers, PHP order.
        // Allowed only on a method; on a constructor/field/hook they are `E-ATTR-TARGET`.
        let attrs = self.parse_attributes()?;
        let modifiers = self.parse_modifiers();
        if !attrs.is_empty() && !self.check(&TokenKind::Function) {
            let asp = attrs[0].span;
            return Err(Diagnostic::new(
                Stage::Parse,
                "attributes (`#[…]`) are only allowed on a method".to_string(),
                asp.line,
                asp.col,
            )
            .with_code("E-ATTR-TARGET")
            .with_hint("place the `#[…]` attribute directly above a `function` member"));
        }
        match self.peek() {
            TokenKind::Constructor => {
                self.advance();
                self.expect(&TokenKind::LParen, "'(' after 'constructor'")?;
                let params = self.parse_ctor_params()?;
                self.expect(&TokenKind::RParen, "')' to close constructor parameters")?;
                let body = self.parse_block()?;
                Ok(ClassMember::Constructor {
                    modifiers,
                    params,
                    body,
                    span: sp,
                })
            }
            TokenKind::Function => Ok(ClassMember::Method(
                self.parse_function(modifiers, attrs, sp)?,
            )),
            _ => {
                // field or property hook: [modifiers] Type name …
                let ty = self.parse_type()?;
                let name = self.expect_ident("a field name")?;
                // A `{` after the name opens a **property hook** body (M-mut.7b):
                // `Type name { get => expr; set(Type v) { stmts } }`. Anything else is a field. A
                // hook is virtual behavior, not storage, so it carries no modifiers (`mutable`/
                // `static` would describe a backing slot it doesn't have).
                if self.check(&TokenKind::LBrace) {
                    if !modifiers.is_empty() {
                        return Err(self.error("a property hook to carry no modifiers"));
                    }
                    return self.parse_property_hook(ty, name, sp);
                }
                // field: [modifiers] Type name [= init] ;
                // An optional field-level initializer (`static mutable int total = 0;`). The checker
                // requires it for `static` fields and forbids it on instance fields (M-mut.7).
                let init = if self.check(&TokenKind::Eq) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(&TokenKind::Semicolon, "';' after field declaration")?;
                Ok(ClassMember::Field {
                    modifiers,
                    ty,
                    name,
                    init,
                    span: sp,
                })
            }
        }
    }

    /// A property hook body (M-mut.7b): `{ get => expr; [set(Type v) { stmts }] }` — clauses in
    /// either order, each at most once, at least one required. Assumes the current token is `{`.
    pub(in crate::parser) fn parse_property_hook(
        &mut self,
        ty: Type,
        name: String,
        sp: Span,
    ) -> Result<ClassMember, Diagnostic> {
        self.expect(&TokenKind::LBrace, "'{' to open a property hook body")?;
        let mut get: Option<Expr> = None;
        let mut set: Option<(Param, Vec<Stmt>)> = None;
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let clause = self.expect_ident("`get` or `set`")?;
            match clause.as_str() {
                "get" => {
                    if get.is_some() {
                        return Err(self.error("a single `get` clause"));
                    }
                    // `get => expr ;`
                    self.expect(&TokenKind::FatArrow, "'=>' after `get`")?;
                    let body = self.parse_expr()?;
                    self.expect(&TokenKind::Semicolon, "';' after the `get` expression")?;
                    get = Some(body);
                }
                "set" => {
                    if set.is_some() {
                        return Err(self.error("a single `set` clause"));
                    }
                    // `set(Type v) { stmts }`
                    self.expect(&TokenKind::LParen, "'(' after `set`")?;
                    let params = self.parse_params()?;
                    self.expect(&TokenKind::RParen, "')' to close the `set` parameter")?;
                    if params.len() != 1 {
                        return Err(self.error("exactly one `set` parameter `set(Type v)`"));
                    }
                    let body = self.parse_block()?;
                    set = Some((params.into_iter().next().unwrap(), body));
                }
                _ => return Err(self.error("`get` or `set` in a property hook")),
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close the property hook body")?;
        if get.is_none() && set.is_none() {
            return Err(self.error("at least a `get` or `set` clause in the property hook"));
        }
        Ok(ClassMember::Hook {
            ty,
            name,
            get,
            set,
            span: sp,
        })
    }

    /// Consume any run of visibility/binding modifiers.
    pub(in crate::parser) fn parse_modifiers(&mut self) -> Vec<Modifier> {
        let mut mods = Vec::new();
        loop {
            let m = match self.peek() {
                TokenKind::Public => Modifier::Public,
                TokenKind::Private => Modifier::Private,
                TokenKind::Protected => Modifier::Protected,
                TokenKind::Const => Modifier::Const,
                // `open` method — opts into override (M-RT S6); final-by-default otherwise.
                TokenKind::Open => Modifier::Open,
                // `mutable` field / promoted ctor param (M-mut.6); immutable by default.
                TokenKind::Mutable => Modifier::Mutable,
                // `static` class field (M-mut.7) — class-level state.
                TokenKind::Static => Modifier::Static,
                // `abstract` method (M-RT S6b) — bodyless, implicitly `open`.
                TokenKind::Abstract => Modifier::Abstract,
                _ => break,
            };
            self.advance();
            mods.push(m);
        }
        mods
    }

    /// Constructor parameters: like normal params, but each may carry promotion modifiers
    /// (`constructor(private string name)`). Allows zero; allows a trailing comma.
    pub(in crate::parser) fn parse_ctor_params(&mut self) -> Result<Vec<CtorParam>, Diagnostic> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let sp = self.peek_span();
            let modifiers = self.parse_modifiers();
            let ty = self.parse_type()?;
            let name = self.expect_ident("a parameter name")?;
            params.push(CtorParam {
                modifiers,
                ty,
                name,
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

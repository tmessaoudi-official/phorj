//! Item parsing — class members: fields, methods, constructors, property hooks, modifiers.

use super::*;

impl Parser {
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
                // DEC-221: an optional `throws T (| T)* (, …)*` clause between the params and the body,
                // reusing the function/interface clause parser. Absent → the ctor throws nothing.
                let throws = if self.eat(&TokenKind::Throws) {
                    self.parse_throws_clause()?
                } else {
                    Vec::new()
                };
                let body = self.parse_block()?;
                Ok(ClassMember::Constructor {
                    modifiers,
                    params,
                    throws,
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

    /// DEC-241: true when the three tokens AFTER the current one are exactly `( set )` — the
    /// asymmetric-visibility suffix of `private(set)` / `protected(set)`. Checked whole (all
    /// three) so `parse_modifiers` can consume unconditionally without a fallible expect.
    fn peek_is_set_group_after(&self) -> bool {
        matches!(self.peek2(), TokenKind::LParen)
            && matches!(self.peek3(), TokenKind::Ident(s) if s == "set")
            && matches!(
                &self.tokens[(self.pos + 3).min(self.tokens.len() - 1)].kind,
                TokenKind::RParen
            )
    }

    /// Consume any run of visibility/binding modifiers.
    pub(in crate::parser) fn parse_modifiers(&mut self) -> Vec<Modifier> {
        let mut mods = Vec::new();
        loop {
            let m = match self.peek() {
                TokenKind::Public => Modifier::Public,
                // DEC-241 asymmetric visibility: `private(set)` / `protected(set)` — the `(set)`
                // suffix is munched here (three tokens), so `private (set)` and plain `private`
                // both keep working. `set` is contextual (an ordinary Ident elsewhere).
                TokenKind::Private if self.peek_is_set_group_after() => {
                    self.advance(); // `private`
                    self.advance(); // `(`
                    self.advance(); // `set`
                    self.advance(); // `)`
                    mods.push(Modifier::PrivateSet);
                    continue;
                }
                TokenKind::Protected if self.peek_is_set_group_after() => {
                    self.advance(); // `protected`
                    self.advance(); // `(`
                    self.advance(); // `set`
                    self.advance(); // `)`
                    mods.push(Modifier::ProtectedSet);
                    continue;
                }
                TokenKind::Private => Modifier::Private,
                TokenKind::Protected => Modifier::Protected,
                // `internal` member (Q-B DV-3) — package-subtree-visible.
                TokenKind::Internal => Modifier::Internal,
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
            // Optional default value (DEC-236 ctor default params): `public string user = ""` —
            // the checker enforces literal-only + trailing-only, and fills call sites (M4 fill).
            let default = if self.eat(&TokenKind::Eq) {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            params.push(CtorParam {
                modifiers,
                ty,
                name,
                default,
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

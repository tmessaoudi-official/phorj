//! Item parsing — functions, parameter lists, and attribute groups.

use super::*;

impl Parser {
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
            // Attribute args reuse the call argument parser, so both positional string-literal
            // patterns (`#[Route("GET", r"/users/{id}")]`) AND named args (`#[Entry(kind: EntryKind.Web)]`,
            // DEC-331 D1) parse uniformly. Named args land as `Expr::NamedArg`; the checker reads
            // them structurally (never as runtime expressions — attribute args are not type-checked).
            let args = if self.eat(&TokenKind::LParen) {
                let args = self.parse_arg_list()?;
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

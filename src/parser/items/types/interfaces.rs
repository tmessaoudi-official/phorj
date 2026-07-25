//! Item parsing — interface declarations (M-RT S2, DEC-257 generic interfaces).

use super::*;

impl Parser {
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
        // Optional `<T, U>` (DEC-257 generic interfaces). Bounds are class/function-only for now —
        // rejected below so `interface I<T: X>` fails loudly instead of silently dropping the bound.
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        if let Some((p, b)) = type_param_bounds.first() {
            return Err(Diagnostic::new(
                Stage::Parse,
                format!(
                    "interface type parameters cannot carry bounds yet — remove `: {b}` from `{p}`"
                ),
                sp.line,
                sp.col,
            ));
        }
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
            type_params,
            extends,
            methods,
            sealed,
            injected: false,
            span: sp,
        })
    }
}

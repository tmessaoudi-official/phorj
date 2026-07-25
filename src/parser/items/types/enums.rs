//! Item parsing — enum declarations (`enum Name[<T>] [: Backing] { Variant… }`).

use super::*;

impl Parser {
    /// `enum Name[<T,…>] [: BackingType] { Variant[(Type field,…)] [= value], … }` — assumes current
    /// token is `enum`. `: BackingType` + per-variant `= value` are DEC-302 backed enums (PHP 8.1).
    pub(in crate::parser) fn parse_enum(&mut self, sp: Span) -> Result<EnumDecl, Diagnostic> {
        self.expect(&TokenKind::Enum, "'enum'")?;
        let name = self.expect_ident("an enum name")?;
        // Optional generic parameter list `<T, E>` immediately after the enum name (M-RT generic
        // enums) — `enum Result<T, E> { Success(T value), Failure(E error) }`.
        let (type_params, type_param_bounds) = self.parse_type_params()?;
        // DEC-302: optional scalar backing type `: string` / `: int` (PHP-8.1 backed enum). Comes
        // after the generic list and before `{`; the checker rejects mixing it with generics/payloads.
        let backing_type = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
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
            // DEC-302: optional scalar `= value` for a backed-enum variant (mirrors a param default).
            let backing_value = if self.eat(&TokenKind::Eq) {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            variants.push(EnumVariant {
                name: vname,
                fields,
                backing_value,
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
            backing_type,
            variants,
            injected: false, // user-written; only `cli::inject_*_prelude` sets this true
            span: sp,
        })
    }
}

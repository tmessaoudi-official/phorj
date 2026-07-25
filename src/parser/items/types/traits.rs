//! Item parsing — traits and multi-inheritance resolution clauses (M-RT S8/S6b).

use super::*;

impl Parser {
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
}

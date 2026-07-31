//! Recursive-descent parser — the statements that carry a CLEANUP or UNWIND edge: `using` (DEC-364),
//! `throw` and `try`/`catch`/`finally` (M-faults 2b). Split out of `stmts.rs` when DEC-364 pushed that
//! file past Invariant 13's cap; grouped by cohesion, not by line count — `using` lowers to exactly
//! the `try`/`finally` these other two build, so the three parse one shared surface.

use super::*;

impl Parser {
    /// `using (T name = init) BLOCK` (DEC-364) — the scope guard. `using` is a contextual keyword
    /// (DEC-364.1), so it arrives as an `Ident` token and is consumed with a bare `advance()`; the
    /// tokenizer knows nothing about it and no identifier is reserved.
    ///
    /// The type is **mandatory** by construction here (there is no inferred form to parse): it is
    /// what lets the checker prove the binding is releasable. `var` would parse as a type and is
    /// rejected by the checker with `E-USING-INFER` rather than by a parse error, so the message can
    /// name the fix.
    pub(super) fn parse_using(&mut self) -> Result<Stmt, Diagnostic> {
        let sp = self.peek_span();
        self.advance(); // the contextual `using`
        self.expect(&TokenKind::LParen, "'(' after 'using'")?;
        // `using (var h = …)` is a natural thing to try, and `parse_type` would read `var` as an
        // ordinary named type — yielding "unknown type `var`", which points at the wrong thing.
        // Capture it as `Type::Infer` so the checker reports `E-USING-INFER`, whose message names the
        // actual fix (spell the type). Rejected in the CHECKER, not here, for that reason.
        let ty = if self.at_kw("var") && matches!(self.peek2(), TokenKind::Ident(_)) {
            let vsp = self.peek_span();
            self.advance();
            Type::Infer(vsp)
        } else {
            self.parse_type()?
        };
        let name = self.expect_ident("a `using` binding name")?;
        self.expect(&TokenKind::Eq, "'=' in a `using` header")?;
        let init = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')' after the `using` header")?;
        let body = self.parse_block()?;
        Ok(Stmt::Using {
            ty,
            name,
            init,
            body,
            span: sp,
        })
    }

    /// `throw expr;` (M-faults 2b).
    pub(super) fn parse_throw(&mut self) -> Result<Stmt, Diagnostic> {
        let sp = self.peek_span();
        self.expect(&TokenKind::Throw, "'throw'")?;
        let value = self.parse_expr()?;
        self.expect(&TokenKind::Semicolon, "';' after 'throw <expr>'")?;
        Ok(Stmt::Throw { value, span: sp })
    }

    /// `try { .. } catch (Type name) { .. } [catch …] [finally { .. }]` (M-faults 2b). Requires at
    /// least one `catch` **or** a `finally` (a bare `try {}` is a parse error). A catch type may be a
    /// union (`catch (A | B e)`), parsed by the shared `parse_type`.
    pub(super) fn parse_try(&mut self) -> Result<Stmt, Diagnostic> {
        let sp = self.peek_span();
        self.expect(&TokenKind::Try, "'try'")?;
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        while self.check(&TokenKind::Catch) {
            let csp = self.peek_span();
            self.advance(); // 'catch'
            self.expect(&TokenKind::LParen, "'(' after 'catch'")?;
            let ty = self.parse_type()?;
            let name = self.expect_ident("a binding name in the catch clause")?;
            self.expect(&TokenKind::RParen, "')' to close the catch clause")?;
            let cbody = self.parse_block()?;
            catches.push(crate::ast::CatchClause {
                ty,
                name,
                body: cbody,
                span: csp,
            });
        }
        let finally_block = if self.eat(&TokenKind::Finally) {
            Some(self.parse_block()?)
        } else {
            None
        };
        if catches.is_empty() && finally_block.is_none() {
            return Err(self.error("'catch' or 'finally' after the try block"));
        }
        Ok(Stmt::Try {
            body,
            catches,
            finally_block,
            span: sp,
        })
    }
}

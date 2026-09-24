//! DEC-532 (scout row 5l): `throw e` as an EXPRESSION. It is parsed only where a value can be
//! abandoned — the right operand of `??`, an `if`-expression arm, a `match`-arm body and a lambda's
//! `=>` body — through [`Parser::parse_throwable_expr`]; its operand is a full expression (PHP's rule).
//! A `throw` reaching [`Parser::parse_primary`] anywhere else is `E-THROW-POSITION`.

use super::*;

impl Parser {
    /// A full expression, or `throw <expr>` when the next token is `throw` — for the four operand
    /// positions DEC-532 allows.
    pub(in crate::parser) fn parse_throwable_expr(&mut self) -> Result<Expr, Diagnostic> {
        if !self.check(&TokenKind::Throw) {
            return self.parse_expr();
        }
        let span = self.peek_span();
        self.advance();
        let value = self.parse_expr()?;
        Ok(Expr::Throw {
            value: Box::new(value),
            span,
        })
    }

    /// `throw` where a throw-expression is not allowed.
    pub(in crate::parser) fn throw_position_error(&self) -> Diagnostic {
        self.error_msg(
            "`throw` is an expression only as the right operand of `??`, an `if`-expression branch, \
             a `match` arm or a lambda's `=>` body",
        )
        .with_code("E-THROW-POSITION")
        .with_hint("write it as a statement here: `throw e;`")
    }
}

//! PHP lifter — PHP 8's throw-expression (DEC-532, scout row 5l). phorj allows `throw e` as an
//! expression in four positions: the right operand of `??`, an `if`-expression arm (PHP's ternary
//! branch), a `match`-arm body and a lambda's `=>` body (PHP's arrow fn). Those four call
//! [`lift_throwable`]; a PHP throw-expression anywhere else (`f(throw $e)`, `$a || throw $e`,
//! `$x = throw $e`) reaches `lift_expr` and is refused with [`POSITION_REFUSAL`].

use super::*;

pub(super) const POSITION_REFUSAL: &str =
    "lift: `throw` as an expression lifts only as the right operand of `??`, a ternary branch, \
     a `match` arm or an arrow-fn body (DEC-532) — rewrite it as a `throw` statement here";

/// Lift an operand in one of the four positions: a PHP throw-expression becomes `Expr::Throw`,
/// anything else lifts as usual.
pub(super) fn lift_throwable(e: &php::PhpExpr) -> Result<Expr, String> {
    match e {
        php::PhpExpr::Throw(value) => Ok(Expr::Throw {
            value: Box::new(lift_expr(value)?),
            span: SP,
        }),
        other => lift_expr(other),
    }
}

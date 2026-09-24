//! Row 5g — DEC-523 as built by DEC-529: PHP `/` is float division, phorj's `/` on two ints is
//! integer division, so a 1:1 operator mapping lifted `7 / 2` to a draft that printed `3` where PHP
//! prints `3.5`. The lifter has no variable types, so every operand is made float: an int literal
//! becomes a float literal (its sign included — `7.0 / -2` does not check), an operand that is
//! already float passes through, and anything else is wrapped in `as float`. A float variable
//! therefore gets a redundant cast, which the checker reports as `W-REDUNDANT-CAST` (a warning).

use super::*;

/// `left / right` with both operands made float. Shared by the binary operator and `$x /= e` so the
/// two construction sites cannot drift.
pub(super) fn float_division(
    left: &php::PhpExpr,
    lhs: Expr,
    right: &php::PhpExpr,
    rhs: Expr,
) -> Expr {
    Expr::Binary {
        op: BinaryOp::Div,
        lhs: Box::new(float_operand(left, lhs)),
        rhs: Box::new(float_operand(right, rhs)),
        span: SP,
    }
}

/// `lifted` (the lift of `php`) as a float-typed operand. Decided on the PHP source, not the lifted
/// expression, because the PHP node is what says whether the value is provably float.
fn float_operand(php: &php::PhpExpr, lifted: Expr) -> Expr {
    use php::PhpExpr as P;
    match php {
        P::Int(n) => Expr::Float(*n as f64, SP),
        P::Unary {
            op: php::PhpUnOp::Neg,
            expr,
        } => match expr.as_ref() {
            P::Int(n) => Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::Float(*n as f64, SP)),
                span: SP,
            },
            P::Float(_) => lifted,
            _ => cast_to_float(lifted),
        },
        P::Float(_) => lifted,
        P::Cast { ty, .. } if ty == "float" => lifted,
        P::Binary {
            op: php::PhpBinOp::Div,
            ..
        } => lifted,
        _ => cast_to_float(lifted),
    }
}

fn cast_to_float(value: Expr) -> Expr {
    Expr::Cast {
        value: Box::new(value),
        type_name: "float".into(),
        span: SP,
    }
}

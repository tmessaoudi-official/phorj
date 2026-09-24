//! DEC-532 (scout row 5l): `throw e` is an EXPRESSION in exactly four positions — the right operand of
//! `??`, an `if`-expression branch, a `match`-arm body, a lambda's `=>` body — and its operand is a
//! full expression (PHP's rule: `s ?? throw f(1) + 2` throws `f(1) + 2`). Everywhere else it is
//! `E-THROW-POSITION`; the statement `throw e;` is unchanged.

use super::support::*;
use crate::ast::{BinaryOp, LambdaBody};

fn thrown(e: &Expr) -> &Expr {
    match e {
        Expr::Throw { value, .. } => value,
        other => panic!("expected a throw-expression, got {other:?}"),
    }
}

#[test]
fn throw_is_the_right_operand_of_coalesce() {
    let Expr::Binary {
        op: BinaryOp::Coalesce,
        rhs,
        ..
    } = expr("s ?? throw new Bad(\"x\")")
    else {
        panic!("expected `??`")
    };
    assert!(matches!(thrown(&rhs), Expr::New(..)), "{rhs:?}");
}

#[test]
fn throw_takes_a_full_expression_operand() {
    let Expr::Binary { rhs, .. } = expr("s ?? throw f(1) + 2") else {
        panic!("expected `??`")
    };
    assert!(
        matches!(
            thrown(&rhs),
            Expr::Binary {
                op: BinaryOp::Add,
                ..
            }
        ),
        "{rhs:?}"
    );
    let Expr::Binary { rhs, .. } = expr("a ?? throw e ?? f") else {
        panic!("expected `??`")
    };
    assert!(
        matches!(
            thrown(&rhs),
            Expr::Binary {
                op: BinaryOp::Coalesce,
                ..
            }
        ),
        "{rhs:?}"
    );
}

#[test]
fn throw_is_an_if_expression_branch_either_side() {
    let Expr::If {
        then_expr,
        else_expr,
        ..
    } = expr("if (c) { throw e } else { throw f }")
    else {
        panic!("expected an if-expression")
    };
    thrown(&then_expr);
    thrown(&else_expr);
}

#[test]
fn throw_is_a_match_arm_body() {
    let Expr::Match { arms, .. } = expr("match (a) { \"x\" => throw e, default => 1 }") else {
        panic!("expected a match")
    };
    thrown(&arms[0].body);
}

#[test]
fn throw_is_a_lambda_expression_body() {
    let Expr::Lambda { body, .. } = expr("function(int x): int throws Bad => throw new Bad(\"l\")")
    else {
        panic!("expected a lambda")
    };
    let LambdaBody::Expr(b) = body else {
        panic!("expected an expression body")
    };
    thrown(&b);
}

#[test]
fn throw_anywhere_else_is_refused_by_name() {
    for src in [
        "function f(): int { return 1 + throw e; }",
        "function f(): int { return g(throw e); }",
        "function f(): void { var x = throw e; }",
        "function f(): int { return throw e; }",
        "function f(bool c): bool { return c || throw e; }",
    ] {
        let msg = prog_err(src);
        assert!(msg.contains("E-THROW-POSITION"), "{src}\n{msg}");
    }
}

#[test]
fn the_throw_statement_is_unchanged() {
    assert!(matches!(stmt("throw new Bad(\"x\");"), Stmt::Throw { .. }));
}

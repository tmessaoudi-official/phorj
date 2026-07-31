//! The shared **child-expression enumerator** — `push_subexprs` and its statement helper. Split out
//! of `walk.rs` when DEC-364 pushed that file past Invariant 13's cap; a distinct concern from the
//! free-variable/`this`-use analyses that remain there: this one enumerates one level of children so a
//! worklist can reach every nested expression, and answers no question of its own.
//!
//! Exhaustive over `Expr` and `Stmt` (Invariant 3 / DEC-356) — a new variant extends it in the same
//! change, by design.

use super::*;

/// Push borrowed references to every direct sub-expression of `e` — one level, including the
/// expressions held by a block-body lambda's statements — so a worklist seeded with one expression
/// reaches every nested expression. The shared child-enumerator for worklist walks (first user:
/// the parser's DEC-239 pipe placeholder validation). Exhaustive over `Expr`, so a new variant
/// extends this in the same change.
pub fn push_subexprs<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::Decimal { .. }
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::Bytes(..)
        | Expr::Ident(..)
        | Expr::This(..)
        | Expr::Inject { .. }
        | Expr::NewColl { .. }
        | Expr::PipePlaceholder(..) => {}
        Expr::Str(parts, _) | Expr::Html(parts, _) | Expr::TaggedTemplate { parts, .. } => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    out.push(x);
                }
            }
        }
        Expr::Unary { expr, .. } => out.push(expr),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => out.push(inner),
        Expr::Binary { lhs, rhs, .. } | Expr::Pipe { lhs, rhs, .. } => {
            out.push(lhs);
            out.push(rhs);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => out.push(value),
        Expr::Call { callee, args, .. } => {
            out.push(callee);
            out.extend(args.iter());
        }
        Expr::Member { object, .. } => out.push(object),
        Expr::Index { object, index, .. } => {
            out.push(object);
            out.push(index);
        }
        Expr::List(xs, _) | Expr::Tuple(xs, _) => out.extend(xs.iter()),
        Expr::NamedArg { value, .. } => out.push(value),
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                out.push(k);
                out.push(v);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            out.push(scrutinee);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    out.push(g);
                }
                out.push(&arm.body);
            }
        }
        Expr::Range { start, end, .. } => {
            out.push(start);
            out.push(end);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            out.push(cond);
            out.push(then_expr);
            out.push(else_expr);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => out.push(x),
            LambdaBody::Block(stmts) => push_stmt_exprs(stmts, out),
        },
        Expr::CloneWith { object, fields, .. } => {
            out.push(object);
            for (_, v) in fields {
                out.push(v);
            }
        }
        Expr::New(inner, _) => out.push(inner),
        Expr::Spawn { call, .. } | Expr::OverloadSelect { call, .. } => out.push(call),
        Expr::ParentCall { args, .. } => out.extend(args.iter()),
    }
}

/// The statement half of [`push_subexprs`]: push every expression a statement list holds directly.
fn push_stmt_exprs<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Expr>) {
    for s in stmts {
        match s {
            Stmt::VarDecl { init, .. } => out.push(init),
            Stmt::Assign { target, value, .. } => {
                out.push(target);
                out.push(value);
            }
            Stmt::Return { value, .. } => {
                if let Some(e) = value {
                    out.push(e);
                }
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                out.push(cond);
                push_stmt_exprs(then_block, out);
                if let Some(b) = else_block {
                    push_stmt_exprs(b, out);
                }
            }
            Stmt::For { iter, body, .. } => {
                out.push(iter);
                push_stmt_exprs(body, out);
            }
            Stmt::Using { init, body, .. } => {
                out.push(init);
                push_stmt_exprs(body, out);
            }
            Stmt::While { cond, body, .. } => {
                out.push(cond);
                push_stmt_exprs(body, out);
            }
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                ..
            } => {
                if let Some(i) = init {
                    push_stmt_exprs(std::slice::from_ref(i), out);
                }
                if let Some(c) = cond {
                    out.push(c);
                }
                if let Some(st) = step {
                    push_stmt_exprs(std::slice::from_ref(st), out);
                }
                push_stmt_exprs(body, out);
            }
            Stmt::Block(b, _) => push_stmt_exprs(b, out),
            Stmt::Destructure {
                init, else_block, ..
            } => {
                out.push(init);
                if let Some(eb) = else_block {
                    push_stmt_exprs(eb, out);
                }
            }
            Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => out.push(e),
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                push_stmt_exprs(body, out);
                for c in catches {
                    push_stmt_exprs(&c.body, out);
                }
                if let Some(fb) = finally_block {
                    push_stmt_exprs(fb, out);
                }
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

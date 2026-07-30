//! `desugar_auto_router`'s total AST walk — split out of `desugar_router.rs` (Invariant 13, M-Decomp).
//!
//! DEC-356 added six recursion arms here (`Tuple`, `NamedArg`, `ParentCall`, `OverloadSelect`,
//! `TaggedTemplate`, `Pipe` — all previously swallowed by one `leaf => leaf`), which pushed the parent
//! past its ceiling. The walk trio is the natural cohesion unit: it is the pass's *traversal*, distinct
//! from its route collection and router construction, and it is what DEC-356's follow-up B (one shared
//! total visitor) would eventually consolidate across all these rewriters.

use super::*;

pub(super) fn rexpr(e: Expr, r: &[Route]) -> Expr {
    match e {
        Expr::Call {
            callee,
            args,
            type_args,
            span,
        } => {
            if is_auto_router(&callee, &args) {
                build_router(r, span)
            } else {
                Expr::Call {
                    callee: Box::new(rexpr(*callee, r)),
                    args: args.into_iter().map(|a| rexpr(a, r)).collect(),
                    type_args,
                    span,
                }
            }
        }
        Expr::Str(parts, span) => Expr::Str(
            parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, r))),
                    lit => lit,
                })
                .collect(),
            span,
        ),
        Expr::List(items, span) => {
            Expr::List(items.into_iter().map(|e| rexpr(e, r)).collect(), span)
        }
        Expr::Map(pairs, span) => Expr::Map(
            pairs
                .into_iter()
                .map(|(k, v)| (rexpr(k, r), rexpr(v, r)))
                .collect(),
            span,
        ),
        Expr::Unary { op, expr, span } => Expr::Unary {
            op,
            expr: Box::new(rexpr(*expr, r)),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(rexpr(*lhs, r)),
            rhs: Box::new(rexpr(*rhs, r)),
            span,
        },
        Expr::InstanceOf {
            value,
            type_name,
            span,
        } => Expr::InstanceOf {
            value: Box::new(rexpr(*value, r)),
            type_name,
            span,
        },
        Expr::Cast {
            value,
            type_name,
            span,
        } => Expr::Cast {
            value: Box::new(rexpr(*value, r)),
            type_name,
            span,
        },
        Expr::Member {
            object,
            name,
            safe,
            sep: _,
            span,
        } => Expr::Member {
            object: Box::new(rexpr(*object, r)),
            name,
            safe,
            sep: crate::ast::MemberSep::Dot,
            span,
        },
        Expr::Index {
            object,
            index,
            span,
        } => Expr::Index {
            object: Box::new(rexpr(*object, r)),
            index: Box::new(rexpr(*index, r)),
            span,
        },
        Expr::Force { inner, span } => Expr::Force {
            inner: Box::new(rexpr(*inner, r)),
            span,
        },
        Expr::Propagate { inner, span } => Expr::Propagate {
            inner: Box::new(rexpr(*inner, r)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(rexpr(*scrutinee, r)),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| rexpr(g, r)),
                    body: rexpr(a.body, r),
                    span: a.span,
                })
                .collect(),
            span,
        },
        Expr::Range {
            start,
            end,
            inclusive,
            span,
        } => Expr::Range {
            start: Box::new(rexpr(*start, r)),
            end: Box::new(rexpr(*end, r)),
            inclusive,
            span,
        },
        Expr::If {
            cond,
            then_expr,
            else_expr,
            span,
        } => Expr::If {
            cond: Box::new(rexpr(*cond, r)),
            then_expr: Box::new(rexpr(*then_expr, r)),
            else_expr: Box::new(rexpr(*else_expr, r)),
            span,
        },
        Expr::Lambda {
            params,
            ret,
            throws,
            body,
            span,
        } => Expr::Lambda {
            params,
            ret,
            throws,
            body: match body {
                LambdaBody::Expr(e) => LambdaBody::Expr(Box::new(rexpr(*e, r))),
                LambdaBody::Block(stmts) => LambdaBody::Block(rblock(stmts, r)),
            },
            span,
        },
        Expr::CloneWith {
            object,
            fields,
            span,
        } => Expr::CloneWith {
            object: Box::new(rexpr(*object, r)),
            fields: fields.into_iter().map(|(n, e)| (n, rexpr(e, r))).collect(),
            span,
        },
        Expr::New(inner, span) => Expr::New(Box::new(rexpr(*inner, r)), span),
        // `spawn <call>` (M6 W4): walk the nested call.
        Expr::Spawn { call, span } => Expr::Spawn {
            call: Box::new(rexpr(*call, r)),
            span,
        },
        Expr::Html(parts, span) => Expr::Html(parts, span),
        // leaves carry no nested expression: Int / Float / Bool / Null / Bytes / Ident / This
        // DEC-356: these six bear expressions and were silently passed through by `leaf => leaf`.
        Expr::Tuple(items, span) => {
            Expr::Tuple(items.into_iter().map(|e| rexpr(e, r)).collect(), span)
        }
        Expr::NamedArg { name, value, span } => Expr::NamedArg {
            name,
            value: Box::new(rexpr(*value, r)),
            span,
        },
        Expr::ParentCall {
            ancestor,
            method,
            args,
            span,
        } => Expr::ParentCall {
            ancestor,
            method,
            args: args.into_iter().map(|a| rexpr(a, r)).collect(),
            span,
        },
        Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
            ty,
            call: Box::new(rexpr(*call, r)),
            span,
        },
        Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
            tag,
            parts: parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, r))),
                    StrPart::Literal(s) => StrPart::Literal(s),
                })
                .collect(),
            span,
        },
        Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
            lhs: Box::new(rexpr(*lhs, r)),
            rhs: Box::new(rexpr(*rhs, r)),
            span,
        },
        // Carries no nested expression — the set is single-sourced in `ast::leaves`, so adding an
        // `Expr` variant breaks the build here until someone rules whether it is a leaf (DEC-356).
        e @ (crate::expr_leaves!() | Expr::NewColl { .. } | Expr::Inject { .. }) => e,
    }
}

pub(super) fn rstmt(s: Stmt, r: &[Route]) -> Stmt {
    match s {
        Stmt::VarDecl {
            ty,
            name,
            init,
            mutable,
            span,
        } => Stmt::VarDecl {
            ty,
            name,
            init: rexpr(init, r),
            mutable,
            span,
        },
        Stmt::Assign {
            target,
            value,
            span,
        } => Stmt::Assign {
            target: rexpr(target, r),
            value: rexpr(value, r),
            span,
        },
        Stmt::Return { value, span } => Stmt::Return {
            value: value.map(|e| rexpr(e, r)),
            span,
        },
        Stmt::If {
            cond,
            bind,
            then_block,
            else_block,
            span,
        } => Stmt::If {
            cond: rexpr(cond, r),
            bind,
            then_block: rblock(then_block, r),
            else_block: else_block.map(|b| rblock(b, r)),
            span,
        },
        Stmt::For {
            ty,
            name,
            val,
            iter,
            body,
            span,
        } => Stmt::For {
            ty,
            name,
            val,
            iter: rexpr(iter, r),
            body: rblock(body, r),
            span,
        },
        Stmt::While {
            cond,
            body,
            post_cond,
            span,
        } => Stmt::While {
            cond: rexpr(cond, r),
            body: rblock(body, r),
            post_cond,
            span,
        },
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            span,
        } => Stmt::CFor {
            init: init.map(|s| Box::new(rstmt(*s, r))),
            cond: cond.map(|e| rexpr(e, r)),
            step: step.map(|s| Box::new(rstmt(*s, r))),
            body: rblock(body, r),
            span,
        },
        Stmt::Break(span) => Stmt::Break(span),
        Stmt::Continue(span) => Stmt::Continue(span),
        Stmt::Block(stmts, span) => Stmt::Block(rblock(stmts, r), span),
        Stmt::Expr(e, span) => Stmt::Expr(rexpr(e, r), span),
        Stmt::Discard(e, span) => Stmt::Discard(rexpr(e, r), span),
        Stmt::Throw { value, span } => Stmt::Throw {
            value: rexpr(value, r),
            span,
        },
        Stmt::Try {
            body,
            catches,
            finally_block,
            span,
        } => Stmt::Try {
            body: rblock(body, r),
            catches: catches
                .into_iter()
                .map(|c| CatchClause {
                    ty: c.ty,
                    name: c.name,
                    body: rblock(c.body, r),
                    span: c.span,
                })
                .collect(),
            finally_block: finally_block.map(|b| rblock(b, r)),
            span,
        },
        Stmt::Destructure {
            pat,
            init,
            else_block,
            span,
        } => Stmt::Destructure {
            pat,
            init: rexpr(init, r),
            else_block: else_block.map(|b| rblock(b, r)),
            span,
        },
    }
}

pub(super) fn rblock(stmts: Vec<Stmt>, r: &[Route]) -> Vec<Stmt> {
    stmts.into_iter().map(|s| rstmt(s, r)).collect()
}

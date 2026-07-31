//! Import-aware STATEMENT resolution — `resolve_block` / `resolve_stmt`. Split out of `resolve.rs`
//! when DEC-364 pushed that file past Invariant 13's cap; cohesive on its own (the type, item and
//! expression resolvers stay in the parent, which this calls into).
//!
//! Exhaustive over `Stmt` (Invariant 3 / DEC-356): every statement that carries a type, an expression
//! or a nested block must be rebuilt with the resolved parts, so a new variant has to be considered
//! here rather than silently passing through unresolved.

use super::resolve::{resolve_expr, resolve_type};
use super::*;

pub(super) fn resolve_block(stmts: Vec<Stmt>, ctx: &ResolveCtx) -> Vec<Stmt> {
    stmts.into_iter().map(|s| resolve_stmt(s, ctx)).collect()
}

pub(super) fn resolve_stmt(stmt: Stmt, ctx: &ResolveCtx) -> Stmt {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            init,
            mutable,
            span,
        } => Stmt::VarDecl {
            ty: resolve_type(&ty, ctx),
            name,
            init: resolve_expr(init, ctx),
            mutable,
            span,
        },
        Stmt::Assign {
            target,
            value,
            span,
        } => Stmt::Assign {
            target: resolve_expr(target, ctx),
            value: resolve_expr(value, ctx),
            span,
        },
        Stmt::Return { value, span } => Stmt::Return {
            value: value.map(|e| resolve_expr(e, ctx)),
            span,
        },
        Stmt::If {
            cond,
            bind,
            then_block,
            else_block,
            span,
        } => Stmt::If {
            cond: resolve_expr(cond, ctx),
            bind,
            then_block: resolve_block(then_block, ctx),
            else_block: else_block.map(|b| resolve_block(b, ctx)),
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
            ty: resolve_type(&ty, ctx),
            name,
            val: val.map(|(t, n)| (resolve_type(&t, ctx), n)),
            iter: resolve_expr(iter, ctx),
            body: resolve_block(body, ctx),
            span,
        },
        Stmt::Using {
            ty,
            name,
            init,
            body,
            span,
        } => Stmt::Using {
            ty: resolve_type(&ty, ctx),
            name,
            init: resolve_expr(init, ctx),
            body: resolve_block(body, ctx),
            span,
        },
        Stmt::While {
            cond,
            body,
            post_cond,
            span,
        } => Stmt::While {
            cond: resolve_expr(cond, ctx),
            body: resolve_block(body, ctx),
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
            init: init.map(|s| Box::new(resolve_stmt(*s, ctx))),
            cond: cond.map(|e| resolve_expr(e, ctx)),
            step: step.map(|s| Box::new(resolve_stmt(*s, ctx))),
            body: resolve_block(body, ctx),
            span,
        },
        Stmt::Break(span) => Stmt::Break(span),
        Stmt::Continue(span) => Stmt::Continue(span),
        // Slice 5: mangle a cross-package struct head to its FQN (mirrors `instanceof`/`new`), and
        // resolve the init expr + the `else` block. A list pattern carries no type name.
        Stmt::Destructure {
            pat,
            init,
            else_block,
            span,
        } => {
            let pat = match pat {
                crate::ast::DestructurePat::Struct {
                    type_name,
                    fields,
                    span: psp,
                } => crate::ast::DestructurePat::Struct {
                    type_name: resolve_type_ref(&type_name, ctx).unwrap_or(type_name),
                    fields,
                    span: psp,
                },
                list => list,
            };
            Stmt::Destructure {
                pat,
                init: resolve_expr(init, ctx),
                else_block: else_block.map(|b| resolve_block(b, ctx)),
                span,
            }
        }
        Stmt::Block(stmts, span) => Stmt::Block(resolve_block(stmts, ctx), span),
        Stmt::Expr(e, span) => Stmt::Expr(resolve_expr(e, ctx), span),
        Stmt::Discard(e, span) => Stmt::Discard(resolve_expr(e, ctx), span),
        Stmt::Throw { value, span } => Stmt::Throw {
            value: resolve_expr(value, ctx),
            span,
        },
        Stmt::Try {
            body,
            catches,
            finally_block,
            span,
        } => Stmt::Try {
            body: resolve_block(body, ctx),
            catches: catches
                .into_iter()
                .map(|c| crate::ast::CatchClause {
                    ty: resolve_type(&c.ty, ctx),
                    name: c.name,
                    body: resolve_block(c.body, ctx),
                    span: c.span,
                })
                .collect(),
            finally_block: finally_block.map(|b| resolve_block(b, ctx)),
            span,
        },
    }
}

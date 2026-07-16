//! The shared bottom-up expression visitor for the DEC-239 pipe passes (`lower_pipes` /
//! `materialize_pipe_params`): every expression in the program is visited children-first, then
//! handed to the closure, which may rewrite it in place. Bottom-up order is what lets a nested
//! pipe (anywhere in an outer pipe's operands) lower — placeholders substituted — before the
//! outer pipe's own lowering inspects its RHS.

use super::*;
use crate::ast::{CatchClause, ClassMember, Expr, Item, LambdaBody, MatchArm, Stmt, StrPart};

/// Visit every expression of the program bottom-up, applying `f` to each (children first).
pub(super) fn visit_exprs_mut(program: &mut Program, f: &mut impl FnMut(&mut Expr)) {
    for item in &mut program.items {
        match item {
            Item::Function(func) => vblock(&mut func.body, f),
            Item::Class(c) => vmembers(&mut c.members, f),
            Item::Trait(t) => vmembers(&mut t.members, f),
            // Enums/interfaces/imports/aliases carry no expressions to rewrite.
            _ => {}
        }
    }
}

fn vmembers(members: &mut [ClassMember], f: &mut impl FnMut(&mut Expr)) {
    for m in members {
        match m {
            ClassMember::Method(func) => vblock(&mut func.body, f),
            ClassMember::Constructor { body, .. } => vblock(body, f),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init {
                    vexpr(e, f);
                }
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    vexpr(g, f);
                }
                if let Some((_, body)) = set {
                    vblock(body, f);
                }
            }
        }
    }
}

fn vblock(stmts: &mut [Stmt], f: &mut impl FnMut(&mut Expr)) {
    for s in stmts {
        vstmt(s, f);
    }
}

fn vstmt(s: &mut Stmt, f: &mut impl FnMut(&mut Expr)) {
    match s {
        Stmt::VarDecl { init, .. } => vexpr(init, f),
        Stmt::Assign { target, value, .. } => {
            vexpr(target, f);
            vexpr(value, f);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                vexpr(e, f);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            vexpr(cond, f);
            vblock(then_block, f);
            if let Some(b) = else_block {
                vblock(b, f);
            }
        }
        Stmt::For { iter, body, .. } => {
            vexpr(iter, f);
            vblock(body, f);
        }
        Stmt::While { cond, body, .. } => {
            vexpr(cond, f);
            vblock(body, f);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                vstmt(i, f);
            }
            if let Some(c) = cond {
                vexpr(c, f);
            }
            if let Some(st) = step {
                vstmt(st, f);
            }
            vblock(body, f);
        }
        Stmt::Block(b, _) => vblock(b, f),
        Stmt::Destructure {
            init, else_block, ..
        } => {
            vexpr(init, f);
            if let Some(eb) = else_block {
                vblock(eb, f);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => vexpr(e, f),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            vblock(body, f);
            for CatchClause { body, .. } in catches {
                vblock(body, f);
            }
            if let Some(fb) = finally_block {
                vblock(fb, f);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

pub(super) fn vexpr(e: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    match e {
        Expr::Unary { expr, .. } => vexpr(expr, f),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => vexpr(inner, f),
        Expr::Binary { lhs, rhs, .. } | Expr::Pipe { lhs, rhs, .. } => {
            vexpr(lhs, f);
            vexpr(rhs, f);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => vexpr(value, f),
        Expr::Call { callee, args, .. } => {
            vexpr(callee, f);
            for a in args {
                vexpr(a, f);
            }
        }
        Expr::Member { object, .. } => vexpr(object, f),
        Expr::Index { object, index, .. } => {
            vexpr(object, f);
            vexpr(index, f);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) | Expr::TaggedTemplate { parts, .. } => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    vexpr(x, f);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                vexpr(x, f);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                vexpr(k, f);
                vexpr(v, f);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            vexpr(scrutinee, f);
            for MatchArm { guard, body, .. } in arms {
                if let Some(g) = guard {
                    vexpr(g, f);
                }
                vexpr(body, f);
            }
        }
        Expr::Range { start, end, .. } => {
            vexpr(start, f);
            vexpr(end, f);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            vexpr(cond, f);
            vexpr(then_expr, f);
            vexpr(else_expr, f);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => vexpr(x, f),
            LambdaBody::Block(b) => vblock(b, f),
        },
        Expr::CloneWith { object, fields, .. } => {
            vexpr(object, f);
            for (_, v) in fields {
                vexpr(v, f);
            }
        }
        Expr::New(inner, _) => vexpr(inner, f),
        Expr::Spawn { call, .. } => vexpr(call, f),
        Expr::OverloadSelect { call, .. } => vexpr(call, f),
        Expr::ParentCall { args, .. } => {
            for a in args {
                vexpr(a, f);
            }
        }
        // Literals / `Ident` / `This` / `Inject` / `PipePlaceholder` have no sub-expressions.
        // (A placeholder is substituted by its owning pipe's lowering.)
        _ => {}
    }
    f(e);
}

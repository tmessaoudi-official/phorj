//! The shared bottom-up expression visitor for the DEC-239 pipe passes (`lower_pipes` /
//! `materialize_pipe_params`): every expression in the program is visited children-first, then
//! handed to the closure, which may rewrite it in place. Bottom-up order is what lets a nested
//! pipe (anywhere in an outer pipe's operands) lower — placeholders substituted — before the
//! outer pipe's own lowering inspects its RHS.

use super::*;
use crate::ast::{CatchClause, ClassMember, Expr, Item, LambdaBody, MatchArm, Stmt, StrPart};

/// Visit every expression of the program bottom-up, applying `f` to each (children first).
pub(in crate::checker) fn visit_exprs_mut(program: &mut Program, f: &mut impl FnMut(&mut Expr)) {
    visit_exprs_mut_ordered(program, false, f);
}

/// Visit every expression TOP-DOWN (`f` first, then the — possibly freshly spliced — children).
/// The order the default-fill pass needs: a parent splice restores original (unfilled) child
/// subtrees, which the subsequent child recursion then fills; bottom-up would instead let the
/// parent splice DESTROY already-applied child fills (the nested `db.transaction` hang).
pub(in crate::checker) fn visit_exprs_mut_pre(
    program: &mut Program,
    f: &mut impl FnMut(&mut Expr),
) {
    visit_exprs_mut_ordered(program, true, f);
}

fn visit_exprs_mut_ordered(program: &mut Program, pre: bool, f: &mut impl FnMut(&mut Expr)) {
    for item in &mut program.items {
        match item {
            Item::Function(func) => vblock(&mut func.body, pre, f),
            Item::Class(c) => vmembers(&mut c.members, pre, f),
            Item::Trait(t) => vmembers(&mut t.members, pre, f),
            // Enums/interfaces/imports/aliases carry no expressions to rewrite.
            _ => {}
        }
    }
}

fn vmembers(members: &mut [ClassMember], pre: bool, f: &mut impl FnMut(&mut Expr)) {
    for m in members {
        match m {
            ClassMember::Method(func) => vblock(&mut func.body, pre, f),
            ClassMember::Constructor { body, .. } => vblock(body, pre, f),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init {
                    vexpr(e, pre, f);
                }
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    vexpr(g, pre, f);
                }
                if let Some((_, body)) = set {
                    vblock(body, pre, f);
                }
            }
        }
    }
}

fn vblock(stmts: &mut [Stmt], pre: bool, f: &mut impl FnMut(&mut Expr)) {
    for s in stmts {
        vstmt(s, pre, f);
    }
}

fn vstmt(s: &mut Stmt, pre: bool, f: &mut impl FnMut(&mut Expr)) {
    match s {
        Stmt::VarDecl { init, .. } => vexpr(init, pre, f),
        Stmt::Assign { target, value, .. } => {
            vexpr(target, pre, f);
            vexpr(value, pre, f);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                vexpr(e, pre, f);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            vexpr(cond, pre, f);
            vblock(then_block, pre, f);
            if let Some(b) = else_block {
                vblock(b, pre, f);
            }
        }
        Stmt::For { iter, body, .. } => {
            vexpr(iter, pre, f);
            vblock(body, pre, f);
        }
        Stmt::While { cond, body, .. } => {
            vexpr(cond, pre, f);
            vblock(body, pre, f);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                vstmt(i, pre, f);
            }
            if let Some(c) = cond {
                vexpr(c, pre, f);
            }
            if let Some(st) = step {
                vstmt(st, pre, f);
            }
            vblock(body, pre, f);
        }
        Stmt::Block(b, _) => vblock(b, pre, f),
        Stmt::Destructure {
            init, else_block, ..
        } => {
            vexpr(init, pre, f);
            if let Some(eb) = else_block {
                vblock(eb, pre, f);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => vexpr(e, pre, f),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            vblock(body, pre, f);
            for CatchClause { body, .. } in catches {
                vblock(body, pre, f);
            }
            if let Some(fb) = finally_block {
                vblock(fb, pre, f);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

pub(super) fn vexpr(e: &mut Expr, pre: bool, f: &mut impl FnMut(&mut Expr)) {
    if pre {
        f(e);
    }
    match e {
        Expr::Unary { expr, .. } => vexpr(expr, pre, f),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => vexpr(inner, pre, f),
        Expr::Binary { lhs, rhs, .. } | Expr::Pipe { lhs, rhs, .. } => {
            vexpr(lhs, pre, f);
            vexpr(rhs, pre, f);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => vexpr(value, pre, f),
        Expr::Call { callee, args, .. } => {
            vexpr(callee, pre, f);
            for a in args {
                vexpr(a, pre, f);
            }
        }
        Expr::Member { object, .. } => vexpr(object, pre, f),
        Expr::Index { object, index, .. } => {
            vexpr(object, pre, f);
            vexpr(index, pre, f);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) | Expr::TaggedTemplate { parts, .. } => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    vexpr(x, pre, f);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                vexpr(x, pre, f);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                vexpr(k, pre, f);
                vexpr(v, pre, f);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            vexpr(scrutinee, pre, f);
            for MatchArm { guard, body, .. } in arms {
                if let Some(g) = guard {
                    vexpr(g, pre, f);
                }
                vexpr(body, pre, f);
            }
        }
        Expr::Range { start, end, .. } => {
            vexpr(start, pre, f);
            vexpr(end, pre, f);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            vexpr(cond, pre, f);
            vexpr(then_expr, pre, f);
            vexpr(else_expr, pre, f);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => vexpr(x, pre, f),
            LambdaBody::Block(b) => vblock(b, pre, f),
        },
        Expr::CloneWith { object, fields, .. } => {
            vexpr(object, pre, f);
            for (_, v) in fields {
                vexpr(v, pre, f);
            }
        }
        Expr::New(inner, _) => vexpr(inner, pre, f),
        Expr::Spawn { call, .. } => vexpr(call, pre, f),
        Expr::OverloadSelect { call, .. } => vexpr(call, pre, f),
        Expr::ParentCall { args, .. } => {
            for a in args {
                vexpr(a, pre, f);
            }
        }
        // Literals / `Ident` / `This` / `Inject` / `PipePlaceholder` have no sub-expressions.
        // (A placeholder is substituted by its owning pipe's lowering.)
        _ => {}
    }
    if !pre {
        f(e);
    }
}

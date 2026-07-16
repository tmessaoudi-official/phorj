//! DEC-239 — expand the pipe operator `lhs |> rhs` out of the AST **before any other pass runs**.
//!
//! The parser keeps `|>` as a real [`Expr::Pipe`] node so the formatter round-trips the surface
//! syntax (`x |> f` must not reformat to `f(x)`); this pass is the single lowering point that
//! erases it for the checker and every backend — the same "front-end sugar, expanded out"
//! discipline as `new` / `html"…"` / type aliases. It runs FIRST in `cli::check_and_expand_reified`
//! (and its `front_end_diagnostics` mirror), so no other pass ever sees `Pipe`/`PipePlaceholder`.
//!
//! Lowering rules (DEC-239, PHP-8.5-aligned callable application):
//! - **Plain** `x |> f` → `f(x)` — exactly the shipped semantics.
//! - **Placeholder, one `%`** `x |> f(%, 2)` → `f(x, 2)` — whole-argument substitution (the ruled
//!   equivalence; a single slot cannot duplicate evaluation).
//! - **Placeholder, several `%`** `x |> f(%, %)` → `((__pipeN) => f(__pipeN, __pipeN))(x)` — a
//!   single-evaluation IIFE; the fresh param carries [`Type::Infer`], resolved by the checker from
//!   the piped value's type (the same contextual mechanism as the pipe lambda). The fresh name is
//!   collision-scanned against every identifier in the RHS call, so user bindings can't be shadowed.
//! - A **contextually-typed pipe lambda** `x |> (v => v * 2)` is already an ordinary
//!   [`Expr::Lambda`] RHS (with a [`Type::Infer`] param) by the time it reaches here — it lowers by
//!   the plain rule into an IIFE the checker's contextual call path types.
//!
//! Placeholder SHAPE validation (`E-PIPE-PLACEHOLDER`) happens at parse time, so this pass only
//! ever sees placeholders as whole direct arguments of a pipe's top-level RHS call. Bottom-up
//! rewriting guarantees a nested pipe's placeholders are substituted before the outer pipe lowers.

use super::*;
use crate::ast::{
    CatchClause, ClassMember, Expr, Item, LambdaBody, MatchArm, Param, Stmt, StrPart,
};
use crate::token::Span;

/// Expand every `Expr::Pipe` / `Expr::PipePlaceholder` throughout the program (in place).
pub fn lower_pipes(mut program: Program) -> Program {
    for item in &mut program.items {
        match item {
            Item::Function(f) => lp_block(&mut f.body),
            Item::Class(c) => lp_members(&mut c.members),
            Item::Trait(t) => lp_members(&mut t.members),
            // Enums/interfaces/imports/aliases carry no expressions to rewrite.
            _ => {}
        }
    }
    program
}

fn lp_members(members: &mut [ClassMember]) {
    for m in members {
        match m {
            ClassMember::Method(f) => lp_block(&mut f.body),
            ClassMember::Constructor { body, .. } => lp_block(body),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init {
                    lp_expr(e);
                }
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    lp_expr(g);
                }
                if let Some((_, body)) = set {
                    lp_block(body);
                }
            }
        }
    }
}

fn lp_block(stmts: &mut [Stmt]) {
    for s in stmts {
        lp_stmt(s);
    }
}

fn lp_stmt(s: &mut Stmt) {
    match s {
        Stmt::VarDecl { init, .. } => lp_expr(init),
        Stmt::Assign { target, value, .. } => {
            lp_expr(target);
            lp_expr(value);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                lp_expr(e);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            lp_expr(cond);
            lp_block(then_block);
            if let Some(b) = else_block {
                lp_block(b);
            }
        }
        Stmt::For { iter, body, .. } => {
            lp_expr(iter);
            lp_block(body);
        }
        Stmt::While { cond, body, .. } => {
            lp_expr(cond);
            lp_block(body);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                lp_stmt(i);
            }
            if let Some(c) = cond {
                lp_expr(c);
            }
            if let Some(st) = step {
                lp_stmt(st);
            }
            lp_block(body);
        }
        Stmt::Block(b, _) => lp_block(b),
        Stmt::Destructure {
            init, else_block, ..
        } => {
            lp_expr(init);
            if let Some(eb) = else_block {
                lp_block(eb);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => lp_expr(e),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            lp_block(body);
            for CatchClause { body, .. } in catches {
                lp_block(body);
            }
            if let Some(fb) = finally_block {
                lp_block(fb);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn lp_expr(e: &mut Expr) {
    // Children first (bottom-up), so a nested pipe — anywhere in `lhs`, or inside an RHS argument —
    // is fully lowered (its own placeholders substituted) before this node's lowering inspects it.
    match e {
        Expr::Unary { expr, .. } => lp_expr(expr),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => lp_expr(inner),
        Expr::Binary { lhs, rhs, .. } => {
            lp_expr(lhs);
            lp_expr(rhs);
        }
        Expr::Pipe { lhs, rhs, .. } => {
            lp_expr(lhs);
            lp_expr(rhs);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => lp_expr(value),
        Expr::Call { callee, args, .. } => {
            lp_expr(callee);
            for a in args {
                lp_expr(a);
            }
        }
        Expr::Member { object, .. } => lp_expr(object),
        Expr::Index { object, index, .. } => {
            lp_expr(object);
            lp_expr(index);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    lp_expr(x);
                }
            }
        }
        Expr::TaggedTemplate { parts, .. } => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    lp_expr(x);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                lp_expr(x);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                lp_expr(k);
                lp_expr(v);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            lp_expr(scrutinee);
            for MatchArm { guard, body, .. } in arms {
                if let Some(g) = guard {
                    lp_expr(g);
                }
                lp_expr(body);
            }
        }
        Expr::Range { start, end, .. } => {
            lp_expr(start);
            lp_expr(end);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            lp_expr(cond);
            lp_expr(then_expr);
            lp_expr(else_expr);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => lp_expr(x),
            LambdaBody::Block(b) => lp_block(b),
        },
        Expr::CloneWith { object, fields, .. } => {
            lp_expr(object);
            for (_, v) in fields {
                lp_expr(v);
            }
        }
        Expr::New(inner, _) => lp_expr(inner),
        Expr::Spawn { call, .. } => lp_expr(call),
        Expr::OverloadSelect { call, .. } => lp_expr(call),
        Expr::ParentCall { args, .. } => {
            for a in args {
                lp_expr(a);
            }
        }
        // Literals / `Ident` / `This` / `Inject` / `PipePlaceholder` have no sub-expressions.
        // (A placeholder is substituted by its owning pipe's lowering, below.)
        _ => {}
    }
    if let Expr::Pipe { lhs, rhs, span } = e {
        let sp: Span = *span;
        let lhs = std::mem::replace(lhs.as_mut(), Expr::Null(sp));
        let rhs = std::mem::replace(rhs.as_mut(), Expr::Null(sp));
        *e = lower_one_pipe(lhs, rhs, sp);
    }
}

/// Lower one pipe whose operands are already pipe-free. `sp` is the `|>` token's span — kept as the
/// lowered call's span (matching the pre-DEC-239 parser lowering, so diagnostics don't move).
fn lower_one_pipe(lhs: Expr, rhs: Expr, sp: Span) -> Expr {
    if let Expr::Call {
        callee,
        mut args,
        type_args,
        span: csp,
    } = rhs
    {
        let slots: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| matches!(a, Expr::PipePlaceholder(_)))
            .map(|(i, _)| i)
            .collect();
        match slots.len() {
            0 => {
                // No placeholders: reassemble and fall through to plain application below.
                let rhs = Expr::Call {
                    callee,
                    args,
                    type_args,
                    span: csp,
                };
                return apply(lhs, rhs, sp);
            }
            1 => {
                // One `%`: whole-argument substitution — the ruled equivalence `x |> f(%, 2)` ≡
                // `f(x, 2)`. The RHS call IS the pipe's value.
                args[slots[0]] = lhs;
                return Expr::Call {
                    callee,
                    args,
                    type_args,
                    span: csp,
                };
            }
            _ => {
                // Several `%`: the piped value must evaluate ONCE — wrap in an IIFE whose fresh
                // param carries `Type::Infer` (the checker resolves it from the piped value's
                // type, the same contextual path as the pipe lambda).
                let fresh = {
                    let probe = Expr::Call {
                        callee: callee.clone(),
                        args: args.clone(),
                        type_args: type_args.clone(),
                        span: csp,
                    };
                    fresh_pipe_name(&probe)
                };
                for i in slots {
                    args[i] = Expr::Ident(fresh.clone(), sp);
                }
                let inner = Expr::Call {
                    callee,
                    args,
                    type_args,
                    span: csp,
                };
                let lambda = Expr::Lambda {
                    params: vec![Param {
                        ty: crate::ast::Type::Infer(sp),
                        name: fresh,
                        default: None,
                        span: sp,
                    }],
                    ret: None,
                    throws: Vec::new(),
                    body: LambdaBody::Expr(Box::new(inner)),
                    span: sp,
                };
                return Expr::Call {
                    callee: Box::new(lambda),
                    args: vec![lhs],
                    type_args: Vec::new(),
                    span: sp,
                };
            }
        }
    }
    apply(lhs, rhs, sp)
}

/// Plain callable application `rhs(lhs)` — the shipped, PHP-8.5-identical base semantics.
fn apply(lhs: Expr, rhs: Expr, sp: Span) -> Expr {
    Expr::Call {
        callee: Box::new(rhs),
        args: vec![lhs],
        type_args: Vec::new(),
        span: sp,
    }
}

/// A fresh IIFE parameter name that no FREE variable of the RHS call references — the exact
/// shadowing set: substituting the fresh param into whole-argument slots can only capture a name
/// that reaches the call from the enclosing scope (a nested lambda's own params rebind inside it
/// and are correctly excluded by [`crate::ast::free_vars`]).
fn fresh_pipe_name(rhs_call: &Expr) -> String {
    let used: std::collections::HashSet<String> =
        crate::ast::free_vars(&[], &LambdaBody::Expr(Box::new(rhs_call.clone())))
            .into_iter()
            .collect();
    let mut n = 0usize;
    loop {
        let cand = format!("__pipe{n}");
        if !used.contains(&cand) {
            return cand;
        }
        n += 1;
    }
}

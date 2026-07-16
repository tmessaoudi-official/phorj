//! DEC-239 — expand the pipe operator `lhs |> rhs` out of the AST **before any other pass runs**.
//!
//! The parser keeps `|>` as a real [`Expr::Pipe`] node so the formatter round-trips the surface
//! syntax (`x |> f` must not reformat to `f(x)`); [`lower_pipes`] is the single lowering point
//! that erases it for the checker and every backend — the same "front-end sugar, expanded out"
//! discipline as `new` / `html"…"` / type aliases. It runs FIRST in
//! `cli::check_and_expand_reified` (and its `front_end_diagnostics` mirror), so no other pass ever
//! sees `Pipe`/`PipePlaceholder`.
//!
//! Lowering rules (DEC-239, PHP-8.5-aligned callable application):
//! - **Plain** `x |> f` → `f(x)` — exactly the shipped semantics.
//! - **Placeholder, one `%`** `x |> f(%, 2)` → `f(x, 2)` — whole-argument substitution (the ruled
//!   equivalence; a single slot cannot duplicate evaluation).
//! - **Placeholder, several `%`** `x |> f(%, %)` → `((__pipeN) => f(__pipeN, __pipeN))(x)` — a
//!   single-evaluation IIFE; the fresh param carries [`Type::Infer`], resolved by the checker from
//!   the piped value's type (the same contextual mechanism as the pipe lambda). The fresh name is
//!   collision-scanned against the RHS call's free variables, so user bindings can't be shadowed.
//! - A **contextually-typed pipe lambda** `x |> (v => v * 2)` is already an ordinary
//!   [`Expr::Lambda`] RHS (with a [`Type::Infer`] param) by the time it reaches here — it lowers by
//!   the plain rule into an IIFE the checker's contextual call path types. After checking,
//!   [`materialize_pipe_params`] (LAST in the rewrite chain) writes the checker-inferred type into
//!   the param, so the VM compiler and transpiler see a concretely-typed lambda (Invariant 7).
//!
//! Placeholder SHAPE validation (`E-PIPE-PLACEHOLDER`) happens at parse time, so this pass only
//! ever sees placeholders as whole direct arguments of a pipe's top-level RHS call. Bottom-up
//! rewriting (see [`walk`]) guarantees a nested pipe's placeholders are substituted before the
//! outer pipe lowers.

use super::*;
use crate::ast::{Expr, LambdaBody, Param};
use crate::token::Span;

pub(in crate::checker) mod materialize;
pub(in crate::checker) mod walk;

pub use materialize::materialize_pipe_params;

/// Expand every `Expr::Pipe` / `Expr::PipePlaceholder` throughout the program (in place).
pub fn lower_pipes(mut program: Program) -> Program {
    walk::visit_exprs_mut(&mut program, &mut |e| {
        if let Expr::Pipe { lhs, rhs, span } = e {
            let sp: Span = *span;
            let lhs = std::mem::replace(lhs.as_mut(), Expr::Null(sp));
            let rhs = std::mem::replace(rhs.as_mut(), Expr::Null(sp));
            *e = lower_one_pipe(lhs, rhs, sp);
        }
    });
    program
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
/// and are correctly excluded by [`crate::ast::free_vars`]). camelCase because lambda params are
/// `E-NAME-CASE`-checked (the `phorjInject<T>` DI-factory precedent) — and unlike DI's disclosed
/// collision risk, the free-var scan makes a user `phorjPipe0` collision impossible (it bumps).
fn fresh_pipe_name(rhs_call: &Expr) -> String {
    let used: std::collections::HashSet<String> =
        crate::ast::free_vars(&[], &LambdaBody::Expr(Box::new(rhs_call.clone())))
            .into_iter()
            .collect();
    let mut n = 0usize;
    loop {
        let cand = format!("phorjPipe{n}");
        if !used.contains(&cand) {
            return cand;
        }
        n += 1;
    }
}

//! DEC-239 — write the checker-inferred type of each contextual pipe-lambda parameter into the
//! AST, so every backend sees a concretely-typed lambda.
//!
//! A pipe lambda `x |> (v => v * 2)` (and the multi-`%` IIFE) parses with `Type::Infer` on its one
//! param; the checker resolves it from the piped value's type and records the resolution keyed by
//! the param's `span.start` (`Checker::pipe_param_resolutions`). This pass runs **LAST** in
//! `cli::check_and_expand_reified`'s rewrite chain — after `rewrite_ufcs` has spliced any recorded
//! replacements back into the tree — and mutates only `Param.ty`, so there is no cloned-subtree
//! staleness to manage. Leaving `Infer` in a backend-bound param would de-specialize the VM
//! compiler's `resolve_cty` (CTy `Other`) and the transpiler's kind analysis — the run≠runvm
//! CTy-operand trap (Invariant 7) — which the differential `pipe-lambda-result + 1` case guards.

use super::*;
use crate::ast::{Expr, Type};
use crate::token::Span;

/// Materialize every recorded pipe-lambda param resolution into the program (in place). A no-op
/// when no contextual pipe lambda was checked.
pub fn materialize_pipe_params(mut program: Program, pipes: &HashMap<usize, Ty>) -> Program {
    if pipes.is_empty() {
        return program;
    }
    super::walk::visit_exprs_mut(&mut program, &mut |e| {
        if let Expr::Lambda { params, .. } = e {
            for p in params {
                if matches!(p.ty, Type::Infer(_)) {
                    if let Some(t) = pipes.get(&p.span.start) {
                        p.ty = ty_to_ast_type(t, p.span);
                    }
                }
            }
        }
    });
    program
}

/// The AST [`Type`] a checker [`Ty`] materializes to, for backend consumption (`resolve_cty` /
/// `kind_of_type` key on these exact names). This runs AFTER `erase_generics`, so a generic type
/// parameter maps straight to [`Type::Erased`] (what erasure would have produced); `Null`/`Error`
/// have no annotation form and map to `Erased` too (boxed, never a specialized operand — safe).
pub(in crate::checker) fn ty_to_ast_type(t: &Ty, sp: Span) -> Type {
    let named = |name: &str, args: Vec<Type>| Type::Named {
        name: name.to_string(),
        args,
        span: sp,
    };
    match t {
        Ty::Int => named("int", Vec::new()),
        Ty::Float => named("float", Vec::new()),
        Ty::Decimal => named("decimal", Vec::new()),
        Ty::Bool => named("bool", Vec::new()),
        Ty::String => named("string", Vec::new()),
        Ty::Bytes => named("bytes", Vec::new()),
        Ty::Html => named("Html", Vec::new()),
        Ty::Attr => named("Attr", Vec::new()),
        Ty::Void => named("void", Vec::new()),
        Ty::Empty => named("empty", Vec::new()),
        Ty::Never => named("never", Vec::new()),
        Ty::Named(n, args) => named(n, args.iter().map(|a| ty_to_ast_type(a, sp)).collect()),
        Ty::List(el) | Ty::FixedList(el, _) => named("List", vec![ty_to_ast_type(el, sp)]),
        Ty::Map(k, v) => named("Map", vec![ty_to_ast_type(k, sp), ty_to_ast_type(v, sp)]),
        Ty::Set(el) => named("Set", vec![ty_to_ast_type(el, sp)]),
        Ty::Optional(inner) => Type::Optional {
            inner: Box::new(ty_to_ast_type(inner, sp)),
            span: sp,
        },
        Ty::Union(ms) => Type::Union(ms.iter().map(|m| ty_to_ast_type(m, sp)).collect(), sp),
        Ty::Intersection(ms) => {
            Type::Intersection(ms.iter().map(|m| ty_to_ast_type(m, sp)).collect(), sp)
        }
        Ty::Function(ps, ret, throws) => Type::Function {
            params: ps.iter().map(|p| ty_to_ast_type(p, sp)).collect(),
            ret: Box::new(ty_to_ast_type(ret, sp)),
            throws: throws.iter().map(|e| ty_to_ast_type(e, sp)).collect(),
            span: sp,
        },
        // A tuple erases to a List before any backend (DEC-288b); it never reaches a specialized pipe
        // operand, so materializing it as `Erased` is safe (same as a generic param).
        Ty::Tuple(_) | Ty::Param(_) | Ty::Null | Ty::Error => Type::Erased(sp),
    }
}

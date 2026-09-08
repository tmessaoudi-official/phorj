//! DEC-239 / DEC-504 — write a checker-inferred type back into the AST wherever the source wrote
//! none, so every backend sees a concretely-typed node instead of `Type::Infer`.
//!
//! TWO node kinds are materialized, both keyed by `span.start` in one table
//! (`Checker::inferred_type_resolutions`) because they are the same fact — *"the user wrote no
//! annotation here and the checker worked one out"*:
//!
//! - **Contextual pipe-lambda params** (DEC-239). `x |> (v => v * 2)` (and the multi-`%` IIFE)
//!   parses with `Type::Infer` on its one param; the checker resolves it from the piped value.
//! - **`var` declarations whose inferred type is a TUPLE** (DEC-504). Scoped to tuples on purpose:
//!   every other `var` is already served by the consumers' initializer fallback, and annotating all
//!   of them would be a large, untested change in what the backends see.
//!
//! Why the tuple case is not optional. Both consumers prefer the annotation and fall back to the
//! INITIALIZER when there is none — `compiler::stmt` (`Type::Infer(_) => self.ctype(init)`) and
//! `transpile::stmt` (`OpKind::Other => self.expr_kind(init)`). By the time either runs, a tuple
//! literal has been erased to a plain list, whose element type is position 0's. So `var t = (source:
//! "z", bp: 7); t.bp + 1` resolves `bp` as a STRING: the VM refuses to infer a numeric type where
//! the interpreter reads an int, and the PHP leg emits `.` instead of `+` and prints `71` for `8`.
//! That is the Invariant-7 CTy-operand trap, and an annotation is what closes it — the same fix, at
//! the same place, as the `pipe-lambda-result + 1` case this pass already guarded.
//!
//! This pass runs **LAST** in `cli::check_and_expand_reified`'s rewrite chain — after `rewrite_ufcs`
//! has spliced any recorded replacements back into the tree — and mutates only a `Param.ty` /
//! `VarDecl.ty`, never a subtree, so there is no cloned-subtree staleness to manage. It runs BEFORE
//! `rewrite_tuple_fields` + `erase_tuples`, which is what lets it write a `Type::Tuple` at all: the
//! value form is erased after it, the type form deliberately survives.

use super::*;
use crate::ast::{Expr, Item, LambdaBody, Stmt, Type};
use crate::token::Span;

/// Materialize every recorded inferred type into the program (in place). A no-op when the checker
/// recorded none.
pub fn materialize_inferred_types(mut program: Program, inferred: &HashMap<usize, Ty>) -> Program {
    if inferred.is_empty() {
        return program;
    }
    // `var` declarations (DEC-504). Statement positions the expression walk below cannot reach, so
    // this uses the same total statement walk as `materialize_tuple_binds`.
    let write_decl = &mut |s: &mut Stmt| {
        if let Stmt::VarDecl { ty, span, .. } = s {
            if matches!(ty, Type::Infer(_)) {
                if let Some(t) = inferred.get(&span.start) {
                    *ty = ty_to_ast_type(t, *span);
                }
            }
        }
    };
    use crate::checker::rewrite_foreach::{walk_member_stmts, walk_stmts};
    for item in &mut program.items {
        match item {
            Item::Function(f) => walk_stmts(&mut f.body, write_decl),
            Item::Class(c) => walk_member_stmts(&mut c.members, write_decl),
            Item::Trait(t) => walk_member_stmts(&mut t.members, write_decl),
            Item::Test { body, .. } => walk_stmts(body, write_decl),
            _ => {}
        }
    }
    super::walk::visit_exprs_mut(&mut program, &mut |e| {
        if let Expr::Lambda { params, body, .. } = e {
            // Contextual pipe-lambda params (DEC-239).
            for p in params {
                if matches!(p.ty, Type::Infer(_)) {
                    if let Some(t) = inferred.get(&p.span.start) {
                        p.ty = ty_to_ast_type(t, p.span);
                    }
                }
            }
            // A `var` inside a block-bodied lambda is not reached by the item walk above.
            if let LambdaBody::Block(stmts) = body {
                walk_stmts(stmts, write_decl);
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
        // DEC-504 / Invariant 7: a tuple materializes to its REAL type, labels and all. The value
        // form is erased to a list before any backend, but the type form survives, and it is the
        // only thing that tells `resolve_cty` / `kind_of_type` that position 1 is an `int` while
        // position 0 is a `string`. `Erased` here (what this arm did until named fields made `t.bp`
        // — and so `t[1]` — writable) collapses to `CTy::Other`/`OpKind::Other`, and the consumers
        // then fall back to the ERASED list literal, whose element type is position 0's. That is
        // the CTy-operand trap: the VM refuses to infer a numeric type where the interpreter reads
        // an int, and the PHP leg emits `.` for a `+`. Labels are carried because a materialized
        // binder must still answer `.bp` — dropping them un-names the tuple and `t.bp` would be
        // refused with `E-TUPLE-POSITIONAL-FIELD` on a tuple the user did name.
        Ty::Tuple(ts, ls) => Type::Tuple(
            ts.iter().map(|t| ty_to_ast_type(t, sp)).collect(),
            crate::ast::to_ast_labels(ls.as_deref(), sp),
            sp,
        ),
        Ty::Param(_) | Ty::Null | Ty::Error => Type::Erased(sp),
    }
}

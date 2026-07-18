//! DEC-288 — tuple erasure: rewrite every `Expr::Tuple(a, b, …)` into the plain `Expr::List([a, b, …])`
//! it desugars to, before any backend runs. A tuple is a compile-time-only sugar checked as a
//! [`crate::types::Ty::Tuple`] (arity + per-position element types), then erased here to its runtime
//! representation — an ordinary list (`Value::List`) — the same "expanded out before backends"
//! discipline as generics / `FixedList` / `html"…"` (Invariant 5).
//!
//! Only the VALUE form (`Expr::Tuple`) is rewritten: its backend match arms are `unreachable!`, so it
//! MUST be gone before compile/interpret/transpile. The TYPE form (`Type::Tuple` annotations) is left
//! in place and handled backend-side as a list view — the compiler's `resolve_cty` maps it to
//! `CTy::List` and the transpiler's `emit_type` to a PHP type that does not affect stdout — so
//! run ≡ runvm ≡ PHP output is byte-identical either way.
//!
//! Runs in `cli::check_and_expand_reified` (the chokepoint BOTH the interpreter and the VM/transpile
//! paths flow through), so no backend ever sees `Expr::Tuple` (Invariant 6).

use crate::ast::{Expr, Program};

/// Rewrite every `Expr::Tuple` in the program to the equivalent `Expr::List` (DEC-288). A no-op for a
/// program with no tuple literals, so tuple-free code stays byte-identical.
pub fn erase_tuples(mut program: Program) -> Program {
    super::rewrite_pipe::walk::visit_exprs_mut(&mut program, &mut |e| {
        if let Expr::Tuple(elems, sp) = e {
            let sp = *sp;
            let elems = std::mem::take(elems);
            *e = Expr::List(elems, sp);
        }
    });
    program
}

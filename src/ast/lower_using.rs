//! THE single lowering of [`Stmt::Using`] (DEC-364) — shared verbatim by all three backends.
//!
//! **Why one function instead of three hand-lowerings.** `using` promises release on *every* exit
//! path, and Invariant 1 demands the interpreter, the VM and the transpiled PHP fail and succeed
//! identically. Three separate lowerings would be three chances to disagree about a single edge
//! (a `return` out of the block, a `break` crossing it, a throw from the initializer) — and a
//! disagreement there is exactly the class of bug the differential harness catches late and
//! expensively. With ONE lowering there is nothing to reconcile: every leg runs the same tree, so
//! byte-identity holds by construction rather than by testing.
//!
//! The shape is the guard a programmer would write by hand:
//!
//! ```text
//! using (T h = init) { body }
//! ==>
//! {                              // a block, so `h` cannot outlive its own release
//!     T h = init;                // immutable — `h` can never be reassigned out from under close()
//!     try { body } finally { h.close(); }
//! }
//! ```
//!
//! **Why the `VarDecl` sits OUTSIDE the `try`.** A fault raised while evaluating `init` means no
//! handle was ever acquired, so there is nothing to release; putting the declaration inside the
//! `try` would call `close()` on an unbound name. This ordering is also what PHP does with the
//! equivalent hand-written guard, which is what keeps the PHP leg a literal `try`/`finally` with no
//! `__phorj_*` helper (Invariant 16's trade is not needed here).
//!
//! **No new `Op` and no new `Value`:** the produced tree uses only [`Stmt::Block`],
//! [`Stmt::VarDecl`], [`Stmt::Try`] and a method call, all of which the three backends already
//! agree on and the differential harness already covers. `finally` already runs on the normal edge,
//! the caught edge, the re-propagated edge, and a `return`/`break`/`continue` escaping the block
//! (see [`Stmt::Try`]), so "every exit path" is inherited rather than re-implemented.

use super::{Expr, Stmt, Type};
use crate::token::Span;

/// The method every `Core.Closable` implementor provides — single-sourced so the checker's
/// conformance lookup and this lowering's emitted call can never drift apart.
pub const CLOSE_METHOD: &str = "close";

/// The interface a `using` type must implement. Bound by the `Core.ClosableModule` prelude; named
/// here so the checker, the lowering and the diagnostics all read one constant.
pub const CLOSABLE_INTERFACE: &str = "Closable";

/// Lower one `using` statement to its `try`/`finally` equivalent. Total — every field of
/// [`Stmt::Using`] is consumed, so a future field addition breaks this function too.
#[must_use]
pub fn lower_using(ty: &Type, name: &str, init: &Expr, body: &[Stmt], span: Span) -> Stmt {
    let decl = Stmt::VarDecl {
        ty: ty.clone(),
        name: name.to_string(),
        init: init.clone(),
        // Immutable: a `using` binding that could be reassigned would let `close()` run on a
        // different object than the one that was acquired.
        mutable: false,
        span,
    };
    let close_call = Stmt::Expr(
        Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(Expr::Ident(name.to_string(), span)),
                name: CLOSE_METHOD.to_string(),
                // Not `?.` — the checker proved the binding is a non-optional `Closable`, so the
                // release call is unconditional. A safe-call here would silently skip the release.
                safe: false,
                sep: super::MemberSep::Dot,
                span,
            }),
            args: Vec::new(),
            type_args: Vec::new(),
            span,
        },
        span,
    );
    let guarded = Stmt::Try {
        body: body.to_vec(),
        catches: Vec::new(),
        finally_block: Some(vec![close_call]),
        span,
    };
    Stmt::Block(vec![decl, guarded], span)
}

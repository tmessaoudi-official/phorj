//! Expected-type threading into collection LITERALS (UA-1.6 / DEC-178). Split out of `stmt/core.rs`
//! when DEC-364 pushed that file past Invariant 13's cap — a self-contained concern: it is the one
//! place a declared/return type flows *down* into a list/map literal instead of being inferred up
//! from its first element.

use super::*;

impl Checker {
    /// Thread an EXPECTED `List<T>`/`Map<K,V>` type into a list/map literal (UA-1.6 / DEC-178): check
    /// each member against `T` / `K,V` — allowing a **union** or subtype-upcast member — instead of the
    /// bottom-up first-element/first-pair inference in `check_list`/`check_map` (which rejects
    /// heterogeneous members as "must share one type"). Also supplies the element type for an empty
    /// `[]`. Returns the expected collection type on a literal/type match, `None` otherwise (the caller
    /// falls back to `check_expr`). Shared by the declaration initializer and the `return` value; the
    /// generic-call-argument position (which needs bidirectional inference) is deferred to Wave C.
    pub(in crate::checker) fn thread_literal_expected(
        &mut self,
        e: &crate::ast::Expr,
        expected: &Ty,
    ) -> Option<Ty> {
        // DEC-214 part-2: a bare empty `[]` is rejected before any expected-type threading — an empty
        // collection needs `new List<T>()` / `new Map<K,V>()`, never contextual inference from the
        // declared/return type. (Return `Some(Error)` so the caller reports exactly once.)
        if let crate::ast::Expr::List(elems, span) = e {
            if elems.is_empty() {
                return Some(self.err_empty_literal(*span));
            }
        }
        match (e, expected) {
            (crate::ast::Expr::List(elems, _), Ty::List(elem_ty)) => {
                for el in elems {
                    let et = self.check_expr(el);
                    if !self.ty_assignable(&et, elem_ty) {
                        self.err_assign(Self::expr_span(el), &et, elem_ty);
                    }
                }
                Some(Ty::List(elem_ty.clone()))
            }
            (crate::ast::Expr::Map(pairs, _), Ty::Map(key_ty, val_ty)) => {
                // Keys must be the hashable subset (`int`/`bool`/`string`) — mirror `check_map`'s
                // `E-MAP-KEY` guard, which this expected-type path bypasses.
                if !matches!(&**key_ty, Ty::Int | Ty::Bool | Ty::String | Ty::Error) {
                    self.err_coded(
                        Self::expr_span(e),
                        format!(
                            "map key type must be `int`, `bool`, or `string`, found `{key_ty}`"
                        ),
                        "E-MAP-KEY",
                        None,
                    );
                }
                for (k, v) in pairs {
                    let kt = self.check_expr(k);
                    if !self.ty_assignable(&kt, key_ty) {
                        self.err_assign(Self::expr_span(k), &kt, key_ty);
                    }
                    let vt = self.check_expr(v);
                    if !self.ty_assignable(&vt, val_ty) {
                        self.err_assign(Self::expr_span(v), &vt, val_ty);
                    }
                }
                Some(Ty::Map(key_ty.clone(), val_ty.clone()))
            }
            _ => None,
        }
    }
}

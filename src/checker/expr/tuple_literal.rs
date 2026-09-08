//! DEC-504 — checking a TUPLE literal, positional (`(a, b)`) or named (`(bp: 3, source: "x")`).
//!
//! Split out of `literals.rs` (Invariant 13): a tuple is the one literal whose elements do NOT
//! share a type, so it has nothing in common with the list/map inference next door beyond being
//! written in brackets.

use super::*;

impl Checker {
    /// `(a, b[, …])` (DEC-288) and its named-field form `(bp: 3, source: "x")` (DEC-504): a
    /// fixed-arity heterogeneous tuple. Unlike a list, elements do NOT share one type — each
    /// position keeps its own. Result: `Ty::Tuple([T0, T1, …], labels)`, erased to a `List` before
    /// any backend, so the labels never reach the interpreter, the VM, the JIT or the transpiler.
    ///
    /// Elements are checked in SOURCE ORDER whether or not they are labelled — the labels name the
    /// positions, they do not reorder the evaluation, and every leg must agree on the order the
    /// field expressions ran in.
    pub(in crate::checker) fn check_tuple(
        &mut self,
        elems: &[crate::ast::Expr],
        labels: &crate::ast::TupleLabels,
        _span: Span,
    ) -> Ty {
        let tys = elems.iter().map(|e| self.check_expr(e)).collect();
        self.reject_duplicate_labels(labels);
        Ty::Tuple(tys, crate::ast::to_ty_labels(labels))
    }

    /// `(a: 1, a: 2)` — a repeated field name. Refused by NAME rather than silently letting the
    /// first or last win: with order-significant fields (DEC-504) a duplicate makes `t.a`
    /// ambiguous, and picking a winner would be a silent semantic choice.
    pub(in crate::checker) fn reject_duplicate_labels(&mut self, labels: &crate::ast::TupleLabels) {
        let Some(ls) = labels else { return };
        for (i, l) in ls.iter().enumerate() {
            if ls[..i].iter().any(|p| p.name == l.name) {
                self.err_coded(
                    l.span,
                    format!("duplicate tuple field `{}`", l.name),
                    "E-TUPLE-DUP-FIELD",
                    None,
                );
            }
        }
    }
}

//! `using (T h = init) { … }` — the scope guard's checking (DEC-364).
//!
//! Split out of `flow.rs` rather than appended to it: `flow.rs` is already past Invariant 13's hard
//! cap, so the split-as-you-go rule applies to this feature at the point of arrival, not later.
//!
//! **What this file is responsible for, and why each part is not optional.** The whole promise of
//! `using` is that the lowering in [`crate::ast::lower_using`] emits a `close()` call that is
//! *guaranteed to be callable*. Nothing at runtime probes for the method — so if the checker admits
//! a type that does not have it, the backends emit a call to a method that is not there, and the
//! promise becomes a fault on the release path. Hence three enforcement obligations:
//!
//! 1. **The type is mandatory** (no `var`): an inferred binding could be typed by the initializer
//!    to anything, so there would be nothing to check the conformance *against*.
//! 2. **The type implements `Core.ClosableModule`'s `Closable`**: this is what makes the emitted
//!    call total.
//! 3. **`close()`'s declared throws are discharged**: interface conformance compares parameters and
//!    the return type only ([`crate::checker::Checker::one_sig_conforms`]) — NOT `throws` — so an
//!    implementor may legally declare `function close(): void throws IoError`. That call is
//!    synthesized into a `finally`, so without this check a *checked* fault would escape a function
//!    that neither catches nor declares it, which no other call site in the language allows. The
//!    rule applied is the one DEC-257 already ruled for a throwing iterator's `foreach`: legal when
//!    every fault is caught by an enclosing `try` or declared by the enclosing function.

use super::*;
use crate::ast::{CLOSABLE_INTERFACE, CLOSE_METHOD};

impl Checker {
    /// Check `using (T name = init) { body }`.
    pub(in crate::checker) fn check_using(
        &mut self,
        ty: &crate::ast::Type,
        name: &str,
        init: &crate::ast::Expr,
        body: &[crate::ast::Stmt],
        span: Span,
    ) {
        // (1) The type is mandatory. `using (var h = …)` parses (the parser reads a type, and `var`
        // is a type position) but cannot be checked — reject it with the fix in the message.
        if matches!(ty, crate::ast::Type::Infer(_)) {
            self.err_coded(
                span,
                format!("`using` needs an explicit type — write `using (T {name} = …)`"),
                "E-USING-INFER",
                Some(format!(
                    "the type is what proves `{name}` can be released, so it cannot be inferred"
                )),
            );
        }
        let declared = self.resolve_type(ty);
        let actual = self.check_expr(init);
        if !self.ty_assignable(&actual, &declared) {
            self.err_assign(Self::expr_span(init), &actual, &declared);
        }
        // (2) Conformance. Skipped for a poisoned type so one mistake reports once.
        if !matches!(declared, Ty::Error) {
            self.require_closable(&declared, span);
            // (3) The release call's own throws.
            self.discharge_close_throws(&declared, span);
        }
        self.push_scope();
        self.declare(name, declared, span);
        self.check_block(body);
        self.pop_scope();
    }

    /// Reject a `using` type that does not implement `Closable`.
    fn require_closable(&mut self, declared: &Ty, span: Span) {
        let closable = Ty::Named(CLOSABLE_INTERFACE.to_string(), Vec::new());
        if self.ty_assignable(declared, &closable) {
            return;
        }
        // A far more likely cause than "the class forgot to implement it" is that the interface is
        // not in scope at all, in which case NOTHING would conform and the bare message would send
        // the reader looking in the wrong place.
        let hint = if self.interfaces.contains_key(CLOSABLE_INTERFACE) {
            Some(format!(
                "declare `class {declared} implements {CLOSABLE_INTERFACE}` with a `{CLOSE_METHOD}(): void` method, \
                 or release it by hand with `try`/`finally`"
            ))
        } else {
            Some(format!(
                "no `{CLOSABLE_INTERFACE}` interface is in scope here — add `import Core.ClosableModule;`"
            ))
        };
        self.err_coded(
            span,
            format!("`using` requires a `{CLOSABLE_INTERFACE}` type, found `{declared}`"),
            "E-USING-NOT-CLOSABLE",
            hint,
        );
    }

    /// Require every fault `close()` declares to be caught by an enclosing `try` or declared by the
    /// enclosing function — the DEC-257 auto-propagation rule, applied to the synthesized release
    /// call. See this module's header for why conformance does not already cover this.
    fn discharge_close_throws(&mut self, declared: &Ty, span: Span) {
        let Ty::Named(cname, cargs) = declared else {
            return;
        };
        let Some(ci) = self.classes.get(cname) else {
            return; // the interface itself, or a type with no recorded class body
        };
        let Some(sig) = ci.methods.get(CLOSE_METHOD).and_then(|s| s.first()) else {
            return; // absent `close` is `require_closable`'s report, not a second one here
        };
        let theta = self.class_subst(cname, cargs);
        let throws: Vec<Ty> = sig
            .throws
            .iter()
            .map(|t| crate::checker::common::apply_subst(t, &theta))
            .collect();
        for e in throws {
            if !self.covered_by_try(&e) && !self.throws_declared(&e) {
                self.err_coded(
                    span,
                    format!(
                        "releasing this `using` can throw `{e}` (its `{CLOSE_METHOD}` declares it), which is not handled here"
                    ),
                    "E-USING-CLOSE-THROWS",
                    Some(format!(
                        "wrap the `using` in a `try`/`catch ({e} …)`, or declare `throws {e}` on the enclosing function"
                    )),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// The lowering is the contract every backend runs, so its SHAPE is asserted directly rather
    /// than only through end-to-end output: a regression that moved the `VarDecl` inside the `try`
    /// would still print the right thing on the happy path while calling `close()` on an unbound
    /// name when the initializer faults.
    #[test]
    fn the_lowering_declares_outside_the_try_and_closes_in_the_finally() {
        use crate::ast::{Expr, Stmt, Type};
        use crate::token::Span;
        let sp = Span {
            start: 0,
            len: 0,
            line: 1,
            col: 1,
        };
        let lowered = crate::ast::lower_using(
            &Type::Named {
                name: "Conn".into(),
                args: Vec::new(),
                span: sp,
            },
            "h",
            &Expr::Int(1, sp),
            &[Stmt::Break(sp)],
            sp,
        );
        let Stmt::Block(items, _) = &lowered else {
            panic!("`using` must lower to a block so the binding cannot outlive its release");
        };
        assert_eq!(items.len(), 2, "expected `var` + `try`, got {items:?}");
        // The declaration is OUTSIDE the guarded region: a fault in the initializer means no handle
        // was acquired, so there must be nothing to release.
        let Stmt::VarDecl { name, mutable, .. } = &items[0] else {
            panic!("first item must be the binding, got {:?}", items[0]);
        };
        assert_eq!(name, "h");
        assert!(!mutable, "a `using` binding must be immutable");
        let Stmt::Try {
            body,
            catches,
            finally_block: Some(fin),
            ..
        } = &items[1]
        else {
            panic!("second item must be a try/finally, got {:?}", items[1]);
        };
        assert_eq!(
            body,
            &[Stmt::Break(sp)],
            "the body must be guarded verbatim"
        );
        assert!(
            catches.is_empty(),
            "`using` must not swallow anything — it releases and re-propagates"
        );
        // The release call is `h.close()` on the binding itself.
        let [Stmt::Expr(
            Expr::Call {
                callee: box_callee,
                args,
                ..
            },
            _,
        )] = &fin[..]
        else {
            panic!("the finally must hold exactly one call, got {fin:?}");
        };
        assert!(args.is_empty(), "`close()` takes no arguments");
        let Expr::Member {
            object,
            name: method,
            ..
        } = &**box_callee
        else {
            panic!("the release call must be a method call, got {box_callee:?}");
        };
        assert_eq!(method, crate::ast::CLOSE_METHOD);
        assert_eq!(&**object, &Expr::Ident("h".into(), sp));
    }
}

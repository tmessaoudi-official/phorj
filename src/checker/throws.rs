//! `impl Checker` — throws cluster (M-Decomp W2). See checker/mod.rs for the struct + entry points.

use super::*;

/// The result of a throws-mode `?` attempt — see [`Checker::try_throws_propagate`].
pub(in crate::checker) enum PropagateOutcome {
    /// The operand is a throwing call: propagation validated, node erased; the call's type.
    Throws(Ty),
    /// The operand is a call that throws NOTHING — already checked; its type is handed back so
    /// the caller takes the Result-mode / position-error path without a duplicate check.
    Plain(Ty),
}

impl Checker {
    /// Flatten any union member of a resolved `throws` list into its individual exception types, so
    /// `throws A | B` becomes the set `{A, B}` — discharge and validation operate per type, and a
    /// call/`?` that propagates a union must discharge every member (M-faults 2b).
    pub(super) fn flatten_throws(tys: Vec<Ty>) -> Vec<Ty> {
        let mut out = Vec::new();
        for t in tys {
            match t {
                Ty::Union(members) => out.extend(members),
                other => out.push(other),
            }
        }
        out
    }

    /// Whether `t` is a thrown/declared/caught exception type — a class or interface that is `<:` the
    /// built-in `Error` marker (the `Error` interface itself qualifies). Anything else (a primitive,
    /// an enum, a function, a poison `<error>`) is not throwable/catchable (`E-THROW-TYPE`/
    /// `E-CATCH-TYPE`). Poison (`Ty::Error`) returns `true` so a prior error doesn't cascade.
    pub(super) fn is_error_type(&self, t: &Ty) -> bool {
        match t {
            Ty::Error => true,
            Ty::Named(n, _) => self.is_subtype(n, "Error"),
            _ => false,
        }
    }

    /// Whether a thrown `e` is *declared* by the enclosing function — `<:` some member of
    /// [`Self::cur_throws`]. The propagate (`?`) path and a bare `throw e` both discharge this way.
    pub(super) fn throws_declared(&self, e: &Ty) -> bool {
        self.cur_throws.iter().any(|d| self.ty_assignable(e, d))
    }

    /// Whether a thrown `e` is *caught* by an enclosing `try` we are currently inside the body of —
    /// `<:` some catch-clause type in any active [`Self::try_catch_stack`] frame (innermost or outer).
    pub(super) fn covered_by_try(&self, e: &Ty) -> bool {
        self.try_catch_stack
            .iter()
            .any(|frame| frame.iter().any(|c| self.ty_assignable(e, c)))
    }

    /// Route one checked exception a call site must account for: under `?`-propagation
    /// (`skip = true`, the outermost-call suppression flag) it is COLLECTED into
    /// [`Checker::propagate_sink`] for `try_throws_propagate` to validate; at a bare call it is
    /// discharged (caught-or-error) as before. Every discharge site funnels here, so free
    /// functions, methods, and overload sets all propagate uniformly.
    pub(in crate::checker) fn route_call_throw(
        &mut self,
        skip: bool,
        name: &str,
        e: &Ty,
        span: Span,
    ) {
        if skip {
            if !self.propagate_sink.contains(e) {
                self.propagate_sink.push(e.clone());
            }
        } else {
            self.discharge_call_throw(name, e, span);
        }
    }

    /// Throws-mode `?` (M-faults 2b): `f()?` / `recv.m()?` where the callee declares `throws E`
    /// — free functions AND methods (the method half closed the old `free_call_throws`
    /// deferral). Checks the inner call ONCE with discharge suppressed; the sites collect the
    /// callee's throws into [`Checker::propagate_sink`]. A non-empty set is validated against
    /// the enclosing `throws` (propagation — not a `try`), the node recorded for erasure, and
    /// the call's *normal* type returned as [`PropagateOutcome::Throws`]. An empty set means
    /// the call throws nothing — [`PropagateOutcome::Plain`] hands the already-computed type
    /// back so the caller can take the Result-mode / position-error path WITHOUT re-checking.
    /// `None` = not a call operand at all (nothing was checked).
    pub(super) fn try_throws_propagate(
        &mut self,
        inner: &crate::ast::Expr,
        span: Span,
    ) -> Option<PropagateOutcome> {
        if !matches!(inner, crate::ast::Expr::Call { .. }) {
            return None;
        }
        let prev_sink = std::mem::take(&mut self.propagate_sink);
        // The wrapped call propagates instead of discharging locally — suppress its own check.
        self.skip_throws_discharge = true;
        let t = self.check_expr(inner);
        // Defensive: if the operand never reached a consuming call-check, clear the flag.
        self.skip_throws_discharge = false;
        let collected = std::mem::replace(&mut self.propagate_sink, prev_sink);
        if collected.is_empty() {
            return Some(PropagateOutcome::Plain(t));
        }
        for e in &collected {
            if !self.throws_declared(e) {
                self.err_coded(
                    span,
                    format!(
                        "`?` propagates `{e}`, but the enclosing function does not declare `throws {e}`"
                    ),
                    "E-CALL-UNHANDLED",
                    Some(format!(
                        "add `throws {e}` to the enclosing function, or wrap the call in `try`/`catch`"
                    )),
                );
            }
        }
        // Record for erasure: a throws-mode `?` is a checker-only marker (the call's own throw
        // unwinds), so the backend-facing AST keeps just the inner call (see `resolve_html`).
        self.html_resolutions.insert(span.start, inner.clone());
        Some(PropagateOutcome::Throws(t))
    }

    /// Validate a function's `throws` declaration (M-faults 2b): the entry `main` may not declare it
    /// (`E-UNCAUGHT-THROW`); every declared type must implement `Error` (`E-THROW-TYPE`); and naming
    /// the bare `Error` root is too broad (`E-THROWS-TOO-BROAD` — declare the specific subtype so
    /// callers know what to catch). `throws` is resolved into `resolved` in declaration order.
    pub(super) fn validate_throws_decl(&mut self, f: &crate::ast::FunctionDecl, resolved: &[Ty]) {
        // An entry `main` (top-level OR a class-static method, Batch-1 D) may not declare `throws`;
        // an instance method named `main` is an ordinary method and is exempt.
        let is_entry_main = f.name == "main" && (self.cur_class.is_none() || self.in_static_method);
        if is_entry_main && !f.throws.is_empty() {
            self.err_coded(
                f.span,
                "`main` is the program entry point and may not declare `throws`",
                "E-UNCAUGHT-THROW",
                Some(
                    "handle every error inside `main` with `try`/`catch` — nothing may escape the entry point"
                        .into(),
                ),
            );
        }
        for t in resolved {
            match t {
                Ty::Error => {} // poison from an earlier error — don't cascade
                Ty::Named(n, _) if n == "Error" => {
                    self.err_coded(
                        f.span,
                        "`throws Error` is too broad — declare the specific exception type(s) you throw",
                        "E-THROWS-TOO-BROAD",
                        Some("e.g. `throws BadInput` so callers know exactly what to catch".into()),
                    );
                }
                _ if !self.is_error_type(t) => {
                    self.err_coded(
                        f.span,
                        format!(
                            "`throws {t}` is not allowed — a thrown type must implement `Error`"
                        ),
                        "E-THROW-TYPE",
                        Some("declare the throwing type `class Foo implements Error { … }`".into()),
                    );
                }
                _ => {}
            }
        }
    }
}

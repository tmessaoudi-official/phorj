//! `impl Checker` — DEC-331 D9a `#[Invoke]` call resolution (split from `calls/core.rs` per
//! Invariant 13). `x(args)` on a class-typed receiver resolves against the class's `#[Invoke]`
//! overload set and records the chosen method for `resolve_invoke_tostring` to rewrite.

use super::*;

impl Checker {
    /// Resolve a call `x(args)` whose receiver `x` has type `Ty::Named(cls, cargs)`. Matches `args`
    /// against the class's `#[Invoke]` overload set (class type parameters substituted by the
    /// receiver's arguments), records the chosen method NAME keyed by the call `span` (consumed by
    /// [`crate::checker::resolve_invoke_tostring`], which rewrites `x(args)` → `x.<method>(args)`),
    /// and returns the chosen method's result type. `E-NOT-CALLABLE` when the type has no `#[Invoke]`
    /// method (or is not a class — an interface/enum receiver is out of scope this slice);
    /// `E-OVERLOAD-NO-MATCH` when no method's parameters accept the arguments. First match in
    /// declaration order wins — there is NO runtime re-dispatch (the rewrite names a concrete method),
    /// so the checker's choice is the uniform semantics across interp/VM/transpile. `#[Invoke]` methods
    /// carry no default/variadic params (`E-INVOKE-DEFAULTS`), so exact-arity matching is unambiguous.
    pub(in crate::checker) fn resolve_invoke_call(
        &mut self,
        cls: &str,
        cargs: &[Ty],
        args: &[crate::ast::Expr],
        span: Span,
        skip_throws: bool,
    ) -> Ty {
        let invoke_names = self
            .classes
            .get(cls)
            .map(|i| i.invoke_methods.clone())
            .unwrap_or_default();
        // Always type the arguments (so their sub-expressions are checked whether or not a match).
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();
        if invoke_names.is_empty() {
            // Accurate wording for a non-class `Ty::Named` (interface/enum): those cannot carry an
            // `#[Invoke]` method at all, so don't suggest adding one to "the class".
            let is_class = self.classes.contains_key(cls);
            let hint = if is_class {
                "add an `#[Invoke]` method to make instances callable, or call a named method"
            } else {
                "an interface- or enum-typed value is not callable this slice — call a named method"
            };
            return self.err_coded(
                span,
                format!("a value of type `{cls}` is not callable (no `#[Invoke]` method)"),
                "E-NOT-CALLABLE",
                Some(hint.into()),
            );
        }
        let theta = self.class_subst(cls, cargs);
        // (name, substituted params, substituted ret, substituted throws) per `#[Invoke]` overload.
        let mut candidates: Vec<(String, Vec<Ty>, Ty, Vec<Ty>)> = Vec::new();
        for name in &invoke_names {
            if let Some(sigs) = self.classes.get(cls).and_then(|i| i.methods.get(name)) {
                for s in sigs {
                    candidates.push((
                        name.clone(),
                        s.params.iter().map(|p| apply_subst(p, &theta)).collect(),
                        apply_subst(&s.ret, &theta),
                        s.throws.iter().map(|t| apply_subst(t, &theta)).collect(),
                    ));
                }
            }
        }
        let mut chosen: Option<usize> = None;
        for (i, (_, ps, _, _)) in candidates.iter().enumerate() {
            if ps.len() == arg_tys.len()
                && ps
                    .iter()
                    .zip(&arg_tys)
                    .all(|(p, a)| self.ty_assignable(a, p))
            {
                chosen = Some(i);
                break;
            }
        }
        match chosen {
            Some(i) => {
                let (name, _, ret, throws) = &candidates[i];
                let (name, ret, throws) = (name.clone(), ret.clone(), throws.clone());
                self.invoke_call_targets.insert(span.start, name);
                for e in &throws {
                    self.route_call_throw(skip_throws, "<invoke>", e, span);
                }
                ret
            }
            None => {
                let got = arg_tys
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                self.err_coded(
                    span,
                    format!("no `#[Invoke]` method of `{cls}` accepts arguments `({got})`"),
                    "E-OVERLOAD-NO-MATCH",
                    Some(
                        "a call `x(…)` must match one `#[Invoke]` method's arity and parameter types"
                            .into(),
                    ),
                )
            }
        }
    }

    /// DEC-331 D9b (spec §2 P2): `Conversion.toString(x)` on a class instance stringifies through the
    /// SAME `#[ToString]` call as interpolation — record the ARGUMENT for the `<x>.<method>()` wrap
    /// (`resolve_invoke_tostring` leaves the outer `Conversion.toString(x.m())`, whose string arg is
    /// identity → byte-identical). A class WITHOUT `#[ToString]` is `E-NO-TOSTRING`; a primitive is
    /// unchanged (returns `string`, stays a native call for the backends).
    pub(in crate::checker) fn check_conversion_to_string(
        &mut self,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        name: &str,
        span: Span,
    ) -> Ty {
        self.reject_turbofish(tf, name, span);
        let at = self.check_expr(&args[0]);
        let asp = Self::expr_span(&args[0]);
        if let Ty::Named(cls, _) = &at {
            if !self.record_to_string_target(&at, asp) {
                self.err_coded(
                    asp,
                    format!("`Conversion.toString` needs a `#[ToString]` method on `{cls}`"),
                    "E-NO-TOSTRING",
                    Some("add `#[ToString] function toString(): string { … }` to the class, or convert a primitive".into()),
                );
            }
        }
        Ty::String
    }

    /// DEC-331 D9b: if `t` is a class type with a `#[ToString]` method, record the string-context
    /// rewrite for the expression at `sp` (keyed by `sp.start`) and return `true`; else `false` (the
    /// caller emits `E-NO-TOSTRING`). Shared by interpolation (`check_str`) and the `Conversion.toString`
    /// argument path so the two contexts lower identically (spec §2 P2).
    pub(in crate::checker) fn record_to_string_target(&mut self, t: &Ty, sp: Span) -> bool {
        if let Ty::Named(cls, _) = t {
            if let Some(method) = self
                .classes
                .get(cls)
                .and_then(|i| i.to_string_method.clone())
            {
                self.to_string_targets.insert(sp.start, method);
                return true;
            }
        }
        false
    }

    /// DEC-331 D9b: a non-primitive interpolation hole `e: t` — record the `#[ToString]` rewrite when
    /// `t` is a class with one, else `E-NO-TOSTRING`. Extracted from `check_str` (Invariant 13).
    pub(in crate::checker) fn check_string_context_hole(&mut self, t: &Ty, e: &crate::ast::Expr) {
        let sp = Self::expr_span(e);
        if !self.record_to_string_target(t, sp) {
            self.err_coded(
                sp,
                format!("type `{t}` cannot be interpolated into a string — only primitives auto-stringify, or add a `#[ToString]` method"),
                "E-NO-TOSTRING",
                Some("give the class a `#[ToString] function toString(): string { … }`, or interpolate a primitive".into()),
            );
        }
    }
}

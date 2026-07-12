//! Call checking — parent calls, overload sets, method signatures, generic unification.

use super::*;

/// A resolved method-overload signature for call checking (Batch C): `(params, ret, throws)`, with
/// the class type-argument substitution already applied. The `throws` set is discharged at the call
/// site exactly like a free-function call.
pub(in crate::checker) type MethodSig = (Vec<Ty>, Ty, Vec<Ty>);

impl Checker {
    /// Check a `parent`/super dispatch call (M-RT super/parent). Validates the context (an instance
    /// method/constructor body), resolves the concrete `(declaring_class, method)` via the shared
    /// `ast::resolve_parent_method`, type-checks the arguments against the resolved method's signature,
    /// and returns its return type. The error codes mirror the resolver's failure kinds.
    pub(in crate::checker) fn check_parent_call(
        &mut self,
        ancestor: Option<&str>,
        method: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        // B1b: consume the statement-position flag set by `check_stmt` (a `parent.constructor(…)` is
        // valid only as a bare statement — see `check_parent_ctor_call`). Taken before anything else so
        // a nested `parent.constructor(…)` inside an argument sees it cleared.
        let stmt_ok = std::mem::take(&mut self.parent_ctor_ok);
        // `parent` is valid only inside an instance method/constructor body.
        let Some(lexical) = self.cur_class.clone() else {
            for a in args {
                self.check_expr(a); // type args anyway, so nested errors surface
            }
            return self.err_coded(
                span,
                "`parent` is only valid inside an instance method or constructor".to_string(),
                "E-PARENT-OUTSIDE-METHOD",
                Some("`parent.m(…)` dispatches to an inherited method; there is no parent outside a method body".into()),
            );
        };
        if self.in_static_method || self.in_static_init || self.in_field_init {
            for a in args {
                self.check_expr(a);
            }
            return self.err_coded(
                span,
                "`parent` is not available here — a static method, static initializer, or field initializer has no instance".to_string(),
                "E-PARENT-OUTSIDE-METHOD",
                Some("use `parent` only inside an instance method or constructor body".into()),
            );
        }
        // B1b: parent-constructor forwarding (`parent.constructor(…)`) is a distinct, statement-only
        // form — front-end-inlined into the existing instance, never a runtime method dispatch.
        if method == "constructor" {
            return self.check_parent_ctor_call(&lexical, ancestor, args, span, stmt_ok);
        }
        // Type the arguments, so nested errors surface and the method overload is selected by them.
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();
        let (decl, m) = match crate::ast::resolve_parent_method(
            &self.parent_parents,
            &self.parent_mro,
            &self.parent_origins,
            &lexical,
            ancestor,
            method,
        ) {
            Ok(t) => t,
            Err(e) => {
                use crate::ast::ParentResolveError as PE;
                let (msg, code, hint): (String, &str, String) = match e {
                    PE::NoParent => (
                        format!("`{lexical}` has no parent class, so `parent` has nothing to dispatch to"),
                        "E-PARENT-NO-PARENT",
                        "add an `extends` parent, or remove the `parent` call".into(),
                    ),
                    PE::NotAncestor => (
                        format!("`{}` is not an ancestor of `{lexical}`", ancestor.unwrap_or("")),
                        "E-PARENT-NOT-ANCESTOR",
                        "name a class this one transitively extends".into(),
                    ),
                    PE::NoMethod => (
                        format!("no ancestor of `{lexical}` declares a method `{method}`"),
                        "E-PARENT-NO-METHOD",
                        "check the method name, or the ancestor in `parent(A).m()`".into(),
                    ),
                    PE::Ambiguous => (
                        format!("`parent.{method}()` is ambiguous in `{lexical}` — two parents declare `{method}`"),
                        "E-PARENT-AMBIGUOUS",
                        format!("qualify the ancestor: `parent(SomeParent).{method}(…)`"),
                    ),
                };
                return self.err_coded(span, msg, code, Some(hint));
            }
        };
        // Type against the resolved method's signature (the declaring class declares `m`). An overloaded
        // parent method selects the arm matching the static argument types (like a normal overloaded
        // call); a single method is the common path.
        let sigs = match self.classes.get(&decl).and_then(|c| c.methods.get(&m)) {
            Some(s) => s.clone(),
            None => {
                return self.err(
                    span,
                    format!("internal: resolved parent method `{decl}.{m}` has no signature"),
                );
            }
        };
        let matched = sigs.iter().find(|s| {
            s.params.len() == arg_tys.len()
                && s.params
                    .iter()
                    .zip(&arg_tys)
                    .all(|(p, a)| self.ty_assignable(a, p))
        });
        match matched {
            Some(sig) => {
                for e in sig.throws.clone() {
                    self.discharge_call_throw(method, &e, span);
                }
                sig.ret.clone()
            }
            None => {
                let got = arg_tys
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                self.err_coded(
                    span,
                    format!("no overload of `{decl}.{m}` accepts arguments `({got})`"),
                    "E-OVERLOAD-NO-MATCH",
                    Some("the argument types must match the parent method's parameters".into()),
                )
            }
        }
    }

    /// Check a `parent.constructor(args)` forwarding call (B1b, single inheritance). Valid only as a
    /// bare statement inside a constructor body; forwards to the resolved parent's *effective*
    /// constructor (its params, promotions, field initializers, body), which the front-end
    /// `inline_parent_ctors` pass later splices onto the existing instance. Returns `Ty::Empty` (a
    /// constructor produces no value). Errors mirror the resolution failures + the position guards.
    pub(in crate::checker) fn check_parent_ctor_call(
        &mut self,
        lexical: &str,
        ancestor: Option<&str>,
        args: &[crate::ast::Expr],
        span: Span,
        stmt_ok: bool,
    ) -> Ty {
        // Type the args on every error path so nested errors still surface.
        macro_rules! cascade {
            () => {
                for a in args {
                    self.check_expr(a);
                }
            };
        }
        if !self.in_constructor {
            cascade!();
            return self.err_coded(
                span,
                "`parent.constructor(…)` may be called only inside a constructor body".to_string(),
                "E-PARENT-CTOR-OUTSIDE",
                Some(
                    "forward to the parent constructor from inside this class's `constructor(…)` body"
                        .into(),
                ),
            );
        }
        if !stmt_ok {
            cascade!();
            return self.err_coded(
                span,
                "`parent.constructor(…)` must be a bare statement, not a value".to_string(),
                "E-PARENT-CTOR-STMT",
                Some(
                    "write `parent.constructor(…);` as its own statement; it produces no value"
                        .into(),
                ),
            );
        }
        // Resolve the target parent class (single inheritance this slice).
        let parents = self
            .parent_parents
            .get(lexical)
            .cloned()
            .unwrap_or_default();
        let target: String = match ancestor {
            None => match parents.as_slice() {
                [] => {
                    cascade!();
                    return self.err_coded(
                        span,
                        format!("`{lexical}` has no parent class, so `parent.constructor(…)` has nothing to forward to"),
                        "E-PARENT-NO-PARENT",
                        Some("add an `extends` parent, or remove the `parent.constructor(…)` call".into()),
                    );
                }
                [p] => p.clone(),
                _ => {
                    cascade!();
                    return self.err_coded(
                        span,
                        format!("`parent.constructor(…)` under multiple inheritance is not yet supported (`{lexical}` has {} parents)", parents.len()),
                        "E-PARENT-CTOR-MI",
                        Some("multiple-inheritance constructor forwarding (one `parent(P).constructor(…)` per parent) lands in a follow-up".into()),
                    );
                }
            },
            Some(a) => {
                let is_anc = self
                    .parent_mro
                    .get(lexical)
                    .is_some_and(|mro| mro.iter().any(|c| c == a));
                if !is_anc {
                    cascade!();
                    return self.err_coded(
                        span,
                        format!("`{a}` is not an ancestor of `{lexical}`"),
                        "E-PARENT-NOT-ANCESTOR",
                        Some("name a class this one transitively extends".into()),
                    );
                }
                a.to_string()
            }
        };
        // Validate the arguments against the resolved parent's effective constructor parameter types
        // (`info.ctor` — own or inherited). A ctor-less parent has an empty param list, so a zero-arg
        // forward is accepted (and lowers to an empty block); any args then fail the arity check.
        let parent_ctor = self
            .classes
            .get(&target)
            .map(|i| i.ctor.clone())
            .unwrap_or_default();
        self.check_args(&target, &parent_ctor, args, span);
        Ty::Empty
    }

    /// Resolve a multi-overload free-function call (M-RT). Evaluates the argument types, selects the
    /// statically-matching overloads (arity + assignability), reports `E-OVERLOAD-NO-MATCH` when none
    /// match, discharges the union of the matching overloads' checked exceptions, and returns the
    /// shared return type (all overloads share it by `E-OVERLOAD-RETURN`).
    pub(in crate::checker) fn check_overload_call(
        &mut self,
        name: &str,
        sigs: &[FnSig],
        args: &[crate::ast::Expr],
        span: Span,
        skip_throws: bool,
    ) -> Ty {
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();
        let matches: Vec<&FnSig> = sigs
            .iter()
            .filter(|s| {
                s.params.len() == arg_tys.len()
                    && s.params
                        .iter()
                        .zip(&arg_tys)
                        .all(|(p, a)| self.ty_assignable(a, p))
            })
            .collect();
        if matches.is_empty() {
            let got = arg_tys
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return self.err_coded(
                span,
                format!("no overload of `{name}` accepts arguments `({got})`"),
                "E-OVERLOAD-NO-MATCH",
                Some("the argument types must match one overload's parameter types".into()),
            );
        }
        {
            let mut discharged: Vec<Ty> = Vec::new();
            for m in &matches {
                for e in &m.throws {
                    if !discharged.contains(e) {
                        discharged.push(e.clone());
                        self.route_call_throw(skip_throws, name, e, span);
                    }
                }
            }
        }
        matches[0].ret.clone()
    }

    /// Resolve a *method* call against its overload set `applied` (class type-parameters already
    /// substituted in each `(params, ret)`). One overload → the pre-overloading path, including a
    /// method-level generic (`check_generic_call`). Multiple → a static match by arity +
    /// assignability, returning the shared return type (`E-OVERLOAD-RETURN`); none → `E-OVERLOAD-NO-MATCH`.
    pub(in crate::checker) fn check_method_sigs(
        &mut self,
        name: &str,
        applied: &[MethodSig],
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        // Batch C: a method call discharges its declared checked exceptions exactly like a free-fn
        // call (finding #3 — previously dropped, letting a `throws E` escape uncaught). The `?`
        // suppression flag is honored for symmetry, though method-`?` propagation is still the
        // documented `free_call_throws` deferral (a method throw must be caught in a `try`).
        let skip_throws = std::mem::take(&mut self.skip_throws_discharge);
        if applied.len() == 1 {
            let (params, ret, throws) = &applied[0];
            for e in throws.clone() {
                self.route_call_throw(skip_throws, name, &e, span);
            }
            return if params.iter().any(ty_has_param) || ty_has_param(ret) {
                self.check_generic_call(name, params, ret, args, span)
            } else {
                self.check_args(name, params, args, span);
                ret.clone()
            };
        }
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.check_expr(a)).collect();
        // Discharge the union of every statically-matching overload's throws — runtime dispatch may
        // pick any of them (mirrors `check_overload_call`).
        {
            let mut discharged: Vec<Ty> = Vec::new();
            for (params, _, throws) in applied {
                let matches = params.len() == arg_tys.len()
                    && params
                        .iter()
                        .zip(&arg_tys)
                        .all(|(p, a)| self.ty_assignable(a, p));
                if matches {
                    for e in throws {
                        if !discharged.contains(e) {
                            discharged.push(e.clone());
                            self.route_call_throw(skip_throws, name, e, span);
                        }
                    }
                }
            }
        }
        let matched = applied.iter().find(|(params, _, _)| {
            params.len() == arg_tys.len()
                && params
                    .iter()
                    .zip(&arg_tys)
                    .all(|(p, a)| self.ty_assignable(a, p))
        });
        match matched {
            Some((_, ret, _)) => ret.clone(),
            None => {
                let got = arg_tys
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.err_coded(
                    span,
                    format!("no overload of method `{name}` accepts arguments `({got})`"),
                    "E-OVERLOAD-NO-MATCH",
                    Some("the argument types must match one overload's parameter types".into()),
                )
            }
        }
    }

    /// Discharge one checked exception `e` a called function `name` may throw at a *bare* (non-`?`)
    /// call site: it must be caught by an enclosing `try`, else `E-CALL-UNHANDLED`. Propagation is
    /// the `?` path ([`Self::try_throws_propagate`]) — a bare call may not silently propagate.
    pub(in crate::checker) fn discharge_call_throw(&mut self, name: &str, e: &Ty, span: Span) {
        if self.covered_by_try(e) {
            return;
        }
        self.err_coded(
            span,
            format!("call to `{name}` can throw `{e}`, which is not handled here"),
            "E-CALL-UNHANDLED",
            Some(format!(
                "wrap the call in `try {{ … }} catch ({e} e) {{ … }}`, or propagate it with `?` and declare `throws {e}`"
            )),
        );
    }

    /// Check a call to a *generic* function (M-RT S7). Unifies each declared parameter type (which
    /// contains `Ty::Param` occurrences) against the inferred argument type to build a substitution
    /// `θ`, then applies `θ` to the declared return type. First-binding-wins, structural; `θ` lives
    /// only here and never touches the AST (the function's type params are erased separately, before
    /// any backend). A unification failure is a normal argument-type error.
    pub(in crate::checker) fn check_generic_call(
        &mut self,
        name: &str,
        params: &[Ty],
        ret: &Ty,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        if params.len() != args.len() {
            self.err(
                span,
                format!(
                    "`{name}` expects {} argument(s), found {}",
                    params.len(),
                    args.len()
                ),
            );
            for a in args {
                self.check_expr(a);
            }
            return Ty::Error;
        }
        let mut theta: HashMap<String, Ty> = HashMap::new();
        let mut ok = true;
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let at = self.check_arg(arg, param);
            if !self.unify(param, &at, &mut theta) {
                ok = false;
                let want = apply_subst(param, &theta);
                self.err(
                    span,
                    format!("`{name}` argument {} expects `{want}`, found `{at}`", i + 1),
                );
            }
        }
        if !ok {
            return Ty::Error;
        }
        apply_subst(ret, &theta)
    }

    /// Structural unification of a declared type (possibly containing `Ty::Param`) against a concrete
    /// argument type, accumulating bindings in `θ`. Returns false on a mismatch. A parameter binds
    /// the first concrete type it meets; a later occurrence must be *consistent* (assignable either
    /// way, so subtyping is tolerated). A non-parameter position falls back to ordinary
    /// assignability. `Ty::Error` (poison) unifies with anything (M-RT S7).
    pub(in crate::checker) fn unify(
        &self,
        declared: &Ty,
        actual: &Ty,
        theta: &mut HashMap<String, Ty>,
    ) -> bool {
        if matches!(declared, Ty::Error) || matches!(actual, Ty::Error) {
            return true;
        }
        match (declared, actual) {
            (Ty::Param(p), a) => match theta.get(p) {
                None => {
                    theta.insert(p.clone(), a.clone());
                    true
                }
                Some(bound) => self.ty_assignable(a, bound) || self.ty_assignable(bound, a),
            },
            (Ty::List(d), Ty::List(a)) | (Ty::Set(d), Ty::Set(a)) => self.unify(d, a, theta),
            (Ty::Optional(d), Ty::Optional(a)) => self.unify(d, a, theta),
            // A non-null, non-optional argument against an `Optional(T)` parameter binds `T` from the
            // inner type — `Option.ofNullable(42)` infers `T = int` (an `int` IS assignable to `int?`,
            // so this just aligns `unify` with the existing `(other, Optional(t))` assignability rule).
            // A bare `null` is deliberately excluded: it cannot determine `T` (falls through to plain
            // assignability — `null` is assignable to any optional, but binds nothing).
            (Ty::Optional(d), a) if !matches!(a, Ty::Null) => self.unify(d, a, theta),
            (Ty::Map(dk, dv), Ty::Map(ak, av)) => {
                self.unify(dk, ak, theta) && self.unify(dv, av, theta)
            }
            (Ty::Function(dp, dr), Ty::Function(ap, ar)) => {
                dp.len() == ap.len()
                    && dp.iter().zip(ap).all(|(d, a)| self.unify(d, a, theta))
                    && self.unify(dr, ar, theta)
            }
            // Two generic class instances with the same head — unify their arguments so a generic
            // function over a generic class (`function unwrap<T>(Box<T> b) -> T`) binds `T` from a
            // `Box<int>` argument (M-RT generics-all). Different heads fall through to assignability.
            (Ty::Named(dn, da), Ty::Named(an, aa)) if dn == an && da.len() == aa.len() => {
                da.iter().zip(aa).all(|(d, a)| self.unify(d, a, theta))
            }
            // No type parameter at this position — ordinary assignability (actual → declared).
            (d, a) => self.ty_assignable(a, d),
        }
    }
}

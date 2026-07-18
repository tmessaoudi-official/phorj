//! Collection pass — functions: signatures, param defaults, overload validation.

use super::entry::{literal_ty, overloads_erase_alike};
use super::*;

impl Checker {
    pub(in crate::checker) fn collect_function(&mut self, f: &crate::ast::FunctionDecl) {
        if is_intrinsic_name(&f.name) {
            self.err_coded(
                f.span,
                format!(
                    "`{}` is a reserved built-in intrinsic and cannot be redefined",
                    f.name
                ),
                "E-RESERVED-INTRINSIC",
                Some("panic/todo/unreachable/assert are built in (M-faults)".into()),
            );
            return;
        }
        self.validate_type_params(&f.type_params, f.span);
        self.reject_dup_param_names(f.params.iter().map(|p| (p.name.as_str(), p.span)));
        // DEC-298: a variadic last param (`int ...nums`) — free functions only in v1. Validate it's
        // last + un-defaulted; the effective type is `List<T>` (via `effective_param_ty`).
        let variadic = self.validate_variadic_params(&f.params);
        // v1 collection only handles the NON-GENERIC single-signature call path (`core.rs`); a generic
        // (`check_generic_call`) or overloaded (`check_overload_call`) variadic would be accepted at the
        // decl but never collected at the call — so reject those decls up front, keeping the accepted
        // surface == the working surface (no confusing call-site arity error).
        if variadic && !f.type_params.is_empty() {
            self.err_coded(
                f.span,
                format!("variadic function `{}` cannot be generic (v1)", f.name),
                "E-VARIADIC-UNSUPPORTED",
                Some("v1 variadics require a non-generic, single-signature free function".into()),
            );
        }
        // Resolve the signature with the type parameters in scope so `T` becomes `Ty::Param("T")`.
        self.active_type_params = f.type_params.clone();
        let params: Vec<Ty> = f
            .params
            .iter()
            .map(|p| self.effective_param_ty(p))
            .collect();
        let ret = match &f.ret {
            Some(t) => self.resolve_type(t),
            None => Ty::Void,
        };
        // Resolve the declared throws set with the type parameters still in scope, then clear. A
        // union `throws A | B` is flattened to its members (`throws` is a set of exception types).
        let throws = Self::flatten_throws(f.throws.iter().map(|t| self.resolve_type(t)).collect());
        self.active_type_params.clear();
        // M-RT overloading: a same-named function joins an overload set rather than colliding. The
        // set must share a return type and hold no two identical signatures; a generic overload is
        // not allowed to participate (deferred). Push regardless of legality so downstream resolution
        // sees the whole set (errors already reported).
        // M4 default parameters (free functions only in v1): validate trailing-only ordering,
        // literal-only values, and type assignability, building the per-param default list.
        let defaults = self.collect_param_defaults(&f.params, &params);
        let sig = FnSig {
            params,
            defaults,
            ret,
            type_params: f.type_params.clone(),
            type_param_bounds: f.type_param_bounds.clone(),
            throws,
            is_static: false, // free functions are never static
            variadic,
        };
        let existing = self.funcs.get(&f.name).cloned().unwrap_or_default();
        // DEC-298: a variadic free function must be the SOLE signature of its name — an overloaded
        // variadic routes through `check_overload_call` (no collection). Reject when this decl joins a
        // set AND either side is variadic (a variadic joining others, OR a plain sig joining a variadic).
        if !existing.is_empty() && (variadic || existing.iter().any(|s| s.variadic)) {
            self.err_coded(
                f.span,
                format!("variadic function `{}` cannot be overloaded (v1)", f.name),
                "E-VARIADIC-UNSUPPORTED",
                Some("a variadic free function must be the only signature of its name".into()),
            );
        }
        // Free functions allow return-type overloading (M-RT Slice C1).
        self.validate_new_overload(&existing, &sig, &f.name, f.span, "function", true);
        // Record the declaration site so `finalize_overloads` can emit a per-decl mangled rename if
        // this name turns out to be a return-overload set (the span pins the exact `FunctionDecl`).
        self.free_fn_decls
            .push((f.name.clone(), f.span, sig.params.clone(), sig.ret.clone()));
        self.funcs.entry(f.name.clone()).or_default().push(sig);
    }

    /// DEC-298: variadics are free-function-only in v1. Reject a variadic parameter in a NON-free-fn
    /// position (method OR lambda) with `E-VARIADIC-UNSUPPORTED` — single-sourced so both the method
    /// path (`check_function` when `cur_class` is set) and the lambda path (`check_lambda_with`) reject
    /// with identical logic (the shared-chokepoint discipline; a per-site copy is what let a variadic
    /// lambda slip through once).
    pub(in crate::checker) fn reject_nonfree_variadic(&mut self, params: &[crate::ast::Param]) {
        for p in params.iter().filter(|p| p.variadic) {
            self.err_coded(
                p.span,
                format!(
                    "variadic parameter `{}` is only supported on free functions (v1)",
                    p.name
                ),
                "E-VARIADIC-UNSUPPORTED",
                Some("use a `List<T>` parameter here, or declare a free function".into()),
            );
        }
    }

    /// DEC-298: the EFFECTIVE type of a parameter — `List<T>` for a variadic `T ...name`, else `T`
    /// itself. Single-sourced so the free-fn signature (`collect_function`) and the body binding
    /// (`check_function`) can never disagree on what a variadic param's type is (the shared-chokepoint
    /// discipline). A non-variadic param is unchanged, so this is a drop-in for `resolve_type(&p.ty)`.
    pub(in crate::checker) fn effective_param_ty(&mut self, p: &crate::ast::Param) -> Ty {
        let t = self.resolve_type(&p.ty);
        if p.variadic {
            Ty::List(Box::new(t))
        } else {
            t
        }
    }

    /// DEC-298: validate a free function's variadic parameter and report whether its LAST param is
    /// variadic (→ `FnSig.variadic`, driving the call-site collection). A variadic param must be the
    /// last one (`E-VARIADIC-NOT-LAST`) and carry no default (`E-VARIADIC-DEFAULT` — it already
    /// defaults to an empty list). Errors only; returns the flag regardless so the sig is well-formed.
    pub(in crate::checker) fn validate_variadic_params(
        &mut self,
        params: &[crate::ast::Param],
    ) -> bool {
        let mut last_is_variadic = false;
        for (i, p) in params.iter().enumerate() {
            if !p.variadic {
                continue;
            }
            let is_last = i + 1 == params.len();
            if !is_last {
                self.err_coded(
                    p.span,
                    format!("variadic parameter `{}` must be the last parameter", p.name),
                    "E-VARIADIC-NOT-LAST",
                    Some("only the final parameter may be variadic (`...`)".into()),
                );
            }
            if p.default.is_some() {
                self.err_coded(
                    p.span,
                    format!(
                        "variadic parameter `{}` cannot have a default value",
                        p.name
                    ),
                    "E-VARIADIC-DEFAULT",
                    Some("a variadic parameter already defaults to an empty list".into()),
                );
            }
            last_is_variadic |= is_last;
        }
        last_is_variadic
    }

    /// M4 default parameters: build the per-parameter default list for a free function, validating
    /// (a) trailing-only ordering — a defaulted parameter may not be followed by a required one
    /// (`E-DEFAULT-PARAM-ORDER`); (b) literal-only values (`E-DEFAULT-PARAM-EXPR`); (c) the default
    /// literal's type is assignable to the parameter type (`E-DEFAULT-PARAM-TYPE`). `resolved` is the
    /// already-resolved parameter types (parallel to `params`). Errors only — the list is returned
    /// regardless so the fill pass and arity check see the declared shape.
    pub(in crate::checker) fn collect_param_defaults(
        &mut self,
        params: &[crate::ast::Param],
        resolved: &[Ty],
    ) -> Vec<Option<crate::ast::Expr>> {
        let mut out = Vec::with_capacity(params.len());
        let mut seen_default = false;
        for (p, pty) in params.iter().zip(resolved) {
            match &p.default {
                None => {
                    if seen_default {
                        self.err_coded(
                            p.span,
                            format!(
                                "required parameter `{}` cannot follow a parameter with a default",
                                p.name
                            ),
                            "E-DEFAULT-PARAM-ORDER",
                            Some("move every defaulted parameter to the end of the list".into()),
                        );
                    }
                    out.push(None);
                }
                Some(e) => {
                    seen_default = true;
                    match literal_ty(e).or_else(|| self.variant_default_ty(e)) {
                        None => {
                            self.err_coded(
                                Self::expr_span(e),
                                format!(
                                    "default value for `{}` must be a literal constant",
                                    p.name
                                ),
                                "E-DEFAULT-PARAM-EXPR",
                                Some(
                                    "use a literal — a number, string, bool, bytes, null, or a zero-payload enum variant (`new Enum.Variant()`)".into(),
                                ),
                            );
                        }
                        Some(lt) => {
                            if !self.ty_assignable(&lt, pty) {
                                self.err_coded(
                                    Self::expr_span(e),
                                    format!(
                                        "default value of type `{lt}` is not assignable to parameter `{}` of type `{pty}`",
                                        p.name
                                    ),
                                    "E-DEFAULT-PARAM-TYPE",
                                    None,
                                );
                            }
                        }
                    }
                    out.push(Some((**e).clone()));
                }
            }
        }
        out
    }

    /// DEC-258: a qualified ZERO-payload enum-variant construction — `new Naming.Exact()` — also
    /// qualifies as a default-parameter constant: it is compile-time-known like any literal (the
    /// fill splices the construction expression itself, and the later `unwrap_new`/backend passes
    /// treat the spliced copy exactly like a hand-written one). Payload-carrying variants and
    /// generic enums stay rejected (their arguments/inference are not "a constant"). Returns the
    /// enum's nominal type when recognized.
    fn variant_default_ty(&self, e: &crate::ast::Expr) -> Option<Ty> {
        use crate::ast::Expr;
        let Expr::New(inner, _) = e else { return None };
        let Expr::Call { callee, args, .. } = &**inner else {
            return None;
        };
        if !args.is_empty() {
            return None;
        }
        let Expr::Member { object, name, .. } = &**callee else {
            return None;
        };
        let Expr::Ident(enum_name, _) = &**object else {
            return None;
        };
        let info = self.enums.get(enum_name)?;
        if !info.type_params.is_empty() || !info.variants.get(name)?.is_empty() {
            return None;
        }
        Some(Ty::Named(enum_name.clone(), Vec::new()))
    }

    /// Reject duplicate parameter names (Soundness Batch G, finding #7) on a function/method/ctor
    /// signature — previously the last declaration silently won (`E-DUP-PARAM`). Takes `(name, span)`
    /// pairs so it serves both `Param` and `CtorParam` sites.
    pub(in crate::checker) fn reject_dup_param_names<'a>(
        &mut self,
        params: impl Iterator<Item = (&'a str, Span)>,
    ) {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (name, span) in params {
            if !seen.insert(name) {
                self.err_coded(
                    span,
                    format!("duplicate parameter `{name}`"),
                    "E-DUP-PARAM",
                    Some("each parameter must have a distinct name".into()),
                );
            }
        }
    }

    /// M-RT overloading: validate a new overload `sig` against the overloads of the same name already
    /// collected in `existing`. Emits diagnostics only; the caller pushes `sig` regardless so later
    /// resolution sees the full set. A legal set shares one return type (`E-OVERLOAD-RETURN`) and has
    /// no two identical parameter signatures (`E-OVERLOAD-DUPLICATE`); a generic member cannot
    /// participate (`E-OVERLOAD-GENERIC`, deferred). The first declaration is always fine.
    pub(in crate::checker) fn validate_new_overload(
        &mut self,
        existing: &[FnSig],
        sig: &FnSig,
        name: &str,
        span: Span,
        kind: &str,
        allow_return_overload: bool,
    ) {
        if existing.is_empty() {
            return;
        }
        if !sig.type_params.is_empty() || existing.iter().any(|e| !e.type_params.is_empty()) {
            self.err_coded(
                span,
                format!("generic {kind} `{name}` cannot be overloaded"),
                "E-OVERLOAD-GENERIC",
                Some("a generic declaration must be the only one with its name; remove the type parameters or rename".into()),
            );
            return;
        }
        // Statics-B: every overload of one name must agree on `static`-ness. A mixed set has no sound
        // call form — `ClassName.m(args)` dispatches only the static candidates (the interpreter's
        // `call_static_method` filters by `static`), while `x.m(args)` dispatches only the instance
        // ones, so the checker (which sees the whole set) would accept calls the runtime rejects. PHP
        // also forbids a static and an instance method sharing a name.
        if sig.is_static != existing[0].is_static {
            self.err_coded(
                span,
                format!("overloaded {kind} `{name}` mixes `static` and instance declarations"),
                "E-OVERLOAD-STATIC-MIX",
                Some(
                    "all overloads of one name must be either all `static` or all instance methods"
                        .into(),
                ),
            );
        }
        // The PHP-erasure-collision guard: two non-identical parameter lists that collide under PHP
        // erasure (`string`/`bytes` → PHP `string`; `List`/`Map`/`Set` → PHP `array`). The
        // transpiler's `instanceof`/`is_*` dispatch can't tell them apart, so an ambiguous call would
        // fault on the Phorj backends but silently take the first PHP branch — reject at declaration.
        let erase_collision = |this: &mut Self| {
            if existing
                .iter()
                .any(|e| e.params != sig.params && overloads_erase_alike(&e.params, &sig.params))
            {
                this.err_coded(
                    span,
                    format!("overloaded {kind} `{name}` has two declarations indistinguishable in transpiled PHP"),
                    "E-OVERLOAD-ERASE",
                    Some("`string`/`bytes` both become PHP `string`, and `List`/`Map`/`Set` all become PHP `array`, so the dispatch can't tell these overloads apart — differentiate them by another parameter, or merge them".into()),
                );
            }
        };
        if !allow_return_overload {
            // Methods (and any caller that opts out): the classic rule — all overloads share one
            // return type, no two identical parameter signatures. Unchanged from pre-Slice-C.
            let want_ret = &existing[0].ret;
            if &sig.ret != want_ret {
                self.err_coded(
                    span,
                    format!(
                        "overloaded {kind} `{name}` must return the same type as its other overloads (`{want_ret}`), found `{}`",
                        sig.ret
                    ),
                    "E-OVERLOAD-RETURN",
                    Some("overloads model one operation over different argument types; differing returns suggest separate functions or generics".into()),
                );
            }
            if existing.iter().any(|e| e.params == sig.params) {
                self.err_coded(
                    span,
                    format!("overloaded {kind} `{name}` has two declarations with identical parameter types"),
                    "E-OVERLOAD-DUPLICATE",
                    Some("each overload must differ in its parameter types".into()),
                );
            } else {
                erase_collision(self);
            }
            return;
        }
        // Free functions (M-RT Slice C1): identical parameters with a DIFFERENT return type now form a
        // return-type overload set (resolved by a `<Type>` selector, mangled per return before any
        // backend). Two soundness guards remain: identical parameters AND return is still a true
        // duplicate; and a name must be EITHER a parameter-overload set (distinct params, shared
        // return) OR a pure return-overload set (identical params, distinct returns) — never both,
        // since runtime parameter dispatch cannot tell two identical-`ParamKind` overloads apart.
        match existing.iter().find(|e| e.params == sig.params) {
            Some(e) if e.ret == sig.ret => {
                self.err_coded(
                    span,
                    format!("overloaded {kind} `{name}` has two declarations with identical parameter types"),
                    "E-OVERLOAD-DUPLICATE",
                    Some("each overload must differ in its parameter types, or (return-type overloading) its return type".into()),
                );
            }
            Some(_) => {
                // Identical parameters, different return — a return-overload member. Reject only if the
                // set already mixes in a different-parameter overload.
                if existing.iter().any(|e| e.params != sig.params) {
                    self.mixed_overload_err(name, span, kind);
                }
            }
            None => {
                // Different parameters — a parameter-overload member.
                let existing_is_return_set =
                    existing.iter().all(|e| e.params == existing[0].params)
                        && existing.iter().any(|e| e.ret != existing[0].ret);
                if existing_is_return_set {
                    self.mixed_overload_err(name, span, kind);
                } else if sig.ret != existing[0].ret {
                    self.err_coded(
                        span,
                        format!(
                            "overloaded {kind} `{name}` must return the same type as its other overloads (`{}`), found `{}`",
                            existing[0].ret, sig.ret
                        ),
                        "E-OVERLOAD-RETURN",
                        Some("parameter overloads model one operation over different argument types and share a return type; for return-type overloading keep the parameters identical".into()),
                    );
                } else {
                    erase_collision(self);
                }
            }
        }
    }

    /// A name that mixes parameter-overloading and return-type overloading (M-RT Slice C1): rejected
    /// because the runtime parameter dispatch cannot disambiguate two identical-`ParamKind` overloads.
    pub(in crate::checker) fn mixed_overload_err(&mut self, name: &str, span: Span, kind: &str) {
        self.err_coded(
            span,
            format!("overloaded {kind} `{name}` mixes parameter overloading with return-type overloading"),
            "E-OVERLOAD-RETURN",
            Some("a name is EITHER overloaded by parameter types (sharing one return) OR by return type (identical parameters, differing returns) — split it into differently-named functions".into()),
        );
    }

    /// Validate a function's declared generic parameters: reject duplicates (`E-GENERIC-PARAM`) and
    /// names that shadow a built-in type (`int`, `List`, …), which would be silently ineffective
    /// because `resolve_type` matches the built-in first (M-RT S7).
    pub(in crate::checker) fn validate_type_params(&mut self, type_params: &[String], span: Span) {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for tp in type_params {
            if is_builtin_type_name(tp) {
                self.err_coded(
                    span,
                    format!("type parameter `{tp}` shadows a built-in type"),
                    "E-GENERIC-PARAM",
                    Some("pick a distinct name, e.g. `T`, `U`, `Elem`".into()),
                );
            } else if !seen.insert(tp.as_str()) {
                self.err_coded(
                    span,
                    format!("duplicate type parameter `{tp}`"),
                    "E-GENERIC-PARAM",
                    None,
                );
            }
        }
    }
}

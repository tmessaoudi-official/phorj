//! Call checking — free-function and named calls, bare-fn imports, `String.format`
//! directive analysis.

use super::*;

impl Checker {
    /// DEC-208 slice A: emit `E-TURBOFISH-NON-GENERIC` when explicit type arguments were written at a
    /// call site whose callee cannot take them (a non-generic user function, an intrinsic, a native, a
    /// constructor, a lambda value, …). Non-fatal — the caller then type-checks the call as if no
    /// turbofish were present, so argument diagnostics are still produced.
    pub(in crate::checker) fn reject_turbofish(&mut self, tf: &[Ty], what: &str, span: Span) {
        if tf.is_empty() {
            return;
        }
        self.err_coded(
            span,
            format!("`{what}` does not take explicit type arguments"),
            "E-TURBOFISH-NON-GENERIC",
            Some(
                "remove the `<…>` — only a generic function or method accepts explicit type arguments"
                    .into(),
            ),
        );
    }

    pub(in crate::checker) fn check_call(
        &mut self,
        callee: &crate::ast::Expr,
        args: &[crate::ast::Expr],
        type_args: &[crate::ast::Type],
        span: Span,
    ) -> Ty {
        use crate::ast::Expr;
        // DEC-208 slice A: resolve the written turbofish types once. Empty in the common inferred form.
        // Threaded to the generic free-fn / method seams (which consume it); the non-generic terminal
        // branches call `reject_turbofish` so a stray `<…>` is a clear error, never silently ignored.
        let tf: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(t)).collect();
        match callee {
            Expr::Ident(name, _) => {
                // Built-in fault intrinsics (M-faults 2a): `panic`/`todo`/`unreachable` (→ `never`) and
                // `assert` (→ `unit`). Recognized here before any user-function lookup; the names are
                // reserved (`E-RESERVED-INTRINSIC`) so this can't be shadowed.
                if let Some(t) = self.check_intrinsic_call(name, args, span) {
                    self.reject_turbofish(&tf, name, span);
                    return t;
                }
                // If the name is a local (or a `match`-arm binding) with function type, treat it
                // as a function-value call rather than a named-function call — the latter only
                // looks in `self.funcs` (top-level declarations) and would report "unknown
                // function `name`" for a lambda-typed local (M3 S3 Task 4).
                if let Some(Ty::Function(param_tys, ret_ty, throws)) = self.lookup(name) {
                    // DEC-222: consume the `?`-suppression flag and route each declared throw so a call
                    // of a throwing function VALUE discharges (or propagates) exactly like a named
                    // throwing call. Taken BEFORE `check_args` so it cannot leak into an argument.
                    let skip_throws = std::mem::take(&mut self.skip_throws_discharge);
                    // DEC-208 slice A: a function-value (lambda-typed local) call is non-generic —
                    // a stray turbofish on it is a clear error, never silently ignored.
                    self.reject_turbofish(&tf, name, span);
                    self.check_args("<lambda>", &param_tys, args, span);
                    for e in &throws {
                        self.route_call_throw(skip_throws, "<lambda>", e, span);
                    }
                    return *ret_ty;
                }
                // DEC-331 D9a: `x(args)` where `x` is a class-typed local carrying `#[Invoke]`
                // method(s) — a function-typed local was handled just above, so a `Named` local here
                // is an invoke receiver. Resolve which `#[Invoke]` method the arguments select and
                // record the rewrite `x(args)` → `x.<method>(args)` for `resolve_invoke_tostring`.
                if let Some(Ty::Named(cls, cargs)) = self.lookup(name) {
                    self.reject_turbofish(&tf, name, span);
                    let skip = std::mem::take(&mut self.skip_throws_discharge);
                    return self.resolve_invoke_call(&cls, &cargs, args, span, skip);
                }
                let ty = self.check_named_call(name, args, &tf, span);
                self.record_pending_fill(callee, args, span);
                ty
            }
            Expr::Member {
                object, name, safe, ..
            } => {
                // Namespaced native call: `console.println(x)` — head is an imported module
                // qualifier. The shadowing guard keeps an imported qualifier disjoint from every
                // value binding, so membership in the import map is decisive (no scope check).
                if !*safe {
                    if let Expr::Ident(q, _) = &**object {
                        if let Some(idx) = self
                            .imports
                            .get(q)
                            .and_then(|m| crate::native::index_of(m, name))
                        {
                            // `Reflection.typeName(x)` is resolved from `x`'s STATIC type and erased
                            // before any backend (Core.Reflection, Tier 3) — never the generic-native
                            // path. `q` is reused for the synthesized `className`/`kind` calls.
                            let n = &crate::native::registry()[idx];
                            if n.module == "Core.Reflection" && n.name == "typeName" {
                                return self.check_reflect_type_name(q, args, span);
                            }
                            // W3-5/DEC-199: `String.format(spec, args)` gets custom arg-type validation
                            // + a compile-time `E-FORMAT-UNSUPPORTED` gate on a literal spec; it stays a
                            // real runtime native (no rewrite), so the backends run `text_format` /
                            // `__phorj_format`.
                            if n.module == "Core.String" && n.name == "format" {
                                return self.check_string_format(args, span);
                            }
                            // DEC-331 D9b (spec §2 P2): `Conversion.toString(x)` on a class instance
                            // stringifies through the SAME `#[ToString]` call as interpolation.
                            if n.module == "Core.Conversion"
                                && n.name == "toString"
                                && args.len() == 1
                            {
                                return self.check_conversion_to_string(args, &tf, name, span);
                            }
                            self.require_option_for_result_bridge(n.module, n.name, span);
                            // Native turbofish is a slice-A limitation (natives carry no ordered
                            // type-parameter list); reject explicit type arguments on a native call.
                            self.reject_turbofish(&tf, name, span);
                            let ty = self.check_native_call(idx, args, span);
                            self.record_pending_fill(callee, args, span);
                            return ty;
                        }
                    }
                }
                // Static method call `ClassName.method(args)` (slice B0): the head is a class *name*
                // (not a value binding), resolved after the native path (an explicit import wins a
                // name collision with a class) but before instance-method dispatch. Mirrors the
                // static-field read in `check_member`.
                if !*safe {
                    if let Expr::Ident(cls, _) = &**object {
                        // DEC-234: a `new`-wrapped qualified injected construction wins over the
                        // static-method route when the qualifier is ALSO an injected class
                        // (`new Uri.UriMalformedError(…)`, `new Db.TimeoutError(…)` — `Uri`/`Db` name both
                        // the module qualifier and its main class). Only `new` heads divert:
                        // `Uri.parse(…)` still resolves as the static call below.
                        if self.under_new
                            && self.lookup_binding(cls).is_none()
                            && super::enforce_injected::module_of(name) == Some(cls.as_str())
                            && self.classes.contains_key(name)
                        {
                            self.under_new = false;
                            if let Some(t) = self.try_variant_or_class_call(name, args, span, false)
                            {
                                self.reject_turbofish(&tf, name, span);
                                return t;
                            }
                        }
                        if self.lookup_binding(cls).is_none() && self.classes.contains_key(cls) {
                            return self
                                .check_static_method_call(callee, cls, name, args, &tf, span);
                        }
                        // Built-in concurrency static — `Channel.new()` (M6 W4). `Channel`/`Task` are
                        // reserved built-in type names (not user classes), so route them before the
                        // instance-method fallthrough (which would type `Channel` as an unknown value).
                        if (cls == "Channel" || cls == "Task") && self.lookup_binding(cls).is_none()
                        {
                            self.reject_turbofish(&tf, name, span);
                            return self.check_concurrency_static(cls, name, args, span);
                        }
                        // Qualified injected-type construction `new Http.Router(args)` /
                        // `new Time.Duration(args)` (import-redesign S2): the head is an injected module
                        // qualifier (Http/Time/Decimal) and `name` one of its injected *classes*. Resolve
                        // as construction of the bare injected class — `unwrap_new` erases the `Member`
                        // callee to the bare `Router(args)` every backend builds, exactly like the
                        // qualified-variant path. Guarded on `name` being a known class, so a bare
                        // injected enum (`RoundingMode`) and any non-injected `A.B(...)` fall through.
                        if self.lookup_binding(cls).is_none()
                            && super::enforce_injected::module_of(name) == Some(cls.as_str())
                            && self.classes.contains_key(name)
                        {
                            let was_new = std::mem::take(&mut self.under_new);
                            if !was_new {
                                self.err_coded(
                                    span,
                                    format!("construct `{cls}.{name}` with `new {cls}.{name}(…)`"),
                                    "E-NEW-REQUIRED",
                                    Some(format!("write `new {cls}.{name}(…)`")),
                                );
                            }
                            // Injected-class ctors (Http.Router / Time.Duration / Decimal) declare no
                            // `throws`, so the discharge set is empty — pass `false` (bare discharge).
                            if let Some(t) = self.try_variant_or_class_call(name, args, span, false)
                            {
                                self.reject_turbofish(&tf, name, span);
                                return t;
                            }
                        }
                        // DEC-302 enum static methods `Enum.cases()` / `Enum.from(x)` /
                        // `Enum.tryFrom(x)` — resolved before variant construction (they are reserved
                        // names, never variants — `E-ENUM-RESERVED-VARIANT`). Not `new`-wrapped, so the
                        // `Member` callee survives `unwrap_new` and every backend sees `Enum.cases()`.
                        if self.lookup_binding(cls).is_none()
                            && self.enums.contains_key(cls)
                            && matches!(name.as_str(), "cases" | "from" | "tryFrom")
                        {
                            self.reject_turbofish(&tf, name, span);
                            return self.check_enum_static(cls, name, args, span);
                        }
                        // Qualified enum-variant construction `Enum.Variant(args)` (slice A1): the head
                        // is an enum *name* (not a value binding). Resolves after native/class/concurrency
                        // (an import or class of the same name wins), before instance-method dispatch.
                        // Erased to the bare variant call before any backend (see
                        // `check_qualified_variant_call`).
                        if self.lookup_binding(cls).is_none() && self.enums.contains_key(cls) {
                            self.reject_turbofish(&tf, name, span);
                            return self.check_qualified_variant_call(cls, name, args, span);
                        }
                    }
                }
                self.check_method_call(callee, object, name, args, &tf, *safe, span)
            }
            other => {
                // A callee expression that is itself a call (`f()()`, `getFn()?()`) may carry a
                // `?`-suppression flag meant for THIS outer call — take it before evaluating the callee
                // so `check_expr(other)` doesn't consume it, then apply it to the outer call's throws.
                let skip_throws = std::mem::take(&mut self.skip_throws_discharge);
                // DEC-239 contextual pipe lambda: `x |> (v => …)` (and the multi-`%` IIFE) lowers to
                // an immediately-invoked lambda whose single param is `Type::Infer`. Check the piped
                // argument FIRST, flow its type into the param (recorded for AST materialization),
                // and type the call as the lambda body's type. Only the pipe parser/lowering can
                // produce an `Infer` lambda param, so this cannot fire on user-written IIFEs.
                if let Expr::Lambda {
                    params,
                    ret,
                    throws,
                    body,
                    span: lspan,
                } = other
                {
                    if params.len() == 1
                        && matches!(params[0].ty, crate::ast::Type::Infer(_))
                        && args.len() == 1
                    {
                        let arg_ty = self.check_expr(&args[0]);
                        // Piping `void` would bind "no value" into the lambda param — the same
                        // footgun E-VOID-CAPTURE blocks for `var x = noop()`. PHP silently coerces
                        // void→null here; phorj rejects (DEC-239 recorded divergence, phorj-better).
                        let arg_ty = if matches!(arg_ty, Ty::Void) {
                            self.err_coded(
                                span,
                                "a `void` value cannot be piped into a lambda — it produces no value",
                                "E-VOID-CAPTURE",
                                Some("pipe a value-producing expression, or drop the pipe".into()),
                            );
                            Ty::Error
                        } else {
                            arg_ty
                        };
                        self.reject_turbofish(&tf, "call", span);
                        let lam_ty = self.check_lambda_with(
                            params,
                            ret,
                            throws,
                            body,
                            *lspan,
                            Some(&arg_ty),
                        );
                        if let Ty::Function(_, ret_ty, lthrows) = lam_ty {
                            for e in &lthrows {
                                self.route_call_throw(skip_throws, "<lambda>", e, span);
                            }
                            return *ret_ty;
                        }
                        return Ty::Error;
                    }
                }
                // Evaluate the callee to see if it is a function value (closure or named-fn ref).
                let callee_ty = self.check_expr(other);
                match callee_ty {
                    Ty::Function(param_tys, ret_ty, throws) => {
                        self.reject_turbofish(&tf, "call", span);
                        self.check_args("<lambda>", &param_tys, args, span);
                        // DEC-222: route each declared throw of the called function value.
                        for e in &throws {
                            self.route_call_throw(skip_throws, "<lambda>", e, span);
                        }
                        *ret_ty
                    }
                    Ty::Optional(inner) if matches!(*inner, Ty::Function(..)) => {
                        for a in args {
                            self.check_expr(a);
                        }
                        self.err(
                            span,
                            "not callable — the function value is optional; unwrap it first with `??` or `if (var …)`",
                        )
                    }
                    Ty::Error => {
                        for a in args {
                            self.check_expr(a);
                        }
                        Ty::Error
                    }
                    // DEC-331 D9a: a call-returning callee of class type (`getAdder()(5)`) is callable
                    // iff its class carries an `#[Invoke]` method. (A `Member` callee `x.m(5)` is a
                    // method call handled earlier, so it never reaches this arm.)
                    Ty::Named(cls, cargs) => {
                        self.reject_turbofish(&tf, "call", span);
                        self.resolve_invoke_call(&cls, &cargs, args, span, skip_throws)
                    }
                    _ => {
                        for a in args {
                            self.check_expr(a);
                        }
                        self.err(span, "expression is not callable")
                    }
                }
            }
        }
    }

    /// `name(args)` — a free function, enum-variant constructor (Task 5), or class
    /// constructor (Task 6). Free-function case here.
    pub(in crate::checker) fn check_named_call(
        &mut self,
        name: &str,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        span: Span,
    ) -> Ty {
        // Consume the throws-mode `?` suppression flag up front (a throwing call under `?` propagates
        // instead of discharging locally). Taken before the variant/ctor probe so it cannot leak —
        // the flag is only ever set for a free throwing function (`free_call_throws`), never a ctor.
        let skip_throws = std::mem::take(&mut self.skip_throws_discharge);
        // Feature C: take the `new`-prefix flag BEFORE checking args, so a bare construction *argument*
        // still requires its own `new`. A construction reached without `new` is `E-NEW-REQUIRED`.
        let was_new = std::mem::take(&mut self.under_new);
        if self.is_construction_name(name) && !was_new {
            self.err_coded(
                span,
                format!("construct `{name}` with `new {name}(…)`"),
                "E-NEW-REQUIRED",
                Some(format!("write `new {name}(…)`")),
            );
        }
        if let Some(t) = self.try_variant_or_class_call(name, args, span, skip_throws) {
            // A variant constructor / class construction is not a generic call site here (DEC-208
            // slice A leaves construction turbofish out of scope).
            self.reject_turbofish(tf, name, span);
            return t;
        }
        let sigs = match self.funcs.get(name) {
            Some(s) => s.clone(),
            None => {
                // DEC-197: a bare call to a member-imported module function (`import Core.Output.printLine;`
                // ⇒ `printLine(x)`). Resolved AFTER user functions, so `local > user fn > imported native`
                // holds (locals are handled earlier in `check_call`, user funcs in the `Some` arm above).
                if let Some((module, real)) = self.fn_imports.get(name).cloned() {
                    // Member-imported natives carry no ordered type-parameter list (slice-A limitation).
                    self.reject_turbofish(tf, name, span);
                    return self.resolve_bare_fn_import(&module, &real, args, span);
                }
                for a in args {
                    self.check_expr(a);
                }
                return self.err(span, format!("unknown function `{name}`"));
            }
        };
        // M-RT Slice C1: a return-type-overloaded call reached without a `<Type>` selector has no type
        // context to choose a member (the selector arm `check_overload_select` handles the resolved
        // case and never funnels here). C2 will resolve these from a shallow sink; in C1 it is an error.
        if self.return_overload_sets.contains_key(name) {
            self.reject_turbofish(tf, name, span);
            for a in args {
                self.check_expr(a);
            }
            return self.err_coded(
                span,
                format!("call to return-type-overloaded `{name}` has no type context to pick an overload"),
                "E-OVERLOAD-NO-CONTEXT",
                Some(format!("add a return-type selector — `<Type>{name}(…)` — naming which overload's return type you want")),
            );
        }
        // Single overload — the common case, identical to pre-overloading behaviour (incl. generics).
        if sigs.len() == 1 {
            let sig = &sigs[0];
            // Discharge each checked exception the callee declares: a bare call must catch it in an
            // enclosing `try` (M-faults 2b); the propagate (`?`) path used the suppression flag.
            for e in &sig.throws {
                self.route_call_throw(skip_throws, name, e, span);
            }
            return if sig.type_params.is_empty() {
                // M4: defaulted-arity check (a non-default function has all-`None` defaults, so this
                // is exactly the old exact-arity `check_args`).
                self.reject_turbofish(tf, name, span);
                // DEC-297: a named/positional-mixed call is front-normalized to positional (defaults
                // filled) then checked + recorded as a REPLACE fill; named + variadic is rejected in v1.
                if crate::checker::calls::args::has_named_args(args) {
                    if sig.variadic {
                        self.err_coded(
                            span,
                            format!("`{name}`: named arguments are not supported with a variadic parameter (v1)"),
                            "E-NAMED-ARG-UNSUPPORTED",
                            Some("call a variadic function positionally".into()),
                        );
                    } else if let Some(pos) =
                        self.normalize_named_args(name, &sig.param_names, &sig.defaults, args, span)
                    {
                        self.check_args(name, &sig.params, &pos, span);
                        self.pending_named = Some(pos);
                    }
                } else {
                    // DEC-298: a variadic free function (single sig) collects trailing args into a list.
                    self.check_args_defaulted_v(
                        name,
                        &sig.params,
                        &sig.defaults,
                        args,
                        span,
                        sig.variadic,
                    );
                }
                sig.ret.clone()
            } else {
                // DEC-297: named args on a generic call are rejected in v1 (inference + reorder combo).
                if crate::checker::calls::args::has_named_args(args) {
                    self.err_coded(
                        span,
                        format!(
                            "`{name}`: named arguments are not supported on a generic call (v1)"
                        ),
                        "E-NAMED-ARG-UNSUPPORTED",
                        Some("call the generic function with positional arguments".into()),
                    );
                }
                // DEC-208 slice A: a generic free function accepts turbofish (`identity<int>(5)`),
                // consumed by `check_generic_call` to pre-seed the substitution.
                self.check_generic_call(
                    name,
                    &sig.type_params,
                    &sig.params,
                    &sig.ret,
                    &sig.type_param_bounds,
                    tf,
                    args,
                    span,
                )
            };
        }
        // DEC-297: named args on an overloaded call are rejected in v1 (name-based slot resolution
        // would have to pick the overload first — deferred).
        if crate::checker::calls::args::has_named_args(args) {
            self.err_coded(
                span,
                format!("`{name}`: named arguments are not supported on an overloaded call (v1)"),
                "E-NAMED-ARG-UNSUPPORTED",
                Some("call an overloaded function with positional arguments".into()),
            );
        }
        // Overload set (M-RT): generic members were rejected at collection, so every overload is
        // monomorphic. The call's result is the shared return type (`E-OVERLOAD-RETURN`); resolution
        // here is *static* (for typing) — the runtime dispatch is byte-identical by construction.
        self.reject_turbofish(tf, name, span);
        self.check_overload_call(name, &sigs, args, span, skip_throws)
    }

    /// DEC-197: type a bare call to a member-imported module function and record the qualified rewrite
    /// (`printLine(x)` → `Output.printLine(x)`) that every backend already lowers. `module`/`real` come
    /// from the `fn_imports` map (built from `index_of` matches, so the native exists). Types via the
    /// SAME [`Self::check_native_call`] the qualified path uses, so arity/generics/defaults/deprecation
    /// warnings are identical. The rewrite REUSES the original call `span` (its `span.start` keys both
    /// the replacement and the reified-operand side-table) so `native(x) + 1` types on the VM exactly as
    /// the interpreter accepts it (Invariant 6/7). Any trailing default omitted by the call (`pending_fill`
    /// set inside `check_native_call`) is folded into the rewritten argument list, so the recorded call is
    /// full-arity — one merged rewrite, never a separate default-fill collision on the same span.
    pub(in crate::checker) fn resolve_bare_fn_import(
        &mut self,
        module: &str,
        real: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        // W3-5/DEC-199: a bare member-imported `format(spec, args)` gets `String.format`'s custom
        // validation (`check_string_format`) AND the standard bare→qualified rewrite (so the backends
        // resolve the real native via the `String.` qualifier, like every other bare fn import). The
        // qualified Member-arm path needs no rewrite (the call is already qualified); only the bare path
        // does — hence the rewrite is recorded here, not inside `check_string_format`.
        if module == "Core.String" && real == "format" {
            let ret = self.check_string_format(args, span);
            let qualified = crate::ast::Expr::Call {
                callee: Box::new(crate::ast::Expr::Member {
                    object: Box::new(crate::ast::Expr::Ident("String".to_string(), span)),
                    name: "format".to_string(),
                    safe: false,
                    sep: crate::ast::MemberSep::Dot,
                    span,
                }),
                args: args.to_vec(),
                type_args: Vec::new(),
                span,
            };
            self.ufcs_resolutions.insert(span.start, qualified);
            return ret;
        }
        let idx = crate::native::index_of(module, real)
            .expect("fn_imports entries are built from index_of matches");
        let ret = self.check_native_call(idx, args, span);
        let mut full = args.to_vec();
        if let Some(defaults) = self.pending_fill.take() {
            full.extend(defaults);
        }
        let leaf = module.rsplit('.').next().unwrap_or(module);
        let qualified = crate::ast::Expr::Call {
            callee: Box::new(crate::ast::Expr::Member {
                object: Box::new(crate::ast::Expr::Ident(leaf.to_string(), span)),
                name: real.to_string(),
                safe: false,
                sep: crate::ast::MemberSep::Dot,
                span,
            }),
            args: full,
            type_args: Vec::new(),
            span,
        };
        self.ufcs_resolutions.insert(span.start, qualified);
        ret
    }
}

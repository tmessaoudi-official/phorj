//! Method-call dispatch — the `Ty::Named` receiver arm of `check_method_call`.

use super::*;

impl Checker {
    /// Dispatch `object.name(args)` when the (optional-peeled, bound-resolved) receiver is a concrete
    /// class or an interface `Ty::Named(cls, cargs)` — extracted from `check_method_call`'s big match
    /// so the caller stays under the file-size cap. Returns the FINAL type: the tail results
    /// (`check_method_sigs`, the no-method error) are `opt_wrap`ped under a `?.` call (`safe`), while
    /// the early `return`s (static-via-instance, overload-no-context, and the UFCS fallback whose
    /// null-safety `try_ufcs` already applied via `ufcs_nav`) return raw — byte-identical to the
    /// pre-split control flow.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::checker) fn dispatch_named_call(
        &mut self,
        object: &crate::ast::Expr,
        cls: String,
        cargs: Vec<Ty>,
        name: &str,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        fill_callee: Option<&crate::ast::Expr>,
        safe: bool,
        span: Span,
        ufcs_nav: UfcsNav,
    ) -> Ty {
        // A class method, or — when `cls` is an interface (M-RT S2) — an interface method
        // from its flattened (own + `extends`) signature set. Interface-typed receivers
        // dispatch polymorphically at runtime through the concrete class, so only the static
        // signature is needed here.
        // The method's overload set (M-RT): one or more signatures sharing a return type. An
        // interface method (no overloading) contributes a single signature.
        let sigs = self
            .classes
            .get(&cls)
            .and_then(|info| info.methods.get(name))
            .map(|v| {
                v.iter()
                    .map(|s| {
                        (
                            s.params.clone(),
                            s.ret.clone(),
                            s.throws.clone(),
                            s.defaults.clone(),
                            s.param_names.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .or_else(|| {
                if self.interfaces.contains_key(&cls) {
                    // Interface-method `throws` via an interface-typed receiver is a
                    // documented follow-up (the flattened form drops `throws`); the concrete
                    // implementer's class-method call still discharges. Emit no throws here.
                    self.iface_flat_methods(&cls)
                        .into_iter()
                        .find(|(m, _)| m == name)
                        .map(|(_, sig)| {
                            let arity = sig.0.len();
                            // Interface-flattened sigs carry no param names → named args on an
                            // interface-typed receiver reject cleanly (DEC-297, v1 follow-up).
                            vec![(sig.0, sig.1, Vec::new(), vec![None; arity], Vec::new())]
                        })
                } else {
                    None
                }
            });
        // Substitute the *class* type parameters with this instance's type arguments
        // (`Box<int>` ⇒ `{T → int}`), so a method returning/taking `T` is checked at the
        // concrete type (M-RT generics-all). empty for a non-generic class/interface, so this
        // is the identity in the common case. Any *method-level* `<U>` that survives is then
        // inferred from the call's arguments below.
        let theta = self.class_subst(&cls, &cargs);
        let ret = match sigs {
            Some(sigs) => {
                // W0-3: a `static` method reached through an instance value (`a.m()` /
                // `this.m()`) is rejected — static members are reachable only via the class
                // name (`ClassName.m()`), mirroring the static-field-via-instance rule. PHP
                // tolerates `$a->staticMethod()`, but the developer's rule is "static not via
                // instance"; a `ClassName.m()` site never funnels here (`check_static_method_call`).
                if self
                    .classes
                    .get(&cls)
                    .is_some_and(|i| i.static_methods.contains(name))
                {
                    for a in args {
                        self.check_expr(a);
                    }
                    return self.err_coded(
                        span,
                        format!("`{name}` is a static method of `{cls}` — call it as `{cls}.{name}(…)`, not through an instance"),
                        "E-STATIC-VIA-INSTANCE",
                        Some(format!("write `{cls}.{name}(…)`")),
                    );
                }
                // M-RT S2.2: a bare (selector-less) return-overloaded method call has no type
                // context to pick a member — C1 requires a `<Type>` selector at the call site.
                // The selector path resolves via `resolve_method_return_overload` and never
                // funnels here, so any return-overload method reaching this point is bare.
                if self.is_return_overload_method(&cls, name) {
                    for a in args {
                        self.check_expr(a);
                    }
                    return self.err_coded(
                        span,
                        format!("call to return-type-overloaded method `{name}` has no type context to pick an overload"),
                        "E-OVERLOAD-NO-CONTEXT",
                        Some(format!("add a return-type selector — `<Type>receiver.{name}(…)` — naming which overload's return type you want")),
                    );
                }
                // Wave 1.1: a `private`/`protected` method called from outside its scope is
                // rejected (interface methods have no `method_vis` entry ⇒ public ⇒ no-op).
                let v = self
                    .classes
                    .get(&cls)
                    .and_then(|i| i.method_vis.get(name).cloned());
                self.enforce_member_vis(v, name, span, false);
                // DEC-208 slice A: the method's ordered type-parameter names for turbofish
                // seeding — a generic method is single-overload (overloaded generics are
                // rejected at collection), so only a lone signature contributes names.
                let method_tps: Vec<String> = self
                    .classes
                    .get(&cls)
                    .and_then(|i| i.methods.get(name))
                    .filter(|v| v.len() == 1)
                    .map(|v| v[0].type_params.clone())
                    .unwrap_or_default();
                let applied: Vec<MethodSig> = sigs
                    .iter()
                    .map(|(ps, r, th, ds, pn)| {
                        (
                            ps.iter().map(|p| apply_subst(p, &theta)).collect(),
                            apply_subst(r, &theta),
                            th.iter().map(|t| apply_subst(t, &theta)).collect(),
                            ds.clone(),
                            pn.clone(),
                        )
                    })
                    .collect();
                self.check_method_sigs(name, &applied, &method_tps, fill_callee, args, tf, span)
            }
            None => {
                // UFCS fallback (Slice 6): `inst.f(args)` with no method `f` may be the free
                // function / imported native `f(inst, args)`. `?.` desugars to a null-safe
                // `match` (F-002).
                self.reject_turbofish(tf, name, span);
                if let Some(ret) = self.try_ufcs(
                    object,
                    &Ty::Named(cls.clone(), cargs.clone()),
                    name,
                    args,
                    span,
                    ufcs_nav,
                ) {
                    return ret;
                }
                for a in args {
                    self.check_expr(a);
                }
                self.err(span, format!("type `{cls}` has no method `{name}`"))
            }
        };
        if safe {
            Self::opt_wrap(ret)
        } else {
            ret
        }
    }
}

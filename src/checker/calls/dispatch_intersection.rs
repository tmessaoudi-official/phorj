//! Method-call dispatch — the `Ty::Intersection` receiver arm of `check_method_call`.

use super::*;

impl Checker {
    /// Dispatch `object.name(args)` when the receiver is an intersection `Ty::Intersection(members)`
    /// (M-RT S5, DEC-245) — extracted from `check_method_call`. Collects `name`'s signatures from
    /// EVERY member (interfaces + the lone class) into one merged OVERLOAD SET, enforces visibility on
    /// the declaring class member, then dispatches through the DEC-058 overload machinery. Returns the
    /// FINAL type: the tail results are `opt_wrap`ped under a `?.` call (`safe`), while the UFCS
    /// fallback early-`return`s raw (its null-safety is already applied by `try_ufcs` via `ufcs_nav`) —
    /// byte-identical to the pre-split control flow.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::checker) fn dispatch_intersection_call(
        &mut self,
        object: &crate::ast::Expr,
        members: Vec<Ty>,
        name: &str,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        fill_callee: Option<&crate::ast::Expr>,
        safe: bool,
        span: Span,
        ufcs_nav: UfcsNav,
    ) -> Ty {
        // Member access over an intersection (M-RT S5, DEC-245): collect `name`'s
        // signatures from EVERY member (interfaces + the lone class) into one merged
        // OVERLOAD SET — identical signatures dedupe, distinct parameter lists coexist
        // and dispatch through the DEC-058 overload machinery (`check_method_sigs`
        // multi-arm); the uninhabitable same-params/different-return combo was rejected
        // at the type site (`E-INTERSECT-SIG`, narrowed). None → E-INTERSECT-NO-MEMBER.
        // The value is a concrete instance underneath, so dispatch stays polymorphic at
        // runtime — no Op change.
        let mut found: Option<Vec<MethodSig>> = None;
        // DEC-208 slice A: ordered type-parameter names of the resolved member method (for
        // turbofish seeding). Only a lone class-method signature contributes any.
        let mut found_tps: Vec<String> = Vec::new();
        for m in &members {
            if let Ty::Named(mn, margs) = m {
                let sig = self
                    .classes
                    .get(mn)
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
                        if self.interfaces.contains_key(mn) {
                            self.iface_flat_methods(mn)
                                .into_iter()
                                .find(|(mm, _)| mm == name)
                                .map(|(_, sig)| {
                                    let arity = sig.0.len();
                                    vec![(sig.0, sig.1, Vec::new(), vec![None; arity], Vec::new())]
                                })
                        } else {
                            None
                        }
                    });
                if let Some(sigs) = sig {
                    if found.is_none() {
                        // Turbofish seeding stays first-declarer (a generic member method
                        // is single-overload by collection, so the merge can't grow it).
                        found_tps = self
                            .classes
                            .get(mn)
                            .and_then(|info| info.methods.get(name))
                            .filter(|v| v.len() == 1)
                            .map(|v| v[0].type_params.clone())
                            .unwrap_or_default();
                    }
                    let theta = self.class_subst(mn, margs);
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
                    let set = found.get_or_insert_with(Vec::new);
                    for s in applied {
                        // Merge identical signatures (params+ret agree — throws/defaults
                        // may differ between an interface's empty view and the class's
                        // real one; the FIRST occurrence wins, matching the old
                        // first-found behavior for the agree case).
                        if !set.iter().any(|(ps, r, _, _, _)| *ps == s.0 && *r == s.1) {
                            set.push(s);
                        }
                    }
                }
            }
        }
        let ret = match found {
            Some(applied) => {
                // DEC-251(c): an intersection receiver must NOT bypass method visibility.
                // Enforce on the lone CLASS member that declares `name` (≤1 by
                // E-INTERSECT-MULTI-CLASS), INDEPENDENT of which member the signature resolved
                // from — members are sorted by name (`intersection_of`), so an interface
                // declaring the same name could otherwise be found first and skip enforcement
                // (interfaces have no `method_vis` ⇒ public). This closes that name-order bypass:
                // `x.privateMethod()` on an `I & C`-typed `x` is rejected as the `Ty::Named` path would.
                for m in &members {
                    if let Ty::Named(mn, _) = m {
                        if self
                            .classes
                            .get(mn)
                            .is_some_and(|i| i.methods.contains_key(name))
                        {
                            let v = self
                                .classes
                                .get(mn)
                                .and_then(|i| i.method_vis.get(name).cloned());
                            self.enforce_member_vis(v, name, span, false);
                            break;
                        }
                    }
                }
                self.check_method_sigs(name, &applied, &found_tps, fill_callee, args, tf, span)
            }
            None => {
                self.reject_turbofish(tf, name, span);
                if let Some(ret) = self.try_ufcs(
                    object,
                    &Ty::Intersection(members.clone()),
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
                self.err_coded(
                    span,
                    format!(
                        "no member of `{}` has method `{name}`",
                        Ty::Intersection(members)
                    ),
                    "E-INTERSECT-NO-MEMBER",
                    None,
                )
            }
        };
        if safe {
            Self::opt_wrap(ret)
        } else {
            ret
        }
    }
}

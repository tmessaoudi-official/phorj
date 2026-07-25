//! Call checking — method/static-method call entry points, substitutions, visibility.
//!
//! The two big receiver-kind arms of [`Checker::check_method_call`] live in sibling modules
//! `dispatch_named` / `dispatch_intersection`; member reads in `member`, the substitution builders
//! in `subst`, visibility enforcement in `visibility`, and the SQL-injection lint in `lint`.

use super::*;

impl Checker {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::checker) fn check_method_call(
        &mut self,
        callee: &crate::ast::Expr,
        object: &crate::ast::Expr,
        name: &str,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        safe: bool,
        span: Span,
    ) -> Ty {
        // DEC-249: a `?.` call cannot take the default-fill rewrite — its own null-safe desugar is
        // keyed by this same call span, and two rewrites on one key would silently collide. Omitted
        // defaulted args on `?.` become a clean deferral error inside `check_method_sigs`.
        let fill_callee = if safe { None } else { Some(callee) };
        let obj = self.check_expr(object);
        // Peel an optional/null receiver, enforcing the non-null discipline: a plain `.m()` on a
        // `T?` is `E-OPT-USE`; `?.m()` unwraps and re-wraps the result as optional (M3 S2.3).
        let base = match &obj {
            Ty::Error => {
                for a in args {
                    self.check_expr(a);
                }
                return Ty::Error;
            }
            Ty::Null if safe => {
                for a in args {
                    self.check_expr(a);
                }
                return Ty::Null; // `null?.m()` short-circuits to null
            }
            Ty::Optional(_) | Ty::Null if !safe => {
                for a in args {
                    self.check_expr(a);
                }
                return self.err_opt_use(span, name, &obj, "call method");
            }
            Ty::Optional(inner) => (**inner).clone(),
            other => other.clone(),
        };
        // How a UFCS fallback (if reached) was navigated — plain `.`, `?.` on a nullable receiver (the
        // null-safe `match` desugar), or `?.` on a non-null receiver (F-002).
        let ufcs_nav = if !safe {
            UfcsNav::Plain
        } else if matches!(obj, Ty::Optional(_) | Ty::Null) {
            UfcsNav::SafeNullable
        } else {
            UfcsNav::SafeNonNull
        };
        // DEC-211: a BOUNDED type parameter resolves member access against its bound interface — so
        // `a.cmp(b)` where `a: T` and `T: Comparable` type-checks against `Comparable`'s members. An
        // unbounded `T` is left opaque (unchanged). The instantiation site separately guarantees the
        // concrete type argument implements the bound, so this resolution is sound after erasure.
        let base = match &base {
            Ty::Param(p) => match self.active_type_param_bounds.iter().find(|(n, _)| n == p) {
                Some((_, iface)) => Ty::Named(iface.clone(), Vec::new()),
                None => base,
            },
            _ => base,
        };
        // DEC-208 slice F — SQL-injection compile-time lint (`W-SQL-INJECTION`). Type-directed: fires
        // only on `Core.DatabaseModule`'s `Database.prepare(<interpolated SQL>)` when a hole splices a non-constant value
        // (a variable / field / call) into the SQL text — steering to a `?` placeholder + `.bind(...)`.
        // A non-fatal lint (the program still compiles — the interpolation escape hatch is preserved).
        self.lint_sql_injection(&base, name, args, span);
        let ret = match base {
            // Built-in concurrency handles (M6 W4): `Channel<T>` (send/recv), `Task<T>` (join).
            // Dispatched before user-class lookup — `Channel`/`Task` are reserved built-ins, never a
            // user class. `?.` on a (never-optional) handle behaves like a plain call.
            Ty::Named(ref cls, ref cargs) if cls == "Channel" || cls == "Task" => {
                let elem = cargs.first().cloned().unwrap_or(Ty::Error);
                self.reject_turbofish(tf, name, span);
                return self
                    .check_concurrency_method(cls, &elem, name, args, span)
                    .expect("concurrency method dispatch is total");
            }
            // A class method, or an interface method via an interface-typed receiver (M-RT S2) —
            // see `dispatch_named_call`. It returns the FINAL type (opt-wrap already applied).
            Ty::Named(cls, cargs) => {
                return self.dispatch_named_call(
                    object,
                    cls,
                    cargs,
                    name,
                    args,
                    tf,
                    fill_callee,
                    safe,
                    span,
                    ufcs_nav,
                );
            }
            // Member access over an intersection (M-RT S5, DEC-245) — see `dispatch_intersection_call`.
            Ty::Intersection(members) => {
                return self.dispatch_intersection_call(
                    object,
                    members,
                    name,
                    args,
                    tf,
                    fill_callee,
                    safe,
                    span,
                    ufcs_nav,
                );
            }
            Ty::Error => Ty::Error,
            other => {
                // UFCS fallback (Slice 6): a member call on a primitive/container receiver (`xs.map(g)`,
                // `s.upper()`) is `f(receiver, args)` — a free function or imported native. A `?.` call
                // desugars to a null-safe `match` (F-002). Turbofish on a UFCS-dispatched free function
                // is a slice-A limitation.
                self.reject_turbofish(tf, name, span);
                if let Some(ret) = self.try_ufcs(object, &other, name, args, span, ufcs_nav) {
                    return ret;
                }
                for a in args {
                    self.check_expr(a);
                }
                self.err(span, format!("type `{other}` has no method `{name}`"))
            }
        };
        if safe {
            Self::opt_wrap(ret)
        } else {
            ret
        }
    }

    /// `ClassName.method(args)` — a **static** method call (slice B0). The class is known (the caller
    /// verified `cls` is a class name, not a value binding). The method must be declared `static`;
    /// calling an instance method this way is `E-STATIC-CALL`. Arg/overload/throws checking reuses
    /// [`check_method_sigs`] (no receiver, so no class-type-arg substitution — a static method that
    /// uses the class's own type parameter is out of scope this slice).
    pub(in crate::checker) fn check_static_method_call(
        &mut self,
        callee: &crate::ast::Expr,
        cls: &str,
        name: &str,
        args: &[crate::ast::Expr],
        tf: &[Ty],
        span: Span,
    ) -> Ty {
        let sigs: Option<Vec<MethodSig>> = self
            .classes
            .get(cls)
            .and_then(|i| i.methods.get(name))
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
                    .collect()
            });
        // DEC-208 slice A: ordered type-parameter names of a lone static method signature (for
        // turbofish seeding).
        let method_tps: Vec<String> = self
            .classes
            .get(cls)
            .and_then(|i| i.methods.get(name))
            .filter(|v| v.len() == 1)
            .map(|v| v[0].type_params.clone())
            .unwrap_or_default();
        let Some(sigs) = sigs else {
            for a in args {
                self.check_expr(a);
            }
            return self.err(span, format!("class `{cls}` has no static method `{name}`"));
        };
        if !self.classes[cls].static_methods.contains(name) {
            for a in args {
                self.check_expr(a);
            }
            return self.err_coded(
                span,
                format!("`{name}` is an instance method of `{cls}`, not a static one"),
                "E-STATIC-CALL",
                Some(format!(
                    "`ClassName.{name}(…)` calls a `static` method — make `{name}` static, or call it on an instance (`x.{name}(…)`)"
                )),
            );
        }
        // Statics-B: an overloaded static is dispatched at runtime exactly like an instance overload
        // (the VM's `method_overloads` table + `dispatch::select_overload`, the same selector the
        // interpreter's `call_static_method` runs), so `check_method_sigs` handles the multi-sig set
        // here just as it does for `x.m(args)`. The static/instance-consistency of the overload set is
        // guaranteed at declaration (`E-OVERLOAD-STATIC-MIX`), so every candidate here is static.
        // Visibility, mirroring the instance method-call site (Wave 1.1).
        let v = self
            .classes
            .get(cls)
            .and_then(|i| i.method_vis.get(name).cloned());
        self.enforce_member_vis(v, name, span, false);
        self.check_method_sigs(name, &sigs, &method_tps, Some(callee), args, tf, span)
    }

    /// UFCS fallback (Slice 6, `docs/plans/2026-06-25-overnight-design-forks-review.plan.md` F-001):
    /// a member call `object.name(args)` that did **not** resolve to a method is re-resolved as the
    /// free/native call `name(object, args)`, **method-first** having already failed. A candidate is,
    /// in priority order: (1) a user free function `name`, or (2) any *imported* `Core.*` native
    /// `name`, whose **first parameter accepts the receiver type** (`unify`, so a generic native like
    /// `map: (List<T>,(T)->U)` matches a `List<int>` receiver). Returns `Some(ret)` once a candidate is
    /// chosen (recording the desugared call in `ufcs_resolutions` for [`rewrite_ufcs`], which the
    /// backends consume verbatim — no new `Op`), or `None` when no callable named `name` fits at all
    /// (the caller then emits the original "no method" error). The receiver `recv_ty` is the
    /// already-checked, optional-peeled type — the receiver expression is *not* re-checked here, so a
    /// throwing-call receiver discharges exactly once.
    /// `Result.toOption` (Wave B B-2b, DEC-185) bridges to `Core.Option`: its transpiled helper builds
    /// `new Some(…)`/`new None()`, and those PHP classes exist ONLY when the Option prelude is injected
    /// (gated on `import Core.Option;`). Used without that import, the call type-checks and runs on the
    /// interpreter+VM (which build `Value::Enum(ty:"Option")` directly) but FATALS in the transpiled PHP
    /// (`Class "Some" not found`) — a byte-identity break (Invariant #1). Reject it in the checker so all
    /// three backends refuse in lockstep, matching DEC-182's explicit-import model. Called from both the
    /// qualified (`Result.toOption(r)`) and UFCS (`r.toOption()`) native-resolution sites.
    pub(in crate::checker) fn require_option_for_result_bridge(
        &mut self,
        module: &str,
        name: &str,
        span: Span,
    ) {
        if module == "Core.Result"
            && name == "toOption"
            && !self.imports.values().any(|m| m == "Core.Option")
        {
            self.err_coded(
                span,
                "`Result.toOption` returns `Option<T>` but `Core.Option` is not imported"
                    .to_string(),
                "E-RESULT-TOOPTION-NEEDS-OPTION",
                Some(
                    "add `import Core.Option;` — the bridge produces a `Core.Option` value"
                        .to_string(),
                ),
            );
        }
    }
}

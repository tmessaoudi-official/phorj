//! Call checking — method/static-method/member access, substitutions, visibility.

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
        // only on `Core.Db`'s `Db.prepare(<interpolated SQL>)` when a hole splices a non-constant value
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
            Ty::Named(cls, cargs) => {
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
                                    vec![(sig.0, sig.1, Vec::new(), vec![None; arity])]
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
                match sigs {
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
                            .map(|(ps, r, th, ds)| {
                                (
                                    ps.iter().map(|p| apply_subst(p, &theta)).collect(),
                                    apply_subst(r, &theta),
                                    th.iter().map(|t| apply_subst(t, &theta)).collect(),
                                    ds.clone(),
                                )
                            })
                            .collect();
                        self.check_method_sigs(
                            name,
                            &applied,
                            &method_tps,
                            fill_callee,
                            args,
                            tf,
                            span,
                        )
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
                }
            }
            Ty::Intersection(members) => {
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
                                            vec![(sig.0, sig.1, Vec::new(), vec![None; arity])]
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
                                .map(|(ps, r, th, ds)| {
                                    (
                                        ps.iter().map(|p| apply_subst(p, &theta)).collect(),
                                        apply_subst(r, &theta),
                                        th.iter().map(|t| apply_subst(t, &theta)).collect(),
                                        ds.clone(),
                                    )
                                })
                                .collect();
                            let set = found.get_or_insert_with(Vec::new);
                            for s in applied {
                                // Merge identical signatures (params+ret agree — throws/defaults
                                // may differ between an interface's empty view and the class's
                                // real one; the FIRST occurrence wins, matching the old
                                // first-found behavior for the agree case).
                                if !set.iter().any(|(ps, r, _, _)| *ps == s.0 && *r == s.1) {
                                    set.push(s);
                                }
                            }
                        }
                    }
                }
                match found {
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
                        self.check_method_sigs(
                            name,
                            &applied,
                            &found_tps,
                            fill_callee,
                            args,
                            tf,
                            span,
                        )
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
                }
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

    pub(in crate::checker) fn check_member(
        &mut self,
        object: &crate::ast::Expr,
        name: &str,
        safe: bool,
        span: Span,
    ) -> Ty {
        // Static field read `ClassName.field` (M-mut.7): the head is a class *name* not shadowed by a
        // local (locals-first), and `?.` makes no sense on a class. Resolved before `check_expr`,
        // which would otherwise reject the bare class name as an unknown variable.
        if !safe {
            if let crate::ast::Expr::Ident(cls, _) = object {
                if self.lookup_binding(cls).is_none() && self.classes.contains_key(cls) {
                    // A `const` class constant (Feature A) is resolved before a static field — it is
                    // class-name-only and visibility-checked. `consts` already carries inherited
                    // entries (merge_inherited), so `Sub.MAX` resolves an inherited `MAX`.
                    if let Some(entry) = self.classes[cls].consts.get(name).cloned() {
                        let visible = match entry.vis {
                            MemberVis::Public => true,
                            MemberVis::Private => {
                                self.cur_class.as_deref() == Some(entry.owner.as_str())
                            }
                            MemberVis::Protected => self
                                .cur_class
                                .as_deref()
                                .is_some_and(|c| self.is_subtype(c, &entry.owner)),
                        };
                        if !visible {
                            let kind = if entry.vis == MemberVis::Private {
                                "private"
                            } else {
                                "protected"
                            };
                            self.err_coded(
                                span,
                                format!("`{name}` is a {kind} constant of `{}`", entry.owner),
                                "E-CONST-VISIBILITY",
                                Some(format!(
                                    "it is readable only {}",
                                    if entry.vis == MemberVis::Private {
                                        format!("inside `{}`", entry.owner)
                                    } else {
                                        format!("inside `{}` and its subclasses", entry.owner)
                                    }
                                )),
                            );
                        }
                        return entry.ty;
                    }
                    return match self.classes[cls].statics.get(name).cloned() {
                        Some(t) => {
                            // W0-2: a `private`/`protected` static read from outside its scope is
                            // rejected here (E-FIELD-VISIBILITY), mirroring the const path above and
                            // the instance-field path below — closing the run≡runvm≡PHP hole.
                            let v = self.classes[cls].static_vis.get(name).cloned();
                            self.enforce_member_vis(v, name, span, true);
                            t
                        }
                        None => self.err_coded(
                            span,
                            format!("`{cls}` has no static field `{name}`"),
                            "E-STATIC-UNKNOWN",
                            Some(
                                "static fields are declared `static …` and read as `Class.field`"
                                    .into(),
                            ),
                        ),
                    };
                }
            }
        }
        let obj = self.check_expr(object);
        // Peel an optional/null receiver, enforcing the non-null discipline: a plain `.field` on a
        // `T?` is `E-OPT-USE`; `?.field` unwraps and re-wraps the result as optional (M3 S2.3).
        let base = match &obj {
            Ty::Error => return Ty::Error,
            Ty::Null if safe => return Ty::Null, // `null?.field` short-circuits to null
            Ty::Optional(_) | Ty::Null if !safe => {
                return self.err_opt_use(span, name, &obj, "read field");
            }
            Ty::Optional(inner) => (**inner).clone(),
            other => other.clone(),
        };
        let field_ty = match base {
            Ty::Named(cls, cargs) => {
                // A property hook (M-mut.7b) is resolved before a stored field: `o.name` runs its
                // `get`. Reading a hook with no `get` (write-only) is `E-HOOK-NO-GET`. A hook is not
                // generic (`package Main` only), so no substitution applies to its type.
                if let Some(h) = self.classes.get(&cls).and_then(|info| info.hooks.get(name)) {
                    let (hty, has_get) = (h.ty.clone(), h.has_get);
                    if !has_get {
                        return self.err_coded(
                            span,
                            format!("property `{name}` of `{cls}` is write-only (no `get`)"),
                            "E-HOOK-NO-GET",
                            Some("add a `get => …;` clause to read it".into()),
                        );
                    }
                    return if safe { Self::opt_wrap(hty) } else { hty };
                }
                let found = self
                    .classes
                    .get(&cls)
                    .and_then(|info| info.fields.get(name).cloned());
                match found {
                    // Substitute the class type parameters with the instance's type arguments, so a
                    // `T` field reads at the concrete type (`Box<int>().value : int`) — identity for a
                    // non-generic class (M-RT generics-all). Wave 1.1: a `private`/`protected` field
                    // read from outside its scope is rejected here (closing the run↔PHP hole).
                    Some(t) => {
                        let v = self
                            .classes
                            .get(&cls)
                            .and_then(|i| i.field_vis.get(name).cloned());
                        self.enforce_member_vis(v, name, span, true);
                        apply_subst(&t, &self.class_subst(&cls, &cargs))
                    }
                    // A `const` is class-name-only: reading it through an instance (`c.MAX`) is an
                    // error, with a hint pointing at the correct `ClassName.MAX` form (Feature A).
                    None if self
                        .classes
                        .get(&cls)
                        .is_some_and(|info| info.consts.contains_key(name)) =>
                    {
                        self.err_coded(
                            span,
                            format!("`{name}` is a constant of `{cls}` — read it as `{cls}.{name}`, not through an instance"),
                            "E-CONST-INSTANCE-ACCESS",
                            Some(format!("write `{cls}.{name}`")),
                        )
                    }
                    // A `static` field is class-name-only too: reading it through an instance
                    // (`a.count`) is rejected, mirroring the static-*method*-via-instance rule
                    // (E-STATIC-VIA-INSTANCE) and the const sibling above (UA-0.6). Before this,
                    // `a.staticField` fell through to the generic "has no field" message.
                    None if self
                        .classes
                        .get(&cls)
                        .is_some_and(|info| info.statics.contains_key(name)) =>
                    {
                        self.err_coded(
                            span,
                            format!("`{name}` is a static field of `{cls}` — read it as `{cls}.{name}`, not through an instance"),
                            "E-STATIC-FIELD-VIA-INSTANCE",
                            Some(format!("write `{cls}.{name}`")),
                        )
                    }
                    None => self.err(span, format!("type `{cls}` has no field `{name}`")),
                }
            }
            Ty::Intersection(members) => {
                // Only the lone class member can carry fields (interfaces have none, M-RT S5). Search
                // for the field on the class member; none → E-INTERSECT-NO-MEMBER.
                let mut found: Option<(Ty, String)> = None;
                for m in &members {
                    if let Ty::Named(mn, margs) = m {
                        if let Some(t) = self
                            .classes
                            .get(mn)
                            .and_then(|info| info.fields.get(name).cloned())
                        {
                            found =
                                Some((apply_subst(&t, &self.class_subst(mn, margs)), mn.clone()));
                            break;
                        }
                    }
                }
                match found {
                    Some((t, owner)) => {
                        // DEC-251(c): an intersection receiver must NOT bypass field visibility —
                        // enforce private/protected on the owning class, exactly as the `Ty::Named`
                        // path above (else `x.privateField` on an `I & C`-typed `x` slips through).
                        let v = self
                            .classes
                            .get(&owner)
                            .and_then(|i| i.field_vis.get(name).cloned());
                        self.enforce_member_vis(v, name, span, true);
                        t
                    }
                    None => self.err_coded(
                        span,
                        format!(
                            "no member of `{}` has field `{name}`",
                            Ty::Intersection(members)
                        ),
                        "E-INTERSECT-NO-MEMBER",
                        None,
                    ),
                }
            }
            Ty::Error => Ty::Error,
            other => self.err(span, format!("type `{other}` has no field `{name}`")),
        };
        if safe {
            Self::opt_wrap(field_ty)
        } else {
            field_ty
        }
    }

    /// Build the substitution mapping a generic class's type parameters to a concrete instance's type
    /// arguments — `{T → int}` for a `Box<int>` receiver (M-RT generics-all). empty (the identity
    /// substitution) for a non-generic class or any non-class name, so member/method access on a
    /// non-generic type is unchanged. `zip` tolerates an arity mismatch defensively.
    pub(in crate::checker) fn class_subst(&self, cls: &str, cargs: &[Ty]) -> HashMap<String, Ty> {
        match self.classes.get(cls) {
            Some(info) => info
                .type_params
                .iter()
                .cloned()
                .zip(cargs.iter().cloned())
                .collect(),
            None => HashMap::new(),
        }
    }

    /// Enforce member visibility (Wave 1.1) at an instance-member access site. `entry` is the
    /// member's `(visibility, declaring-owner)` (cloned out of the receiver class's `field_vis` /
    /// `method_vis`); `None` ⇒ no recorded visibility ⇒ public by construction (e.g. an interface
    /// method) ⇒ no-op. `private` is reachable only from inside the owner; `protected` from the owner
    /// and its subclasses (`cur_class` is the enclosing class, `None` in a free function). Mirrors the
    /// `const` check (`E-CONST-VISIBILITY`) so `run ≡ runvm ≡ transpiled PHP` all reject the same
    /// out-of-scope access — closing the documented byte-identity hole. `is_field` picks the code.
    /// DEC-241: enforce a member's asymmetric SET visibility at a WRITE site (`o.f = e`,
    /// `C.f = e`, a `with { f = … }` override). `entry` is the `set_vis`/`static_set_vis` row —
    /// absent means writes follow the ordinary read visibility (already enforced by the caller).
    pub(in crate::checker) fn enforce_set_vis(
        &mut self,
        entry: Option<(MemberVis, String)>,
        name: &str,
        span: Span,
    ) {
        let Some((vis, owner)) = entry else { return };
        let cur = self.cur_class.clone();
        let allowed = match vis {
            MemberVis::Public => true,
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if allowed {
            return;
        }
        let (visword, scope) = if vis == MemberVis::Private {
            ("private(set)", format!("inside `{owner}`"))
        } else {
            (
                "protected(set)",
                format!("inside `{owner}` and its subclasses"),
            )
        };
        self.err_coded(
            span,
            format!("field `{name}` is {visword} — assignable only {scope}"),
            "E-ASSIGN-SET-VISIBILITY",
            Some("read access is unaffected; assign it from within the owning scope, or widen the `(set)` modifier".into()),
        );
    }

    pub(in crate::checker) fn enforce_member_vis(
        &mut self,
        entry: Option<(MemberVis, String)>,
        name: &str,
        span: Span,
        is_field: bool,
    ) {
        let Some((vis, owner)) = entry else { return };
        if vis == MemberVis::Public {
            return;
        }
        let cur = self.cur_class.clone();
        let visible = match vis {
            MemberVis::Public => true,
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if visible {
            return;
        }
        let (kindword, code) = if is_field {
            ("field", "E-FIELD-VISIBILITY")
        } else {
            ("method", "E-METHOD-VISIBILITY")
        };
        let (visword, scope) = if vis == MemberVis::Private {
            ("private", format!("inside `{owner}`"))
        } else {
            ("protected", format!("inside `{owner}` and its subclasses"))
        };
        self.err_coded(
            span,
            format!("`{name}` is a {visword} {kindword} of `{owner}`"),
            code,
            Some(format!("it is accessible only {scope}")),
        );
    }

    /// Enforce a constructor's visibility at a `new C(...)` site (Soundness Batch A — the 7th
    /// member-visibility access site). A `private` ctor is constructible only inside its declaring
    /// class (`cur_class == owner`); a `protected` ctor inside the declaring class or a subclass. The
    /// in-scope cases are the factory/singleton patterns (a static factory method or a static field
    /// initializer, both running in the class's scope). Public (the default) is always allowed.
    pub(in crate::checker) fn enforce_ctor_vis(&mut self, class_name: &str, span: Span) {
        let Some(info) = self.classes.get(class_name) else {
            return;
        };
        let vis = info.ctor_vis;
        if vis == MemberVis::Public {
            return;
        }
        let owner = info.ctor_owner.clone();
        let cur = self.cur_class.clone();
        let visible = match vis {
            MemberVis::Public => true,
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if visible {
            return;
        }
        let (visword, scope) = if vis == MemberVis::Private {
            ("private", format!("inside `{owner}`"))
        } else {
            ("protected", format!("inside `{owner}` and its subclasses"))
        };
        self.err_coded(
            span,
            format!("the constructor of `{class_name}` is {visword}"),
            "E-CTOR-VISIBILITY",
            Some(format!(
                "construct it only {scope} — e.g. a static factory method or a static field initializer"
            )),
        );
    }

    /// The substitution mapping a generic enum's type parameters to a scrutinee's type arguments
    /// (`Option<int>` ⇒ `{T → int}`), so a `match` binds a variant payload at the concrete type
    /// (`Some(n)` ⇒ `n: int`). empty for a non-generic enum, so it is the identity in the common case
    /// (M-RT generic enums). Mirror of [`class_subst`].
    pub(in crate::checker) fn enum_subst(
        &self,
        enum_name: &str,
        eargs: &[Ty],
    ) -> HashMap<String, Ty> {
        match self.enums.get(enum_name) {
            Some(info) => info
                .type_params
                .iter()
                .cloned()
                .zip(eargs.iter().cloned())
                .collect(),
            None => HashMap::new(),
        }
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

    /// DEC-208 slice F — the SQL-injection lint. Fires `W-SQL-INJECTION` when a `Core.Db` `Db.prepare`
    /// receives a string-INTERPOLATED literal whose hole splices a NON-constant value into the SQL text.
    ///
    /// Type-directed and import-gated ("nothing in the wind"): the receiver must type to the `Db` class
    /// AND the program must import `Core.Db` (module or member form), so a user class happening to be
    /// named `Db` with a `prepare` method is never hijacked. A fully-constant interpolation (every hole a
    /// literal) does NOT warn; a plain non-interpolated literal has no hole so never warns. This is a
    /// non-fatal lint — the program still compiles (the deliberately-built-query escape hatch stays open).
    fn lint_sql_injection(&mut self, base: &Ty, name: &str, args: &[crate::ast::Expr], span: Span) {
        if name != "prepare" {
            return;
        }
        // Receiver must be the `Db` class …
        if !matches!(base, Ty::Named(cls, _) if cls == "Db") {
            return;
        }
        // … and it must be Core.Db's `Db` (imported — module `Core.Db` or a member `Core.Db.X`), never a
        // coincidental user class named `Db`.
        if !self
            .imports
            .values()
            .any(|m| m == "Core.Db" || m.starts_with("Core.Db."))
        {
            return;
        }
        // The SQL argument must be a string LITERAL with at least one NON-constant interpolation hole.
        let crate::ast::Expr::Str(parts, _) = (match args.first() {
            Some(a) => a,
            None => return,
        }) else {
            return;
        };
        let has_nonconst_hole = parts.iter().any(|p| match p {
            crate::ast::StrPart::Literal(_) => false,
            crate::ast::StrPart::Expr(inner) => !expr_is_const_sql(inner),
        });
        if !has_nonconst_hole {
            return;
        }
        self.warn_coded(
            span,
            "interpolating a value into SQL risks injection — use a `?` placeholder and `.bind(...)`",
            "W-SQL-INJECTION",
            Some(
                "replace the interpolated `{…}` with a `?` placeholder and pass the value to `.bind(...)` \
                 (or a `:name` placeholder + `.bindNamed(...)`) — the value is then sent separately from the \
                 SQL text and can never be parsed as SQL"
                    .to_string(),
            ),
        );
    }
}

/// True iff `e` is a compile-time constant for the SQL-injection lint (DEC-208 slice F): a literal
/// scalar, or a string literal whose every interpolation hole is itself constant (recursively). Any
/// other form — a variable, field access, call, index, arithmetic, cast, … — is NON-constant: it may
/// carry user data, so splicing it into SQL text is the injection risk the lint flags. Conservative by
/// design (a named/class `const` interpolated into SQL still warns — steering it to a bind is harmless
/// and keeps the rule simple); the escape hatch is that it is only a warning, never an error.
fn expr_is_const_sql(e: &crate::ast::Expr) -> bool {
    use crate::ast::{Expr, StrPart};
    match e {
        Expr::Int(..)
        | Expr::Float(..)
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::Bytes(..)
        | Expr::Decimal { .. } => true,
        Expr::Str(parts, _) => parts.iter().all(|p| match p {
            StrPart::Literal(_) => true,
            StrPart::Expr(inner) => expr_is_const_sql(inner),
        }),
        _ => false,
    }
}

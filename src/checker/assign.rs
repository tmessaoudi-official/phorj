//! `impl Checker` — assign cluster (M-Decomp W2). See checker/mod.rs for the struct + entry points.

use super::*;

impl Checker {
    /// `name = value` local reassignment (M-mut.1): the binding must exist and be `mutable`, and the
    /// value assignable to its type.
    pub(super) fn check_local_reassign(
        &mut self,
        name: &str,
        vty: &Ty,
        target: &crate::ast::Expr,
        value: &crate::ast::Expr,
    ) {
        match self.lookup_binding(name) {
            None => {
                self.err_coded(
                    Self::expr_span(target),
                    format!("cannot assign to unknown variable `{name}`"),
                    "E-ASSIGN-UNKNOWN",
                    None,
                );
            }
            Some((bty, false)) => {
                self.err_coded(
                    Self::expr_span(target),
                    format!("`{name}` is immutable and cannot be reassigned"),
                    "E-ASSIGN-IMMUTABLE",
                    Some(format!(
                        "declare it `mutable` (e.g. `mutable {bty} {name} = …;`)"
                    )),
                );
            }
            Some((bty, true)) => {
                // DEC-537: a direct statement of the block that narrowed `name` is checked against the
                // DECLARED type, and the narrowing then follows the value.
                if let Some(declared) = self.direct_narrowed_declared(name) {
                    if !self.ty_assignable(vty, &declared) {
                        self.err_coded(
                            Self::expr_span(value),
                            format!("cannot assign `{vty}` to `{name}: {declared}`"),
                            "E-ASSIGN-TYPE",
                            None,
                        );
                    } else if !matches!(vty, Ty::Error | Ty::Never) {
                        self.retype_direct(name, vty.clone());
                    }
                    return;
                }
                if !self.ty_assignable(vty, &bty) {
                    let hint = self
                        .narrowed_declared(name)
                        .filter(|d| self.ty_assignable(vty, d))
                        .map(|d| {
                            format!(
                                "`{name}` is declared `{d}` but narrowed to `{bty}` here; an assignment that changes a narrowed type must be a direct statement of the block that narrowed it (DEC-537)"
                            )
                        });
                    self.err_coded(
                        Self::expr_span(value),
                        format!("cannot assign `{vty}` to `{name}: {bty}`"),
                        "E-ASSIGN-TYPE",
                        hint,
                    );
                }
            }
        }
    }

    /// `container[index] = value` value-type element set (M-mut.5). The container must be a `mutable`
    /// local `List<T>` or `Map<K, V>` (nested places `a[i][j]`/`this.f[i]` are a later slice →
    /// `E-ASSIGN-TARGET`). For a list the index must be `int` and the value a `T`; for a map the
    /// index must be the key type `K` and the value a `V`.
    /// The innermost base of an assignment place, walking `[index]` steps down. `g[i][j]` → `g`;
    /// `g` → `g`. Returns `None` for a non-local base (a field access, a call result) — those are the
    /// deferred field-base slice. Index-only chains (slice 1a) resolve to their root local.
    fn place_root(mut e: &crate::ast::Expr) -> Option<&str> {
        use crate::ast::Expr;
        loop {
            match e {
                Expr::Ident(n, _) => return Some(n),
                Expr::Index { object, .. } => e = object,
                _ => return None,
            }
        }
    }

    pub(super) fn check_index_assign(
        &mut self,
        object: &crate::ast::Expr,
        index: &crate::ast::Expr,
        vty: &Ty,
        value: &crate::ast::Expr,
        span: Span,
    ) {
        let ity = self.check_expr(index);
        // The place root is the innermost base of the target. A **nested value-index** chain
        // (`g[i][j]`) is allowed when the root is a mutable local: COW makes `make_mut` root-to-leaf
        // mutate the root's nested container in place (Spec `nested-value-index-assign`, slice 1a). A
        // non-local base (a field `this.f[i]`, a call result) is the deferred slice 1b.
        let name = match Self::place_root(object) {
            Some(n) => n.to_string(),
            None => {
                self.err_coded(
                    Self::expr_span(object),
                    "the container of an element assignment must be a variable or a nested index of one",
                    "E-ASSIGN-TARGET",
                    Some("a field base (`this.f[i]`, `obj.f[i]`) lands in the next slice".into()),
                );
                return;
            }
        };
        let mutable = match self.lookup_binding(&name) {
            Some((_, m)) => m,
            None => {
                self.err_coded(
                    Self::expr_span(object),
                    format!("cannot assign into unknown variable `{name}`"),
                    "E-ASSIGN-UNKNOWN",
                    None,
                );
                return;
            }
        };
        // The container the final index targets is the *type of `object`* — for a flat place this is
        // the root's declared type; for a nested place it is the element type at the outer index.
        let cty = self.check_expr(object);
        if !mutable {
            self.err_coded(
                Self::expr_span(object),
                format!("`{name}` is immutable; its elements cannot be set"),
                "E-ASSIGN-IMMUTABLE",
                Some(format!(
                    "declare it `mutable` (e.g. `mutable {cty} {name} = …;`)"
                )),
            );
            return;
        }
        match cty {
            Ty::List(elem) => {
                if !self.ty_assignable(&ity, &Ty::Int) {
                    self.err(span, format!("list index must be `int`, found `{ity}`"));
                }
                if !self.ty_assignable(vty, &elem) {
                    self.err_coded(
                        Self::expr_span(value),
                        format!("cannot set a `{vty}` element into `{name}: List<{elem}>`"),
                        "E-ASSIGN-TYPE",
                        None,
                    );
                }
            }
            // `pair[i] = e` on a `[T; N]`: element-set is length-preserving, so it is allowed; the
            // static literal-index bounds are checked here too (Phase 1 types slice).
            Ty::FixedList(elem, n) => {
                if !self.ty_assignable(&ity, &Ty::Int) {
                    self.err(span, format!("list index must be `int`, found `{ity}`"));
                }
                self.fixedlist_static_bounds(index, n, &elem, span);
                if !self.ty_assignable(vty, &elem) {
                    self.err_coded(
                        Self::expr_span(value),
                        format!("cannot set a `{vty}` element into `{name}: [{elem}; {n}]`"),
                        "E-ASSIGN-TYPE",
                        None,
                    );
                }
            }
            Ty::Map(k, v) => {
                if !self.ty_assignable(&ity, &k) {
                    self.err(span, format!("map key must be `{k}`, found `{ity}`"));
                }
                if !self.ty_assignable(vty, &v) {
                    self.err_coded(
                        Self::expr_span(value),
                        format!("cannot set a `{vty}` value into `{name}: Map<{k}, {v}>`"),
                        "E-ASSIGN-TYPE",
                        None,
                    );
                }
            }
            other => {
                self.err_coded(
                    Self::expr_span(object),
                    format!("`{name}: {other}` is not indexable for assignment"),
                    "E-ASSIGN-TARGET",
                    Some("only `List<T>` and `Map<K, V>` support `container[i] = e`".into()),
                );
            }
        }
    }

    /// `obj with { f = e, … }` (M-mut.4a): `obj` must be a concrete class; each overridden name must
    /// be one of its fields and each value assignable to that field's type. The result type is the
    /// class itself (a fresh instance). Codes `E-WITH-NONCLASS`/`E-WITH-FIELD`/`E-WITH-TYPE`.
    pub(super) fn check_clone_with(
        &mut self,
        object: &crate::ast::Expr,
        fields: &[(String, crate::ast::Expr)],
        span: Span,
    ) -> Ty {
        let obj_ty = self.check_expr(object);
        // Always check the override value expressions (surface nested errors regardless).
        let value_tys: Vec<Ty> = fields.iter().map(|(_, e)| self.check_expr(e)).collect();
        let class = match &obj_ty {
            Ty::Error => return Ty::Error,
            Ty::Named(name, _) if self.classes.contains_key(name) => name.clone(),
            other => {
                return self.err_coded(
                    span,
                    format!("`with` requires a class instance, found `{other}`"),
                    "E-WITH-NONCLASS",
                    Some(
                        "`with` produces a copy of a class instance with some fields replaced"
                            .into(),
                    ),
                );
            }
        };
        // Snapshot the class's field types (clone to drop the borrow before `err_coded` needs &mut).
        let field_tys = self.classes[&class].fields.clone();
        for ((name, _), vty) in fields.iter().zip(value_tys.iter()) {
            match field_tys.get(name) {
                None => {
                    self.err_coded(
                        Self::expr_span(object),
                        format!("`{class}` has no field `{name}` to set in `with`"),
                        "E-WITH-FIELD",
                        None,
                    );
                }
                Some(fty) => {
                    // Wave 1.1: `with` lowers to PHP `clone($o, [...])`, which enforces visibility on
                    // the overridden properties — so an out-of-scope override of a `private`/`protected`
                    // field is rejected here too (sibling of the field-write hole).
                    let v = self.classes[&class].field_vis.get(name).cloned();
                    self.enforce_member_vis(v, name, span, true);
                    // DEC-241: a `with { f = … }` override is a WRITE — PHP 8.4's clone-with
                    // enforces `(set)` visibility on overridden properties; mirror it.
                    let sv = self.classes[&class].set_vis.get(name).cloned();
                    self.enforce_set_vis(sv, name, span);
                    if !self.ty_assignable(vty, fty) {
                        self.err_coded(
                            span,
                            format!("cannot set `{name}: {fty}` to `{vty}` in `with`"),
                            "E-WITH-TYPE",
                            None,
                        );
                    }
                }
            }
        }
        obj_ty
    }

    /// `opt!` checked force-unwrap (M3 S2.5): `T?` → `T`. Every use is linted (`W-FORCE-UNWRAP`) to
    /// nudge toward `??`/`?.`/if-let; force-unwrapping a non-optional is `E-OPT-UNWRAP`.
    /// Is `name` a `Result`-shaped enum — exactly an `Success` variant (arity 1) and an `Failure` variant
    /// (arity 1)? `?` propagation is defined only over this shape (M-faults 2a).
    pub(super) fn is_result_enum(&self, name: &str) -> bool {
        self.enums.get(name).is_some_and(|e| {
            e.variants.get("Success").is_some_and(|f| f.len() == 1)
                && e.variants.get("Failure").is_some_and(|f| f.len() == 1)
        })
    }

    /// The type of variant `v`'s single payload field on enum `name`, with the enum's type parameters
    /// substituted by `args` (`Success` payload of `Result<int, _>` ⇒ `int`).
    pub(super) fn result_payload(&self, name: &str, v: &str, args: &[Ty]) -> Ty {
        let theta = self.enum_subst(name, args);
        self.enums[name].variants[v]
            .first()
            .map_or(Ty::Error, |t| apply_subst(t, &theta))
    }

    /// `expr?` — Result-error propagation (M-faults 2a). Unwraps an `Success` payload to its value, or
    /// early-returns the `Failure` from the enclosing function. Requires the operand to be a `Result`-shaped
    /// enum AND the enclosing function (`cur_ret`) to return that *same* enum, with the operand's `Failure`
    /// payload assignable to the function's (`E-PROPAGATE-CONTEXT`/`E-PROPAGATE-ERR`). Returns the
    /// unwrapped `Success` payload type. Called only from a let-initializer; any other position is rejected
    /// by the `Expr::Propagate` arm in `check_expr` (`E-PROPAGATE-POSITION`).
    pub(super) fn check_propagate(&mut self, inner: &crate::ast::Expr, span: Span) -> Ty {
        let t = self.check_expr(inner);
        self.check_propagate_typed(t, span)
    }

    /// The typed half of [`Self::check_propagate`] — validates an ALREADY-COMPUTED operand type
    /// (the throws-mode attempt hands its `Plain` type here so the operand is checked once).
    pub(super) fn check_propagate_typed(&mut self, t: Ty, span: Span) -> Ty {
        let (name, args) = match &t {
            Ty::Error => return Ty::Error,
            Ty::Named(n, a) if self.is_result_enum(n) => (n.clone(), a.clone()),
            other => {
                return self.err_coded(
                    span,
                    format!("`?` requires a `Result`-shaped operand (an enum with `Success`/`Failure` variants), found `{other}`"),
                    "E-PROPAGATE-CONTEXT",
                    Some("`?` unwraps `Success` or early-returns `Failure`".into()),
                );
            }
        };
        match self.cur_ret.clone() {
            Ty::Named(rn, rargs) if rn == name => {
                let err_in = self.result_payload(&name, "Failure", &args);
                let err_ret = self.result_payload(&rn, "Failure", &rargs);
                if !self.ty_assignable(&err_in, &err_ret) {
                    self.err_coded(
                        span,
                        format!("`?` propagates an `Failure({err_in})` but the enclosing `{name}` carries `Failure({err_ret})`"),
                        "E-PROPAGATE-ERR",
                        None,
                    );
                }
                self.result_payload(&name, "Success", &args)
            }
            other => self.err_coded(
                span,
                format!("`?` early-returns the `Failure`, so the enclosing function must return `{name}<…>`, but it returns `{other}`"),
                "E-PROPAGATE-CONTEXT",
                Some(format!("declare the function to return `{name}<…>`")),
            ),
        }
    }

    /// Recognize and check a fault intrinsic call (`panic`/`todo`/`unreachable`/`assert`); returns
    /// `Some(ty)` if `name` is one, else `None` (a normal call). Messages must be string *literals*
    /// (compile-time const) so both backends bake byte-identical fault text (M-faults 2a).
    pub(super) fn check_intrinsic_call(
        &mut self,
        name: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Option<Ty> {
        match name {
            "panic" => {
                if args.len() != 1 {
                    self.err(span, "`panic` takes one string-literal message");
                } else {
                    self.require_str_literal(&args[0], span);
                }
                Some(Ty::Never)
            }
            "todo" | "unreachable" => {
                if !args.is_empty() {
                    self.err(span, format!("`{name}` takes no arguments"));
                }
                Some(Ty::Never)
            }
            "assert" => {
                if args.is_empty() || args.len() > 2 {
                    self.err(
                        span,
                        "`assert` takes a bool and an optional string-literal message",
                    );
                } else {
                    let c = self.check_expr(&args[0]);
                    if !self.ty_assignable(&c, &Ty::Bool) {
                        self.err(
                            span,
                            format!("`assert` condition must be `bool`, found `{c}`"),
                        );
                    }
                    if let Some(m) = args.get(1) {
                        self.require_str_literal(m, span);
                    }
                }
                Some(Ty::Void)
            }
            _ => None,
        }
    }

    /// Require `e` to be a string *literal* (one `StrPart::Literal`, no interpolation) — the fault
    /// intrinsics bake their message at compile time (M-faults 2a).
    pub(super) fn require_str_literal(&mut self, e: &crate::ast::Expr, span: Span) {
        if !matches!(e, crate::ast::Expr::Str(parts, _)
            if parts.len() == 1 && matches!(parts[0], crate::ast::StrPart::Literal(_)))
        {
            self.err_coded(
                span,
                "this intrinsic's message must be a plain string literal",
                "E-INTRINSIC-LITERAL",
                Some("interpolation/expressions aren't allowed here yet".into()),
            );
        }
        self.check_expr(e);
    }

    pub(super) fn check_force(&mut self, inner: &crate::ast::Expr, span: Span) -> Ty {
        let t = self.check_expr(inner);
        match t {
            Ty::Error => Ty::Error,
            Ty::Optional(inner_ty) => {
                self.warn_coded(
                    span,
                    "force-unwrap `!` asserts an optional is non-null and faults at runtime if it is null",
                    "W-FORCE-UNWRAP",
                    Some("prefer `??` (default), `?.` (safe access), or `if (var x = opt)` to handle null without a possible fault".into()),
                );
                *inner_ty
            }
            other => self.err_coded(
                span,
                format!("force-unwrap `!` requires an optional `T?`, found non-optional `{other}`"),
                "E-OPT-UNWRAP",
                Some("`!` unwraps a `T?` to `T`; a non-optional value is already non-null".into()),
            ),
        }
    }

    /// `E-OPT-USE`: a plain `.`/`.m()` was used on an optional (or `null`) receiver, which could
    /// dereference null. Steers the developer to `?.`, `??`, or a checked unwrap `!`.
    pub(super) fn err_opt_use(&mut self, span: Span, name: &str, recv: &Ty, verb: &str) -> Ty {
        self.err_coded(
            span,
            format!("cannot {verb} `{name}` of optional `{recv}`; use `?.` for null-safe access or unwrap with `!`"),
            "E-OPT-USE",
            Some(format!("`{name}` is only present when the receiver is non-null")),
        )
    }

    /// Wrap a member/method result in `Optional` for a `?.` access (a safe access yields a nullable
    /// result), without double-wrapping an already-optional member and leaving `Error` to cascade.
    pub(super) fn opt_wrap(t: Ty) -> Ty {
        match t {
            Ty::Error => Ty::Error,
            Ty::Optional(_) => t,
            other => Ty::Optional(Box::new(other)),
        }
    }
}

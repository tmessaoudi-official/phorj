//! Expression checking — dispatch + construction (`new`).

use super::*;

impl Checker {
    // ---- expressions ----
    /// Depth-guarded entry to expression checking. Every recursive descent (`check_binary`,
    /// `check_call`, … all call back through here) is bounded by [`MAX_EXPR_DEPTH`], so a
    /// pathologically deep AST faults cleanly instead of overflowing the walker's stack. `depth`
    /// is balanced on every path (the result is captured before the decrement).
    pub(in crate::checker) fn check_expr(&mut self, expr: &crate::ast::Expr) -> Ty {
        self.depth += 1;
        let ty = if self.depth > MAX_EXPR_DEPTH {
            self.err(
                Self::expr_span(expr),
                format!("expression nests too deeply (limit {MAX_EXPR_DEPTH})"),
            )
        } else {
            self.check_expr_inner(expr)
        };
        // S2.1-broad: record the reified operand type of a call / method-call / field-read / index whose
        // result is concrete, so the VM compiler can specialize `box.get() + 1`, `box.value + 1`,
        // `xs[i] + 1` as the operand the checker proved (the static shape may erase to `mixed`). Keyed by
        // the node's own span; only `Call`/`Member`/`Index` (the shapes `ctype` can fail to specialize)
        // and only when the type carries operand information (not `Error`/`Void`/a type parameter), so
        // the map stays small. Non-operand types (e.g. a function-returning call) are dropped at the
        // compile boundary by `ty_to_cty`, so this never overrides `ctype`'s fn-value/class resolution.
        if matches!(
            expr,
            crate::ast::Expr::Call { .. }
                | crate::ast::Expr::Member { .. }
                | crate::ast::Expr::Index { .. }
        ) && Self::is_reifiable_operand(&ty)
        {
            self.reified_operands
                .insert(Self::expr_span(expr).start, ty.clone());
        }
        self.depth -= 1;
        ty
    }

    /// Whether `ty` carries operand information worth recording for the VM compiler's `ctype` (S2.1-broad)
    /// — a concrete scalar/container/instance whose erased static shape would otherwise collapse to
    /// `CTy::Other`. Excludes `Error`/`Void`/`Unit`/`Never`/`Null`/a bare type parameter (`Ty::Param`),
    /// which carry no useful operand type.
    pub(in crate::checker) fn is_reifiable_operand(ty: &Ty) -> bool {
        matches!(
            ty,
            Ty::Int
                | Ty::Float
                | Ty::Decimal
                | Ty::String
                | Ty::Bool
                | Ty::List(_)
                | Ty::Map(_, _)
                | Ty::Named(_, _)
                | Ty::Optional(_)
        )
    }

    pub(in crate::checker) fn check_expr_inner(&mut self, expr: &crate::ast::Expr) -> Ty {
        use crate::ast::Expr;
        match expr {
            Expr::Int(_, _) => Ty::Int,
            Expr::Float(_, _) => Ty::Float,
            Expr::Decimal { .. } => Ty::Decimal,
            Expr::Bool(_, _) => Ty::Bool,
            Expr::Null(_) => Ty::Null,
            Expr::Str(parts, _) => self.check_str(parts), // Task 7
            Expr::Bytes(_, _) => Ty::Bytes,
            Expr::Ident(name, span) => match self.lookup(name) {
                Some(t) => t,
                None => {
                    // A4: bare named-function reference in value position — `fn_name` where
                    // `fn_name` is a top-level function, not a local. Return its function type so
                    // it can be passed as a first-class argument or stored in a variable.
                    if let Some(sigs) = self.funcs.get(name) {
                        // An overloaded function has no single first-class value (which overload?).
                        if sigs.len() > 1 {
                            return self.err_coded(
                                *span,
                                format!("`{name}` is overloaded — an overloaded function has no single first-class value"),
                                "E-OVERLOAD-FN-VALUE",
                                Some("call it directly, or wrap the intended overload in a lambda".into()),
                            );
                        }
                        let sig = &sigs[0];
                        let param_tys = sig.params.clone();
                        let ret_ty = sig.ret.clone();
                        return Ty::Function(param_tys, Box::new(ret_ty));
                    }
                    // A bare instance-field reference. Phorj requires `this.field` everywhere (like
                    // PHP's `$this->field`; no bare field access) — so this is always an error, with a
                    // distinct code per context: in a static method there is no instance at all
                    // (`E-STATIC-THIS`), otherwise the fix is to qualify it (`E-BARE-FIELD`).
                    if self.is_cur_field(name) {
                        if self.in_static_method {
                            return self.err_coded(
                                *span,
                                format!("instance field `{name}` is not accessible in a static method"),
                                "E-STATIC-THIS",
                                Some("a static method has no instance — pass the value as a parameter, use a static field, or make the method non-static".into()),
                            );
                        }
                        return self.err_coded(
                            *span,
                            format!("bare field reference `{name}` — write `this.{name}`"),
                            "E-BARE-FIELD",
                            Some(format!("Phorj has no bare field access (like PHP's `$this->`): qualify it as `this.{name}`")),
                        );
                    }
                    let cands = self.in_scope_names();
                    let hint = self
                        .nearest_name(name, &cands)
                        .map(|c| format!("did you mean `{c}`?"));
                    self.err_coded(
                        *span,
                        format!("unknown identifier `{name}`"),
                        "E-UNKNOWN-IDENT",
                        hint,
                    )
                }
            },
            Expr::This(span) if self.in_static_init => {
                self.err(*span, "`this` is not available in a static field initializer")
            }
            Expr::This(span) if self.in_static_method => self.err_coded(
                *span,
                "`this` is not available in a static method".to_string(),
                "E-STATIC-THIS",
                Some("a static method has no instance — access static members as `Class.member`, or make the method non-static".into()),
            ),
            Expr::This(span) => match &self.cur_class {
                // Inside a generic class body, `this` carries the class's own type parameters as
                // opaque `Ty::Param`s, so `this.value` (a `T` field) types as `T` and member access
                // substitutes identically (M-RT generics-all). Empty args for a non-generic class.
                Some(c) => {
                    let args = self
                        .cur_class_type_params
                        .iter()
                        .map(|p| Ty::Param(p.clone()))
                        .collect();
                    Ty::Named(c.clone(), args)
                }
                None => self.err(*span, "`this` is only valid inside a method"),
            },
            Expr::List(elems, span) => self.check_list(elems, *span), // Task 5
            Expr::Map(pairs, span) => self.check_map(pairs, *span),   // M-RT S3
            Expr::NewColl { kind, args, span } => self.check_new_coll(*kind, args, *span), // DEC-214

            Expr::Unary { op, expr, span } => self.check_unary(*op, expr, *span),
            Expr::Binary { op, lhs, rhs, span } => self.check_binary(*op, lhs, rhs, *span),
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => self.check_instanceof(value, type_name, *span),
            Expr::Cast {
                value,
                type_name,
                span,
            } => self.check_cast(value, type_name, *span),
            Expr::Call { callee, args, span } => self.check_call(callee, args, *span), // Task 4
            // `<Type>f(args)` — a return-type overload selector (M-RT Slice C1).
            Expr::OverloadSelect { ty, call, span } => self.check_overload_select(ty, call, *span),
            // `parent.m(args)` / `parent(A).m(args)` — super/parent dispatch (M-RT super/parent).
            Expr::ParentCall {
                ancestor,
                method,
                args,
                span,
            } => self.check_parent_call(ancestor.as_deref(), method, args, *span),
            Expr::New(inner, span) => self.check_new(inner, *span),
            Expr::Spawn { call, span } => self.check_spawn(call, *span),
            Expr::Member {
                object,
                name,
                safe,
                span,
            } => self.check_member(object, name, *safe, *span),
            Expr::Index {
                object,
                index,
                span,
            } => self.check_index(object, index, *span), // Task 5
            Expr::Force { inner, span } => self.check_force(inner, *span),
            // A `?` in general expression position. Throws-mode `?` (a throwing call) is valid here
            // and returns the call's normal type (M-faults 2b). Result-mode `?` is *not* — it is
            // restricted to a let-initializer (the one position with a clean PHP hoist; M-faults 2a):
            // flag it, but still check the inner to avoid a cascade.
            Expr::Propagate { inner, span } => match self.try_throws_propagate(inner, *span) {
                Some(crate::checker::throws::PropagateOutcome::Throws(t)) => t,
                other => {
                    // A non-throwing call (`Plain` — already checked) or a non-call operand
                    // (`None` — check it now): Result-mode `?` is invalid in this position.
                    if other.is_none() {
                        self.check_expr(inner);
                    }
                    self.err_coded(
                        *span,
                        "Result-mode `?` propagation is only allowed as the whole initializer of a `var`/typed binding",
                        "E-PROPAGATE-POSITION",
                        Some("bind the call's result to a local first (`var r = f(); …`), then handle it".into()),
                    )
                }
            },
            Expr::CloneWith {
                object,
                fields,
                span,
            } => self.check_clone_with(object, fields, *span),
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => self.check_match(scrutinee, arms, *span), // Task 8
            Expr::Range {
                start, end, span, ..
            } => self.check_range(start, end, *span),
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => self.check_if_expr(cond, then_expr, else_expr, *span),
            Expr::Lambda {
                params,
                ret,
                body,
                span,
            } => self.check_lambda(params, ret, body, *span),
            Expr::Html(parts, span) => self.check_html(parts, *span),
            // DI: `desugar_di` expands this away before the checker on the normal path. This arm is
            // only reached by the raw-checker path (LSP `diagnostics_for`, no desugar) — type it
            // gracefully as the target `T` (bare `inject()` has no static target here → `Ty::Error`,
            // which suppresses a cascade) rather than panicking.
            Expr::Inject { ty, .. } => match ty {
                Some(t) => self.resolve_type(t),
                None => Ty::Error,
            },
        }
    }

    /// `new <call>` (Feature C). Validates the inner is a class/enum-variant construction
    /// (`E-NEW-ON-NONCONSTRUCT` otherwise) and, when it is, type-checks it with the `under_new` flag so
    /// the construction does not also fire `E-NEW-REQUIRED`. Returns the construction's type (or the
    /// inner's type on the error path, to avoid a cascade). The node is later stripped by `unwrap_new`.
    pub(in crate::checker) fn check_new(&mut self, inner: &crate::ast::Expr, span: Span) -> Ty {
        use crate::ast::Expr;
        match inner {
            Expr::Call { callee, .. } if self.is_construction_callee(callee) => {
                // Batch A: a class constructor's visibility is enforced here (the 7th access site).
                // Enum-variant construction has no visibility, so gate only real class names.
                if let Expr::Ident(cname, _) = &**callee {
                    if self.classes.contains_key(cname) {
                        self.enforce_ctor_vis(cname, span);
                    }
                }
                self.under_new = true;
                let t = self.check_expr(inner);
                self.under_new = false; // defensive — the construction call already took it
                t
            }
            _ => {
                let t = self.check_expr(inner); // check normally to avoid a cascade
                self.err_coded(
                    span,
                    "`new` is only for constructing a class or enum variant".to_string(),
                    "E-NEW-ON-NONCONSTRUCT",
                    Some("call a function without `new`; `new` precedes a class/variant construction".into()),
                );
                t
            }
        }
    }

    /// Whether `callee` names a class or enum variant *constructor* (Feature C) — an unshadowed
    /// identifier that is a known class name or enum-variant name. A local binding of the same name
    /// shadows it (then it is a value, never a construction).
    pub(in crate::checker) fn is_construction_callee(&self, callee: &crate::ast::Expr) -> bool {
        match callee {
            crate::ast::Expr::Ident(name, _) => self.is_construction_name(name),
            // Qualified enum-variant construction `new Enum.Variant(…)` (slice A1): the callee is a
            // `Member` whose object is an (unshadowed) enum name and whose member is one of its
            // variants. Resolved + erased in `check_qualified_variant_call`.
            crate::ast::Expr::Member {
                object,
                name,
                safe: false,
                ..
            } => match &**object {
                crate::ast::Expr::Ident(en, _) => {
                    self.lookup(en).is_none()
                        && (self
                            .enums
                            .get(en)
                            .is_some_and(|info| info.variants.contains_key(name))
                            // S2: qualified injected-CLASS construction `new Http.Router(…)` /
                            // `new Time.Duration(…)` — the callee is an injected module qualifier + one
                            // of its injected classes. Erased to bare construction by `unwrap_new`.
                            || (super::enforce_injected::module_of(name) == Some(en.as_str())
                                && self.classes.contains_key(name)))
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Whether `name` is a class or enum-variant constructor not shadowed by a local binding.
    pub(in crate::checker) fn is_construction_name(&self, name: &str) -> bool {
        self.lookup(name).is_none()
            && (self.classes.contains_key(name)
                || self
                    .enums
                    .values()
                    .any(|info| info.variants.contains_key(name)))
    }
}

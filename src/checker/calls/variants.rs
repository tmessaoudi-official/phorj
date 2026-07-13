//! Call checking — variant/class construction and the concurrency call forms.

use super::*;

impl Checker {
    /// Returns `Some(ret)` if `name` is an enum variant or class constructor.
    pub(in crate::checker) fn try_variant_or_class_call(
        &mut self,
        name: &str,
        args: &[crate::ast::Expr],
        span: Span,
        skip_throws: bool,
    ) -> Option<Ty> {
        // enum variant constructor: find the (unique) enum that owns this variant name
        let owner = self
            .enums
            .iter()
            .find(|(_, info)| info.variants.contains_key(name))
            .map(|(enum_name, info)| {
                (
                    enum_name.clone(),
                    info.variants[name].clone(),
                    info.type_params.clone(),
                    info.injected,
                )
            });
        if let Some((enum_name, fields, type_params, injected)) = owner {
            // Variant-qualification B: a compiler-injected enum's variant must be constructed
            // *qualified* (`new Json.Object(…)`) — a bare `new Object(…)` is a name the user never
            // wrote, "in the wind" (DEC-020). Qualified construction routes through
            // `check_qualified_variant_call`, never here, so reaching this point bare is the error.
            if injected {
                self.err_coded(
                    span,
                    format!("`{name}` is a variant of the injected enum `{enum_name}` — construct it qualified"),
                    "E-INJECTED-VARIANT-BARE",
                    Some(format!("write `new {enum_name}.{name}(…)`")),
                );
            }
            return Some(self.type_variant_construction(
                &enum_name,
                name,
                &fields,
                &type_params,
                args,
                span,
            ));
        }
        // class constructor: `ClassName(args)`
        if let Some(info) = self.classes.get(name) {
            let ctor = info.ctor.clone();
            let ctor_throws = info.ctor_throws.clone();
            let type_params = info.type_params.clone();
            let is_abstract = info.is_abstract;
            // DEC-221: a throwing constructor makes `new X(args)` a throwing expression. Route each
            // declared throw exactly like a free/method call — a bare construction must be caught by an
            // enclosing `try` (else `E-CALL-UNHANDLED`); under `?`-propagation the set is collected.
            // Routed here (before the generic/non-generic split) so a generic throwing ctor is covered too.
            for e in &ctor_throws {
                self.route_call_throw(skip_throws, name, e, span);
            }
            // M-RT S6b: an abstract class has unimplemented methods and cannot be instantiated.
            if is_abstract {
                self.err_coded(
                    span,
                    format!("cannot instantiate abstract class `{name}`"),
                    "E-ABSTRACT-INSTANTIATE",
                    Some(format!(
                        "`{name}` is `abstract`; instantiate a concrete subclass that implements its \
                         abstract methods"
                    )),
                );
            }
            if type_params.is_empty() {
                self.check_args(name, &ctor, args, span);
                return Some(Ty::Named(name.to_string(), Vec::new()));
            }
            // A generic class: infer its type arguments from the constructor call (M-RT generics-all),
            // the same first-binding-wins unifier as a generic function. A parameter the constructor
            // does not mention stays un-inferred and defaults to `Ty::Error` (permissive).
            if ctor.len() != args.len() {
                self.err(
                    span,
                    format!(
                        "`{name}` expects {} argument(s), found {}",
                        ctor.len(),
                        args.len()
                    ),
                );
                for a in args {
                    self.check_expr(a);
                }
                return Some(Ty::Named(
                    name.to_string(),
                    vec![Ty::Error; type_params.len()],
                ));
            }
            let mut theta: HashMap<String, Ty> = HashMap::new();
            for (param, arg) in ctor.iter().zip(args) {
                let at = self.check_arg(arg, param);
                if !self.unify(param, &at, &mut theta) {
                    let want = apply_subst(param, &theta);
                    self.err(
                        span,
                        format!("`{name}` constructor expects `{want}`, found `{at}`"),
                    );
                }
            }
            let inst_args = type_params
                .iter()
                .map(|p| theta.get(p).cloned().unwrap_or(Ty::Error))
                .collect();
            return Some(Ty::Named(name.to_string(), inst_args));
        }
        None
    }

    /// Shared enum-variant construction typing — both the bare `Variant(args)` path
    /// ([`Self::try_variant_or_class_call`]) and the qualified `Enum.Variant(args)` path
    /// ([`Self::check_qualified_variant_call`]) route here. Checks the args against the variant's field
    /// types, inferring a generic enum's type arguments (first-binding-wins), and returns the enum type.
    pub(in crate::checker) fn type_variant_construction(
        &mut self,
        enum_name: &str,
        variant: &str,
        fields: &[Ty],
        type_params: &[String],
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        if type_params.is_empty() {
            self.check_args(variant, fields, args, span);
            return Ty::Named(enum_name.to_string(), Vec::new());
        }
        if fields.len() != args.len() {
            self.err(
                span,
                format!(
                    "variant `{variant}` expects {} argument(s), found {}",
                    fields.len(),
                    args.len()
                ),
            );
            for a in args {
                self.check_expr(a);
            }
            return Ty::Named(enum_name.to_string(), vec![Ty::Error; type_params.len()]);
        }
        let mut theta: HashMap<String, Ty> = HashMap::new();
        for (field, arg) in fields.iter().zip(args) {
            let at = self.check_arg(arg, field);
            if !self.unify(field, &at, &mut theta) {
                let want = apply_subst(field, &theta);
                self.err(
                    span,
                    format!("variant `{variant}` expects `{want}`, found `{at}`"),
                );
            }
        }
        let inst_args = type_params
            .iter()
            .map(|p| theta.get(p).cloned().unwrap_or(Ty::Error))
            .collect();
        Ty::Named(enum_name.to_string(), inst_args)
    }

    /// Qualified enum-variant construction `new Enum.Variant(args)` (injected-enum qualification,
    /// slice A1). The caller has confirmed `enum_name` is a known enum. Validates the variant belongs
    /// to it, types the construction via [`Self::type_variant_construction`], and records an *erase*
    /// substitution (`Enum.Variant(args)` → bare `Variant(args)`) into `ufcs_resolutions` — the same
    /// "compile-time sugar erased before any backend" discipline as UFCS/casts (Invariant 5): every
    /// backend and the transpiler see the ordinary bare construction, so byte-identity is unchanged.
    pub(in crate::checker) fn check_qualified_variant_call(
        &mut self,
        enum_name: &str,
        variant: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        // Take the `new`-prefix flag before checking args (a construction argument still needs its own
        // `new`). A qualified construction reached without `new` is `E-NEW-REQUIRED`, exactly like the
        // bare form (DEC-083 mandatory `new`).
        let was_new = std::mem::take(&mut self.under_new);
        let info = &self.enums[enum_name];
        let Some(fields) = info.variants.get(variant).cloned() else {
            for a in args {
                self.check_expr(a);
            }
            return self.err(
                span,
                format!("enum `{enum_name}` has no variant `{variant}`"),
            );
        };
        let type_params = info.type_params.clone();
        if !was_new {
            self.err_coded(
                span,
                format!("construct `{enum_name}.{variant}` with `new {enum_name}.{variant}(…)`"),
                "E-NEW-REQUIRED",
                Some(format!("write `new {enum_name}.{variant}(…)`")),
            );
        }
        // The qualifier is erased to the bare `Variant(args)` construction by `unwrap_new`, which
        // strips the enclosing `new` and rewrites the `Member` callee to the bare variant *after* its
        // recursion has unwrapped any nested `new` in the args — so no stale `New` survives (a
        // post-`unwrap_new` side-table rewrite re-embeds check-time args and would leak nested `New`s).
        self.type_variant_construction(enum_name, variant, &fields, &type_params, args, span)
    }

    /// `spawn <call>` — type a green-task spawn (M6 W4 / S4.3). The operand must be a call; its return
    /// type `T` becomes the handle type `Task<T>`. A `void`/`never` call can't be a task payload this
    /// slice (the `join` result would be uncapturable) — `E-SPAWN-VOID`. The result types as
    /// `Ty::Named("Task", [T])`, the same nominal shape `resolve_type` produces for a `Task<T>`
    /// annotation, so `Task<int> t = spawn f();` and `var t = spawn f();` both check.
    pub(in crate::checker) fn check_spawn(&mut self, call: &crate::ast::Expr, span: Span) -> Ty {
        if !matches!(call, crate::ast::Expr::Call { .. }) {
            let _ = self.check_expr(call); // surface nested errors
            return self.err_coded(
                span,
                "`spawn` must be applied to a function or method call, e.g. `spawn work(x)`",
                "E-SPAWN-NOT-CALL",
                Some("`spawn` starts a task from a call; it cannot wrap a plain value".into()),
            );
        }
        match self.check_expr(call) {
            Ty::Error => Ty::Error,
            t @ (Ty::Void | Ty::Never) => self.err_coded(
                span,
                format!("a `spawn`ned call must return a value, found `{t}`"),
                "E-SPAWN-VOID",
                Some(
                    "spawn a call that returns a value; fire-and-forget void tasks are a follow-up"
                        .into(),
                ),
            ),
            t => Ty::Named("Task".into(), vec![t]),
        }
    }

    /// `true` if `e` is exactly the built-in channel constructor `Channel.new()` (M6 W4). Used by the
    /// `VarDecl` checker to type it from the binding's `Channel<T>` annotation (the static call has no
    /// argument to infer `T` from).
    pub(in crate::checker) fn is_channel_new(e: &crate::ast::Expr) -> bool {
        matches!(
            e,
            crate::ast::Expr::Call { callee, .. }
                if matches!(&**callee, crate::ast::Expr::Member { object, name, safe: false, .. }
                    if name == "create" && matches!(&**object, crate::ast::Expr::Ident(h, _) if h == "Channel"))
        )
    }

    /// Static call on a built-in concurrency type — `Channel.new()` / (no `Task` statics yet). Reached
    /// only when `Channel.new()` is used **without** a `Channel<T>` binding context (e.g. `return
    /// Channel.new();`): with no argument to infer the element type, an explicit annotation is required
    /// (`E-CHANNEL-ANNOTATION`). The annotated-`VarDecl` path types it directly and never funnels here.
    pub(in crate::checker) fn check_concurrency_static(
        &mut self,
        head: &str,
        name: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        for a in args {
            self.check_expr(a);
        }
        match (head, name) {
            ("Channel", "create") => self.err_coded(
                span,
                "`Channel.create()` needs a `Channel<T>` annotation to fix its element type".to_string(),
                "E-CHANNEL-ANNOTATION",
                Some("bind it to an annotated local first, e.g. `Channel<int> ch = Channel.create();`".into()),
            ),
            ("Channel", other) => self.err_coded(
                span,
                format!("`Channel` has no static method `{other}` — did you mean `Channel.create()`?"),
                "E-CONCURRENCY-METHOD",
                None,
            ),
            (_, other) => self.err_coded(
                span,
                format!("`{head}` has no static method `{other}`"),
                "E-CONCURRENCY-METHOD",
                None,
            ),
        }
    }

    /// Built-in method dispatch for the concurrency handle types (M6 W4): `Channel<T>` `send`/`recv`,
    /// `Task<T>` `join`. `cargs[0]` is the element type `T`. Returns `None` if `name` is not a known
    /// built-in method, so the caller falls through to ordinary class-method dispatch.
    pub(in crate::checker) fn check_concurrency_method(
        &mut self,
        head: &str,
        elem: &Ty,
        name: &str,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Option<Ty> {
        match (head, name) {
            ("Channel", "send") => {
                if args.len() != 1 {
                    return Some(self.err_coded(
                        span,
                        format!("`send` takes exactly one argument, got {}", args.len()),
                        "E-CONCURRENCY-ARITY",
                        None,
                    ));
                }
                let at = self.check_expr(&args[0]);
                if !self.ty_assignable(&at, elem) {
                    self.err_assign(span, &at, elem);
                }
                Some(Ty::Void)
            }
            ("Channel", "receive") => {
                if !args.is_empty() {
                    return Some(self.err_coded(
                        span,
                        "`receive` takes no arguments".to_string(),
                        "E-CONCURRENCY-ARITY",
                        None,
                    ));
                }
                Some(elem.clone())
            }
            ("Task", "join") => {
                if !args.is_empty() {
                    return Some(self.err_coded(
                        span,
                        "`join` takes no arguments".to_string(),
                        "E-CONCURRENCY-ARITY",
                        None,
                    ));
                }
                Some(elem.clone())
            }
            _ => {
                for a in args {
                    self.check_expr(a);
                }
                Some(self.err_coded(
                    span,
                    format!("`{head}<T>` has no method `{name}`"),
                    "E-CONCURRENCY-METHOD",
                    None,
                ))
            }
        }
    }
}

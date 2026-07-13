//! Call checking — free-function and named calls, bare-fn imports, `String.format`
//! directive analysis.

use super::*;

impl Checker {
    pub(in crate::checker) fn check_call(
        &mut self,
        callee: &crate::ast::Expr,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        use crate::ast::Expr;
        match callee {
            Expr::Ident(name, _) => {
                // Built-in fault intrinsics (M-faults 2a): `panic`/`todo`/`unreachable` (→ `never`) and
                // `assert` (→ `unit`). Recognized here before any user-function lookup; the names are
                // reserved (`E-RESERVED-INTRINSIC`) so this can't be shadowed.
                if let Some(t) = self.check_intrinsic_call(name, args, span) {
                    return t;
                }
                // If the name is a local (or a `match`-arm binding) with function type, treat it
                // as a function-value call rather than a named-function call — the latter only
                // looks in `self.funcs` (top-level declarations) and would report "unknown
                // function `name`" for a lambda-typed local (M3 S3 Task 4).
                if let Some(Ty::Function(param_tys, ret_ty)) = self.lookup(name) {
                    self.check_args("<lambda>", &param_tys, args, span);
                    return *ret_ty;
                }
                let ty = self.check_named_call(name, args, span);
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
                            // `Reflect.typeName(x)` is resolved from `x`'s STATIC type and erased
                            // before any backend (Core.Reflect, Tier 3) — never the generic-native
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
                            self.require_option_for_result_bridge(n.module, n.name, span);
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
                        if self.lookup_binding(cls).is_none() && self.classes.contains_key(cls) {
                            return self.check_static_method_call(cls, name, args, span);
                        }
                        // Built-in concurrency static — `Channel.new()` (M6 W4). `Channel`/`Task` are
                        // reserved built-in type names (not user classes), so route them before the
                        // instance-method fallthrough (which would type `Channel` as an unknown value).
                        if (cls == "Channel" || cls == "Task") && self.lookup_binding(cls).is_none()
                        {
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
                            if let Some(t) = self.try_variant_or_class_call(name, args, span) {
                                return t;
                            }
                        }
                        // Qualified enum-variant construction `Enum.Variant(args)` (slice A1): the head
                        // is an enum *name* (not a value binding). Resolves after native/class/concurrency
                        // (an import or class of the same name wins), before instance-method dispatch.
                        // Erased to the bare variant call before any backend (see
                        // `check_qualified_variant_call`).
                        if self.lookup_binding(cls).is_none() && self.enums.contains_key(cls) {
                            return self.check_qualified_variant_call(cls, name, args, span);
                        }
                    }
                }
                self.check_method_call(object, name, args, *safe, span)
            }
            other => {
                // Evaluate the callee to see if it is a function value (closure or named-fn ref).
                let callee_ty = self.check_expr(other);
                match callee_ty {
                    Ty::Function(param_tys, ret_ty) => {
                        self.check_args("<lambda>", &param_tys, args, span);
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
        if let Some(t) = self.try_variant_or_class_call(name, args, span) {
            return t;
        }
        let sigs = match self.funcs.get(name) {
            Some(s) => s.clone(),
            None => {
                // DEC-197: a bare call to a member-imported module function (`import Core.Output.printLine;`
                // ⇒ `printLine(x)`). Resolved AFTER user functions, so `local > user fn > imported native`
                // holds (locals are handled earlier in `check_call`, user funcs in the `Some` arm above).
                if let Some((module, real)) = self.fn_imports.get(name).cloned() {
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
                self.check_args_defaulted(name, &sig.params, &sig.defaults, args, span);
                sig.ret.clone()
            } else {
                self.check_generic_call(name, &sig.params, &sig.ret, args, span)
            };
        }
        // Overload set (M-RT): generic members were rejected at collection, so every overload is
        // monomorphic. The call's result is the shared return type (`E-OVERLOAD-RETURN`); resolution
        // here is *static* (for typing) — the runtime dispatch is byte-identical by construction.
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
            span,
        };
        self.ufcs_resolutions.insert(span.start, qualified);
        ret
    }

    /// W3-5 / DEC-199 slice 1: type-check `String.format(spec, args)` — a real `%`-sprintf native
    /// (`text_format` / `__phorj_format`), NOT a desugar. Validates arg 0 is a `string` and arg 1 a
    /// list, then — for a LITERAL spec — gates the directive set at compile time (`%s`/`%d`/`%%` this
    /// slice; anything else, incl. width/precision/flags/`%f`/`%N$`, is `E-FORMAT-UNSUPPORTED`) and,
    /// when the values are a list literal, checks the directive count (`E-FORMAT-ARG-COUNT`). A runtime
    /// spec / non-literal list is left to the strict runtime renderer (which faults on a bad directive,
    /// a `%d` type mismatch, or a count mismatch). Returns `Ty::String`.
    pub(in crate::checker) fn check_string_format(
        &mut self,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        use crate::ast::{Expr, StrPart};
        if args.len() != 2 {
            return self.err_coded(
                span,
                format!(
                    "`String.format` expects 2 arguments (a format string and a list of values), found {}",
                    args.len()
                ),
                "E-FORMAT-ARGS",
                Some("call it as `String.format(\"%s = %d\", [name, count])`".into()),
            );
        }
        let spec_ty = self.check_expr(&args[0]);
        if !matches!(spec_ty, Ty::String | Ty::Error) {
            self.err_coded(
                Self::expr_span(&args[0]),
                format!("`String.format`'s format string must be a `string`, found `{spec_ty}`"),
                "E-FORMAT-SPEC-TYPE",
                None,
            );
        }
        // Values: a LIST LITERAL may be heterogeneous printable scalars (`["Ada", 3, 50]` — `%s`/`%d`
        // consume them by position), so check each element individually rather than as a homogeneous
        // `List<T>` (which would reject the mix / an empty `[]`). A non-literal list arg (a `List<T>`
        // variable) is accepted by its list type; the strict runtime renderer enforces per-directive
        // element types (`%d` needs an int) with clean faults.
        let scalar_ok = |t: &Ty| {
            matches!(
                t,
                Ty::Int | Ty::Float | Ty::Decimal | Ty::Bool | Ty::String | Ty::Error
            )
        };
        match &args[1] {
            Expr::List(items, _) => {
                for it in items {
                    let t = self.check_expr(it);
                    if !scalar_ok(&t) {
                        self.err_coded(
                            Self::expr_span(it),
                            format!(
                                "`String.format` values must be printable scalars, found `{t}`"
                            ),
                            "E-FORMAT-ARG-TYPE",
                            Some("`%s`/`%d` format `int`/`float`/`decimal`/`bool`/`string`".into()),
                        );
                    }
                }
            }
            other => {
                let t = self.check_expr(other);
                if !matches!(t, Ty::List(_) | Ty::FixedList(..) | Ty::Error) {
                    self.err_coded(
                        Self::expr_span(other),
                        format!("`String.format`'s values must be a list, found `{t}`"),
                        "E-FORMAT-ARGS-TYPE",
                        Some("pass the values as a list — `String.format(\"%s\", [x])`".into()),
                    );
                }
            }
        }
        // Compile-time gate for a LITERAL spec (the common case): only `%s`/`%d`/`%%` this slice, and
        // (against a list literal) an exact directive/value count. A runtime spec is validated at runtime.
        if let Expr::Str(parts, _) = &args[0] {
            if parts.iter().all(|p| matches!(p, StrPart::Literal(_))) {
                let spec: String = parts
                    .iter()
                    .map(|p| match p {
                        StrPart::Literal(s) => s.as_str(),
                        StrPart::Expr(_) => "",
                    })
                    .collect();
                match analyze_format_directives(&spec) {
                    Ok(info) => {
                        if let Expr::List(items, _) = &args[1] {
                            let len = items.len();
                            if info.positional && info.sequential {
                                self.err_coded(
                                    span,
                                    "`String.format` cannot mix positional (`%N$`) and sequential directives in one spec".to_string(),
                                    "E-FORMAT-MIXED-POSITIONAL",
                                    Some("use all-positional (`%1$s %2$s`) or all-sequential (`%s %s`), not both".into()),
                                );
                            } else if info.positional {
                                // Positional: reuse + reorder allowed, but every value must be referenced
                                // and no index may exceed the value count.
                                if info.max_arg > len {
                                    self.err_coded(
                                        span,
                                        format!("`String.format` references `%{}$` but was given only {len} value(s)", info.max_arg),
                                        "E-FORMAT-ARG-COUNT",
                                        Some("a positional index must be between 1 and the number of values".into()),
                                    );
                                } else if info.referenced.len() != len {
                                    let unused = (1..=len)
                                        .find(|k| !info.referenced.contains(k))
                                        .unwrap_or(len);
                                    self.err_coded(
                                        span,
                                        format!("`String.format` never references value {unused} of {len} (every value must be used)"),
                                        "E-FORMAT-ARG-COUNT",
                                        Some("reference every value with a `%N$` (reuse/reorder is allowed)".into()),
                                    );
                                }
                            } else if info.seq_count != len {
                                self.err_coded(
                                    span,
                                    format!(
                                        "`String.format` uses {} directive(s) but was given {len} value(s)",
                                        info.seq_count
                                    ),
                                    "E-FORMAT-ARG-COUNT",
                                    Some("give exactly one value per `%s`/`%d` (use `%%` for a literal `%`)".into()),
                                );
                            }
                        }
                    }
                    Err(bad) => {
                        self.err_coded(
                            span,
                            bad,
                            "E-FORMAT-UNSUPPORTED",
                            Some(
                                "this version supports `%s`/`%d`/`%f`/`%%`, scientific `%e`/`%E`, shortest-repr \
                                 `%g`/`%G`, integer-radix `%x`/`%X`/`%o`/`%b`, `%N$` positional, flags `-`/`0`/`+`, \
                                 width, precision on `%s` (truncate) and the float conversions. Precision on `%d` is \
                                 deliberately unsupported (PHP silently ignores it)"
                                    .into(),
                            ),
                        );
                    }
                }
            }
        }
        Ty::String
    }
}

/// Structured analysis of a LITERAL `String.format` spec — how many sequential directives, whether any
/// positional (`%N$`) directives appear, the highest explicit index, and the set of referenced indices.
/// Lets `check_string_format` validate the value count against BOTH the sequential and the positional
/// (reuse/reorder/no-unused) rules.
#[derive(Default)]
pub(in crate::checker) struct FormatSpecInfo {
    seq_count: usize,
    positional: bool,
    sequential: bool,
    max_arg: usize,
    referenced: std::collections::BTreeSet<usize>,
}

/// W3-5/DEC-199: scan a LITERAL `String.format` spec (`%%` is a literal `%`), returning its
/// [`FormatSpecInfo`] or the first unsupported-directive message for `E-FORMAT-UNSUPPORTED`. Uses the
/// SAME [`crate::native::parse_format_directive`] the runtime renderer uses, so the compile-time gate and
/// `text_format` accept exactly the same specs.
pub(in crate::checker) fn analyze_format_directives(spec: &str) -> Result<FormatSpecInfo, String> {
    let mut info = FormatSpecInfo::default();
    let mut chars = spec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            continue;
        }
        let d = crate::native::parse_format_directive(&mut chars)?;
        match d.arg {
            Some(n) => {
                info.positional = true;
                info.max_arg = info.max_arg.max(n);
                info.referenced.insert(n);
            }
            None => {
                info.sequential = true;
                info.seq_count += 1;
            }
        }
    }
    Ok(info)
}

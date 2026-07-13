//! Call checking — native calls, argument checking, default-parameter fill.

use super::*;

impl Checker {
    /// `console.println(args)` — a namespaced native call resolved through the import map (M3
    /// Wave 1). The native single-sources its signature, so checking is the same arg/arity pass as a
    /// free function; the leaf-qualified label (`console.println`) drives the error messages.
    pub(in crate::checker) fn check_native_call(
        &mut self,
        idx: usize,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        let n = &crate::native::registry()[idx];
        let leaf = n.module.rsplit('.').next().unwrap_or(n.module);
        let label = format!("{leaf}.{}", n.name);
        // W-DEPRECATED (rock 3): a deprecated stdlib symbol keeps working but warns, naming its
        // replacement + removal version (a non-fatal lint on the warning channel). The flag lives in
        // the `deprecation_of` side table — empty in a release build — so this is a no-op for every
        // current native. `n` borrows the `'static` registry, so the `&mut self` lint doesn't alias.
        if let Some(dep) = crate::native::deprecation_of(n.module, n.name) {
            self.warn_coded(
                span,
                format!(
                    "`{label}` is deprecated and will be removed in {}",
                    dep.removed_in
                ),
                "W-DEPRECATED",
                Some(format!("use {} instead", dep.replacement)),
            );
        }
        // W-SECRET (Fork B): a freshly-`expose()`d `Secret` flowing *directly* into a sink is almost
        // certainly a leak (the plaintext gets logged / persisted). Syntactic on the direct argument —
        // a value laundered through a local is not flagged (full taint analysis is out of scope; the
        // type-system non-printability of `Secret` is the real guarantee). Sinks: Output.printLine/print,
        // File.write. `n` borrows the `'static` registry, so the `&mut self` lint call does not alias.
        if matches!(
            (n.module, n.name),
            ("Core.Output", "printLine") | ("Core.Output", "print") | ("Core.File", "write")
        ) {
            for a in args {
                if self.arg_is_secret_expose(a) {
                    self.warn_coded(
                        span,
                        "exposing a Secret directly into a sink — the plaintext will be logged or persisted",
                        "W-SECRET",
                        Some("bind the exposed value and use it deliberately, or avoid sending a secret's plaintext to a sink".into()),
                    );
                }
            }
        }
        // A native whose stored signature carries a type parameter (`Map.keys(Map<K,V>) -> List<K>`,
        // `List.reverse(List<T>) -> List<T>`) is checked exactly like a generic free function: unify
        // the declared params against the argument types, then substitute into the return (M-RT S7b).
        // `θ` lives only in `check_generic_call`; the native's `Ty::Param` is registry-only and never
        // reaches a backend (the compiler types a native call by expression shape → `CTy::Other`, and
        // the transpiler emits via the `php` closure). `n` borrows the `'static` registry, so passing
        // `&n.params`/`&n.ret` alongside `&mut self` does not alias.
        if n.params.iter().any(ty_has_param) || ty_has_param(&n.ret) {
            self.check_generic_call(&label, &n.params, &n.ret, &[], args, span)
        } else {
            // M4: a native may declare defaults for trailing params (e.g. `parseFloat(string, bool
            // permissive = false)`). Build the parallel `Option<Expr>` list from `native_defaults` —
            // `None` for the leading required params, then the trailing default literals — and run the
            // defaulted-arity check (a native with no defaults gets all-`None` ⇒ exact arity).
            let nd = crate::native::native_defaults(n.module, n.name);
            let ret = n.ret.clone();
            let params = n.params.clone();
            let mut defaults: Vec<Option<crate::ast::Expr>> = vec![None; params.len()];
            for (off, d) in nd.iter().enumerate() {
                let idx = params.len() - nd.len() + off;
                defaults[idx] = Some(native_default_expr(*d, span));
            }
            self.check_args_defaulted(&label, &params, &defaults, args, span);
            ret
        }
    }

    /// W-SECRET helper (Fork B): is `arg` syntactically `<recv>.expose()` where `recv: Secret<_>`?
    /// Matches the method-call shape (`Call` whose callee is a `Member` named `expose`, no args) and
    /// confirms the receiver's type. Re-checking the receiver is side-effect-free for the common case
    /// (a local var read) and span-keyed records are idempotent, so the later full arg-check is safe.
    pub(in crate::checker) fn arg_is_secret_expose(&mut self, arg: &crate::ast::Expr) -> bool {
        if let crate::ast::Expr::Call { callee, args, .. } = arg {
            if args.is_empty() {
                if let crate::ast::Expr::Member { object, name, .. } = callee.as_ref() {
                    if name == "expose" {
                        return matches!(self.check_expr(object), Ty::Named(n, _) if n == "Secret");
                    }
                }
            }
        }
        false
    }

    /// Check a single call argument against its expected parameter type. Identical to `check_expr`
    /// except that an **empty list literal** `[]` — which has no element to infer a type from —
    /// adopts the expected `List<T>` element type instead of erroring with "cannot infer element
    /// type". This is the one place an expected type is threaded into expression checking
    /// (bidirectional, call-argument-only by design); an empty `[]` in any other position (a
    /// declaration initializer, a `return`) still requires a non-empty literal. It lets the
    /// zero-attribute / zero-child HTML builders read naturally — `el("p", [], [text("hi")])`.
    pub(in crate::checker) fn check_arg(&mut self, arg: &crate::ast::Expr, expected: &Ty) -> Ty {
        // An EMPTY `[]` against any `List<T>` param (concrete OR generic `T`) types as that list — the
        // original behavior, which lets a generic callee's unifier bind `T` from the other args
        // (`SomeGeneric.of([])`). Must come first: the `ty_has_param` guard below would otherwise drop
        // an empty-`[]`→`List<T>` arg to `check_expr`, which cannot infer an empty literal's element.
        if let crate::ast::Expr::List(elems, _) = arg {
            if elems.is_empty() {
                if let Ty::List(inner) = expected {
                    return Ty::List(inner.clone());
                }
            }
        }
        // Thread a NON-empty list/map LITERAL against a CONCRETE collection type — so `f([1, "x"])`
        // checks against a `List<int | string>` param (each element against the union), exactly like
        // the declaration-initializer / return-position threading (UA-1.6 / DEC-178, Wave C
        // foundation). Checker-only: the runtime value is already a `List`/`Map`, so every backend is
        // unchanged (byte-identity-safe).
        //
        // CRITICAL: thread ONLY a CONCRETE expected type. A generic callee's param is `List<T>` with
        // `T` an unbound `Ty::Param` — `thread_literal_expected` would still match `Ty::List(_)` and
        // wrongly check each element against the unbound `T` (breaking `Set.of([1,2,3])`). The
        // `ty_has_param` guard leaves every generic callee on the existing unify path (deferred).
        if !ty_has_param(expected) {
            if let Some(t) = self.thread_literal_expected(arg, expected) {
                return t;
            }
        }
        self.check_expr(arg)
    }

    /// M4 default parameters: consume a `pending_fill` (set by `check_named_call`/`check_native_call`
    /// when a call legally omitted trailing defaulted args) and record the full replacement `Call`
    /// (provided args + appended default literals) in `default_fills`, keyed by the call span. The
    /// replacement is applied by `rewrite_ufcs` like any other call rewrite, so every backend sees a
    /// full-arity call (byte-identity-safe — the default literal is identical on all three).
    pub(in crate::checker) fn record_pending_fill(
        &mut self,
        callee: &crate::ast::Expr,
        args: &[crate::ast::Expr],
        span: Span,
    ) {
        if let Some(defaults) = self.pending_fill.take() {
            let mut full: Vec<crate::ast::Expr> = args.to_vec();
            full.extend(defaults);
            self.default_fills.insert(
                span.start,
                crate::ast::Expr::Call {
                    callee: Box::new(callee.clone()),
                    args: full,
                    span,
                },
            );
        }
    }

    /// M4 default parameters: type-check a call that may omit trailing defaulted arguments. `defaults`
    /// is parallel to `params` (`Some(literal)` for a defaulted param). The required arity is the count
    /// of leading non-default params. A call with `args.len()` in `[required, params.len()]` is valid:
    /// the provided args are checked against their params, and if any trailing params were omitted the
    /// caller is told (via `pending_fill`) to append their default literals. Outside that range, the
    /// usual "expects N argument(s)" error fires. Returns nothing (the caller owns the return type).
    pub(in crate::checker) fn check_args_defaulted(
        &mut self,
        name: &str,
        params: &[Ty],
        defaults: &[Option<crate::ast::Expr>],
        args: &[crate::ast::Expr],
        span: Span,
    ) {
        let required = defaults.iter().take_while(|d| d.is_none()).count();
        if args.len() < required || args.len() > params.len() {
            let detail = if required == params.len() {
                format!("{}", params.len())
            } else {
                format!("{required} to {}", params.len())
            };
            self.err(
                span,
                format!(
                    "`{name}` expects {detail} argument(s), found {}",
                    args.len()
                ),
            );
            for a in args {
                self.check_expr(a);
            }
            return;
        }
        // Type-check the provided args against their params (the omitted defaults were validated at
        // the signature, so they need no re-check here).
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let at = self.check_arg(arg, param);
            if !self.ty_assignable(&at, param) {
                self.err(
                    span,
                    format!(
                        "`{name}` argument {} expects `{param}`, found `{at}`",
                        i + 1
                    ),
                );
            }
        }
        // Record the trailing defaults to append (only when some were omitted).
        if args.len() < params.len() {
            let fill: Vec<crate::ast::Expr> = defaults[args.len()..]
                .iter()
                .map(|d| d.clone().expect("trailing params are defaulted"))
                .collect();
            self.pending_fill = Some(fill);
        }
    }

    /// Check call arguments against expected parameter types.
    pub(in crate::checker) fn check_args(
        &mut self,
        name: &str,
        params: &[Ty],
        args: &[crate::ast::Expr],
        span: Span,
    ) {
        if params.len() != args.len() {
            self.err(
                span,
                format!(
                    "`{name}` expects {} argument(s), found {}",
                    params.len(),
                    args.len()
                ),
            );
            for a in args {
                self.check_expr(a);
            }
            return;
        }
        for (i, (param, arg)) in params.iter().zip(args).enumerate() {
            let at = self.check_arg(arg, param);
            if !self.ty_assignable(&at, param) {
                self.err(
                    span,
                    format!(
                        "`{name}` argument {} expects `{param}`, found `{at}`",
                        i + 1
                    ),
                );
            }
        }
    }
}

/// Convert a native parameter's [`NativeDefault`] literal to the equivalent `Expr` literal, used when
/// filling an omitted trailing native argument (M4 default parameters). The span is the call site's
/// (the synthesized literal needs *a* span; it is never re-matched by the call-rewrite pass since its
/// offset differs from the call key only conceptually — it carries no nested sugar regardless).
pub(in crate::checker) fn native_default_expr(
    d: crate::native::NativeDefault,
    span: Span,
) -> crate::ast::Expr {
    use crate::ast::{Expr, StrPart};
    use crate::native::NativeDefault as D;
    match d {
        D::Bool(b) => Expr::Bool(b, span),
        D::Int(n) => Expr::Int(n, span),
        D::Float(f) => Expr::Float(f, span),
        D::Str(s) => Expr::Str(vec![StrPart::Literal(s.to_string())], span),
        D::Null => Expr::Null(span),
    }
}

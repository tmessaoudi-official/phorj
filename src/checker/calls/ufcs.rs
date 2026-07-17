//! Call checking — UFCS navigation and resolution.

use super::*;

/// How a UFCS member call was navigated (Slice 6 / F-002): a plain `.`, a `?.` on a genuinely nullable
/// receiver (needs the null-safe `match` desugar), or a `?.` on a non-null receiver (rare — a plain call
/// with an optional-typed result, matching `?.`-on-non-optional elsewhere).
#[derive(Clone, Copy)]
pub(in crate::checker) enum UfcsNav {
    Plain,
    SafeNullable,
    SafeNonNull,
}

/// The resolved UFCS call site handed to [`Checker::finish_ufcs`] — bundled so the finalizer stays
/// within the argument-count budget. `leaf = None` ⇒ a free-function call; `Some(q)` ⇒ a `q.name(…)`
/// native module call.
pub(in crate::checker) struct UfcsSite<'a> {
    span: Span,
    leaf: Option<&'a str>,
    name: &'a str,
    object: &'a crate::ast::Expr,
    args: &'a [crate::ast::Expr],
    nav: UfcsNav,
}

impl Checker {
    pub(in crate::checker) fn try_ufcs(
        &mut self,
        object: &crate::ast::Expr,
        recv_ty: &Ty,
        name: &str,
        args: &[crate::ast::Expr],
        call_span: Span,
        nav: UfcsNav,
    ) -> Option<Ty> {
        // (1) A user free function wins over any stdlib native of the same name. Single-overload only
        // this slice (overload-set + multi-package UFCS deferred — F-004); a non-fitting arity/first
        // param falls through to the native search rather than committing to an error.
        if let Some(sigs) = self.funcs.get(name).cloned() {
            if sigs.len() == 1
                && sigs[0].params.len() == args.len() + 1
                && self.ufcs_first_accepts(&sigs[0].params[0], recv_ty)
            {
                let sig = &sigs[0];
                let ret =
                    self.check_ufcs_call(name, &sig.params, &sig.ret, recv_ty, args, call_span);
                return Some(self.finish_ufcs(
                    UfcsSite {
                        span: call_span,
                        leaf: None,
                        name,
                        object,
                        args,
                        nav,
                    },
                    ret,
                ));
            }
        }
        // (2) An imported native `name` whose first parameter accepts the receiver. A native is
        // eligible only when its module's *leaf* is the imported qualifier (`import Core.List` ⇒
        // `imports["List"] == "Core.List"`), so the leaf we emit resolves identically on every backend
        // (`index_of_qualified` — import-map-first with a Native-excluded leaf fallback — on the
        // interpreter/compiler, the import map on the transpiler). An aliased-only core import is
        // skipped (call it explicitly). Two distinct matches ⇒ ambiguous.
        let mut matched: Option<(usize, &'static str)> = None;
        let mut ambiguous = false;
        for (i, n) in crate::native::registry().iter().enumerate() {
            // A native matches by its own name, or (DEC-274) by a function-import alias mapping
            // this call-site name onto it (`import Core.List.reverse as rev;` ⇒ `xs.rev()`).
            let alias_hit = self
                .fn_imports
                .get(name)
                .is_some_and(|(m, real)| m == n.module && real == n.name);
            if (n.name != name && !alias_hit) || n.params.len() != args.len() + 1 {
                continue;
            }
            // `Reflection.typeName` is resolved from its argument's static type and erased before any
            // backend; a UFCS-produced raw `typeName(x)` call would instead reach the backend (where
            // its PHP erasure is only coarse) and diverge. Exclude it — call it qualified. `kind` /
            // `className` are plain natives (byte-identical), so they stay UFCS-eligible.
            if n.module == "Core.Reflection" && n.name == "typeName" {
                continue;
            }
            let leaf = n.module.rsplit('.').next().unwrap_or(n.module);
            // DEC-274 sugar gate: a native is method-position eligible when its MODULE is imported
            // (`import Core.List;` — today's rule, ratified) OR when the FUNCTION ITSELF is
            // member-imported under this call-site name (`import Core.List.reverse;` ⇒
            // `xs.reverse()` — the ruled function-level gate; aliased imports match on the alias).
            let module_imported = self.imports.get(leaf).map(String::as_str) == Some(n.module);
            let function_imported = alias_hit;
            if (module_imported || function_imported)
                && self.ufcs_first_accepts(&n.params[0], recv_ty)
            {
                if matched.is_some() {
                    ambiguous = true;
                } else {
                    matched = Some((i, leaf));
                }
            }
        }
        if ambiguous {
            self.err_coded(
                call_span,
                format!(
                    "UFCS call `.{name}(…)` is ambiguous — more than one imported native matches"
                ),
                "E-UFCS-AMBIGUOUS",
                Some(format!(
                    "call it explicitly, e.g. `Module.{name}(receiver, …)`"
                )),
            );
            return Some(Ty::Error);
        }
        if let Some((idx, leaf)) = matched {
            let n = &crate::native::registry()[idx];
            self.require_option_for_result_bridge(n.module, n.name, call_span);
            // The rewrite must carry the native's REAL name — an aliased function import
            // (`import Core.List.reverse as rev;`, DEC-274) matched under the alias, but
            // `List.rev` resolves on no backend. The label keeps the call-site spelling for
            // diagnostics.
            let label = format!("{leaf}.{name}");
            let ret = self.check_ufcs_call(&label, &n.params, &n.ret, recv_ty, args, call_span);
            return Some(self.finish_ufcs(
                UfcsSite {
                    span: call_span,
                    leaf: Some(leaf),
                    name: n.name,
                    object,
                    args,
                    nav,
                },
                ret,
            ));
        }
        None
    }

    /// Does a candidate's first parameter type accept the UFCS receiver? Uses [`Self::unify`] against a
    /// throwaway substitution so a generic first parameter (`List<T>`) matches a concrete receiver
    /// (`List<int>`); for a non-generic parameter this reduces to ordinary assignability (receiver →
    /// parameter), so subtyping is honored.
    pub(in crate::checker) fn ufcs_first_accepts(&self, param0: &Ty, recv_ty: &Ty) -> bool {
        let mut theta: HashMap<String, Ty> = HashMap::new();
        self.unify(param0, recv_ty, &mut theta)
    }

    /// Type-check a chosen UFCS candidate as `name(receiver, args)`: the receiver fills the first
    /// parameter (already shown to fit by [`Self::ufcs_first_accepts`]), the call's `args` fill the
    /// rest. A generic candidate (its signature mentions `Ty::Param`) infers the substitution from the
    /// receiver *and* the arguments and applies it to the return type — exactly [`Self::check_generic_call`]
    /// with a prepended receiver; a monomorphic candidate validates each argument by assignability.
    /// The caller guarantees `params.len() == args.len() + 1`.
    pub(in crate::checker) fn check_ufcs_call(
        &mut self,
        label: &str,
        params: &[Ty],
        ret: &Ty,
        recv_ty: &Ty,
        args: &[crate::ast::Expr],
        span: Span,
    ) -> Ty {
        if params.iter().any(ty_has_param) || ty_has_param(ret) {
            let mut theta: HashMap<String, Ty> = HashMap::new();
            // Seed θ from the receiver (the first parameter), then each remaining argument.
            self.unify(&params[0], recv_ty, &mut theta);
            for (param, arg) in params[1..].iter().zip(args) {
                let at = self.check_arg(arg, param);
                if !self.unify(param, &at, &mut theta) {
                    let want = apply_subst(param, &theta);
                    self.err(
                        span,
                        format!("`{label}` argument expects `{want}`, found `{at}`"),
                    );
                }
            }
            apply_subst(ret, &theta)
        } else {
            for (param, arg) in params[1..].iter().zip(args) {
                let at = self.check_arg(arg, param);
                if !self.ty_assignable(&at, param) {
                    self.err(
                        span,
                        format!("`{label}` argument expects `{param}`, found `{at}`"),
                    );
                }
            }
            ret.clone()
        }
    }

    /// Record the desugared UFCS call for [`rewrite_ufcs`] (keyed by the enclosing `Call` node's
    /// `Span.start` — each call site's `(` is at a unique byte offset) and return the call site's type.
    /// `leaf = None` ⇒ a free-function call `name(object, args)`; `leaf = Some(q)` ⇒ a native module
    /// call `q.name(object, args)` (the AST shape the user would hand-write). The receiver and arguments
    /// are carried verbatim so `rewrite_ufcs` re-walks them for nested UFCS (`xs.filter(p).map(g)`).
    ///
    /// For a **null-safe** UFCS on a nullable receiver (`x?.f(a)`, F-002), the plain call would lose the
    /// null short-circuit, so the substitution is instead `match x { null => null, r => f(r, a) }` — the
    /// receiver is evaluated once, a `null` short-circuits to `null`, otherwise the unwrapped value is
    /// passed. This reuses match-over-optional (no new `Op`; runs/transpiles on every backend), and the
    /// call site's type is `opt_wrap(ret)`. A `?.` on a non-nullable receiver (rare) is a plain call
    /// with an `opt_wrap`ped type, matching the existing `?.`-on-non-optional behavior.
    pub(in crate::checker) fn finish_ufcs(&mut self, site: UfcsSite, ret: Ty) -> Ty {
        use crate::ast::{Expr, MatchArm, Pattern};
        let UfcsSite {
            span: call_span,
            leaf,
            name,
            object,
            args,
            nav,
        } = site;
        let build_call = |receiver: Expr| -> Expr {
            let mut new_args = Vec::with_capacity(args.len() + 1);
            new_args.push(receiver);
            new_args.extend(args.iter().cloned());
            let callee = match leaf {
                None => Expr::Ident(name.to_string(), call_span),
                Some(q) => Expr::Member {
                    object: Box::new(Expr::Ident(q.to_string(), call_span)),
                    name: name.to_string(),
                    safe: false,
                    sep: crate::ast::MemberSep::Dot,
                    span: call_span,
                },
            };
            Expr::Call {
                callee: Box::new(callee),
                args: new_args,
                type_args: Vec::new(),
                span: call_span,
            }
        };
        let repl = if matches!(nav, UfcsNav::SafeNullable) {
            let recv = Expr::Ident("__ufcs_recv".to_string(), call_span);
            Expr::Match {
                scrutinee: Box::new(object.clone()),
                arms: vec![
                    MatchArm {
                        pattern: Pattern::Null(call_span),
                        guard: None,
                        body: Expr::Null(call_span),
                        span: call_span,
                    },
                    MatchArm {
                        pattern: Pattern::Binding {
                            name: "__ufcs_recv".to_string(),
                            span: call_span,
                        },
                        guard: None,
                        body: build_call(recv),
                        span: call_span,
                    },
                ],
                span: call_span,
            }
        } else {
            build_call(object.clone())
        };
        self.ufcs_resolutions.insert(call_span.start, repl);
        if matches!(nav, UfcsNav::SafeNullable | UfcsNav::SafeNonNull) {
            Self::opt_wrap(ret)
        } else {
            ret
        }
    }
}

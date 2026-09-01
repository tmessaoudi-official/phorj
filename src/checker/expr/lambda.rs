//! Checker — LAMBDA expressions: parameter typing, the body walk, and the inferred function type.
//!
//! Split out of `literals.rs` by cohesion (Invariant 13, M-Decomp). A lambda is its own FUNCTION
//! BOUNDARY, which is exactly why it earns its own home: DEC-339's `fn_scope_floor` is raised here, and
//! that is what keeps a lambda parameter legally able to shadow an enclosing local (accepted cases
//! 19-21 of `docs/specs/UNIFIED-SPEC.md#block-scope-shadowing--the-redeclaration-rule`). Re-attached to the same `impl Checker`,
//! so every call site is unchanged.

use super::*;

impl Checker {
    /// Type-check a lambda expression (M3 S3, Task 3). Returns `Ty::Function(params, ret, throws)`
    /// (DEC-222 — the lambda's declared checked-exception set; empty when no `throws` clause).
    ///
    /// Type-checks a lambda. A method-body lambda **may** capture `this` (Phase 1 closures slice): it
    /// is captured by value (the `Rc` instance handle, so mutations stay live), `this` types as the
    /// enclosing class via `cur_class`, and the two backends + PHP all bind the same receiver. The one
    /// place it stays rejected is a **field/static initializer** (`in_field_init`): the instance is
    /// only partially built when an initializer runs, so capturing the receiver is the F8 footgun.
    pub(in crate::checker) fn check_lambda(
        &mut self,
        params: &[crate::ast::Param],
        ret: &Option<crate::ast::Type>,
        throws: &[crate::ast::Type],
        body: &crate::ast::LambdaBody,
        span: Span,
    ) -> Ty {
        self.check_lambda_with(params, ret, throws, body, span, None)
    }

    /// [`Self::check_lambda`] with an optional **contextual parameter type** (DEC-239): a pipe
    /// lambda `x |> (v => …)` (and the multi-`%` IIFE) has one param written as `Type::Infer`; the
    /// call site (`check_call`'s IIFE intercept) checks the piped argument first and passes its
    /// type here, which both types the param and records the resolution for AST materialization.
    pub(in crate::checker) fn check_lambda_with(
        &mut self,
        params: &[crate::ast::Param],
        ret: &Option<crate::ast::Type>,
        throws: &[crate::ast::Type],
        body: &crate::ast::LambdaBody,
        span: Span,
        ctx_param: Option<&Ty>,
    ) -> Ty {
        use crate::ast::LambdaBody;
        // A field-default lambda may not capture `this` (partially-built instance, F8).
        if self.in_field_init && crate::ast::lambda_uses_this(body) {
            self.err_coded(
                span,
                "a field-initializer lambda cannot capture `this` — the instance is not fully built yet",
                "E-LAMBDA-THIS",
                Some("move the closure into the constructor body, or capture a specific value (`var v = this.x;`) instead".into()),
            );
        }
        // DEC-298: variadics are free-function-only in v1 — a variadic lambda param is rejected
        // (shared logic with the method path), never silently mis-typed.
        self.reject_nonfree_variadic(params);
        let param_tys: Vec<Ty> = params
            .iter()
            .map(|p| self.resolve_lambda_param_ty(p, ctx_param))
            .collect();
        // DEC-222: resolve + normalize the lambda's DECLARED throws (flatten unions, canonical-sort,
        // dedupe — like `resolve_type`'s function-type path), validate each is an `Error` subtype
        // (`E-THROW-TYPE`/`E-THROWS-TOO-BROAD`, shared with fn/ctor decls), and check the body with
        // these throws in context. Absent clause ⇒ empty set ⇒ a `throw`/`?` in the body still hits
        // `E-THROW-UNDECLARED`/`E-CALL-UNHANDLED` (a bare closure declares nothing, exactly like a
        // named function with no `throws`). No inference — a throwing lambda must declare its throws.
        let lambda_throws: Vec<Ty> = {
            let resolved: Vec<Ty> = throws.iter().map(|t| self.resolve_type(t)).collect();
            let mut es = Self::flatten_throws(resolved);
            es.sort_by_key(std::string::ToString::to_string);
            es.dedup();
            self.validate_throw_types(&es, span);
            es
        };
        // Save and replace the current return type (a lambda has its own return scope).
        let saved_ret = std::mem::replace(&mut self.cur_ret, Ty::Error);
        // A lambda is a separate callable: its body discharges against ITS OWN declared throws
        // (DEC-222), and it does not see the lexical `try` it is written inside (it may be invoked
        // elsewhere — e.g. passed to a native), so the enclosing `try` stack is cleared (M-faults 2b).
        let saved_throws = std::mem::replace(&mut self.cur_throws, lambda_throws.clone());
        let saved_try = std::mem::take(&mut self.try_catch_stack);
        let saved_main = std::mem::replace(&mut self.cur_is_main, false);
        // DEC-339: "a lambda starts a new function" — raising the floor here is what keeps accepted
        // cases 19-21 legal (a lambda param MAY shadow an enclosing local; PHP arrow-fn params shadow
        // correctly and the block-body capture list is `free_vars` minus params, so both legs agree).
        let saved_floor = std::mem::replace(&mut self.fn_scope_floor, self.scopes.len());
        self.push_scope();
        for p in params {
            let pty = self.resolve_lambda_param_ty(p, ctx_param);
            self.declare(&p.name, pty, p.span);
        }
        let ret_ty = match body {
            LambdaBody::Expr(e) => {
                let inferred = self.check_expr(e);
                if let Some(rt) = ret {
                    let declared = self.resolve_type(rt);
                    if !self.ty_assignable(&inferred, &declared) {
                        self.err_assign(span, &inferred, &declared);
                    }
                    declared
                } else {
                    inferred
                }
            }
            LambdaBody::Block(stmts) => {
                // A2/F10: an explicit `-> T` annotation is required for statement-body lambdas.
                match ret {
                    Some(rt) => {
                        let declared = self.resolve_type(rt);
                        self.cur_ret = declared.clone();
                        // Batch F (finding #6): a statement-body lambda must return on all paths just
                        // like a free fn/method — falling off the end of a `-> int` lambda bound `unit`
                        // into an `int` slot. Route through `check_body` (W-UNREACHABLE) + enforce
                        // return totality.
                        self.check_body(stmts);
                        self.check_return_totality(&declared, stmts, span);
                        declared
                    }
                    None => self.err(
                        span,
                        "a statement-body lambda requires an explicit `-> T` return type",
                    ),
                }
            }
        };
        self.pop_scope();
        self.cur_ret = saved_ret;
        self.cur_throws = saved_throws;
        self.try_catch_stack = saved_try;
        self.cur_is_main = saved_main;
        self.fn_scope_floor = saved_floor;
        Ty::Function(param_tys, Box::new(ret_ty), lambda_throws)
    }

    /// Resolve one lambda parameter's type. A `Type::Infer` param (only a pipe lambda / multi-`%`
    /// IIFE can produce one — DEC-239) takes the contextual type when the call site supplied it,
    /// recording the resolution (keyed by the param's `span.start`) so `materialize_pipe_params`
    /// can write it into the AST for the backends. Without a contextual type the lambda escaped
    /// pipe application (e.g. `x |> (v => v) + 1` binds the `+` to the lambda, per the uniform RHS
    /// grammar) — error loudly with the pipe-specific message, never silent.
    fn resolve_lambda_param_ty(&mut self, p: &crate::ast::Param, ctx: Option<&Ty>) -> Ty {
        if matches!(p.ty, crate::ast::Type::Infer(_)) {
            if let Some(t) = ctx {
                self.pipe_param_resolutions.insert(p.span.start, t.clone());
                return t.clone();
            }
            return self.err_coded(
                p.span,
                format!(
                    "a pipe lambda `({0} => …)` has no parameter type of its own — it must be \
                     applied directly by `|>`, but here it is used as a plain value",
                    p.name
                ),
                "E-PIPE-LAMBDA-CONTEXT",
                Some(
                    "parenthesize the pipe if an operator follows — `(x |> (v => …)) + 1` — or \
                     write a full lambda `function(T v) => …` to use it as a value"
                        .into(),
                ),
            );
        }
        self.resolve_type(&p.ty)
    }
}

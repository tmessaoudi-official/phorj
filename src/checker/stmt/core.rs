//! Statement checking — dispatch + literal-expectation threading.

use super::*;

impl Checker {
    pub(in crate::checker) fn stmt_span(s: &crate::ast::Stmt) -> Span {
        use crate::ast::Stmt;
        match s {
            Stmt::VarDecl { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::If { span, .. }
            | Stmt::For { span, .. }
            | Stmt::While { span, .. }
            | Stmt::CFor { span, .. }
            | Stmt::Expr(_, span)
            | Stmt::Discard(_, span)
            | Stmt::Block(_, span)
            | Stmt::Throw { span, .. }
            | Stmt::Try { span, .. }
            | Stmt::Destructure { span, .. }
            | Stmt::Using { span, .. } => *span,
            Stmt::Break(span) | Stmt::Continue(span) => *span,
        }
    }

    /// Whether `e` is exactly a `parent.constructor(…)` call (B1b). Used to flag the one legal
    /// statement position for the forwarding form before checking it.
    pub(in crate::checker) fn is_parent_ctor_call(e: &crate::ast::Expr) -> bool {
        matches!(e, crate::ast::Expr::ParentCall { method, .. } if method == "constructor")
    }

    pub(in crate::checker) fn check_stmt(&mut self, stmt: &crate::ast::Stmt) {
        use crate::ast::Stmt;
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => self.check_var_decl(ty, name, init, *mutable, *span),
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                use crate::ast::Expr;
                // Always check the value (surfaces nested errors regardless of the target's fate).
                let vty = self.check_expr(value);
                match target {
                    Expr::Ident(name, _) => self.check_local_reassign(name, &vty, target, value),
                    // Value-type element set `xs[i] = e` / `m[k] = e` (M-mut.5).
                    Expr::Index { object, index, .. } => {
                        self.check_index_assign(object, index, &vty, value, *span)
                    }
                    // Shared-mutable instance field set `o.f = e` / `this.f = e` (M-mut.6).
                    Expr::Member {
                        object, name, safe, ..
                    } => self.check_field_assign(object, name, *safe, &vty, value, *span),
                    _ => {
                        self.err_coded(
                            *span,
                            "assignment target must be a variable, an indexed element, or a field",
                            "E-ASSIGN-TARGET",
                            Some(
                                "only `name = e;`, `container[i] = e;`, and `obj.field = e;` are supported; nested places (`a.b.c`, `this.f[i]`) land in a later slice"
                                    .into(),
                            ),
                        );
                    }
                }
            }
            Stmt::Return { value, span } => {
                let want = self.cur_ret.clone();
                let actual = match value {
                    // C2 sink: `return f()` resolves a bare return-overloaded call against the declared
                    // return type. Skipped for a `void`/poison return type (no real overload returns
                    // it) — that falls through to `E-OVERLOAD-NO-CONTEXT`.
                    Some(e)
                        if !matches!(want, Ty::Void | Ty::Error)
                            && self.is_return_overload_call(e) =>
                    {
                        self.try_resolve_sink_overload(e, &want)
                            .unwrap_or(Ty::Error)
                    }
                    // `return <list/map literal>` against a `-> List<T>` / `-> Map<K,V>` return type:
                    // thread the declared element/value type into the literal (UA-1.6 / DEC-178, the
                    // same path as the decl initializer) — a union member (`return [1, "two"]` with
                    // `-> List<int | string>`), a subtype-upcast, or an empty `[]` all type-check.
                    // Non-literals / non-collection return types return `None` and fall to `check_expr`.
                    Some(e) => self
                        .thread_literal_expected(e, &want)
                        .unwrap_or_else(|| self.check_expr(e)),
                    None => Ty::Void,
                };
                if !self.ty_assignable(&actual, &want) {
                    self.err_assign(*span, &actual, &want);
                }
            }
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => {
                let c = self.check_expr(cond);
                if let Some(name) = bind {
                    // `if (var name = cond)`: the scrutinee must be optional; inside the then-block
                    // `name` is smart-cast to the non-optional inner `T` (and only there). The else
                    // block sees neither `name` nor any narrowing.
                    let inner = match &c {
                        Ty::Optional(i) => (**i).clone(),
                        Ty::Error => Ty::Error,
                        other => self.err_coded(
                            *span,
                            format!("`if (var {name} = …)` requires an optional `T?` scrutinee, found `{other}`"),
                            "E-IF-LET-TYPE",
                            Some("if-let narrows an optional to its non-null inner; the scrutinee is already non-optional".into()),
                        ),
                    };
                    self.push_scope();
                    self.declare(name, inner, *span);
                    self.check_block(then_block);
                    self.pop_scope();
                } else {
                    if !self.ty_assignable(&c, &Ty::Bool) {
                        self.err(*span, format!("`if` condition must be `bool`, found `{c}`"));
                    }
                    // Flow-narrowing (S5.3): the then-block sees the variables the condition implies
                    // when *true* (e.g. `if (x instanceof T)` narrows `x` to `T`). The narrowed shadows
                    // are installed in a child scope and dropped after the block. The else-block sees
                    // the *false*-polarity narrowing (T2: the remaining union members, the null arm…).
                    let then_narrow = self.narrow_from_condition(cond, true);
                    self.check_block_narrowed(then_block, &then_narrow, *span);
                    if let Some(eb) = else_block {
                        let else_narrow = self.narrow_from_condition(cond, false);
                        self.check_block_narrowed(eb, &else_narrow, *span);
                    }
                    return;
                }
                // The if-let (`bind`) path: its else-block sees neither the binding nor any narrowing.
                if let Some(eb) = else_block {
                    self.check_block(eb);
                }
            }
            Stmt::For { .. } => self.check_for(stmt), // implemented in Task 5
            Stmt::Using {
                ty,
                name,
                init,
                body,
                span,
            } => self.check_using(ty, name, init, body, *span),
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => self.check_while(cond, body, *post_cond, *span),
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                ..
            } => self.check_cfor(init.as_deref(), cond.as_ref(), step.as_deref(), body),
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    self.err_coded(
                        *span,
                        "`break` outside a loop",
                        "E-BREAK-OUTSIDE-LOOP",
                        Some(
                            "`break` may only appear inside a `for`/`while`/`do-while` loop".into(),
                        ),
                    );
                }
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    self.err_coded(
                        *span,
                        "`continue` outside a loop",
                        "E-CONTINUE-OUTSIDE-LOOP",
                        Some(
                            "`continue` may only appear inside a `for`/`while`/`do-while` loop"
                                .into(),
                        ),
                    );
                }
            }
            Stmt::Block(stmts, _) => self.check_block(stmts),
            Stmt::Expr(e, _) => {
                // B1b: a bare `parent.constructor(…)` statement is the only legal position for the
                // forwarding form — flag it so `check_parent_ctor_call` accepts it (every other position
                // is `E-PARENT-CTOR-STMT`).
                self.parent_ctor_ok = Self::is_parent_ctor_call(e);
                // M-must-use Slice A: a non-`void`/`empty` result used as a bare statement would be
                // dropped silently — forbid it (`E-UNUSED-VALUE`). `discard <expr>;` is the escape hatch.
                let t = self.check_expr(e);
                self.parent_ctor_ok = false;
                if !matches!(t, Ty::Void | Ty::Empty | Ty::Error | Ty::Never) {
                    self.err_coded(
                        Self::expr_span(e),
                        format!("unused `{t}` value"),
                        "E-UNUSED-VALUE",
                        Some(
                            "a non-`void`/`empty` result must be used; bind it or prefix `discard`"
                                .into(),
                        ),
                    );
                }
            }
            Stmt::Discard(e, _) => {
                // Must-use escape hatch: type-check the expression; any result type may be dropped.
                // A `discard parent.constructor(…)` is still a statement position (B1b).
                self.parent_ctor_ok = Self::is_parent_ctor_call(e);
                self.check_expr(e);
                self.parent_ctor_ok = false;
            }
            // M-faults 2b.3: `throw e` — the value must implement `Error` (`E-THROW-TYPE`), and the
            // exception must be *discharged* in context: caught by an enclosing `try` or declared in
            // the enclosing `throws` (`E-THROW-UNDECLARED`, or `E-UNCAUGHT-THROW` inside `main`).
            Stmt::Throw { value, span } => {
                let e = self.check_expr(value);
                if matches!(e, Ty::Error) {
                    // poison — an earlier error already reported
                } else if !self.is_error_type(&e) {
                    self.err_coded(
                        *span,
                        format!(
                            "can only `throw` a value whose type implements `Error`, found `{e}`"
                        ),
                        "E-THROW-TYPE",
                        Some("define the thrown type as `class Foo implements Error { … }`".into()),
                    );
                } else if !self.covered_by_try(&e) && !self.throws_declared(&e) {
                    if self.cur_is_main {
                        self.err_coded(
                            *span,
                            format!("`{e}` thrown in `main` escapes the program entry point"),
                            "E-UNCAUGHT-THROW",
                            Some("wrap it in `try { … } catch (… e) { … }` — `main` may not let an exception escape".into()),
                        );
                    } else {
                        self.err_coded(
                            *span,
                            format!("`{e}` is thrown here but neither caught nor declared"),
                            "E-THROW-UNDECLARED",
                            Some(format!("add `throws {e}` to the enclosing function, or wrap this in `try`/`catch`")),
                        );
                    }
                }
            }
            // M-faults 2b.3: a `try` — validate each catch type (`<: Error`, flag a shadowed clause
            // `W-CATCH-UNREACHABLE`), check the body with the catch set active so a throw inside is
            // discharged, then each catch body with its binding in scope, then `finally`.
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                // Resolve + validate catch types, building the active frame and the per-clause
                // binding types. A union catch `(A | B e)` contributes both members to the frame.
                let mut frame: Vec<Ty> = Vec::new();
                let mut seen: Vec<Ty> = Vec::new();
                let mut clause_tys: Vec<Ty> = Vec::with_capacity(catches.len());
                for c in catches {
                    let cty = self.resolve_type(&c.ty);
                    let members: Vec<Ty> = match &cty {
                        Ty::Union(ms) => ms.clone(),
                        other => vec![other.clone()],
                    };
                    for m in &members {
                        if !self.is_error_type(m) {
                            self.err_coded(
                                c.span,
                                format!("a `catch` type must implement `Error`, found `{m}`"),
                                "E-CATCH-TYPE",
                                Some("catch a type defined `class Foo implements Error { … }` (or the `Error` base itself)".into()),
                            );
                        }
                    }
                    // A clause every member of which is already covered by an earlier clause can
                    // never run (PHP is silent here; Phorj lints — see the totality cluster).
                    if !members.is_empty()
                        && members
                            .iter()
                            .all(|m| seen.iter().any(|s| self.ty_assignable(m, s)))
                    {
                        self.warn_coded(
                            c.span,
                            "unreachable `catch`: an earlier clause already catches this type",
                            "W-CATCH-UNREACHABLE",
                            Some(
                                "remove it, or reorder so the more specific clause comes first"
                                    .into(),
                            ),
                        );
                    }
                    seen.extend(members.iter().cloned());
                    frame.extend(members.iter().cloned());
                    clause_tys.push(cty);
                }
                // The catch set covers throws inside the *body* only (a throw in a catch/finally is
                // not caught by the same `try`): push for the body, pop before the clauses.
                self.try_catch_stack.push(frame);
                self.check_block(body);
                self.try_catch_stack.pop();
                for (c, cty) in catches.iter().zip(clause_tys) {
                    self.push_scope();
                    self.declare(&c.name, cty, c.span);
                    self.check_block(&c.body);
                    self.pop_scope();
                }
                if let Some(fb) = finally_block {
                    self.check_block(fb);
                }
            }
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => self.check_destructure(pat, init, else_block.as_deref(), *span),
        }
    }
}

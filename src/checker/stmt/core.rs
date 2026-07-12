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
            | Stmt::Destructure { span, .. } => *span,
            Stmt::Break(span) | Stmt::Continue(span) => *span,
        }
    }

    /// Whether `e` is exactly a `parent.constructor(…)` call (B1b). Used to flag the one legal
    /// statement position for the forwarding form before checking it.
    pub(in crate::checker) fn is_parent_ctor_call(e: &crate::ast::Expr) -> bool {
        matches!(e, crate::ast::Expr::ParentCall { method, .. } if method == "constructor")
    }

    /// Thread an EXPECTED `List<T>`/`Map<K,V>` type into a list/map literal (UA-1.6 / DEC-178): check
    /// each member against `T` / `K,V` — allowing a **union** or subtype-upcast member — instead of the
    /// bottom-up first-element/first-pair inference in `check_list`/`check_map` (which rejects
    /// heterogeneous members as "must share one type"). Also supplies the element type for an empty
    /// `[]`. Returns the expected collection type on a literal/type match, `None` otherwise (the caller
    /// falls back to `check_expr`). Shared by the declaration initializer and the `return` value; the
    /// generic-call-argument position (which needs bidirectional inference) is deferred to Wave C.
    pub(in crate::checker) fn thread_literal_expected(
        &mut self,
        e: &crate::ast::Expr,
        expected: &Ty,
    ) -> Option<Ty> {
        match (e, expected) {
            (crate::ast::Expr::List(elems, _), Ty::List(elem_ty)) => {
                for el in elems {
                    let et = self.check_expr(el);
                    if !self.ty_assignable(&et, elem_ty) {
                        self.err_assign(Self::expr_span(el), &et, elem_ty);
                    }
                }
                Some(Ty::List(elem_ty.clone()))
            }
            (crate::ast::Expr::Map(pairs, _), Ty::Map(key_ty, val_ty)) => {
                // Keys must be the hashable subset (`int`/`bool`/`string`) — mirror `check_map`'s
                // `E-MAP-KEY` guard, which this expected-type path bypasses.
                if !matches!(&**key_ty, Ty::Int | Ty::Bool | Ty::String | Ty::Error) {
                    self.err_coded(
                        Self::expr_span(e),
                        format!(
                            "map key type must be `int`, `bool`, or `string`, found `{key_ty}`"
                        ),
                        "E-MAP-KEY",
                        None,
                    );
                }
                for (k, v) in pairs {
                    let kt = self.check_expr(k);
                    if !self.ty_assignable(&kt, key_ty) {
                        self.err_assign(Self::expr_span(k), &kt, key_ty);
                    }
                    let vt = self.check_expr(v);
                    if !self.ty_assignable(&vt, val_ty) {
                        self.err_assign(Self::expr_span(v), &vt, val_ty);
                    }
                }
                Some(Ty::Map(key_ty.clone(), val_ty.clone()))
            }
            _ => None,
        }
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
            } => {
                // A let-initializer is the one position where Result-mode `?` propagation is allowed
                // (M-faults 2a): detect it here and type it via `check_propagate` (the unwrapped `Ok`
                // payload). Throws-mode `?` (a throwing call) is allowed in *any* position and tried
                // first; it returns the call's normal type and erases the node (`try_throws_propagate`).
                //
                // Resolve the declared type ONCE (non-`Infer`), reused by the literal expected-type
                // arms below and the final assignability check — a second `resolve_type(ty)` re-emits
                // any resolution error (e.g. `E-TYPE-ARG-COUNT` for `Map<int>`) a second time.
                let declared_once: Option<Ty> = match ty {
                    crate::ast::Type::Infer(_) => None,
                    _ => Some(self.resolve_type(ty)),
                };
                let actual = match init {
                    crate::ast::Expr::Propagate { inner, span: psp } => {
                        match self.try_throws_propagate(inner, *psp) {
                            Some(crate::checker::throws::PropagateOutcome::Throws(t)) => t,
                            // A call that throws nothing was already checked — hand its type to
                            // Result-mode without a duplicate check.
                            Some(crate::checker::throws::PropagateOutcome::Plain(t)) => {
                                self.check_propagate_typed(t, *psp)
                            }
                            None => self.check_propagate(inner, *psp),
                        }
                    }
                    // C2 sink: a bare return-overloaded call binds to a concrete declared type without a
                    // `<Type>` selector — the typed binding supplies the resolving context. (`var x = …`
                    // is `Type::Infer`, has no context, and falls through to the `E-OVERLOAD-NO-CONTEXT`
                    // path.)
                    _ if !matches!(ty, crate::ast::Type::Infer(_))
                        && self.is_return_overload_call(init) =>
                    {
                        let expected = self.resolve_type(ty);
                        self.try_resolve_sink_overload(init, &expected)
                            .unwrap_or(Ty::Error)
                    }
                    // `Channel<T> ch = Channel.new();` (M6 W4): the static constructor has no argument
                    // to infer `T` from, so it takes its element type from the binding's annotation
                    // (the one construction site for a channel). Validate the call shape (0 args) here;
                    // a non-`Channel` annotation falls through to the normal assign-mismatch path.
                    _ if !matches!(ty, crate::ast::Type::Infer(_))
                        && Self::is_channel_new(init) =>
                    {
                        if let crate::ast::Expr::Call {
                            args, span: csp, ..
                        } = init
                        {
                            if !args.is_empty() {
                                self.err_coded(
                                    *csp,
                                    "`Channel.new()` takes no arguments",
                                    "E-CHANNEL-NEW-ARITY",
                                    None,
                                );
                            }
                        }
                        let declared = self.resolve_type(ty);
                        if matches!(&declared, Ty::Named(n, _) if n == "Channel") {
                            declared
                        } else {
                            self.err_coded(
                                *span,
                                format!(
                                    "`Channel.new()` produces a `Channel<T>`, not `{declared}`"
                                ),
                                "E-CHANNEL-NEW-TYPE",
                                None,
                            )
                        }
                    }
                    // A list literal with a `List<T>` annotation is checked against that expected
                    // element type (M-DOGFOOD W0 + W3): each element must be assignable to `T`. This
                    // (a) supplies the element type for an empty `[]` (the runtime value is an empty
                    // `Value::List` regardless of `T`, so no backend change), and (b) lets a
                    // heterogeneous list of subtypes upcast — e.g. `List<Shape> xs = [new Sq(), new
                    // Tri()]` — which post-hoc element unification could not (a) do at all and (b)
                    // `List` is invariant so `List<Sq>` is not assignable to `List<Shape>`. A
                    // non-`List` annotation (e.g. a fixed-length `[T; N]`) falls through to the
                    // normal path, which owns its own length/element checks.
                    // A list/map literal with a `List<T>`/`Map<K,V>` annotation is checked against the
                    // declared element/value types (UA-1.6 / DEC-178) via `thread_literal_expected`:
                    // each member must be assignable to `T` / `K,V`, which (a) supplies the element type
                    // for an empty `[]`, (b) lets a heterogeneous list of subtypes upcast
                    // (`List<Shape> = [new Sq(), new Tri()]`), and (c) lets a union value/element
                    // type-check (`Map<string, int | string> = ["a" => 1, "b" => "two"]`) — none of which
                    // the bottom-up `check_list`/`check_map` inference can. A non-collection annotation
                    // (e.g. `[T; N]`) returns `None` and falls through to the normal path below.
                    crate::ast::Expr::List(..) | crate::ast::Expr::Map(..)
                        if !matches!(ty, crate::ast::Type::Infer(_)) =>
                    {
                        let expected = declared_once.clone().unwrap_or(Ty::Error);
                        self.thread_literal_expected(init, &expected)
                            .unwrap_or_else(|| self.check_expr(init))
                    }
                    _ => self.check_expr(init),
                };
                let declared = match ty {
                    crate::ast::Type::Infer(infer_span) => {
                        // `var` binds the initializer's type — but a bare `null` (type `Ty::Null`)
                        // has no inferable element type and needs an explicit annotation, e.g.
                        // `int? x = null;` (S0.2 / S2).
                        if matches!(actual, Ty::Null) {
                            self.err_coded(
                                *infer_span,
                                "cannot infer a type from `null`",
                                "E-INFER-NULL",
                                Some("annotate the optional, e.g. `int? x = null;`".into()),
                            )
                        } else {
                            actual.clone()
                        }
                    }
                    _ => {
                        // Reuse the single resolution from above (non-`Infer` ⇒ `Some`).
                        let declared = declared_once.clone().unwrap_or(Ty::Error);
                        // `[T; N] p = [e0, e1, …]`: a list literal carries a known length, so the
                        // fixed-length is checked *here* (the literal is the one place the length is
                        // statically known) — `List` itself is length-erased, so this is the only
                        // path that introduces a `[T; N]` value (Phase 1 types slice).
                        if let (Ty::FixedList(elem, n), crate::ast::Expr::List(elems, _)) =
                            (&declared, init)
                        {
                            if elems.len() != *n {
                                self.err_coded(
                                    *span,
                                    format!(
                                        "expected `[{elem}; {n}]` (length {n}), found a list literal of length {}",
                                        elems.len()
                                    ),
                                    "E-FIXEDLIST-LEN",
                                    None,
                                );
                            }
                            // Element-type compatibility (List is invariant, so this checks `elem`).
                            if !self.ty_assignable(&actual, &Ty::List(elem.clone())) {
                                self.err_assign(*span, &actual, &declared);
                            }
                        } else if !self.ty_assignable(&actual, &declared) {
                            self.err_assign(*span, &actual, &declared);
                        }
                        declared
                    }
                };
                // S0a: a `void` value is uncapturable. Binding one into a variable is an error —
                // *unless* the declared type is the holdable `empty` (`empty x = noop();`), the
                // explicit escape hatch (`void <: empty`). This catches both `var x = noop()`
                // (inferred `declared` = `Void`) and `void x = noop()` (declared = `Void`).
                let declared = if actual == Ty::Void && declared != Ty::Empty {
                    self.err_coded(
                        *span,
                        "a `void` value cannot be captured — the expression produces nothing",
                        "E-VOID-CAPTURE",
                        Some(
                            "drop the binding and call it as a statement; or, to hold the empty value, annotate it `empty` (e.g. `empty x = …;`)"
                                .into(),
                        ),
                    )
                } else {
                    declared
                };
                self.declare_binding(name, declared, *mutable, *span);
            }
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

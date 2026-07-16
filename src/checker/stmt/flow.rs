//! Statement checking — destructuring, narrowing, loops.

use super::*;

impl Checker {
    /// Let-destructuring (Phase 1 slice 5): type the initializer, decide refutability, enforce the
    /// `else` rules, then bind every binder into the **current** scope at its resolved element/field
    /// type. The `else` block (refutable list only) is checked in a scope *without* the binders and
    /// must diverge (Swift `guard let`); a present `else` on an irrefutable pattern is an error.
    pub(in crate::checker) fn check_destructure(
        &mut self,
        pat: &crate::ast::DestructurePat,
        init: &crate::ast::Expr,
        else_block: Option<&[crate::ast::Stmt]>,
        span: Span,
    ) {
        use crate::ast::DestructurePat;
        let init_ty = self.check_expr(init);
        // (binding name, span, resolved type) for each binder, filled per pattern kind below.
        let mut binds: Vec<(String, Span, Ty)> = Vec::new();
        // Whether the pattern is refutable (a present `else` is then required and must diverge).
        let mut refutable = false;
        match pat {
            DestructurePat::Struct {
                type_name, fields, ..
            } => {
                // The head names a concrete class; the init must BE that class (irrefutable). A generic
                // instance must match the head exactly so its type args resolve the fields; a plain
                // subtype is accepted only for a non-generic class (no args to recover).
                let class_args: Option<Vec<Ty>> = match &init_ty {
                    Ty::Error => Some(vec![]), // poison: emit no further errors
                    Ty::Named(cls, cargs) if cls == type_name => Some(cargs.clone()),
                    Ty::Named(cls, _)
                        if self.is_subtype(cls, type_name)
                            && self
                                .classes
                                .get(type_name)
                                .is_some_and(|i| i.type_params.is_empty()) =>
                    {
                        Some(vec![])
                    }
                    other => {
                        self.err_coded(
                            span,
                            format!("cannot destructure `{other}` as `{type_name}`"),
                            "E-DESTRUCTURE-TYPE",
                            Some(format!(
                                "the value must be a `{type_name}` (or a subtype) — destructure it at its own type"
                            )),
                        );
                        None
                    }
                };
                if !matches!(init_ty, Ty::Error) && !self.classes.contains_key(type_name) {
                    self.err_coded(
                        span,
                        format!("`{type_name}` is not a class — only classes can be struct-destructured"),
                        "E-DESTRUCTURE-NOT-CLASS",
                        Some("list values destructure with `var [a, b] = …`".into()),
                    );
                }
                if let Some(cargs) = class_args {
                    let subst = self.class_subst(type_name, &cargs);
                    for f in fields {
                        let fty = self
                            .classes
                            .get(type_name)
                            .and_then(|i| i.fields.get(&f.field).cloned());
                        let resolved = match fty {
                            Some(t) => {
                                // Wave 1.1: destructuring reads the field (→ PHP `$obj->field`), so an
                                // out-of-scope `private`/`protected` field is rejected here too.
                                let v = self
                                    .classes
                                    .get(type_name)
                                    .and_then(|i| i.field_vis.get(&f.field).cloned());
                                self.enforce_member_vis(v, &f.field, f.span, true);
                                apply_subst(&t, &subst)
                            }
                            // Only emit "no field" when the class is real and not already poisoned
                            // (avoids double-reporting against an upstream error).
                            None => {
                                if self.classes.contains_key(type_name)
                                    && !matches!(init_ty, Ty::Error)
                                {
                                    self.err_coded(
                                        f.span,
                                        format!("type `{type_name}` has no field `{}`", f.field),
                                        "E-DESTRUCTURE-FIELD-UNKNOWN",
                                        None,
                                    );
                                }
                                Ty::Error
                            }
                        };
                        binds.push((f.binding.clone(), f.span, resolved));
                    }
                }
            }
            DestructurePat::List { binders, .. } => {
                let arity = binders.len();
                let elem = match &init_ty {
                    Ty::Error => Ty::Error,
                    // A `List<T>` carries no static length → refutable, `else` mandatory.
                    Ty::List(e) => {
                        refutable = true;
                        (**e).clone()
                    }
                    // A `[T; N]` is irrefutable iff its length matches the pattern arity (slice-3 payoff).
                    Ty::FixedList(e, n) => {
                        if *n != arity {
                            self.err_coded(
                                span,
                                format!(
                                    "destructuring binds {arity} element(s) but the value is `[{e}; {n}]` (length {n})"
                                ),
                                "E-FIXEDLIST-DESTRUCTURE-LEN",
                                Some(format!("bind exactly {n} element(s), or destructure a `List<{e}>` with an `else`")),
                            );
                        }
                        (**e).clone()
                    }
                    other => {
                        self.err_coded(
                            span,
                            format!("cannot list-destructure `{other}` — expected a list"),
                            "E-DESTRUCTURE-NOT-LIST",
                            Some("struct values destructure with `var Type { … } = …`".into()),
                        );
                        Ty::Error
                    }
                };
                for (name, bsp) in binders {
                    binds.push((name.clone(), *bsp, elem.clone()));
                }
            }
        }
        // `else` rules: required iff refutable; forbidden otherwise; and a present `else` must diverge.
        match (refutable, else_block) {
            (true, None) => {
                self.err_coded(
                    span,
                    "this destructuring can fail at runtime and needs an `else` that bails out",
                    "E-DESTRUCTURE-NEEDS-ELSE",
                    Some("add `else { … }` that returns/throws/breaks (a `List` has no static length)".into()),
                );
            }
            (false, Some(_)) => {
                self.err_coded(
                    span,
                    "this destructuring always succeeds, so it cannot have an `else`",
                    "E-DESTRUCTURE-ELSE-IRREFUTABLE",
                    Some(
                        "remove the `else` — an irrefutable destructuring binds unconditionally"
                            .into(),
                    ),
                );
            }
            _ => {}
        }
        if let Some(eb) = else_block {
            // The else block sees none of the binders (it runs only on the destructure-failed path).
            self.push_scope();
            self.check_block(eb);
            self.pop_scope();
            if !self.block_terminates(eb) {
                self.err_coded(
                    span,
                    "the destructuring `else` must not fall through — it has to bail out",
                    "E-DESTRUCTURE-ELSE-FALLTHROUGH",
                    Some(
                        "end every path of the `else` with `return`/`throw`/`break`/`continue`"
                            .into(),
                    ),
                );
            }
        }
        // Bind every binder into the current (enclosing) scope, immutable. A `void`/optional element
        // type is impossible here (init is a real List/class), so no E-VOID-CAPTURE guard is needed.
        // A duplicate binder would silently alias one slot on the VM (the SetLocal target collides) —
        // reject it up front (`var [a, a]` / `var P { x, x }`).
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (name, bsp, _) in &binds {
            if !seen.insert(name.as_str()) {
                self.err_coded(
                    *bsp,
                    format!("`{name}` is bound twice in this destructuring"),
                    "E-DESTRUCTURE-DUP-BIND",
                    Some("each binder must be distinct — rename one (e.g. `y: y2`)".into()),
                );
            }
        }
        for (name, bsp, ty) in binds {
            self.declare_binding(&name, ty, false, bsp);
        }
    }

    /// Check `block` with the given flow-narrowings (`(var, narrowed-type)`) installed as shadows in
    /// a fresh child scope. Each narrowed shadow inherits its outer binding's mutability, so a
    /// `mutable` variable stays reassignable inside the narrowed block (reassignment is still checked
    /// against the narrowed type, keeping narrowing sound — the M-mut.1 smart-cast interaction). An
    /// empty narrowing list just checks the block in the current scope (no extra frame).
    pub(in crate::checker) fn check_block_narrowed(
        &mut self,
        block: &[crate::ast::Stmt],
        narrowings: &[(String, Ty)],
        span: Span,
    ) {
        if narrowings.is_empty() {
            self.check_block(block);
            return;
        }
        self.push_scope();
        for (name, ty) in narrowings {
            let m = self.lookup_binding(name).map(|(_, m)| m).unwrap_or(false);
            self.declare_binding(name, ty.clone(), m, span);
        }
        self.check_block(block);
        self.pop_scope();
    }

    /// The variables a boolean condition narrows when it evaluates to `polarity` (`true` = then-branch,
    /// `false` = else-branch), as `(var, narrowed-type)` shadows. Flow-narrowing engine (S5.3); a `&self`
    /// query (installation is the caller's job). Sources: `x instanceof T` (true ⇒ `T`; false ⇒ the
    /// remaining union members), `x == null` / `x != null` over a `T?` (both polarities), `!c` (flips
    /// polarity), and `a && b` (true side) / `a || b` (false side, De Morgan).
    pub(in crate::checker) fn narrow_from_condition(
        &self,
        cond: &crate::ast::Expr,
        polarity: bool,
    ) -> Vec<(String, Ty)> {
        use crate::ast::{BinaryOp, Expr, UnaryOp};
        let mut out = Vec::new();
        match cond {
            Expr::InstanceOf {
                value, type_name, ..
            } => {
                if let Expr::Ident(name, _) = &**value {
                    // Slice 3 (DEC-184): a primitive type-test narrows the variable to the tested
                    // primitive in the then-branch (`if (x is int)` ⇒ `x: int`). The VM compiler
                    // replicates this exact then-branch narrowing (`compile_if`), so arithmetic on the
                    // narrowed value (`x + 1`) is lockstep.
                    if let Some(prim) = prim_pat_ty(type_name) {
                        if polarity {
                            out.push((name.clone(), prim));
                        } else if let Some((Ty::Optional(inner), _)) = self.lookup_binding(name) {
                            // `is null` over an optional: the complement is the non-null inner.
                            // Lockstep-safe — an optional local already carries its inner `CTy` on the
                            // VM (`resolve_cty`), so no compiler narrowing is needed to specialize it.
                            if matches!(prim, Ty::Null) {
                                out.push((name.clone(), *inner));
                            }
                        }
                        // Deliberately NO union-minus-primitive complement (`(int | string)` else-branch
                        // ⇒ `string`): the VM compiler can't derive it — a union local is `CTy::Other`
                        // and the member set is lost — so narrowing it here would be a
                        // checker-accepts/VM-rejects divergence. Reach the complement with a nested
                        // `is`/`match`. General fix tracked as W2-12 (erased-operand dynamic fallback).
                        return out;
                    }
                    let known = self.classes.contains_key(type_name)
                        || self.interfaces.contains_key(type_name);
                    if !known {
                        return out;
                    }
                    if polarity {
                        // then-branch: narrow to the tested type. `instanceof` carries no type
                        // arguments at runtime (`instanceof Box<int>` ≡ `instanceof Box`), so a
                        // generic class narrows with erased (poison) args — its generic members read
                        // as `mixed` (M-RT generics-all).
                        let arity = self
                            .classes
                            .get(type_name)
                            .map_or(0, |c| c.type_params.len());
                        out.push((
                            name.clone(),
                            Ty::Named(type_name.clone(), vec![Ty::Error; arity]),
                        ));
                    } else if let Some((Ty::Union(members), _)) = self.lookup_binding(name) {
                        // else-branch: drop the tested member (and any subtype of it) from the union.
                        let orig = members.len();
                        let rest: Vec<Ty> = members
                            .into_iter()
                            .filter(|m| {
                                !matches!(m, Ty::Named(n, _)
                                    if n == type_name || self.is_subtype(n, type_name))
                            })
                            .collect();
                        if !rest.is_empty() && rest.len() < orig {
                            out.push((name.clone(), Ty::union_of(rest)));
                        }
                    }
                }
            }
            // (Phorj has no `x == null` / `x != null` comparison — the checker rejects comparing a
            // `T?` to the null literal; optionals are tested via if-let / `??` / match-over-optional,
            // so there is no null-equality narrowing source here.)
            // `a && b` narrows the conjunction on its true side; `a || b` narrows on its false side
            // (De Morgan: `!(a || b)` ≡ `!a && !b`). The other polarity yields a disjunction — no
            // single narrowing — so it contributes nothing.
            Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
                ..
            } if polarity => {
                out.extend(self.narrow_from_condition(lhs, true));
                out.extend(self.narrow_from_condition(rhs, true));
            }
            Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
                ..
            } if !polarity => {
                out.extend(self.narrow_from_condition(lhs, false));
                out.extend(self.narrow_from_condition(rhs, false));
            }
            // `!c` flips the polarity.
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => out.extend(self.narrow_from_condition(expr, !polarity)),
            _ => {}
        }
        out
    }

    pub(in crate::checker) fn check_for(&mut self, stmt: &crate::ast::Stmt) {
        if let crate::ast::Stmt::For {
            ty,
            name,
            val,
            iter,
            body,
            span,
        } = stmt
        {
            let iter_ty = self.check_expr(iter);
            self.push_scope();
            if let Some((vty, vname)) = val {
                // B1 two-binding form `for (K k, V v in map)` — requires a `Map<K, V>`. The first
                // binding takes the key type, the second the value type.
                let (kt, vt) = match iter_ty {
                    Ty::Map(k, v) => (*k, *v),
                    Ty::Error => (Ty::Error, Ty::Error),
                    other => {
                        self.err(
                            *span,
                            format!(
                                "two-binding `for (k, v in …)` requires a Map, found `{other}`"
                            ),
                        );
                        (Ty::Error, Ty::Error)
                    }
                };
                let kd = self.bind_loop_var(ty, name, &kt, *span);
                self.declare(name, kd, *span);
                let vd = self.bind_loop_var(vty, vname, &vt, *span);
                self.declare(vname, vd, *span);
            } else {
                let elem = match iter_ty {
                    // B1 iteration protocol: a `List<T>` or `Set<T>` iterates its elements (`T`).
                    Ty::List(e) | Ty::Set(e) => *e,
                    // A `string` iterates its characters, each a 1-char `string`.
                    Ty::String => Ty::String,
                    // A `Map<K, V>` requires the two-binding form.
                    Ty::Map(_, _) => {
                        self.err(
                            *span,
                            "iterating a Map needs two bindings — `for (K k, V v in map)`"
                                .to_string(),
                        );
                        Ty::Error
                    }
                    Ty::Error => Ty::Error,
                    // DEC-257: an `Iterator<E>` implementor (or an `Iterator<E>`-typed value)
                    // iterates its element type via the hasNext/next pull protocol. The loop is
                    // lowered to a while-pull BLOCK before any backend (`lower_foreach_iter`);
                    // the concrete methods' throws are discharged HERE (the ruled
                    // auto-propagation: caught by an enclosing try, or the enclosing function
                    // declares them — same rule as any bare throwing call).
                    Ty::Named(ref n, ref cargs) if self.iterator_elem(n, cargs).is_some() => {
                        let (elem, throws) = self.iterator_elem(n, cargs).expect("guard checked");
                        self.for_iter_lowerings.insert(span.start);
                        // The ruled auto-propagation: a throwing iterator's foreach is legal when
                        // each fault is caught by an enclosing `try` OR declared by the enclosing
                        // function (the union a `?` pull would give — the lowered pulls unwind
                        // identically at runtime either way).
                        for e in &throws {
                            if !self.covered_by_try(e) && !self.throws_declared(e) {
                                self.err_coded(
                                    *span,
                                    format!(
                                        "iterating this value can throw `{e}` (its `hasNext`/`next` declare it), which is not handled here"
                                    ),
                                    "E-CALL-UNHANDLED",
                                    Some(format!(
                                        "wrap the loop in `try {{ … }} catch ({e} e) {{ … }}`, or declare `throws {e}` on the enclosing function"
                                    )),
                                );
                            }
                        }
                        elem
                    }
                    other => {
                        let hint = if matches!(&other, Ty::Named(n, _)
                            if self.class_implements.get(n.as_str())
                                .is_some_and(|is| is.iter().any(|i| i == "Iterator")))
                        {
                            // Implements Iterator only via a parent — the generic arguments are
                            // not recorded for inherited implements (documented deferral).
                            " (it implements `Iterator` only through a parent — declare \
                             `implements Iterator<…>` on the class itself to make it foreach-able)"
                        } else {
                            ""
                        };
                        self.err(
                            *span,
                            format!(
                                "`for`-`in` requires a List, Set, string, Map, or an `Iterator<T>` implementor, found `{other}`{hint}"
                            ),
                        );
                        Ty::Error
                    }
                };
                let declared = self.bind_loop_var(ty, name, &elem, *span);
                self.declare(name, declared, *span);
            }
            self.loop_depth += 1;
            for s in body {
                self.check_stmt(s);
            }
            self.loop_depth -= 1;
            self.pop_scope();
        }
    }

    /// DEC-257: is `name<cargs>` iterable via the `Core.Iterator` pull protocol? Returns the
    /// element type plus the union of the CONCRETE `hasNext`/`next` throws (to discharge at the
    /// loop site). Two shapes hit: the injected `Iterator<E>` interface itself (throws empty —
    /// interface-method throws are an existing documented deferral), and a class DIRECTLY
    /// implementing it (`ClassInfo::iface_args`; the class's own type parameters substitute from
    /// the instance arguments). Inherited-only implements has no recorded arguments — the caller
    /// falls to the error arm with a targeted hint.
    fn iterator_elem(&self, name: &str, cargs: &[Ty]) -> Option<(Ty, Vec<Ty>)> {
        if name == "Iterator" && self.interfaces.contains_key("Iterator") {
            return cargs.first().cloned().map(|e| (e, Vec::new()));
        }
        let ci = self.classes.get(name)?;
        let args = ci.iface_args.get("Iterator")?;
        let theta = self.class_subst(name, cargs);
        let elem = crate::checker::common::apply_subst(args.first()?, &theta);
        let mut throws: Vec<Ty> = Vec::new();
        for m in ["hasNext", "next"] {
            if let Some(sig) = ci.methods.get(m).and_then(|s| s.first()) {
                for t in &sig.throws {
                    let t = crate::checker::common::apply_subst(t, &theta);
                    if !throws.contains(&t) {
                        throws.push(t);
                    }
                }
            }
        }
        Some((elem, throws))
    }

    /// Resolve a loop-variable annotation against the element type it iterates. An inferred binding
    /// (`var x` / `foreach … as x`) takes the element type directly; an explicit `T x` is validated
    /// against it (`E`-less diagnostic on mismatch). Returns the binding's resolved type.
    pub(in crate::checker) fn bind_loop_var(
        &mut self,
        ty: &crate::ast::Type,
        name: &str,
        elem: &Ty,
        span: Span,
    ) -> Ty {
        if matches!(ty, crate::ast::Type::Infer(_)) {
            elem.clone()
        } else {
            let d = self.resolve_type(ty);
            if !self.ty_assignable(elem, &d) {
                self.err(
                    span,
                    format!("loop variable `{name}` declared `{d}` but iterating `{elem}`"),
                );
            }
            d
        }
    }

    /// `while (cond) { .. }` / `do { .. } while (cond);` (M-mut.3). The condition must be `bool` and
    /// is checked in the loop's *outer* scope (the body's own bindings are not visible to it — true
    /// for do-while too, matching the interpreter's scope-pop-before-retest).
    pub(in crate::checker) fn check_while(
        &mut self,
        cond: &crate::ast::Expr,
        body: &[crate::ast::Stmt],
        _post_cond: bool,
        span: Span,
    ) {
        let ct = self.check_expr(cond);
        if !self.ty_assignable(&ct, &Ty::Bool) {
            self.err(span, format!("loop condition must be `bool`, found `{ct}`"));
        }
        self.push_scope();
        self.loop_depth += 1;
        for s in body {
            self.check_stmt(s);
        }
        self.loop_depth -= 1;
        self.pop_scope();
    }

    /// C-style `for (init; cond; step) { .. }` (M-mut.3). `init`'s binding lives in the loop's own
    /// scope and is visible to `cond`/`step`/`body`; `cond` (if present) must be `bool`.
    pub(in crate::checker) fn check_cfor(
        &mut self,
        init: Option<&crate::ast::Stmt>,
        cond: Option<&crate::ast::Expr>,
        step: Option<&crate::ast::Stmt>,
        body: &[crate::ast::Stmt],
    ) {
        self.push_scope();
        if let Some(s) = init {
            self.check_stmt(s);
        }
        if let Some(c) = cond {
            let ct = self.check_expr(c);
            if !self.ty_assignable(&ct, &Ty::Bool) {
                self.err(
                    Self::expr_span(c),
                    format!("loop condition must be `bool`, found `{ct}`"),
                );
            }
        }
        // `step` runs each iteration (not the loop body) but is checked once; a bare `break`/
        // `continue` in `step` is nonsensical, so it is NOT inside the loop-depth bump.
        if let Some(s) = step {
            self.check_stmt(s);
        }
        self.loop_depth += 1;
        for s in body {
            self.check_stmt(s);
        }
        self.loop_depth -= 1;
        self.pop_scope();
    }
}

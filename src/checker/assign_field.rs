//! `impl Checker` — instance/static FIELD assignment (`o.f = e`, `Class.f = e`), split out of
//! `assign.rs` at its Invariant-13 ratchet (behaviour-identical move, scout row 5j) so the DEC-524
//! constructor-once rule has room.

use super::*;

impl Checker {
    /// `o.f = e` / `this.f = e` shared-mutable instance field set (M-mut.6). The object must resolve
    /// to a concrete class (`this`, a local, or any field path `a.b` whose type is a class — handle
    /// semantics make a write through any binding visible everywhere); the field must exist
    /// (`E-ASSIGN-UNKNOWN`), be declared `mutable` (`E-ASSIGN-IMMUTABLE`), and the value must be
    /// assignable to its (generics-substituted) type (`E-ASSIGN-TYPE`). A `?.` target is rejected
    /// (`E-ASSIGN-TARGET`); nested index-into-field (`this.f[i] = e`) stays deferred to a later slice.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_field_assign(
        &mut self,
        object: &crate::ast::Expr,
        name: &str,
        safe: bool,
        vty: &Ty,
        value: &crate::ast::Expr,
        span: Span,
        target: &crate::ast::Expr,
    ) {
        if safe {
            self.err_coded(
                span,
                "cannot assign through a `?.` safe-access target",
                "E-ASSIGN-TARGET",
                Some("write `o.f = e` on a non-optional receiver".into()),
            );
            return;
        }
        // Static field write `ClassName.field = e` (M-mut.7): the head is a class name (not a local).
        // The field must be a `static mutable` of that class.
        if let crate::ast::Expr::Ident(cls, _) = object {
            if self.lookup_binding(cls).is_none() && self.classes.contains_key(cls) {
                let info = &self.classes[cls];
                // A `const` class constant is immutable — reassigning it is always an error (Feature A).
                if info.consts.contains_key(name) {
                    self.err_coded(
                        span,
                        format!("`{name}` is a constant of `{cls}` and cannot be reassigned"),
                        "E-CONST-REASSIGN",
                        Some("constants are fixed at declaration; use a `static mutable` field for class-level mutable state".into()),
                    );
                    return;
                }
                // Hoist owned copies out of the `info` borrow before any `&mut self` call below
                // (E-* emit + `enforce_member_vis`), so the immutable borrow of `self.classes[cls]`
                // has ended (W0-2: static write visibility joins the existing mut/type checks).
                let static_ty = info.statics.get(name).cloned();
                let is_mut = info.static_mut.contains(name);
                let svis = info.static_vis.get(name).cloned();
                let s_set_vis = info.static_set_vis.get(name).cloned();
                match static_ty {
                    None => {
                        self.err_coded(
                            span,
                            format!("`{cls}` has no static field `{name}` to assign"),
                            "E-ASSIGN-UNKNOWN",
                            None,
                        );
                    }
                    Some(fty) => {
                        // W0-2: a `private`/`protected` static write from outside its scope is
                        // rejected here (E-FIELD-VISIBILITY) — closing the interp ≡ VM ≡ PHP hole (PHP
                        // fatals writing a `private static` from outside).
                        self.enforce_member_vis(svis, name, span, true);
                        // DEC-241: static writes honor `(set)` visibility too.
                        self.enforce_set_vis(s_set_vis, name, span);
                        if !is_mut {
                            self.err_coded(
                                span,
                                format!("static field `{name}` of `{cls}` is immutable and cannot be assigned"),
                                "E-ASSIGN-IMMUTABLE",
                                Some(format!("declare it `static mutable {fty} {name} = …;`")),
                            );
                        } else if !self.ty_assignable(vty, &fty) {
                            self.err_coded(
                                Self::expr_span(value),
                                format!("cannot assign `{vty}` to static field `{name}: {fty}`"),
                                "E-ASSIGN-TYPE",
                                None,
                            );
                        }
                    }
                }
                return;
            }
        }
        let obj_ty = self.check_expr(object);
        let (class, cargs) = match &obj_ty {
            Ty::Error => return,
            Ty::Named(n, cargs) if self.classes.contains_key(n) => (n.clone(), cargs.clone()),
            Ty::Tuple(..) => {
                self.check_tuple_field_assign(object, name, &obj_ty, vty, value, target);
                return;
            }
            other => {
                self.err_coded(
                    Self::expr_span(object),
                    format!("cannot set field `{name}` on non-class `{other}`"),
                    "E-ASSIGN-TARGET",
                    Some("field assignment requires a class instance or a named tuple".into()),
                );
                return;
            }
        };
        // A property hook (M-mut.7b) is resolved before a stored field: `o.name = e` runs its
        // `set`. Writing a hook with no `set` (read-only computed) is `E-HOOK-NO-SET`; otherwise the
        // value must be assignable to the hook's type. A hook is never `mutable`-gated (it has no
        // storage); the set body decides what to mutate.
        if let Some(h) = self
            .classes
            .get(&class)
            .and_then(|info| info.hooks.get(name))
        {
            let (hty, has_set) = (h.ty.clone(), h.has_set);
            if !has_set {
                self.err_coded(
                    span,
                    format!("property `{name}` of `{class}` is read-only (no `set`)"),
                    "E-HOOK-NO-SET",
                    Some("add a `set(T v) { … }` clause to assign it".into()),
                );
            } else if !self.ty_assignable(vty, &hty) {
                self.err_coded(
                    Self::expr_span(value),
                    format!("cannot assign `{vty}` to property `{name}: {hty}`"),
                    "E-ASSIGN-TYPE",
                    None,
                );
            }
            return;
        }
        let fty = match self.classes[&class].fields.get(name).cloned() {
            Some(t) => {
                // Wave 1.1: writing a `private`/`protected` field from outside its scope is rejected
                // here too (PHP enforces visibility on writes — keep the backends in agreement).
                let v = self.classes[&class].field_vis.get(name).cloned();
                self.enforce_member_vis(v, name, span, true);
                // DEC-241: the WRITE additionally honors an asymmetric `(set)` visibility.
                let sv = self.classes[&class].set_vis.get(name).cloned();
                self.enforce_set_vis(sv, name, span);
                apply_subst(&t, &self.class_subst(&class, &cargs))
            }
            None => {
                self.err_coded(
                    span,
                    format!("`{class}` has no field `{name}` to assign"),
                    "E-ASSIGN-UNKNOWN",
                    None,
                );
                return;
            }
        };
        // DEC-524: the class's own constructor may assign an immutable, uninitialized, unpromoted
        // field through `this` — once per path, which `program/ctor_once.rs` checks.
        let ctor_once =
            matches!(object, crate::ast::Expr::This(_)) && self.ctor_once_fields.contains(name);
        if !self.classes[&class].mutable_fields.contains(name) && !ctor_once {
            self.err_coded(
                span,
                format!("field `{name}` of `{class}` is immutable and cannot be assigned"),
                "E-ASSIGN-IMMUTABLE",
                Some(format!(
                    "declare it `mutable` (e.g. `mutable {fty} {name};`) — or, if it has no initializer, assign it once in its own class's constructor"
                )),
            );
            return;
        }
        if !self.ty_assignable(vty, &fty) {
            self.err_coded(
                Self::expr_span(value),
                format!("cannot assign `{vty}` to field `{name}: {fty}`"),
                "E-ASSIGN-TYPE",
                None,
            );
        }
    }

    /// DEC-536 (scout row 5s): `place.f = v` where `place` holds a NAMED tuple. The tuple is a value, so
    /// the assignment rebuilds it with `f` replaced — which is only meaningful through a `mutable` local
    /// place: a local, then `[i]` and named-tuple `.f` steps (Swift struct semantics). The field's
    /// position is recorded by [`Self::check_tuple_field`] against the target `Member`'s own span, so
    /// `rewrite_tuple_fields` turns the target into the positional index chain every backend already
    /// supports (`kept[0].tags = v` → `kept[0][1] = v`).
    fn check_tuple_field_assign(
        &mut self,
        object: &crate::ast::Expr,
        name: &str,
        obj_ty: &Ty,
        vty: &Ty,
        value: &crate::ast::Expr,
        target: &crate::ast::Expr,
    ) {
        let fty = match self.check_tuple_field(obj_ty, name, false, Self::expr_span(target)) {
            Some(Ty::Error) | None => return, // refused by name (positional / unknown field)
            Some(t) => t,
        };
        if !self.value_place_ok(object, "a tuple field") {
            return;
        }
        if !self.ty_assignable(vty, &fty) {
            self.err_coded(
                Self::expr_span(value),
                format!("cannot assign `{vty}` to tuple field `{name}: {fty}`"),
                "E-ASSIGN-TYPE",
                None,
            );
        }
    }

    /// Whether the place contains a `.f` step at all — such a place is type-checked BEFORE its root is
    /// resolved, because only the check records which `.f` steps are tuple fields.
    pub(super) fn place_has_member(mut e: &crate::ast::Expr) -> bool {
        use crate::ast::Expr;
        loop {
            match e {
                Expr::Member { .. } => return true,
                Expr::Index { object, .. } => e = object,
                _ => return false,
            }
        }
    }

    /// DEC-536: `place` (already type-checked) is a mutable VALUE place — a local root, then `[i]` and
    /// named-tuple `.f` steps. Reports `E-ASSIGN-TARGET` / `E-ASSIGN-UNKNOWN` / `E-ASSIGN-IMMUTABLE`
    /// and returns `false` on refusal.
    pub(super) fn value_place_ok(&mut self, place: &crate::ast::Expr, what: &str) -> bool {
        let Some(name) = self.place_root(place).map(str::to_string) else {
            self.err_coded(
                Self::expr_span(place),
                format!("{what} can be assigned only through a local variable, a nested index of one, or a named-tuple field of one"),
                "E-ASSIGN-TARGET",
                Some("a class-field or call base (`this.t.f`, `obj.t.f`, `f().f`) is not an assignable place yet — copy it into a `mutable` local, assign, and store it back".into()),
            );
            return false;
        };
        match self.lookup_binding(&name) {
            None => {
                self.err_coded(
                    Self::expr_span(place),
                    format!("cannot assign into unknown variable `{name}`"),
                    "E-ASSIGN-UNKNOWN",
                    None,
                );
                false
            }
            Some((ty, false)) => {
                self.err_coded(
                    Self::expr_span(place),
                    format!("`{name}` is immutable; {what} of it cannot be set"),
                    "E-ASSIGN-IMMUTABLE",
                    Some(format!(
                        "declare it `mutable` (e.g. `mutable {ty} {name} = …;`)"
                    )),
                );
                false
            }
            Some((_, true)) => true,
        }
    }
}

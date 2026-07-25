//! Member access — `object.field` / `ClassName.field` read checking.

use super::*;

impl Checker {
    pub(in crate::checker) fn check_member(
        &mut self,
        object: &crate::ast::Expr,
        name: &str,
        safe: bool,
        span: Span,
    ) -> Ty {
        // Static field read `ClassName.field` (M-mut.7): the head is a class *name* not shadowed by a
        // local (locals-first), and `?.` makes no sense on a class. Resolved before `check_expr`,
        // which would otherwise reject the bare class name as an unknown variable.
        if !safe {
            if let crate::ast::Expr::Ident(cls, _) = object {
                if self.lookup_binding(cls).is_none() && self.classes.contains_key(cls) {
                    // A `const` class constant (Feature A) is resolved before a static field — it is
                    // class-name-only and visibility-checked. `consts` already carries inherited
                    // entries (merge_inherited), so `Sub.MAX` resolves an inherited `MAX`.
                    if let Some(entry) = self.classes[cls].consts.get(name).cloned() {
                        let visible = match entry.vis {
                            MemberVis::Public => true,
                            MemberVis::Internal => self.internal_member_visible(&entry.owner),
                            MemberVis::Private => {
                                self.cur_class.as_deref() == Some(entry.owner.as_str())
                            }
                            MemberVis::Protected => self
                                .cur_class
                                .as_deref()
                                .is_some_and(|c| self.is_subtype(c, &entry.owner)),
                        };
                        if !visible {
                            let (kind, scope) = match entry.vis {
                                MemberVis::Private => {
                                    ("private", format!("inside `{}`", entry.owner))
                                }
                                MemberVis::Protected => (
                                    "protected",
                                    format!("inside `{}` and its subclasses", entry.owner),
                                ),
                                _ => (
                                    "internal",
                                    format!(
                                        "inside `{}`'s package and its sub-packages",
                                        entry.owner
                                    ),
                                ),
                            };
                            let article = if kind == "internal" { "an" } else { "a" };
                            self.err_coded(
                                span,
                                format!(
                                    "`{name}` is {article} {kind} constant of `{}`",
                                    entry.owner
                                ),
                                "E-CONST-VISIBILITY",
                                Some(format!("it is readable only {scope}")),
                            );
                        }
                        return entry.ty;
                    }
                    return match self.classes[cls].statics.get(name).cloned() {
                        Some(t) => {
                            // W0-2: a `private`/`protected` static read from outside its scope is
                            // rejected here (E-FIELD-VISIBILITY), mirroring the const path above and
                            // the instance-field path below — closing the interp ≡ VM ≡ PHP hole.
                            let v = self.classes[cls].static_vis.get(name).cloned();
                            self.enforce_member_vis(v, name, span, true);
                            t
                        }
                        None => self.err_coded(
                            span,
                            format!("`{cls}` has no static field `{name}`"),
                            "E-STATIC-UNKNOWN",
                            Some(
                                "static fields are declared `static …` and read as `Class.field`"
                                    .into(),
                            ),
                        ),
                    };
                }
            }
        }
        let obj = self.check_expr(object);
        // Peel an optional/null receiver, enforcing the non-null discipline: a plain `.field` on a
        // `T?` is `E-OPT-USE`; `?.field` unwraps and re-wraps the result as optional (M3 S2.3).
        let base = match &obj {
            Ty::Error => return Ty::Error,
            Ty::Null if safe => return Ty::Null, // `null?.field` short-circuits to null
            Ty::Optional(_) | Ty::Null if !safe => {
                return self.err_opt_use(span, name, &obj, "read field");
            }
            Ty::Optional(inner) => (**inner).clone(),
            other => other.clone(),
        };
        let field_ty = match base {
            // DEC-302 backed enum: `s.value` reads the variant's scalar backing (→ the backing Ty).
            // A `.value` on a plain (non-backed) enum is `E-ENUM-NOT-BACKED`; any other field on an
            // enum value has no storage.
            Ty::Named(cls, _) if self.enums.contains_key(&cls) => {
                let backing = self.enums[&cls].backing.clone();
                match (name, backing) {
                    ("value", Some(bt)) => bt,
                    ("value", None) => self.err_coded(
                        span,
                        format!("enum `{cls}` has no `value` — it is not a backed enum"),
                        "E-ENUM-NOT-BACKED",
                        Some("declare a backing type (`enum E: int { … }`) to give variants a `.value`".into()),
                    ),
                    _ => self.err(span, format!("enum `{cls}` has no field `{name}`")),
                }
            }
            Ty::Named(cls, cargs) => {
                // A property hook (M-mut.7b) is resolved before a stored field: `o.name` runs its
                // `get`. Reading a hook with no `get` (write-only) is `E-HOOK-NO-GET`. A hook is not
                // generic (`package Main` only), so no substitution applies to its type.
                if let Some(h) = self.classes.get(&cls).and_then(|info| info.hooks.get(name)) {
                    let (hty, has_get) = (h.ty.clone(), h.has_get);
                    if !has_get {
                        return self.err_coded(
                            span,
                            format!("property `{name}` of `{cls}` is write-only (no `get`)"),
                            "E-HOOK-NO-GET",
                            Some("add a `get => …;` clause to read it".into()),
                        );
                    }
                    return if safe { Self::opt_wrap(hty) } else { hty };
                }
                let found = self
                    .classes
                    .get(&cls)
                    .and_then(|info| info.fields.get(name).cloned());
                match found {
                    // Substitute the class type parameters with the instance's type arguments, so a
                    // `T` field reads at the concrete type (`Box<int>().value : int`) — identity for a
                    // non-generic class (M-RT generics-all). Wave 1.1: a `private`/`protected` field
                    // read from outside its scope is rejected here (closing the run↔PHP hole).
                    Some(t) => {
                        let v = self
                            .classes
                            .get(&cls)
                            .and_then(|i| i.field_vis.get(name).cloned());
                        self.enforce_member_vis(v, name, span, true);
                        apply_subst(&t, &self.class_subst(&cls, &cargs))
                    }
                    // A `const` is class-name-only: reading it through an instance (`c.MAX`) is an
                    // error, with a hint pointing at the correct `ClassName.MAX` form (Feature A).
                    None if self
                        .classes
                        .get(&cls)
                        .is_some_and(|info| info.consts.contains_key(name)) =>
                    {
                        self.err_coded(
                            span,
                            format!("`{name}` is a constant of `{cls}` — read it as `{cls}.{name}`, not through an instance"),
                            "E-CONST-INSTANCE-ACCESS",
                            Some(format!("write `{cls}.{name}`")),
                        )
                    }
                    // A `static` field is class-name-only too: reading it through an instance
                    // (`a.count`) is rejected, mirroring the static-*method*-via-instance rule
                    // (E-STATIC-VIA-INSTANCE) and the const sibling above (UA-0.6). Before this,
                    // `a.staticField` fell through to the generic "has no field" message.
                    None if self
                        .classes
                        .get(&cls)
                        .is_some_and(|info| info.statics.contains_key(name)) =>
                    {
                        self.err_coded(
                            span,
                            format!("`{name}` is a static field of `{cls}` — read it as `{cls}.{name}`, not through an instance"),
                            "E-STATIC-FIELD-VIA-INSTANCE",
                            Some(format!("write `{cls}.{name}`")),
                        )
                    }
                    None => self.err(span, format!("type `{cls}` has no field `{name}`")),
                }
            }
            Ty::Intersection(members) => {
                // Only the lone class member can carry fields (interfaces have none, M-RT S5). Search
                // for the field on the class member; none → E-INTERSECT-NO-MEMBER.
                let mut found: Option<(Ty, String)> = None;
                for m in &members {
                    if let Ty::Named(mn, margs) = m {
                        if let Some(t) = self
                            .classes
                            .get(mn)
                            .and_then(|info| info.fields.get(name).cloned())
                        {
                            found =
                                Some((apply_subst(&t, &self.class_subst(mn, margs)), mn.clone()));
                            break;
                        }
                    }
                }
                match found {
                    Some((t, owner)) => {
                        // DEC-251(c): an intersection receiver must NOT bypass field visibility —
                        // enforce private/protected on the owning class, exactly as the `Ty::Named`
                        // path above (else `x.privateField` on an `I & C`-typed `x` slips through).
                        let v = self
                            .classes
                            .get(&owner)
                            .and_then(|i| i.field_vis.get(name).cloned());
                        self.enforce_member_vis(v, name, span, true);
                        t
                    }
                    None => self.err_coded(
                        span,
                        format!(
                            "no member of `{}` has field `{name}`",
                            Ty::Intersection(members)
                        ),
                        "E-INTERSECT-NO-MEMBER",
                        None,
                    ),
                }
            }
            Ty::Error => Ty::Error,
            other => self.err(span, format!("type `{other}` has no field `{name}`")),
        };
        if safe {
            Self::opt_wrap(field_ty)
        } else {
            field_ty
        }
    }
}

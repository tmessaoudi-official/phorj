//! Collection pass — inheritance: member merging, interface flattening, subtyping.

use super::*;

impl Checker {
    /// Inherit ancestor-class members into each class's [`ClassInfo`] (M-RT S6). Every class's
    /// parents are fully merged first (so transitive members flow down), then a parent's
    /// fields/statics/methods/hooks are copied into the child where the child declares none of its
    /// own — the child's own member wins on a clash. Cycle-safe via the `done` set.
    pub(in crate::checker) fn inherit_class_members(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        let parents: HashMap<String, Vec<String>> = program
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Class(c) => Some((c.name.clone(), c.extends.clone())),
                _ => None,
            })
            .collect();
        let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
        let names: Vec<String> = parents.keys().cloned().collect();
        for name in names {
            self.merge_inherited(&name, &parents, &mut done);
        }
        // M-RT S8: after class inheritance is flattened, merge each `use`d trait's members (methods +
        // fields) into the using class's `ClassInfo`, so calling a trait method / reading a trait field
        // on the class type-checks. A class's own (and inherited) member of the same name wins
        // (`or_insert`); the dispatch table (`class_method_origins`) independently flags any unresolved
        // trait/parent/trait collision as `E-MI-CONFLICT`.
        let class_uses: Vec<(String, Vec<String>)> = program
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Class(c) => Some((
                    c.name.clone(),
                    c.uses.iter().map(|u| u.name.clone()).collect(),
                )),
                _ => None,
            })
            .collect();
        for (cls, used) in class_uses {
            for t in used {
                let Some(tinfo) = self.classes.get(&t).cloned() else {
                    continue; // unknown trait — reported separately as E-USE-UNKNOWN
                };
                let Some(child) = self.classes.get_mut(&cls) else {
                    continue;
                };
                for (k, v) in &tinfo.fields {
                    if !child.fields.contains_key(k) {
                        child.fields.insert(k.clone(), v.clone());
                        if tinfo.mutable_fields.contains(k) {
                            child.mutable_fields.insert(k.clone());
                        }
                    }
                }
                for (k, v) in &tinfo.methods {
                    child.methods.entry(k.clone()).or_insert_with(|| v.clone());
                }
                // Statics-A (2026-06-28): a trait's `static` method is callable as `Class.m()` on the
                // using class — propagate its static-method names (mirrors the `extends` path).
                for sm in &tinfo.static_methods {
                    child.static_methods.insert(sm.clone());
                }
                // Wave 1.1: trait members flatten INTO the using class, so their visibility is
                // re-owned to `cls` (PHP `use` semantics — a trait's `private` member is accessible
                // from the using class, unlike an inherited `private` parent member). Own members win.
                for (k, v) in &tinfo.field_vis {
                    child
                        .field_vis
                        .entry(k.clone())
                        .or_insert((v.0, cls.clone()));
                }
                // DEC-241: a trait's asymmetric SET visibility flattens + re-owns identically.
                for (k, v) in &tinfo.set_vis {
                    child.set_vis.entry(k.clone()).or_insert((v.0, cls.clone()));
                }
                for (k, v) in &tinfo.method_vis {
                    child
                        .method_vis
                        .entry(k.clone())
                        .or_insert((v.0, cls.clone()));
                }
                // M-RT S8: a `use`d trait's `static` field becomes a per-using-class copy — each using
                // class gets its own `ClassName.field` (PHP `use` semantics). Merge it into the class's
                // static table so `C.field` type-checks; the backends seed a distinct slot per class.
                for (k, v) in &tinfo.statics {
                    if !child.statics.contains_key(k) {
                        child.statics.insert(k.clone(), v.clone());
                        if tinfo.static_mut.contains(k) {
                            child.static_mut.insert(k.clone());
                        }
                        // W0-2: a `use`d trait's static is re-owned to the using class (trait `use`
                        // semantics — its `private` static is reachable from the using class),
                        // mirroring the `field_vis` re-own above.
                        if let Some(sv) = tinfo.static_vis.get(k) {
                            child
                                .static_vis
                                .entry(k.clone())
                                .or_insert((sv.0, cls.clone()));
                        }
                        if let Some(sv) = tinfo.static_set_vis.get(k) {
                            child
                                .static_set_vis
                                .entry(k.clone())
                                .or_insert((sv.0, cls.clone()));
                        }
                    }
                }
                // Feature A: a `use`d trait's constants flatten into the using class (own wins).
                for (k, v) in &tinfo.consts {
                    child.consts.entry(k.clone()).or_insert_with(|| v.clone());
                }
                // M-RT S8 (T4): a `use`d trait's property hooks flatten into the using class so
                // `c.hookName` resolves (the backends dispatch the synthetic `$get`/`$set` methods).
                for (k, v) in &tinfo.hooks {
                    child.hooks.entry(k.clone()).or_insert_with(|| v.clone());
                }
                // M-RT S8 (T3): a `use`d trait's constructor becomes the class's ctor signature,
                // replacing any inherited parent ctor (trait wins, PHP P2). The class's own ctor still
                // wins over both (`has_ctor` set in `collect_class`). First trait with a ctor wins for
                // the merge; two unresolved trait ctors are `E-TRAIT-CTOR-COLLISION`.
                if !child.has_ctor && tinfo.has_ctor {
                    child.ctor = tinfo.ctor.clone();
                    child.ctor_defaults = tinfo.ctor_defaults.clone();
                    // DEC-221: the trait ctor's declared throws come with its signature.
                    child.ctor_throws = tinfo.ctor_throws.clone();
                    child.has_ctor = true;
                }
            }
        }
    }

    /// Ensure `name`'s [`ClassInfo`] has merged in all of its (already-merged) parents' members.
    /// Recurses parents-first; memoized via `done`, which also breaks any `extends` cycle.
    pub(in crate::checker) fn merge_inherited(
        &mut self,
        name: &str,
        parents: &HashMap<String, Vec<String>>,
        done: &mut std::collections::HashSet<String>,
    ) {
        if !done.insert(name.to_string()) {
            return;
        }
        let Some(ps) = parents.get(name).cloned() else {
            return;
        };
        for p in &ps {
            self.merge_inherited(p, parents, done);
            let Some(parent_info) = self.classes.get(p).cloned() else {
                continue; // unknown parent — already reported E-EXTEND-UNKNOWN
            };
            let Some(child) = self.classes.get_mut(name) else {
                return;
            };
            for (k, v) in &parent_info.fields {
                if !child.fields.contains_key(k) {
                    child.fields.insert(k.clone(), v.clone());
                    if parent_info.mutable_fields.contains(k) {
                        child.mutable_fields.insert(k.clone());
                    }
                    // DEC-241: inherit the SET visibility, preserving the declaring owner (like
                    // `field_vis` — an inherited `private(set)` stays assignable only in the
                    // parent; a `protected(set)` one is assignable in subclasses of the owner).
                    if let Some(sv) = parent_info.set_vis.get(k) {
                        child.set_vis.entry(k.clone()).or_insert_with(|| sv.clone());
                    }
                }
            }
            for (k, v) in &parent_info.statics {
                if !child.statics.contains_key(k) {
                    child.statics.insert(k.clone(), v.clone());
                    if parent_info.static_mut.contains(k) {
                        child.static_mut.insert(k.clone());
                    }
                    // W0-2: inherit static visibility, **preserving the declaring owner** (like
                    // `field_vis`/consts) — an inherited `private` static is checked against the
                    // parent (not visible from the child), a `protected` one is (child <: owner).
                    if let Some(sv) = parent_info.static_set_vis.get(k) {
                        child
                            .static_set_vis
                            .entry(k.clone())
                            .or_insert_with(|| sv.clone());
                    }
                    if let Some(sv) = parent_info.static_vis.get(k) {
                        child
                            .static_vis
                            .entry(k.clone())
                            .or_insert_with(|| sv.clone());
                    }
                }
            }
            // Feature A: inherit class constants (own/nearer wins). The `owner` in each `ConstEntry`
            // is preserved, so a `private`/`protected` access is checked against the declaring class —
            // `Sub.MAX` resolves an inherited `MAX` and PHP's `Sub::MAX` resolves it the same way.
            for (k, v) in &parent_info.consts {
                child.consts.entry(k.clone()).or_insert_with(|| v.clone());
            }
            for (k, v) in &parent_info.methods {
                child.methods.entry(k.clone()).or_insert_with(|| v.clone());
            }
            // Statics-A (2026-06-28): a `static` method is inherited too — propagate the parent's
            // static-method *names* so `Child.parentStatic()` passes the `static_methods` gate (the
            // signature already flattened via `methods` above; the compiler's `class_method_origins`
            // aliases the dispatch entry, and the interpreter walks ancestors). Mirrors `methods`.
            for sm in &parent_info.static_methods {
                child.static_methods.insert(sm.clone());
            }
            // Wave 1.1: inherit member visibility, **preserving the declaring owner** (like consts) —
            // so an inherited `private` member is checked against the parent (not visible from the
            // child, matching PHP) while a `protected` one is (the child is a subtype of the owner).
            for (k, v) in &parent_info.field_vis {
                child
                    .field_vis
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
            for (k, v) in &parent_info.method_vis {
                child
                    .method_vis
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
            for (k, v) in &parent_info.hooks {
                child.hooks.entry(k.clone()).or_insert_with(|| v.clone());
            }
            // M-RT S6c.2: a class with no own constructor inherits its parents' constructor
            // signature(s) for `ClassName(args)` type-checking — single inheritance takes the one
            // parent's, multiple inheritance **concatenates** every parent's in `extends` order (the
            // orchestrating ctor's params, matching the interpreter's plan + the compiler's flattened
            // descriptor). Parents merge first, so each `parent_info.ctor` is already its *effective*
            // (own-or-inherited) signature; appending across the loop builds the full concatenation. A
            // class declaring its own ctor keeps it (the deferred parent-forwarding case, KNOWN_ISSUES).
            if !child.has_ctor {
                child.ctor.extend(parent_info.ctor.iter().cloned());
                // DEC-236: defaults travel in lockstep with the concatenated signature.
                child
                    .ctor_defaults
                    .extend(parent_info.ctor_defaults.iter().cloned());
                // DEC-221: inherit the parent ctor's declared throws alongside its param signature, so
                // `new Child(args)` propagates whatever the inherited constructor can throw.
                child
                    .ctor_throws
                    .extend(parent_info.ctor_throws.iter().cloned());
                // Inherit the constructor's visibility + declaring owner (Batch A) from the first
                // parent in `extends` order — so `new Child(...)` is gated by the inherited ctor's
                // visibility (an inherited `private`/`protected __construct` blocks external
                // construction, matching PHP). `ctor_owner` defaults to the child's own name, so a
                // public-by-default chain is unaffected; only a non-public parent ctor changes it.
                if child.ctor_owner == name && parent_info.ctor_vis != MemberVis::Public {
                    child.ctor_vis = parent_info.ctor_vis;
                    child.ctor_owner = parent_info.ctor_owner.clone();
                }
            }
        }
    }

    /// True if `name`'s `extends` chain reaches `name` again (a cycle). Visited-guarded.
    pub(in crate::checker) fn iface_in_cycle(
        &self,
        name: &str,
        stack: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        fn walk(
            this: &Checker,
            cur: &str,
            target: &str,
            seen: &mut std::collections::BTreeSet<String>,
        ) -> bool {
            let Some(info) = this.interfaces.get(cur) else {
                return false;
            };
            for parent in &info.extends {
                if parent == target {
                    return true;
                }
                if seen.insert(parent.clone()) && walk(this, parent, target, seen) {
                    return true;
                }
            }
            false
        }
        walk(self, name, name, stack)
    }

    /// An interface's flattened method set: its own methods plus every (transitive) parent's,
    /// the child's signature winning on a name clash. Cycle-guarded.
    pub(in crate::checker) fn iface_flat_methods(
        &self,
        name: &str,
    ) -> Vec<(String, (Vec<Ty>, Ty))> {
        let mut acc: HashMap<String, (Vec<Ty>, Ty)> = HashMap::new();
        let mut seen = std::collections::BTreeSet::new();
        self.iface_collect_methods(name, &mut acc, &mut seen);
        let mut out: Vec<_> = acc.into_iter().collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub(in crate::checker) fn iface_collect_methods(
        &self,
        name: &str,
        acc: &mut HashMap<String, (Vec<Ty>, Ty)>,
        seen: &mut std::collections::BTreeSet<String>,
    ) {
        if !seen.insert(name.to_string()) {
            return;
        }
        let Some(info) = self.interfaces.get(name) else {
            return;
        };
        // Parents first, so a child interface's own signature overrides on a clash.
        for parent in &info.extends {
            self.iface_collect_methods(parent, acc, seen);
        }
        for (m, sig) in &info.methods {
            acc.insert(m.clone(), (sig.params.clone(), sig.ret.clone()));
        }
    }

    /// A class method conforms to an interface signature when arities match and each parameter type
    /// and the return type are equal (exact — no variance this slice, matching `assignable`'s
    /// function rule).
    /// `have` is the class's overload set for the method name (M-RT overloading): conformance holds
    /// when *any* overload matches the interface signature exactly.
    pub(in crate::checker) fn sig_conforms(&self, have: &[FnSig], want: &(Vec<Ty>, Ty)) -> bool {
        have.iter().any(|h| {
            h.params.len() == want.0.len()
                && h.params.iter().zip(&want.0).all(|(a, b)| a == b)
                && h.ret == want.1
        })
    }

    /// Nominal subtyping for assignability and `instanceof`: `a` is a subtype of `b` when they are
    /// equal, when class `a` implements interface `b` (transitively, via [`Self::class_implements`]),
    /// when class `a` extends class `b` (transitively, via [`Self::class_supertypes`] — M-RT S6), or
    /// when interface `a` extends interface `b` (transitively).
    pub(in crate::checker) fn is_subtype(&self, a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }
        if self
            .class_implements
            .get(a)
            .is_some_and(|ifaces| ifaces.iter().any(|i| i == b))
        {
            return true;
        }
        // class `a` extends class `b` transitively (M-RT S6)?
        if self
            .class_supertypes
            .get(a)
            .is_some_and(|sup| sup.iter().any(|s| s == b))
        {
            return true;
        }
        // interface `a` extends `b` transitively?
        if self.interfaces.contains_key(a) {
            let mut seen = std::collections::BTreeSet::new();
            return self.iface_in_cycle_to(a, b, &mut seen);
        }
        false
    }

    pub(in crate::checker) fn iface_in_cycle_to(
        &self,
        cur: &str,
        target: &str,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        let Some(info) = self.interfaces.get(cur) else {
            return false;
        };
        for parent in &info.extends {
            if parent == target {
                return true;
            }
            if seen.insert(parent.clone()) && self.iface_in_cycle_to(parent, target, seen) {
                return true;
            }
        }
        false
    }

    /// Context-aware assignability: [`Ty::assignable`] plus this checker's nominal subtyping.
    pub(in crate::checker) fn ty_assignable(&self, from: &Ty, to: &Ty) -> bool {
        // DEC-211: a BOUNDED type parameter is assignable to its bound interface (and anything the
        // bound is assignable to) — so `T` where `T: Comparable` satisfies a `Comparable` parameter.
        // The same-param `T <: T` case falls through to the structural path below.
        if let Ty::Param(p) = from {
            if let Some((_, iface)) = self.active_type_param_bounds.iter().find(|(n, _)| n == p) {
                if self.ty_assignable(&Ty::Named(iface.clone(), Vec::new()), to) {
                    return true;
                }
            }
        }
        Ty::assignable_with(from, to, &|a, b| self.is_subtype(a, b))
    }
}

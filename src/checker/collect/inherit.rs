//! Collection pass — inheritance: member merging (class `extends` and trait `use`).

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
            self.merge_inherited_pass(&name, &parents, &mut done, false);
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
        for (cls, used) in &class_uses {
            for t in used {
                self.trait_users
                    .entry(t.clone())
                    .or_default()
                    .push(cls.clone());
            }
        }
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
                // DEC-331 D9: a trait method carrying `#[Invoke]`/`#[ToString]` flattens its role into
                // the using class too (a no-op unless the trait declares one). Own roles win.
                for im in &tinfo.invoke_methods {
                    if !child.invoke_methods.contains(im) {
                        child.invoke_methods.push(im.clone());
                    }
                }
                if child.to_string_method.is_none() {
                    child.to_string_method = tinfo.to_string_method.clone();
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
                // DEC-379: the per-overload vector inherits ALONGSIDE `method_vis`, or a
                // subclass would lose the data conformance needs and the bypass would reopen
                // for inherited overload sets.
                for (k, v) in &tinfo.method_overload_vis {
                    child
                        .method_overload_vis
                        .entry(k.clone())
                        .or_insert_with(|| v.clone());
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
                    // DEC-297: the param names travel with the signature, so `new U(k: 3)` resolves.
                    child.ctor_param_names = tinfo.ctor_param_names.clone();
                    // DEC-221: the trait ctor's declared throws come with its signature.
                    child.ctor_throws = tinfo.ctor_throws.clone();
                    child.has_ctor = true;
                }
            }
        }
        // Plan row 4c item 5: a SUBCLASS of a trait-using class inherits what its parent composed (the
        // runtime's dispatch table and `ast::ctor_plan` already reach a parent's trait). The first pass
        // ran before any trait merged, so inherit again: every member copy is `or_insert`-shaped, so
        // nothing the first pass placed changes, and a class with no own constructor rebuilds its
        // concatenated signature from its parents' final ones (`rebuild_ctor`).
        let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
        let names: Vec<String> = parents.keys().cloned().collect();
        for name in names {
            self.merge_inherited_pass(&name, &parents, &mut done, true);
        }
    }

    /// Ensure `name`'s [`ClassInfo`] has merged in all of its (already-merged) parents' members.
    /// Recurses parents-first; memoized via `done`, which also breaks any `extends` cycle.
    /// `rebuild_ctor` (the second pass, after traits merged) first clears a no-own-ctor class's
    /// concatenated constructor signature so the parents' final ones rebuild it instead of appending
    /// a second copy — the one non-`or_insert` step, which makes the pass safe to run twice.
    pub(in crate::checker) fn merge_inherited_pass(
        &mut self,
        name: &str,
        parents: &HashMap<String, Vec<String>>,
        done: &mut std::collections::HashSet<String>,
        rebuild_ctor: bool,
    ) {
        if !done.insert(name.to_string()) {
            return;
        }
        let Some(ps) = parents.get(name).cloned() else {
            return;
        };
        if rebuild_ctor {
            if let Some(child) = self.classes.get_mut(name) {
                if !child.has_ctor && !ps.is_empty() {
                    child.ctor.clear();
                    child.ctor_defaults.clear();
                    child.ctor_param_names.clear();
                    child.ctor_throws.clear();
                }
            }
        }
        for p in &ps {
            self.merge_inherited_pass(p, parents, done, rebuild_ctor);
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
            // DEC-331 D9: `#[Invoke]`/`#[ToString]` roles inherit with the method (spec §2). A parent's
            // `#[Invoke]` names extend the child's callable set (union, deduped); a child inheriting a
            // parent's `#[ToString]` gets it only when it declares none of its own (an override keeps
            // the role via its own declaration — harvested in `collect_class`). Mirrors `methods`.
            for im in &parent_info.invoke_methods {
                if !child.invoke_methods.contains(im) {
                    child.invoke_methods.push(im.clone());
                }
            }
            if child.to_string_method.is_none() {
                child.to_string_method = parent_info.to_string_method.clone();
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
            // DEC-379: same reason as the trait path above.
            for (k, v) in &parent_info.method_overload_vis {
                child
                    .method_overload_vis
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
                // DEC-297: param names travel in lockstep too, so a named `new Child(x: …)` on an
                // inherited ctor resolves to the right slot.
                child
                    .ctor_param_names
                    .extend(parent_info.ctor_param_names.iter().cloned());
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
}

//! Collection pass — traits and interfaces: declaration collection + interface-graph checks.

use super::*;

impl Checker {
    /// M-RT S8: collect a trait by reusing the class machinery. A synthetic `ClassDecl` carries the
    /// trait's members into a [`ClassInfo`] (keyed by the trait name) so the trait body type-checks and
    /// the trait's members can be merged into each using class. Marked `is_abstract` so an abstract
    /// *requirement* method doesn't trip the concrete-class unimpl check on the trait itself; recorded
    /// in [`Self::traits`] so the name is rejected wherever a *type* is expected (a trait is reuse, not
    /// a type), and so construction (`Loud()`) is caught by the abstract-instantiate guard.
    pub(in crate::checker) fn collect_trait(&mut self, t: &crate::ast::TraitDecl) {
        let synthetic = crate::ast::ClassDecl {
            vis: crate::ast::Visibility::Public,
            attrs: Vec::new(), // synthetic trait→class carries no attributes
            name: t.name.clone(),
            type_params: Vec::new(),
            extends: Vec::new(),
            implements: Vec::new(),
            open: false,
            is_abstract: true,
            sealed: false,
            resolutions: Vec::new(),
            uses: Vec::new(),
            members: t.members.clone(),
            foreign: false,
            span: t.span,
        };
        self.collect_class(&synthetic);
        self.traits.insert(t.name.clone());
    }

    pub(in crate::checker) fn collect_interface(&mut self, i: &crate::ast::InterfaceDecl) {
        if is_builtin_type_name(&i.name) {
            self.err(
                i.span,
                format!("cannot redefine built-in type `{}`", i.name),
            );
            return;
        }
        if !self.prebound.contains(&i.name)
            && (self.classes.contains_key(&i.name)
                || self.enums.contains_key(&i.name)
                || self.interfaces.contains_key(&i.name))
        {
            self.err_coded(
                i.span,
                format!("type `{}` is already defined", i.name),
                "E-DUP-TYPE",
                Some("rename one declaration — a class/enum/interface/trait/type name must be unique".into()),
            );
            return;
        }
        // W5-3: record a `sealed` interface so a `match` over it is exhaustive over its whole-program
        // permitted implementors (checked in `check_match`; compile-time-only).
        if i.sealed {
            self.sealed_types.insert(i.name.clone());
        }
        // Register the name first so a method signature may reference the interface itself.
        self.interfaces.insert(
            i.name.clone(),
            InterfaceInfo {
                methods: HashMap::new(),
                extends: i.extends.clone(),
            },
        );
        let mut methods = HashMap::new();
        for m in &i.methods {
            if methods.contains_key(&m.name) {
                self.err(
                    m.span,
                    format!("duplicate method `{}` in interface `{}`", m.name, i.name),
                );
                continue;
            }
            let params = m.params.iter().map(|p| self.resolve_type(&p.ty)).collect();
            // S0b: an interface method signature must declare its return type too (it never flows
            // through `check_function`, so enforce it here at collection).
            if m.ret.is_none() {
                self.err_coded(
                    m.span,
                    format!("interface method `{}` must declare a return type", m.name),
                    "E-MISSING-RETURN-TYPE",
                    Some(
                        "every function and method declares its return type — add `-> void` for a side-effecting method".into(),
                    ),
                );
            }
            let ret = match &m.ret {
                Some(t) => self.resolve_type(t),
                None => Ty::Void,
            };
            methods.insert(
                m.name.clone(),
                FnSig {
                    params,
                    // Default parameters are free-function-only in v1 (methods are a deferral); an
                    // interface method carries none.
                    defaults: vec![None; m.params.len()],
                    ret,
                    type_params: Vec::new(),
                    // Interface-method throws are not enforced through dynamic dispatch this slice
                    // (a documented deferral); keep the set empty so no call site mis-discharges.
                    throws: Vec::new(),
                    is_static: false,
                },
            );
        }
        self.interfaces.get_mut(&i.name).unwrap().methods = methods;
    }

    /// Validate the interface graph and class conformance, then build [`Self::class_implements`].
    ///
    /// Reports `E-IFACE-CYCLE` (an `extends` cycle), `E-IFACE-IMPL` (a name in `implements`/`extends`
    /// that is not a declared interface), `E-IFACE-UNIMPL` (a class missing an interface method), and
    /// `E-IFACE-SIG` (a class method whose signature does not match the interface's).
    pub(in crate::checker) fn check_interface_graph(&mut self, program: &crate::ast::Program) {
        use crate::ast::Item;
        // Always safe to compute (the shared fn is cycle-guarded); diagnostics below catch malformed
        // graphs, and the backends only run after a clean check, so a cyclic table never reaches them.
        self.class_implements = crate::ast::class_implements(program);
        self.class_supertypes = crate::ast::class_supertypes(program);

        // Class `extends` targets must be `open` classes; detect cycles (M-RT S6).
        let class_open: std::collections::HashMap<&str, bool> = program
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Class(c) => Some((c.name.as_str(), c.open)),
                _ => None,
            })
            .collect();
        for item in &program.items {
            if let Item::Class(c) = item {
                if self
                    .class_supertypes
                    .get(&c.name)
                    .is_some_and(|s| s.contains(&c.name))
                {
                    self.err_coded(
                        c.span,
                        format!("class `{}` is part of an `extends` cycle", c.name),
                        "E-MI-CYCLE",
                        Some("a class may not extend itself transitively".into()),
                    );
                    continue; // skip per-parent checks for a cyclic class (avoids noise)
                }
                for parent in &c.extends {
                    if !self.classes.contains_key(parent) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` extends `{parent}`, which is not a class",
                                c.name
                            ),
                            "E-EXTEND-UNKNOWN",
                            Some(
                                "`extends` lists parent classes; use `implements` for interfaces"
                                    .into(),
                            ),
                        );
                    } else if !class_open.get(parent.as_str()).copied().unwrap_or(false) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` cannot extend `{parent}`, which is not `open`",
                                c.name
                            ),
                            "E-EXTEND-FINAL",
                            Some(format!(
                                "mark the parent `open class {parent}` to allow extension"
                            )),
                        );
                    }
                }
            }
        }

        // Inherit each class's ancestors' members into its `ClassInfo` (child wins on a clash),
        // before interface-conformance below — so an inherited method can satisfy an interface.
        self.inherit_class_members(program);

        // M-RT S6b: a `rename P.m as n` clause exposes parent `P`'s method `m` on the child under the
        // new name `n`, so a `child.n()` call type-checks (the backends dispatch it via the shared
        // origin table). `use`/`exclude` keep method names unchanged, so they need no signature edit.
        for item in &program.items {
            if let Item::Class(c) = item {
                for r in &c.resolutions {
                    if let crate::ast::Resolution::Rename {
                        parent,
                        method,
                        as_name,
                        ..
                    } = r
                    {
                        if let Some(sigs) = self
                            .classes
                            .get(parent)
                            .and_then(|p| p.methods.get(method))
                            .cloned()
                        {
                            if let Some(child) = self.classes.get_mut(&c.name) {
                                child.methods.entry(as_name.clone()).or_insert(sigs);
                            }
                        }
                    }
                }
            }
        }

        // M-RT S6: a method that overrides an ancestor's method requires that ancestor's method to be
        // `open` (final-by-default), else `E-OVERRIDE-FINAL`. (Signature-variance checking on override
        // is deferred — see KNOWN_ISSUES.) `method_open[(class, name)]` is true if the class declares
        // that name with at least one `open` overload.
        let mut method_open: std::collections::HashMap<(String, String), bool> =
            std::collections::HashMap::new();
        // Shared method-resolution order (nearest-first BFS over *every* parent) — the same table the
        // backends dispatch through, so the override check sees the exact ancestor a call would (M-RT
        // S6b: multi-parent, not just the first-parent chain).
        let mro = crate::ast::class_mro(program);
        for item in &program.items {
            if let Item::Class(c) = item {
                for m in &c.members {
                    if let crate::ast::ClassMember::Method(f) = m {
                        // An `abstract` method is implicitly `open` (it exists to be implemented).
                        let is_open = f.modifiers.contains(&crate::ast::Modifier::Open)
                            || f.modifiers.contains(&crate::ast::Modifier::Abstract);
                        method_open
                            .entry((c.name.clone(), f.name.clone()))
                            .and_modify(|v| *v = *v || is_open)
                            .or_insert(is_open);
                    }
                }
            }
        }
        for item in &program.items {
            if let Item::Class(c) = item {
                let mut checked: std::collections::BTreeSet<&str> =
                    std::collections::BTreeSet::new();
                for m in &c.members {
                    let crate::ast::ClassMember::Method(f) = m else {
                        continue;
                    };
                    if !checked.insert(f.name.as_str()) {
                        continue; // one diagnostic per overridden name
                    }
                    // Nearest ancestor (across every parent, nearest-first) that declares this name.
                    for anc in mro.get(&c.name).into_iter().flatten() {
                        if let Some(&open) = method_open.get(&(anc.clone(), f.name.clone())) {
                            if !open {
                                self.err_coded(
                                    f.span,
                                    format!(
                                        "method `{}` overrides `{anc}`'s `{}`, which is not `open`",
                                        f.name, f.name
                                    ),
                                    "E-OVERRIDE-FINAL",
                                    Some(format!(
                                        "mark it `open function {}(…)` on `{anc}` to allow overriding",
                                        f.name
                                    )),
                                );
                            }
                            // M-DX S1 (soundness hole B): an override's return type must be a subtype
                            // of the overridden one (covariance). A wider/unrelated return used to
                            // type-check clean, then store a wrong-typed value on the Rust backends —
                            // and *fatal* in transpiled PHP (`Sub::k(): string` vs `Base::k(): int`).
                            // Scoped to the simple case: single (non-overloaded), non-generic
                            // signatures on both sides. Parameter contravariance and overloaded/
                            // generic overrides remain documented deferrals (KNOWN_ISSUES).
                            let rets = {
                                let child = self
                                    .classes
                                    .get(&c.name)
                                    .and_then(|ci| ci.methods.get(&f.name));
                                let parent =
                                    self.classes.get(anc).and_then(|ci| ci.methods.get(&f.name));
                                match (child, parent) {
                                    (Some(cs), Some(ps))
                                        if cs.len() == 1
                                            && ps.len() == 1
                                            && cs[0].type_params.is_empty()
                                            && ps[0].type_params.is_empty() =>
                                    {
                                        Some((cs[0].ret.clone(), ps[0].ret.clone()))
                                    }
                                    _ => None,
                                }
                            };
                            if let Some((child_ret, parent_ret)) = rets {
                                if !self.ty_assignable(&child_ret, &parent_ret) {
                                    self.err_coded(
                                        f.span,
                                        format!(
                                            "method `{}` overrides `{anc}`'s `{}` but returns \
                                             `{child_ret}`, which is not assignable to the \
                                             overridden return type `{parent_ret}`",
                                            f.name, f.name
                                        ),
                                        "E-OVERRIDE-SIG",
                                        Some(format!(
                                            "make `{}`'s return type `{parent_ret}` or a subtype of it",
                                            f.name
                                        )),
                                    );
                                }
                            }
                            break; // the nearest declaration decides
                        }
                    }
                }
            }
        }

        // M-RT S6b: an unresolved cross-parent method collision is `E-MI-CONFLICT`. The shared origin
        // resolver returns every name a class inherits from ≥2 distinct parents without a `use`/
        // `rename`/`exclude` clause (or own override) to disambiguate. A clean program produces an
        // empty list; the backends then dispatch through the same resolved table.
        let (origins, conflicts) = crate::ast::class_method_origins(program);
        for (class, name, span) in conflicts {
            self.err_coded(
                span,
                format!(
                    "method `{name}` is inherited from more than one parent of class `{class}`"
                ),
                "E-MI-CONFLICT",
                Some(format!(
                    "resolve it: `use P.{name}` to pick a parent, `rename P.{name} as <new>` to keep \
                     both, `exclude P.{name}` to drop one, or override `function {name}(…)` in `{class}`"
                )),
            );
        }

        // M-RT S6c.1: a same-named instance field inherited from ≥2 distinct parents is
        // `E-MI-FIELD-CONFLICT`. PHP has no `insteadof` for properties, so unlike a method collision
        // it can be resolved *only* by the child redeclaring the field (or renaming it in a parent).
        // A diamond-shared field (both arms reach the same declaring class) auto-merges, like methods.
        for (class, name, span) in crate::ast::class_field_conflicts(program) {
            self.err_coded(
                span,
                format!("field `{name}` is inherited from more than one parent of class `{class}`"),
                "E-MI-FIELD-CONFLICT",
                Some(format!(
                    "PHP has no `insteadof` for properties — redeclare `{name}` in `{class}` (or \
                     rename it in a parent) to resolve the collision"
                )),
            );
        }

        // M-RT S6b: abstract-method bookkeeping. `abstract_methods[(class, name)]` is set when a class
        // declares a bodyless `abstract function name`; `E-OPEN-STATIC` rejects a method that is both
        // `open` and `static` (statics are not virtual, so overridability is meaningless).
        let mut abstract_methods: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for item in &program.items {
            if let Item::Class(c) = item {
                for m in &c.members {
                    if let crate::ast::ClassMember::Method(f) = m {
                        if f.modifiers.contains(&crate::ast::Modifier::Abstract) {
                            abstract_methods.insert((c.name.clone(), f.name.clone()));
                        }
                        if f.modifiers.contains(&crate::ast::Modifier::Open)
                            && f.modifiers.contains(&crate::ast::Modifier::Static)
                        {
                            self.err_coded(
                                f.span,
                                format!("method `{}` is both `open` and `static`", f.name),
                                "E-OPEN-STATIC",
                                Some(
                                    "static methods are not virtual; drop `open` or `static`"
                                        .into(),
                                ),
                            );
                        }
                    }
                }
            }
            // M-RT S8: a trait's abstract method is a *requirement* on every using class. Recording it
            // under the trait name means the shared origins table (which maps a using class's method to
            // its `(trait, m)` origin) makes the same `E-ABSTRACT-UNIMPL` check below fire when a using
            // class leaves the requirement unmet.
            if let Item::Trait(t) = item {
                for m in &t.members {
                    if let crate::ast::ClassMember::Method(f) = m {
                        if f.modifiers.contains(&crate::ast::Modifier::Abstract) {
                            abstract_methods.insert((t.name.clone(), f.name.clone()));
                        }
                    }
                }
            }
        }
        // M-RT S8: every `use T;` must name a declared trait — not a class, interface, or unknown.
        for item in &program.items {
            if let Item::Class(c) = item {
                for u in &c.uses {
                    if !self.traits.contains(&u.name) {
                        let hint = if self.classes.contains_key(&u.name) {
                            "that name is a class — `use` composes a `trait`, `extends` inherits a class"
                        } else {
                            "declare it with `trait <Name> { … }`"
                        };
                        self.err_coded(
                            u.span,
                            format!("unknown trait `{}` in a `use` clause", u.name),
                            "E-USE-UNKNOWN",
                            Some(hint.into()),
                        );
                    }
                }
            }
        }
        // M-RT S8 (T3): trait-constructor footguns become clean ahead-of-time diagnostics (D5/D6/D8).
        for item in &program.items {
            let Item::Class(c) = item else { continue };
            let has_own_ctor = c
                .members
                .iter()
                .any(|m| matches!(m, crate::ast::ClassMember::Constructor { .. }));
            // Used traits (known + declaring a constructor), in source order.
            let trait_ctors: Vec<&str> = c
                .uses
                .iter()
                .filter(|u| {
                    self.classes
                        .get(&u.name)
                        .is_some_and(|t| t.has_ctor && self.traits.contains(&u.name))
                })
                .map(|u| u.name.as_str())
                .collect();
            if has_own_ctor {
                // The class's own ctor wins; any trait ctor is dead unless aliased (PHP P1).
                if let Some(t) = trait_ctors.first() {
                    self.warn_coded(
                        c.span,
                        format!(
                            "class `{}` declares its own constructor, so trait `{t}`'s constructor is never run",
                            c.name
                        ),
                        "W-TRAIT-CTOR-SHADOWED",
                        Some("remove the class ctor to use the trait's, or keep it (the trait ctor is intentionally shadowed)".into()),
                    );
                }
            } else if trait_ctors.len() >= 2 {
                // Two trait constructors collide — PHP would fatal; require a resolution.
                self.err_coded(
                    c.span,
                    format!(
                        "class `{}` composes constructors from multiple traits ({})",
                        c.name,
                        trait_ctors.join(", ")
                    ),
                    "E-TRAIT-CTOR-COLLISION",
                    Some("a class can compose at most one trait constructor; give one its own ctor or drop a trait".into()),
                );
            } else if trait_ctors.len() == 1 {
                // One trait ctor + a parent that has a ctor: the trait ctor wins, the parent's is not
                // auto-run (PHP P2) — surface the silent skip.
                let parent_has_ctor = c
                    .extends
                    .iter()
                    .any(|p| !crate::ast::ctor_plan(program, p).is_empty());
                if parent_has_ctor {
                    self.warn_coded(
                        c.span,
                        format!(
                            "class `{}` runs trait `{}`'s constructor; the parent constructor is not run",
                            c.name, trait_ctors[0]
                        ),
                        "W-TRAIT-CTOR-PARENT-SKIPPED",
                        Some("call the parent's initializer explicitly if it must run, or give the class its own ctor".into()),
                    );
                }
            }
        }
        // M-RT S6b: a concrete class must implement every abstract method it declares or inherits. The
        // shared dispatch table resolves each callable name to the body it runs; if that body is still
        // an abstract signature on a *non-abstract* class, the method is unimplemented. This one check
        // covers both "a concrete class declares an abstract method" (origin is itself) and "a concrete
        // subclass fails to override an inherited abstract method" (origin is an ancestor).
        if !abstract_methods.is_empty() {
            for item in &program.items {
                if let Item::Class(c) = item {
                    if c.is_abstract {
                        continue; // an abstract class may carry unimplemented abstract methods
                    }
                    let mut reported: std::collections::BTreeSet<&str> =
                        std::collections::BTreeSet::new();
                    for ((cls, name), (oc, om)) in &origins {
                        if cls != &c.name {
                            continue;
                        }
                        if abstract_methods.contains(&(oc.clone(), om.clone()))
                            && reported.insert(name.as_str())
                        {
                            self.err_coded(
                                c.span,
                                format!(
                                    "class `{}` must implement abstract method `{name}` from `{oc}`",
                                    c.name
                                ),
                                "E-ABSTRACT-UNIMPL",
                                Some(format!(
                                    "provide `function {name}(…)` in `{}`, or declare `{}` `abstract`",
                                    c.name, c.name
                                )),
                            );
                        }
                    }
                }
            }
        }

        // `extends` targets must be interfaces; detect cycles.
        for item in &program.items {
            if let Item::Interface(i) = item {
                for parent in &i.extends {
                    if !self.interfaces.contains_key(parent) {
                        self.err_coded(
                            i.span,
                            format!(
                                "interface `{}` extends `{parent}`, which is not an interface",
                                i.name
                            ),
                            "E-IFACE-IMPL",
                            Some("`extends` on an interface lists other interfaces".into()),
                        );
                    }
                }
                let mut visited = std::collections::BTreeSet::new();
                if self.iface_in_cycle(&i.name, &mut visited) {
                    self.err_coded(
                        i.span,
                        format!("interface `{}` is part of an `extends` cycle", i.name),
                        "E-IFACE-CYCLE",
                        Some("interfaces may not extend themselves transitively".into()),
                    );
                }
            }
        }

        // Class conformance: every interface method (own + inherited) must be provided.
        for item in &program.items {
            if let Item::Class(c) = item {
                for iface in &c.implements {
                    if !self.interfaces.contains_key(iface) {
                        self.err_coded(
                            c.span,
                            format!(
                                "class `{}` implements `{iface}`, which is not an interface",
                                c.name
                            ),
                            "E-IFACE-IMPL",
                            Some("`implements` lists declared interfaces".into()),
                        );
                        continue;
                    }
                    let required = self.iface_flat_methods(iface);
                    for (mname, sig) in &required {
                        match self
                            .classes
                            .get(&c.name)
                            .and_then(|ci| ci.methods.get(mname))
                        {
                            None => {
                                self.err_coded(
                                    c.span,
                                    format!(
                                        "class `{}` does not implement method `{mname}` required by interface `{iface}`",
                                        c.name
                                    ),
                                    "E-IFACE-UNIMPL",
                                    Some(format!("add `function {mname}(…)` to `{}`", c.name)),
                                );
                            }
                            Some(have) => {
                                if !self.sig_conforms(have, sig) {
                                    self.err_coded(
                                        c.span,
                                        format!(
                                            "class `{}` method `{mname}` does not match interface `{iface}`'s signature",
                                            c.name
                                        ),
                                        "E-IFACE-SIG",
                                        Some("the parameter types and return type must match the interface".into()),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

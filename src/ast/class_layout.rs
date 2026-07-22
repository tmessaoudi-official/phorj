//! Class analyses — layout: field conflicts, field layout, consts, initializers.

use super::*;

/// M-RT S6c — instance-field collision detection, the field analog of [`class_method_origins`].
/// Returns every `(class, field, class_span)` where a class inherits a same-named instance field from
/// **two or more distinct declaring origins** without redeclaring it — the checker reports each as
/// `E-MI-FIELD-CONFLICT`. Unlike methods there are no resolution clauses (PHP has no `insteadof` for
/// properties), so a child can resolve a collision only by redeclaring the field itself.
///
/// "Instance field" = an explicit non-`static` `Field` member plus every promoted constructor
/// parameter (one carrying a `public`/`private`/`protected` modifier — these become fields, EV-4).
/// A diamond auto-merges exactly like a shared method: a field reached through two arms that resolve
/// to the *same* declaring class dedups (no conflict). Static fields are out of scope this slice.
pub fn class_field_conflicts(program: &Program) -> Vec<(String, String, Span)> {
    use std::collections::{BTreeMap, BTreeSet};

    struct Ctx {
        decl: BTreeMap<String, BTreeSet<String>>,
        extends: BTreeMap<String, Vec<String>>,
        spans: BTreeMap<String, Span>,
        memo: BTreeMap<String, BTreeMap<String, String>>,
        conflicts: Vec<(String, String, Span)>,
        in_progress: BTreeSet<String>,
    }

    impl Ctx {
        /// Resolve `c`'s flat instance-field table: each field name → its single declaring origin
        /// class. Own fields win (redeclare); a name arriving from ≥2 distinct origins is recorded as
        /// a conflict (a deterministic pick still lands in the table so the build can continue).
        fn resolve(&mut self, c: &str) -> BTreeMap<String, String> {
            if let Some(m) = self.memo.get(c) {
                return m.clone();
            }
            if !self.in_progress.insert(c.to_string()) {
                return BTreeMap::new(); // `extends` cycle — `E-MI-CYCLE` reported elsewhere
            }
            let mut map: BTreeMap<String, String> = BTreeMap::new();
            // Own fields win over anything inherited (the child redeclaring resolves a collision).
            if let Some(fs) = self.decl.get(c).cloned() {
                for f in fs {
                    map.insert(f, c.to_string());
                }
            }
            // Gather each direct parent's resolved fields, tracking the true declaring origin so a
            // diamond dedups by origin.
            let mut contrib: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for p in self.extends.get(c).cloned().unwrap_or_default() {
                for (name, origin) in self.resolve(&p) {
                    if map.contains_key(&name) {
                        continue; // redeclared by C itself
                    }
                    contrib.entry(name).or_default().insert(origin);
                }
            }
            for (name, origins) in contrib {
                if map.contains_key(&name) {
                    continue;
                }
                let mut it = origins.into_iter();
                if let Some(first) = it.next() {
                    if it.next().is_some() {
                        let span = self.spans.get(c).copied().unwrap_or(Span {
                            start: 0,
                            len: 0,
                            line: 1,
                            col: 1,
                        });
                        self.conflicts.push((c.to_string(), name.clone(), span));
                    }
                    map.insert(name, first); // deterministic pick (sorted-first)
                }
            }
            self.in_progress.remove(c);
            self.memo.insert(c.to_string(), map.clone());
            map
        }
    }

    let mut ctx = Ctx {
        decl: BTreeMap::new(),
        extends: BTreeMap::new(),
        spans: BTreeMap::new(),
        memo: BTreeMap::new(),
        conflicts: Vec::new(),
        in_progress: BTreeSet::new(),
    };
    for item in &program.items {
        if let Item::Class(c) = item {
            let mut fs = BTreeSet::new();
            for m in &c.members {
                match m {
                    ClassMember::Field {
                        name, modifiers, ..
                    } if !modifiers.contains(&Modifier::Static) => {
                        fs.insert(name.clone());
                    }
                    ClassMember::Constructor { params, .. } => {
                        for p in params {
                            if p.modifiers.iter().any(|m| {
                                matches!(
                                    m,
                                    Modifier::Public | Modifier::Private | Modifier::Protected
                                )
                            }) {
                                fs.insert(p.name.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            ctx.decl.insert(c.name.clone(), fs);
            ctx.extends.insert(c.name.clone(), c.extends.clone());
            ctx.spans.insert(c.name.clone(), c.span);
        }
    }
    let names: Vec<String> = ctx.extends.keys().cloned().collect();
    for n in &names {
        ctx.resolve(n);
    }
    ctx.conflicts
}

/// The **instance-field layout** of every class: each class name maps to the deterministic, sorted list
/// of its storage field *names* (the slot order is its index in this list). The single source both
/// backends consult to build an instance's slot-indexed `Vec` and to resolve a `name → slot` at a field
/// access (M-perf slot-indexed fields). Because *both* construction and access go through this same
/// layout resolved against the instance's **runtime** class, the slot order is irrelevant to
/// correctness — so a stable sorted order suffices and the multiple-inheritance base-offset hazard never
/// arises (slots are always runtime-resolved, never statically baked).
///
/// "Storage field" = every name that could ever be populated on an instance: an explicit non-`static`
/// `Field` member, a promoted constructor parameter (`public`/`private`/`protected`), the same from each
/// `use`d trait, and all of the above inherited transitively through `extends`. Property hooks, `static`
/// fields, and `const`s are excluded (they are not per-instance storage). The union is intentionally a
/// **superset-or-equal** of what construction populates — an extra never-populated slot is harmless
/// (stays the `None` sentinel), whereas a missing slot would be fatal, so the walk errs toward
/// inclusion. Cycle-safe via a visited set (an `extends` cycle is `E-MI-CYCLE` elsewhere).
pub fn class_field_layout(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet};
    // Own storage field names per class OR trait (promoted ctor params + non-static `Field` members).
    let own_fields = |members: &[ClassMember]| -> Vec<String> {
        let mut v = Vec::new();
        for m in members {
            match m {
                ClassMember::Field {
                    name, modifiers, ..
                } if !modifiers.contains(&Modifier::Static)
                    && !modifiers.contains(&Modifier::Const) =>
                {
                    v.push(name.clone());
                }
                ClassMember::Constructor { params, .. } => {
                    for p in params {
                        if p.modifiers.iter().any(|m| {
                            matches!(
                                m,
                                Modifier::Public | Modifier::Private | Modifier::Protected
                            )
                        }) {
                            v.push(p.name.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        v
    };
    let mut own: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut extends: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &program.items {
        match item {
            Item::Class(c) => {
                own.insert(c.name.clone(), own_fields(&c.members));
                extends.insert(c.name.clone(), c.extends.clone());
                uses.insert(
                    c.name.clone(),
                    c.uses.iter().map(|u| u.name.clone()).collect(),
                );
            }
            Item::Trait(t) => {
                own.insert(t.name.clone(), own_fields(&t.members));
            }
            _ => {}
        }
    }
    // Transitively gather a class's full storage-field set (own + used-trait + every ancestor's),
    // cycle-safe. The `extends`/`uses` of a trait are empty, so a trait contributes only its own.
    fn gather(
        name: &str,
        own: &BTreeMap<String, Vec<String>>,
        extends: &BTreeMap<String, Vec<String>>,
        uses: &BTreeMap<String, Vec<String>>,
        seen: &mut BTreeSet<String>,
        out: &mut BTreeSet<String>,
    ) {
        if !seen.insert(name.to_string()) {
            return;
        }
        if let Some(fs) = own.get(name) {
            out.extend(fs.iter().cloned());
        }
        for t in uses.get(name).into_iter().flatten() {
            gather(t, own, extends, uses, seen, out);
        }
        for p in extends.get(name).into_iter().flatten() {
            gather(p, own, extends, uses, seen, out);
        }
    }
    let mut layout = BTreeMap::new();
    for class in extends.keys() {
        let mut set = BTreeSet::new();
        let mut seen = BTreeSet::new();
        gather(class, &own, &extends, &uses, &mut seen, &mut set);
        // `BTreeSet` iteration is sorted ⇒ a deterministic, backend-independent slot order.
        layout.insert(class.clone(), set.into_iter().collect());
    }
    layout
}

/// The flattened **class-constant** table (Feature A — `const` class constants): for each
/// `(class, NAME)` it gives the constant's literal `Value` and its declared `Type`. Includes a class's
/// own `const` members **plus** those inherited through `extends` and folded in from `use`d traits —
/// own declaration wins over an inherited one (PHP redeclare semantics; the field analog of
/// [`class_field_conflicts`]). A subclass therefore reaches an inherited const through its **own** name
/// (`Sub.MAX` resolves even when `MAX` is declared on a parent), matching PHP.
///
/// This is the single source consumed by all three backends — the interpreter inlines the `Value`, the
/// compiler emits `Op::Const` + derives the operand `CTy` from the `Type`, and the transpiler keys its
/// `ClassName::NAME` access emission on the table's keys — so a const access can never diverge between
/// the interpreter, the VM, and real PHP. The checker validates each initializer is a compile-time literal
/// before any backend runs, so `const_literal` here is total (a checker-unreachable non-literal folds
/// to `Unit`, like a static's). The `extends`/`use` walk is cycle-safe via a visited set.
pub fn class_consts(
    program: &Program,
) -> std::collections::BTreeMap<(String, String), (crate::value::Value, Type)> {
    use std::collections::{BTreeMap, BTreeSet};
    // Own `const` members per class/trait: NAME → (literal Value, Type).
    let mut own: BTreeMap<String, BTreeMap<String, (crate::value::Value, Type)>> = BTreeMap::new();
    let mut extends: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let collect_own = |members: &[ClassMember]| -> BTreeMap<String, (crate::value::Value, Type)> {
        let mut m = BTreeMap::new();
        for mem in members {
            if let ClassMember::Field {
                modifiers,
                name,
                ty,
                init,
                ..
            } = mem
            {
                if modifiers.contains(&Modifier::Const) {
                    let v = init
                        .as_ref()
                        .and_then(crate::value::const_literal)
                        .unwrap_or(crate::value::Value::Unit);
                    m.insert(name.clone(), (v, ty.clone()));
                }
            }
        }
        m
    };
    for item in &program.items {
        match item {
            Item::Class(c) => {
                own.insert(c.name.clone(), collect_own(&c.members));
                extends.insert(c.name.clone(), c.extends.clone());
                uses.insert(
                    c.name.clone(),
                    c.uses.iter().map(|u| u.name.clone()).collect(),
                );
            }
            Item::Trait(t) => {
                own.insert(t.name.clone(), collect_own(&t.members));
            }
            _ => {}
        }
    }
    // Flatten `c`'s consts: own + each `use`d trait's + each parent's (transitively), own/nearer wins.
    fn flatten(
        c: &str,
        own: &BTreeMap<String, BTreeMap<String, (crate::value::Value, Type)>>,
        extends: &BTreeMap<String, Vec<String>>,
        uses: &BTreeMap<String, Vec<String>>,
        seen: &mut BTreeSet<String>,
    ) -> BTreeMap<String, (crate::value::Value, Type)> {
        let mut acc: BTreeMap<String, (crate::value::Value, Type)> = BTreeMap::new();
        if !seen.insert(c.to_string()) {
            return acc; // `extends` cycle — `E-MI-CYCLE` reported elsewhere
        }
        // Own declarations win — insert them first and never overwrite.
        if let Some(m) = own.get(c) {
            for (k, v) in m {
                acc.insert(k.clone(), v.clone());
            }
        }
        // Then `use`d traits, then parents — only filling names not already bound (nearer wins).
        for t in uses.get(c).map(Vec::as_slice).unwrap_or(&[]) {
            for (k, v) in flatten(t, own, extends, uses, seen) {
                acc.entry(k).or_insert(v);
            }
        }
        for p in extends.get(c).map(Vec::as_slice).unwrap_or(&[]) {
            for (k, v) in flatten(p, own, extends, uses, seen) {
                acc.entry(k).or_insert(v);
            }
        }
        seen.remove(c);
        acc
    }
    let mut out: BTreeMap<(String, String), (crate::value::Value, Type)> = BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            let mut seen = BTreeSet::new();
            for (name, vt) in flatten(&c.name, &own, &extends, &uses, &mut seen) {
                out.insert((c.name.clone(), name), vt);
            }
        }
    }
    out
}

/// The class's **own** expression field initializers (Feature B): each `(field_name, init_expr)` for a
/// plain instance field (non-`static`, non-`const`) carrying an initializer, in declaration order.
/// Used by the transpiler to build *this* class's `__construct` prelude (PHP doesn't auto-chain, so a
/// class's constructor runs only its own initializers — see [`field_initializers`]).
pub fn own_field_initializers(decl: &ClassDecl) -> Vec<(String, Expr)> {
    decl.members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Field {
                modifiers,
                name,
                init: Some(e),
                ..
            } if !modifiers.contains(&Modifier::Static)
                && !modifiers.contains(&Modifier::Const) =>
            {
                Some((name.clone(), e.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The ordered **expression field initializers** to run when constructing `class` (Feature B) — the
/// own initializers of the class whose constructor PHP actually invokes for `new class()`.
///
/// PHP does **not** auto-chain `parent::__construct`, so `new C()` runs exactly *one* class's
/// constructor body+prelude: `C`'s own if `C` declares a constructor or has its own field
/// initializers, otherwise the nearest ancestor's (the one PHP inherits the constructor from). This
/// helper returns that class's own field initializers, in declaration order — so an initializer may
/// read `this` and an **earlier-declared sibling** (a later field is `E-FIELD-INIT-FORWARD-REF`).
///
/// Returning the *invoked* constructor's own initializers (rather than every ancestor's) keeps the
/// three backends byte-identical with PHP's constructor inheritance: the interpreter sets these after
/// promotion, the compiler emits `SetField` for them, and the transpiler prepends exactly these to the
/// same class's `__construct`. Cycle-safe (reuses [`class_mro`]).
pub fn field_initializers(program: &Program, class: &str) -> Vec<(String, Expr)> {
    let find = |name: &str| -> Option<&ClassDecl> {
        program.items.iter().find_map(|it| match it {
            Item::Class(c) if c.name == name => Some(c),
            _ => None,
        })
    };
    // self-first, then ancestors nearest-first (the order PHP resolves an inherited constructor).
    let chain: Vec<String> = std::iter::once(class.to_string())
        .chain(class_mro(program).get(class).cloned().unwrap_or_default())
        .collect();
    for cls in &chain {
        let Some(decl) = find(cls) else { continue };
        let has_ctor = decl
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor { .. }));
        let own = own_field_initializers(decl);
        // The first class with an own constructor OR own field initializers is the one whose
        // `__construct` PHP invokes — run its initializers (which may be empty if it only has a ctor).
        if has_ctor || !own.is_empty() {
            return own;
        }
    }
    Vec::new()
}

/// The ordered list of constructors a `ClassName(args)` call runs (M-RT S6c.2). Each entry is one
/// `(params, body)` to execute, in order, on the single instance being built; the call's full argument
/// list is the entries' params concatenated in this order, sliced per entry.
///
/// - A class with its **own** constructor → just that one (`[own]`).
/// - **Single** inheritance, no own ctor → the parent's plan (the nearest ancestor's ctor, transitively
///   chained — S6c.2a).
/// - **Multiple** inheritance, no own ctor → each parent's plan concatenated in `extends` order, so
///   every parent's constructor runs and initializes its fields (S6c.2b). A diamond-shared base's ctor
///   runs once per arm — identically on all three backends, so byte-identity holds.
/// - No ctor anywhere → `[]` (a zero-arg `ClassName()` builds an empty instance).
///
/// Single source of the construction decision: checker (signature = concatenated param types),
/// compiler (instance descriptor + synthetic ctor body + arity), interpreter (run each entry with its
/// arg slice). A child that declares its *own* ctor under inheritance returns just its own — initializing
/// inherited state then needs the deferred `super`-replacement (KNOWN_ISSUES).
pub fn ctor_plan(program: &Program, class: &str) -> Vec<(Vec<CtorParam>, Vec<Stmt>)> {
    let Some(decl) = program.items.iter().find_map(|it| match it {
        Item::Class(c) if c.name == class => Some(c),
        _ => None,
    }) else {
        return Vec::new();
    };
    if let Some((p, b)) = decl.members.iter().find_map(|m| match m {
        ClassMember::Constructor { params, body, .. } => Some((params.clone(), body.clone())),
        _ => None,
    }) {
        return vec![(p, b)];
    }
    // M-RT S8: a `use`d trait's constructor becomes the class's constructor and **wins over an
    // inherited parent ctor** (PHP P2 — the parent ctor is not auto-run; the checker warns
    // `W-TRAIT-CTOR-PARENT-SKIPPED`). The checker rejects two unresolved trait ctors
    // (`E-TRAIT-CTOR-COLLISION`), so a clean program has at most one — take it deterministically.
    if let Some(tc) = decl.uses.iter().find_map(|u| {
        program.items.iter().find_map(|it| match it {
            Item::Trait(t) if t.name == u.name => t.members.iter().find_map(|m| match m {
                ClassMember::Constructor { params, body, .. } => {
                    Some((params.clone(), body.clone()))
                }
                _ => None,
            }),
            _ => None,
        })
    }) {
        return vec![tc];
    }
    match decl.extends.len() {
        0 => Vec::new(),
        1 => ctor_plan(program, &decl.extends[0]),
        _ => decl
            .extends
            .iter()
            .flat_map(|p| ctor_plan(program, p))
            .collect(),
    }
}

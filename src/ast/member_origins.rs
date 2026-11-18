//! Member origins (M-RT S6b, split out of `class_hierarchy.rs` 2026-11-17): for each `(class, member)`
//! the class whose body runs — methods ([`class_method_origins`]) and property-hook accessors
//! ([`class_hook_origins`], DEC-572). One resolver, so override, multi-parent composition and the
//! diamond merge are the same for both, and the same for every backend.

use super::*;

/// `(class, member) -> (declaring class, member)`, plus every unresolved cross-parent collision as
/// `(class, member, class span)`.
pub type MemberOrigins = (
    std::collections::BTreeMap<(String, String), (String, String)>,
    Vec<(String, String, Span)>,
);

/// The fully-resolved method-dispatch table for every class (M-RT S6b): for each `(class, name)` it
/// gives the `(declaring_class, declaring_method)` a call of `name` on an instance of `class` runs.
/// This is the **single source of dispatch** for *both* backends — the interpreter looks up the
/// origin and the compiler aliases the bytecode method-table entry to it — so multi-parent dispatch
/// (including resolution clauses and renamed aliases) can never diverge between the interpreter and the VM.
///
/// Composition: a class's own methods map to itself (override); each direct parent contributes its
/// own resolved table; a diamond shared base auto-merges (both arms reach the *same* declaring
/// method). Resolution clauses (`use`/`rename`/`exclude`) are applied before finalizing. The second
/// return value lists every **unresolved** cross-parent collision as `(class, name, class_span)` —
/// the checker reports each as `E-MI-CONFLICT`. On a conflict the table still records a deterministic
/// pick so a backend never panics (the checker fails the build first).
pub fn class_method_origins(program: &Program) -> MemberOrigins {
    member_origins(program, Members::Methods)
}

/// DEC-572 (row 4e): the hook twin of [`class_method_origins`] — for each `(class, <name>$get|$set)` the
/// class whose hook body runs, inherited PER ACCESSOR as in PHP. A `use`d trait's hook is the using
/// class's own (where the compiler registers it); an own stored field shadows an inherited hook (PENDING
/// DEC-582), so an origin with no such hook means "the slot". Kept apart from the method table, which the
/// transpiler reads as PHP method names. Two parents bringing a property with different declarations
/// leave a slot marker (origin = the class itself), so the class and its subclasses keep the pre-DEC-572
/// behaviour; the collision is listed, unreported (PENDING DEC-584).
pub fn class_hook_origins(program: &Program) -> MemberOrigins {
    member_origins(program, Members::Hooks)
}

/// Which member names [`member_origins`] resolves.
#[derive(Clone, Copy, PartialEq)]
enum Members {
    Methods,
    Hooks,
}

#[allow(clippy::type_complexity)]
fn member_origins(program: &Program, kind: Members) -> MemberOrigins {
    use std::collections::{BTreeMap, BTreeSet};

    struct Ctx {
        decl: BTreeMap<String, BTreeSet<String>>,
        extends: BTreeMap<String, Vec<String>>,
        /// M-RT S8: class → the traits it `use`s. A used trait contributes its methods like a parent's
        /// table, so trait collisions and `use`/`rename`/`exclude` clauses reuse the same machinery.
        uses: BTreeMap<String, Vec<String>>,
        /// M-RT S8: trait name → its declared method names.
        trait_decl: BTreeMap<String, BTreeSet<String>>,
        resolutions: BTreeMap<String, Vec<Resolution>>,
        spans: BTreeMap<String, Span>,
        memo: BTreeMap<String, BTreeMap<String, (String, String)>>,
        conflicts: Vec<(String, String, Span)>,
        in_progress: BTreeSet<String>,
        /// Hooks only (DEC-584): a clash leaves a slot; a trait's entry is one declaration in every user.
        hooks: bool,
        via_trait: BTreeMap<(String, String), String>,
    }

    impl Ctx {
        fn resolve(&mut self, c: &str) -> BTreeMap<String, (String, String)> {
            if let Some(m) = self.memo.get(c) {
                return m.clone();
            }
            if !self.in_progress.insert(c.to_string()) {
                // `extends` cycle — reported as `E-MI-CYCLE` elsewhere; break to avoid infinite loop.
                return BTreeMap::new();
            }
            let mut map: BTreeMap<String, (String, String)> = BTreeMap::new();
            // Own methods win over anything inherited (override).
            if let Some(ms) = self.decl.get(c).cloned() {
                for m in ms {
                    map.insert(m.clone(), (c.to_string(), m));
                }
            }
            // Gather each direct parent's resolved contributions, tracking the direct parent the
            // method arrives through (so a `use/rename/exclude P.m` clause can target it) and the true
            // origin (so a diamond dedups by origin).
            let mut contrib: BTreeMap<String, Vec<(String, (String, String))>> = BTreeMap::new();
            for p in self.extends.get(c).cloned().unwrap_or_default() {
                let p_map = self.resolve(&p);
                for (name, origin) in p_map {
                    if map.contains_key(&name) {
                        continue; // overridden by C itself
                    }
                    contrib.entry(name).or_default().push((p.clone(), origin));
                }
            }
            // M-RT S8: each `use`d trait contributes its own declared methods. Tracked by the trait
            // name (so a `use/rename/exclude Trait.m` clause can target it) with origin `(trait, m)`
            // (so two traits supplying the *same* method collide and need resolution; the class's own
            // method still wins via the `map.contains_key` guard).
            for t in self.uses.get(c).cloned().unwrap_or_default() {
                for name in self.trait_decl.get(&t).cloned().unwrap_or_default() {
                    if map.contains_key(&name) {
                        continue; // overridden by C itself
                    }
                    contrib
                        .entry(name.clone())
                        .or_default()
                        .push((t.clone(), (t.clone(), name)));
                }
            }
            // Apply resolution clauses in source order.
            for r in self.resolutions.get(c).cloned().unwrap_or_default() {
                match r {
                    Resolution::Use { parent, method, .. } => {
                        if let Some(v) = contrib.get_mut(&method) {
                            v.retain(|(pn, _)| pn == &parent);
                        }
                    }
                    Resolution::Exclude { parent, method, .. } => {
                        if let Some(v) = contrib.get_mut(&method) {
                            v.retain(|(pn, _)| pn != &parent);
                        }
                    }
                    Resolution::Rename {
                        parent,
                        method,
                        as_name,
                        ..
                    } => {
                        let moved: Vec<(String, (String, String))> =
                            if let Some(v) = contrib.get_mut(&method) {
                                let (keep, take): (Vec<_>, Vec<_>) =
                                    v.drain(..).partition(|(pn, _)| pn != &parent);
                                *v = keep;
                                take
                            } else {
                                Vec::new()
                            };
                        if !moved.is_empty() {
                            contrib.entry(as_name).or_default().extend(moved);
                        }
                    }
                }
            }
            // Hooks: a property clashes when two direct parents bring it with different declaration sets
            // (classes or traits, any accessor); one parent's get override over a stored field is not one.
            let prop = |n: &str| n.rsplit_once('$').map_or(n, |(p, _)| p).to_string();
            let mut by_parent: BTreeMap<String, BTreeMap<&String, BTreeSet<String>>> =
                BTreeMap::new();
            if self.hooks {
                for (name, v) in &contrib {
                    for (via, o) in v {
                        let d = self.via_trait.get(o).unwrap_or(&o.0).clone();
                        by_parent
                            .entry(prop(name))
                            .or_default()
                            .entry(via)
                            .or_default()
                            .insert(d);
                    }
                }
            }
            let clashes: BTreeSet<String> = by_parent
                .into_iter()
                .filter(|(_, per)| per.values().collect::<BTreeSet<_>>().len() > 1)
                .map(|(p, _)| p)
                .collect();
            // Finalize each inherited name: dedup by origin (diamond), else conflict.
            for (name, v) in contrib {
                if map.contains_key(&name) {
                    continue;
                }
                let distinct: BTreeSet<(String, String)> = v.into_iter().map(|(_, o)| o).collect();
                let clash = if self.hooks {
                    clashes.contains(&prop(&name))
                } else {
                    distinct.len() > 1
                };
                match distinct.into_iter().next() {
                    None => {}
                    Some(first) => {
                        if clash {
                            let span = self.spans.get(c).copied().unwrap_or(Span {
                                start: 0,
                                len: 0,
                                line: 1,
                                col: 1,
                            });
                            self.conflicts.push((c.to_string(), name.clone(), span));
                        }
                        if clash && self.hooks {
                            // Slot marker: no hook runs; a subclass sees `c` as one more declaration.
                            map.insert(name.clone(), (c.to_string(), name));
                        } else {
                            map.insert(name, first); // deterministic pick (sorted-first)
                        }
                    }
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
        uses: BTreeMap::new(),
        trait_decl: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        spans: BTreeMap::new(),
        memo: BTreeMap::new(),
        conflicts: Vec::new(),
        in_progress: BTreeSet::new(),
        hooks: kind == Members::Hooks,
        via_trait: BTreeMap::new(),
    };
    let traits: BTreeMap<&str, &[ClassMember]> = program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Trait(t) => Some((t.name.as_str(), t.members.as_slice())),
            _ => None,
        })
        .collect();
    for item in &program.items {
        match item {
            // Hooks: fold each used trait's hooks and storage (`traits`, by name) into its user.
            Item::Class(c) if kind == Members::Hooks => {
                let mut ms = BTreeSet::new();
                let used = c
                    .uses
                    .iter()
                    .filter_map(|u| Some((Some(&u.name), *traits.get(u.name.as_str())?)));
                for (from, members) in std::iter::once((None, c.members.as_slice())).chain(used) {
                    let mut own = BTreeSet::new();
                    hook_accessors(members, &mut own);
                    // DEC-582 PENDING: an own stored field shadows an inherited hook of its name.
                    for f in own_storage_fields(members) {
                        own.insert(format!("{f}$get"));
                        own.insert(format!("{f}$set"));
                    }
                    for a in own {
                        // An accessor the class declares itself stays the class's own declaration.
                        if let Some(t) = from.filter(|_| !ms.contains(&a)) {
                            ctx.via_trait.insert((c.name.clone(), a.clone()), t.clone());
                        }
                        ms.insert(a);
                    }
                }
                ctx.decl.insert(c.name.clone(), ms);
                ctx.extends.insert(c.name.clone(), c.extends.clone());
                ctx.spans.insert(c.name.clone(), c.span);
            }
            Item::Class(c) => {
                let mut ms = BTreeSet::new();
                for m in &c.members {
                    if let ClassMember::Method(f) = m {
                        ms.insert(f.name.clone());
                    }
                }
                ctx.decl.insert(c.name.clone(), ms);
                ctx.extends.insert(c.name.clone(), c.extends.clone());
                ctx.uses.insert(
                    c.name.clone(),
                    c.uses.iter().map(|u| u.name.clone()).collect(),
                );
                ctx.resolutions
                    .insert(c.name.clone(), c.resolutions.clone());
                ctx.spans.insert(c.name.clone(), c.span);
            }
            Item::Trait(t) if kind == Members::Methods => {
                let mut ms = BTreeSet::new();
                for m in &t.members {
                    if let ClassMember::Method(f) = m {
                        ms.insert(f.name.clone());
                    }
                }
                ctx.trait_decl.insert(t.name.clone(), ms);
            }
            _ => {}
        }
    }
    let names: Vec<String> = ctx.extends.keys().cloned().collect();
    for n in &names {
        ctx.resolve(n);
    }
    let mut out: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for (c, m) in &ctx.memo {
        for (name, origin) in m {
            out.insert((c.clone(), name.clone()), origin.clone());
        }
    }
    (out, ctx.conflicts)
}

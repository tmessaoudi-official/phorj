//! Class analyses — hierarchy: implements/supertypes/entry points/instanceof/MRO/parent
//! resolution (method origins: `member_origins.rs`). Single-sourced across backends (Invariant discipline).

use super::*;

/// (every interface in the `implements` set *and* the `extends` closure of each). This is the single
/// runtime table behind `instanceof` against an interface: `x instanceof I` is true iff `I` is in
/// `class_implements[class_of(x)]`. It is computed **once** by this shared function and consumed
/// identically by the checker (subtyping + conformance), the interpreter, and the compiler/VM — one
/// algorithm, so the three backends can never diverge (the same discipline as [`free_vars`]).
///
/// The per-class list is **sorted** (invariant #8: deterministic order for all backends) and the
/// `extends` walk is **cycle-safe** via a visited set, so a malformed cyclic interface graph (which
/// the checker rejects as `E-IFACE-CYCLE` before any backend runs) can never make this loop forever.
/// Names are whatever the (already loader-mangled, if multi-package) AST carries — consistent across
/// every consumer.
pub fn class_implements(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet};
    // Direct `extends` edges for interfaces and for classes (M-RT S6), plus each class's own
    // `implements` list. A class inherits the interfaces of all its ancestor classes.
    let mut iface_extends: BTreeMap<&str, &[String]> = BTreeMap::new();
    let mut class_extends: BTreeMap<&str, &[String]> = BTreeMap::new();
    let mut own_implements: BTreeMap<&str, &[String]> = BTreeMap::new();
    for item in &program.items {
        match item {
            Item::Interface(i) => {
                iface_extends.insert(i.name.as_str(), &i.extends);
            }
            Item::Class(c) => {
                class_extends.insert(c.name.as_str(), &c.extends);
                own_implements.insert(c.name.as_str(), &c.implements);
            }
            _ => {}
        }
    }
    // Transitive closure of a name's `extends` chain (the name itself included), visited-guarded
    // against cycles. Used for both the interface graph and the class graph.
    fn closure<'a>(
        name: &'a str,
        edges: &BTreeMap<&'a str, &'a [String]>,
        acc: &mut BTreeSet<String>,
    ) {
        if !acc.insert(name.to_string()) {
            return; // already visited — also breaks any cycle
        }
        if let Some(parents) = edges.get(name) {
            for p in parents.iter() {
                closure(p, edges, acc);
            }
        }
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            // The class itself plus every ancestor class (so inherited interfaces flow down, M-RT S6).
            let mut family: BTreeSet<String> = BTreeSet::new();
            closure(c.name.as_str(), &class_extends, &mut family);
            let mut ifaces: BTreeSet<String> = BTreeSet::new();
            for cls in &family {
                if let Some(impls) = own_implements.get(cls.as_str()) {
                    for i in impls.iter() {
                        closure(i, &iface_extends, &mut ifaces);
                    }
                }
            }
            out.insert(c.name.clone(), ifaces.into_iter().collect());
        }
    }
    out
}

/// Transitive parent-class closure for every class: `class_supertypes[c]` is the sorted set of all
/// ancestor class names reachable through `extends` — **not** including `c` itself, except when `c`
/// is part of an `extends` cycle (then `c` appears in its own set, which the checker uses to report
/// `E-MI-CYCLE`). Mirrors [`class_implements`]; the `extends` walk is cycle-safe via a visited set.
/// Consumed by the checker's nominal-subtype oracle (so `Dog <: Animal`) and (S6b+) the backends for
/// `instanceof` against a parent class — one algorithm, so the three backends can never diverge.
pub fn class_supertypes(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut class_extends: BTreeMap<&str, &[String]> = BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            class_extends.insert(c.name.as_str(), &c.extends);
        }
    }
    // Accumulate the ancestors of `name` (parents, grandparents, …) — `name` itself is added only if
    // a cycle leads back to it.
    fn ancestors<'a>(
        name: &'a str,
        edges: &BTreeMap<&'a str, &'a [String]>,
        acc: &mut BTreeSet<String>,
    ) {
        if let Some(parents) = edges.get(name) {
            for p in parents.iter() {
                if acc.insert(p.clone()) {
                    ancestors(p, edges, acc);
                }
            }
        }
    }
    let mut out: std::collections::BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            let mut anc: BTreeSet<String> = BTreeSet::new();
            ancestors(c.name.as_str(), &class_extends, &mut anc);
            out.insert(c.name.clone(), anc.into_iter().collect());
        }
    }
    out
}

/// The **runtime subtype oracle** (M-RT S6c.3): for each class, every type name it is an instance of —
/// its transitive parent classes ([`class_supertypes`]) **and** its transitive interfaces
/// ([`class_implements`]). This is the single source consumed by `instanceof`, match type-patterns, and
/// overload subtyping on **both** backends, so a `Dog instanceof Animal` / `Duck instanceof Swimmer`
/// (a *class* ancestor, not just an interface) is true and can never diverge between the interpreter and the VM.
/// (The checker keeps a separate interfaces-only `class_implements` for interface *conformance*; its
/// `is_subtype` already consults `class_supertypes` independently.)
pub fn instanceof_table(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut table = class_implements(program);
    for (cls, sups) in class_supertypes(program) {
        let entry = table.entry(cls).or_default();
        for s in sups {
            if !entry.contains(&s) {
                entry.push(s);
            }
        }
    }
    for v in table.values_mut() {
        v.sort();
        v.dedup();
    }
    table
}

/// Method-resolution order for every class: `class_mro[c]` is `c`'s ancestor classes in
/// **nearest-first breadth-first** order (direct parents in `extends` order, then their parents, …),
/// excluding `c` itself. Cycle-safe via a visited set. This is the **single source of dispatch
/// precedence** consumed by both the interpreter's `call_method` parent walk and the compiler's
/// method-table pre-flatten (M-RT S6b), so the two backends can never disagree on *which* ancestor a
/// method is inherited from. A method is resolved by scanning `[c] ++ class_mro[c]` and taking the
/// first class that declares it (so a nearer declaration overrides a farther one); a diamond shared
/// base is visited once, auto-merging when both arms reach the same declaring method.
pub fn class_mro(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    use std::collections::{BTreeMap, HashSet};
    let parents: BTreeMap<&str, &[String]> = program
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Class(c) => Some((c.name.as_str(), c.extends.as_slice())),
            _ => None,
        })
        .collect();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            let mut order = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut queue: Vec<String> = c.extends.clone();
            let mut i = 0;
            while i < queue.len() {
                let p = queue[i].clone();
                i += 1;
                if !seen.insert(p.clone()) {
                    continue;
                }
                order.push(p.clone());
                if let Some(gps) = parents.get(p.as_str()) {
                    queue.extend(gps.iter().cloned());
                }
            }
            out.insert(c.name.clone(), order);
        }
    }
    out
}

/// Direct parents (`extends`) of every class, keyed by class name — `Dog → [Animal]`, `Both →
/// [Left, Right]`, a root class → `[]`. The immediate-parent view (vs. [`class_mro`]'s transitive
/// ancestors); used by `parent`/super resolution (M-RT super/parent dispatch).
pub fn class_parents(program: &Program) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            out.insert(c.name.clone(), c.extends.clone());
        }
    }
    out
}

/// Why a `parent`/super call could not resolve (M-RT super/parent dispatch). Mapped to the
/// `E-PARENT-*` diagnostics by the checker; the backends never see an error (the build fails first),
/// but they call the same [`resolve_parent_method`] so a resolved target is byte-identical across
/// interp/VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentResolveError {
    /// The lexical class has no parents at all (`E-PARENT-NO-PARENT`).
    NoParent,
    /// `parent(A)` where `A` is not a transitive ancestor of the lexical class (`E-PARENT-NOT-ANCESTOR`).
    NotAncestor,
    /// No ancestor (or the named one) declares/inherits the method (`E-PARENT-NO-METHOD`).
    NoMethod,
    /// Bare `parent.m()` in a multiple-inheritance class where ≥2 parent arms resolve `m` to distinct
    /// origins (`E-PARENT-AMBIGUOUS`); the fix is to qualify with `parent(A).m()`.
    Ambiguous,
}

/// Resolve a `parent`/super method call to its concrete `(declaring_class, method)` target — the
/// **single source** consumed by the checker (errors + typing), the interpreter (dispatch), and the
/// compiler (bakes the function index), so `interp ≡ VM` by construction (the same discipline as
/// [`class_method_origins`]). `parent` is **lexical**: `lexical_class` is the class whose method body
/// contains the call (where the override is *written*), NOT the receiver's runtime class — so an
/// override calling `parent.m()` reaches the version it shadows.
///
/// - `ancestor = Some(A)` (`parent(A).m()`): `A` must be in `mro[lexical_class]`
///   (`NotAncestor`); the target is `origins[(A, method)]` — the `m` an instance of `A` runs
///   (its own or its nearest ancestor's). `A` may be **any** transitive ancestor (C++-style jump).
/// - `ancestor = None` (`parent.m()`): the nearest ancestor declaring `m`, found by collecting the
///   distinct origins each **direct** parent resolves `m` to — exactly one ⇒ that target; zero ⇒
///   `NoMethod`; ≥2 distinct ⇒ `Ambiguous` (only possible under multiple inheritance).
///
/// `method` may be `"constructor"` (a parent-constructor call); origins never contains a constructor
/// entry, so a ctor call always resolves via the direct-parent arm against `parents`, handled by the
/// caller — this function is method-only and the caller special-cases the constructor.
pub fn resolve_parent_method(
    parents: &std::collections::BTreeMap<String, Vec<String>>,
    mro: &std::collections::BTreeMap<String, Vec<String>>,
    origins: &std::collections::BTreeMap<(String, String), (String, String)>,
    lexical_class: &str,
    ancestor: Option<&str>,
    method: &str,
) -> Result<(String, String), ParentResolveError> {
    let direct = parents.get(lexical_class).map_or(&[][..], Vec::as_slice);
    if direct.is_empty() {
        return Err(ParentResolveError::NoParent);
    }
    match ancestor {
        Some(a) => {
            let is_ancestor = mro
                .get(lexical_class)
                .is_some_and(|anc| anc.iter().any(|x| x == a));
            if !is_ancestor {
                return Err(ParentResolveError::NotAncestor);
            }
            origins
                .get(&(a.to_string(), method.to_string()))
                .cloned()
                .ok_or(ParentResolveError::NoMethod)
        }
        None => {
            // Distinct origins each direct parent resolves `method` to (a diamond's shared base
            // collapses to one origin, so it is not ambiguous).
            let mut found: Vec<(String, String)> = Vec::new();
            for p in direct {
                if let Some(t) = origins.get(&(p.clone(), method.to_string())) {
                    if !found.contains(t) {
                        found.push(t.clone());
                    }
                }
            }
            match found.len() {
                0 => Err(ParentResolveError::NoMethod),
                1 => Ok(found.into_iter().next().unwrap()),
                _ => Err(ParentResolveError::Ambiguous),
            }
        }
    }
}

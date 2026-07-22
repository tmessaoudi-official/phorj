//! Class analyses — hierarchy: implements/supertypes/entry points/instanceof/MRO/parent
//! resolution/method origins. Single-sourced across backends (Invariant discipline).

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

/// DEC-191: the `#[Entry]` marker, in every "nothing in the wind" import form (bare after
/// `import Core.Runtime.Entry;`, or fully-qualified) — single source [`crate::ast::Attribute::is_entry`].
/// (`#[Config]`, DEC-318, has no free-fn twin — call [`crate::ast::Attribute::is_config`] directly.)
pub fn is_entry_attr(a: &crate::ast::Attribute) -> bool {
    a.is_entry()
}

/// DEC-191: the ROLE an `#[Entry]` function plays, inferred from its signature (never from its
/// name — the magic `main`/`handle` names are retired).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryRole {
    /// `(): void` / `(): int` / `(List<string>): void` / `(List<string>): int` — the `phg run`
    /// entry. An `int` return is the process exit status (0–255); `void` exits 0 on clean.
    Cli,
    /// `(Request): Response` — the `phg serve` per-request handler.
    Web,
}

/// Classify an `#[Entry]` function's signature into its role, or `None` when it matches neither
/// (the checker turns that into `E-ENTRY-SIG`). AST-level shape matching — runs on the expanded
/// program in every backend, so it stays checker-independent.
pub fn entry_role(f: &FunctionDecl) -> Option<EntryRole> {
    fn named_is(t: &crate::ast::Type, want: &str) -> bool {
        matches!(t, crate::ast::Type::Named { name, args, .. } if name == want && args.is_empty())
    }
    let ret_cli = match &f.ret {
        None => true, // no annotation on an entry is not valid Phorj anyway; checker rejects
        Some(t) => named_is(t, "void") || named_is(t, "int"),
    };
    let params_cli = f.params.is_empty()
        || (f.params.len() == 1
            && matches!(&f.params[0].ty, crate::ast::Type::Named { name, args, .. }
                if name == "List" && args.len() == 1
                    && matches!(&args[0], crate::ast::Type::Named { name, args, .. }
                        if name == "string" && args.is_empty())));
    if params_cli && ret_cli {
        return Some(EntryRole::Cli);
    }
    let web = f.params.len() == 1
        && named_is(&f.params[0].ty, "Request")
        && f.ret.as_ref().is_some_and(|t| named_is(t, "Response"));
    if web {
        return Some(EntryRole::Web);
    }
    None
}

/// DEC-191: every `#[Entry]`-attributed function in the program — top-level functions and class
/// STATIC methods (an attributed instance method is invalid; the checker rejects it, and this
/// resolver simply does not surface it). Returns `(class, decl)` pairs in declaration order; role
/// classification and the one-per-role rule (`E-MULTIPLE-ENTRY`) live above this.
pub fn entry_candidates(program: &Program) -> Vec<(Option<&str>, &FunctionDecl)> {
    let mut out = Vec::new();
    for item in &program.items {
        match item {
            Item::Function(f) if f.attrs.iter().any(is_entry_attr) => out.push((None, f)),
            Item::Class(c) => {
                for m in &c.members {
                    if let ClassMember::Method(f) = m {
                        if f.attrs.iter().any(is_entry_attr)
                            && f.modifiers.contains(&Modifier::Static)
                        {
                            out.push((Some(c.name.as_str()), f));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// DEC-191: resolve the program's entry for one role — what the backends call. `None` when the
/// program declares no entry of that role (a library file, or `phg run` on a web-only program).
pub fn entry_for(program: &Program, role: EntryRole) -> Option<(Option<&str>, &FunctionDecl)> {
    entry_candidates(program)
        .into_iter()
        .find(|(_, f)| entry_role(f) == Some(role))
}

/// Resolve a program **entry point** (`main` / `handle`) — the single source of truth all backends
/// share so they invoke the same function (Batch-1 D, `docs/specs/2026-06-27-class-entry-points-design.md`).
///
/// An entry is **either** a top-level free function named `name` (returns `Some((None, decl))`) **or**
/// a `static` method named `name` on some class (`Some((Some(class), decl))`). An *instance* method
/// named `name` is **not** an entry (an ordinary method). Top-level wins the scan order, but a valid
/// program has at most one entry — [`entry_point_count`] backs the checker's `E-MULTIPLE-MAIN`, so by
/// the time any backend calls this the entry is unambiguous.
pub fn entry_point<'a>(
    program: &'a Program,
    name: &str,
) -> Option<(Option<&'a str>, &'a FunctionDecl)> {
    for item in &program.items {
        if let Item::Function(f) = item {
            if f.name == name {
                return Some((None, f));
            }
        }
    }
    for item in &program.items {
        if let Item::Class(c) = item {
            for m in &c.members {
                if let ClassMember::Method(f) = m {
                    if f.name == name && f.modifiers.contains(&Modifier::Static) {
                        return Some((Some(c.name.as_str()), f));
                    }
                }
            }
        }
    }
    None
}

/// How many distinct entry points named `name` a program declares (a top-level function plus every
/// class-static method of that name). `> 1` is the checker's `E-MULTIPLE-MAIN` — an ambiguous entry is
/// an error, never a silent pick.
pub fn entry_point_count(program: &Program, name: &str) -> usize {
    let mut n = 0;
    for item in &program.items {
        match item {
            Item::Function(f) if f.name == name => n += 1,
            Item::Class(c) => {
                for m in &c.members {
                    if let ClassMember::Method(f) = m {
                        if f.name == name && f.modifiers.contains(&Modifier::Static) {
                            n += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    n
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
#[allow(clippy::type_complexity)]
pub fn class_method_origins(
    program: &Program,
) -> (
    std::collections::BTreeMap<(String, String), (String, String)>,
    Vec<(String, String, Span)>,
) {
    use std::collections::{BTreeMap, BTreeSet};

    struct Ctx {
        decl: BTreeMap<String, BTreeSet<String>>,
        extends: BTreeMap<String, Vec<String>>,
        /// M-RT S8: class → the traits it `use`s. A used trait contributes its own methods exactly like
        /// a parent contributes its resolved table, so trait-vs-trait / trait-vs-parent collisions and
        /// `use`/`rename`/`exclude` resolution clauses all reuse the same machinery.
        uses: BTreeMap<String, Vec<String>>,
        /// M-RT S8: trait name → its declared method names.
        trait_decl: BTreeMap<String, BTreeSet<String>>,
        resolutions: BTreeMap<String, Vec<Resolution>>,
        spans: BTreeMap<String, Span>,
        memo: BTreeMap<String, BTreeMap<String, (String, String)>>,
        conflicts: Vec<(String, String, Span)>,
        in_progress: BTreeSet<String>,
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
            // Finalize each inherited name: dedup by origin (diamond), else conflict.
            for (name, v) in contrib {
                if map.contains_key(&name) {
                    continue;
                }
                let distinct: BTreeSet<(String, String)> = v.into_iter().map(|(_, o)| o).collect();
                let mut it = distinct.into_iter();
                match it.next() {
                    None => {}
                    Some(first) => {
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
    };
    for item in &program.items {
        match item {
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
            Item::Trait(t) => {
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

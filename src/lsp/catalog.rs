//! LSP completion catalog — the ONE enumeration API over the Core registries (2026-07-20 alignment
//! pass). Sources of truth, never re-listed by hand:
//!   * importable module paths ← `cli::preludes::core_module_paths()` (derived from `CORE_MODULES`)
//!   * per-module members       ← `native::registry()` (the same registry the checker/transpile use)
//!
//! This keeps completion aligned with what the language actually accepts by construction: a new Core
//! module or native shows up in completion the moment it is registered, with no LSP edit. Project-source
//! package discovery (scanning the user's `src/`/`bin/`/`views/`/`vendor/`) is a follow-up increment
//! wired through `crate::loader`.
use crate::ast::{class_supertypes, ClassMember, Item, Program};
use crate::native;

/// The completable instance members (methods + fields + property hooks + ctor-promoted params) of a
/// USER class `class_name`, INCLUDING members inherited from its transitive `extends` supertypes
/// (via `class_supertypes` — the same hierarchy the backends use). Sorted + deduped (a subclass member
/// shadows an inherited one). Empty when `class_name` is not a user class/interface/trait in `program`
/// — a STDLIB class (Date/Instant/Uri/ServeConfig…) is answered by [`prelude_class_members`], which
/// the caller falls back to precisely when this returns empty. Kind: Method=2, Field/property=5.
pub(super) fn class_members(program: &Program, class_name: &str) -> Vec<(String, u32)> {
    let supers = class_supertypes(program);
    let mut chain: Vec<&str> = vec![class_name];
    if let Some(anc) = supers.get(class_name) {
        chain.extend(anc.iter().map(String::as_str));
    }
    let mut out: Vec<(String, u32)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for cname in chain {
        for (name, kind) in decl_members(program, cname) {
            if seen.insert(name.clone()) {
                out.push((name, kind));
            }
        }
    }
    out.sort();
    out
}

/// Members of the class / interface / trait named `name` in `program` (own members only — inheritance
/// is composed by [`class_members`]).
fn decl_members(program: &Program, name: &str) -> Vec<(String, u32)> {
    for it in &program.items {
        match it {
            Item::Class(c) if c.name == name => return collect_members(&c.members),
            Item::Trait(t) if t.name == name => return collect_members(&t.members),
            Item::Interface(i) if i.name == name => {
                return i.methods.iter().map(|m| (m.name.clone(), 2)).collect();
            }
            _ => {}
        }
    }
    Vec::new()
}

/// (name, CompletionItemKind) for each member. Methods → 2; fields / hooks / ctor-PROMOTED params
/// (those carrying a visibility modifier, i.e. real instance fields) → 5. Plain ctor params are locals,
/// not members, so they are skipped.
fn collect_members(members: &[ClassMember]) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for m in members {
        match m {
            ClassMember::Method(f) => out.push((f.name.clone(), 2)),
            ClassMember::Field { name, .. } | ClassMember::Hook { name, .. } => {
                out.push((name.clone(), 5))
            }
            ClassMember::Constructor { params, .. } => {
                for p in params {
                    // A ctor param is a real instance FIELD iff it carries a visibility modifier
                    // (promotion) — single-sourced with the checker/backends. A `mutable`-only or plain
                    // param is a constructor local, not a completable member.
                    if p.modifiers.iter().any(|m| m.is_member_visibility()) {
                        out.push((p.name.clone(), 5));
                    }
                }
            }
        }
    }
    out
}

/// Importable `Core.*` module paths (dotted, sorted) for `import X.` completion.
pub(super) fn core_module_paths() -> Vec<String> {
    crate::cli::module_catalog::core_module_paths()
}

/// The names importable FROM the Core module at dotted `path` — the second half of an import, as in
/// `import Core.ErrorModule.RuntimeError;`. Sorted + deduped; empty when `path` names no Core module.
///
/// This exists because a MEMBER-GATED module has no other way in: referencing one of its types bare is
/// `E-INJECTED-TYPE-BARE`, so `import Core.ErrorModule;` alone buys nothing and the member import is
/// mandatory. Completion offered module paths only, which left DEC-421's six error types untypeable
/// from the editor — the same 100%-rule hole `withLock` fell through, one level further down the path.
///
/// TWO sources, matching what the checker's import resolution accepts:
///   * the row's injected TYPE names (`bare_types` — `Core.ErrorModule.RuntimeError`);
///   * natives registered under that exact module (`Core.Output.printLine`).
///
/// Prelude class STATICS are deliberately absent: `withLock` is a member of the `FileSystem` class,
/// reached through `import Core.FileSystemModule.FileSystem;`, and is not itself importable.
pub(super) fn module_member_imports(path: &str) -> Vec<String> {
    crate::cli::module_catalog::core_module_members(path)
}

/// The completable members of the Core module whose qualifier (last dotted segment) equals
/// `qualifier` — e.g. `"List"` → `map`/`filter`/…, `"Output"` → `printLine`/`print`,
/// `"FileSystem"` → `readText`/`withLock`/… Sorted + deduped; empty when the qualifier names no
/// Core module (the caller then falls back to general completion).
///
/// TWO sources, unioned (DEC-348 — Invariant 17's 100% rule):
///   * `native::registry()` for registry-only modules (`Output`, `Map`, `Math`, `List`);
///   * the module's PRELUDE class statics for modules whose user-facing surface is written in phorj
///     (`FileSystem.withLock`, and the `Date`/`Uri` classes the old doc comment deferred).
///
/// **`Core.Native.*` twins are excluded**, the same filter `core_module_paths` already applies. Their
/// leaf segment collides with the friendly class name (`Core.Native.FileSystem` → `FileSystem`), so
/// including them did two wrong things at once: it advertised INTERNAL natives users must never call
/// (`FileSystem.lockAcquire` — precisely the leak-prone manual API the DEC-348 ruling rejected), and
/// the friendly statics with no same-named native (`withLock`) were missing entirely. That is why
/// `withLock` shipped invisible to the editor and DEC-417's 100% bar was already broken before
/// `tryWithLock` existed.
pub(super) fn module_members(qualifier: &str) -> Vec<String> {
    let mut names: Vec<String> = native::registry()
        .iter()
        .filter(|n| !n.module.starts_with("Core.Native."))
        .filter(|n| n.module.rsplit('.').next() == Some(qualifier))
        .map(|n| n.name.to_string())
        .collect();
    names.extend(super::prelude_catalog::prelude_class_statics(qualifier));
    names.sort();
    names.dedup();
    names
}

/// Completable ATTRIBUTE names for the `#[` context, as `(label, detail)`.
///
/// Sourced from [`crate::ast::BUILTIN_ATTRIBUTE_PATHS`] — the same array the `is_*` recognizers are
/// defined against — so a new built-in attribute becomes completable with no edit here, exactly like a
/// new Core module or native. `qualified` selects the spelling: the bare leaf (`Entry`, the idiomatic
/// import-gated form) or the full canonical path (`Core.Runtime.Entry`, self-gating, needed when the
/// user is typing a dotted attribute path).
///
/// User-declared attributes (DEC-194: a class carrying `#[Attribute]`) are appended by
/// [`user_attributes`] — the built-in set alone would leave a user's own attributes uncompletable,
/// which Invariant 17's 100% rule counts as incomplete.
pub(super) fn builtin_attributes(qualified: bool) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = crate::ast::BUILTIN_ATTRIBUTE_PATHS
        .iter()
        .map(|(path, detail)| {
            let label = if qualified {
                (*path).to_string()
            } else {
                crate::ast::attr_path_leaf(path).to_string()
            };
            (label, (*detail).to_string())
        })
        .collect();
    out.sort();
    out
}

/// The names of classes in `program` that carry the `#[Attribute]` marker — user-defined attribute
/// types (DEC-194), completable at a `#[` use site exactly like a built-in. Sorted + deduped so the
/// rendered list is deterministic (Invariant 10).
pub(super) fn user_attributes(program: &Program) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for it in &program.items {
        let Item::Class(c) = it else { continue };
        if c.attrs
            .iter()
            .any(crate::ast::Attribute::is_attribute_marker)
        {
            out.push((c.name.clone(), "user-defined attribute".to_string()));
        }
    }
    out.sort();
    out.dedup();
    out
}

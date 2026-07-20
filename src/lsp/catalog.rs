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
/// — prelude classes (Date/Instant/Uri…) need the injected prelude program (a follow-up). Kind:
/// Method=2, Field/property=5.
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
                    if !p.modifiers.is_empty() {
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

/// The native members of the module whose qualifier (last dotted segment) equals `qualifier` —
/// e.g. `"List"` → `map`/`filter`/…, `"Output"` → `printLine`/`print`. Sorted + deduped; empty when
/// the qualifier names no Core module (the caller then falls back to general completion).
pub(super) fn module_members(qualifier: &str) -> Vec<String> {
    let mut names: Vec<String> = native::registry()
        .iter()
        .filter(|n| n.module.rsplit('.').next() == Some(qualifier))
        .map(|n| n.name.to_string())
        .collect();
    names.sort();
    names.dedup();
    names
}

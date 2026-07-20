//! LSP completion catalog — the ONE enumeration API over the Core registries (2026-07-20 alignment
//! pass). Sources of truth, never re-listed by hand:
//!   * importable module paths ← `cli::preludes::core_module_paths()` (derived from `CORE_MODULES`)
//!   * per-module members       ← `native::registry()` (the same registry the checker/transpile use)
//!
//! This keeps completion aligned with what the language actually accepts by construction: a new Core
//! module or native shows up in completion the moment it is registered, with no LSP edit. Project-source
//! package discovery (scanning the user's `src/`/`bin/`/`views/`/`vendor/`) is a follow-up increment
//! wired through `crate::loader`.
use crate::native;

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

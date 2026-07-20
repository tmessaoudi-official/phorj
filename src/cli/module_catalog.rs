//! Importable-module catalog — the LSP import-path completion source (2026-07-20 alignment pass).
//! Derived from `preludes::CORE_MODULES` so it never drifts from what `import` actually accepts: a new
//! Core module shows up in completion the moment it is registered, with no LSP edit. Kept out of
//! `preludes.rs` (already over the Invariant-13 hard cap) as a small sibling module.

/// Every importable `Core.*` module path (dotted, sorted, deduped). The `Core.Native.*` raw twins are
/// excluded — users import the friendly module (e.g. `Core.FileSystemModule`), not the raw twin.
pub(crate) fn core_module_paths() -> Vec<String> {
    let mut v: Vec<String> = super::preludes::CORE_MODULES
        .iter()
        .map(|vm| vm.module.join("."))
        .filter(|p| !p.starts_with("Core.Native."))
        .collect();
    v.sort();
    v.dedup();
    v
}

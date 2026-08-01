//! Importable-module catalog — the LSP import-path completion source (2026-07-20 alignment pass).
//! Derived from `preludes::CORE_MODULES` so it never drifts from what `import` actually accepts: a new
//! Core module shows up in completion the moment it is registered, with no LSP edit. Kept out of
//! `preludes.rs` (already over the Invariant-13 hard cap) as a small sibling module.

/// Every importable `Core.*` module path (dotted, sorted, deduped): the prelude/virtual modules
/// (`CORE_MODULES`) UNION the pure-native modules from `native::registry()` — an import target can be
/// either kind (`Core.Json` is a prelude twin; `Core.Output`/`Core.Map`/`Core.Math` are registry-only),
/// and listing only the prelude side silently hid every native-only module from import completion.
/// The `Core.Native.*` raw twins are excluded — users import the friendly module
/// (e.g. `Core.FileSystemModule`), not the raw twin.
pub(crate) fn core_module_paths() -> Vec<String> {
    let mut v: Vec<String> = super::preludes::CORE_MODULES
        .iter()
        .map(|vm| vm.module.join("."))
        .chain(
            crate::native::registry()
                .iter()
                .map(|n| n.module.to_string()),
        )
        .filter(|p| !p.starts_with("Core.Native."))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The names importable FROM the Core module at dotted `path` — the second half of an import, as in
/// `import Core.ErrorModule.RuntimeError;`. Sorted + deduped; empty when `path` names no Core module.
///
/// Same two sources as [`core_module_paths`], and for the same reason — either kind of module can be
/// imported from: a row's injected TYPE names (`bare_types`) and the natives registered under that
/// exact module path (`Core.Output.printLine`). Derived, so a new type or native is completable the
/// moment it is registered.
pub(crate) fn core_module_members(path: &str) -> Vec<String> {
    let mut v: Vec<String> = super::preludes::CORE_MODULES
        .iter()
        .filter(|vm| vm.module.join(".") == path)
        .flat_map(|vm| vm.bare_types.iter().map(|t| (*t).to_string()))
        .chain(
            crate::native::registry()
                .iter()
                .filter(|n| n.module == path)
                .map(|n| n.name.to_string()),
        )
        .collect();
    v.sort();
    v.dedup();
    v
}

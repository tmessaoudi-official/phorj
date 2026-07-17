//! DEC-273 — the EXTENSION layer: everything phorj-the-language can function without, shipped as
//! self-contained, flag-gated, Rust+JIT modules (self-hosting is NOT a goal — the build flag
//! gates INCLUSION, never implementation language or speed; `Core.Db` is the proof).
//!
//! Layout (AMENDMENT 2): one folder per extension — `src/ext/<name>/` holds its natives, its
//! prelude source, and its tests, colocated. (PHP-twin helper EMISSION still lives at the
//! transpiler's runtime-tables chokepoint for already-migrated extensions — the twins join the
//! folders when the transpile extension's own structural wave lands; the AMENDMENT-2 end state,
//! not yet the pilot's.) [`registry`] is THE
//! one-row-per-extension list; the `cli/preludes.rs` monolith dissolves as each extension
//! migrates here. Extensions KEEP the `Core.` import root (`Core.Ini` stays `Core.Ini`) — only
//! build membership and the flag change. Importing a compiled-out extension is a clean
//! `E-EXTENSION-DISABLED` naming the flag to add — never a runtime surprise.
//!
//! Tiers: MANDATORY (always in the default build and expected everywhere — `transpile`/`lift`
//! head the list, keeping the byte-identity spine's PHP leg in every gate build) · DEFAULT
//! (batteries-included, compiled in unless `--no-default-features`) · OPT-IN (explicit flag,
//! e.g. the DB drivers' heavier cousins). Which extensions are default vs opt-in is a recorded
//! future ruling; rows exist for every feature-gated capability; not-yet-migrated ruled extensions gain rows as the wave reaches them.

pub mod registry;

#[cfg(feature = "crypto")]
pub mod crypto;
#[cfg(feature = "csv")]
pub mod csv;
#[cfg(feature = "encoding")]
pub mod encoding;
#[cfg(feature = "ini")]
pub mod ini;
#[cfg(feature = "regex")]
pub mod regex;
/// The regex extension's PRELUDE source — compiled UNCONDITIONALLY (the `CORE_MODULES` const
/// array references it on every build; on a no-`regex` build the disabled-import gate rejects
/// `import Core.Regex;` long before the prelude could matter). Colocated with the extension in
/// spirit; unconditional in letter because a const array cannot be feature-spliced.
pub mod regex_prelude {
    pub const PRELUDE: &str = "class Regex { constructor(public string pattern) {} }";
}

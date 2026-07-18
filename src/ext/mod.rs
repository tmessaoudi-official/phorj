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

#[cfg(feature = "cryptography")]
pub mod crypto;
#[cfg(feature = "csv")]
pub mod csv;
#[cfg(feature = "database")]
pub mod db;
/// The db extension's prelude source — unconditional (see [`regex_prelude`]), colocated via `#[path]`.
#[path = "db/prelude.rs"]
pub mod db_prelude;
#[cfg(feature = "debug")]
pub mod debug;
/// The debug extension's prelude source — unconditional (see [`regex_prelude`]'s rationale),
/// colocated in the extension folder via `#[path]`.
#[path = "debug/prelude.rs"]
pub mod debug_prelude;
#[cfg(feature = "decimal")]
pub mod decimal;
#[cfg(feature = "encoding")]
pub mod encoding;
#[cfg(feature = "hash")]
pub mod hash;
#[cfg(feature = "http-client")]
pub mod http_client;
/// The http_client extension's prelude source — unconditional, colocated via `#[path]`.
#[path = "http_client/prelude.rs"]
pub mod http_client_prelude;
#[cfg(feature = "ini")]
pub mod ini;
#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "mail")]
pub mod mail;
/// The mail extension's prelude source — unconditional, colocated via `#[path]`.
#[path = "mail/prelude.rs"]
pub mod mail_prelude;
#[cfg(feature = "path")]
pub mod path;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "session")]
pub mod session;
/// The session extension's prelude source — unconditional, colocated via `#[path]`.
#[path = "session/prelude.rs"]
pub mod session_prelude;
#[cfg(feature = "test")]
pub mod test;
#[cfg(feature = "uri")]
pub mod uri;
/// The uri extension's prelude source — unconditional, colocated via `#[path]`.
#[path = "uri/prelude.rs"]
pub mod uri_prelude;
/// The regex extension's PRELUDE source — compiled UNCONDITIONALLY (the `CORE_MODULES` const
/// array references it on every build; on a no-`regex` build the disabled-import gate rejects
/// `import Core.Regex;` long before the prelude could matter). Colocated with the extension in
/// spirit; unconditional in letter because a const array cannot be feature-spliced.
pub mod regex_prelude {
    // `RegexMatch` (DEC-295) is the typed value handed to a `Regex.replaceCallback` callback — beats
    // PHP's untyped `$matches` array: `full()` is the whole match, `group(name)` is a named capture or
    // `null` (never a silent `""`). The native builds instances directly (hand-built value, like the
    // `Regex` carrier); these methods dispatch on both backends and transpile to a PHP `RegexMatch`
    // class. `import Core.Map;` mirrors HTTP_PRELUDE's cross-Core pattern (needed for `Map.get`).
    pub const PRELUDE: &str = r#"import Core.Map;
class Regex { constructor(public string pattern) {} }
class RegexMatch {
    constructor(public string matched, public Map<string, string> groups) {}
    function full(): string { return this.matched; }
    function group(string name): string? { return Map.get(this.groups, name); }
}"#;
}

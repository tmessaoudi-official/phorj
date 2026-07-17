//! DEC-273 — THE extension registry: one row per extension, the single source the compiler, the
//! `phg extensions` listing, `docs/EXTENSIONS.md`, and the disabled-import diagnostic all read.
//! A new extension = one row here (+ its `src/ext/<name>/` folder and Cargo feature).

/// How an extension participates in the default build (DEC-273 tiering; the MANDATORY/DEFAULT
/// split of the rich-methods family is a recorded future ruling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Always compiled into the default build and expected by the toolchain itself —
    /// `transpile`/`lift` head this list (the byte-identity spine's PHP leg stays in every
    /// gate/CI build; the jit-default precedent).
    Mandatory,
    /// Batteries-included: in the default feature set, absent only under `--no-default-features`.
    Default,
    /// Explicit opt-in flag (heavier deps or niche capability).
    OptIn,
}

impl Tier {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Tier::Mandatory => "mandatory",
            Tier::Default => "default",
            Tier::OptIn => "opt-in",
        }
    }
}

/// One extension row. `modules` lists every dotted Core module the extension provides (prelude
/// twins AND `Core.Native.*` internals) — the disabled-import gate matches imports against these.
pub struct Extension {
    /// The extension's short name. Usually also its `src/ext/<name>/` folder once migrated —
    /// EXCEPT the DEC-284 renames (`database`, `cryptography`) whose folders stay `src/ext/db/` /
    /// `src/ext/crypto/`: the folder rename is a deferred structural slice (it is spine-coupled —
    /// `tests/differential.rs` gates the byte-identity quarantine on the literal `db` dir name).
    pub name: &'static str,
    /// The Cargo feature gating build inclusion ("-" for a Mandatory row whose structural
    /// migration behind the seam is still queued — always compiled today).
    pub feature: &'static str,
    /// Compiled into THIS build?
    pub enabled: bool,
    pub tier: Tier,
    /// Dotted Core modules this extension provides (import surface + native twins).
    pub modules: &'static [&'static str],
    /// One-line human summary (the `phg extensions` listing + docs/EXTENSIONS.md).
    pub summary: &'static str,
    /// Physically migrated to `src/ext/<name>/` yet? (The preludes monolith dissolves row by
    /// row; `false` = still living in its pre-DEC-273 location, listed here for discovery.)
    pub migrated: bool,
}

/// THE list. Ordering: Mandatory first (transpile/lift head it per the ruling), then Default,
/// then OptIn; alphabetical within a tier.
///
/// Row SCOPE: one row per capability that is (or now is) Cargo-feature-gated. Ruled extensions
/// that are not yet flag-gated (Json/Csv/Uri/Path/Debug/Test/…) gain their rows as the DEC-273
/// migration wave reaches them — absence here means "not yet migrated", never "core".
/// Deliberately NOT rows: `green` (the green-thread SPAWN SEAM is ruled CORE — the ruling keeps
/// the seam in the kernel; only a structured-concurrency FRAMEWORK on top would be an extension)
/// and the `database-all` convenience alias (it is a feature GROUP, not a capability).
pub const EXTENSIONS: &[Extension] = &[
    Extension {
        name: "transpile",
        feature: "-",
        enabled: true,
        tier: Tier::Mandatory,
        modules: &[],
        summary: "the Phorj→PHP transpiler backend (`phg transpile`) — keeps the byte-identity \
                  spine's PHP leg in every build",
        migrated: false,
    },
    Extension {
        name: "lift",
        feature: "-",
        enabled: true,
        tier: Tier::Mandatory,
        modules: &[],
        summary: "the PHP→Phorj lifter (`phg lift`) — Tier-1/2 modernization drafts",
        migrated: false,
    },
    Extension {
        name: "cryptography",
        feature: "cryptography",
        enabled: cfg!(feature = "cryptography"),
        tier: Tier::Default,
        modules: &["Core.Cryptography"],
        summary: "password hashing (Argon2id) — the one crate-backed crypto primitive",
        migrated: true,
    },
    Extension {
        name: "csv",
        feature: "csv",
        enabled: cfg!(feature = "csv"),
        tier: Tier::Default,
        modules: &["Core.Csv"],
        summary: "CSV parse/render (RFC-4180 quoting) — DEC-273 migration wave",
        migrated: true,
    },
    Extension {
        name: "database",
        feature: "database",
        enabled: cfg!(feature = "database"),
        tier: Tier::Default,
        modules: &["Core.DatabaseModule", "Core.Native.Database"],
        summary: "multi-driver SQL (bundled SQLite; Postgres/MySQL via their own flags), typed \
                  hydration, transactions",
        migrated: true,
    },
    Extension {
        name: "debug",
        feature: "debug",
        enabled: cfg!(feature = "debug"),
        tier: Tier::Default,
        modules: &["Core.DebugModule", "Core.Native.Debug"],
        summary: "Debug.dump/dd value introspection (the walk-any-value SEAM stays core)",
        migrated: true,
    },
    Extension {
        name: "decimal",
        feature: "decimal",
        enabled: cfg!(feature = "decimal"),
        tier: Tier::Default,
        modules: &["Core.Decimal"],
        summary: "exact fixed-point `decimal` MODULE natives (the `1.50d` primitive itself is kernel)",
        migrated: true,
    },
    Extension {
        name: "encoding",
        feature: "encoding",
        enabled: cfg!(feature = "encoding"),
        tier: Tier::Default,
        modules: &["Core.Encoding"],
        summary: "base64/hex encode-decode — DEC-273 migration wave",
        migrated: true,
    },
    Extension {
        name: "hash",
        feature: "hash",
        enabled: cfg!(feature = "hash"),
        tier: Tier::Default,
        modules: &["Core.Hash"],
        summary: "MAC/KDF natives — hmac/equals/hkdf/pbkdf2 (std-only, RFC-KAT-gated)",
        migrated: true,
    },
    Extension {
        name: "ini",
        feature: "ini",
        enabled: cfg!(feature = "ini"),
        tier: Tier::Default,
        modules: &["Core.Ini"],
        summary: "INI config parsing (`Ini.parse`) — the DEC-273 pilot extension",
        migrated: true,
    },
    Extension {
        name: "jit",
        feature: "jit",
        enabled: cfg!(feature = "jit"),
        tier: Tier::Default,
        modules: &[],
        summary:
            "native codegen for hot int/float loops — CORE-classified (the kernel list); this \
                  row documents its BUILD FLAG for discoverability (DEC-273 addendum). `--no-jit` \
                  / artifact `PHG_NO_JIT=1` = the byte-identical escape hatches",
        migrated: false,
    },
    Extension {
        name: "json",
        feature: "json",
        enabled: cfg!(feature = "json"),
        tier: Tier::Default,
        modules: &["Core.Json"],
        summary: "JSON parse/render + the injected `Json` enum",
        migrated: true,
    },
    Extension {
        name: "path",
        feature: "path",
        enabled: cfg!(feature = "path"),
        tier: Tier::Default,
        modules: &["Core.Path"],
        summary: "pure path-string manipulation (join/normalize/…)",
        migrated: true,
    },
    Extension {
        name: "regex",
        feature: "regex",
        enabled: cfg!(feature = "regex"),
        tier: Tier::Default,
        modules: &["Core.Regex"],
        summary: "regular expressions (crate-backed; PCRE-compatible surface subset)",
        migrated: true,
    },
    Extension {
        name: "session",
        feature: "session",
        enabled: cfg!(feature = "session"),
        tier: Tier::Default,
        modules: &["Core.SessionModule", "Core.Native.Session"],
        summary: "in-process HTTP sessions for `phg serve` (secure-default cookie via Core.Http)",
        migrated: true,
    },
    Extension {
        name: "signals",
        feature: "signals",
        enabled: cfg!(feature = "signals"),
        tier: Tier::Default,
        modules: &[],
        summary: "SIGINT/SIGTERM graceful shutdown for `phg serve` (drain in-flight, exit 0)",
        migrated: false,
    },
    Extension {
        name: "test",
        feature: "test",
        enabled: cfg!(feature = "test"),
        tier: Tier::Default,
        modules: &["Core.Test"],
        summary: "the `Core.Test` assertion natives behind `phg test`",
        migrated: true,
    },
    Extension {
        name: "unicode",
        feature: "unicode",
        enabled: cfg!(feature = "unicode"),
        tier: Tier::Default,
        modules: &[],
        summary: "UAX #29 grapheme segmentation behind `String.graphemeLength`/`graphemes` \
                  (DEC-256 native-only tier)",
        migrated: false,
    },
    Extension {
        name: "uri",
        feature: "uri",
        enabled: cfg!(feature = "uri"),
        tier: Tier::Default,
        modules: &["Core.UriModule", "Core.Native.Uri", "Core.Url"],
        summary: "RFC 3986 URIs (DEC-240) — kernel + injected Uri class + the deprecated Core.Url compat twins",
        migrated: true,
    },
    Extension {
        name: "database-mysql",
        feature: "database-mysql",
        enabled: cfg!(feature = "database-mysql"),
        tier: Tier::OptIn,
        modules: &[],
        summary: "MySQL/MariaDB driver for the database extension",
        migrated: false,
    },
    Extension {
        name: "database-postgres",
        feature: "database-postgres",
        enabled: cfg!(feature = "database-postgres"),
        tier: Tier::OptIn,
        modules: &[],
        summary: "PostgreSQL driver for the database extension",
        migrated: false,
    },
    Extension {
        name: "http-client",
        feature: "http-client",
        enabled: cfg!(feature = "http-client"),
        tier: Tier::OptIn,
        modules: &["Core.HttpClientModule", "Core.Native.HttpClient"],
        summary: "outbound HTTP(S) client (rustls; DEC-264 same-host redirect hygiene)",
        migrated: true,
    },
    Extension {
        name: "mail",
        feature: "mail",
        enabled: cfg!(feature = "mail"),
        tier: Tier::OptIn,
        modules: &["Core.Mail", "Core.Native.Mail"],
        summary: "SMTP mail (lettre; DEC-265 TLS-or-refuse)",
        migrated: true,
    },
];

/// The rows whose modules are NOT compiled into this build — the disabled-import gate's input.
pub fn disabled() -> impl Iterator<Item = &'static Extension> {
    EXTENSIONS.iter().filter(|e| !e.enabled)
}

/// The listing body. `with_state = true` is the `phg extensions` CLI form (an "in this build"
/// column for THIS binary); `false` is the committed `docs/EXTENSIONS.md` form — deliberately
/// BUILD-INDEPENDENT (a committed doc cannot claim per-build state), which is what lets the
/// sync test run unconditionally under every feature combination.
#[must_use]
pub fn render_listing(with_state: bool) -> String {
    let mut out = String::from(
        "# Phorj extensions (DEC-273)\n\n\
         Generated by `phg extensions --docs` — do not edit by hand (a test keeps this file in\n\
         sync). Run `phg extensions` for the same table plus whether YOUR build carries each one.\n\n\
         The minimal CORE is what the language cannot function without; everything else is an\n\
         extension: Rust + JIT like the core, flag-gated for build inclusion only. Importing a\n\
         compiled-out extension is `E-EXTENSION-DISABLED`, naming the flag to add. Extensions\n\
         keep the `Core.` import root.\n\n",
    );
    if with_state {
        out.push_str(
            "| extension | tier | flag | in this build | provides | summary |\n|---|---|---|---|---|---|\n",
        );
    } else {
        out.push_str("| extension | tier | flag | provides | summary |\n|---|---|---|---|---|\n");
    }
    for e in EXTENSIONS {
        let modules = if e.modules.is_empty() {
            "—".to_string()
        } else {
            e.modules.join(", ")
        };
        let summary = e.summary.split_whitespace().collect::<Vec<_>>().join(" ");
        if with_state {
            out.push_str(&format!(
                "| {} | {} | `{}` | {} | {} | {} |\n",
                e.name,
                e.tier.label(),
                e.feature,
                if e.enabled { "yes" } else { "no" },
                modules,
                summary,
            ));
        } else {
            out.push_str(&format!(
                "| {} | {} | `{}` | {} | {} |\n",
                e.name,
                e.tier.label(),
                e.feature,
                modules,
                summary,
            ));
        }
    }
    out.push_str(
        "\nMigration status: rows marked by `src/ext/<name>/` folders are physically migrated; \
         the rest are listed for discovery and move with the DEC-273 migration wave.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `docs/EXTENSIONS.md` is generated from this registry (`phg extensions --docs >
    /// docs/EXTENSIONS.md`) — the explain-coverage pattern: the committed file must match the
    /// code's rendering. The docs form is build-independent (no per-build state column), so this
    /// runs under EVERY feature combination.
    #[test]
    fn docs_extensions_md_is_current() {
        let committed =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/EXTENSIONS.md"))
                .expect("docs/EXTENSIONS.md exists");
        assert_eq!(
            committed,
            render_listing(false),
            "docs/EXTENSIONS.md is stale — regenerate: `phg extensions --docs > docs/EXTENSIONS.md`"
        );
    }

    /// Registry hygiene: unique names, tier ordering (Mandatory → Default → OptIn), and the
    /// ruled MANDATORY heads (transpile, lift — the byte-identity spine's PHP leg).
    #[test]
    fn registry_rows_are_well_formed() {
        let mut seen = std::collections::BTreeSet::new();
        for e in EXTENSIONS {
            assert!(seen.insert(e.name), "duplicate extension name {}", e.name);
            assert!(!e.summary.is_empty(), "{} needs a summary", e.name);
        }
        assert_eq!(EXTENSIONS[0].name, "transpile");
        assert_eq!(EXTENSIONS[1].name, "lift");
        let tiers: Vec<Tier> = EXTENSIONS.iter().map(|e| e.tier).collect();
        let mut sorted = tiers.clone();
        sorted.sort_by_key(|t| match t {
            Tier::Mandatory => 0,
            Tier::Default => 1,
            Tier::OptIn => 2,
        });
        assert_eq!(
            tiers, sorted,
            "rows must be grouped Mandatory → Default → OptIn"
        );
        // Alphabetical within Default/OptIn (the Mandatory tier is ruled-ordered: transpile, lift).
        for tier in [Tier::Default, Tier::OptIn] {
            let names: Vec<&str> = EXTENSIONS
                .iter()
                .filter(|e| e.tier == tier)
                .map(|e| e.name)
                .collect();
            let mut sorted_names = names.clone();
            sorted_names.sort_unstable();
            assert_eq!(names, sorted_names, "{tier:?} rows must be alphabetical");
        }
    }
}

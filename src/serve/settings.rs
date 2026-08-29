//! S3.2 Part C — the flag-vs-config precedence rule for `phg serve` (DEC-455.14).
//!
//! **Ruled by the developer 2026-08-23: the CLI flag wins, but LOUDLY.** The registered
//! `Http.ServeConfig` is the DEFAULT source for the four settings the serve loop binds; a flag that
//! was explicitly passed and whose value DIFFERS overrides it, after printing one
//! `W-SERVE-CONFIG-OVERRIDDEN` line naming the field. That honours the precedence chain the spec
//! already writes (CLI > env > `#[Config]` > `phorj.json` > attribute default) *and* the repo's
//! no-silent-winner posture: the override still works, it just cannot be invisible.
//!
//! **Only FOUR fields are resolved here, and that is the whole surface this function binds:**
//! `host`+`port` (one address), `workers`, `timeout`.
//!
//! **`cert`/`key`/`tlsMinVersion` are read, but NOT here** (S3.5, D7). They go through
//! `serve::tls::requested` in `prepare_serve` instead, for a reason worth stating so nobody
//! "unifies" the two later: this function is the flag-vs-config PRECEDENCE rule, and D7 rules that
//! TLS has no CLI flag at all. With only one source there is no precedence to resolve, and threading
//! TLS through here would invent a conflict that cannot occur — while giving `ServeSettings` a field
//! whose type (`rustls::ServerConfig`) has neither `PartialEq` nor `Eq`.
//!
//! Still consumed by nobody, and still deliberately unwired: `maxBodySize` belongs to the wire parser
//! in `Core.Native.Http`, and `serverName` has no consumer at all. Wiring a field whose reader does
//! not exist would be a config that still does nothing.
//!
//! ## Provenance is approximated by VALUE, and that is a real limitation
//!
//! On the flag side we know whether a flag was passed (`Option::is_some`). On the config side we do
//! NOT: `new ServeConfig()` fills `port` with `8080`, and nothing distinguishes that from a program
//! that wrote `port: 8080` by hand. So a config field is treated as SET iff it differs from D4's
//! declared class default ([`class_defaults`]).
//!
//! Two consequences, both stated rather than discovered later:
//!   * `new ServeConfig(timeout: 0)` cannot express "no timeout" — `0` IS the class default, so it
//!     reads as unset and the CLI's 30s default applies. `--timeout 0` still disables it.
//!   * The same holds for any field written explicitly at its default value; the override notice
//!     will not fire for it, because no difference is observable.
//!
//! Recorded in KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE. The fix is a nullable field set in D4, which
//! changes a ruled class shape and is therefore a developer question, not this slice's to take.
//!
//! ## Why the CLI's timeout default is not the config's
//!
//! `phg serve` defaults `--timeout` to **30s** (GA blocker B4: an idle-socket guard, and what makes
//! keep-alive possible); D4 declares the class default **0 = no timeout**. Applying the config
//! unconditionally would therefore have silently disabled that guard for every existing server the
//! moment this slice landed. The differs-from-class-default rule is what keeps today's behaviour
//! exactly intact for `new ServeConfig()`.
use super::super::native::http::serve_register::ServeCfg;
use std::time::Duration;

/// `phg serve`'s address default — also `ServeConfig`'s `host`/`port` defaults, by construction.
pub const DEFAULT_ADDR: &str = "127.0.0.1:8080";
/// `phg serve --timeout`'s default in seconds. Deliberately NOT D4's class default — see the module doc.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// The stable code carried by every override notice, so it is greppable and `phg explain`-able.
pub const OVERRIDE_CODE: &str = "W-SERVE-CONFIG-OVERRIDDEN";

/// What the user actually typed. `None` = the flag was not passed at all, which is the distinction
/// the whole precedence rule turns on — so these must NOT be pre-defaulted by the caller.
#[derive(Debug, Default, Clone)]
pub struct ServeFlags {
    pub addr: Option<String>,
    pub timeout_secs: Option<u64>,
    pub workers: Option<usize>,
}

/// The resolved settings the serve loop binds, plus the notices to print before binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeSettings {
    pub addr: String,
    pub timeout: Option<Duration>,
    pub workers: usize,
    /// One line per field a passed flag overrode. Empty when nothing conflicted.
    pub notices: Vec<String>,
}

/// D4's declared class defaults, mirrored as the "unset" reference. Single-sourced with
/// `src/cli/serve_config_prelude.rs` — a change there without a change here is caught by
/// `class_defaults_match_the_prelude_source`.
#[must_use]
pub fn class_defaults() -> ServeCfg {
    ServeCfg {
        host: "127.0.0.1".to_string(),
        port: 8080,
        workers: 0,
        timeout: 0,
        cert: None,
        key: None,
        server_name: None,
        max_body_size: 8_388_608,
        tls_min_version: "1.2".to_string(),
    }
}

/// Apply the ruled precedence. `cores` is injected rather than queried so the resolution is a pure
/// function of its inputs — the serve loop passes `available_parallelism`.
#[must_use]
pub fn resolve(flags: &ServeFlags, cfg: Option<&ServeCfg>, cores: usize) -> ServeSettings {
    let defaults = class_defaults();
    let mut notices = Vec::new();

    // ── address (host + port are ONE setting: `--address` replaces both) ──────────────────────
    let base_addr = match cfg {
        Some(c) if c.host != defaults.host || c.port != defaults.port => {
            format!("{}:{}", c.host, c.port)
        }
        _ => DEFAULT_ADDR.to_string(),
    };
    let addr = match &flags.addr {
        Some(a) => {
            note(&mut notices, "--address", "host/port", &base_addr, a, cfg);
            a.clone()
        }
        None => base_addr,
    };

    // ── workers (0 is the AUTO sentinel on BOTH sides, so it is also the "unset" value) ───────
    let base_workers = match cfg {
        Some(c) if c.workers != defaults.workers => usize::try_from(c.workers).unwrap_or(cores),
        _ => cores,
    };
    let workers = match flags.workers {
        Some(w) => {
            note(
                &mut notices,
                "--workers",
                "workers",
                &base_workers.to_string(),
                &w.to_string(),
                cfg,
            );
            w
        }
        None => base_workers,
    };

    // ── timeout ───────────────────────────────────────────────────────────────────────────────
    let base_timeout = match cfg {
        // A NEGATIVE value falls back to the default rather than to `0`: `0` means "no timeout", so
        // `unwrap_or(0)` would let `timeout: -3` silently disable the B4 idle-socket guard — a typo
        // turning off a slowloris defence, which is the silent-failure shape this whole slice exists
        // to prevent. Nonsense reads as UNSET, matching what a negative `workers` already does. Real
        // range validation is still owed (KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE); until it lands,
        // fail SAFE.
        Some(c) if c.timeout != defaults.timeout => {
            u64::try_from(c.timeout).unwrap_or(DEFAULT_TIMEOUT_SECS)
        }
        _ => DEFAULT_TIMEOUT_SECS,
    };
    let timeout_secs = match flags.timeout_secs {
        Some(t) => {
            note(
                &mut notices,
                "--timeout",
                "timeout",
                &base_timeout.to_string(),
                &t.to_string(),
                cfg,
            );
            t
        }
        None => base_timeout,
    };

    ServeSettings {
        addr,
        timeout: (timeout_secs > 0).then(|| Duration::from_secs(timeout_secs)),
        workers: workers.max(1),
        notices,
    }
}

/// Push an override notice iff a config was registered AND the flag actually CHANGES the value.
/// A flag that merely restates what the config already said is not an override, and saying so would
/// train the reader to ignore the line.
fn note(
    out: &mut Vec<String>,
    flag: &str,
    field: &str,
    from: &str,
    to: &str,
    cfg: Option<&ServeCfg>,
) {
    if cfg.is_some() && from != to {
        out.push(format!(
            "serve: {flag} overrides ServeConfig.{field} ({from} → {to}) [{OVERRIDE_CODE}]"
        ));
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;

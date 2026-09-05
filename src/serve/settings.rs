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
//! ## Provenance is EXACT (DEC-475)
//!
//! On the flag side we know whether a flag was passed (`Option::is_some`), and since DEC-475 we know
//! the same on the config side: every `ServeConfig` field is nullable and defaults to `null`, so
//! `None` means the program did not write it and any value means it did. The effective defaults are
//! applied HERE, at the point of consumption, rather than being baked into the class.
//!
//! That replaces an approximation that could not be made correct: a field was previously treated as
//! SET iff it differed from the declared class default, so `new ServeConfig(timeout: 0)` could not
//! express "no timeout" (`0` WAS the default), and a value written by hand at its default got no
//! override notice. Both now behave as D4 describes. Range validation ([`validate`]) arrived with
//! the same ruling and for the same reason: refusing a nonsense number was impossible while the
//! class default was itself a value that had to be accepted.
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

/// The EFFECTIVE defaults for the two halves of the address, applied when the program left the
/// field `null` (DEC-475). They are the two components of [`DEFAULT_ADDR`]; a test pins that, so
/// setting one field cannot silently disagree with setting neither.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// See [`DEFAULT_HOST`].
pub const DEFAULT_PORT: i64 = 8080;

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

/// The `tlsMinVersion` applied when the field is left unset.
pub const DEFAULT_TLS_MIN_VERSION: &str = "1.2";

/// The diagnostic a `ServeConfig` field outside its legal range raises (DEC-475).
pub const E_SERVE_CONFIG_RANGE: &str = "E-SERVE-CONFIG-RANGE";

/// Reject a `ServeConfig` whose numbers cannot mean anything, BEFORE the serve loop binds a socket.
///
/// DEC-475 rules this validation in alongside the nullable field set, and the two belong together:
/// once `null` carries "unset", every non-null value is something the program stated on purpose, so
/// a nonsense one is a mistake to report rather than a value to quietly reinterpret. The previous
/// shape had no choice — `timeout: -3` fell back to the default, because refusing it would have
/// meant refusing the class default too.
///
/// Ranges, and why each is where it is:
/// * `port` 1..=65535 — 0 asks the OS to pick a port, which a server nobody can find is not.
/// * `workers` >= 0 — `0` is D4's AUTO sentinel (one per core); negative is a typo.
/// * `timeout` >= 0 — `0` means no timeout, which is now expressible; negative is a typo that used
///   to silently disable the idle-socket guard.
/// * `maxBodySize` >= 1 — a zero-byte cap rejects every request with a body, which no program means.
///
/// # Errors
/// One [`E_SERVE_CONFIG_RANGE`] message naming the field, its value and the legal range.
pub fn validate(cfg: &ServeCfg) -> Result<(), String> {
    let bad = |field: &str, value: i64, want: &str| {
        Err(format!(
            "{E_SERVE_CONFIG_RANGE}: ServeConfig `{field}` is {value}, which is outside {want}"
        ))
    };
    if let Some(p) = cfg.port {
        if !(1..=65535).contains(&p) {
            return bad("port", p, "1..=65535");
        }
    }
    if let Some(w) = cfg.workers {
        if w < 0 {
            return bad("workers", w, "0 or more (0 = one worker per core)");
        }
    }
    if let Some(t) = cfg.timeout {
        if t < 0 {
            return bad("timeout", t, "0 or more seconds (0 = no timeout)");
        }
    }
    if let Some(m) = cfg.max_body_size {
        if m < 1 {
            return bad("maxBodySize", m, "1 byte or more");
        }
    }
    Ok(())
}

/// Apply the ruled precedence. `cores` is injected rather than queried so the resolution is a pure
/// function of its inputs — the serve loop passes `available_parallelism`.
#[must_use]
pub fn resolve(flags: &ServeFlags, cfg: Option<&ServeCfg>, cores: usize) -> ServeSettings {
    let mut notices = Vec::new();

    // ── address (host + port are ONE setting: `--address` replaces both) ──────────────────────
    let base_addr = match cfg {
        Some(c) if c.host.is_some() || c.port.is_some() => format!(
            "{}:{}",
            c.host.as_deref().unwrap_or(DEFAULT_HOST),
            c.port.unwrap_or(DEFAULT_PORT)
        ),
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
    let base_workers = match cfg.and_then(|c| c.workers) {
        // `0` is the AUTO sentinel D4 declares — one worker per core — and it is now written
        // deliberately rather than being indistinguishable from "unset". A NEGATIVE value cannot
        // reach here: `validate` refuses it before the serve loop binds anything (DEC-475).
        Some(0) | None => cores,
        Some(w) => usize::try_from(w).unwrap_or(cores),
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
    let base_timeout = match cfg.and_then(|c| c.timeout) {
        // `timeout: 0` now means what it says — NO TIMEOUT — because `None` carries "unset". Before
        // DEC-475 the two were the same value and `0` was silently promoted to the 30s default, so
        // the one thing D4 declares the field for could not be expressed. A NEGATIVE value cannot
        // reach here: `validate` refuses it before the serve loop binds anything.
        Some(t) => u64::try_from(t).unwrap_or(DEFAULT_TIMEOUT_SECS),
        None => DEFAULT_TIMEOUT_SECS,
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

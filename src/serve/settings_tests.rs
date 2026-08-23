//! S3.2 Part C — the ruled precedence, pinned. Split from `settings.rs` per Invariant 13.
//!
//! The rule under test, in one line: **the registered `ServeConfig` is the default source; a flag
//! that was PASSED and whose value DIFFERS wins, loudly.**
use super::{
    class_defaults, resolve, ServeFlags, DEFAULT_ADDR, DEFAULT_TIMEOUT_SECS, OVERRIDE_CODE,
};
use crate::native::http::serve_register::ServeCfg;

const CORES: usize = 8;

fn cfg(mut edit: impl FnMut(&mut ServeCfg)) -> ServeCfg {
    let mut c = class_defaults();
    edit(&mut c);
    c
}

#[test]
fn with_no_config_and_no_flags_nothing_changes_from_today() {
    // The regression guard for every existing server: `phg serve app.phg` on a program that never
    // registers a config must bind exactly what it bound before this slice.
    let got = resolve(&ServeFlags::default(), None, CORES);
    assert_eq!(got.addr, DEFAULT_ADDR);
    assert_eq!(got.workers, CORES, "auto = one worker per core");
    assert_eq!(
        got.timeout,
        Some(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        "the B4 idle-socket guard must survive this slice"
    );
    assert!(
        got.notices.is_empty(),
        "nothing conflicted: {:?}",
        got.notices
    );
}

#[test]
fn an_all_default_config_is_indistinguishable_from_no_config() {
    // `new ServeConfig()` sets every field to its class default, so no field is observably SET —
    // and in particular D4's `timeout = 0` must NOT silently disable the CLI's 30s guard.
    let got = resolve(&ServeFlags::default(), Some(&class_defaults()), CORES);
    assert_eq!(got.addr, DEFAULT_ADDR);
    assert_eq!(got.workers, CORES);
    assert_eq!(
        got.timeout,
        Some(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        "an all-default config must not turn the timeout off"
    );
    assert!(got.notices.is_empty(), "{:?}", got.notices);
}

#[test]
fn the_config_binds_the_socket_when_no_flag_is_passed() {
    // The whole point of the slice: `Http.serve(new ServeConfig(port: 3000, workers: 4), h)` must
    // actually move the socket. Before Part C the registered config was inert.
    let c = cfg(|c| {
        c.port = 3000;
        c.workers = 4;
        c.timeout = 15;
    });
    let got = resolve(&ServeFlags::default(), Some(&c), CORES);
    assert_eq!(got.addr, "127.0.0.1:3000");
    assert_eq!(got.workers, 4);
    assert_eq!(got.timeout, Some(std::time::Duration::from_secs(15)));
    assert!(
        got.notices.is_empty(),
        "no flag was passed: {:?}",
        got.notices
    );
}

#[test]
fn a_configured_host_moves_the_bind_address_too() {
    let c = cfg(|c| c.host = "0.0.0.0".to_string());
    let got = resolve(&ServeFlags::default(), Some(&c), CORES);
    assert_eq!(got.addr, "0.0.0.0:8080");
}

#[test]
fn a_passed_flag_wins_and_says_so() {
    // The ruled behaviour: the flag wins, and the override is NOT silent.
    let c = cfg(|c| {
        c.port = 3000;
        c.workers = 4;
    });
    let flags = ServeFlags {
        addr: Some("127.0.0.1:8080".to_string()),
        workers: Some(2),
        timeout_secs: None,
    };
    let got = resolve(&flags, Some(&c), CORES);
    assert_eq!(got.addr, "127.0.0.1:8080", "the flag wins");
    assert_eq!(got.workers, 2, "the flag wins");
    assert_eq!(
        got.notices.len(),
        2,
        "one line per overridden field: {:?}",
        got.notices
    );
    let joined = got.notices.join("\n");
    for want in [
        "--address",
        "127.0.0.1:3000 → 127.0.0.1:8080",
        "--workers",
        "4 → 2",
        OVERRIDE_CODE,
    ] {
        assert!(joined.contains(want), "want {want:?} in\n{joined}");
    }
}

#[test]
fn a_flag_that_merely_restates_the_config_is_not_an_override() {
    // Warning fatigue is a real cost: a line that fires when nothing changed trains the reader to
    // ignore the line that matters.
    let c = cfg(|c| c.port = 3000);
    let flags = ServeFlags {
        addr: Some("127.0.0.1:3000".to_string()),
        ..ServeFlags::default()
    };
    let got = resolve(&flags, Some(&c), CORES);
    assert_eq!(got.addr, "127.0.0.1:3000");
    assert!(got.notices.is_empty(), "nothing changed: {:?}", got.notices);
}

#[test]
fn a_flag_without_a_registered_config_never_warns() {
    // A program that registers nothing has nothing to be overridden — the flags ARE the source.
    let flags = ServeFlags {
        addr: Some("0.0.0.0:9000".to_string()),
        workers: Some(3),
        timeout_secs: Some(5),
    };
    let got = resolve(&flags, None, CORES);
    assert_eq!(got.addr, "0.0.0.0:9000");
    assert_eq!(got.workers, 3);
    assert_eq!(got.timeout, Some(std::time::Duration::from_secs(5)));
    assert!(got.notices.is_empty(), "{:?}", got.notices);
}

#[test]
fn timeout_zero_from_the_flag_still_disables_the_timeout() {
    // `--timeout 0` is the documented off switch and must keep working — it is the flag side, where
    // provenance IS observable, so `0` there means "the user asked for none".
    let flags = ServeFlags {
        timeout_secs: Some(0),
        ..ServeFlags::default()
    };
    let got = resolve(&flags, Some(&class_defaults()), CORES);
    assert_eq!(got.timeout, None);
}

#[test]
fn a_negative_config_timeout_reads_as_unset_not_as_no_timeout() {
    // `timeout: -3` DIFFERS from the class default, so the value-provenance rule reads it as SET —
    // and a naive `u64::try_from(...).unwrap_or(0)` then turns the B4 idle-socket guard OFF. A typo
    // silently disabling a slowloris guard is exactly the shape this slice exists to prevent, so a
    // nonsense value falls back to UNSET (fail-safe), matching what a negative `workers` already
    // does. Real range validation is still owed — KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE.
    let c = cfg(|c| c.timeout = -3);
    let got = resolve(&ServeFlags::default(), Some(&c), CORES);
    assert_eq!(
        got.timeout,
        Some(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
        "a negative timeout must not read as `no timeout`"
    );
}

#[test]
fn a_negative_config_worker_count_reads_as_unset_too() {
    // The control for the test above: this path was already fail-safe, and must stay that way.
    let c = cfg(|c| c.workers = -2);
    let got = resolve(&ServeFlags::default(), Some(&c), CORES);
    assert_eq!(got.workers, CORES);
}

#[test]
fn class_defaults_match_the_prelude_source() {
    // The one thing that makes the value-provenance rule sound: `class_defaults()` must mirror what
    // `serve_config_prelude.rs` actually declares. If a default drifts there and not here, a field
    // silently changes from "unset" to "set" for every program — so pin it against the SOURCE, not
    // against a second copy of the numbers.
    let src = crate::cli::serve_config_prelude::SERVE_CONFIG_PRELUDE;
    let d = class_defaults();
    for want in [
        format!("host = \"{}\"", d.host),
        format!("port = {}", d.port),
        format!("workers = {}", d.workers),
        format!("timeout = {}", d.timeout),
        format!("tlsMinVersion = \"{}\"", d.tls_min_version),
    ] {
        assert!(
            src.contains(&want),
            "class_defaults() drifted from the prelude: {want:?} not in the source"
        );
    }
    assert!(
        src.contains("maxBodySize = 8_388_608"),
        "maxBodySize default drifted from the prelude"
    );
    assert_eq!(d.max_body_size, 8_388_608);
}

//! S3.2 Part C — the ruled precedence, pinned. Split from `settings.rs` per Invariant 13.
//!
//! The rule under test, in one line: **the registered `ServeConfig` is the default source; a flag
//! that was PASSED and whose value DIFFERS wins, loudly.**
use super::{
    resolve, validate, ServeFlags, DEFAULT_ADDR, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_TIMEOUT_SECS,
    DEFAULT_TLS_MIN_VERSION, E_SERVE_CONFIG_RANGE, OVERRIDE_CODE,
};
use crate::native::http::serve_register::ServeCfg;

const CORES: usize = 8;

/// A config with every field UNSET (DEC-475: `null` is the class default now), edited by the case.
/// That is what `new ServeConfig()` produces, so a test setting one field is testing exactly one
/// field — no longer "a field whose value happens to differ from the class default".
fn cfg(mut edit: impl FnMut(&mut ServeCfg)) -> ServeCfg {
    let mut c = unset();
    edit(&mut c);
    c
}

fn unset() -> ServeCfg {
    ServeCfg {
        host: None,
        port: None,
        workers: None,
        timeout: None,
        cert: None,
        key: None,
        server_name: None,
        max_body_size: None,
        tls_min_version: None,
    }
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
    // `new ServeConfig()` leaves every field `null`, so nothing is SET — and in particular the
    // timeout guard stays on. Before DEC-475 this test made the same assertion for a different
    // reason: every field carried a VALUE and "set" meant "differs from the class default".
    let got = resolve(&ServeFlags::default(), Some(&unset()), CORES);
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
        c.port = Some(3000);
        c.workers = Some(4);
        c.timeout = Some(15);
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
    let c = cfg(|c| c.host = Some("0.0.0.0".to_string()));
    let got = resolve(&ServeFlags::default(), Some(&c), CORES);
    assert_eq!(got.addr, "0.0.0.0:8080");
}

#[test]
fn a_passed_flag_wins_and_says_so() {
    // The ruled behaviour: the flag wins, and the override is NOT silent.
    let c = cfg(|c| {
        c.port = Some(3000);
        c.workers = Some(4);
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
    let c = cfg(|c| c.port = Some(3000));
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
    let got = resolve(&flags, Some(&unset()), CORES);
    assert_eq!(got.timeout, None);
}

#[test]
fn a_zero_config_timeout_now_means_no_timeout() {
    // THE case DEC-475 exists for. `timeout = 0` is what D4 declares "no timeout" to be, and until
    // the field set became nullable it was ALSO the class default — so writing it by hand read as
    // unset and the CLI's 30s guard applied instead. The one thing the field is for could not be
    // expressed. With `null` carrying "unset", `0` means what it says.
    let got = resolve(
        &ServeFlags::default(),
        Some(&cfg(|c| c.timeout = Some(0))),
        CORES,
    );
    assert_eq!(got.timeout, None, "`timeout: 0` must disable the timeout");
    // And leaving it alone still keeps the B4 idle-socket guard on.
    let unset_got = resolve(&ServeFlags::default(), Some(&unset()), CORES);
    assert_eq!(
        unset_got.timeout,
        Some(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    );
}

#[test]
fn a_config_field_written_at_its_effective_default_is_still_observably_set() {
    // The provenance property itself, which the value-comparison rule could not express: writing
    // `port: 8080` by hand is a decision, and a `--address` flag that overrides it must SAY so.
    let c = cfg(|c| c.port = Some(DEFAULT_PORT));
    let flags = ServeFlags {
        addr: Some("0.0.0.0:9000".to_string()),
        ..ServeFlags::default()
    };
    let got = resolve(&flags, Some(&c), CORES);
    assert_eq!(got.addr, "0.0.0.0:9000");
    assert!(
        got.notices.iter().any(|n| n.contains(OVERRIDE_CODE)),
        "overriding an explicitly-written default must still be announced: {:?}",
        got.notices
    );
}

#[test]
fn a_negative_number_is_refused_before_anything_binds() {
    // Nonsense used to fall back to the default, because refusing it would have meant refusing the
    // class default too — `-3` and `0` were both "not the default". Now they are distinguishable,
    // so the typo that silently disabled a slowloris guard is an error instead.
    for (edit, field) in [
        (
            Box::new(|c: &mut ServeCfg| c.timeout = Some(-3)) as Box<dyn FnMut(&mut ServeCfg)>,
            "timeout",
        ),
        (Box::new(|c: &mut ServeCfg| c.workers = Some(-2)), "workers"),
        (Box::new(|c: &mut ServeCfg| c.port = Some(0)), "port"),
        (Box::new(|c: &mut ServeCfg| c.port = Some(70_000)), "port"),
        (
            Box::new(|c: &mut ServeCfg| c.max_body_size = Some(0)),
            "maxBodySize",
        ),
    ] {
        let c = cfg(edit);
        let err = validate(&c).expect_err(&format!("{field} must be refused"));
        assert!(err.contains(E_SERVE_CONFIG_RANGE), "{err}");
        // The literal too, not just the constant: the code is what a user reads and what
        // `phg explain` is keyed on, and the surface ratchet counts a code as covered only when a
        // test names it as text.
        assert!(err.contains("E-SERVE-CONFIG-RANGE"), "{err}");
        assert!(
            err.contains(field),
            "the message must name the field: {err}"
        );
    }
    validate(&unset()).expect("an all-unset config is valid");
    validate(&cfg(|c| {
        c.timeout = Some(0);
        c.workers = Some(0);
        c.port = Some(65535);
        c.max_body_size = Some(1);
    }))
    .expect("the legal edges are legal");
}

#[test]
fn the_effective_defaults_match_what_the_prelude_documents() {
    // The nullable field set moved the DEFAULTS out of the class and into the consumers, so the
    // drift risk moved with them: the prelude now DOCUMENTS each effective default and this pins
    // that documentation against the constants the code actually applies. A default that changes in
    // one place and not the other is a field that silently behaves unlike what the editor shows.
    let src = crate::cli::serve_config_prelude::SERVE_CONFIG_PRELUDE;
    for want in [
        format!("Effective default \"{DEFAULT_HOST}\""),
        format!("Effective default {DEFAULT_PORT}"),
        format!("effective default {DEFAULT_TIMEOUT_SECS}"),
        format!("Effective default \"{DEFAULT_TLS_MIN_VERSION}\""),
    ] {
        assert!(
            src.contains(&want),
            "the prelude no longer documents this effective default: {want:?}"
        );
    }
    // Every field must actually BE nullable — the property the whole ruling rests on.
    for field in [
        "string? host = null",
        "int? port = null",
        "int? workers = null",
        "int? timeout = null",
        "int? maxBodySize = null",
        "string? tlsMinVersion = null",
        "RequestParsing? requestParsing = null",
    ] {
        assert!(src.contains(field), "not nullable in the prelude: {field}");
    }
    assert_eq!(DEFAULT_ADDR, format!("{DEFAULT_HOST}:{DEFAULT_PORT}"));
}

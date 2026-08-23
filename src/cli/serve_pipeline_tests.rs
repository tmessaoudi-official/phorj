//! The `phg serve` startup chain, pinned end to end **short of the blocking bind**: check → build
//! the handler factory → read the registered `ServeConfig` → resolve the ruled precedence.
//!
//! Why this file exists (6C finding, S3.2 Part C): `serve::settings`'s 9 unit tests all hand
//! `resolve` a config explicitly, so they prove the RULE and nothing about the WIRING. DEC-455.14's
//! first pinned fact — that the config is readable only AFTER the factory build, because the
//! factory's startup validation run is what executes the `Web` entry and populates the global — was
//! protected by a comment and one manual socket run. A comment does not go red when someone hoists
//! the read, and hoisting it makes every program's config silently inert: exactly the mutation class
//! the sabotage doctrine says the suite must notice.
//!
//! `CONFIG` is a process global; `cargo-nextest` runs one process per test, so each case starts from
//! a clean `None`.
use super::prepare_serve;
use crate::serve::ServeFlags;

/// A web program whose `ServeConfig` is unmistakable — no default, no CLI default, nothing else in
/// the repo binds it.
fn program(port: u16, workers: u8) -> crate::ast::Program {
    let src = format!(
        "package Main;\n\
         import Core.Http;\n\
         import Core.Http.Request;\n\
         import Core.Http.Response;\n\
         import Core.Http.ServeConfig;\n\
         import Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\n\
         #[Entry(kind: EntryKind.Web)]\n\
         function web(): void {{\n\
         \x20 Http.serve(new ServeConfig(port: {port}, workers: {workers}), function(Request req): Response {{\n\
         \x20   return Response.text(200, \"ok\");\n\
         \x20 }});\n\
         }}\n"
    );
    crate::cli::parse_program(&src).expect("the fixture parses")
}

#[test]
fn serve_settings_are_resolved_from_the_registered_config() {
    // The wiring pin. If the `config()` read is ever hoisted above the factory build it returns
    // `None`, the defaults win, and this reds — which is the whole point.
    let prog = program(42317, 3);
    let (_factory, settings) =
        prepare_serve(&prog, "", &ServeFlags::default(), true).expect("prepare");
    assert_eq!(
        settings.addr, "127.0.0.1:42317",
        "the registered ServeConfig must decide the bind address"
    );
    assert_eq!(settings.workers, 3, "and the worker count");
    assert!(
        settings.notices.is_empty(),
        "no flag was passed: {:?}",
        settings.notices
    );
}

#[test]
fn a_passed_flag_overrides_the_registered_config_through_the_real_chain() {
    let prog = program(42317, 3);
    let flags = ServeFlags {
        addr: Some("127.0.0.1:42318".to_string()),
        ..ServeFlags::default()
    };
    let (_factory, settings) = prepare_serve(&prog, "", &flags, true).expect("prepare");
    assert_eq!(settings.addr, "127.0.0.1:42318", "the flag wins");
    assert_eq!(
        settings.workers, 3,
        "an unflagged field still comes from the config"
    );
    let joined = settings.notices.join("\n");
    assert!(
        joined.contains("42317 → 127.0.0.1:42318") && joined.contains("W-SERVE-CONFIG-OVERRIDDEN"),
        "the override must be announced: {joined}"
    );
}

#[test]
fn a_program_that_registers_nothing_keeps_the_cli_defaults() {
    // The control: same chain, no `Http.serve` call at all → `E-SERVE-NO-HANDLER` at startup, i.e.
    // the factory REFUSES rather than handing back a config-less server. Pins that the ordering
    // change cannot be "fixed" by making the factory tolerate an unregistered program.
    let src = "package Main;\n\
               import Core.Runtime.Entry;\n\
               import Core.Runtime.EntryKind;\n\
               #[Entry(kind: EntryKind.Web)]\n\
               function web(): void {}\n";
    let prog = crate::cli::parse_program(src).expect("parses");
    // `expect_err` needs `Debug` on the Ok side, and a `HandlerFactory` is a boxed closure — match
    // instead of unwrapping.
    let err = match prepare_serve(&prog, src, &ServeFlags::default(), true) {
        Err(e) => e,
        Ok(_) => panic!("an unservable program must be refused before any bind"),
    };
    assert!(
        err.contains("E-SERVE-NO-HANDLER"),
        "want the startup refusal, got: {err}"
    );
}

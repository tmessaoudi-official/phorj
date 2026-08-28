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

#[test]
fn serve_on_a_cli_only_program_names_the_mismatch_and_the_run_verb() {
    // DEC-331 S3.4 (spec D6), the wiring pin for the serve half. `prepare_serve` — not `main.rs` —
    // is where the guard lives, precisely so this test can go red when it is removed.
    let cli_only = crate::cli::parse_program(
        "package Main;\n\
         import Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\n\
         #[Entry(kind: EntryKind.Cli)]\n\
         function main(): void { }\n",
    )
    .expect("the fixture parses");
    // `.err()`, not `.unwrap_err()`: the Ok type is a boxed handler factory and is not `Debug`.
    let err = prepare_serve(&cli_only, "", &ServeFlags::default(), true)
        .err()
        .expect("a CLI-only program cannot be served");
    assert!(
        err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
        "the wrong verb must be named as such: {err}"
    );
    assert!(err.contains("phg run"), "and the verb that works: {err}");
    assert!(
        !err.contains("E-SERVE-NO-HANDLER"),
        "a CLI program is NOT an unservable web program — that diagnosis sends the user to rewrite \
         a program that was already correct: {err}"
    );
}

#[test]
fn serve_on_a_library_still_reports_no_handler() {
    // The other side of the same coin: a program with NEITHER role is not a wrong-verb mistake, so
    // `E-SERVE-NO-HANDLER` must survive S3.4 intact.
    let lib =
        crate::cli::parse_program("package Main;\nfunction helper(int n): int { return n + 1; }\n")
            .expect("the fixture parses");
    let err = prepare_serve(&lib, "", &ServeFlags::default(), true)
        .err()
        .expect("a library has nothing to serve");
    assert!(err.contains("E-SERVE-NO-HANDLER"), "{err}");
    assert!(
        !err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
        "a library declares no role at all, so there is no mismatch: {err}"
    );
}

#[test]
fn serve_preamble_disables_stdin_which_is_why_the_role_guard_must_run_first() {
    // S3.4 ordering pin. `serve_cli` runs the role guard BEFORE `serve_preamble`, and the reason is
    // this assertion: the preamble disables stdin process-wide and `src/native/input.rs` offers NO
    // inverse, so a serve->run switch taken afterwards would run the user's CLI program with stdin
    // already dead — `Input.readLine()` returning null instead of the line they typed. That is not
    // what `phg run <file>` does, which breaks the very "what is displayed is what runs" promise the
    // preamble exists to keep in the other direction.
    //
    // If someone later hoists `serve_preamble` above the guard, this test still passes — it cannot
    // see call order. What it protects is the PREMISE: the day `set_stdin_disabled` stops being
    // one-way (or the preamble stops setting it), the comment justifying the ordering becomes false
    // and this goes red, which is the moment to revisit it.
    //
    // Safe as a process global: no other lib test executes a stdin read (verified by grep), and
    // nextest runs one process per test anyway.
    assert!(
        !crate::native::stdin_disabled(),
        "precondition: nothing has disabled stdin yet in this test process"
    );
    let profile = crate::cli::serve_preamble(false);
    assert!(
        crate::native::stdin_disabled(),
        "serve disables stdin (DEC-281) — the fact the S3.4 guard ordering depends on"
    );
    assert_eq!(
        profile,
        crate::profile::Profile::Release,
        "and a serve with no --dev is Release, not the Dev a `phg run` would have left set"
    );
}

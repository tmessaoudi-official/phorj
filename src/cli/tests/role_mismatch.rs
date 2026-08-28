//! Tests: DEC-331 S3.4 role-mismatch UX — `phg run` on a Web-only program and `phg serve` on a
//! Cli-only one report `E-NO-ENTRY-FOR-ROLE` naming the mismatch and the verb that would work.
//!
//! **These are the wiring pins, not just the rule.** The pure predicate is tested alongside, but the
//! cases below drive `cmd_run`/`cmd_treewalk` — the same entry points `phg run` uses — so deleting
//! the guard from the pipeline turns them red. A guard wired only into `main.rs` would be invisible
//! to this suite: nothing here executes `main.rs`.
use super::super::*;
use super::wp;

/// A web-only program: one `#[Entry(kind: EntryKind.Web)]` closure factory, no CLI entry.
const WEB_ONLY: &str = "package Main;\n\
     import Core.Http;\n\
     import Core.Http.Request;\n\
     import Core.Http.Response;\n\
     import Core.Http.ServeConfig;\n\
     import Core.Runtime.Entry;\n\
     import Core.Runtime.EntryKind;\n\
     #[Entry(kind: EntryKind.Web)]\n\
     function web(): void {\n\
     \x20 Http.serve(new ServeConfig(), function(Request req): Response {\n\
     \x20   return Response.text(200, \"ok\");\n\
     \x20 });\n\
     }\n";

#[test]
fn run_on_a_web_only_program_names_the_mismatch_and_the_serve_verb() {
    for (backend, err) in [
        ("vm", cmd_run(WEB_ONLY).unwrap_err()),
        ("tree-walker", cmd_treewalk(WEB_ONLY).unwrap_err()),
    ] {
        assert!(
            err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
            "{backend}: the mismatch must carry its code so `phg explain` can be reached: {err}"
        );
        assert!(
            err.contains("EntryKind.Cli"),
            "{backend}: it must name the role that is MISSING: {err}"
        );
        assert!(
            err.contains("EntryKind.Web"),
            "{backend}: and the role that is PRESENT — that is the whole diagnosis: {err}"
        );
        assert!(
            err.contains("phg serve"),
            "{backend}: and the verb that would have worked: {err}"
        );
    }
}

#[test]
fn a_library_with_no_entry_at_all_is_not_a_role_mismatch() {
    // The non-regression pin for `execution.rs`'s `library_file_without_main_…`: a program with
    // NEITHER role is not a wrong-verb mistake, it is a library. It must keep the old message, and
    // must never be offered a `phg serve` it has nothing to serve.
    let lib = wp("function helper(int n): int { return n + 1; }");
    let err = cmd_run(&lib).unwrap_err();
    assert!(err.contains("no entry point"), "{err}");
    assert!(
        !err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
        "a library is not a role mismatch: {err}"
    );
}

#[test]
fn a_reserved_kind_entry_is_not_a_role_mismatch() {
    // `entry_declared_role` is Active-only, so a `kind: Desktop` program has NO role at all and must
    // fall through to `E-ENTRY-KIND-RESERVED` rather than being told to run `phg serve`. Pinned
    // rather than left to the type: widening `EntryKind::Active` later would silently break it.
    let src = "package Main;\n\
         import Core.Runtime.Entry;\n\
         import Core.Runtime.EntryKind;\n\
         #[Entry(kind: EntryKind.Desktop)]\n\
         function app(): void { }\n";
    let err = cmd_run(src).unwrap_err();
    assert!(
        !err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
        "a reserved kind declares no active role, so there is no mismatch to report: {err}"
    );
    assert!(err.contains("E-ENTRY-KIND-RESERVED"), "{err}");
}

#[test]
fn the_role_mismatch_is_reported_before_type_errors() {
    // Ruled 2026-08-28 (plan §3): the guard runs BEFORE `check`. The verb is wrong regardless of the
    // program's type errors, and inverting this would mean checking twice on every `phg run`. This
    // program is BOTH web-only and type-broken; the mismatch is what the user must see.
    let broken = WEB_ONLY.replace(
        "return Response.text(200, \"ok\");",
        "return Response.text(200, 5);",
    );
    let err = cmd_run(&broken).unwrap_err();
    assert!(
        err.contains(crate::cli::E_NO_ENTRY_FOR_ROLE),
        "the wrong verb outranks the type error: {err}"
    );
    assert!(
        !err.contains("type error"),
        "and the check must not even have run: {err}"
    );
}

#[test]
fn detect_fires_only_when_the_other_role_is_present() {
    use crate::ast::EntryRole;
    let web = crate::cli::parse_program(WEB_ONLY).expect("the fixture parses");
    let cli_only = crate::cli::parse_program(&wp("function main(): void { }")).expect("parses");

    assert!(
        crate::cli::role_mismatch::detect(&web, EntryRole::Cli).is_some(),
        "wanting Cli from a web-only program is the mismatch"
    );
    assert!(
        crate::cli::role_mismatch::detect(&web, EntryRole::Web).is_none(),
        "wanting Web from a web program is no mismatch at all"
    );
    assert!(
        crate::cli::role_mismatch::detect(&cli_only, EntryRole::Web).is_some(),
        "and the rule is symmetric — that is the half D6 calls out explicitly"
    );
    assert!(
        crate::cli::role_mismatch::detect(&cli_only, EntryRole::Cli).is_none(),
        "wanting Cli from a CLI program is no mismatch"
    );
}

#[test]
fn only_a_plain_file_source_may_be_offered_the_switch_prompt() {
    // Ruled 2026-08-28 (plan §3). `phg run` cannot take a directory at all (`loader::load` reads a
    // FILE), while `phg serve <dir>` site-resolves to `<dir>/public/index.phg` — so a prompt naming
    // the directory would offer a command that cannot run. Directory, stdin and `-e` sources get the
    // coded error and no prompt.
    use crate::cli::role_mismatch::prompt_target;
    assert_eq!(
        prompt_target(&crate::cli::SourceSpec::File("app.phg".to_string())),
        Some("app.phg".to_string()),
        "a plain .phg file argument is the one prompt-eligible source"
    );
    assert_eq!(
        prompt_target(&crate::cli::SourceSpec::Stdin),
        None,
        "stdin has no file to name"
    );
    assert_eq!(
        prompt_target(&crate::cli::SourceSpec::Inline("x".to_string())),
        None,
        "`-e` has no file to name either, and `phg serve` accepts neither"
    );
}

#[test]
fn the_switch_is_offered_only_for_a_role_mismatch_with_a_nameable_source() {
    use crate::cli::role_mismatch::switch_command;
    let err = crate::cli::role_mismatch::message(&crate::cli::role_mismatch::Mismatch {
        wanted: crate::ast::EntryRole::Cli,
        found: crate::ast::EntryRole::Web,
    });
    assert_eq!(
        switch_command(&err, Some("app.phg"), "phg serve"),
        Some("phg serve app.phg".to_string()),
        "the prompt offers exactly the command it will run — no flags, so `y` runs serve with defaults"
    );
    assert_eq!(
        switch_command(&err, None, "phg serve"),
        None,
        "a source that cannot be named as an argument gets the diagnostic alone"
    );
    assert_eq!(
        switch_command(
            "compile error: no entry point: …",
            Some("app.phg"),
            "phg serve"
        ),
        None,
        "a library's absence is not a mismatch, so no switch may be offered for it"
    );
}

#[test]
fn the_switch_prompt_defaults_to_no() {
    use crate::cli::role_mismatch::answer_is_yes;
    for yes in ["y", "Y", "yes", " Yes \n"] {
        assert!(answer_is_yes(yes), "{yes:?} accepts");
    }
    // Switching verbs RUNS the user's program. A bare Enter, an EOF-emptied line, and anything
    // unrecognized must all decline — `[y/N]` is a promise, not decoration.
    for no in ["", "\n", "n", "N", "no", "sure", "yolo", "1"] {
        assert!(!answer_is_yes(no), "{no:?} must decline");
    }
}

#[test]
fn a_user_fault_that_merely_quotes_the_code_is_not_offered_a_switch() {
    // `switch_command` keys on the code, so it must key on it TIGHTLY. A program whose own output or
    // fault text contains `E-NO-ENTRY-FOR-ROLE` — a doc string, a test fixture, an error about the
    // error — must not cause `phg` to offer to run a different verb. Both guard paths return
    // `message()` verbatim, which BEGINS with the code, so `starts_with` loses nothing.
    use crate::cli::role_mismatch::switch_command;
    let quoting = format!(
        "runtime error: assertion failed: expected `{}` in the output",
        crate::cli::E_NO_ENTRY_FOR_ROLE
    );
    assert_eq!(
        switch_command(&quoting, Some("app.phg"), "phg serve"),
        None,
        "a fault that merely mentions the code is not a role mismatch: {quoting}"
    );
}

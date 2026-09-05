//! DEC-459 — prelude-internal bindings are ISOLATED from user imports.
//!
//! The friendly preludes import their raw natives under an alias (`import Core.Native.Http as
//! NativeHttp;`). Before DEC-459 that alias lived in the same namespace as the user's imports, so
//! (1) a user `import Core.Native.Http as Raw;` made the injection drop the prelude's own import
//! (same module path, alias ignored) and the serve prelude failed with `E-UNKNOWN-IDENT` at lines
//! the user cannot open; (2) a user alias SPELLED `NativeHttp` captured the prelude's calls; and
//! (3) `NativeHttp` / `NativeInput` / `NativeUri` / `NativeDebug` resolved in user code with no
//! import at all — "in the wind" (panel F6). The injected fragments now bind their aliases under a
//! spelling no user identifier can take, so none of the three can happen.

use phorj::cli;

fn program(extra_import: &str, body: &str) -> String {
    format!(
        "package Main;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\nimport Core.Output;\n\
         import Core.Http.ServeConfig;\n{extra_import}\n\
         #[Entry(kind: EntryKind.Cli)] function main() -> void {{\n  ServeConfig cfg = new ServeConfig();\n  {body}\n}}\n"
    )
}

/// (1) The DEC-459 repro: the user's own alias for the raw module coexists with the prelude's.
#[test]
fn a_user_alias_for_a_raw_native_module_no_longer_breaks_the_injected_prelude() {
    let src = program(
        "import Core.Native.Http as Raw;",
        "Output.printLine(\"{cfg.port ?? 8080} {Raw.decodePath(\\\"%41\\\")}\");",
    );
    let ok = cli::cmd_check(&src).unwrap_or_else(|e| panic!("must type-check clean:\n{e}"));
    assert!(ok.contains("OK"), "{ok}");
    let tw = cli::cmd_treewalk(&src).expect("interpreter runs");
    let vm = cli::cmd_run(&src).expect("vm runs");
    assert_eq!(tw, vm, "run ≡ tree-walker");
    assert_eq!(tw, "8080 A\n");
}

/// (2) A user alias spelled exactly like the prelude's qualifier binds the USER's module only — the
/// prelude keeps calling its own natives.
#[test]
fn a_user_alias_spelled_like_the_prelude_qualifier_cannot_capture_it() {
    let src = program(
        "import Core.Native.Input as NativeHttp;",
        "Output.printLine(\"{cfg.port ?? 8080} {NativeHttp.isInteractive()}\");",
    );
    let ok = cli::cmd_check(&src).unwrap_or_else(|e| panic!("must type-check clean:\n{e}"));
    assert!(ok.contains("OK"), "{ok}");
}

/// (3) Nothing in the wind: a prelude's qualifier is NOT reachable from user code without an import.
#[test]
fn prelude_qualifiers_are_not_in_user_scope() {
    let cases: &[(&str, &str)] = &[
        // (the friendly module the user imports, the leaked call that must be unknown)
        ("", "NativeHttp.decodePath(\"%41\")"),
        ("import Core.Input;", "NativeInput.isInteractive()"),
        ("import Core.UriModule;", "NativeUri.parse(\"http://x\")"),
        ("import Core.DebugModule;", "NativeDebug.dump(1)"),
    ];
    for (import, call) in cases {
        let src = program(
            import,
            &format!("Output.printLine(\"{{cfg.port ?? 8080}} {{{call}}}\");"),
        );
        let err = cli::cmd_check(&src).expect_err(&format!(
            "`{call}` must NOT resolve in user code (nothing in the wind)"
        ));
        assert!(
            err.contains("E-UNKNOWN-IDENT"),
            "`{call}` must fail as an unknown identifier, got:\n{err}"
        );
    }
}

/// The endorsed user spelling (`E-IMPORT-NATIVE-MEMBER`'s hint) keeps working: a user's own
/// `import Core.Native.Http as NativeHttp;` binds the USER's qualifier, independent of the prelude's
/// isolated one. (`E-UNUSED-IMPORT` is the loader's check over the RAW user file and never saw the
/// prelude either way — verified 2026-09-02 on both the pre- and post-DEC-459 binaries.)
#[test]
fn the_endorsed_user_spelling_binds_the_users_own_import() {
    let src = program(
        "import Core.Native.Http as NativeHttp;",
        "Output.printLine(\"{cfg.port ?? 8080} {NativeHttp.decodePath(\\\"%41\\\")}\");",
    );
    assert!(cli::cmd_check(&src).expect("clean").contains("OK"));
    assert_eq!(cli::cmd_run(&src).expect("runs"), "8080 A\n");
}

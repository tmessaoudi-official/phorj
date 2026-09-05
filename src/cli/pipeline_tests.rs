//! DEC-252 drift guard: `front_end_diagnostics` (the LSP path) must agree with `check_and_expand`
//! (the CLI path) on whether a program has errors.
//!
//! Split out of `pipeline.rs` per Invariant 13. `pipeline.rs` is a GRANDFATHERED over-cap file, and
//! `scripts/size-gate.sh` blocks growth on those (`= 922 > baseline 899 — split it, do not grow it`),
//! so adding the `#[Config]` fixtures required splitting rather than appending.

use super::*;

/// DEC-252 drift guard: `front_end_diagnostics` (the LSP path) and `check_and_expand` (the CLI
/// gate) MUST agree on error-presence for every program — they run the same injection + desugar
/// sequence, so if a future diagnostic-emitting pass is added to one but not the other, this fails.
#[test]
fn front_end_diagnostics_agrees_with_check() {
    // A front-end diagnostic is an ERROR unless its code is a `W-` warning.
    fn has_error(prog: &Program) -> bool {
        front_end_diagnostics(prog)
            .iter()
            .any(|d| d.code.is_none_or(|c| !c.starts_with("W-")))
    }
    let cases: &[(&str, bool)] = &[
        // (source, expect_error)
        ("package Main; function main() -> void {}", false),
        (
            "package Main; function main() -> void { var x = nope; }",
            true,
        ),
        // Injected-type program (the DEC-252 case): clean under both.
        (
            "package Main; import Core.Output; import Core.Secret; import Core.String; \
             function main() -> void { var t = new Secret(\"k\"); \
             Output.printLine(\"{String.length(t.expose())}\"); }",
            false,
        ),
        // Injected import + a genuine error: error under both.
        (
            "package Main; import Core.Secret; function main() -> void { var y = missing; }",
            true,
        ),
        // `#[Config]` entry injection (DEC-318 / DEC-331 S3.2 Part B) — the pre-check desugar the
        // LSP path must run too. A MULTI-parameter config entry with both providers present is
        // clean under `check` and must be clean under `front_end_diagnostics`: if the LSP path ever
        // stopped running `desugar_config`, the un-desugared entry would fail `E-ENTRY-SIG` there
        // while `phg check` said OK — silent DEC-252 drift, invisible to every other fixture here
        // (added 2026-08-07: the DEC-268 completeness lens found ZERO `E-CONFIG` coverage in the
        // whole suite, so this pass had no drift gate at all).
        (
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Runtime.Config; class A { } class B { } \
             #[Config] function pa() -> A { return new A(); } \
             #[Config] function pb() -> B { return new B(); } \
             #[Entry(kind: EntryKind.Cli)] function main(A a, B b) -> void { }",
            false,
        ),
        // The same shape with a provider MISSING: `E-CONFIG-MISSING` under both paths.
        (
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Runtime.Config; class A { } class B { } \
             #[Config] function pa() -> A { return new A(); } \
             #[Entry(kind: EntryKind.Cli)] function main(A a, B b) -> void { }",
            true,
        ),
        // DEC-331 S3.3b — a **Web** entry CARRYING config parameters. This is the interaction the
        // checker-level tests in `checker/tests/entry_point.rs` structurally cannot reach: they call
        // `errors_of`, which does not run the `desugar_config` PRE-check, so they only ever see the
        // already-zero-arg entry. Here the full pipeline runs, so this is the only place that proves
        // the two halves compose — desugar erases the parameters, and the widened role gate then
        // accepts the `(): void` that comes out. Before S3.3b this was `E-ENTRY-SIG`.
        (
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Runtime.Config; class Settings { } \
             #[Config] function provide() -> Settings { return new Settings(); } \
             #[Entry(kind: EntryKind.Web)] function web(Settings s) -> void { }",
            false,
        ),
        // …and the Web entry must NOT get a free pass on a genuinely missing provider: the config
        // machinery still applies to it exactly as it does to a Cli entry.
        (
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Runtime.Config; class Settings { } \
             #[Entry(kind: EntryKind.Web)] function web(Settings s) -> void { }",
            true,
        ),
        // DEC-331 S3.3a — the `Http.serve` fragment is a FOURTH injected `Core.Http` source, so it
        // flows through both paths from now on. A drift here (one path injecting it, the other not)
        // would make `Http.serve` resolve under `phg check` and squiggle red in the editor, or the
        // reverse — exactly the DEC-252 failure this table exists to catch.
        (
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Http; import Core.Http.Request; import Core.Http.Response; \
             import Core.Http.ServeConfig; \
             #[Entry(kind: EntryKind.Web)] function web() -> void { \
               Http.serve(new ServeConfig(), function(Request r): Response { \
                 return Response.text(200, \"ok\"); }); }",
            false,
        ),
    ];
    // The CODES the LSP path reports, in order — a bool cannot see multiplicity, and multiplicity is
    // exactly what this feature produces (one `E-CONFIG-MISSING` per unresolved parameter). The
    // DEC-268 completeness lens proved the boolean-only form was too weak by mutating
    // `front_end_diagnostics` to `.take(1)` — dropping every diagnostic but the first — and watching
    // this test still pass. It no longer does.
    fn fe_codes(prog: &Program) -> Vec<String> {
        front_end_diagnostics(prog)
            .iter()
            .filter(|d| d.code.is_none_or(|c| !c.starts_with("W-")))
            .map(|d| d.code.unwrap_or_default().to_string())
            .collect()
    }

    for (src, expect_error) in cases {
        let prog = lex_parse(src).expect("parse");
        let fe = has_error(&prog);
        let cli = check_and_expand(&prog, src).is_err();
        assert_eq!(
            fe, cli,
            "front_end_diagnostics vs check_and_expand disagree on `{src}` (fe={fe}, cli={cli})"
        );
        assert_eq!(fe, *expect_error, "wrong error-verdict for `{src}`");
    }

    // MULTIPLICITY gate: two unresolved config parameters must surface as TWO diagnostics on the LSP
    // path, not one. This is the DEC-252 property the boolean loop above structurally cannot check.
    // The program declares a provider for an UNRELATED type `C`, which is what makes it a config
    // entry at all: DEC-474 (as narrowed 2026-09-04) declines candidacy when a program declares NO
    // `#[Config]` provider, so without `C` this entry would report `E-ENTRY-SIG` and the
    // multiplicity property would have nothing to measure.
    let multi = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
                 import Core.Runtime.Config; class A { } class B { } class C { } \
                 #[Config] function c(): C { return new C(); } \
                 #[Entry(kind: EntryKind.Cli)] function main(A a, B b) -> void { }";
    let prog = lex_parse(multi).expect("parse");
    assert_eq!(
        fe_codes(&prog),
        vec!["E-CONFIG-MISSING", "E-CONFIG-MISSING"],
        "the LSP path must report one diagnostic PER unresolved config parameter"
    );
}

/// DEC-331 S3.3c — `import Core.Http;` synthesizes NO `respond` entry, for ANY web-entry shape.
///
/// This test used to assert the opposite for the legacy `(Request): Response` shape: the bridge was
/// the mechanism that made a bare `handle` servable, and five shipped `examples/web/*` depended on it.
/// S3.3c deleted it, so the assertion is inverted and the two shapes become one case — which is the
/// point of the retirement. What made it worth keeping as a test at all is the S3.3b defect it caught:
/// the bridge resolved the web entry by its DECLARED kind and spliced the name into
/// `handle(req).serialize()`, so once `(): void` became legal for `kind: Web` a well-formed D5 program
/// got `web(req).serialize()` — an arity error plus `type void has no method serialize`, reported
/// against the import line. A synthesized item that misreads the program is a whole class of bug; the
/// only structural cure is to synthesize nothing, and that is what is pinned here.
#[test]
fn importing_core_http_synthesizes_no_respond_entry_for_any_web_shape() {
    for (label, still_checks, src) in [
        (
            "D5 `(): void` factory",
            true,
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Http; \
             #[Entry(kind: EntryKind.Web)] function web() -> void { } \
             #[Entry(kind: EntryKind.Cli)] function main() -> void { }",
        ),
        (
            // DEC-455.12 (S3.3d): this shape no longer type-checks at all — the narrowing moved the
            // retirement from the serve RUNTIME to the CHECKER. It is kept as a case because the
            // "no bridge is synthesized" assertion below runs on the PARSED program and is still
            // meaningful: prelude injection happens before checking, so a bridge could in principle
            // still be spliced into a program that later fails.
            "the RETIRED `(Request): Response` shape",
            false,
            "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
             import Core.Http; import Core.Http.Request; import Core.Http.Response; \
             #[Entry(kind: EntryKind.Web)] function handle(Request r) -> Response { return Response.text(200, \"ok\"); }",
        ),
    ] {
        let prog = lex_parse(src).expect("parse");
        let ds = front_end_diagnostics(&prog);
        let codes: Vec<_> = ds
            .iter()
            .map(|d| format!("{:?}: {}", d.code, d.message))
            .collect();
        assert!(
            !codes
                .iter()
                .any(|m| m.contains("serialize") || m.contains("expects 0 argument")),
            "{label}: no bridge may be synthesized against it: {codes:?}"
        );
        match check_and_expand(&prog, src) {
            Ok(expanded) => {
                assert!(still_checks, "{label}: expected this shape to be REJECTED, but it checked");
                assert!(
                    !expanded
                        .items
                        .iter()
                        .any(|it| matches!(it, crate::ast::Item::Function(f) if f.name == "respond")),
                    "{label}: `respond` was RETIRED in S3.3c — nothing may synthesize it"
                );
            }
            Err(e) => {
                assert!(!still_checks, "{label}: expected this shape to CHECK: {e}");
                assert!(
                    e.to_string().contains("E-ENTRY-SIG"),
                    "{label}: the retired shape must be rejected by the ENTRY-SHAPE gate, not \
                     incidentally by something else: {e}"
                );
            }
        }
    }
}

/// A class whose name EQUALS its own module qualifier must not shadow the qualified TYPE form.
///
/// `Http.serve` (DEC-331 D5) forced the `Core.Http` prelude to define a `class Http`, while `Http` is
/// also that module's import qualifier — so `new Http.ServeConfig()` and `Http.serve(…)` now want the
/// same leading name to mean two different things. `Core.Input` has shipped the same shape since
/// DEC-281 (`class Input` under qualifier `Input`), but only its STATIC-METHOD half is exercised
/// anywhere: `Input.InputLines` is written in no test, example or doc, so the type half had no
/// coverage at all and the precedent did not actually cover this case.
///
/// Both spellings are pinned here because `serve_config_prelude` promises the spec's
/// `new Http.ServeConfig(...)` form in its own doc comment, and it type-checked clean BEFORE this
/// change [verified 2026-08-22 on `phg check`, exit 0] — so a regression would be a silent breakage
/// of a documented surface.
#[test]
fn a_class_named_like_its_qualifier_does_not_shadow_the_qualified_type_form() {
    for src in [
        // The qualified TYPE form, through the qualifier.
        "package Main; import Core.Http; import Core.Output; \
         function main(): void { var c = new Http.ServeConfig(); \
         Output.printLine(\"{c.port ?? 8080}\"); }",
        // The MEMBER-IMPORT spelling: `Http` is now a bare_type, so `import Core.Http.Http;` is a
        // legal import that binds the class alone. It must reach `Http.serve` without the whole-module
        // import — the "nothing in the wind" discipline says every symbol is import-gated, and this
        // pins that the new class is gated the same way every other injected type is.
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
         import Core.Http.Http; import Core.Http.Request; import Core.Http.Response; \
         import Core.Http.ServeConfig; \
         #[Entry(kind: EntryKind.Web)] function web(): void { \
           Http.serve(new ServeConfig(), function(Request r): Response { \
             return Response.text(200, \"ok\"); }); }",
        // The static-METHOD form, through the class of the same name, in the same program as the
        // type form — the two resolutions have to coexist within one file, not merely one repo.
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
         import Core.Http; import Core.Http.Request; import Core.Http.Response; \
         #[Entry(kind: EntryKind.Web)] function web(): void { \
           Http.serve(new Http.ServeConfig(), function(Request r): Response { \
             return Response.text(200, \"ok\"); }); }",
    ] {
        let prog = lex_parse(src).expect("parse");
        assert!(
            check_and_expand(&prog, src).is_ok(),
            "qualifier/class coexistence broke for: {src}"
        );
    }
}

/// DEC-486 (Invariant 17 `check ≡ LSP ≡ test`): a document containing a `test` item is checked in
/// TEST MODE by `phg check` and by the LSP — the item is accepted and its body type-checked — while
/// `run`/`transpile`/`build` keep rejecting it. Before this, every `selftest/*.phg` squiggled
/// `E-TEST-OUTSIDE-TESTS` in both editors on lines `phg test` accepted (panel C9/K1).
#[test]
fn a_document_with_a_test_item_is_checked_in_test_mode_by_check_and_the_lsp() {
    let clean = "package Main; test \"adds\" { var y = 1 + 2; }";
    let prog = lex_parse(clean).expect("parse");
    let fe = front_end_diagnostics(&prog);
    assert!(
        fe.is_empty(),
        "the LSP path must accept a `test` item (test mode), got {fe:?}"
    );
    assert!(
        check_program(&prog, clean).is_ok(),
        "`phg check` must accept a `test` item (test mode)"
    );
    let (json, had_errors) = check_json_program(&prog);
    assert!(
        !had_errors,
        "`phg check --json` must accept a `test` item: {json}"
    );

    // The body is still type-checked: a real error inside the test fires, and it is the BODY error —
    // never the outside-tests gate.
    let bad = "package Main; test \"bad\" { var y = 1 + true; }";
    let prog = lex_parse(bad).expect("parse");
    let fe = front_end_diagnostics(&prog);
    assert!(
        !fe.is_empty(),
        "a body type error inside a test must be reported"
    );
    assert!(
        fe.iter().all(|d| d.code != Some("E-TEST-OUTSIDE-TESTS")),
        "test mode must not raise the outside-tests gate: {fe:?}"
    );
    let err = check_program(&prog, bad).expect_err("body error under `phg check`");
    assert!(!err.contains("E-TEST-OUTSIDE-TESTS"), "{err}");
}

/// The boundary DEC-486 draws: test mode is a property of `check` and the editors ONLY. The run path
/// (`check_and_expand`, shared by run/transpile/build) and the bundle gate (`cmd_check`, the only
/// production caller of which is `bundle/cross.rs`) still reject a `test` item, so production code
/// cannot smuggle test blocks into a release.
#[test]
fn run_transpile_and_bundle_paths_still_reject_a_test_item() {
    let src = "package Main; test \"adds\" { var y = 1 + 2; }";
    let prog = lex_parse(src).expect("parse");
    let err = check_and_expand(&prog, src).expect_err("the run path must reject a `test` item");
    assert!(err.contains("E-TEST-OUTSIDE-TESTS"), "{err}");
    let err = cmd_check(src).expect_err("the bundle gate must reject a `test` item");
    assert!(err.contains("E-TEST-OUTSIDE-TESTS"), "{err}");
}

/// `phg check --json` is the editor-integration seam, and it used to call the RAW checker — no prelude
/// injection, no desugar — so a program `phg check` accepted came back as `unknown function \`Secret\``
/// [reproduced 2026-09-02 on the release binary]. It now routes through the same shared front end as
/// `phg check` and the LSP, and its error verdict is asserted on both fixtures because the exit code
/// is derived from it.
#[test]
fn check_json_routes_through_the_shared_front_end() {
    let injected = "package Main; import Core.Output; import Core.Secret; import Core.String; \
                    function main() -> void { var t = new Secret(\"k\"); \
                    Output.printLine(\"{String.length(t.expose())}\"); }";
    let prog = lex_parse(injected).expect("parse");
    let (json, had_errors) = check_json_program(&prog);
    assert!(
        !had_errors,
        "injected-prelude program must be `--json`-clean: {json}"
    );
    assert!(!json.contains("\"severity\":\"error\""), "{json}");

    let broken = "package Main; function main() -> void { var x = nope; }";
    let prog = lex_parse(broken).expect("parse");
    let (json, had_errors) = check_json_program(&prog);
    assert!(
        had_errors,
        "a genuine error must set the error verdict: {json}"
    );
    assert!(
        json.contains("E-UNKNOWN-IDENT") && json.contains("\"severity\":\"error\""),
        "{json}"
    );
}

/// INTERP-LINE-RESET / W0-5 (fixed 2026-09-02): a diagnostic raised INSIDE a string interpolation
/// names the real line and column. The interpolation sub-tokenizer restarts at 1:1; the parser now
/// re-bases `line`/`col` (not only `start`), so `phg check` no longer reports `1:1` for an error on
/// line 6, and the VM's fault lines agree with the interpreter (`tests/differential.rs`,
/// `interpolation_fault_line_matches_between_backends`, un-ignored the same day).
#[test]
fn a_diagnostic_inside_an_interpolation_names_the_real_line_and_column() {
    let src = "package Main;\nimport Core.Output;\n\nfunction main() -> void {\n    int n = 1;\n    Output.printLine(\"n = {n + nope}\");\n}\n";
    let prog = lex_parse(src).expect("parse");
    let err = check_and_expand(&prog, src).expect_err("unknown identifier");
    assert!(
        err.contains("at 6:32"),
        "expected the real position 6:32, got:\n{err}"
    );
    // The empty interpolation reports its own position too (it used to say `1:1`).
    let empty = "package Main;\nimport Core.Output;\nfunction main() -> void {\n    Output.printLine(\"{}\");\n}\n";
    let err = lex_parse(empty).expect_err("empty interpolation is a parse error");
    assert!(
        err.contains("at 4:24"),
        "expected the real position 4:24, got:\n{err}"
    );
}

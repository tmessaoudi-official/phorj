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
    let multi = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
                 import Core.Runtime.Config; class A { } class B { } \
                 #[Entry(kind: EntryKind.Cli)] function main(A a, B b) -> void { }";
    let prog = lex_parse(multi).expect("parse");
    assert_eq!(
        fe_codes(&prog),
        vec!["E-CONFIG-MISSING", "E-CONFIG-MISSING"],
        "the LSP path must report one diagnostic PER unresolved config parameter"
    );
}

/// DEC-331 S3.3b blast radius, caught by S3.3a's first failing test rather than by S3.3b's own.
///
/// The `Core.Http` respond-bridge substitutes the WEB ENTRY's name into `handle(req).serialize()`,
/// resolving that entry by its DECLARED kind. S3.3b legalized `(): void` for `kind: Web` without
/// narrowing the bridge, so a D5-shaped web entry got spliced in as `web(req).serialize()` — an
/// arity error plus `type void has no method serialize`, reported against `import Core.Http;` on a
/// program that is perfectly well-formed.
///
/// The bridge now filters on the STRUCTURAL shape (`entry_role`), which is the narrower question it
/// always needed and which only diverged from the declared kind in S3.3b.
#[test]
fn a_void_web_entry_does_not_get_the_legacy_respond_bridge() {
    let src = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
               import Core.Http; \
               #[Entry(kind: EntryKind.Web)] function web() -> void { } \
               #[Entry(kind: EntryKind.Cli)] function main() -> void { }";
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
        "the legacy bridge must not be synthesized for a `(): void` web entry: {codes:?}"
    );

    // CONTROL: the legacy `(Request): Response` web entry must STILL get the bridge — narrowing the
    // filter must not switch it off for the shape it exists to serve. Five shipped examples/web/*
    // depend on it until S3.3c.
    let legacy = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
                  import Core.Http; import Core.Http.Request; import Core.Http.Response; \
                  #[Entry(kind: EntryKind.Web)] function handle(Request r) -> Response { return Response.text(200, \"ok\"); }";
    let lp = lex_parse(legacy).expect("parse");
    let expanded = check_and_expand(&lp, legacy).expect("legacy web program still checks");
    assert!(
        expanded
            .items
            .iter()
            .any(|it| matches!(it, crate::ast::Item::Function(f) if f.name == "respond")),
        "the legacy (Request): Response entry must still get its `respond` bridge"
    );
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
         function main(): void { var c = new Http.ServeConfig(); Output.printLine(\"{c.port}\"); }",
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

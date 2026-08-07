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
    ];
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
}

//! DEC-220-S3 — `Output.capture` is an explicit, IMPORT-GATED primitive: reachable ONLY through the
//! user's own `import Core.Output;` (the same import `Output.printLine` already needs), so it can
//! never leak `Output.*` into a program that imported something else. These go through
//! `check_and_expand` (the CLI chokepoint) on purpose — that is where the Core preludes are injected,
//! and a leak, if one existed, would come from a prelude's top-level `import` merging into user scope
//! (the mechanism that sank the ruled `Response.capture` prelude wrapper). The leak-probe below is a
//! standing regression guard: it fails the moment any prelude re-adds `import Core.Output`.
use super::support::*;

fn expand(src: &str) -> Result<crate::ast::Program, String> {
    crate::cli::check_and_expand(&prog_raw(src), src)
}

#[test]
fn output_capture_resolves_under_its_import() {
    // With `import Core.Output;`, `Output.capture(fn)` resolves and type-checks: a `() -> void`
    // closure whose printed output is returned as the captured `string`.
    let src = "package Main; import Core.Output; \
         function main(): void { \
             string s = Output.capture(function(): void { Output.printLine(\"x\"); }); \
             Output.print(s); \
         }";
    assert!(
        expand(src).is_ok(),
        "Output.capture must resolve under `import Core.Output`; got:\n{:?}",
        expand(src).err()
    );
}

#[test]
fn output_not_nameable_without_its_import() {
    // THE LEAK-PROBE (DEC-220-S3): a program importing ONLY `Core.Http.Response` must still get an
    // unknown-identifier error for bare `Output` — no prelude imports `Core.Output`, so the capture
    // primitive (and every other `Output.*`) stays invisible unless the user imports it themselves.
    let src = "package Main; import Core.Http.Response; \
         function main(): void { Output.printLine(\"x\"); }";
    let err = expand(src).expect_err("bare `Output` must be unknown without `import Core.Output`");
    assert!(
        err.contains("E-UNKNOWN-IDENT") && err.contains("Output"),
        "expected an E-UNKNOWN-IDENT naming `Output`; got:\n{err}"
    );
}

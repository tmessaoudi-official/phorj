//! Tests: program execution (run/treewalk, check, parse, lex) and command help.
use super::super::*;
use super::{wp, SAMPLE};

#[test]
fn run_executes_sample() {
    assert_eq!(
        cmd_treewalk(SAMPLE).unwrap(),
        "Hello Tak\narea = 12.56636\narea = 12\n"
    );
}

#[test]
fn run_reports_type_error_and_does_not_execute() {
    // `area` returns float; returning an int literal is a type error.
    let src = wp(r#"import Core.Output;
function area(): float { return 1; } function main(): void { Output.printLine("{area()}"); }"#);
    let err = cmd_treewalk(&src).unwrap_err();
    assert!(err.contains("type error"), "{err}");
}

#[test]
fn run_reports_runtime_error() {
    let err = cmd_treewalk(&wp(r#"import Core.Output;
function main(): void { Output.printLine("{1 / 0}"); }"#))
    .unwrap_err();
    assert!(err.contains("runtime error"), "{err}");
}

#[test]
fn run_reports_parse_error() {
    let err = cmd_treewalk(&wp("function main( {")).unwrap_err();
    assert!(err.contains("parse error"), "{err}");
}

#[test]
fn library_file_without_main_checks_and_transpiles_but_run_errors_clearly() {
    // Batch-1 A: a library/web file with no `main` is valid — it type-checks and transpiles. Only
    // *running* needs an entry point; the interp/VM error names it clearly (not a bare "no main").
    let lib = wp("function helper(int n) -> int { return n + 1; }");
    assert!(cmd_check(&lib).unwrap().contains("OK"), "check should pass");
    assert!(
        cmd_transpile(&lib)
            .expect("transpile")
            .contains("function helper"),
        "transpile should emit the library function"
    );
    let run_err = cmd_treewalk(&lib).unwrap_err();
    assert!(
        run_err.contains("no entry point") && run_err.contains("#[Entry"),
        "run error: {run_err}"
    );
    let vm_err = cmd_run(&lib).unwrap_err();
    assert!(vm_err.contains("no entry point"), "vm error: {vm_err}");
}

#[test]
fn check_passes_on_clean_program() {
    let ok = cmd_check(SAMPLE).unwrap();
    assert!(ok.contains("OK"), "{ok}");
}

#[test]
fn check_fails_on_type_error() {
    let src = wp(r#"function f(): float { return 1; } function main(): void {}"#);
    assert!(cmd_check(&src).unwrap_err().contains("type error"));
}

#[test]
fn parse_dumps_ast() {
    let out = cmd_parse(r#"function main(): void {}"#).unwrap();
    assert!(out.contains("Program"), "{out}");
}

#[test]
fn lex_dumps_tokens() {
    let out = cmd_tokenize(r#"function main(): void {}"#).unwrap();
    assert!(out.contains("@ 1:1"), "{out}");
}

#[test]
fn help_for_known_command_has_examples_and_name() {
    let h = help_for("run");
    assert!(h.contains("examples:"), "{h}");
    assert!(h.contains("phg run"), "{h}");
}

#[test]
fn help_for_unknown_command_falls_back_to_top_level() {
    assert_eq!(help_for("bogus"), help_text());
}

#[test]
fn unimported_deprecated_attribute_is_import_gated() {
    // DEC-417: `#[Deprecated]` is import-gated like `#[Entry]`/`#[Config]` (DEC-337, nothing in the
    // wind). This lives at the CLI level ON PURPOSE: the gate is the `enforce_injected` pass inside
    // `check_and_expand`, which the checker's own unit harness never runs — asserting it there would
    // have passed for the wrong reason.
    let src = wp("#[Deprecated] function old(): int { return 1; }");
    let err = cmd_check(&src).unwrap_err();
    assert!(
        err.contains("E-INJECTED-TYPE-BARE"),
        "expected the injected-bare gate, got {err}"
    );
}

#[test]
fn a_deprecation_warning_never_gates_the_run() {
    // W-DEPRECATED rides the warning channel: the program still type-checks AND still runs (DEC-360 —
    // warnings never fail a build). If this ever starts failing, deprecation has become an error.
    let src = wp(r#"import Core.Output;
import Core.Runtime.Deprecated;
#[Deprecated(message: "use shout")] function yell(string s): string { return s; }
function main(): void { Output.printLine(yell("hi")); }"#);
    assert!(
        cmd_check(&src).unwrap().contains("OK"),
        "check must still pass"
    );
    assert_eq!(cmd_treewalk(&src).unwrap(), "hi\n");
}

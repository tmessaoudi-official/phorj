//! Tests: VM/interpreter parity, benchmarking, and disassembly.
use super::super::benchmark::{bench_report, bench_report_opts};
use super::super::*;
use super::wp;

#[test]
fn pipe_lambda_result_is_a_vm_operand() {
    // DEC-239 / Invariant 7 (CTy-operand trap): the contextual pipe lambda's param type is
    // materialized into the AST after checking, so the VM specializes `v * 2` exactly like the
    // interpreter — and the pipe RESULT is usable as an arithmetic operand on both backends.
    let src = wp(r#"import Core.Output;
function main(): void { int r = (5 |> (v => v * 2)) + 1; Output.printLine("{r}"); }"#);
    assert_eq!(cmd_run(&src).unwrap(), cmd_treewalk(&src).unwrap());
    assert_eq!(cmd_run(&src).unwrap(), "11\n");
}

#[test]
fn vm_matches_interpreter_on_simple_program() {
    let src = wp(r#"import Core.Output;
function main(): void { int x = 21; Output.printLine("{x + x}"); }"#);
    assert_eq!(cmd_run(&src).unwrap(), cmd_treewalk(&src).unwrap());
    assert_eq!(cmd_run(&src).unwrap(), "42\n");
}

#[test]
fn vm_leg_reports_type_error_via_the_gate() {
    let err = cmd_run(&wp(r#"function main(): void { int x = "no"; }"#)).unwrap_err();
    assert!(err.contains("type error"), "{err}");
}

#[test]
fn vm_leg_reports_runtime_error_with_prefix() {
    let err = cmd_run(&wp(r#"import Core.Output;
function main(): void { Output.printLine("{1 / 0}"); }"#))
    .unwrap_err();
    assert!(err.contains("runtime error"), "{err}");
}

#[test]
fn vm_leg_runtime_error_carries_source_line() {
    // div-by-zero in a statement on line 3. The VM now locates the fault via `Chunk.lines`
    // and renders `runtime error at 3: …`, while the canonical body ("division by zero")
    // stays intact so the differential `agree_err` oracle still classifies it identically.
    // NB: the division is *not* inside string interpolation — `split_interpolation`
    // re-lexes interpolated sub-expressions with a fresh tokenizer that resets to line 1, so a
    // fault inside `"{…}"` reports line 1 (a pre-existing interpolation-position limitation,
    // orthogonal to this task — see the M2 P3.5 roadmap decisions log).
    let src = wp("import Core.Output; function main(): void {\n    int z = 0;\n    int x = 1 / z;\n    Output.printLine(\"{x}\");\n}");
    let err = cmd_run(&src).unwrap_err();
    assert!(err.contains("division by zero"), "{err}");
    assert!(err.starts_with("runtime error at 3:"), "{err}");
}

#[test]
fn run_runtime_error_carries_line_via_trace() {
    // Error-handling slice 1 removed the old interpreter/VM asymmetry: the tree-walker now keeps a
    // logical call stack, so a runtime fault backfills the diagnostic line from the innermost
    // frame — the interpreter reports `runtime error at <line>: …`, matching the VM.
    let src = wp("import Core.Output; function main(): void {\n    int z = 0;\n    int x = 1 / z;\n    Output.printLine(\"{x}\");\n}");
    let err = cmd_treewalk(&src).unwrap_err();
    assert!(err.contains("division by zero"), "{err}");
    assert!(err.starts_with("runtime error at 3:"), "{err}");
}

#[test]
fn bench_reports_both_backends_with_identical_output() {
    // Small iteration count keeps the test fast; the report must name both backends, confirm
    // output identity (and the byte count it asserted), and end in a verdict comparing them.
    let src = wp(r#"import Core.Output;
function main(): void { int x = 21; Output.printLine("{x + x}"); }"#);
    let out = bench_report(&src, 5).expect("bench");
    assert!(out.contains("tree-walk run"), "{out}");
    assert!(out.contains("vm run"), "{out}");
    assert!(out.contains("identical on both backends"), "{out}");
    assert!(out.contains("verdict:"), "{out}");
    // Output is "42\n" = 3 bytes — the report states the byte count it asserted identical.
    assert!(out.contains("3 bytes"), "{out}");
}

#[test]
fn bench_vs_php_emits_a_php_section() {
    // `--vs-php` always emits a "vs PHP" section — either the comparison (php present) or a
    // graceful skip note (php absent). Both start with "vs PHP", so the test is host-agnostic.
    let src = wp(r#"import Core.Output;
function main(): void { int x = 21; Output.printLine("{x + x}"); }"#);
    let out = bench_report_opts(&src, 3, true, false).expect("bench");
    assert!(out.contains("vs PHP"), "{out}");
    // The standard report is still present.
    assert!(out.contains("vm run"), "{out}");
}

#[test]
fn bench_json_emits_a_machine_readable_object() {
    // `--json` (M-DOGFOOD W9) emits a JSON object of the measurements instead of the human report.
    let src = wp(r#"import Core.Output;
function main(): void { int x = 21; Output.printLine("{x + x}"); }"#);
    let out = bench_report_opts(&src, 3, false, true).expect("bench json");
    // Structural checks (no JSON dep in the lib): object shape + the required numeric keys.
    assert!(
        out.trim_start().starts_with('{') && out.trim_end().ends_with('}'),
        "{out}"
    );
    for key in [
        "\"iters\":",
        "\"output_bytes\":",
        "\"parse_check_ns\":",
        "\"compile_ns\":",
        "\"tree_walk_ns\":",
        "\"vm_ns\":",
        "\"vm_speedup\":",
        "\"php_ns\":",
    ] {
        assert!(out.contains(key), "missing {key} in {out}");
    }
    // Without --vs-php, php_ns is null; the human report headers must be absent.
    assert!(out.contains("\"php_ns\":null"), "{out}");
    assert!(
        !out.contains("phg benchmark —"),
        "json must not include the text header: {out}"
    );
}

#[test]
fn bench_reports_a_memory_section() {
    // Beyond timing, the report carries a memory block. The header is printed unconditionally
    // (the per-phase numbers are present on Linux, "unavailable" elsewhere), so asserting the
    // header keeps the test platform-independent.
    let src = wp(r#"import Core.Output;
function main(): void { Output.printLine("hi"); }"#);
    let out = bench_report(&src, 5).expect("bench");
    assert!(out.contains("memory"), "{out}");
}

#[test]
fn disasm_dumps_bytecode_with_mnemonics_and_annotations() {
    // The disassembler names the function, prints the type-specialized int-add op, the native
    // call op (the migrated former `Print`), and annotates a constant load with its value.
    let out = cmd_disassemble(&wp(
        r#"import Core.Output; function main(): void { int x = 1 + 2; Output.printLine("{x}"); }"#,
    ))
    .expect("disasm");
    assert!(out.contains("fn #"), "{out}");
    assert!(out.contains("main/0"), "{out}");
    assert!(out.contains("AddI"), "{out}");
    // `Output.printLine` lowers to `Op::CallNative`, annotated with the resolved native path.
    assert!(out.contains("CallNative"), "{out}");
    assert!(out.contains("Core.Output.printLine"), "{out}");
    // Const loads carry a `; <value>` annotation resolved from the pool.
    assert!(out.contains("Const(") && out.contains("; "), "{out}");
}

#[test]
fn disasm_propagates_type_error() {
    // A program that fails the gate can't be disassembled — the type error surfaces instead.
    let err = cmd_disassemble(&wp(r#"function main(): void { int x = "no"; }"#)).unwrap_err();
    assert!(err.contains("type error"), "{err}");
}

#[test]
fn bench_propagates_type_error_without_timing() {
    // A program that fails the gate can't be benchmarked — the error surfaces, no timing runs.
    let err = bench_report(&wp(r#"function main(): void { int x = "no"; }"#), 5).unwrap_err();
    assert!(err.contains("type error"), "{err}");
}

#[test]
fn bench_default_entry_uses_101_samples() {
    // The public entry runs the default-N path end to end (smoke test of `cmd_benchmark`).
    let out = cmd_benchmark(&wp(r#"import Core.Output;
function main(): void { Output.printLine("hi"); }"#))
    .expect("bench");
    assert!(out.starts_with("phg benchmark — median of 101"), "{out}");
}

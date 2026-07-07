//! Slice-1 JIT tests (run under `--features jit`). They prove the codegen substrate end-to-end: a
//! pure-int leaf function compiles to native code and produces the SAME value the VM oracle does, a
//! kernel fault surfaces with the SAME canonical string, and anything outside the subset is
//! default-denied. Byte-identity-under-`phg run` is the *next* (wiring) slice — these establish the
//! substrate the wiring rides on.

use super::{compile_and_run, JitError, JitRun};
use crate::chunk::BytecodeProgram;
use crate::value::Value;

/// Compile loose source through the real front-end (loader → check → compile), same helper shape the
/// VM tests use.
fn compile_source(src: &str) -> BytecodeProgram {
    let unit = crate::loader::load_loose_src(src).unwrap();
    let checked = crate::cli::check_and_expand(&unit.program, &unit.diag_src).unwrap();
    crate::compiler::compile(&checked).unwrap()
}

fn func_index(program: &BytecodeProgram, name: &str) -> usize {
    program
        .functions
        .iter()
        .position(|f| f.name == name)
        .unwrap_or_else(|| panic!("no compiled function `{name}`"))
}

/// `Value` has no `PartialEq` (closures/`Rc`) — compare ints by matching the variant.
fn as_int(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        other => panic!("expected int, got {}", other.type_name()),
    }
}

#[test]
fn jits_int_arithmetic_and_matches_vm_oracle() {
    let program = compile_source(
        "package Main;\n\
         function calc(int a, int b) -> int { return a * b + a - b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "calc");
    let args = vec![Value::Int(6), Value::Int(7)];

    // JIT result.
    let jit = match compile_and_run(&program, f, &args).expect("calc must be JIT-eligible") {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected fault: {m}"),
    };
    assert_eq!(jit, 6 * 7 + 6 - 7);

    // VM oracle result for the same entry + args — byte-identical value (Invariant 2).
    let (vm_val, _stdout) = crate::vm::Vm::new(&program)
        .run_entry(f, args)
        .expect("VM run_entry");
    assert_eq!(as_int(&vm_val), jit, "JIT value must match the VM oracle");
}

#[test]
fn jit_overflow_faults_with_the_shared_kernel_string() {
    let program = compile_source(
        "package Main;\n\
         function mul(int a, int b) -> int { return a * b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "mul");
    let args = vec![Value::Int(i64::MAX), Value::Int(2)];

    match compile_and_run(&program, f, &args).expect("mul must be JIT-eligible") {
        JitRun::Fault(msg) => assert_eq!(
            msg,
            crate::value::FAULT_INT_OVERFLOW,
            "the JIT fault must be the single-sourced kernel string"
        ),
        JitRun::Value(v) => panic!("expected an overflow fault, got {}", as_int(&v)),
    }
}

#[test]
fn jit_division_by_zero_faults_like_the_kernel() {
    let program = compile_source(
        "package Main;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "divi");

    // The kernel fault string for `x / 0` — take it from the VM oracle so the assertion tracks the
    // single source of truth rather than hardcoding it.
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, vec![Value::Int(1), Value::Int(0)])
        .expect_err("VM must fault on divide-by-zero");

    match compile_and_run(&program, f, &[Value::Int(1), Value::Int(0)]).expect("divi is eligible") {
        JitRun::Fault(msg) => assert!(
            vm_fault.render("").contains(&msg),
            "JIT div-by-zero fault `{msg}` must match the VM oracle trace:\n{}",
            vm_fault.render("")
        ),
        JitRun::Value(v) => panic!("expected a divide-by-zero fault, got {}", as_int(&v)),
    }
}

/// Run a JIT-eligible function and unwrap its int value, panicking on fault/ineligibility — the
/// common shape for the control-flow tests below.
fn jit_int(program: &BytecodeProgram, f: usize, args: &[Value]) -> i64 {
    match compile_and_run(program, f, args).expect("function must be JIT-eligible") {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected fault: {m}"),
    }
}

/// The VM oracle's int result for the same entry + args (Invariant 2).
fn vm_int(program: &BytecodeProgram, f: usize, args: Vec<Value>) -> i64 {
    let (v, _stdout) = crate::vm::Vm::new(program)
        .run_entry(f, args)
        .expect("VM run_entry");
    as_int(&v)
}

#[test]
fn jits_while_loop_matches_vm_oracle() {
    // A `while` loop exercises a back-edge (`Jump` to the loop header) and `SetLocal` on a mutable —
    // the heart of the memory-operand-stack control-flow design.
    let program = compile_source(
        "package Main;\n\
         function sumTo(int n) -> int {\n\
           mutable int s = 0;\n\
           mutable int i = 1;\n\
           while (i <= n) { s = s + i; i = i + 1; }\n\
           return s;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "sumTo");
    for n in [0_i64, 1, 5, 10] {
        let jit = jit_int(&program, f, &[Value::Int(n)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(n)]),
            "sumTo({n}) JIT must match the VM oracle"
        );
    }
}

#[test]
fn jits_if_else_selects_the_correct_branch() {
    // Distinguishable per-branch return values (111 vs 222) so a swapped JumpIfFalse true/false edge
    // is caught — not just "no fault" (advisor trap 3). Both edges are checked against the oracle.
    let program = compile_source(
        "package Main;\n\
         function pick(int a) -> int { if (a < 10) { return 111; } else { return 222; } }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "pick");
    for a in [3_i64, 9, 10, 42] {
        let jit = jit_int(&program, f, &[Value::Int(a)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(a)]),
            "pick({a}) JIT branch must match the VM oracle"
        );
    }
}

#[test]
fn jits_comparisons_and_not_match_the_vm_oracle() {
    // One function exercising Gt / Ge / Eq / Ne and `!(a < b)` (Lt + Not) — each contributes a
    // distinct bit, so a transposed dispatch code (Gt↔Ge, Eq↔Ne) or a swapped `Not` changes the
    // result and is caught, not just "no fault". The arg pairs hit BOTH edges of every comparison,
    // all checked against the VM oracle.
    let program = compile_source(
        "package Main;\n\
         function cmps(int a, int b) -> int {\n\
           mutable int r = 0;\n\
           if (a > b) { r = r + 1; }\n\
           if (a >= b) { r = r + 2; }\n\
           if (a == b) { r = r + 4; }\n\
           if (a != b) { r = r + 8; }\n\
           if (!(a < b)) { r = r + 16; }\n\
           return r;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "cmps");
    for (a, b) in [(5_i64, 3_i64), (3, 5), (4, 4), (-1, -1), (7, -2)] {
        let jit = jit_int(&program, f, &[Value::Int(a), Value::Int(b)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(a), Value::Int(b)]),
            "cmps({a},{b}) JIT must match the VM oracle"
        );
    }
}

#[test]
fn jits_function_with_an_unused_param() {
    // `b` is never referenced, but the stack must still be seeded with BOTH args (locals are stack
    // slots seeded from the arguments) — otherwise `GetLocal(0)` reads a slot the seed never created.
    let program = compile_source(
        "package Main;\n\
         function firstArg(int a, int b) -> int { return a; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "firstArg");
    let jit = jit_int(&program, f, &[Value::Int(7), Value::Int(99)]);
    assert_eq!(jit, 7);
    assert_eq!(
        jit,
        vm_int(&program, f, vec![Value::Int(7), Value::Int(99)])
    );
}

#[test]
fn jit_neg_overflow_faults_with_the_shared_kernel_string() {
    // Negating `i64::MIN` is a clean `"integer overflow"` via the shared `int_neg` kernel, never a
    // panic — byte-identical to `exec.rs`.
    let program = compile_source(
        "package Main;\n\
         function neg(int a) -> int { return -a; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "neg");
    match compile_and_run(&program, f, &[Value::Int(i64::MIN)]).expect("neg is eligible") {
        JitRun::Fault(msg) => assert_eq!(
            msg,
            crate::value::FAULT_INT_OVERFLOW,
            "the JIT negation fault must be the single-sourced kernel string"
        ),
        JitRun::Value(v) => panic!("expected an overflow fault, got {}", as_int(&v)),
    }
}

#[test]
fn non_int_function_is_default_denied() {
    // A body with output (`CallNative`/`Const` of a string) is outside the int-arith subset — the
    // default-deny predicate must reject it so callers fall back to the VM.
    let program = compile_source(
        "package Main;\n\
         import Core.Output;\n\
         function greet() -> void { Output.printLine(\"hi\"); }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "greet");
    assert!(
        matches!(
            compile_and_run(&program, f, &[]),
            Err(JitError::Unsupported(_))
        ),
        "a function outside the subset must be Unsupported, not compiled"
    );
}

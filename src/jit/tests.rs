//! Slice-1 JIT tests (run under `--features jit`). They prove the codegen substrate end-to-end: a
//! pure-int function (leaf or recursive) compiles to native code and produces the SAME value the VM
//! oracle does, a kernel fault surfaces with the SAME canonical string, calls compose across the
//! shared value stack, deep recursion faults with the VM's `"stack overflow"` at the same depth, and
//! anything outside the subset is default-denied. Byte-identity-under-`phg run` is the *next* (wiring)
//! slice — these establish the substrate the wiring rides on.

use super::{compile_and_run, Compiled, JitError, JitRun};
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

// --- slice 1(b2): native→native calls + self-recursion ---

#[test]
fn jits_recursive_fib_matches_vm_oracle() {
    // The headline b2 case: `fib` calls itself twice (`Call(self)` at two ips), so the whole thing
    // rides the native self-call path (FuncId resolved at finalize) + the shared value stack. Every
    // value is checked against the VM oracle across the base-case edge (n < 2) and the recursive one.
    let program = compile_source(
        "package Main;\n\
         function fib(int n) -> int {\n\
           if (n < 2) { return n; }\n\
           return fib(n - 1) + fib(n - 2);\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "fib");
    for n in [0_i64, 1, 2, 3, 5, 10, 15] {
        let jit = jit_int(&program, f, &[Value::Int(n)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(n)]),
            "fib({n}) JIT must match the VM oracle"
        );
    }
}

#[test]
fn jits_cross_function_call_matches_vm_oracle() {
    // Two DISTINCT functions in one module — `useAdd` calls `add1` twice (a cross-`FuncId` native
    // call, not just self-recursion), proving the multi-function module + the (pop arity, push result)
    // net stack effect composes for a non-recursive callee.
    let program = compile_source(
        "package Main;\n\
         function add1(int x) -> int { return x + 1; }\n\
         function useAdd(int x) -> int { return add1(x) + add1(x); }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "useAdd");
    for x in [0_i64, 1, 7, -3] {
        let jit = jit_int(&program, f, &[Value::Int(x)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(x)]),
            "useAdd({x}) JIT must match the VM oracle"
        );
    }
}

#[test]
fn jits_self_recursive_and_cross_call_together() {
    // A function that is BOTH self-recursive AND calls a second function in the same body — the union
    // of the fib and useAdd paths, which they exercise only separately. The machinery is uniform, but
    // this proves the two call kinds compose in one frame. Checked against the VM oracle.
    let program = compile_source(
        "package Main;\n\
         function base(int n) -> int { return n + 100; }\n\
         function rec(int n) -> int {\n\
           if (n < 1) { return base(n); }\n\
           return rec(n - 1) + base(n);\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "rec");
    for n in [0_i64, 1, 2, 4, 8] {
        let jit = jit_int(&program, f, &[Value::Int(n)]);
        assert_eq!(
            jit,
            vm_int(&program, f, vec![Value::Int(n)]),
            "rec({n}) JIT must match the VM oracle"
        );
    }
}

#[test]
fn jit_propagates_a_callee_fault() {
    // A fault raised inside a callee (`boom` divides by zero) must propagate up through the caller's
    // post-call status check (`status != 0` → fault-exit) unchanged — the distinct b2 branch from the
    // depth cap. Checked against the VM oracle's rendering, like the direct div-by-zero test.
    let program = compile_source(
        "package Main;\n\
         function boom(int a, int b) -> int { return a / b; }\n\
         function callsBoom(int a) -> int { return boom(a, 0); }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "callsBoom");
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, vec![Value::Int(10)])
        .expect_err("VM must fault: the callee divides by zero");

    match compile_and_run(&program, f, &[Value::Int(10)]).expect("callsBoom is eligible") {
        JitRun::Fault(msg) => assert!(
            vm_fault.render("").contains(&msg),
            "JIT callee-propagated fault `{msg}` must match the VM oracle trace:\n{}",
            vm_fault.render("")
        ),
        JitRun::Value(v) => panic!(
            "expected a propagated divide-by-zero fault, got {}",
            as_int(&v)
        ),
    }
}

#[test]
#[ignore = "timing measurement (best-of-N over VM fib(30) ≈ seconds); run manually with --ignored"]
fn measures_fib_native_jit_vs_vm() {
    // The G-8 mandate signal: is the (boxed, kernel-call) JIT actually faster than the VM on recursive
    // fib, and how close to release php+JIT? Native-JIT vs VM, IDENTICAL workload, best-of-N wall time.
    // Compile cost is reported SEPARATELY (never folded into the per-run number). PRINT-ONLY on timing —
    // a timing assertion would be a flaky, load-dependent gate; the ONLY assertion is that the native
    // value equals the VM oracle (a timing is meaningless until the value is proven identical). PHP+JIT
    // baseline: the recorded ~9.6 ms for fib(30) under Docker `php:8.5-cli` (release, JIT on) — the
    // on-box php is ZTS-debug JIT-off and unusable as a baseline (memory perf-benchmarking-truth). Peak
    // memory (the mandate's other column) is deferred to the proper `phg benchmark` JIT wiring.
    use std::time::Instant;
    let program = compile_source(
        "package Main;\n\
         function fib(int n) -> int {\n\
           if (n < 2) { return n; }\n\
           return fib(n - 1) + fib(n - 2);\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "fib");
    const N: i64 = 30;

    let t = Instant::now();
    let compiled = Compiled::compile(&program, f).expect("fib is JIT-eligible");
    let compile_ns = t.elapsed().as_nanos();

    let jit_val = match compiled.run(&[Value::Int(N)], 1) {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected fib fault: {m}"),
    };
    assert_eq!(
        jit_val,
        vm_int(&program, f, vec![Value::Int(N)]),
        "fib({N}) native-JIT value must equal the VM oracle before any timing is meaningful"
    );

    let best_native = (0..10)
        .map(|_| {
            let s = Instant::now();
            let _ = compiled.run(&[Value::Int(N)], 1);
            s.elapsed().as_nanos()
        })
        .min()
        .unwrap();
    let best_vm = (0..5)
        .map(|_| {
            let s = Instant::now();
            let _ = crate::vm::Vm::new(&program).run_entry(f, vec![Value::Int(N)]);
            s.elapsed().as_nanos()
        })
        .min()
        .unwrap();

    eprintln!(
        "[jit-bench] fib({N}) best-of-N wall time:\n  \
         compile     = {:.3} ms (one-time)\n  \
         native JIT  = {:.3} ms (best of 10)\n  \
         VM          = {:.3} ms (best of 5)\n  \
         php+JIT     = ~9.6 ms (recorded, Docker php:8.5-cli, release+JIT)\n  \
         speedup native-JIT vs VM = {:.2}x",
        compile_ns as f64 / 1e6,
        best_native as f64 / 1e6,
        best_vm as f64 / 1e6,
        best_vm as f64 / best_native as f64,
    );
}

#[test]
fn jit_deep_recursion_faults_like_the_vm_stack_overflow() {
    // Native recursion (unlike the VM's heap `frames`) would exhaust the OS stack — the `depth` cap
    // must fire first with the VM's `"stack overflow"` string. Runs on a 64 MB stack so 4096 native
    // frames fit, and asserts INSIDE the closure because `Value`/`JitRun` hold `Rc` (not `Send`) — the
    // program is rebuilt in-thread and only two `String`s cross the join. The fault is oracle-checked
    // against the VM (the string is a bare literal, not single-sourced, so drift must be caught here).
    const SRC: &str = "package Main;\n\
        function forever(int n) -> int { return forever(n + 1); }\n\
        function main() -> void {}";
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let program = compile_source(SRC);
            let f = func_index(&program, "forever");
            let vm_fault = crate::vm::Vm::new(&program)
                .run_entry(f, vec![Value::Int(0)])
                .expect_err("VM must fault with stack overflow")
                .render("");
            let jit = match compile_and_run(&program, f, &[Value::Int(0)])
                .expect("forever is eligible")
            {
                JitRun::Fault(m) => m,
                JitRun::Value(v) => panic!("expected stack overflow, got {}", as_int(&v)),
            };
            (vm_fault, jit)
        })
        .expect("spawn big-stack thread");
    let (vm_fault, jit) = handle.join().expect("big-stack thread panicked");
    assert!(
        vm_fault.contains(&jit),
        "JIT stack-overflow fault `{jit}` must match the VM oracle trace:\n{vm_fault}"
    );
}

// --- slice u1: UNBOXED leaf int codegen + fault parity (unboxed ≡ VM oracle; boxed stays the ref) ---

/// Compile+run a function through the UNBOXED path, unwrapping its int value (panicking on
/// fault/ineligibility). Distinct from `jit_int`, which drives the boxed path.
fn ub_int(program: &BytecodeProgram, f: usize, args: &[Value]) -> i64 {
    match Compiled::compile_unboxed(program, f)
        .expect("function must be unboxed-eligible")
        .run_unboxed(args, 1)
    {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected unboxed fault: {m}"),
    }
}

#[test]
fn unboxed_arithmetic_matches_vm_oracle() {
    // Pure int arithmetic through native registers (no boxed Vec, no helper calls). Checked against
    // the VM oracle across sign combinations.
    let program = compile_source(
        "package Main;\n\
         function calc(int a, int b) -> int { return a * b + a - b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "calc");
    for (a, b) in [(6_i64, 7_i64), (0, 0), (-3, 5), (5, -3), (1000000, 1000000)] {
        let ub = ub_int(&program, f, &[Value::Int(a), Value::Int(b)]);
        assert_eq!(
            ub,
            vm_int(&program, f, vec![Value::Int(a), Value::Int(b)]),
            "unboxed calc({a},{b}) must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_if_else_and_comparison_match_vm_oracle() {
    // Comparison (Lt) → JumpIfFalse → distinguishable int Consts per branch (a swapped edge changes
    // the result). Exercises native icmp + control flow, all int-returning.
    let program = compile_source(
        "package Main;\n\
         function pick(int a) -> int { if (a < 10) { return 111; } else { return 222; } }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "pick");
    for a in [3_i64, 9, 10, 42, -1] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(a)]),
            vm_int(&program, f, vec![Value::Int(a)]),
            "unboxed pick({a}) branch must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_bool_param_is_handled_natively() {
    // A `bool` PARAM (arrives as 0/1 i64, consumed only in a bool context — `if (b)`) is fine in the
    // unboxed path even though a bool *return* is rejected. Both int-returning branches checked.
    let program = compile_source(
        "package Main;\n\
         function choose(bool b, int n) -> int { if (b) { return n + 1; } return n + 2; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "choose");
    for (b, n) in [(true, 5_i64), (false, 5), (true, -1), (false, 100)] {
        assert_eq!(
            ub_int(&program, f, &[Value::Bool(b), Value::Int(n)]),
            vm_int(&program, f, vec![Value::Bool(b), Value::Int(n)]),
            "unboxed choose({b},{n}) must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_overflow_faults_like_the_kernel() {
    let program = compile_source(
        "package Main;\n\
         function mul(int a, int b) -> int { return a * b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "mul");
    match Compiled::compile_unboxed(&program, f)
        .expect("mul is unboxed-eligible")
        .run_unboxed(&[Value::Int(i64::MAX), Value::Int(2)], 1)
    {
        JitRun::Fault(m) => assert_eq!(m, crate::value::FAULT_INT_OVERFLOW),
        JitRun::Value(v) => panic!("expected overflow, got {}", as_int(&v)),
    }
}

#[test]
fn unboxed_div_zero_and_mod_zero_are_distinct_faults() {
    // The fault CODE→string mapping is the b1 transposition risk — a 2↔3 swap only shows if div-zero
    // and mod-zero are asserted as SEPARATE cases with their DISTINCT strings (advisor).
    let program = compile_source(
        "package Main;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function modi(int a, int b) -> int { return a % b; }\n\
         function main() -> void {}",
    );
    let dv = func_index(&program, "divi");
    let md = func_index(&program, "modi");
    match Compiled::compile_unboxed(&program, dv)
        .expect("divi eligible")
        .run_unboxed(&[Value::Int(1), Value::Int(0)], 1)
    {
        JitRun::Fault(m) => assert_eq!(m, crate::value::FAULT_DIV_ZERO, "div-by-zero string"),
        JitRun::Value(v) => panic!("expected div-zero, got {}", as_int(&v)),
    }
    match Compiled::compile_unboxed(&program, md)
        .expect("modi eligible")
        .run_unboxed(&[Value::Int(1), Value::Int(0)], 1)
    {
        JitRun::Fault(m) => assert_eq!(m, crate::value::FAULT_MOD_ZERO, "mod-by-zero string"),
        JitRun::Value(v) => panic!("expected mod-zero, got {}", as_int(&v)),
    }
}

#[test]
fn unboxed_min_over_neg_one_and_neg_min_overflow_like_the_kernel() {
    // The signed-overflow edge of div/rem and negation — i64::MIN / -1, i64::MIN % -1, -i64::MIN — all
    // clean FAULT_INT_OVERFLOW (never a native trap that would abort the process).
    let program = compile_source(
        "package Main;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function modi(int a, int b) -> int { return a % b; }\n\
         function neg(int a) -> int { return -a; }\n\
         function main() -> void {}",
    );
    for (name, args) in [
        ("divi", vec![Value::Int(i64::MIN), Value::Int(-1)]),
        ("modi", vec![Value::Int(i64::MIN), Value::Int(-1)]),
        ("neg", vec![Value::Int(i64::MIN)]),
    ] {
        let f = func_index(&program, name);
        // The VM oracle faults the same way (byte-identity of the edge).
        let vm_fault = crate::vm::Vm::new(&program)
            .run_entry(f, args.clone())
            .expect_err("VM must fault at the signed-overflow edge")
            .render("");
        match Compiled::compile_unboxed(&program, f)
            .expect("eligible")
            .run_unboxed(&args, 1)
        {
            JitRun::Fault(m) => {
                assert_eq!(
                    m,
                    crate::value::FAULT_INT_OVERFLOW,
                    "{name} overflow string"
                );
                assert!(
                    vm_fault.contains(&m),
                    "{name}: unboxed fault must match VM oracle"
                );
            }
            JitRun::Value(v) => panic!("{name}: expected overflow, got {}", as_int(&v)),
        }
    }
}

#[test]
fn unboxed_rejects_non_int_return() {
    // The type-erasure guard: a bool return (would mis-map to Value::Int), a bare UNPROVEN-int param
    // return (`identity` — n is never an int-arith operand, so unprovable), and a returned bool PARAM
    // (`retb` — proves the provenance pass does NOT over-mark) all fall back — compile_unboxed must
    // return Unsupported, never miscompile. (Mutable locals AND int loops are now eligible — see the
    // mutable-local + loop tests; self-/cross-recursive int functions too.)
    let program = compile_source(
        "package Main;\n\
         function isSmall(int n) -> bool { return n < 10; }\n\
         function identity(int n) -> int { return n; }\n\
         function retb(bool b, int n) -> bool { if (n > 0) { return b; } return b; }\n\
         function main() -> void {}",
    );
    for name in ["isSmall", "identity", "retb"] {
        let f = func_index(&program, name);
        assert!(
            matches!(
                Compiled::compile_unboxed(&program, f),
                Err(JitError::Unsupported(_))
            ),
            "unboxed must reject `{name}` (non-int-return), not miscompile"
        );
    }
}

// --- widen-1 c2: UNBOXED straight-line mutable locals (SetLocal + local decls; loops still rejected) ---

#[test]
fn unboxed_straightline_mutable_local_matches_vm() {
    // A mutable local (SetLocal + GetLocal of slot >= arity), straight-line (no loop), int-returning.
    // Locals are Cranelift Variables; the slot-kind pre-pass proves `a` int (every assignment is int),
    // so `return a` is accepted. Oracle-checked.
    let program = compile_source(
        "package Main;\n\
         function f(int x) -> int { mutable int a = x * 2; a = a + 3; return a; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    for x in [0_i64, 1, -4, 100, -100] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(x)]),
            vm_int(&program, f, vec![Value::Int(x)]),
            "unboxed f({x}) with a mutable local must match the VM oracle"
        );
    }
}

// --- ovf-spec byte-identity GUARDS (lock the fault behavior the speculative-overflow slice MUST
// preserve). Green under the CURRENT immediate-fault codegen; they go RED if a future wrapping +
// sticky-flag + VM-redo rewrite gets fault ORDERING or transient-overflow wrong. See the ovf-spec
// design + refinement in docs/plans/perf-wave.plan.md (all faults must funnel to a VM redo so the VM
// stays the single source of fault truth, preserving which fault fires and in what order). ---

#[test]
fn jit_overflow_before_div_zero_faults_with_overflow_not_div_zero() {
    // `a * a / b` with a*a overflowing AND b == 0: the VM faults at the OVERFLOW (a*a) and never
    // reaches the divide-by-zero. The JIT must produce that SAME first fault (overflow), NOT div-by-zero
    // — the fault-ORDERING invariant. A wrapping+sticky rewrite that emitted div-zero directly (because
    // the overflow was only recorded in the sticky flag) would break this; funnelling every fault to a
    // VM redo is what keeps it correct.
    let program = compile_source(
        "package Main;\n\
         function f(int a, int b) -> int { return a * a / b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    let args = vec![Value::Int(4_000_000_000), Value::Int(0)]; // 4e9 * 4e9 = 1.6e19 > i64::MAX

    // Sanity: the VM's FIRST fault here is overflow (proves a*a precedes /b in execution order).
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, args.clone())
        .expect_err("VM must fault");
    assert!(
        vm_fault
            .render("")
            .contains(crate::value::FAULT_INT_OVERFLOW),
        "sanity: the VM's first fault must be overflow, got:\n{}",
        vm_fault.render("")
    );

    match compile_and_run(&program, f, &args).expect("f must be JIT-eligible") {
        JitRun::Fault(msg) => assert_eq!(
            msg,
            crate::value::FAULT_INT_OVERFLOW,
            "JIT must fault OVERFLOW (order: a*a before /b), not divide-by-zero"
        ),
        JitRun::Value(v) => panic!("expected an overflow fault, got value {}", as_int(&v)),
    }
}

#[test]
fn jit_transient_cancelling_overflow_still_faults() {
    // `a * b - a * c` with b == c and a*b overflowing: under WRAPPING the two products cancel to 0, but
    // the VM faults at the first (checked) `a * b`. The JIT must fault too — a sticky-overflow flag that
    // forces a VM redo preserves this; a naive wrapping rewrite would silently return the wrapped 0.
    // Distinct params (b, c) defeat any CSE so both multiplies are genuinely emitted.
    let program = compile_source(
        "package Main;\n\
         function f(int a, int b, int c) -> int { return a * b - a * c; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    let args = vec![
        Value::Int(4_000_000_000),
        Value::Int(4_000_000_000),
        Value::Int(4_000_000_000),
    ];

    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, args.clone())
        .expect_err("VM must fault on the overflowing product");
    assert!(
        vm_fault
            .render("")
            .contains(crate::value::FAULT_INT_OVERFLOW),
        "sanity: VM must fault overflow, got:\n{}",
        vm_fault.render("")
    );

    match compile_and_run(&program, f, &args).expect("f must be JIT-eligible") {
        JitRun::Fault(msg) => assert_eq!(
            msg,
            crate::value::FAULT_INT_OVERFLOW,
            "JIT must fault on the (cancelling) overflow, not silently return the wrapped 0"
        ),
        JitRun::Value(v) => panic!(
            "expected an overflow fault, got wrapped value {}",
            as_int(&v)
        ),
    }
}

#[test]
fn unboxed_bool_local_not_returned_is_eligible() {
    // A mutable BOOL local (SetLocal from a comparison → Kind::Bool), used only in a bool context
    // (JumpIfFalse), NOT returned; the function returns an int. The slot-kind pre-pass must label the
    // slot Bool (so it can never be mis-returned as Value::Int) yet keep the function eligible.
    // Oracle-checked — the advisor's loop-carried-Bool shape, straight-line variant.
    let program = compile_source(
        "package Main;\n\
         function f(int a, int b) -> int { mutable bool flag = a < b; if (flag) { return 1; } return 0; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    for (a, b) in [(1_i64, 2_i64), (2, 1), (5, 5), (-3, 0)] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(a), Value::Int(b)]),
            vm_int(&program, f, vec![Value::Int(a), Value::Int(b)]),
            "unboxed f({a},{b}) with a bool local must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_call_result_into_local_matches_vm() {
    // Call → SetLocal → return-that-local: the slot-kind pre-pass must pop the callee's arity and push
    // Int for a Call (NOT clear the abstract stack), or `r` is mislabeled and the function over-rejects.
    // `r + g(x)` also places a Call directly after a GetLocal, exercising the arity-pop mid-expression.
    // Oracle-checked — the advisor's arity-pop desync catcher.
    let program = compile_source(
        "package Main;\n\
         function g(int x) -> int { return x + 1; }\n\
         function f(int x) -> int { mutable int r = g(x); return r + g(x); }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    for x in [0_i64, 3, -5, 41] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(x)]),
            vm_int(&program, f, vec![Value::Int(x)]),
            "unboxed f({x}) storing a call result must match the VM oracle"
        );
    }
}

// --- widen-1 c3: UNBOXED loops (back-edges + loop-carried Variables via Cranelift phis) ---

#[test]
fn unboxed_while_accumulator_matches_vm() {
    // The discriminating loop test: a mutable int accumulator + counter across a `while` back-edge.
    // `s` and `i` are loop-carried Variables whose header reads are dominated by the pre-loop def and
    // phi'd with the body's `SetLocal` — the whole point of the loops slice. compile_unboxed MUST accept
    // it (not fall back) AND match the VM oracle.
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
    // Must be unboxed-eligible now (a silent VM fallback would pass the value assert but prove nothing).
    assert!(
        Compiled::compile_unboxed(&program, f).is_ok(),
        "an int `while` accumulator must be unboxed-eligible after the loops slice"
    );
    for n in [0_i64, 1, 5, 10, 100] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "unboxed sumTo({n}) must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_loop_carried_bool_matches_vm() {
    // A loop-carried BOOL local (`go`, reassigned in-loop from a comparison → Kind::Bool) used as the
    // `while` condition, NOT returned; the function returns an int accumulator. Exercises a Bool
    // Variable phi'd across the back-edge AND the kind analysis keeping `go` non-Int (so it can never be
    // mis-returned) while `acc` stays Int. Oracle-checked (the advisor's loop-carried-Bool shape).
    let program = compile_source(
        "package Main;\n\
         function f(int n) -> int {\n\
           mutable int acc = 0;\n\
           mutable int i = 0;\n\
           mutable bool go = i < n;\n\
           while (go) { acc = acc + i; i = i + 1; go = i < n; }\n\
           return acc;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    assert!(
        Compiled::compile_unboxed(&program, f).is_ok(),
        "an int accumulator with a loop-carried bool condition must be unboxed-eligible"
    );
    for n in [0_i64, 1, 3, 7] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "unboxed f({n}) with a loop-carried bool must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_overflow_mid_loop_faults_like_vm() {
    // A loop that overflows at a specific iteration must fault with the SAME kernel string at the SAME
    // iteration as the VM (the `smul_overflow` check fires per-iteration). `p = p * 2` overflows well
    // before i reaches 100.
    let program = compile_source(
        "package Main;\n\
         function powish(int n) -> int {\n\
           mutable int p = 1;\n\
           mutable int i = 0;\n\
           while (i < n) { p = p * 2; i = i + 1; }\n\
           return p;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "powish");
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, vec![Value::Int(100)])
        .expect_err("VM must overflow in the loop");
    match Compiled::compile_unboxed(&program, f)
        .expect("int while is eligible")
        .run_unboxed(&[Value::Int(100)], 1)
    {
        JitRun::Fault(msg) => assert!(
            vm_fault.render("").contains(&msg),
            "unboxed loop overflow `{msg}` must match the VM oracle:\n{}",
            vm_fault.render("")
        ),
        JitRun::Value(v) => panic!("expected overflow, got {}", as_int(&v)),
    }
    // And a non-overflowing run still matches the oracle exactly.
    assert_eq!(
        ub_int(&program, f, &[Value::Int(10)]),
        vm_int(&program, f, vec![Value::Int(10)]),
        "unboxed powish(10) must match the VM oracle"
    );
}

#[test]
fn unboxed_div_zero_mid_loop_faults_like_vm() {
    // A divide-by-zero reached only on a specific loop iteration (when `n - i` hits 0) must fault with
    // the same kernel string as the VM — the fault is inside the back-edge body, not the straight line.
    let program = compile_source(
        "package Main;\n\
         function acc(int n) -> int {\n\
           mutable int a = 0;\n\
           mutable int i = 0;\n\
           while (i <= n) { a = a + 100 / (n - i); i = i + 1; }\n\
           return a;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "acc");
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, vec![Value::Int(3)])
        .expect_err("VM must divide by zero when i == n");
    match Compiled::compile_unboxed(&program, f)
        .expect("int while is eligible")
        .run_unboxed(&[Value::Int(3)], 1)
    {
        JitRun::Fault(msg) => assert!(
            vm_fault.render("").contains(&msg),
            "unboxed loop div-by-zero `{msg}` must match the VM oracle:\n{}",
            vm_fault.render("")
        ),
        JitRun::Value(v) => panic!("expected div-by-zero, got {}", as_int(&v)),
    }
}

#[test]
fn unboxed_all_comparisons_and_not_match_vm_oracle() {
    // Every comparison arm (Gt/Ge/Le/Eq/Ne + Lt via `nt`'s `!(a<b)`) and `Not` — each a hand-written
    // `Op → IntCC` mapping, the b1 transposition-trap family. Branch-return leaf form (u1-legal: no
    // SetLocal), distinguishable per edge, oracle-checked vs the VM. A Le↔Ge / Eq↔Ne swap or an
    // operand-order flip changes a result and is caught here.
    let program = compile_source(
        "package Main;\n\
         function gt(int a, int b) -> int { if (a > b) { return 1; } return 0; }\n\
         function ge(int a, int b) -> int { if (a >= b) { return 1; } return 0; }\n\
         function le(int a, int b) -> int { if (a <= b) { return 1; } return 0; }\n\
         function eq(int a, int b) -> int { if (a == b) { return 1; } return 0; }\n\
         function ne(int a, int b) -> int { if (a != b) { return 1; } return 0; }\n\
         function nt(int a, int b) -> int { if (!(a < b)) { return 1; } return 0; }\n\
         function main() -> void {}",
    );
    for name in ["gt", "ge", "le", "eq", "ne", "nt"] {
        let f = func_index(&program, name);
        for (a, b) in [(5_i64, 3_i64), (3, 5), (4, 4), (-2, 7), (7, -2)] {
            assert_eq!(
                ub_int(&program, f, &[Value::Int(a), Value::Int(b)]),
                vm_int(&program, f, vec![Value::Int(a), Value::Int(b)]),
                "unboxed {name}({a},{b}) must match the VM oracle"
            );
        }
    }
}

#[test]
fn unboxed_add_and_sub_overflow_fault_like_the_kernel() {
    // Independent overflow coverage for Add and Sub (Mul is covered separately) — a per-op `*_overflow`
    // wiring slip would else ship silently.
    let program = compile_source(
        "package Main;\n\
         function add(int a, int b) -> int { return a + b; }\n\
         function sub(int a, int b) -> int { return a - b; }\n\
         function main() -> void {}",
    );
    for (name, args) in [
        ("add", [Value::Int(i64::MAX), Value::Int(1)]),
        ("sub", [Value::Int(i64::MIN), Value::Int(1)]),
    ] {
        let f = func_index(&program, name);
        match Compiled::compile_unboxed(&program, f)
            .expect("eligible")
            .run_unboxed(&args, 1)
        {
            JitRun::Fault(m) => assert_eq!(m, crate::value::FAULT_INT_OVERFLOW, "{name} overflow"),
            JitRun::Value(v) => panic!("{name}: expected overflow, got {}", as_int(&v)),
        }
    }
}

// --- slice u2a: UNBOXED self-recursion (fib JITs unboxed) ---

#[test]
fn unboxed_recursive_fib_matches_vm_oracle() {
    // The headline: recursive fib through the UNBOXED path (native i64, native self-call, no boxed Vec).
    // `n` is proven int via `n - 1` (SubI) so the base-case `return n` types as Int. Checked vs the VM
    // oracle across the base-case edge and the recursion.
    let program = compile_source(
        "package Main;\n\
         function fib(int n) -> int {\n\
           if (n < 2) { return n; }\n\
           return fib(n - 1) + fib(n - 2);\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "fib");
    for n in [0_i64, 1, 2, 3, 5, 10, 15, 20] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "unboxed fib({n}) must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_deep_recursion_faults_like_the_vm_stack_overflow() {
    // Unboxed native recursion must cap at MAX_CALL_DEPTH with the VM's "stack overflow" (code 4), not
    // segfault. Big stack (native frames); assert INSIDE the closure (Value/JitRun hold Rc = not Send).
    const SRC: &str = "package Main;\n\
        function forever(int n) -> int { return forever(n + 1); }\n\
        function main() -> void {}";
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let program = compile_source(SRC);
            let f = func_index(&program, "forever");
            let vm_fault = crate::vm::Vm::new(&program)
                .run_entry(f, vec![Value::Int(0)])
                .expect_err("VM must fault with stack overflow")
                .render("");
            let jit = match Compiled::compile_unboxed(&program, f)
                .expect("forever is unboxed-eligible")
                .run_unboxed(&[Value::Int(0)], 1)
            {
                JitRun::Fault(m) => m,
                JitRun::Value(v) => panic!("expected stack overflow, got {}", as_int(&v)),
            };
            (vm_fault, jit)
        })
        .expect("spawn big-stack thread");
    let (vm_fault, jit) = handle.join().expect("big-stack thread panicked");
    assert!(
        vm_fault.contains(&jit),
        "unboxed stack-overflow fault `{jit}` must match the VM oracle:\n{vm_fault}"
    );
}

#[test]
#[ignore = "timing measurement (best-of-N over VM fib(30) ≈ seconds); run manually with --ignored"]
fn measures_unboxed_fib_vs_vm_and_php() {
    // The G-8 deliverable: does UNBOXED recursive fib beat php+JIT? Best-of-N wall, compile reported
    // separately, PRINT-ONLY on timing (correctness asserted vs the VM oracle first). php+JIT baseline:
    // recorded ~10 ms fib(30) under Docker php:8.5 release+JIT (measured 2026-07-08). Bar = beats php,
    // NOT the 5 ms spike (u2a adds a depth check + multi-return + code-check per call).
    use std::time::Instant;
    let program = compile_source(
        "package Main;\n\
         function fib(int n) -> int {\n\
           if (n < 2) { return n; }\n\
           return fib(n - 1) + fib(n - 2);\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "fib");
    const N: i64 = 30;

    let t = Instant::now();
    let compiled = Compiled::compile_unboxed(&program, f).expect("fib is unboxed-eligible");
    let compile_ns = t.elapsed().as_nanos();

    let jit_val = match compiled.run_unboxed(&[Value::Int(N)], 1) {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected fib fault: {m}"),
    };
    assert_eq!(
        jit_val,
        vm_int(&program, f, vec![Value::Int(N)]),
        "unboxed fib({N}) must equal the VM oracle before timing is meaningful"
    );

    let best_unboxed = (0..10)
        .map(|_| {
            let s = Instant::now();
            let _ = compiled.run_unboxed(&[Value::Int(N)], 1);
            s.elapsed().as_nanos()
        })
        .min()
        .unwrap();
    let best_vm = (0..5)
        .map(|_| {
            let s = Instant::now();
            let _ = crate::vm::Vm::new(&program).run_entry(f, vec![Value::Int(N)]);
            s.elapsed().as_nanos()
        })
        .min()
        .unwrap();

    eprintln!(
        "[jit-unboxed] fib({N}) best-of-N wall time:\n  \
         compile       = {:.3} ms (one-time)\n  \
         UNBOXED JIT   = {:.4} ms (best of 10)\n  \
         VM            = {:.3} ms (best of 5)\n  \
         php+JIT       = ~10 ms (recorded, Docker php:8.5-cli release+JIT)\n  \
         speedup unboxed-JIT vs VM = {:.1}x  | vs php+JIT (~10ms) = {:.1}x",
        compile_ns as f64 / 1e6,
        best_unboxed as f64 / 1e6,
        best_vm as f64 / 1e6,
        best_vm as f64 / best_unboxed as f64,
        10.0 / (best_unboxed as f64 / 1e6),
    );
}

// --- slice u2b: general multi-function UNBOXED calls (non-self) ---

#[test]
fn unboxed_cross_function_calls_match_vm_oracle() {
    // A transitive call graph a → b → c (all int, non-self) compiled into one module; cross-`FuncId`
    // native calls with the (depth+1, args) → (value, code) ABI. Checked vs the VM oracle.
    let program = compile_source(
        "package Main;\n\
         function c(int n) -> int { return n + n; }\n\
         function b(int n) -> int { return c(n) * 2; }\n\
         function a(int n) -> int { return b(n) + 1; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "a");
    for n in [0_i64, 1, 3, 7, -4] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "unboxed a({n}) [a→b→c] must match the VM oracle"
        );
    }
}

#[test]
fn unboxed_cross_call_propagates_a_callee_fault() {
    // A fault raised in a CROSS-called callee (`bad` divides by zero) propagates up through the
    // caller's post-call `code != 0` edge unchanged (code 2 = div-zero, not the depth code 4) — the
    // shared fault_exit carries the callee's exact code. Checked vs the VM oracle.
    let program = compile_source(
        "package Main;\n\
         function bad(int a, int b) -> int { return a / b; }\n\
         function callbad(int n) -> int { return bad(n, 0); }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "callbad");
    let vm_fault = crate::vm::Vm::new(&program)
        .run_entry(f, vec![Value::Int(10)])
        .expect_err("VM must fault: the cross-callee divides by zero");
    match Compiled::compile_unboxed(&program, f)
        .expect("callbad is unboxed-eligible")
        .run_unboxed(&[Value::Int(10)], 1)
    {
        JitRun::Fault(m) => {
            assert_eq!(
                m,
                crate::value::FAULT_DIV_ZERO,
                "propagated div-zero string"
            );
            assert!(
                vm_fault.render("").contains(&m),
                "unboxed propagated fault must match the VM oracle"
            );
        }
        JitRun::Value(v) => panic!("expected a propagated div-zero fault, got {}", as_int(&v)),
    }
}

#[test]
fn unboxed_ineligible_callee_sinks_the_whole_graph() {
    // The fixpoint's core guarantee: an entry eligible ON ITS OWN, but which REACHES an ineligible
    // callee, must fail the whole compile (atomic whole-graph rejection) — never miscompile. `leaf`
    // returns a bare param `m` never used in an int-arith op → unprovable Int → ineligible; `top` is
    // fine alone but calls `leaf`.
    let program = compile_source(
        "package Main;\n\
         function leaf(int n, int m) -> int { return m; }\n\
         function top(int n) -> int { return leaf(n, n); }\n\
         function main() -> void {}",
    );
    for name in ["leaf", "top"] {
        let f = func_index(&program, name);
        assert!(
            matches!(
                Compiled::compile_unboxed(&program, f),
                Err(JitError::Unsupported(_))
            ),
            "`{name}`: an ineligible reached callee must sink the whole graph (Unsupported), not miscompile"
        );
    }
}

// --- slice b3b: `phg run` wiring — the JIT hook on `Op::Call`, VM-fallback, depth parity ---

#[test]
fn phg_run_hook_actually_hits_the_jit() {
    // A silent 100%-fallback to the VM would pass every byte-identity check identically and prove
    // nothing — so this asserts the hit counter is non-zero, i.e. the native path genuinely ran.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function fib(int n) -> int { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }\n\
        function main() -> void { Output.printLine(\"{fib(10)}\"); }";
    // Byte-identity: the jit-wired run must match the interpreter oracle (Invariant 2).
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "jit-wired output must match the interpreter oracle"
    );
    // Prove the JIT path was actually hit (build a Vm with an inspectable shared cache).
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual jit-wired output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the JIT path must actually be hit — a silent fallback false-greens byte-identity"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_an_int_loop() {
    // widen-1 DELIVERY-PATH proof (loops): an int `while` loop in a CALLED function must JIT through the
    // `Op::Call` hook. (A loop in `main` never JITs — `main` prints, so it is ineligible, and the
    // entry-level JIT cannot reach its body; the loop MUST live in a callee, exactly the
    // `bench/micro/intadd.phg` shape.) Byte-identity alone can't prove the flip — a silent VM fallback
    // false-greens it — so this asserts the hit counter fires, i.e. the widened subset genuinely runs
    // native at the CLI.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters) -> int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc;\n\
        }\n\
        function main() -> void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "int-loop jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual int-loop jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "an int loop in a called function must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_stack_overflow_threshold_matches_the_oracle() {
    // The ONE correctness vector the fault-fallback cannot catch: an under-fault (wrong
    // `start_depth`) makes the JIT RETURN A VALUE where the VM overflows — no fault, so no re-run.
    // A LINEAR eligible recursion bracketing `MAX_CALL_DEPTH`: under `--features jit`, cmd_run routes
    // `countdown` through the JIT; the interpreter (cmd_treewalk) is never JITted, so it is the pure
    // depth oracle (Invariant 2). Running through the real cmd_run path (its `on_deep_stack` 256MB
    // thread) also proves 4096 native JIT frames don't blow the production stack.
    use crate::limits::MAX_CALL_DEPTH;
    for n in (MAX_CALL_DEPTH - 3)..=(MAX_CALL_DEPTH + 2) {
        let src = format!(
            "package Main;\n\
             import Core.Output;\n\
             function countdown(int n) -> int {{ if (n <= 0) {{ return 0; }} return countdown(n - 1); }}\n\
             function main() -> void {{ Output.printLine(\"{{countdown({n})}}\"); }}"
        );
        let jit = crate::cli::cmd_run(&src);
        let oracle = crate::cli::cmd_treewalk(&src);
        match (&jit, &oracle) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "countdown({n}): jit output must match the oracle"),
            (Err(a), Err(b)) => assert_eq!(a, b, "countdown({n}): jit fault must match the oracle"),
            _ => panic!(
                "countdown({n}): jit/oracle disagree on success-vs-fault: jit={jit:?}, oracle={oracle:?}"
            ),
        }
    }
}

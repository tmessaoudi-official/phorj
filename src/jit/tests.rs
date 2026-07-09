//! Slice-1 JIT tests (run under `--features jit`). They prove the codegen substrate end-to-end: a
//! pure-int function (leaf or recursive) compiles to native code and produces the SAME value the VM
//! oracle does, a kernel fault surfaces with the SAME canonical string, calls compose across the
//! shared value stack, deep recursion faults with the VM's `"stack overflow"` at the same depth, and
//! anything outside the subset is default-denied. Byte-identity-under-`phg run` is the *next* (wiring)
//! slice — these establish the substrate the wiring rides on.

use super::{compile_and_run, Compiled, JitError, JitRun, REDO_ON_VM};
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
fn unboxed_overflow_funnels_to_vm_redo() {
    // ovf-spec: the unboxed path speculates (wrapping + sticky), so an overflow no longer produces the
    // kernel string directly — it returns code 5 = REDO_ON_VM, and the hook re-runs on the VM (which
    // renders FAULT_INT_OVERFLOW). This asserts the low-level funnel; the end-to-end kernel-string
    // parity is covered by `ovf_spec_*` below. (Also proves the wrapping mul did NOT crash the process.)
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
        JitRun::Fault(m) => assert_eq!(m, REDO_ON_VM, "overflow must funnel to the VM redo"),
        JitRun::Value(v) => panic!("expected redo (code 5), got value {}", as_int(&v)),
    }
}

#[test]
fn unboxed_div_zero_and_mod_zero_both_funnel_to_redo() {
    // ovf-spec: div-zero and mod-zero KEEP their per-op branch (sdiv/srem hardware-trap), but both now
    // funnel to code 5 = REDO_ON_VM rather than emitting distinct codes 2/3. The b1 code→string
    // transposition risk therefore MOVES to the VM redo — its DISTINCTNESS is asserted end-to-end in
    // `ovf_spec_div_zero_and_mod_zero_render_distinct_faults` below. Here: both must funnel (no trap).
    let program = compile_source(
        "package Main;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function modi(int a, int b) -> int { return a % b; }\n\
         function main() -> void {}",
    );
    for name in ["divi", "modi"] {
        let f = func_index(&program, name);
        match Compiled::compile_unboxed(&program, f)
            .expect("eligible")
            .run_unboxed(&[Value::Int(1), Value::Int(0)], 1)
        {
            JitRun::Fault(m) => {
                assert_eq!(m, REDO_ON_VM, "{name}: zero-divisor must funnel to redo")
            }
            JitRun::Value(v) => panic!("{name}: expected redo, got {}", as_int(&v)),
        }
    }
}

#[test]
fn unboxed_min_over_neg_one_and_neg_min_funnel_to_redo_without_trapping() {
    // The signed-overflow edge — i64::MIN / -1, i64::MIN % -1, -i64::MIN. The CRITICAL property this
    // guards is unchanged by ovf-spec: the native code must NOT hardware-trap (SIGFPE/abort) on these,
    // it must RETURN. Under ovf-spec they all return code 5 = REDO_ON_VM (div/rem via their kept branch,
    // neg via the sticky flag since `ineg` wraps); the VM redo then renders FAULT_INT_OVERFLOW (asserted
    // end-to-end below). A process crash here would fail the test by aborting, not by a bad assertion.
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
        match Compiled::compile_unboxed(&program, f)
            .expect("eligible")
            .run_unboxed(&args, 1)
        {
            JitRun::Fault(m) => assert_eq!(m, REDO_ON_VM, "{name}: MIN-edge must funnel to redo"),
            JitRun::Value(v) => panic!("{name}: expected redo, got {}", as_int(&v)),
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

// --- ovf-spec END-TO-END byte-identity (the coverage the boxed guards above CANNOT provide: they run
// the boxed path, unchanged by ovf-spec). These drive the real `phg run` path — `cmd_run` (VM + JIT
// hook) vs `cmd_treewalk` (pure interpreter oracle, never JITted, Invariant 2) — and assert identical
// observable behaviour. Each asserts the JIT-target is unboxed-eligible so a silent VM fallback cannot
// false-green the test (the failure the widen-1 hit-counter discipline exists to prevent). ---

/// Assert `name` is unboxed-eligible in `src` — proves the `Op::Call` hook WILL engage it, so the test
/// genuinely exercises the ovf-spec codegen rather than trivially running on the VM.
fn assert_unboxed_eligible(src: &str, name: &str) {
    let program = compile_source(src);
    let f = func_index(&program, name);
    assert!(
        Compiled::compile_unboxed(&program, f).is_ok(),
        "`{name}` must be unboxed-eligible so the JIT path is actually taken (else the test false-greens)"
    );
}

#[test]
fn ovf_spec_overflow_in_loop_redoes_to_byte_identical_fault() {
    // Concern A (advisor): speculative wrapping in a loop whose exit test reads the wrapped value must
    // NOT diverge from the VM. `3^k mod 2^64` is always odd → never 0 → without the back-edge sticky
    // guard the native `spin` loops FOREVER while the VM faults overflow in ~40 iters. WITH the guard,
    // the first overflow trips the back-edge check → redo on VM → byte-identical overflow fault. If this
    // test ever hangs (CI timeout) rather than fails, the back-edge guard has regressed.
    // The loop var must be initialized from a CONSTANT (proven int) — a bare param is not proven-int
    // unless used as an arith operand, so `spin(int seed){ i = seed; ...}` is ineligible. Arity-0 form
    // (the advisor's exact counterexample) keeps `i` proven-int and unboxed-eligible.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function spin() -> int { mutable int i = 1; while (i != 0) { i = i * 3; } return i; }\n\
        function main() -> void { Output.printLine(\"{spin()}\"); }";
    assert_unboxed_eligible(SRC, "spin");
    let jit = crate::cli::cmd_run(SRC);
    let oracle = crate::cli::cmd_treewalk(SRC);
    match (&jit, &oracle) {
        (Err(a), Err(b)) => {
            assert_eq!(a, b, "spin: jit fault must match the interpreter oracle");
            assert!(
                a.contains(crate::value::FAULT_INT_OVERFLOW),
                "spin must fault overflow (via the VM redo), got:\n{a}"
            );
        }
        _ => panic!("spin: both must fault overflow; jit={jit:?}, oracle={oracle:?}"),
    }
}

#[test]
fn ovf_spec_overflow_before_div_zero_orders_correctly() {
    // Fault ORDERING through the JIT: `a * a / b` with a*a overflowing AND b == 0. The VM faults at the
    // OVERFLOW (a*a precedes /b) and never reaches the divide. The JIT sets sticky at a*a, then its
    // div-zero branch trips — but it emits code 5 (redo), NOT div-zero, so the VM redo reproduces the
    // true first fault: overflow. A design that emitted div-zero directly would fail here.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function f(int a, int b) -> int { return a * a / b; }\n\
        function main() -> void { Output.printLine(\"{f(4000000000, 0)}\"); }";
    assert_unboxed_eligible(SRC, "f");
    let jit = crate::cli::cmd_run(SRC);
    let oracle = crate::cli::cmd_treewalk(SRC);
    match (&jit, &oracle) {
        (Err(a), Err(b)) => {
            assert_eq!(a, b, "ordering: jit fault must match oracle");
            assert!(
                a.contains(crate::value::FAULT_INT_OVERFLOW),
                "must fault OVERFLOW (a*a before /b), not divide-by-zero, got:\n{a}"
            );
        }
        _ => panic!("ordering: both must fault overflow; jit={jit:?}, oracle={oracle:?}"),
    }
}

#[test]
fn ovf_spec_transient_cancelling_overflow_still_faults() {
    // `a * b - a * c` with b == c: under wrapping the products cancel to 0, but the VM faults at the
    // first (checked) `a * b`. The sticky flag forces a redo even though the wrapped result is a clean 0
    // — a naive wrapping rewrite would silently return 0.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function f(int a, int b, int c) -> int { return a * b - a * c; }\n\
        function main() -> void { Output.printLine(\"{f(4000000000, 4000000000, 4000000000)}\"); }";
    assert_unboxed_eligible(SRC, "f");
    let jit = crate::cli::cmd_run(SRC);
    let oracle = crate::cli::cmd_treewalk(SRC);
    match (&jit, &oracle) {
        (Err(a), Err(b)) => {
            assert_eq!(a, b, "transient: jit fault must match oracle");
            assert!(
                a.contains(crate::value::FAULT_INT_OVERFLOW),
                "must fault on the cancelling overflow, not return wrapped 0, got:\n{a}"
            );
        }
        _ => panic!("transient: both must fault overflow; jit={jit:?}, oracle={oracle:?}"),
    }
}

#[test]
fn ovf_spec_div_zero_and_mod_zero_render_distinct_faults() {
    // The b1 code→string transposition risk (2↔3) moved from `run_unboxed` (now both → code 5) to the
    // VM redo — so assert the DISTINCTNESS end-to-end: a pure div-by-zero (no prior overflow) renders
    // FAULT_DIV_ZERO and a pure mod-by-zero renders FAULT_MOD_ZERO, each byte-identical to the oracle.
    const DIV: &str = "package Main;\n\
        import Core.Output;\n\
        function dz(int a, int b) -> int { return a / b; }\n\
        function main() -> void { Output.printLine(\"{dz(1, 0)}\"); }";
    const MOD: &str = "package Main;\n\
        import Core.Output;\n\
        function mz(int a, int b) -> int { return a % b; }\n\
        function main() -> void { Output.printLine(\"{mz(1, 0)}\"); }";
    assert_unboxed_eligible(DIV, "dz");
    assert_unboxed_eligible(MOD, "mz");
    for (src, kernel, label) in [
        (DIV, crate::value::FAULT_DIV_ZERO, "div-zero"),
        (MOD, crate::value::FAULT_MOD_ZERO, "mod-zero"),
    ] {
        let jit = crate::cli::cmd_run(src);
        let oracle = crate::cli::cmd_treewalk(src);
        match (&jit, &oracle) {
            (Err(a), Err(b)) => {
                assert_eq!(a, b, "{label}: jit fault must match oracle");
                assert!(
                    a.contains(kernel),
                    "{label}: expected `{kernel}`, got:\n{a}"
                );
            }
            _ => panic!("{label}: both must fault; jit={jit:?}, oracle={oracle:?}"),
        }
    }
}

#[test]
fn ovf_spec_non_overflowing_loop_returns_the_checked_value() {
    // The happy path: a loop that never overflows must return the SAME value as the VM (wrapping ==
    // checked when no carry ever fires; sticky stays 0 → Return selects code 0 → the real value).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function sumsq(int n) -> int { mutable int s = 0; mutable int i = 1; while (i <= n) { s = s + i * i; i = i + 1; } return s; }\n\
        function main() -> void { Output.printLine(\"{sumsq(100)}\"); }";
    assert_unboxed_eligible(SRC, "sumsq");
    let jit = crate::cli::cmd_run(SRC).expect("no-overflow loop must succeed under jit");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit, oracle, "no-overflow loop value must match the oracle");
}

// --- float slice v1: pure float arith (Const(Float) + AddF/SubF/MulF/DivF), leaf-only, no float
// comparisons (deferred). Floats travel as f64 BITS through the i64 ABI; run_unboxed decodes via
// ret_kind. Byte-identity vs the VM oracle is bit-exact (same f64 ops, same order). ---

fn as_float(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        other => panic!("expected float, got {}", other.type_name()),
    }
}

/// Unboxed-JIT a float-returning function and unwrap its f64 (bit-compared to the VM oracle).
fn ub_float(program: &BytecodeProgram, f: usize, args: &[Value]) -> f64 {
    match Compiled::compile_unboxed(program, f)
        .expect("function must be unboxed-eligible")
        .run_unboxed(args, 1)
    {
        JitRun::Value(v) => as_float(&v),
        JitRun::Fault(m) => panic!("unexpected unboxed float fault: {m}"),
    }
}

fn vm_float(program: &BytecodeProgram, f: usize, args: Vec<Value>) -> f64 {
    let (v, _stdout) = crate::vm::Vm::new(program)
        .run_entry(f, args)
        .expect("VM run_entry");
    as_float(&v)
}

#[test]
fn unboxed_float_arith_leaf_matches_vm_oracle_bit_exact() {
    // A pure-float leaf: `a*b + a - b` (MulF/AddF/SubF) returning float. The JIT stores floats as bits
    // and bitcasts at each op; the result must be BIT-IDENTICAL to the VM oracle (same f64 ops, order).
    let program = compile_source(
        "package Main;\n\
         function calc(float a, float b) -> float { return a * b + a - b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "calc");
    for (a, b) in [(6.0_f64, 7.0_f64), (0.5, 0.25), (-3.5, 2.0), (1e300, 1e-8)] {
        let jit = ub_float(&program, f, &[Value::Float(a), Value::Float(b)]);
        let vm = vm_float(&program, f, vec![Value::Float(a), Value::Float(b)]);
        assert_eq!(
            jit.to_bits(),
            vm.to_bits(),
            "calc({a},{b}) unboxed float must bit-match the VM oracle (jit={jit}, vm={vm})"
        );
    }
}

#[test]
fn unboxed_float_div_by_zero_funnels_to_redo_but_nan_inf_do_not() {
    // Float div: a ZERO divisor faults (value::float_div) → code 5 = REDO_ON_VM. A NaN or inf divisor
    // does NOT fault (fcmp Equal is false for NaN; inf != 0) → a real f64 result, bit-matching the VM.
    let program = compile_source(
        "package Main;\n\
         function div(float a, float b) -> float { return a / b; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "div");
    // +0.0 and -0.0 both fault → redo.
    for z in [0.0_f64, -0.0_f64] {
        match Compiled::compile_unboxed(&program, f)
            .expect("div eligible")
            .run_unboxed(&[Value::Float(1.0), Value::Float(z)], 1)
        {
            JitRun::Fault(m) => assert_eq!(m, REDO_ON_VM, "float /{z} must funnel to redo"),
            JitRun::Value(v) => panic!("expected redo for /{z}, got {}", as_float(&v)),
        }
    }
    // A non-zero (incl. inf) divisor does NOT fault; matches the VM bit-for-bit (inf → 0.0, etc.).
    for (a, b) in [(6.0_f64, 2.0_f64), (1.0, f64::INFINITY), (1.0, 4.0)] {
        let jit = ub_float(&program, f, &[Value::Float(a), Value::Float(b)]);
        let vm = vm_float(&program, f, vec![Value::Float(a), Value::Float(b)]);
        assert_eq!(
            jit.to_bits(),
            vm.to_bits(),
            "div({a},{b}) must bit-match VM"
        );
    }
}

#[test]
fn unboxed_float_comparison_is_rejected_to_vm() {
    // Float comparisons (fcmp/NaN) are DEFERRED — a function comparing floats must fall back (Unsupported),
    // NOT icmp the raw bits. `less` returns int so the only disqualifier is the float compare itself.
    let program = compile_source(
        "package Main;\n\
         function less(float a, float b) -> int { if (a < b) { return 1; } return 0; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "less");
    assert!(
        matches!(
            Compiled::compile_unboxed(&program, f),
            Err(JitError::Unsupported(_))
        ),
        "a float comparison must be rejected to the VM, never icmp'd on bits"
    );
}

#[test]
fn unboxed_float_loop_mixes_int_and_float_at_shared_depths_bit_exact() {
    // MANDATORY guard for the dual-space (ivars/fvars) value model: a float accumulator loop driven by
    // an INT counter. The int comparison `i < n` and the float arith `acc = acc*x + 0.5` reuse the SAME
    // operand-stack DEPTHS for DIFFERENT kinds (Int/Bool vs Float) — the mixed-kind-per-depth case a
    // single-typed Variable cannot hold. If the value model ever reads the wrong space for a shared
    // depth (e.g. the counter as a float, or acc as an int-bit), the loop count or accumulation goes
    // wrong. Result must be BIT-IDENTICAL to the VM oracle across empty/one/many-iteration loops.
    let program = compile_source(
        "package Main;\n\
         function accum(int n, float x) -> float {\n\
           mutable float acc = 0.0;\n\
           mutable int i = 0;\n\
           while (i < n) { acc = acc * x + 0.5; i = i + 1; }\n\
           return acc;\n\
         }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "accum");
    for (n, x) in [(0_i64, 1.5_f64), (1, 2.0), (10, 1.0000001), (100, 0.999)] {
        let jit = ub_float(&program, f, &[Value::Int(n), Value::Float(x)]);
        let vm = vm_float(&program, f, vec![Value::Int(n), Value::Float(x)]);
        assert_eq!(
            jit.to_bits(),
            vm.to_bits(),
            "accum({n},{x}) unboxed must bit-match the VM oracle (jit={jit}, vm={vm})"
        );
    }
}

#[test]
fn unboxed_float_with_call_is_rejected_leaf_only() {
    // Float slice v1 is LEAF-only: a function that touches floats AND calls must fall back (the Call arm
    // models a callee return as Int, so a float flowing through a call would mis-decode).
    let program = compile_source(
        "package Main;\n\
         function twice(float x) -> float { return x + x; }\n\
         function useit(float x) -> float { return twice(x) + 1.0; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "useit");
    assert!(
        matches!(
            Compiled::compile_unboxed(&program, f),
            Err(JitError::Unsupported(_))
        ),
        "float ops + Call in one function must fall back (v1 leaf-only)"
    );
}

#[test]
fn floatmul_micro_jits_and_matches_the_oracle() {
    // The float slice's measurement target: the `floatmul` micro shape (IIR recurrence, pure MulF/AddF,
    // int loop) must JIT through the `Op::Call` hook AND match the interpreter oracle. Asserts the hot
    // fn is eligible + the hit counter fires (no false-green via a silent VM fallback).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Conversion;\n\
        function bench(int iters, float r) -> float {\n\
          mutable float acc = 0.0;\n\
          mutable int i = 0;\n\
          while (i < iters) { acc = acc * r + 0.5; i = i + 1; }\n\
          return acc;\n\
        }\n\
        function main() -> void { Output.printLine(\"{Conversion.truncate(bench(5000, 1.0000001))}\"); }";
    // `bench` must be unboxed-eligible so the JIT path is genuinely taken.
    let program = compile_source(SRC);
    let bench = func_index(&program, "bench");
    assert!(
        Compiled::compile_unboxed(&program, bench).is_ok(),
        "float `bench` must be unboxed-eligible (else the flip is unmeasurable / false-green)"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "floatmul jit output must match the oracle");
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual floatmul jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the float `bench` must actually hit the JIT — else the float flip is unproven"
    );
}

#[test]
fn unboxed_bool_local_not_returned_is_eligible() {
    // A mutable BOOL local (SetLocal from a comparison → Kind::Bool), used only in a bool context
    // (JumpIfFalse), NOT returned; the function returns an int. The slot-kind pre-pass must label the
    // slot Bool (so it can never be mis-returned as Value::Int) yet keep the function eligible.
    // Oracle-checked — the advisor's loop-carried-Bool shape, straight-line variant. (`a < 4`, param-vs-
    // const, so the comparison has a known-Int operand — see the float-slice guard in
    // `unboxed_all_comparisons_and_not_match_vm_oracle`.)
    let program = compile_source(
        "package Main;\n\
         function f(int a) -> int { mutable bool flag = a < 4; if (flag) { return 1; } return 0; }\n\
         function main() -> void {}",
    );
    let f = func_index(&program, "f");
    for a in [1_i64, 2, 5, -3, 4] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(a)]),
            vm_int(&program, f, vec![Value::Int(a)]),
            "unboxed f({a}) with a bool local must match the VM oracle"
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
    // ovf-spec: an in-loop overflow funnels to code 5 = REDO_ON_VM (the back-edge sticky guard trips on
    // the overflowing iteration). Byte-identity with the VM is proven end-to-end in
    // `ovf_spec_overflow_in_loop_redoes_to_byte_identical_fault`.
    match Compiled::compile_unboxed(&program, f)
        .expect("int while is eligible")
        .run_unboxed(&[Value::Int(100)], 1)
    {
        JitRun::Fault(msg) => assert_eq!(msg, REDO_ON_VM, "in-loop overflow must funnel to redo"),
        JitRun::Value(v) => panic!("expected redo, got {}", as_int(&v)),
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
    // ovf-spec: the in-loop divide-by-zero keeps its per-op branch (sdiv traps) but funnels to code 5 =
    // REDO_ON_VM; byte-identity is proven end-to-end in `ovf_spec_div_zero_and_mod_zero_render_distinct_faults`.
    match Compiled::compile_unboxed(&program, f)
        .expect("int while is eligible")
        .run_unboxed(&[Value::Int(3)], 1)
    {
        JitRun::Fault(msg) => {
            assert_eq!(msg, REDO_ON_VM, "in-loop div-by-zero must funnel to redo")
        }
        JitRun::Value(v) => panic!("expected redo, got {}", as_int(&v)),
    }
}

#[test]
fn unboxed_all_comparisons_and_not_match_vm_oracle() {
    // Every comparison arm (Gt/Ge/Le/Eq/Ne + Lt via `nt`'s `!(a<4)`) and `Not` — each a hand-written
    // `Op → IntCC` mapping, the b1 transposition-trap family. Branch-return leaf form, distinguishable
    // per edge (varying `a` around the constant `4`), oracle-checked vs the VM. A Le↔Ge / Eq↔Ne swap or
    // an operand-order flip changes a result and is caught here.
    //
    // Operand form is param-VS-CONST (`a op 4`), NOT param-vs-param (`a op b`): the float slice added a
    // soundness guard rejecting a comparison unless ≥1 operand is a KNOWN non-float (Int/Bool) — because
    // an `Unknown` operand (a param used only in a comparison) is kind-ambiguous (a float param compared
    // is bytecode-identical to an int one; `icmp` on float bits would be a silent byte-identity bug). The
    // const `4` is that known-Int operand. Restoring param-vs-param int comparisons (and float compares)
    // needs param types threaded into the bytecode — a tracked follow-up.
    let program = compile_source(
        "package Main;\n\
         function gt(int a) -> int { if (a > 4) { return 1; } return 0; }\n\
         function ge(int a) -> int { if (a >= 4) { return 1; } return 0; }\n\
         function le(int a) -> int { if (a <= 4) { return 1; } return 0; }\n\
         function eq(int a) -> int { if (a == 4) { return 1; } return 0; }\n\
         function ne(int a) -> int { if (a != 4) { return 1; } return 0; }\n\
         function nt(int a) -> int { if (!(a < 4)) { return 1; } return 0; }\n\
         function main() -> void {}",
    );
    for name in ["gt", "ge", "le", "eq", "ne", "nt"] {
        let f = func_index(&program, name);
        for a in [5_i64, 3, 4, -2, 7] {
            assert_eq!(
                ub_int(&program, f, &[Value::Int(a)]),
                vm_int(&program, f, vec![Value::Int(a)]),
                "unboxed {name}({a}) must match the VM oracle"
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
            JitRun::Fault(m) => assert_eq!(m, REDO_ON_VM, "{name} overflow must funnel to redo"),
            JitRun::Value(v) => panic!("{name}: expected redo, got {}", as_int(&v)),
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
fn unboxed_deep_recursion_caps_at_depth_and_funnels_to_redo() {
    // Unboxed native recursion must cap at MAX_CALL_DEPTH (the depth-guard branch fires) and RETURN,
    // NOT segfault the process. Under ovf-spec that cap funnels to code 5 = REDO_ON_VM; the byte-identical
    // "stack overflow" string is covered end-to-end by `jit_stack_overflow_threshold_matches_the_oracle`
    // (countdown bracketing MAX_CALL_DEPTH through the real hook). Big stack (native frames); assert
    // INSIDE the closure (Value/JitRun hold Rc = not Send). If this segfaulted, the thread would abort.
    const SRC: &str = "package Main;\n\
        function forever(int n) -> int { return forever(n + 1); }\n\
        function main() -> void {}";
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let program = compile_source(SRC);
            let f = func_index(&program, "forever");
            match Compiled::compile_unboxed(&program, f)
                .expect("forever is unboxed-eligible")
                .run_unboxed(&[Value::Int(0)], 1)
            {
                JitRun::Fault(m) => m,
                JitRun::Value(v) => panic!("expected redo (depth cap), got {}", as_int(&v)),
            }
        })
        .expect("spawn big-stack thread");
    let jit = handle.join().expect("big-stack thread panicked");
    assert_eq!(
        jit, REDO_ON_VM,
        "the depth cap must fire and funnel to the VM redo (not segfault, not return a value)"
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
    // ovf-spec: the callee `bad` faults (div-zero → code 5); `callbad` propagates the callee's non-zero
    // code as code 5 = REDO_ON_VM. This asserts the CROSS-CALL fault propagation still fires (a callee
    // fault is not swallowed); the byte-identical div-zero string is the VM redo's job (covered
    // end-to-end in `ovf_spec_div_zero_and_mod_zero_render_distinct_faults`).
    match Compiled::compile_unboxed(&program, f)
        .expect("callbad is unboxed-eligible")
        .run_unboxed(&[Value::Int(10)], 1)
    {
        JitRun::Fault(m) => assert_eq!(m, REDO_ON_VM, "a callee fault must propagate as a redo"),
        JitRun::Value(v) => panic!("expected a propagated redo, got {}", as_int(&v)),
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

// --- range-analysis (docs/plans/perf-wave.plan.md): the induction-counter overflow-guard drop. These
// UNIT-TEST the `range_proven_ops` recognizer directly (the soundness surface) — a counter can't be run
// to 2^63 to observe an overflow fault, so correctness is proven structurally (which ops are proven) +
// by byte-identity vs the VM oracle on the emitted code. The ONE unsound spot is the guard↔increment
// link, so the rejection cases (wrong slot, `<=`, `!=`, double-write, nested) are the load-bearing ones:
// each must NOT prove, so it keeps its overflow guard. ---

/// How many `AddI` ops the range analysis proves as no-overflow induction increments in `name`.
fn proven_count(program: &BytecodeProgram, name: &str) -> usize {
    let f = func_index(program, name);
    super::range_proven_ops(&program.functions[f])
        .iter()
        .filter(|&&p| p)
        .count()
}

#[test]
fn range_analysis_proves_strict_lt_plus_one_counter() {
    // The canonical counted loop `while (i < n) { i = i + 1; }`: strict `<`, `+1`, single writer, guard
    // on the induction slot at the loop header → PROVEN (exactly one). Byte-identical to the VM oracle.
    let program = compile_source(
        "package Main;\n\
         function count(int n) -> int { mutable int i = 0; while (i < n) { i = i + 1; } return i; }\n\
         function main() -> void {}",
    );
    assert_eq!(
        proven_count(&program, "count"),
        1,
        "the strict-`<` `+1` counter must be range-proven (overflow guard droppable)"
    );
    let f = func_index(&program, "count");
    assert!(
        Compiled::compile_unboxed(&program, f).is_ok(),
        "must stay unboxed-eligible"
    );
    for n in [0_i64, 1, 5, 100, -3] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "range-proven counter count({n}) must still match the VM oracle"
        );
    }
}

#[test]
fn range_analysis_rejects_le_ne_and_wrong_slot_guards() {
    // Each is a real bound on the counter that the recognizer INTENTIONALLY does not prove (fail closed),
    // so each keeps its overflow guard: `<=` (`+1` at `i64::MAX` would overflow), `!=` (not `<`), and a
    // guard on a DIFFERENT slot than the increment (`n < 100` guards `n`, not `i`). None may be proven.
    let program = compile_source(
        "package Main;\n\
         function le(int n)    -> int { mutable int i = 0; while (i <= n)   { i = i + 1; } return i; }\n\
         function ne(int n)    -> int { mutable int i = 0; while (i != n)   { i = i + 1; } return i; }\n\
         function wrong(int n) -> int { mutable int i = 0; while (n < 100)  { i = i + 1; } return i; }\n\
         function main() -> void {}",
    );
    for name in ["le", "ne", "wrong"] {
        assert_eq!(
            proven_count(&program, name),
            0,
            "`{name}` must NOT be range-proven — it keeps its overflow guard (sound)"
        );
    }
    // `le`/`ne` terminate and must stay byte-identical (the guard they kept is harmless here).
    for (name, n) in [("le", 5_i64), ("ne", 5)] {
        let f = func_index(&program, name);
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "unproven-counter {name}({n}) must match the VM oracle"
        );
    }
}

#[test]
fn range_analysis_rejects_double_write_and_nested_loop() {
    // Double-write: two `SetLocal(i)` → single-writer fails → not proven. Nested: the outer counter's
    // guarded body contains an inner back-edge → condition (4) fails → outer not proven; the inner `!=`
    // counter is not proven either → zero proven total.
    let program = compile_source(
        "package Main;\n\
         function dbl(int n) -> int { mutable int i = 0; while (i < n) { i = i + 1; i = i + 1; } return i; }\n\
         function nest(int n) -> int {\n\
           mutable int i = 0;\n\
           while (i < n) {\n\
             mutable int j = 0;\n\
             while (j != n) { j = j + 1; }\n\
             i = i + 1;\n\
           }\n\
           return i;\n\
         }\n\
         function main() -> void {}",
    );
    // The soundness-critical assertion is that NEITHER counter is proven (both keep their overflow
    // guards). `dbl`/`nest` are not necessarily unboxed-eligible (the block-local `j` / statement shape
    // introduces a `Pop`), so they run on the VM — byte-identity of unproven counters is covered by the
    // `le`/`ne` cases and the existing loop suite; here we only pin the recognizer's rejection.
    assert_eq!(
        proven_count(&program, "dbl"),
        0,
        "double-write counter must not be proven"
    );
    assert_eq!(
        proven_count(&program, "nest"),
        0,
        "a counter with a nested loop in its body must not be proven"
    );
}

#[test]
fn range_analysis_float_counted_loop_matches_vm_and_drops_guard() {
    // The floatmul WIN shape: a float accumulator + a strict-`<` `+1` int counter. The counter is the
    // ONLY int-arith op → it is proven AND `needs_sticky` becomes false → all sticky machinery is gone.
    // Correctness = bit-exact float result vs the VM oracle (the WIN itself is measured separately).
    let program = compile_source(
        "package Main;\n\
         function bench(int iters, float r) -> float {\n\
           mutable float acc = 0.0;\n\
           mutable int i = 0;\n\
           while (i < iters) { acc = acc * r + 0.5; i = i + 1; }\n\
           return acc;\n\
         }\n\
         function main() -> void {}",
    );
    assert_eq!(
        proven_count(&program, "bench"),
        1,
        "the float loop's int counter must be range-proven"
    );
    let f = func_index(&program, "bench");
    assert!(
        Compiled::compile_unboxed(&program, f).is_ok(),
        "float counted loop must be unboxed-eligible"
    );
    for iters in [0_i64, 1, 10, 1000] {
        let jit = ub_float(&program, f, &[Value::Int(iters), Value::Float(1.0000001)]);
        let vm = vm_float(
            &program,
            f,
            vec![Value::Int(iters), Value::Float(1.0000001)],
        );
        assert_eq!(
            jit.to_bits(),
            vm.to_bits(),
            "bench({iters}) must be bit-exact vs the VM oracle"
        );
    }
}

#[test]
fn range_analysis_proven_counter_coexists_with_unproven_op_that_still_faults() {
    // intadd-PARTIAL + the fault-preservation guard: a strict-`<` `+1` counter (PROVEN → plain `iadd`)
    // sharing a loop with `s = s * 3` (a `MulI`, never proven → keeps its overflow guard). Exactly one
    // op proven (the counter). Byte-identical for small n; and for n past the overflow point the UNPROVEN
    // multiply must STILL funnel to the VM redo — proving dropping the counter's guard did not drop the
    // accumulator's (3^40 > i64::MAX, so the VM faults overflow around i=39).
    let program = compile_source(
        "package Main;\n\
         function f(int n) -> int { mutable int s = 1; mutable int i = 0; while (i < n) { s = s * 3; i = i + 1; } return s; }\n\
         function main() -> void {}",
    );
    assert_eq!(
        proven_count(&program, "f"),
        1,
        "only the counter is proven; the `*3` accumulator is not"
    );
    let f = func_index(&program, "f");
    for n in [0_i64, 1, 5, 20] {
        assert_eq!(
            ub_int(&program, f, &[Value::Int(n)]),
            vm_int(&program, f, vec![Value::Int(n)]),
            "coexist f({n}) (no overflow) must match the VM oracle"
        );
    }
    // n = 50 overflows the `*3` accumulator: the unproven op's guard must still fire → VM redo.
    match Compiled::compile_unboxed(&program, f)
        .expect("eligible")
        .run_unboxed(&[Value::Int(50)], 1)
    {
        JitRun::Fault(m) => assert_eq!(
            m, REDO_ON_VM,
            "the unproven `*3` overflow must still funnel to redo"
        ),
        JitRun::Value(v) => panic!("expected redo (accumulator overflow), got {}", as_int(&v)),
    }
}

// --- `#[Unchecked]` (import Core.Unchecked): whole-function two's-complement wrapping int arithmetic.
// The fn-level `unchecked` flag makes the JIT emit plain `iadd`/`isub`/`imul`/`ineg` (no overflow guard,
// no sticky) — the WIN path (intadd LOSS→WIN) — and the result must be byte-identical to the VM, which
// reads the same flag and calls the same `value::int_wrapping_*` kernels. ---

#[test]
fn unchecked_function_wraps_add_sub_mul_without_faulting_and_matches_vm() {
    let program = compile_source(
        "package Main;\n\
         import Core.Unchecked;\n\
         #[Unchecked]\n\
         function wadd(int a, int b) -> int { return a + b; }\n\
         #[Unchecked]\n\
         function wsub(int a, int b) -> int { return a - b; }\n\
         #[Unchecked]\n\
         function wmul(int a, int b) -> int { return a * b; }\n\
         function main() -> void {}",
    );
    // The overflow edges that WOULD fault in a checked function must WRAP here (no redo, no fault).
    let cases: &[(&str, i64, i64, i64)] = &[
        ("wadd", i64::MAX, 1, i64::MIN), // MAX + 1 wraps
        ("wsub", i64::MIN, 1, i64::MAX), // MIN - 1 wraps
        ("wmul", i64::MAX, 2, i64::MAX.wrapping_mul(2)),
    ];
    for &(name, a, b, want) in cases {
        let f = func_index(&program, name);
        match Compiled::compile_unboxed(&program, f)
            .expect("an #[Unchecked] int fn is unboxed-eligible")
            .run_unboxed(&[Value::Int(a), Value::Int(b)], 1)
        {
            JitRun::Value(v) => assert_eq!(
                as_int(&v),
                want,
                "unchecked {name}({a},{b}) must WRAP to {want}, not fault"
            ),
            JitRun::Fault(m) => panic!("unchecked {name} must NOT fault (wraps), got {m}"),
        }
        // Byte-identity vs the VM oracle across the edges + an ordinary value.
        for &(a, b) in &[(a, b), (2, 3), (-7, 5)] {
            assert_eq!(
                ub_int(&program, f, &[Value::Int(a), Value::Int(b)]),
                vm_int(&program, f, vec![Value::Int(a), Value::Int(b)]),
                "unchecked {name}({a},{b}) JIT must match the VM oracle"
            );
        }
    }
}

//! Boxed-path substrate tests: arithmetic/control-flow vs the VM oracle, kernel-fault
//! parity, call composition, depth caps, default-deny.

use super::*;

#[test]
fn jits_int_arithmetic_and_matches_vm_oracle() {
    let program = compile_source(
        "package Main;\n\
         function calc(int a, int b) -> int { return a * b + a - b; }\n\
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
        #[Entry] function main() -> void {}";
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
pub(super) fn ub_int(program: &BytecodeProgram, f: usize, args: &[Value]) -> i64 {
    match Compiled::compile_unboxed(program, f)
        .expect("function must be unboxed-eligible")
        .run_unboxed(args, 1)
    {
        JitRun::Value(v) => as_int(&v),
        JitRun::Fault(m) => panic!("unexpected unboxed fault: {m}"),
    }
}

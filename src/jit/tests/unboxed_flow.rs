//! Unboxed-path tests — floats, locals, recursion, cross-function calls.

use super::boxed::ub_int;
use super::unboxed_int::as_float;
use super::*;

/// Unboxed-JIT a float-returning function and unwrap its f64 (bit-compared to the VM oracle).
pub(super) fn ub_float(program: &BytecodeProgram, f: usize, args: &[Value]) -> f64 {
    match Compiled::compile_unboxed(program, f)
        .expect("function must be unboxed-eligible")
        .run_unboxed(args, 1)
    {
        JitRun::Value(v) => as_float(&v),
        JitRun::Fault(m) => panic!("unexpected unboxed float fault: {m}"),
    }
}

pub(super) fn vm_float(program: &BytecodeProgram, f: usize, args: Vec<Value>) -> f64 {
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
        #[Entry] function main() -> void { Output.printLine(\"{Conversion.truncate(bench(5000, 1.0000001))}\"); }";
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
        #[Entry] function main() -> void {}";
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
         #[Entry] function main() -> void {}",
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
fn inline_conversions_match_the_oracle_and_hit_the_jit() {
    // P-2c: `Conversion.toFloat` (fcvt_from_sint) + `Conversion.truncate` (guarded fcvt_to_sint)
    // run fully inline — the exact floatarith shape must JIT and stay byte-identical.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Conversion;\n\
        function bench(int iters): int {\n\
          mutable float acc = 0.0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Conversion.toFloat(i) * 0.5;\n\
            i = i + 1;\n\
          }\n\
          return Conversion.truncate(acc);\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "conversion loop must match the oracle");
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual run ok");
    assert_eq!(manual, oracle);
    assert!(cache.borrow().hits > 0, "the conversion loop must JIT");
}

#[test]
fn inline_truncate_range_fault_matches_the_vm() {
    // Out-of-range truncate (2^63 doubles) faults canonically on both paths; negatives and
    // fractional values truncate toward zero exactly like the kernel.
    const OK: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Conversion;\n\
        function bench(): int {\n\
          mutable float f = 0.0 - 12345.678;\n\
          return Conversion.truncate(f) + Conversion.truncate(9.9);\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_out = crate::cli::cmd_run(OK).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(OK).expect("oracle ok");
    assert_eq!(jit_out, oracle, "negative/fractional truncate must match");

    const OOR: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Conversion;\n\
        function bench(): int {\n\
          mutable float f = 1.0e300;\n\
          return Conversion.truncate(f);\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_err = crate::cli::cmd_run(OOR).expect_err("must fault");
    let oracle_err = crate::cli::cmd_treewalk(OOR).expect_err("must fault");
    assert!(
        jit_err.contains("out of int range"),
        "jit fault must be canonical, got: {jit_err}"
    );
    assert!(oracle_err.contains("out of int range"));
}

//! Unboxed-path tests: kind analysis, checked arith + sticky faults, floats, locals,
//! recursion, cross-function calls.

use super::boxed::ub_int;
use super::*;

#[test]
fn proven_rem_by_pow2_masks_and_matches_the_oracle() {
    // P-2c: `i % 4` with a proven non-negative induction dividend lowers to a single `band` —
    // values must stay byte-identical to the interpreter oracle across the whole range walk.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + (i % 4) * 10 + (i % 8);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "masked rem must match the oracle");
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual run ok");
    assert_eq!(manual, oracle);
    assert!(cache.borrow().hits > 0, "the rem loop must actually JIT");
}

#[test]
fn negative_dividend_rem_is_unproven_and_matches_the_oracle() {
    // A negative-init counter must NOT be masked (`-7 % 4 == -3`, mask would give 1): the proof
    // requires init ≥ 0, so this stays checked `srem` — byte-identical to the oracle either way.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0 - 7;\n\
          while (i < iters) {\n\
            acc = acc + (i % 4);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(9)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "negative-dividend rem must match the oracle"
    );
}

#[test]
fn non_pow2_rem_stays_checked_and_matches_the_oracle() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + (i % 3);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(100)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "%3 must match the oracle");
}

#[test]
fn unboxed_arithmetic_matches_vm_oracle() {
    // Pure int arithmetic through native registers (no boxed Vec, no helper calls). Checked against
    // the VM oracle across sign combinations.
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         function calc(int a, int b) -> int { return a * b + a - b; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function pick(int a) -> int { if (a < 10) { return 111; } else { return 222; } }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function choose(bool b, int n) -> int { if (b) { return n + 1; } return n + 2; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function mul(int a, int b) -> int { return a * b; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function modi(int a, int b) -> int { return a % b; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function divi(int a, int b) -> int { return a / b; }\n\
         function modi(int a, int b) -> int { return a % b; }\n\
         function neg(int a) -> int { return -a; }\n\
         #[Entry] function main() -> void {}",
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
    // The type-erasure guard: a bare UNPROVEN-int param return (`identity` — n is never an
    // int-arith operand, so unprovable) and a returned bool PARAM (`retb` — proves the
    // provenance pass does NOT over-mark) fall back — compile_unboxed must return Unsupported,
    // never miscompile. A PROVEN bool return (`isSmall` — the hofpipe predicate shape) is now
    // in the subset: `ret_kind` records Bool and `run_unboxed` decodes `Value::Bool`, so it
    // must COMPILE and stay byte-identical to the oracle.
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         function isSmall(int n) -> bool { return n < 10; }\n\
         function identity(int n) -> int { return n; }\n\
         function retb(bool b, int n) -> bool { if (n > 0) { return b; } return b; }\n\
         #[Entry] function main() -> void {}",
    );
    for name in ["identity", "retb"] {
        let f = func_index(&program, name);
        assert!(
            matches!(
                Compiled::compile_unboxed(&program, f),
                Err(JitError::Unsupported(_))
            ),
            "unboxed must reject `{name}` (unproven return), not miscompile"
        );
    }
    let f = func_index(&program, "isSmall");
    let compiled = Compiled::compile_unboxed(&program, f)
        .expect("a proven bool return is in the subset (hofpipe predicate shape)");
    for n in [-3_i64, 9, 10, 42] {
        match compiled.run_unboxed(&[Value::Int(n)], 1) {
            crate::jit::JitRun::Value(Value::Bool(b)) => {
                assert_eq!(b, n < 10, "isSmall({n}) must decode as the oracle's bool")
            }
            other => panic!("isSmall({n}) must return a decoded Bool, got {other:?}"),
        }
    }
}

// --- widen-1 c2: UNBOXED straight-line mutable locals (SetLocal + local decls; loops still rejected) ---

#[test]
fn unboxed_straightline_mutable_local_matches_vm() {
    // A mutable local (SetLocal + GetLocal of slot >= arity), straight-line (no loop), int-returning.
    // Locals are Cranelift Variables; the slot-kind pre-pass proves `a` int (every assignment is int),
    // so `return a` is accepted. Oracle-checked.
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         function f(int x) -> int { mutable int a = x * 2; a = a + 3; return a; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function f(int a, int b) -> int { return a * a / b; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function f(int a, int b, int c) -> int { return a * b - a * c; }\n\
         #[Entry] function main() -> void {}",
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function spin() -> int { mutable int i = 1; while (i != 0) { i = i * 3; } return i; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{spin()}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function f(int a, int b) -> int { return a * a / b; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{f(4000000000, 0)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function f(int a, int b, int c) -> int { return a * b - a * c; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{f(4000000000, 4000000000, 4000000000)}\"); }";
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
    const DIV: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function dz(int a, int b) -> int { return a / b; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{dz(1, 0)}\"); }";
    const MOD: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function mz(int a, int b) -> int { return a % b; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{mz(1, 0)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function sumsq(int n) -> int { mutable int s = 0; mutable int i = 1; while (i <= n) { s = s + i * i; i = i + 1; } return s; }\n\
        #[Entry] function main() -> void { Output.printLine(\"{sumsq(100)}\"); }";
    assert_unboxed_eligible(SRC, "sumsq");
    let jit = crate::cli::cmd_run(SRC).expect("no-overflow loop must succeed under jit");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit, oracle, "no-overflow loop value must match the oracle");
}

// --- float slice v1: pure float arith (Const(Float) + AddF/SubF/MulF/DivF), leaf-only, no float
// comparisons (deferred). Floats travel as f64 BITS through the i64 ABI; run_unboxed decodes via
// ret_kind. Byte-identity vs the VM oracle is bit-exact (same f64 ops, same order). ---

pub(super) fn as_float(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        other => panic!("expected float, got {}", other.type_name()),
    }
}

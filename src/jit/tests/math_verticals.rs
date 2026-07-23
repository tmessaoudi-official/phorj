//! JIT Math scalar-native verticals (`Math.max`/`min`/`sign`/`abs`) — delivery-path proofs
//! (`hits>0`) + byte-identity edges (negative operands, `i64::MIN` abs → code-5 redo). Split from
//! the `verticals.rs` monolith by cohesion (Invariant 13, M-Decomp).

use super::*;

#[test]
fn phg_run_hook_hits_the_jit_on_the_mathmax_vertical() {
    // Mathmax-vertical DELIVERY-PATH proof: the exact `bench/micro/mathmax.phg` loop shape — a hot
    // `while` folding `Math.max(int, int)` with DATA-DEPENDENT operands (`i % 1000`, `i * 3 % 1000`)
    // so nothing constant-folds and the native call cannot be hoisted. The inline Cranelift `smax`
    // is byte-identical to the interpreter's `i64::max` kernel; a silent VM fallback would false-green
    // the byte-identity assert, so `hits>0` is the load-bearing check (proves the perf flip fired).
    // Deterministic output only (checksum via printLine — no monotonicNanos timing field).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.max(i % 1000, i * 3 % 1000);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathmax-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathmax-vertical run ok");
    assert_eq!(
        manual, oracle,
        "manual mathmax-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathmax vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_mathmax_negative_operands_match_the_oracle() {
    // SIGNEDNESS edge: the mathmax vertical emits `smax` (SIGNED max, matching the `i64::max`
    // kernel), not `umax`. The primary vertical test's operands are all non-negative, so it would
    // green-light a `umax` mistake too — this case picks operands that SPAN negatives and where
    // signed vs unsigned max DIVERGE (`i - 2000` and `1000 - i` are negative for small/large `i`,
    // and under `umax` a negative i64 reads as a huge unsigned value → the wrong branch). Byte-
    // identity against the interpreter oracle (authoritative signed `i64::max`) discriminates, and
    // `hits>0` keeps a silent VM fallback from false-greening it.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.max(i - 2000, 1000 - i);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathmax negative-operand jit output must match the interpreter oracle (smax, not umax)"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathmax negative-operand run ok");
    assert_eq!(
        manual, oracle,
        "manual mathmax negative-operand jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathmax negative-operand edge must actually hit the JIT — else signedness is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mathmin_vertical() {
    // Mathmin-vertical DELIVERY-PATH proof: the exact `bench/micro/mathmin.phg` loop shape — a hot
    // `while` folding `Math.min(int, int)` with DATA-DEPENDENT operands (`i % 1000`, `i * 3 % 1000`)
    // so nothing constant-folds and the native call cannot be hoisted. The inline Cranelift `smin`
    // is byte-identical to the interpreter's `i64::min` kernel; a silent VM fallback would false-green
    // the byte-identity assert, so `hits>0` is the load-bearing check (proves the perf flip fired).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.min(i % 1000, i * 3 % 1000);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathmin-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathmin-vertical run ok");
    assert_eq!(
        manual, oracle,
        "manual mathmin-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathmin vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_mathmin_negative_operands_match_the_oracle() {
    // SIGNEDNESS edge: the mathmin vertical emits `smin` (SIGNED min, matching the `i64::min`
    // kernel), not `umin`. The primary vertical test's operands are all non-negative, so it would
    // green-light a `umin` mistake too — this case picks operands that SPAN negatives and where
    // signed vs unsigned min DIVERGE (`i - 2000` and `1000 - i` are negative for small/large `i`,
    // and under `umin` a negative i64 reads as a huge unsigned value → the wrong branch). Byte-
    // identity against the interpreter oracle (authoritative signed `i64::min`) discriminates, and
    // `hits>0` keeps a silent VM fallback from false-greening it.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.min(i - 2000, 1000 - i);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathmin negative-operand jit output must match the interpreter oracle (smin, not umin)"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathmin negative-operand run ok");
    assert_eq!(
        manual, oracle,
        "manual mathmin negative-operand jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathmin negative-operand edge must actually hit the JIT — else signedness is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mathsign_vertical() {
    // Mathsign-vertical DELIVERY-PATH proof: the exact `bench/micro/mathsign.phg` loop shape — a hot
    // `while` folding `Math.sign(int)` with a DATA-DEPENDENT operand (`i % 3 - 1`) that SPANS all
    // three sign outputs (-1, 0, +1) across iterations, so nothing constant-folds and every branch of
    // the branchless `pos - neg` materialization is exercised. The inline result is byte-identical to
    // the interpreter's `i64::from(n>0) - i64::from(n<0)` kernel; a silent VM fallback would false-
    // green the byte-identity assert, so `hits>0` is the load-bearing check.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.sign(i % 3 - 1);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathsign-vertical jit-wired output must match the interpreter oracle (spans -1/0/+1)"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathsign-vertical run ok");
    assert_eq!(
        manual, oracle,
        "manual mathsign-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathsign vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mathabs_vertical() {
    // Mathabs-vertical DELIVERY-PATH proof: the exact `bench/micro/mathabs.phg` loop shape — a hot
    // `while` folding `Math.abs(int)` with a DATA-DEPENDENT operand (`i % 2000 - 1000`) that SPANS
    // negatives (so `iabs` is genuinely exercised, not a no-op on non-negative inputs) but NEVER
    // reaches i64::MIN (no fault here — the fault edge has its own tests below). The inline
    // Cranelift `iabs` is byte-identical to the interpreter's `checked_abs` kernel for every in-range
    // operand; `hits>0` keeps a silent VM fallback from false-greening the byte-identity assert.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Math.abs(i % 2000 - 1000);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mathabs-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual mathabs-vertical run ok");
    assert_eq!(
        manual, oracle,
        "manual mathabs-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mathabs vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_mathabs_i64_min_funnels_to_redo_not_wrap() {
    // THE ABS FAULT GUARD, proven UNAMBIGUOUSLY on the JIT path: `Math.abs(i64::MIN)`. The kernel is
    // `checked_abs`, which returns `None` on i64::MIN → the VM faults. Cranelift's `iabs` would WRAP
    // i64::MIN to i64::MIN with no trap — so the arm guards `n == i64::MIN` → code 5 (redo on VM).
    // Compiling the function directly with `compile_unboxed` and running it forces the JIT-generated
    // code (no hotness threshold, no VM-path ambiguity), so a `Fault` here proves the GUARD fired —
    // a `Value` result would be the `iabs`-wrap divergence bug. i64::MIN is materialized as an
    // Int-kind LOCAL (`-9223372036854775807 - 1`), not a bare param: a bare param reads as `Unknown`
    // kind (bytecode carries no param types) and the arm would decline it before the guard is reached.
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         import Core.Math;\n\
         function abs1() -> int {\n\
           mutable int acc = 0;\n\
           mutable int i = 0;\n\
           int imin = -9223372036854775807 - 1;\n\
           while (i < 1) { acc = Math.abs(imin); i = i + 1; }\n\
           return acc;\n\
         }\n\
         #[Entry] function main() -> void {}",
    );
    let f = func_index(&program, "abs1");
    match Compiled::compile_unboxed(&program, f)
        .expect("int while with Math.abs on an Int-kind local is eligible")
        .run_unboxed(&[], 1)
    {
        JitRun::Fault(msg) => {
            assert_eq!(
                msg, REDO_ON_VM,
                "abs(i64::MIN) must funnel to VM redo, not wrap"
            )
        }
        JitRun::Value(v) => panic!(
            "abs(i64::MIN) must fault (guard), got wrapped value {} — the iabs-wrap divergence bug",
            as_int(&v)
        ),
    }
}

#[test]
fn jit_mathabs_i64_min_fault_matches_the_vm() {
    // FULL-DELIVERY fault parity for the abs guard: a JIT-eligible loop calls `Math.abs` on a
    // RUNTIME-materialized i64::MIN (`-9223372036854775807 - 1` — i64::MAX negated then minus 1, both
    // in-range so no literal-overflow fault masks the abs fault). The loop-containing `bench` compiles
    // EAGERLY on its first call, so the fault is reached on the JIT path; the code-5 guard redoes on
    // the VM, which renders the canonical `checked_abs` fault. `phg run` (JIT-wired) and the
    // interpreter oracle must BOTH fault with the same "integer overflow in Math.abs" string —
    // interp ≡ JIT fault parity (the whole point of the abs care; a wrapped value would NOT fault).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Math;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          int imin = -9223372036854775807 - 1;\n\
          while (i < iters) {\n\
            acc = acc + Math.abs(imin);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_err = crate::cli::cmd_run(SRC).expect_err("abs(i64::MIN) must fault on the jit path");
    let oracle_err =
        crate::cli::cmd_treewalk(SRC).expect_err("abs(i64::MIN) must fault on the oracle");
    assert!(
        jit_err.contains("integer overflow in Math.abs"),
        "jit fault must be the canonical kernel string, got: {jit_err}"
    );
    assert!(
        oracle_err.contains("integer overflow in Math.abs"),
        "oracle fault must be the canonical kernel string, got: {oracle_err}"
    );
    assert_eq!(
        jit_err, oracle_err,
        "abs(i64::MIN) jit fault must be byte-identical to the interpreter oracle fault"
    );
}

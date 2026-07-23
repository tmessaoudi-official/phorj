//! JIT range-analysis + `#[UncheckedOverflow]` + int-list vertical tests — the induction-counter
//! overflow-guard-drop recognizer (soundness surface), whole-function two's-complement wrapping, and
//! the flat int-list index vertical. Split from the `verticals.rs` monolith by cohesion (Inv 13).

use super::boxed::ub_int;
use super::unboxed_flow::{ub_float, vm_float};
use super::*;

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
        "package Main; import Core.Runtime.Entry;\n\
         function count(int n) -> int { mutable int i = 0; while (i < n) { i = i + 1; } return i; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function le(int n)    -> int { mutable int i = 0; while (i <= n)   { i = i + 1; } return i; }\n\
         function ne(int n)    -> int { mutable int i = 0; while (i != n)   { i = i + 1; } return i; }\n\
         function wrong(int n) -> int { mutable int i = 0; while (n < 100)  { i = i + 1; } return i; }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
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
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function bench(int iters, float r) -> float {\n\
           mutable float acc = 0.0;\n\
           mutable int i = 0;\n\
           while (i < iters) { acc = acc * r + 0.5; i = i + 1; }\n\
           return acc;\n\
         }\n\
         #[Entry] function main() -> void {}",
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
        "package Main; import Core.Runtime.Entry;\n\
         function f(int n) -> int { mutable int s = 1; mutable int i = 0; while (i < n) { s = s * 3; i = i + 1; } return s; }\n\
         #[Entry] function main() -> void {}",
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

// --- `#[UncheckedOverflow]` (import Core.Runtime.Integer.UncheckedOverflow): whole-function two's-complement wrapping int arithmetic.
// The fn-level `unchecked` flag makes the JIT emit plain `iadd`/`isub`/`imul`/`ineg` (no overflow guard,
// no sticky) — the WIN path (intadd LOSS→WIN) — and the result must be byte-identical to the VM, which
// reads the same flag and calls the same `value::int_wrapping_*` kernels. ---

#[test]
fn unchecked_function_wraps_add_sub_mul_without_faulting_and_matches_vm() {
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         import Core.Runtime.Integer.UncheckedOverflow;\n\
         #[UncheckedOverflow]\n\
         function wadd(int a, int b) -> int { return a + b; }\n\
         #[UncheckedOverflow]\n\
         function wsub(int a, int b) -> int { return a - b; }\n\
         #[UncheckedOverflow]\n\
         function wmul(int a, int b) -> int { return a * b; }\n\
         #[Entry] function main() -> void {}",
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
            .expect("an #[UncheckedOverflow] int fn is unboxed-eligible")
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

#[test]
fn qualified_unchecked_overflow_attribute_is_recognized_and_wraps_on_the_vm() {
    // Two-mode "nothing in the wind": the QUALIFIED form (`import Core.Runtime.Integer;` +
    // `#[Integer.UncheckedOverflow]`) is the SAME attribute as the bare leaf-import form, recognized
    // through the single-sourced `Attribute::is_unchecked_overflow`. Every other test/example exercises
    // the BARE form; this locks the qualified surface so a future recognition change can't silently drop
    // it. Asserts the compiler set the fn `unchecked` flag AND the VM wraps (MAX+1 → MIN) instead of
    // faulting — the VM reads that same flag, so a wrap proves end-to-end recognition on the VM path (the
    // interpreter reads the same predicate via `attrs_unchecked`; the shipped example covers interp ≡ VM).
    let program = compile_source(
        "package Main; import Core.Runtime.Entry;\n\
         import Core.Runtime.Integer;\n\
         #[Integer.UncheckedOverflow]\n\
         function wadd(int a, int b) -> int { return a + b; }\n\
         #[Entry] function main() -> void {}",
    );
    let f = func_index(&program, "wadd");
    assert!(
        program.functions[f].unchecked,
        "the compiler must set `unchecked` from the QUALIFIED `#[Integer.UncheckedOverflow]` form"
    );
    assert_eq!(
        vm_int(&program, f, vec![Value::Int(i64::MAX), Value::Int(1)]),
        i64::MIN,
        "qualified `#[Integer.UncheckedOverflow]`: MAX + 1 must WRAP to MIN on the VM, not fault"
    );
}

#[test]
fn unchecked_checked_call_boundary_byte_identical_both_directions() {
    // The mixed-call boundary — the load-bearing surface for the `cur_unchecked` save/restore in the
    // interp's `run_call` (and the VM reading each frame's own fn flag). NEITHER direction is covered by
    // the leaf tests above, so a future refactor that dropped the save/restore would only fail HERE.
    // Both directions asserted interp ≡ VM (`cmd_run` = VM+JIT vs `cmd_treewalk` = interp oracle).

    // (1) `#[UncheckedOverflow]` outer calling a CHECKED inner: the checked inner must STILL fault on overflow
    // even though the caller wraps — the callee's own flag governs, not the caller's.
    const A: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Runtime.Integer.UncheckedOverflow;\n\
        function inner(int n) -> int { return n + 1; }\n\
        #[UncheckedOverflow] function outer(int n) -> int { return inner(n); }\n\
        #[Entry] function main() -> void { Output.printLine(\"{outer(9223372036854775807)}\"); }";
    let a_jit = crate::cli::cmd_run(A);
    let a_oracle = crate::cli::cmd_treewalk(A);
    match (&a_jit, &a_oracle) {
        (Err(a), Err(b)) => assert_eq!(
            a, b,
            "checked inner under an #[UncheckedOverflow] outer must FAULT identically on both backends"
        ),
        _ => panic!("checked inner must fault on BOTH backends: jit={a_jit:?} oracle={a_oracle:?}"),
    }

    // (2) reverse — a CHECKED outer calling an `#[UncheckedOverflow]` inner: the inner WRAPS (its own flag),
    // and re-entering the checked outer afterward must restore checking (the save/restore).
    const B: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Runtime.Integer.UncheckedOverflow;\n\
        #[UncheckedOverflow] function inner(int n) -> int { return n + 1; }\n\
        function outer(int n) -> int { return inner(n); }\n\
        #[Entry] function main() -> void { Output.printLine(\"{outer(9223372036854775807)}\"); }";
    let b_jit = crate::cli::cmd_run(B).expect("wrapping inner returns a value");
    let b_oracle = crate::cli::cmd_treewalk(B).expect("wrapping inner returns a value");
    assert_eq!(
        b_jit, b_oracle,
        "#[UncheckedOverflow] inner under a checked outer must WRAP identically on both backends"
    );
    assert!(
        b_jit.contains("-9223372036854775808"),
        "the #[UncheckedOverflow] inner must wrap MAX+1 -> MIN, got {b_jit}"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_int_list_vertical() {
    // P-2c DELIVERY-PATH proof: the exact `bench/micro/listindex.phg` shape — an all-int
    // `MakeList` (seals FLAT as raw i64 slots) with a data-dependent `Index` — must JIT through
    // the hook AND stay byte-identical to the interpreter oracle.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int idx = (i + acc) % 8;\n\
            acc = acc + xs[idx];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "int-list vertical must match the oracle");
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual run ok");
    assert_eq!(manual, oracle);
    assert!(
        cache.borrow().hits > 0,
        "the int-list vertical must actually hit the JIT"
    );
}

#[test]
fn jit_int_list_oob_fault_matches_the_vm() {
    // Out-of-range on a flat int list → code 5 → the VM redo renders the canonical bounds fault.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          List<int> xs = [10, 20];\n\
          return xs[5];\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_err = crate::cli::cmd_run(SRC).expect_err("jit-wired run must fault");
    let oracle_err = crate::cli::cmd_treewalk(SRC).expect_err("interpreter must fault");
    assert!(
        jit_err.contains("list index out of range"),
        "jit-wired fault must be canonical, got: {jit_err}"
    );
    assert!(oracle_err.contains("list index out of range"));
}

#[test]
fn jit_int_list_negative_values_and_index_edges_match_the_oracle() {
    // Negative VALUES flow through the raw-i64 slots untouched; index 0 and len-1 both hit.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          List<int> xs = [0 - 5, 7, 0 - 9223372036854775807];\n\
          return xs[0] + xs[1] + xs[2] % 1000;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "negative int-list values must match");
}

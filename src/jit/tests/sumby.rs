//! `List.sumBy` unboxed fold vertical (the `count` loop with a CHECKED `sadd_overflow` accumulator,
//! byte-identical to the interpreter's `list_sum_by`) — delivery-path proof (`hits>0`) + edge
//! coverage (capture, negative projection, empty list) + the overflow → code-5 VM-redo fault path.
//! Sibling of `listcontains.rs` (Invariant 13).

use super::*;

#[test]
fn phg_run_hook_hits_the_jit_on_the_sumby_vertical() {
    // The exact `bench/micro/sumby.phg` shape: a constant `List<int>` and `List.sumBy(xs, x => x +
    // bump)` in a hot `while` — an `FnCap1` projection (captures `bump`) summed with the checked
    // accumulator. Must JIT through the `Op::Call` hook AND stay byte-identical to the interpreter;
    // a silent VM fallback would false-green the byte-identity assert, so `hits>0` is load-bearing.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 2, 3, 4, 5, 6, 7, 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 2;\n\
            acc = acc + List.sumBy(xs, function(int x) => x + bump);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "sumby-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual sumby-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the sumby vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_sumby_capture_free_negative_and_empty_edges_match_the_oracle() {
    // Edge coverage through the inline fold (in a hot loop so the vertical fires): a capture-free
    // `Fn` projection, a NEGATIVE element and a negating projection (i64 sum, not unsigned), and an
    // EMPTY list → 0 (the 0-element loop-header skip). Byte-identity + exact value.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [10, 0 - 3, 5];\n\
          List<int> empty = new List<int>();\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + List.sumBy(xs, function(int x) => x);\n\
            acc = acc + List.sumBy(xs, function(int x) => 0 - x);\n\
            acc = acc + List.sumBy(empty, function(int x) => x + 1);\n\
            acc = acc + 1;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1200)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit_out, oracle, "sumby edges must match the oracle");
    // per iter: (10-3+5)=12, plus -(12)=-12, plus empty=0, plus 1 = 1. × 1200 = 1200.
    assert_eq!(jit_out.trim(), (1200).to_string(), "sumby edge semantics");
}

#[test]
fn jit_sumby_overflow_faults_byte_identically_to_the_oracle() {
    // The checked accumulator's overflow path: two near-`i64::MAX` elements whose SUM overflows.
    // The interpreter's `checked_add` faults "integer overflow in List.sumBy"; the JIT's
    // `sadd_overflow` carry → code 5 → VM redo reproduces the SAME fault, never a silent wrap.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function f(): int {\n\
          List<int> xs = [9000000000000000000, 9000000000000000000];\n\
          return List.sumBy(xs, function(int x) => x);\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{f()}\"); }";
    let jit = crate::cli::cmd_run(SRC);
    let oracle = crate::cli::cmd_treewalk(SRC);
    match (&jit, &oracle) {
        (Err(a), Err(b)) => {
            assert_eq!(a, b, "sumby overflow: jit fault must match the oracle");
            assert!(
                a.contains("integer overflow in List.sumBy"),
                "must fault the canonical sumBy overflow, got:\n{a}"
            );
        }
        _ => panic!("sumby overflow: both must fault; jit={jit:?}, oracle={oracle:?}"),
    }
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_listreduce_vertical() {
    // The `bench/micro/listreduce.phg` shape: `List.reduce(xs, seed, (a,x) => a + x)` in a hot loop —
    // a 2-arg fold SEEDED from a data-dependent value. The `arm_list_reduce` vertical must JIT AND
    // match the interpreter oracle; `hits>0` is load-bearing (a silent VM fallback would false-green).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 2, 3, 4, 5, 6, 7, 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int seed = i % 7;\n\
            int total = List.reduce(xs, seed, function(int a, int x) => a + x);\n\
            acc = acc + total;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "listreduce jit output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual listreduce jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the listreduce vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_listreduce_seed_empty_and_negative_edges_match_the_oracle() {
    // Edge coverage: the SEED threads out unchanged on an EMPTY list (0-element loop skip), a
    // NEGATIVE-producing fold (i64, not unsigned), and a non-`a+x` combiner (`a - x`). Byte-identity
    // + exact value.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [10, 0 - 3, 5];\n\
          List<int> empty = new List<int>();\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + List.reduce(xs, 100, function(int a, int x) => a - x);\n\
            acc = acc + List.reduce(empty, 7, function(int a, int x) => a + x);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit_out, oracle, "listreduce edges must match the oracle");
    // per iter: (100 - 10 - (-3) - 5) = 88, plus empty→seed 7 = 95. × 1000 = 95000.
    assert_eq!(
        jit_out.trim(),
        (95000).to_string(),
        "listreduce edge semantics"
    );
}

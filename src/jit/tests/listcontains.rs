//! `List.contains` unboxed vertical (inline linear scan of the flat int block, byte-identical to the
//! interpreter's `list_contains`) — delivery-path proof (`hits>0`) + found/miss/negative edge
//! coverage against the oracle. Split out of the grandfathered `verticals.rs` (Invariant 13).

use super::*;

#[test]
fn phg_run_hook_hits_the_jit_on_the_listcontains_vertical() {
    // The exact `bench/micro/listcontains.phg` shape: a constant `List<int>` and `List.contains(xs,
    // i % 12)` in a hot `while` — the needle both HITS (present values) and MISSES (exhausted scan →
    // CLEAN false). Must JIT through the `Op::Call` hook AND stay byte-identical to the interpreter;
    // a silent VM fallback would false-green the byte-identity assert, so `hits>0` is load-bearing.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (List.contains(xs, i % 12)) { acc = acc + 1; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "listcontains-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual listcontains-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the listcontains vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn jit_listcontains_found_miss_negative_edges_match_the_oracle() {
    // Edge coverage through the inline scan (in a hot loop so the vertical fires): a NEGATIVE element
    // (i64 compare, not unsigned), FIRST + LAST positions present, and an absent needle → clean false.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 0 - 7, 4, 1, 5];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (List.contains(xs, 3)) { acc = acc + 1; }\n\
            if (List.contains(xs, 0 - 7)) { acc = acc + 10; }\n\
            if (List.contains(xs, 5)) { acc = acc + 100; }\n\
            if (List.contains(xs, 99)) { acc = acc + 1000; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1200)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit_out, oracle, "listcontains edges must match the oracle");
    // present 3 (+1), present -7 (+10), present 5 (+100), absent 99 (+0) = 111 per iter × 1200.
    assert_eq!(
        jit_out.trim(),
        (111 * 1200).to_string(),
        "listcontains edge semantics"
    );
}

#[test]
fn jit_listcontains_two_lists_same_needles_stay_exact() {
    // Two DIFFERENT lists probed with the SAME rotating needles in one loop — a guard against
    // any future caching/memo lever cross-hitting between receivers (a memo attempt here was
    // REVERTED 2026-07-23: 12 rotating pairs thrashed the 8 direct-mapped lines and the
    // per-miss install call cost 3x the plain scan — see the scorecard's listcontains note).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> a = [1, 2, 3, 4, 5, 6, 7, 8];\n\
          List<int> b = [2, 4, 6, 8, 10, 12, 14, 16];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (List.contains(a, i % 10)) { acc = acc + 1; }\n\
            if (List.contains(b, i % 10)) { acc = acc + 100; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "memo eviction rounds must stay exact");
}

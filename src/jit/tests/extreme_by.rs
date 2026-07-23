//! The ??-FUSED `List.maxBy`/`minBy` fold (DEC-332 maxby/minby flips) — delivery-path proofs
//! (`hits>0` on both bench shapes) + the edges that stress the fusion: the PARITY-AFFECTING
//! first-wins tie-break, a runtime-EMPTY receiver taking the `??` default, a `maxBy` OUTSIDE
//! the fusion window (falls back to the VM — correct, un-JITted), and the selector-overflow
//! fault path. Sibling of `hof_filter_map.rs` (Invariant 13).

use super::*;

fn assert_jit_hits(src: &str, label: &str) -> String {
    let jit_out = crate::cli::cmd_run(src).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(src).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "{label}: jit output must match the oracle");
    let program = compile_source(src);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(manual, oracle, "{label}: manual jit output must match");
    assert!(
        cache.borrow().hits > 0,
        "{label}: must actually hit the JIT — else the perf flip is unproven"
    );
    jit_out
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_maxby_vertical() {
    // The exact `bench/micro/maxby.phg` shape: data-dependent selector `(x + bump) % 7`, the
    // `?? 0` fusion window, the checksum folds the returned ELEMENT (not the key).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [5, 2, 8, 1, 9, 3, 7, 4];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 3;\n\
            acc = acc + (List.maxBy(xs, function(int x) => (x + bump) % 7) ?? 0);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "maxby vertical");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_minby_vertical() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [5, 2, 8, 1, 9, 3, 7, 4];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 3;\n\
            acc = acc + (List.minBy(xs, function(int x) => (x + bump) % 7) ?? 0);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "minby vertical");
}

#[test]
fn jit_extreme_by_tie_break_is_first_wins() {
    // Distinct elements with EQUAL selector keys: `x % 3` ties 7/1/4 (key 1) and 2/8/5 (key 2).
    // The fold must return the FIRST element achieving the extreme key (7 for min-key... the
    // max key 2 is first achieved by 2; the min key 0 by 3) — the interpreter's strict
    // replace-on-better fold. Any last-wins drift changes the checksum.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [7, 2, 3, 1, 8, 6, 4, 5];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int hi = List.maxBy(xs, function(int x) => x % 3) ?? 0;\n\
            int lo = List.minBy(xs, function(int x) => x % 3) ?? 0;\n\
            acc = acc + hi * 100 + lo;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "extreme-by tie-break");
}

#[test]
fn jit_extreme_by_empty_receiver_takes_the_coalesce_default() {
    // A runtime-EMPTY receiver (filter rejects everything) must yield the `??` default — the
    // fused fold's `count == 0` select leg, byte-identical to `null ?? 42`.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 3, 5];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            List<int> evens = List.filter(xs, function(int x) => x % 2 == 0);\n\
            acc = acc + (List.maxBy(evens, function(int x) => x) ?? 42);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "extreme-by empty default");
}

#[test]
fn maxby_outside_the_fusion_window_still_runs_correctly_on_the_vm() {
    // No `??` — the nullable result stays un-JITtable (fail closed) and the whole function
    // runs on the VM: output parity is the contract, a JIT hit is NOT asserted.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function pick(): int {\n\
          List<int> xs = [5, 2, 8];\n\
          int? best = List.maxBy(xs, function(int x) => x);\n\
          return best ?? -1;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{pick()}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit_out, oracle, "window-less maxBy must match the oracle");
}

#[test]
fn jit_extreme_by_selector_overflow_faults_byte_identically_to_the_oracle() {
    // The selector's checked add overflows mid-fold → code-5 VM redo renders the canonical
    // interpreter fault (pure graph — the whole-callee redo is exact).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [9223372036854775807, 1];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + (List.maxBy(xs, function(int x) => x + 1) ?? 0);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(3)}\"); }";
    let jit = crate::cli::cmd_run(SRC).expect_err("overflowing selector must fault");
    let oracle = crate::cli::cmd_treewalk(SRC).expect_err("oracle must fault");
    assert_eq!(jit, oracle, "fault strings must be byte-identical");
}

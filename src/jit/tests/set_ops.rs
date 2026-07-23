//! Set-OP verticals (DEC-332 setdifference/setunion flips) — delivery-path proofs (`hits>0` on
//! both bench shapes: memoized flat×flat builds, inline `Set.size`) + the edges: memo-line
//! collisions across BOTH ops on the same pair (difference vs union must never alias), results
//! feeding `Set.contains` (the sealed result is a real flat set), disjoint/subset/empty-result
//! pairs, and chained ops (a memoized result as an operand). Sibling of `map_materialize.rs`
//! (Invariant 13).

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
fn phg_run_hook_hits_the_jit_on_the_setdifference_vertical() {
    // The exact `bench/micro/setdifference.phg` shape: constant `a`, rotating `bs[i % 4]`
    // (a SetList index), the survivor CARDINALITY folds into the checksum.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> a = Set.of([1, 2, 3, 4, 5, 6, 7, 8]);\n\
          List<Set<int>> bs = [\n\
            Set.of([3, 4, 5, 6, 7, 8, 9, 10]),\n\
            Set.of([9, 10, 11, 12]),\n\
            Set.of([1, 2, 3, 4]),\n\
            Set.of([13, 14, 15, 16, 17])\n\
          ];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Set<int> b = bs[i % 4];\n\
            acc = acc + Set.size(Set.difference(a, b));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "setdifference vertical");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_setunion_vertical() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> a = Set.of([1, 2, 3, 4, 5, 6, 7, 8]);\n\
          List<Set<int>> bs = [\n\
            Set.of([3, 4, 5, 6, 7, 8, 9, 10]),\n\
            Set.of([9, 10, 11, 12]),\n\
            Set.of([1, 2, 3, 4]),\n\
            Set.of([13, 14, 15, 16, 17])\n\
          ];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Set<int> b = bs[i % 4];\n\
            acc = acc + Set.size(Set.union(a, b));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "setunion vertical");
}

#[test]
fn jit_set_ops_same_pair_both_ops_never_alias() {
    // difference(a, b) AND union(a, b) on the SAME pair in one loop: the two ops memoize in
    // SEPARATE entry ranges (24..32 vs 32..40) — an aliased line would swap counts (2 vs 12).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> a = Set.of([1, 2, 3, 4, 5, 6, 7, 8]);\n\
          Set<int> b = Set.of([3, 4, 5, 6, 7, 8, 9, 10]);\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Set.size(Set.difference(a, b)) * 100 + Set.size(Set.union(a, b));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "set ops no-alias");
}

#[test]
fn jit_set_op_results_answer_contains_and_chain() {
    // A memoized RESULT is a real sealed flat set: `Set.contains` probes it inline, and it can
    // be an OPERAND of the next op (chained difference-of-union).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> a = Set.of([1, 2, 3]);\n\
          Set<int> b = Set.of([3, 4, 5]);\n\
          Set<int> c = Set.of([5, 6]);\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Set<int> u = Set.union(a, b);\n\
            Set<int> d = Set.difference(u, c);\n\
            if (Set.contains(d, 4)) {\n\
              acc = acc + 10;\n\
            }\n\
            if (Set.contains(d, 5)) {\n\
              acc = acc + 1000;\n\
            }\n\
            acc = acc + Set.size(d);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "set ops contains/chain");
}

#[test]
fn jit_set_difference_disjoint_subset_and_empty_results() {
    // Edges: b ⊇ a (empty difference), disjoint (full difference), and union with a subset.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> a = Set.of([1, 2, 3]);\n\
          Set<int> superset = Set.of([1, 2, 3, 4]);\n\
          Set<int> disjoint = Set.of([7, 8]);\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Set.size(Set.difference(a, superset)) * 10000;\n\
            acc = acc + Set.size(Set.difference(a, disjoint)) * 100;\n\
            acc = acc + Set.size(Set.union(a, superset));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "set ops edges");
}

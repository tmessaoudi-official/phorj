//! HOF filter/map verticals (DEC-332 listfilter/mapfilter/mapmap flips) — delivery-path proofs
//! (`hits>0` on each bench shape: an inline closure call per element, an ACL builder / AMB map
//! record result, NO per-iteration seal) + the edges that stress the record scheme: order
//! preservation through `Map.get`, a `m[k] = v` builder-set onto a filtered AMB record, an
//! all-rejected (empty) survivor set, and the transform-overflow fault path. Sibling of
//! `map_materialize.rs` (Invariant 13).

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
fn phg_run_hook_hits_the_jit_on_the_listfilter_vertical() {
    // The exact `bench/micro/listfilter.phg` shape: a data-dependent predicate (`bump` flips
    // the surviving parity each iteration) so the survivor set cannot be folded or memoized.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 2, 3, 4, 5, 6, 7, 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 2;\n\
            List<int> ys = List.filter(xs, function(int x) => (x + bump) % 2 == 0);\n\
            acc = acc + List.length(ys);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "listfilter vertical");
}

#[test]
fn jit_listfilter_survivors_keep_order_and_values() {
    // The filtered ACL builder must hold the ORIGINAL elements in input order — indexed reads
    // (not just the length) prove element identity survives the conditional-append loop.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [7, 14, 3, 28, 5, 42];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            List<int> evens = List.filter(xs, function(int x) => x % 2 == 0);\n\
            acc = acc + evens[0] * 1000000 + evens[1] * 1000 + evens[2] + List.length(evens);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "listfilter order/values");
}

#[test]
fn jit_listfilter_all_rejected_is_an_empty_list() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 3, 5];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + List.length(List.filter(xs, function(int x) => x % 2 == 0));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "listfilter empty survivors");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mapfilter_vertical() {
    // The exact `bench/micro/mapfilter.phg` shape: a data-dependent value predicate over a
    // constant sealed flat map; the survivor CARDINALITY (`Map.size` over the AMB record) folds
    // into the checksum. A fresh record per iteration — recycled, never sealed.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 2, \"c\" => 3, \"d\" => 4,\n\
                                \"e\" => 5, \"f\" => 6, \"g\" => 7, \"h\" => 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 2;\n\
            Map<string, int> kept = Map.filter(m, function(int v) => (v + bump) % 2 == 0);\n\
            acc = acc + Map.size(kept);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "mapfilter vertical");
}

#[test]
fn jit_mapfilter_result_answers_gets_and_builder_sets() {
    // The filtered AMB record is a REAL builder map: `kept[\"key\"]` probes its table (the
    // `arm_index_map` AMB leg) and a subsequent `m[k] = v` set must extend it through
    // `rt_u_map_builder_set` — layout compatibility between `rt_u_map_ext_*` and the
    // mapinsert vertical is exactly what this asserts.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 2, \"c\" => 3, \"d\" => 4];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            mutable Map<string, int> kept = Map.filter(m, function(int v) => v % 2 == 0);\n\
            kept[\"z\"] = i;\n\
            acc = acc + kept[\"b\"] + kept[\"d\"] + kept[\"z\"] + Map.size(kept);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "mapfilter get/set compat");
}

#[test]
fn jit_mapfilter_all_rejected_is_an_empty_map() {
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 3];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + Map.size(Map.filter(m, function(int v) => v % 2 == 0));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "mapfilter empty survivors");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mapmap_vertical() {
    // The exact `bench/micro/mapmap.phg` shape: a data-dependent value transform, then
    // `Map.values` over the AMB result (the un-memoized AMB leg — a fresh recycled ACL per
    // iteration) indexed per iteration.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 2, \"c\" => 3, \"d\" => 4,\n\
                                \"e\" => 5, \"f\" => 6, \"g\" => 7, \"h\" => 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int bump = i % 3;\n\
            Map<string, int> mapped = Map.map(m, function(int v) => v + bump);\n\
            List<int> vs = Map.values(mapped);\n\
            acc = acc + vs[i % List.length(vs)];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "mapmap vertical");
}

#[test]
fn jit_mapmap_preserves_key_association_and_order() {
    // Keys keep their values' association through the transform: indexed gets by KEY (the AMB
    // table probe) and `Map.values` order (the rank walk) must both match the oracle.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"x\" => 10, \"y\" => 20, \"z\" => 30];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> sq = Map.map(m, function(int v) => v * v);\n\
            List<int> vs = Map.values(sq);\n\
            acc = acc + sq[\"y\"] * 10 + vs[0] + vs[2] + Map.size(sq);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(700)}\"); }";
    assert_jit_hits(SRC, "mapmap association/order");
}

#[test]
fn jit_mapmap_transform_overflow_faults_byte_identically_to_the_oracle() {
    // The transform's checked add overflows mid-loop → code-5 VM redo renders the canonical
    // interpreter fault (the unboxed graph is pure, so the whole-callee redo is exact).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 9223372036854775807];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> bumped = Map.map(m, function(int v) => v + 1);\n\
            acc = acc + Map.size(bumped);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(3)}\"); }";
    let jit = crate::cli::cmd_run(SRC).expect_err("overflowing transform must fault");
    let oracle = crate::cli::cmd_treewalk(SRC).expect_err("oracle must fault");
    assert_eq!(jit, oracle, "fault strings must be byte-identical");
}

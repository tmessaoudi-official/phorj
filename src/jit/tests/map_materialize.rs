//! Map MATERIALIZATION verticals (DEC-332 mapkeys/mapvalues/mapmerge/mapsize flips) —
//! delivery-path proofs (`hits>0` on each bench shape) + the edges that stress the memo scheme:
//! rotation through a `MapList`, memo-slot collisions (>8 distinct maps), a `List.append` onto a
//! SHARED `Map.keys` record (must COPY, never corrupt the memo), merge override/append order,
//! and the boxed long-key fallback. Sibling of `sumby.rs` (Invariant 13).

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
fn phg_run_hook_hits_the_jit_on_the_mapkeys_vertical() {
    // The exact `bench/micro/mapkeys.phg` shape: rotating maps (a MapList index), `Map.keys`
    // (memoized SHARED record), an indexed read of the keys list, `String.length`.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          Map<string, int> m1 = [\"a\" => 1, \"b\" => 2];\n\
          Map<string, int> m2 = [\"a\" => 1, \"b\" => 2, \"c\" => 3, \"d\" => 4, \"e\" => 5];\n\
          Map<string, int> m3 = [\"solo\" => 9];\n\
          List<Map<string, int>> maps = [m1, m2, m3];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> m = maps[i % 3];\n\
            List<string> ks = Map.keys(m);\n\
            acc = acc + String.length(ks[i % List.length(ks)]);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "mapkeys vertical");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mapvalues_vertical() {
    // The `bench/micro/mapvalues.phg` shape: `Map.values` (the memo entry's int twin) indexed
    // per iteration.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m1 = [\"a\" => 11, \"b\" => 22];\n\
          Map<string, int> m2 = [\"a\" => 3, \"b\" => 5, \"c\" => 7, \"d\" => 9, \"e\" => 13];\n\
          Map<string, int> m3 = [\"solo\" => 99];\n\
          List<Map<string, int>> maps = [m1, m2, m3];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> m = maps[i % 3];\n\
            List<int> vs = Map.values(m);\n\
            acc = acc + vs[i % List.length(vs)];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    assert_jit_hits(SRC, "mapvalues vertical");
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mapmerge_vertical() {
    // The `bench/micro/mapmerge.phg` shape: a constant lhs merged with a rotating rhs, the
    // merged-key cardinality folded (`Map.size` — flat count bits inline).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> a = [\"a\" => 1, \"b\" => 2, \"c\" => 3];\n\
          List<Map<string, int>> others = [\n\
            [\"b\" => 20, \"d\" => 4],\n\
            [\"c\" => 30, \"e\" => 5, \"f\" => 6],\n\
            [\"a\" => 10, \"g\" => 7]\n\
          ];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> merged = Map.merge(a, others[i % 3]);\n\
            acc = acc + Map.size(merged);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1500)}\"); }";
    let out = assert_jit_hits(SRC, "mapmerge vertical");
    // Per iter: |a ∪ b1| = 4, |a ∪ b2| = 5, |a ∪ b3| = 4 → (4+5+4) × 500 = 6500.
    assert_eq!(out.trim(), (6500).to_string(), "mapmerge cardinality");
}

#[test]
fn jit_mapmerge_override_and_append_order_match_the_oracle() {
    // Merge SEMANTICS through the memoized flat result: a shared key keeps `a`'s position but
    // takes `b`'s value; `b`'s new keys append. Read back via the mapget probe in a hot loop.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> a = [\"x\" => 1, \"y\" => 2];\n\
          Map<string, int> b = [\"y\" => 20, \"z\" => 30];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> m = Map.merge(a, b);\n\
            acc = acc + m[\"x\"] + m[\"y\"] + m[\"z\"];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1400)}\"); }";
    let out = assert_jit_hits(SRC, "mapmerge semantics");
    // Per iter: 1 (a's x) + 20 (b overrides y) + 30 (b's z appended) = 51 → × 1400 = 71400.
    assert_eq!(out.trim(), (71400).to_string(), "override/append semantics");
}

#[test]
fn jit_mapkeys_memo_collisions_and_empty_map_match_the_oracle() {
    // >8 distinct maps rotating through the 8-entry direct-mapped memo (evictions every round)
    // + an EMPTY map (a 0-word record — `List.length` reads 0, nothing is indexed). Correctness
    // must never depend on a memo hit.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          List<Map<string, int>> maps = [\n\
            [\"a\" => 1], [\"b\" => 2, \"bb\" => 3], [\"c\" => 4], [\"d\" => 5, \"dd\" => 6],\n\
            [\"e\" => 7], [\"f\" => 8, \"ff\" => 9], [\"g\" => 10], [\"h\" => 11], [\"i\" => 12],\n\
            [\"j\" => 13, \"jj\" => 14]\n\
          ];\n\
          Map<string, int> empty = new Map<string, int>();\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Map<string, int> m = maps[i % 10];\n\
            acc = acc + List.length(Map.keys(m)) + List.length(Map.values(m));\n\
            acc = acc + List.length(Map.keys(empty));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1300)}\"); }";
    let out = assert_jit_hits(SRC, "mapkeys memo collisions");
    // Per 10-iter round: key counts (1+2+1+2+1+2+1+1+1+2)=14, twice (keys+values)=28, empty=0.
    // 1300 iters = 130 rounds → 28 × 130 = 3640.
    assert_eq!(out.trim(), (3640).to_string(), "collision-round checksum");
}

#[test]
fn jit_list_append_onto_a_shared_keys_record_copies_never_corrupts() {
    // `ks = List.append(Map.keys(m), "x")` at a proven accumulator site: the SHARED record must
    // be COPIED into a fresh builder (inline fast path excluded by the SHARED bit; the helper
    // converts) — a later `Map.keys(m)` must still see the pristine memoized record.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"k1\" => 1, \"k2\" => 2];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            mutable List<string> ks = Map.keys(m);\n\
            ks = List.append(ks, \"extra\");\n\
            acc = acc + List.length(ks) + List.length(Map.keys(m));\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1200)}\"); }";
    let out = assert_jit_hits(SRC, "append onto SHARED keys record");
    // Per iter: appended length 3 + pristine length 2 = 5 → × 1200 = 6000. A corrupted memo
    // record would inflate the second term round over round.
    assert_eq!(out.trim(), (6000).to_string(), "SHARED copy-on-append");
}

#[test]
fn jit_mapkeys_long_key_boxed_map_falls_back_byte_identically() {
    // A >22-byte key defeats the flat seal (boxed `Value::Map`) — keys/values take the
    // canonical clone leg (untagged handles, un-memoized) and must stay byte-identical.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.Map;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"this-key-is-way-longer-than-twenty-two-bytes\" => 7, \"b\" => 8];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            List<string> ks = Map.keys(m);\n\
            List<int> vs = Map.values(m);\n\
            acc = acc + String.length(ks[0]) + vs[1] + Map.size(m);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("oracle ok");
    assert_eq!(jit_out, oracle, "boxed-map fallback must match the oracle");
    // Per iter: 44 (long key) + 8 (vs[1]) + 2 (size) = 54 → × 600 = 32400.
    assert_eq!(
        jit_out.trim(),
        (32400).to_string(),
        "boxed fallback checksum"
    );
}

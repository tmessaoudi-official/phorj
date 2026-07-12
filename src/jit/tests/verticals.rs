//! Delivery-path + handle-vertical tests: the `phg run` hook, string/list/map verticals,
//! fault parity through code-5 redo.

use super::boxed::ub_int;
use super::unboxed_flow::{ub_float, vm_float};
use super::*;

#[test]
fn phg_run_hook_actually_hits_the_jit() {
    // A silent 100%-fallback to the VM would pass every byte-identity check identically and prove
    // nothing — so this asserts the hit counter is non-zero, i.e. the native path genuinely ran.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function fib(int n) -> int { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }\n\
        function main() -> void { Output.printLine(\"{fib(10)}\"); }";
    // Byte-identity: the jit-wired run must match the interpreter oracle (Invariant 2).
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "jit-wired output must match the interpreter oracle"
    );
    // Prove the JIT path was actually hit (build a Vm with an inspectable shared cache).
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual jit-wired output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the JIT path must actually be hit — a silent fallback false-greens byte-identity"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_an_int_loop() {
    // widen-1 DELIVERY-PATH proof (loops): an int `while` loop in a CALLED function must JIT through the
    // `Op::Call` hook. (A loop in `main` never JITs — `main` prints, so it is ineligible, and the
    // entry-level JIT cannot reach its body; the loop MUST live in a callee, exactly the
    // `bench/micro/intadd.phg` shape.) Byte-identity alone can't prove the flip — a silent VM fallback
    // false-greens it — so this asserts the hit counter fires, i.e. the widened subset genuinely runs
    // native at the CLI.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters) -> int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc;\n\
        }\n\
        function main() -> void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "int-loop jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual int-loop jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "an int loop in a called function must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_string_vertical() {
    // P-2a handle-space DELIVERY-PATH proof: the exact `bench/micro/stringconcat.phg` shape —
    // string consts, `MakeList`, varying `Index`, `Concat`, `String.length`, `Pop` — must JIT
    // through the `Op::Call` hook AND stay byte-identical to the interpreter oracle. 1000
    // iterations also exercise the `UbCtx` free-list steady state (temps are recycled, not grown).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"alpha\", \"beta\", \"gamma\", \"delta\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string s = parts[i % 4] + parts[(i + 1) % 4];\n\
            acc = acc + String.length(s);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "string-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual string-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the string vertical must actually hit the JIT — else the P-2a flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_listappend_vertical() {
    // Listappend-vertical DELIVERY-PATH proof: the exact `bench/micro/listappend.phg` shape —
    // `xs = List.append(xs, i)` at an accumulator site (consumed into an ACL builder record,
    // in-place push), the every-iteration `List.length` reset probe (inline ACL len read),
    // `xs[0]`/`xs[255]` reads through the helper's ACL arm, and the `xs = [0]` reset (the
    // release ladder recycles the record, keeping its grown buffer). Must JIT through the
    // `Op::Call` hook AND stay byte-identical to the interpreter oracle. 2000 iterations
    // cross the 256 reset boundary several times, proving record recycling reaches steady state.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          mutable List<int> xs = [0];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            xs = List.append(xs, i);\n\
            if (List.length(xs) >= 256) {\n\
              acc = acc + List.length(xs) + xs[0] + xs[255];\n\
              xs = [0];\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc + List.length(xs);\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "listappend-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual listappend-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the listappend vertical must actually hit the JIT — else the builder flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_mapinsert_vertical() {
    // Mapinsert-vertical DELIVERY-PATH proof: the exact `bench/micro/mapinsert.phg` shape —
    // `m[k] = v` (`Op::SetIndexLocal`) over cycling flat-list string keys: the first write
    // CONVERTS the sealed map into an AMB builder record (helper), the 8-per-cycle inserts
    // take the helper, every overwrite takes the inline packed-table probe + one store;
    // `m["alpha"]`/`m["theta"]` reads go through the helper's AMB arm, and the `m = [...]`
    // reset recycles the record (grown buffer kept). Must JIT through the `Op::Call` hook
    // AND stay byte-identical to the interpreter oracle. 2000 iterations cross the 64-step
    // reset boundary many times, proving record recycling reaches steady state.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<string> keys = [\"alpha\", \"beta\", \"gamma\", \"delta\", \"epsi\", \"zeta\", \"eta\", \"theta\"];\n\
          mutable Map<string, int> m = [\"alpha\" => 0];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = keys[i % 8];\n\
            m[k] = i;\n\
            if (i % 64 == 63) {\n\
              acc = acc + m[\"alpha\"] + m[\"theta\"];\n\
              m = [\"alpha\" => 0];\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mapinsert-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual mapinsert-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the mapinsert vertical must actually hit the JIT — else the builder flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_hofpipe_vertical() {
    // Hofpipe-vertical DELIVERY-PATH proof: the exact `bench/micro/hofpipe.phg` shape —
    // `List.map` with a ONE-int-capture lambda (`FnCap1`: the capture word IS the stack cell,
    // prepended as arg 0 on the direct per-element call) into an ACL builder output, then
    // `List.count` with a capture-free Bool predicate consuming that owned ACL (record
    // recycled at the release). The varying capture `k` proves the capture is live. Must JIT
    // through the `Op::Call` hook AND stay byte-identical to the interpreter oracle.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int k = i % 7 + 1;\n\
            List<int> ys = List.map(xs, function(int x) => x * k);\n\
            acc = acc + List.count(ys, function(int y) => y % 2 == 0);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "hofpipe-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual hofpipe-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the hofpipe vertical must actually hit the JIT — else the HOF flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_forin_pointer_walk() {
    // Lever-3 DELIVERY-PATH proof: the exact `bench/micro/forin.phg` shape — a nested
    // `for (x in xs)` over a flat const list. The desugar's elems/j cells become (end,
    // cursor) pointers at the `IterElems; Const(0)` init; Len is an identity re-push, the
    // header Lt one unsigned compare, `xs[j]` ONE load, `j+1` a `+64` bump. Must JIT through
    // the `Op::Call` hook AND stay byte-identical to the interpreter oracle (including the
    // empty-list edge: start == end skips the loop like `0 < 0`).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            for (int x in xs) {\n\
              acc = acc + x;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "forin pointer-walk jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual forin pointer-walk jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the forin pointer walk must actually hit the JIT — else lever 3 is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_general_list_append() {
    // The GENERAL (non-accumulator) `List.append` — target != source, so the ACL fast path
    // does not apply and the clone helper carries full PHP value semantics: `xs` must stay
    // 3 elements forever while each `ys` is a fresh 4-element list (read back via the boxed
    // Index helper). Also exercises the str-list variant. hits > 0 + byte-identity.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          List<int> xs = [1, 2, 3];\n\
          List<string> ss = [\"a\", \"b\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            List<int> ys = List.append(xs, i);\n\
            List<string> ts = List.append(ss, \"c\");\n\
            acc = acc + List.length(ys) + ys[3] + List.length(xs) + List.length(ts);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "general list-append jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual general list-append must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the general list-append must actually hit the JIT"
    );
}

#[test]
fn iterated_local_also_written_declines_to_the_vm_byte_identically() {
    // The MUTATION GUARD: iterating a local AND writing it in the same function (append
    // during iteration — the VM's for-in iterates a SNAPSHOT; a JIT ACL append/reseed would
    // mutate or recycle the record IN PLACE under the walker). The whole function must
    // decline (fall back to the VM) and stay byte-identical — snapshot semantics preserved.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.List;\n\
        function bench(int iters): int {\n\
          mutable List<int> xs = [1, 2, 3];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            for (int x in xs) {\n\
              xs = List.append(xs, x);\n\
              acc = acc + x + List.length(xs);\n\
            }\n\
            xs = [1, 2, 3];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(50)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "iterate-while-append must stay byte-identical (snapshot semantics via VM fallback)"
    );
    // And the guard must actually decline it — a compile success here would mean the JIT
    // walks a buffer that the body's in-place append is mutating under it.
    let program = compile_source(SRC);
    let bench = (0..program.functions.len())
        .find(|f| program.functions[*f].arity == 1)
        .expect("bench fn");
    assert!(
        matches!(
            Compiled::compile_unboxed(&program, bench),
            Err(JitError::Unsupported(_))
        ),
        "the mutation guard must decline an iterated-and-written local"
    );
}

#[test]
fn jit_string_vertical_long_and_multibyte_concat_match_the_oracle() {
    // The `Concat` helper routes through the single-sourced `PhStr::concat` kernel: exercise BOTH
    // representations (short → inline, long → heap) and multibyte UTF-8 through the jit-wired run.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"héllo-wörld-Ω\", \"a-deliberately-long-segment-over-22-bytes\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string s = parts[i % 2] + parts[(i + 1) % 2];\n\
            acc = acc + String.length(s);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(64)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "long/multibyte concat must match the oracle"
    );
}

#[test]
fn jit_string_vertical_index_fault_matches_the_vm() {
    // An out-of-range `Index` inside the JIT'd vertical returns the fault sentinel → code 5 → the
    // hook falls back to the VM, which renders the canonical fault. The jit-wired run must fail with
    // the SAME fault body as the interpreter (byte-identical failure behaviour, Invariant 1).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"alpha\", \"beta\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string s = parts[i] + parts[0];\n\
            acc = acc + String.length(s);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(5)}\"); }";
    let jit_err = crate::cli::cmd_run(SRC).expect_err("jit-wired run must fault");
    let oracle_err = crate::cli::cmd_treewalk(SRC).expect_err("interpreter must fault");
    assert!(
        jit_err.contains("list index out of range"),
        "jit-wired fault must be the canonical bounds fault, got: {jit_err}"
    );
    assert!(
        oracle_err.contains("list index out of range"),
        "oracle fault must be the canonical bounds fault, got: {oracle_err}"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_map_vertical() {
    // P-2b DELIVERY-PATH proof: the exact `bench/micro/mapget.phg` shape — a `MakeMap` of short
    // string keys → int values (seals FLAT), a flat key list, and a string-subscripted `Index`
    // (the inline hash-probe) — must JIT through the `Op::Call` hook AND stay byte-identical to
    // the interpreter oracle. 1000 iterations exercise the probe across all four keys.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 10, \"b\" => 20, \"c\" => 30, \"d\" => 40];\n\
          List<string> keys = [\"a\", \"b\", \"c\", \"d\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = keys[i % 4];\n\
            acc = acc + m[k];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(1000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "map-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual map-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the map vertical must actually hit the JIT — else the P-2b flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_mixed_interpolation() {
    // Webish-vertical proof: mixed `Concat(n)` interpolation (`"h={v} p={p}"`) runs FULLY
    // INLINE for the hot shape — IR digit render (sign, zero, i64::MIN/MAX) + slot joins —
    // while >22-byte totals (the MIN/MAX bodies) take the fused helper: BOTH paths exercise
    // in ONE loop. The `check` map probe makes `acc` depend on the EXACT rendered bytes (a
    // wrong render misses the key and faults on the JIT leg only → outputs diverge → caught).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<int> vals = [(0 - 9223372036854775807) - 1, 0 - 42, 0, 7, 9223372036854775807, 123456];\n\
          List<string> paths = [\"/\", \"/users\", \"/posts\", \"/a\", \"/b\", \"/c\"];\n\
          Map<string, int> check = [\"h=-42 p=/users\" => 3, \"h=0 p=/posts\" => 5, \"h=7 p=/a\" => 7];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int v = vals[i % 6];\n\
            string p = paths[i % 6];\n\
            string body = \"h={v} p={p}\";\n\
            int j = i % 6;\n\
            if (j >= 1) { if (j <= 3) { acc = acc + check[body]; } }\n\
            acc = acc + String.length(body);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "mixed-interpolation jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual mixed-interpolation jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "mixed interpolation must actually hit the JIT — else the webish flip is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_string_accumulator() {
    // Strbuild-vertical proof: the `s = s + x` accumulator runs on an ACC record — helper
    // conversion on the FIRST append (fn entry and after every `s = ""` reset), fully-inline
    // cap-checked appends after, helper growth when the doubling cap is hit, record recycle +
    // buffer reuse at each reset. `String.length(s)` reads the record len inline. The `check`
    // map probe pins the EXACT accumulated bytes early (byte-identity through the ACC path),
    // and the length-fold covers every append thereafter.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"alpha\", \"beta\", \"gamma\", \"delta\"];\n\
          Map<string, int> check = [\"alphabeta\" => 7, \"alphabetagamma\" => 11];\n\
          mutable string s = \"\";\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            s = s + parts[i % 4];\n\
            if (i == 1) { acc = acc + check[s]; }\n\
            if (i == 2) { acc = acc + check[s]; }\n\
            if (String.length(s) > 512) {\n\
              acc = acc + String.length(s);\n\
              s = \"\";\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc + String.length(s);\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "string-accumulator jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual string-accumulator jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the string accumulator must actually hit the JIT — else the strbuild flip is unproven"
    );
}

#[test]
fn jit_map_vertical_long_key_stays_boxed_and_matches_the_oracle() {
    // A >22-byte key defeats flattening: the seal falls back to a boxed `Value::Map` and every
    // lookup routes through the helper into the canonical `map_index` kernel. Byte-identity must
    // hold on that path too (long AND short keys mixed — the short one also stays boxed here,
    // exercising the helper's slot-key + boxed-map combination).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a-deliberately-long-key-over-22-bytes\" => 7, \"b\" => 20];\n\
          List<string> keys = [\"a-deliberately-long-key-over-22-bytes\", \"b\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = keys[i % 2];\n\
            acc = acc + m[k];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(64)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "boxed-map lookup must match the oracle");
}

#[test]
fn jit_map_vertical_duplicate_keys_dedup_like_the_kernel() {
    // Duplicate literal keys are legal (checker only type-checks them): `build_map`'s PHP
    // semantics — FIRST position, LAST value — must survive the flat seal. `m[\"a\"]` must read 2,
    // never 1, on all backends.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 5, \"a\" => 2];\n\
          return m[\"a\"] * 100 + m[\"b\"];\n\
        }\n\
        function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "dedup semantics must match the oracle");
    assert!(
        jit_out.contains("205"),
        "last-value-wins dedup must read 205, got: {jit_out}"
    );
}

#[test]
fn jit_map_vertical_larger_map_walks_buckets_and_matches_the_oracle() {
    // 12 pairs → a 32-bucket table: exercises the open-addressed walk (collisions + wraparound)
    // across every key, byte-identical to the oracle.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"k0\" => 1, \"k1\" => 2, \"k2\" => 3, \"k3\" => 4,\n\
            \"k4\" => 5, \"k5\" => 6, \"k6\" => 7, \"k7\" => 8,\n\
            \"k8\" => 9, \"k9\" => 10, \"k10\" => 11, \"k11\" => 12];\n\
          List<string> keys = [\"k0\", \"k1\", \"k2\", \"k3\", \"k4\", \"k5\",\n\
            \"k6\", \"k7\", \"k8\", \"k9\", \"k10\", \"k11\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = keys[i % 12];\n\
            acc = acc + m[k];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(240)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "bucket-walk lookup must match the oracle");
    assert!(
        jit_out.contains("1560"),
        "240 iterations over sum(1..=12)=78 must read 1560, got: {jit_out}"
    );
}

#[test]
fn jit_map_vertical_missing_key_fault_matches_the_vm() {
    // A missing key in a FLAT map exhausts the inline probe → code 5 → the hook falls back to the
    // VM, which renders the canonical `\"map key not found\"` fault — byte-identical failure
    // behaviour (Invariant 1).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          Map<string, int> m = [\"a\" => 10];\n\
          return m[\"zzz\"];\n\
        }\n\
        function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_err = crate::cli::cmd_run(SRC).expect_err("jit-wired run must fault");
    let oracle_err = crate::cli::cmd_treewalk(SRC).expect_err("interpreter must fault");
    assert!(
        jit_err.contains("map key not found"),
        "jit-wired fault must be the canonical missing-key fault, got: {jit_err}"
    );
    assert!(
        oracle_err.contains("map key not found"),
        "oracle fault must be the canonical missing-key fault, got: {oracle_err}"
    );
}

#[test]
fn jit_map_vertical_concat_key_probes_through_the_helper() {
    // An inline-concat result carries hash 0 (\"unavailable\") — the inline probe must PUNT to the
    // helper (which compares bytes), never miss-fault a present key. `\"a\" + \"b\"` == \"ab\" is in
    // the map; the lookup must succeed with the right value on the jit-wired path.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"ab\" => 11, \"cd\" => 22];\n\
          List<string> parts = [\"a\", \"b\", \"c\", \"d\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = parts[(i % 2) * 2] + parts[(i % 2) * 2 + 1];\n\
            acc = acc + m[k];\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(64)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "hash-0 concat keys must route through the helper and match the oracle"
    );
}

#[test]
fn jit_stack_overflow_threshold_matches_the_oracle() {
    // The ONE correctness vector the fault-fallback cannot catch: an under-fault (wrong
    // `start_depth`) makes the JIT RETURN A VALUE where the VM overflows — no fault, so no re-run.
    // A LINEAR eligible recursion bracketing `MAX_CALL_DEPTH`: under `--features jit`, cmd_run routes
    // `countdown` through the JIT; the interpreter (cmd_treewalk) is never JITted, so it is the pure
    // depth oracle (Invariant 2). Running through the real cmd_run path (its `on_deep_stack` 256MB
    // thread) also proves 4096 native JIT frames don't blow the production stack.
    use crate::limits::MAX_CALL_DEPTH;
    for n in (MAX_CALL_DEPTH - 3)..=(MAX_CALL_DEPTH + 2) {
        let src = format!(
            "package Main;\n\
             import Core.Output;\n\
             function countdown(int n) -> int {{ if (n <= 0) {{ return 0; }} return countdown(n - 1); }}\n\
             function main() -> void {{ Output.printLine(\"{{countdown({n})}}\"); }}"
        );
        let jit = crate::cli::cmd_run(&src);
        let oracle = crate::cli::cmd_treewalk(&src);
        match (&jit, &oracle) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "countdown({n}): jit output must match the oracle"),
            (Err(a), Err(b)) => assert_eq!(a, b, "countdown({n}): jit fault must match the oracle"),
            _ => panic!(
                "countdown({n}): jit/oracle disagree on success-vs-fault: jit={jit:?}, oracle={oracle:?}"
            ),
        }
    }
}

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
        "package Main;\n\
         function count(int n) -> int { mutable int i = 0; while (i < n) { i = i + 1; } return i; }\n\
         function main() -> void {}",
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
        "package Main;\n\
         function le(int n)    -> int { mutable int i = 0; while (i <= n)   { i = i + 1; } return i; }\n\
         function ne(int n)    -> int { mutable int i = 0; while (i != n)   { i = i + 1; } return i; }\n\
         function wrong(int n) -> int { mutable int i = 0; while (n < 100)  { i = i + 1; } return i; }\n\
         function main() -> void {}",
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
        "package Main;\n\
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
         function main() -> void {}",
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
        "package Main;\n\
         function bench(int iters, float r) -> float {\n\
           mutable float acc = 0.0;\n\
           mutable int i = 0;\n\
           while (i < iters) { acc = acc * r + 0.5; i = i + 1; }\n\
           return acc;\n\
         }\n\
         function main() -> void {}",
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
        "package Main;\n\
         function f(int n) -> int { mutable int s = 1; mutable int i = 0; while (i < n) { s = s * 3; i = i + 1; } return s; }\n\
         function main() -> void {}",
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
        "package Main;\n\
         import Core.Runtime.Integer.UncheckedOverflow;\n\
         #[UncheckedOverflow]\n\
         function wadd(int a, int b) -> int { return a + b; }\n\
         #[UncheckedOverflow]\n\
         function wsub(int a, int b) -> int { return a - b; }\n\
         #[UncheckedOverflow]\n\
         function wmul(int a, int b) -> int { return a * b; }\n\
         function main() -> void {}",
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
    // interpreter reads the same predicate via `attrs_unchecked`; the shipped example covers run≡runvm).
    let program = compile_source(
        "package Main;\n\
         import Core.Runtime.Integer;\n\
         #[Integer.UncheckedOverflow]\n\
         function wadd(int a, int b) -> int { return a + b; }\n\
         function main() -> void {}",
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
    // Both directions asserted run≡runvm (`cmd_run` = VM+JIT vs `cmd_treewalk` = interp oracle).

    // (1) `#[UncheckedOverflow]` outer calling a CHECKED inner: the checked inner must STILL fault on overflow
    // even though the caller wraps — the callee's own flag governs, not the caller's.
    const A: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Runtime.Integer.UncheckedOverflow;\n\
        function inner(int n) -> int { return n + 1; }\n\
        #[UncheckedOverflow] function outer(int n) -> int { return inner(n); }\n\
        function main() -> void { Output.printLine(\"{outer(9223372036854775807)}\"); }";
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
    const B: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.Runtime.Integer.UncheckedOverflow;\n\
        #[UncheckedOverflow] function inner(int n) -> int { return n + 1; }\n\
        function outer(int n) -> int { return inner(n); }\n\
        function main() -> void { Output.printLine(\"{outer(9223372036854775807)}\"); }";
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
    const SRC: &str = "package Main;\n\
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
        function main(): void { Output.printLine(\"{bench(1000)}\"); }";
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
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          List<int> xs = [10, 20];\n\
          return xs[5];\n\
        }\n\
        function main(): void { Output.printLine(\"{bench()}\"); }";
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
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          List<int> xs = [0 - 5, 7, 0 - 9223372036854775807];\n\
          return xs[0] + xs[1] + xs[2] % 1000;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench()}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "negative int-list values must match");
}

// --- Task 9: accumulator overflow-check elision (the interval pass) — structural proofs +
// byte-identity on the elided code, the guard-decline path, and a genuine overflow fault. ---

/// The task-9 pass result for `name` (`None` = out of the v1 scope / unprovable).
fn acc_elision(program: &BytecodeProgram, name: &str) -> Option<super::AccElision> {
    let f = func_index(program, name);
    let func = &program.functions[f];
    let base = super::range_proven_ops(func);
    super::accumulator_elision(func, &base)
}

#[test]
fn task9_proves_affine_accumulator_with_param_bound_guard() {
    // The intadd shape: `acc = acc + (i * 3 - 1)` — the site AddI, the affine MulI/SubI and
    // the counter AddI must ALL be proven; the runtime param bound needs ONE entry guard.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc; }\n\
        function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let program = compile_source(SRC);
    let acc = acc_elision(&program, "bench").expect("the intadd shape must be provable");
    let proven = acc.proven.iter().filter(|&&p| p).count();
    assert!(
        proven >= 4,
        "site AddI + affine MulI/SubI + counter AddI must be proven, got {proven}"
    );
    assert_eq!(acc.guards.len(), 1, "a param bound needs exactly one guard");
    assert!(acc.guards[0].1 >= (1 << 20), "G ladder floor");
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "elided intadd shape must match the oracle");
}

#[test]
fn task9_proves_const_map_accumulator_and_expression_remi() {
    // The mapget/listindex shapes in one: a const-map value accumulator plus an
    // expression-dividend `% 8` (provably non-negative → the band lowering).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 10, \"b\" => 20, \"c\" => 30, \"d\" => 40];\n\
          List<string> keys = [\"a\", \"b\", \"c\", \"d\"];\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) {\n\
            string k = keys[i % 4];\n\
            int idx = (i + acc) % 8;\n\
            acc = acc + m[k] + xs[idx];\n\
            i = i + 1;\n\
          }\n\
          return acc; }\n\
        function main(): void { Output.printLine(\"{bench(3000)}\"); }";
    let program = compile_source(SRC);
    let acc = acc_elision(&program, "bench").expect("const-collection accumulator must prove");
    assert!(acc.proven.iter().filter(|&&p| p).count() >= 4);
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "elided collection shape must match the oracle"
    );
}

#[test]
fn task9_guard_decline_beyond_g_stays_byte_identical() {
    // A bound ABOVE the verified G must decline at entry (code 5 → the VM runs the call) —
    // and the output must be byte-identical. `acc = acc + 5e12` forces the ladder down to
    // G = 2^20 (5e12·2^31 and 5e12·2^24 overflow i64; 5e12·2^20 = 5.24e18 fits), so
    // iters = 2^20 + 1 crosses the guard while still running quickly on the VM leg.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { acc = acc + 5000000000000; i = i + 1; }\n\
          return acc; }\n\
        function main(): void { Output.printLine(\"{bench(1048577)}\"); }";
    let program = compile_source(SRC);
    let acc = acc_elision(&program, "bench").expect("const-growth accumulator must prove");
    assert_eq!(
        acc.guards[0].1,
        1 << 20,
        "5e12 growth must push the ladder down to G = 2^20"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "the guard-decline path must match the oracle"
    );
}

#[test]
fn task9_rejects_unbounded_growth_and_overflow_faults_identically() {
    // `acc = acc + 20000000000000` (2e13): even G = 2^20 gives 2.1e19 > i64::MAX — the pass
    // must REJECT (checked emission stays), and a genuine overflow must fault identically on
    // both legs (the sticky redo → the VM's canonical fault).
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 9000000000000000000; mutable int i = 0;\n\
          while (i < iters) { acc = acc + 20000000000000; i = i + 1; }\n\
          return acc; }\n\
        function main(): void { Output.printLine(\"{bench(20000)}\"); }";
    let program = compile_source(SRC);
    assert!(
        acc_elision(&program, "bench").is_none(),
        "2e13 growth must fail every ladder rung — stays checked"
    );
    let jit_out = crate::cli::cmd_run(SRC);
    let oracle = crate::cli::cmd_treewalk(SRC);
    assert_eq!(
        format!("{jit_out:?}"),
        format!("{oracle:?}"),
        "the genuine overflow fault must be byte-identical (VM redo renders it)"
    );
}

#[test]
fn task9_rejects_computed_bound_and_body_branches() {
    // A COMPUTED loop bound (not a param, not a const) and an `if` inside the body are both
    // out of the v1 scope — the pass must fail closed on each.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function computed(int n): int { int lim = n * 2; mutable int acc = 0; mutable int i = 0;\n\
          while (i < lim) { acc = acc + 1; i = i + 1; } return acc; }\n\
        function branchy(int n): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { if (i > 2) { acc = acc + 2; } i = i + 1; } return acc; }\n\
        function main(): void { Output.printLine(\"{computed(5)} {branchy(9)}\"); }";
    let program = compile_source(SRC);
    assert!(
        acc_elision(&program, "computed").is_none(),
        "computed bound is out of v1 scope"
    );
    assert!(
        acc_elision(&program, "branchy").is_none(),
        "body branches are out of v1 scope"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle);
}

#[test]
fn phg_run_hook_hits_the_jit_on_for_in_iteration() {
    // Forin-vertical proof: `for (x in xs)` desugars to `IterElems` + an indexed while over
    // `Len` — a BORROWED flat list handle IS its element snapshot (identity, zero
    // instructions) and `Len` reads the count from the handle bits. Covers int-list AND
    // str-list iteration, byte-identity, and hits>0.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          List<string> ws = [\"alpha\", \"be\", \"gamma\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            for (int x in xs) {\n\
              acc = acc + x;\n\
            }\n\
            for (string w in ws) {\n\
              acc = acc + String.length(w);\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(500)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "for-in jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual for-in jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "for-in iteration must actually hit the JIT — else the forin flip is unproven"
    );
}

#[test]
fn task9_v2_proves_nested_for_in_accumulator_and_index_bounds() {
    // The forin shape: an accumulator fed inside an INNER counted loop whose bound is
    // `Len(iter)` of a compile-time-known list. v2 must prove the accumulator sites, the
    // inner+outer counters AND the in-bounds `Index` (its bounds branch drops); byte
    // identity must hold, including at the entry-guard decline.
    const SRC: &str = "package Main;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1, 5, 9, 2, 6];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            for (int x in xs) {\n\
              acc = acc + x;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        function main(): void { Output.printLine(\"{bench(700)}\"); }";
    let program = compile_source(SRC);
    let acc = acc_elision(&program, "bench").expect("the nested for-in shape must prove (v2)");
    let proven = acc.proven.iter().filter(|&&p| p).count();
    // Site AddI + inner counter AddI + outer counter AddI + the in-bounds Index ≥ 4 marks.
    assert!(
        proven >= 4,
        "nested proofs expected >= 4 marks, got {proven}"
    );
    let f = func_index(&program, "bench");
    let func = &program.functions[f];
    let idx_proven = func
        .chunk
        .code
        .iter()
        .enumerate()
        .any(|(ip, op)| matches!(op, Op::Index) && acc.proven[ip]);
    assert!(
        idx_proven,
        "the for-in Index must be proven in-bounds (branch drops)"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "nested elided for-in must match the oracle"
    );
}

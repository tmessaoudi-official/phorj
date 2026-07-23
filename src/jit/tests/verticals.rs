//! Delivery-path + handle-vertical tests: the `phg run` hook, string/list/map verticals,
//! fault parity through code-5 redo.

use super::*;

#[test]
fn phg_run_hook_actually_hits_the_jit() {
    // A silent 100%-fallback to the VM would pass every byte-identity check identically and prove
    // nothing — so this asserts the hit counter is non-zero, i.e. the native path genuinely ran.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function fib(int n) -> int { if (n < 2) { return n; } return fib(n - 1) + fib(n - 2); }
\
        #[Entry] function main() -> void { Output.printLine(\"{fib(10)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(int iters) -> int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc;\n\
        }\n\
        #[Entry] function main() -> void { Output.printLine(\"{bench(1000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
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
fn phg_run_hook_hits_the_jit_on_the_native_bridge2_and_str_eq() {
    // The generic pure-native bridge (join / contains / splitOnce / drop route through the
    // REGISTERED natives — single-sourced kernels) + string `==`/`!=` via the `eq_val` helper.
    // Every result folds into the checksum; hits > 0 + byte-identity.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"alpha\", \"beta\", \"gamma\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string joined = String.join(parts, \", \");\n\
            acc = acc + String.length(joined);\n\
            if (String.contains(joined, \"beta\")) {\n\
              acc = acc + 1;\n\
            }\n\
            List<string> pair = String.splitOnce(joined, \", \");\n\
            acc = acc + List.length(pair);\n\
            List<string> rest = List.drop(parts, 1);\n\
            acc = acc + List.length(rest);\n\
            string head = parts[i % 3];\n\
            if (head == \"beta\") {\n\
              acc = acc + 10;\n\
            }\n\
            if (head != \"gamma\") {\n\
              acc = acc + 100;\n\
            }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "bridge2/str-eq jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual bridge2/str-eq must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the bridge2/str-eq shapes must actually hit the JIT"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_handle_args_and_builder_returns() {
    // W-slices 3+4: handle ARGS move across calls/methods (a fresh list literal and a str
    // const into a free fn; a str arg into a METHOD), and the builder-method return shape —
    // `this` (an Inst param) in, a FRESH Owned instance out (the relaxed transfer gate: an
    // Owned Inst provably comes from the callee's own MakeInstance). hits > 0 + byte-identity.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        class Counter {\n\
          constructor(public int n, public string tag) {}\n\
          function bumped(string t): Counter {\n\
            return new Counter(this.n + 1, t);\n\
          }\n\
        }\n\
        function rowOf(List<string> parts, string sep): string {\n\
          return String.join(parts, sep);\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            string row = rowOf([\"a\", \"b\", \"c\"], \"-\");\n\
            acc = acc + String.length(row);\n\
            Counter c0 = new Counter(i, \"x\");\n\
            Counter c1 = c0.bumped(\"y\");\n\
            acc = acc + c1.n;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "handle-args/builder-return jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual handle-args/builder-return must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the handle-args/builder-return shapes must actually hit the JIT"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_bool_consts_and_to_string() {
    // Bool consts (`mutable bool flag = true`) + `Conversion.toString(int)` (the interpolation
    // renderer's exact bytes) in the unboxed subset. hits > 0 + byte-identity.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.String;\n\
        import Core.Conversion;\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            mutable bool odd = true;\n\
            if (i % 2 == 0) {\n\
              odd = false;\n\
            }\n\
            if (odd) {\n\
              acc = acc + 1;\n\
            }\n\
            string s = Conversion.toString(i * 7 - 3);\n\
            acc = acc + String.length(s);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "bool/toString jit-wired output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(manual, oracle, "manual bool/toString must match the oracle");
    assert!(
        cache.borrow().hits > 0,
        "bool/toString shapes must actually hit the JIT"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_list_fields() {
    // W-slice: HANDLE-LIST instance fields — a List<string> ctor arg MOVES into the field
    // word; GetField borrows it (List.length over the borrow); the per-iteration reassignment
    // releases the instance AND its list field (steady state across 2000 fresh instances).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        class Row {\n\
          constructor(public List<string> cols, public string name, public int n) {}\n\
          function width(): int {\n\
            return List.length(this.cols);\n\
          }\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Row r = new Row([\"a\", \"b\", \"c\"], \"t\", i);\n\
            acc = acc + r.width() + List.length(r.cols) + r.n;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "list-field jit-wired output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual list-field run must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the list-field shapes must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_wide_two_slot_instances() {
    // W8: a WIDE instance (11 fields > the single-slot 8) — fields 0..6 in slot A, A[7] =
    // the B-slot index, 7..14 in B. Mixed int/str/list fields exercise routed loads/stores
    // AND the wide release (B recycled before A) across 2000 fresh instances. The high-index
    // fields (8, 9, 10) live in slot B — reading them proves the two-hop addressing.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        class Wide {\n\
          constructor(\n\
            public int a, public int b, public int c, public int d,\n\
            public int e, public int f, public int g, public string h,\n\
            public int i, public List<string> j, public string k\n\
          ) {}\n\
          function tail(): int {\n\
            return this.i + List.length(this.j) + String.length(this.k);\n\
          }\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int n = 0;\n\
          while (n < iters) {\n\
            Wide w = new Wide(n, 2, 3, 4, 5, 6, 7, \"hi\", n + 8, [\"x\", \"y\"], \"tail\");\n\
            acc = acc + w.a + w.g + String.length(w.h) + w.i + w.tail();\n\
            n = n + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "wide-instance jit-wired output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual wide-instance run must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "wide instances must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_union_dyn_params() {
    // W7: union Dyn cells — `add`'s value param sees Int, Str AND Bool call sites (a
    // genuine scalar-family disagreement → the tagged two-word Dyn), each appended to a
    // `List<union>` field via the tag-dispatched helper (binds starts as the flat-empty
    // literal and the list-family join refines it to DynList). The per-iteration chain
    // frees OWNED temp receivers WITH their DynList fields (steady state across 2000
    // iterations — the sqlbuild builder shape end to end). No float arm here: a float
    // CONST in a calling function still trips the v1 "float subset is leaf-only" gate
    // (a separate lever); the Dyn float tag (1) is wired and waits on that gate.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        class Q {\n\
          constructor(private List<string | int | float | bool> binds, public int n) {}\n\
          function add(string | int | float | bool v): Q {\n\
            return new Q(List.append(this.binds, v), this.n + 1);\n\
          }\n\
          function size(): int { return List.length(this.binds); }\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            Q q = new Q(new List<string | int | float | bool>(), 0).add(i).add(\"paid\").add(true);\n\
            acc = acc + q.size() + q.n;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "union-dyn jit-wired output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual union-dyn run ok");
    assert_eq!(manual, oracle, "manual union-dyn run must match the oracle");
    assert!(
        cache.borrow().hits > 0,
        "union Dyn params must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

#[test]
fn phg_run_hook_takes_list_fields_from_dying_temp_receivers() {
    // Regression (W7 audit): a LIST field read off a DYING owned temp (`new P(..).cols`)
    // TAKES the word — the receiver's field-release walk must EXCLUDE that slot (it used
    // to exclude Str fields only, so the taken list word was freed under the reader:
    // recycled-slot reuse could hand the consumer a different live value — wrong bytes,
    // not just a redo). Steady state over 2000 temps proves take + skip + no leak.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        class P {\n\
          constructor(public List<string> cols, public string tag) {}\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + List.length(new P([\"a\", \"b\"], \"t\").cols) + i;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "dying-temp list-field output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual dying-temp run ok");
    assert_eq!(
        manual, oracle,
        "manual dying-temp list-field run must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the dying-temp list-field take must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

#[test]
fn iterated_local_also_written_declines_to_the_vm_byte_identically() {
    // The MUTATION GUARD: iterating a local AND writing it in the same function (append
    // during iteration — the VM's for-in iterates a SNAPSHOT; a JIT ACL append/reseed would
    // mutate or recycle the record IN PLACE under the walker). The whole function must
    // decline (fall back to the VM) and stay byte-identical — snapshot semantics preserved.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(50)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(64)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(5)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(1000)}\"); }";
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
fn phg_run_hook_hits_the_jit_on_the_maphas_vertical() {
    // Maphas-vertical DELIVERY-PATH proof: the exact `bench/micro/maphas.phg` shape — a `MakeMap`
    // of short string keys (seals FLAT), a flat probe list, and `Map.has(m, probes[i % 6])` in an
    // `if` (the inline bucket probe returning a Bool). Two of the six probes ("e"/"f") MISS — those
    // exercise the NEW fast-path empty-bucket→false codegen that has no precedent in mapget (which
    // faults there). Must JIT through the `Op::Call` hook AND stay byte-identical to the oracle.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"a\" => 10, \"b\" => 20, \"c\" => 30, \"d\" => 40];\n\
          List<string> probes = [\"a\", \"b\", \"c\", \"d\", \"e\", \"f\"];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (Map.has(m, probes[i % 6])) { acc = acc + 1; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1200)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "maphas-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual maphas-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the maphas vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

#[test]
fn maphas_vertical_slow_path_canon0_key_matches_the_oracle() {
    // The maphas SLOW path: a canon-0 key (an inline-`+` concat result, never content-registered)
    // punts from the fast probe to `rt_u_map_has`. Both a HIT ("a"+"b" ⇒ present) and a clean MISS
    // ("x"+"y" ⇒ absent) must be byte-identical to the oracle — the miss exercises the helper's
    // `present:0, code:0` clean-false answer (not a code-5 redo). Also proves the fast path is not
    // silently the only correct answer.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Map;\n\
        function bench(int iters): int {\n\
          Map<string, int> m = [\"ab\" => 1, \"cd\" => 2];\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (Map.has(m, \"a\" + \"b\")) { acc = acc + 1; }\n\
            if (Map.has(m, \"x\" + \"y\")) { acc = acc + 100; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(50)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "maphas slow-path (canon-0 key) output must match the interpreter oracle"
    );
    // Prove the function actually JITs — a canon-0 key routes to `slow_blk` at runtime, so a
    // non-zero hit count means `rt_u_map_has` genuinely ran (a silent VM fallback would false-green
    // the byte-identity assert above and leave the slow helper unexercised).
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual maphas slow-path output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the maphas slow-path function must actually hit the JIT — else rt_u_map_has is unproven"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_the_setcontains_vertical() {
    // Setcontains-vertical DELIVERY-PATH proof: the exact `bench/micro/setcontains.phg` shape — a
    // `Set.of([int literals])` (MakeList → flat int block, re-tagged to an IntSet) and
    // `Set.contains(s, i % 16)` in a hot `while` (the inline linear membership scan). The needle
    // `i % 16` (0..15) both HITS (3,1,4,5,9,2,6 present) and MISSES (0,7,8,10..15 absent) across
    // iterations — the miss exercises the exhausted-scan→CLEAN-false codegen. Must JIT through the
    // `Op::Call` hook AND stay byte-identical to the interpreter oracle. A silent VM fallback would
    // false-green the byte-identity assert, so `hits>0` is the load-bearing check (proves the flip).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> s = Set.of([3, 1, 4, 1, 5, 9, 2, 6]);\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            if (Set.contains(s, i % 16)) { acc = acc + 1; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(1600)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "setcontains-vertical jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual setcontains-vertical jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the setcontains vertical must actually hit the JIT — else the perf flip is unproven"
    );
}

/// FORK-D edge coverage: 4-leg byte-identity (interp ≡ JIT ≡ pure-VM) + `hits>0`, asserting the
/// three cases the int-hash table's `{occupied, key}` layout must get right and that a naive
/// key-0-is-empty scheme would break:
///  * **needle 0 as a MEMBER** — the set CONTAINS 0; a probe for 0 must HIT (occupancy-first is what
///    stops an empty bucket's zero key-word from being mistaken for the value 0, and vice-versa).
///  * **needle 0 as ABSENT** — a different set that does NOT contain 0; a probe for 0 must miss clean.
///  * **duplicate literals** — `Set.of` dedups; the table is sized on the DISTINCT count.
///  * **collisions / wraparound** — a larger set (> tsize/2 after the power-of-two round) exercises
///    the open-addressed linear-probe walk on both hits and misses.
#[test]
fn jit_setcontains_zero_dedup_and_collision_edges_match_the_oracle() {
    // s0 CONTAINS 0 (and negatives); s1 does NOT. The needle `i % 5` sweeps {0,1,2,3,4} so 0 is
    // probed against BOTH sets every 5 iterations. Duplicate literals (0,0 / 7,7) prove dedup. The
    // 10-distinct-element s0 rounds to a 32-bucket table with collisions across the walk.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.Set;\n\
        function bench(int iters): int {\n\
          Set<int> s0 = Set.of([0, 0, 7, 7, 2, 4, 9, 15, 23, 42, 0 - 5, 0 - 5, 100, 3]);\n\
          Set<int> s1 = Set.of([1, 3, 5, 8, 11, 14]);\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            int n = i % 5;\n\
            if (Set.contains(s0, n)) { acc = acc + 1; }\n\
            if (Set.contains(s1, n)) { acc = acc + 10; }\n\
            if (Set.contains(s0, 0 - 5)) { acc = acc + 100; }\n\
            if (Set.contains(s0, 999)) { acc = acc + 1000; }\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench(200)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "setcontains edge (zero/dedup/collision) jit output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    // Pure-VM (no JIT) leg — the fourth backend of the byte-identity spine.
    let vm_only = crate::vm::Vm::new(&program).run().expect("pure-VM run ok");
    assert_eq!(
        vm_only, oracle,
        "pure-VM setcontains edge output must match the oracle"
    );
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual jit setcontains edge output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the setcontains edge vertical must actually hit the JIT — else the flip/coverage is unproven"
    );
}

/// FORK-D `-1` fallback: a set with ≥ 4096 distinct elements trips `rt_u_set_seal`'s `n >= 1<<12`
/// guard → the seal returns `-1` → `Set.of` faults code 5 → the WHOLE call redoes on the VM (which
/// builds a real `Value::Set`). This is NOT a "JIT-stays-and-punts" slow path (there is no such path —
/// a non-flat set can't reach `arm_setcontains`), so `hits>0` is not asserted here; the point is that
/// the abort-to-VM fallback produces byte-identical output. Membership over the large set must still be
/// correct on the jit-wired path (via the redo).
#[test]
fn jit_setcontains_oversized_set_falls_back_to_vm_and_matches_the_oracle() {
    let elems: String = (0..4100)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let src = String::from(
        "package Main; import Core.Runtime.Entry;\n\
         import Core.Output;\n\
         import Core.Set;\n\
         function f(): int {\n\
           Set<int> s = Set.of([",
    ) + &elems
        + "]);\n\
           mutable int acc = 0;\n\
           if (Set.contains(s, 0)) { acc = acc + 1; }\n\
           if (Set.contains(s, 4099)) { acc = acc + 1; }\n\
           if (Set.contains(s, 9999)) { acc = acc + 1; }\n\
           return acc;\n\
         }\n\
         #[Entry] function main(): void { Output.printLine(\"{f()}\"); }";
    let jit_out =
        crate::cli::cmd_run(&src).expect("jit-wired run ok (falls back to VM on the seal)");
    let oracle = crate::cli::cmd_treewalk(&src).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "oversized-set (-1 seal → VM redo) jit output must match the interpreter oracle"
    );
    assert!(
        jit_out.contains('2'),
        "0 and 4099 are members, 9999 is not → acc must be 2, got: {jit_out}"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_mixed_interpolation() {
    // Webish-vertical proof: mixed `Concat(n)` interpolation (`"h={v} p={p}"`) runs FULLY
    // INLINE for the hot shape — IR digit render (sign, zero, i64::MIN/MAX) + slot joins —
    // while >22-byte totals (the MIN/MAX bodies) take the fused helper: BOTH paths exercise
    // in ONE loop. The `check` map probe makes `acc` depend on the EXACT rendered bytes (a
    // wrong render misses the key and faults on the JIT leg only → outputs diverge → caught).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(600)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
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
fn phg_run_hook_hits_the_jit_on_the_chain_accumulator() {
    // Chain-vertical proof (`s = s + A + B + …`, the toQuery shape): EVERY concat in the
    // left-spine appends in place on the same ACC record — the first link consumes the
    // slot's borrow, mid links carry the record, the last link fuses the store. An Int
    // operand mid-chain renders through the interpolation decimal renderer (as_display
    // bytes). The map probe pins the exact accumulated bytes early (byte-identity through
    // the chain), and the length-fold covers every statement thereafter. Before the chain
    // arm this shape leaked one builder record per statement (the accumulator_site
    // positional hole) and re-boxed the WHOLE accumulated string per statement.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(int iters): int {\n\
          List<string> parts = [\"alpha\", \"beta\", \"gamma\", \"delta\"];\n\
          Map<string, int> check = [\"alpha-1|beta-2|\" => 3, \"alpha-1|beta-2|gamma-3|\" => 5];\n\
          mutable string s = \"\";\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            s = s + parts[i % 4] + \"-{i % 4 + 1}\" + \"|\";\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "chain-accumulator jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(
        manual, oracle,
        "manual chain-accumulator jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "the chain accumulator must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

#[test]
fn jit_map_vertical_long_key_stays_boxed_and_matches_the_oracle() {
    // A >22-byte key defeats flattening: the seal falls back to a boxed `Value::Map` and every
    // lookup routes through the helper into the canonical `map_index` kernel. Byte-identity must
    // hold on that path too (long AND short keys mixed — the short one also stays boxed here,
    // exercising the helper's slot-key + boxed-map combination).
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(64)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "boxed-map lookup must match the oracle");
}

#[test]
fn jit_map_vertical_duplicate_keys_dedup_like_the_kernel() {
    // Duplicate literal keys are legal (checker only type-checks them): `build_map`'s PHP
    // semantics — FIRST position, LAST value — must survive the flat seal. `m[\"a\"]` must read 2,
    // never 1, on all backends.
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          Map<string, int> m = [\"a\" => 1, \"b\" => 5, \"a\" => 2];\n\
          return m[\"a\"] * 100 + m[\"b\"];\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(240)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
        import Core.Output;\n\
        function bench(): int {\n\
          Map<string, int> m = [\"a\" => 10];\n\
          return m[\"zzz\"];\n\
        }\n\
        #[Entry] function main(): void { Output.printLine(\"{bench()}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry;\n\
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
        #[Entry] function main(): void { Output.printLine(\"{bench(64)}\"); }";
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
            "package Main; import Core.Runtime.Entry;\n\
             import Core.Output;\n\
             function countdown(int n) -> int {{ if (n <= 0) {{ return 0; }} return countdown(n - 1); }}\n\
             #[Entry] function main() -> void {{ Output.printLine(\"{{countdown({n})}}\"); }}"
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

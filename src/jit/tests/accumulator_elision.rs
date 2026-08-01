//! JIT Task-9 accumulator overflow-check-elision (the interval pass) + for-in + str-list accumulator
//! tests — structural proofs, byte-identity on the elided code, the guard-decline path, a genuine
//! overflow fault. Split from the `verticals.rs` monolith by cohesion (Invariant 13, M-Decomp).

use super::*;

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
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
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
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(3000)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { acc = acc + 5000000000000; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(1048577)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int { mutable int acc = 9000000000000000000; mutable int i = 0;\n\
          while (i < iters) { acc = acc + 20000000000000; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(20000)}\"); }";
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
fn task9_rejects_a_computed_bound_but_now_proves_a_conditional_accumulator() {
    // A COMPUTED loop bound (not a param, not a const) stays out of scope — the trip count is what the
    // envelope is solved against, so an unknown bound has nothing to solve with.
    //
    // A body-level `if`, however, is now IN scope (DEC-425). It used to be refused, and that refusal was
    // why a CONDITIONAL accumulator — "tally while scanning", the commonest shape there is — left a
    // loop-carried sticky overflow phi on every iteration. Soundness comes from the envelope solve,
    // which already takes `min(growth.lo, 0)` / `max(growth.hi, 0)` per site: it has ALWAYS assumed a
    // site may or may not run, which is exactly what a conditional site is.
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function computed(int n): int { int lim = n * 2; mutable int acc = 0; mutable int i = 0;\n\
          while (i < lim) { acc = acc + 1; i = i + 1; } return acc; }\n\
        function branchy(int n): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { if (i > 2) { acc = acc + 2; } i = i + 1; } return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{computed(5)} {branchy(9)}\"); }";
    let program = compile_source(SRC);
    assert!(
        acc_elision(&program, "computed").is_none(),
        "computed bound is still out of scope"
    );
    let branchy = acc_elision(&program, "branchy").expect("a conditional accumulator now proves");
    assert!(
        branchy.proven.iter().filter(|&&p| p).count() >= 2,
        "both the conditional site and the counter increment must prove"
    );
    assert!(
        !branchy.guards.is_empty(),
        "the param bound must carry an entry guard (decline to the VM above it)"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "elided code must match the oracle exactly");
}

/// THE load-bearing test for the conditional-accumulator extension: the elided code must produce the
/// SAME NUMBERS as the interpreter oracle across a spread of trip counts, including 0 and the values
/// either side of the branch condition.
///
/// It is load-bearing because it BITES. Verified by deleting the join-widening (the step that marks
/// every slot a conditional region may have written as unknown at the merge): this test then fails
/// with a wrong tally rather than a crash, which is exactly the failure mode an unsound widening
/// produces. The interval-vs-oracle pair is what catches it; a structural `is_some()` assertion alone
/// would not have.
#[test]
fn task9_conditional_accumulator_is_byte_identical_across_trip_counts() {
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function tally(int n): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { if (i % 3 == 0) { acc = acc + 7; } i = i + 1; } return acc; }\n\
        function withlocal(int n): int { mutable int acc = 0; mutable int t = 0; mutable int i = 0;\n\
          while (i < n) { if (i > 1) { t = i * 2; acc = acc + 1; } i = i + 1; } return acc + t; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
          Output.printLine(\"{tally(0)} {tally(1)} {tally(2)} {tally(3)} {tally(4)} {tally(100)}\");\n\
          Output.printLine(\"{withlocal(0)} {withlocal(2)} {withlocal(3)} {withlocal(10)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle);
    // Pinned literally: a wrong tally is the symptom of a bad region merge, and comparing only against
    // the oracle would miss a bug that broke BOTH legs the same way.
    assert_eq!(
        jit_out.trim(),
        "0 7 7 7 14 238\n0 0 5 26",
        "the tallies themselves must be right"
    );
}

/// THE soundness test for the join-widening — the one step of the conditional extension that can
/// silently produce WRONG ARITHMETIC rather than a crash.
///
/// `t` starts enormous and the conditional assigns it `1`. Without widening at the merge, the analysis
/// would carry the THEN-branch's interval (`t ∈ [1,1]`) past the join and conclude that `acc = acc + t`
/// grows by at most 1 per iteration — proving the elision and dropping the overflow check. On the ELSE
/// path `t` is still 5e18, so `acc` overflows immediately and nothing detects it. Widening every
/// maybe-written slot to unknown at the join is what makes the growth unbounded and forces a refusal.
///
/// Verified to BITE: deleting the widening makes this test fail. It is the only test that does, which is
/// why it is spelled out rather than folded into the fail-closed group.
#[test]
fn task9_join_widening_prevents_a_stale_then_branch_interval() {
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function staleiv(int n): int { mutable int acc = 0; mutable int t = 5000000000000000000;\n\
          mutable int i = 0;\n\
          while (i < n) { if (i > 1) { t = 1; } acc = acc + t; i = i + 1; } return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{staleiv(1)}\"); }";
    let program = compile_source(SRC);
    assert!(
        acc_elision(&program, "staleiv").is_none(),
        "a slot conditionally overwritten must be UNKNOWN after the join — carrying the then-branch \
         interval would elide a real overflow check"
    );
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle);
}

/// Shapes that must fail CLOSED (keep their overflow checks) and stay byte-identical.
///
/// Honest scope note: each of these is refused by a check that predates this extension (the
/// single-writer counter rule, the i64-fit rule), NOT by the two guards the extension added — verified
/// by deleting each guard and re-running, which changed nothing. Those two guards
/// (nested-region refusal, conditional counter write) are therefore DEFENSIVE and currently
/// unreachable; they are kept because the earlier checks are not obviously sufficient forever, and
/// removing them would make a future widening silently unsound. This test pins the BEHAVIOUR
/// (fail-closed + oracle parity), which is what actually matters here.
#[test]
fn task9_out_of_scope_conditional_shapes_fail_closed() {
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function nested(int n): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { if (i > 2) { if (i > 4) { acc = acc + 1; } } i = i + 1; } return acc; }\n\
        function condcounter(int n): int { mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { acc = acc + 1; if (acc > 0) { i = i + 1; } } return acc; }\n\
        function huge(int n): int { mutable int acc = 9000000000000000000; mutable int i = 0;\n\
          while (i < n) { if (i > 2) { acc = acc + 20000000000000; } i = i + 1; } return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
          Output.printLine(\"{nested(9)} {condcounter(5)}\"); }";
    let program = compile_source(SRC);
    for name in ["nested", "condcounter", "huge"] {
        assert!(
            acc_elision(&program, name).is_none(),
            "`{name}` must fail closed (keep its overflow check)"
        );
    }
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "refused shapes must still match the oracle"
    );
}

#[test]
fn phg_run_hook_hits_the_jit_on_for_in_iteration() {
    // Forin-vertical proof: `for (x in xs)` desugars to `IterElems` + an indexed while over
    // `Len` — a BORROWED flat list handle IS its element snapshot (identity, zero
    // instructions) and `Len` reads the count from the handle bits. Covers int-list AND
    // str-list iteration, byte-identity, and hits>0.
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
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
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(500)}\"); }";
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
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
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
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(700)}\"); }";
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

#[test]
fn phg_run_hook_hits_the_jit_on_str_list_accumulators() {
    // L2a: the STR-list ACL accumulator builder — the qualify-loop shape
    // (`out = List.append(out, q)` where q is a fresh OWNED string) consumes each element
    // WORD into a str-word record (zero clones), materializes through List.drop + join,
    // and the record + its owned words release at steady state across 2000 iterations.
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        import Core.List;\n\
        import Core.String;\n\
        function qualify(string col, string alias): string {\n\
          return alias + \".\" + col;\n\
        }\n\
        function bench(int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            List<string> cols = [\"id\", \"name\", \"total\"];\n\
            mutable List<string> out = [\"\"];\n\
            mutable int j = 0;\n\
            while (j < List.length(cols)) {\n\
              string q = qualify(cols[j], \"u\");\n\
              out = List.append(out, q);\n\
              j = j + 1;\n\
            }\n\
            List<string> done = List.drop(out, 1);\n\
            acc = acc + String.length(String.join(done, \", \")) + i;\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(2000)}\"); }";
    let jit_out = crate::cli::cmd_run(SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "str-accumulator jit-wired output must match the oracle"
    );
    let program = compile_source(SRC);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual str-accumulator run ok");
    assert_eq!(
        manual, oracle,
        "manual str-accumulator run must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "str-list accumulators must actually hit the JIT (redos = {})",
        cache.borrow().redos
    );
}

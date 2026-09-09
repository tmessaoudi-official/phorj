//! Why the JIT declines — the subset gaps behind DEC-431's ~320x `throws` cliff, pinned.
//!
//! These are RATCHETS on a known limitation, not assertions that it is desirable. A hot loop in a
//! function that declares `throws` is interpreted, and this file records the exact reasons so that
//! (a) the mechanism cannot be mis-stated again — it already was, twice, when the reason was simply
//! discarded by `.ok()` at the call site — and (b) whoever fixes it is told by a failing test.
//!
//! If one of these starts COMPILING, that is progress: update the test, do not delete it.

use super::*;

const THROWS_SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    import Core.FileSystemModule.FileSystem;\n\
    import Core.FileSystemModule.FileSystemError;\n\
    function work(int n): int throws FileSystemError {\n\
      mutable int acc = 0; mutable int i = 0;\n\
      while (i < n) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
      FileSystem.writeText(\"/tmp/phorj-decline-test.txt\", \"x\")?;\n\
      return acc; }\n\
    #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
      try { Output.printLine(\"{work(10)}\"); } catch (FileSystemError e) { Output.printLine(\"e\"); } }";

/// The cliff itself: the loop-bearing function is declined, and the FIRST reason is its OWN
/// `Const(Unit)` — the dummy receiver the compiler pushes for a prelude-class static call — not the
/// transitive fallible callee. DEC-431 originally recorded only the transitivity, which is why this
/// asserts the specific string rather than merely `is_err()`.
#[test]
fn a_loop_in_a_throws_function_is_declined_on_its_own_dummy_receiver() {
    let program = compile_source(THROWS_SRC);
    let f = func_index(&program, "work");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect(
        "DEC-431: a `throws` function is still declined — if this now compiles, the cliff is fixed",
    );
    let msg = format!("{err:?}");
    assert!(
        msg.contains("Const") && msg.contains("Unit"),
        "expected the dummy-receiver Const(Unit) to be the first blocker, got: {msg}"
    );
}

/// The reason the fix is not "just support `Const(Unit)`": the fallible prelude method is declined
/// independently, on its own un-whitelisted `CallNative`. Both layers have to go.
#[test]
fn the_fallible_prelude_method_is_declined_independently_on_its_native() {
    let program = compile_source(THROWS_SRC);
    let f = func_index(&program, "FileSystem::writeText");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect("FileSystem::writeText is expected to be out-of-subset");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("CallNative"),
        "expected the un-whitelisted fallible native to be the blocker, got: {msg}"
    );
}

/// The control that makes the two above meaningful, and the measured workaround (DEC-431: 773.83 ms ->
/// 2.42 ms): the SAME loop compiles fine once no fallible call shares its function. Without this, the
/// tests above would pass just as well if the JIT declined everything.
#[test]
fn the_same_loop_compiles_once_the_fallible_call_is_not_in_it() {
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function work(int n): int {\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < n) { acc = acc + (i * 3 - 1); i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{work(10)}\"); }";
    let program = compile_source(SRC);
    let f = func_index(&program, "work");
    assert!(
        crate::jit::Compiled::compile_unboxed(&program, f).is_ok(),
        "the hot loop must compile when it does not share a function with a fallible call"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DEC-520 (lane L4c): the TWO-LEVEL list element read, and the boundary that is deliberately kept.
//
// These four tests were DECLINE pins until DEC-520. The lattice had a `MapList` (list-of-maps) and
// a `SetList` (list-of-sets) but no list-of-lists, so `admit_make_list` rejected the row literal at
// `Op::MakeList` — BEFORE any index executed — and since `run_unboxed` is the only production JIT
// path (there is no boxed fallback), the whole function fell to the VM while `listindex` compiled
// and won. DEC-520 adds `Kind::IntListList`: INT-ONLY inner lists, whose inner read reuses
// `IntList`'s flat encoding.
//
// They are now DELIVERY-PATH proofs. `compile_unboxed` succeeding is not enough on its own — a
// silent VM fallback would false-green a byte-identity assert — so each asserts `hits > 0` against
// the same `JitCache` the `phg run` hook uses, with the output pinned to the interpreter oracle.
// The kept boundary keeps its decline pin: a THIRD level and a list-of-STRING-lists are still
// refused at `MakeList`, and that refusal is the evidence the widening is narrow.

/// The row literal a lifted `list<array{…}>` erases to. Two levels, data-dependent index — the
/// `nestedlist` microbench shape, trimmed to 2 rows (the count is irrelevant to admission).
const NESTED_SRC: &str =
    "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    function bench(int iters): int {\n\
      List<List<int>> rows = [[1, 3], [2, 1]];\n\
      mutable int acc = 0; mutable int i = 0;\n\
      while (i < iters) { int idx = (i + acc) % 2;\n\
        acc = acc + rows[idx][1] * 2 - rows[idx][0]; i = i + 1; }\n\
      return acc; }\n\
    #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4000)}\"); }";

/// Run `src` through the JIT-wired `phg run`, the interpreter oracle and a manual VM carrying a
/// `JitCache`, asserting all three agree AND that the function actually ran NATIVELY. `hits > 0` is
/// the load-bearing half: without it every assertion here is equally satisfied by a silent decline.
fn assert_runs_natively(src: &str, what: &str) -> String {
    let jit_out = crate::cli::cmd_run(src).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(src).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "{what}: jit-wired output must match the interpreter oracle"
    );
    let program = compile_source(src);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit run ok");
    assert_eq!(
        manual, oracle,
        "{what}: manual jit output must match the oracle"
    );
    assert!(
        cache.borrow().hits > 0,
        "{what}: must actually hit the JIT — else the DEC-520 flip is unproven"
    );
    oracle
}

/// THE FIX, pinned where the decline used to be. Until DEC-520 this asserted that `compile_unboxed`
/// failed with a message naming `MakeList` and `IntList`; the `unwrap_or_else` below keeps that
/// diagnosis visible, so a REGRESSION reds by NAMING the opcode it bailed on rather than by a bare
/// `is_err()`. Aiming a fix at `Op::Index` would still change nothing — the list is admitted, or
/// rejected, when it is BUILT.
#[test]
fn a_list_of_lists_compiles_and_runs_natively() {
    let program = compile_source(NESTED_SRC);
    let f = func_index(&program, "bench");
    crate::jit::Compiled::compile_unboxed(&program, f).unwrap_or_else(|e| {
        panic!("DEC-520: a list-of-lists must compile — declined at: {e:?}");
    });
    assert_runs_natively(NESTED_SRC, "list-of-lists");
}

/// DEC-504's named tuple erases to exactly the shape above, so it must be ADMITTED for the same
/// reason and produce the same answer — the evidence that the sugar costs nothing at runtime and
/// that DEC-520 reaches the sugar without knowing about it. `nestedlist` and `namedtuplefield`
/// measured 0.030 and 0.027 before the fix; this is why those two numbers are one fact.
#[test]
fn a_named_tuple_list_compiles_identically_to_its_erased_list_of_lists() {
    const TUPLE_SRC: &str =
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<(tier: int, bp: int)> rows = [(tier: 1, bp: 3), (tier: 2, bp: 1)];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { int idx = (i + acc) % 2;\n\
            acc = acc + rows[idx].bp * 2 - rows[idx].tier; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4000)}\"); }";
    let tuple_out = assert_runs_natively(TUPLE_SRC, "named-tuple list");
    let nested_out = assert_runs_natively(NESTED_SRC, "list-of-lists");
    assert_eq!(
        tuple_out, nested_out,
        "the named tuple must compute what its erased form does — a DIFFERENT answer would mean \
         the sugar reaches the backend, contradicting DEC-504"
    );
}

/// The control that keeps the two above honest: the SAME loop over a FLAT list still compiles.
/// Without it, both would pass equally well if the JIT admitted everything. This is the 1.512x
/// `listindex` row — one index level, and `phg run` emits no decline for it at all.
#[test]
fn the_same_loop_compiles_once_the_list_is_flat() {
    const FLAT_SRC: &str =
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<int> xs = [3, 1, 4, 1];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { int idx = (i + acc) % 4; acc = acc + xs[idx]; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";
    let program = compile_source(FLAT_SRC);
    let f = func_index(&program, "bench");
    assert!(
        crate::jit::Compiled::compile_unboxed(&program, f).is_ok(),
        "the flat-list loop must compile — else the nested tests prove nothing about NESTING"
    );
}

/// The asymmetry that made this a lattice HOLE rather than "the JIT doesn't do collections": a list
/// of MAPS was admitted (`Kind::MapList`) on the same `MakeList` the list of LISTS was refused on.
/// DEC-520 added the missing sibling; the map control stays, because it is what proves the new arm
/// widened `admit_make_list` instead of loosening it.
#[test]
fn a_list_of_maps_and_a_list_of_lists_are_both_admitted() {
    const MAPS_SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          Map<string, int> m1 = [\"bp\" => 3]; Map<string, int> m2 = [\"bp\" => 1];\n\
          List<Map<string, int>> rows = [m1, m2];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { int idx = (i + acc) % 2; acc = acc + rows[idx][\"bp\"]; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";
    let program = compile_source(MAPS_SRC);
    let f = func_index(&program, "bench");
    assert!(
        crate::jit::Compiled::compile_unboxed(&program, f).is_ok(),
        "a list-of-MAPS must compile — that asymmetry against the list-of-lists WAS the finding"
    );
    let nested = compile_source(NESTED_SRC);
    let nf = func_index(&nested, "bench");
    assert!(
        crate::jit::Compiled::compile_unboxed(&nested, nf).is_ok(),
        "and the list-of-LISTS must now compile too — that is DEC-520"
    );
}

/// THE KEPT BOUNDARY. DEC-520 is INT-ONLY and ONE level deep, and that is a ruling, not an
/// accident: a third level would need the inner-word guard to discriminate a nested-list word from
/// a flat-int word (both carry `UB_TAG_FLAT` with `UB_TAG_SLOT` clear — the encodings are the
/// SAME), and a list-of-string-lists would need the element read to produce an owned `Str` handle
/// out of a bump-pinned slot. Both still decline at `MakeList`, and the message names the kind that
/// was refused — so a future widening that quietly swallows one of these reds HERE first.
#[test]
fn a_third_level_and_a_string_inner_list_still_decline_at_make_list() {
    const DEEP_SRC: &str =
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<List<List<int>>> rows = [[[1, 3]], [[2, 1]]];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { int idx = (i + acc) % 2;\n\
            acc = acc + rows[idx][0][1] * 2 - rows[idx][0][0]; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";
    const STRS_SRC: &str =
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): string {\n\
          List<List<string>> rows = [[\"a\", \"b\"], [\"c\", \"d\"]];\n\
          mutable string acc = \"\"; mutable int i = 0;\n\
          while (i < iters) { int idx = i % 2; acc = acc + rows[idx][1]; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";
    for (src, refused) in [(DEEP_SRC, "IntListList"), (STRS_SRC, "StrList")] {
        let program = compile_source(src);
        let f = func_index(&program, "bench");
        let err = crate::jit::Compiled::compile_unboxed(&program, f)
            .err()
            .unwrap_or_else(|| {
                panic!("the kept boundary must still decline — a `{refused}` element compiled")
            });
        let msg = format!("{err:?}");
        assert!(
            msg.contains("MakeList") && msg.contains(refused),
            "expected the `{refused}` element kind to be refused at MakeList, got: {msg}"
        );
    }
}

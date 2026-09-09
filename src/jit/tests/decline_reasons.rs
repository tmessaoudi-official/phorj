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
// DEC-514 / DEC-519 (lane L4c): why the TWO-LEVEL list element read is 0.03x vs php:8.5+JIT.
//
// The cliff is JIT COVERAGE, and the reason is a hole in the unboxed `Kind` lattice: it has a
// `MapList` (list-of-maps) and a `SetList` (list-of-sets), but NO list-of-lists. `admit_make_list`
// therefore rejects the row literal at `Op::MakeList` — BEFORE any index executes — and since
// `run_unboxed` is the only production JIT path (there is no boxed fallback), the whole function
// falls to the VM while `listindex` compiles and wins.
//
// These pin the mechanism so it cannot be mis-stated again. DEC-514 recorded "the JIT engages
// without closing it (7x from `--no-jit`)"; it does not engage at all, and that clause is retracted.
// If one of these starts COMPILING, that is the fix landing: update the test, do not delete it.

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
    #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";

/// THE ROOT CAUSE, pinned at its first surfacing. Asserts the specific message rather than
/// `is_err()`, because *where* it bails is the whole finding: a fix aimed at `Op::Index` would
/// change nothing, since the function is already rejected when the list is BUILT.
#[test]
fn a_list_of_lists_is_declined_at_make_list_not_at_the_index() {
    let program = compile_source(NESTED_SRC);
    let f = func_index(&program, "bench");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect(
            "DEC-514: a list-of-lists is still declined — if this compiles, the cliff is fixed",
        );
    let msg = format!("{err:?}");
    assert!(
        msg.contains("MakeList") && msg.contains("IntList"),
        "expected the list-of-lists MakeList to be the blocker (not Index), got: {msg}"
    );
}

/// DEC-504's named tuple erases to exactly the shape above, so it must decline for the SAME reason
/// — the evidence that the sugar costs nothing at runtime and the loss predates it. `nestedlist`
/// and `namedtuplefield` measure 0.030 and 0.027; this is why those two numbers are one fact.
#[test]
fn a_named_tuple_list_declines_identically_to_its_erased_list_of_lists() {
    const TUPLE_SRC: &str =
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        function bench(int iters): int {\n\
          List<(tier: int, bp: int)> rows = [(tier: 1, bp: 3), (tier: 2, bp: 1)];\n\
          mutable int acc = 0; mutable int i = 0;\n\
          while (i < iters) { int idx = (i + acc) % 2;\n\
            acc = acc + rows[idx].bp * 2 - rows[idx].tier; i = i + 1; }\n\
          return acc; }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(4)}\"); }";
    let tuple_err = {
        let p = compile_source(TUPLE_SRC);
        let f = func_index(&p, "bench");
        format!(
            "{:?}",
            crate::jit::Compiled::compile_unboxed(&p, f)
                .err()
                .expect("a named-tuple list is still declined")
        )
    };
    let nested_err = {
        let p = compile_source(NESTED_SRC);
        let f = func_index(&p, "bench");
        format!(
            "{:?}",
            crate::jit::Compiled::compile_unboxed(&p, f)
                .err()
                .expect("a list-of-lists is still declined")
        )
    };
    assert_eq!(
        tuple_err, nested_err,
        "the named tuple must decline for the SAME reason as its erased form — a DIFFERENT message \
         would mean the sugar reaches the backend, contradicting DEC-504"
    );
}

/// The control that makes the two above mean something: the SAME loop over a FLAT list compiles.
/// Without it, both would pass equally well if the JIT declined everything. This is the 1.512x
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

/// The asymmetry that makes this a lattice hole rather than "the JIT doesn't do collections": a
/// list of MAPS is admitted (`Kind::MapList`) on the same `MakeList` the list of LISTS is refused
/// on. Whoever fixes the cliff adds the missing sibling; every `Kind::MapList` site is a site it
/// needs (10 non-test code sites, plus 5 doc-comment mentions, as of 2026-09-09).
#[test]
fn a_list_of_maps_is_admitted_where_a_list_of_lists_is_not() {
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
        "a list-of-MAPS must compile — that asymmetry against the list-of-lists IS the finding"
    );
}

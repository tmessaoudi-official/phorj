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

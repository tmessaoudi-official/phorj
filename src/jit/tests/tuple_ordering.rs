//! Tuple ordering and the JIT tiers (DEC-512) — a RATCHET on where the new operand type can and
//! cannot reach.
//!
//! Admitting tuples to `< > <= >=` changed what the *boxed* JIT tier sees, not only the VM and the
//! interpreter: `Op::Lt`/`Gt`/`Le`/`Ge` are on that tier's eligibility whitelist, and its bridge
//! `rt_cmp` calls `value::compare_ord` — the same kernel the other two legs use (Invariant 4), so
//! the new `(List, List)` arm would be live there the instant it were reachable, with no second
//! copy to drift.
//!
//! It is NOT reachable today, and this file pins that rather than assuming it. The whitelist admits
//! only int/`Unit` constants plus scalar arithmetic, locals, control flow and calls — there is no
//! list-construction Op in it. Eligibility is computed over the whole reachable call graph, so a
//! program that builds a tuple anywhere in that graph is declined in full and takes the VM
//! fallback. The consequence worth stating plainly: a `hits > 0` "delivery-path proof" for tuple
//! ordering is currently impossible to write, and a test that claimed one would be measuring the
//! fallback while reporting the JIT.
//!
//! `Op::Cmp` (`<=>`) is deliberately not whitelisted either, so it declines the same way. That is
//! the conservative default for a new Op, and adding it to the whitelist — together with a
//! list-construction Op — was named here as the flip lever if a DEC-507 bench ever showed a
//! comparator-heavy loss.
//!
//! **That bench came (0.373x on `spaceshipsort`) and the lever pulled was the OTHER one.** DEC-513
//! fuses a both-sides-literal tuple comparison in the COMPILER — `Op::CmpSeq` compares the elements
//! in place and no tuple is built — so the cost was removed rather than moved to another tier. JIT
//! whitelisting was considered and rejected in that ruling. `Op::CmpSeq` is therefore not
//! whitelisted either, and a fused comparator declines on IT rather than on a construction it no
//! longer performs — which the second test below pins, since it is the reason the decline message
//! changed.
//!
//! If either of these starts COMPILING, that is progress: update the test, do not delete it, and
//! add the `hits > 0` proof that becomes possible at that point.

use super::*;

const TUPLE_CMP_SRC: &str =
    "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    function bench(int iters): int {\n\
      mutable int acc = 0;\n\
      mutable int i = 0;\n\
      while (i < iters) {\n\
        var lo = (i % 3, i % 5);\n\
        var hi = (2, 2);\n\
        if (lo < hi) { acc = acc + 1; }\n\
        i = i + 1;\n\
      }\n\
      return acc;\n\
    }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{bench(400)}\"); }\n";

/// The same loop as `TUPLE_CMP_SRC` but with the tuples written INLINE on both sides — the one
/// shape DEC-513 fuses.
const FUSED_CMP_SRC: &str =
    "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    function bench(int iters): int {\n\
      mutable int acc = 0;\n\
      mutable int i = 0;\n\
      while (i < iters) {\n\
        if ((i % 3, i % 5) < (2, 2)) { acc = acc + 1; }\n\
        i = i + 1;\n\
      }\n\
      return acc;\n\
    }\n\
    #[Entry(kind: EntryKind.Cli)]\n\
    function main(): void { Output.printLine(\"{bench(400)}\"); }\n";

/// The decline is real, and it is the tuple CONSTRUCTION that causes it — not the comparison. Pinned
/// as a specific reason rather than a bare `is_err()` so that whitelisting a list Op without
/// revisiting tuple ordering shows up as a failure here.
#[test]
fn a_tuple_comparison_loop_is_declined_on_its_construction_not_its_compare() {
    let program = compile_source(TUPLE_CMP_SRC);
    let f = func_index(&program, "bench");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect(
            "a tuple-building loop is not JIT-eligible — if it now compiles, add a hits>0 proof",
        );
    let msg = format!("{err:?}");
    assert!(
        !msg.contains("Lt"),
        "the comparison itself is whitelisted; the blocker must be the construction, got: {msg}"
    );
}

/// Whatever tier it lands on, the answer is the oracle's. This is the guarantee the fallback exists
/// to provide, and it is what a silent eligibility change could not break without being noticed.
#[test]
fn tuple_ordering_agrees_with_the_oracle_on_the_fallback_path() {
    let jit_out = crate::cli::cmd_run(TUPLE_CMP_SRC).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(TUPLE_CMP_SRC).expect("interpreter oracle ok");
    assert_eq!(
        jit_out, oracle,
        "tuple ordering must agree with the interpreter on whichever tier executes it"
    );
}

/// The FUSED shape (DEC-513): a comparator with a tuple literal on both sides no longer builds a
/// tuple at all, so its decline can no longer be attributed to construction — it declines on
/// `CmpSeq` itself. Pinned as a specific reason for the same purpose as the test above: if
/// `CmpSeq` were ever whitelisted, this fails rather than silently starting to measure a tier
/// nobody chose.
#[test]
fn a_fused_tuple_comparison_declines_on_cmpseq_not_on_construction() {
    let program = compile_source(FUSED_CMP_SRC);
    let f = func_index(&program, "bench");
    // The fusion must actually have happened, or this test would be pinning the wrong reason.
    assert!(
        program.functions[f]
            .chunk
            .code
            .iter()
            .any(|op| matches!(op, crate::chunk::Op::CmpSeq(..))),
        "expected the both-sides-literal comparison to lower to Op::CmpSeq"
    );
    assert!(
        !program.functions[f]
            .chunk
            .code
            .iter()
            .any(|op| matches!(op, crate::chunk::Op::MakeList(_))),
        "the fused form must build NO tuple — that is the whole point of DEC-513"
    );
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect("CmpSeq is not whitelisted — if it now compiles, add a hits>0 proof");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("CmpSeq"),
        "the blocker must now be CmpSeq, not a construction that no longer happens, got: {msg}"
    );
}

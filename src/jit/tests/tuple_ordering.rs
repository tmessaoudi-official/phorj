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
//! list-construction Op — is the flip lever if a DEC-507 bench ever shows a comparator-heavy loss.
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

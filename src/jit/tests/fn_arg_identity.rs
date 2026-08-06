//! A FUNCTION passed across a call boundary keeps its identity (DEC-444, track A increment 1).
//!
//! `Kind::Fn(f)` names a target INDEX, and `arm_call_value` lowers a `CallValue` to a direct call, so
//! a `CallValue` compiles only when the callee's identity is known at compile time. Passing a lambda
//! as an ARGUMENT used to destroy that: both the analyzer and the emitter refused an `Fn` argument
//! outright, so `applyTwice(function(int x) => …, i)` declined and every caller declined with it.
//!
//! The fix records `Kind::Fn(f)` in the call signature so the fixpoint's `param_over` carries the
//! identity into the callee's param slot. The runtime word is untouched — it is the same filler
//! `arm_call_value` already discards (`_fv`), so nothing is allocated, cloned or freed.
//!
//! **Why the hit counter is asserted in every positive test here.** A silent fallback to the VM
//! produces byte-identical output, so an output-only assertion passes whether or not the JIT ran and
//! proves nothing — the exact false-assurance shape `phg_run_hook_actually_hits_the_jit` exists to
//! prevent. The negative tests assert the opposite pairing: still CORRECT, and correct *because* the
//! subset failed closed rather than miscompiled.

use super::*;

/// Two call sites, two DIFFERENT lambdas, one callee — the polymorphic case that must never compile
/// to a direct call to whichever target the fixpoint happened to see first.
const TWO_TARGETS_SRC: &str =
    "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    function apply((int) => int f, int x): int { return f(x); }\n\
    function bench(int n): int {\n\
      mutable int acc = 0; mutable int i = 0;\n\
      while (i < n) {\n\
        acc = acc + apply(function(int a) => a * 2, i) + apply(function(int b) => b + 7, i);\n\
        i = i + 1; }\n\
      return acc; }\n\
    #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(50)}\"); }";

/// The DEC-443 shape: one lambda, passed as a parameter, called through the param.
const ONE_TARGET_SRC: &str =
    "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
    import Core.Output;\n\
    function applyTwice((int) => int f, int x): int { return f(f(x)); }\n\
    function bench(int n): int {\n\
      mutable int acc = 0; mutable int i = 0;\n\
      while (i < n) { acc = applyTwice(function(int x) => x * 2 + 1, i) % 1000003; i = i + 1; }\n\
      return acc; }\n\
    #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(500)}\"); }";

/// THE acceptance test for the increment: the `userhof` shape compiles natively AND agrees with the
/// tree-walking oracle (Invariant 2 — the interpreter is right by definition).
#[test]
fn a_lambda_passed_across_a_call_boundary_compiles_and_matches_the_oracle() {
    let oracle = crate::cli::cmd_treewalk(ONE_TARGET_SRC).expect("interpreter oracle ok");
    let jit_out = crate::cli::cmd_run(ONE_TARGET_SRC).expect("jit-wired run ok");
    assert_eq!(jit_out, oracle, "jit output must match the interpreter");

    let program = compile_source(ONE_TARGET_SRC);
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
        "the JIT must actually run this — a silent VM fallback false-greens the byte-identity check, \
         which is exactly how this shape hid before DEC-443 measured it"
    );
}

/// `bench` — the CALLER that passes the lambda, and the function that owns the hot loop — must
/// compile. Before the fix it declined on `handle/enum/fn argument to Call`, which is what took the
/// whole loop off the JIT: the cliff was never about the lambda alone.
#[test]
fn the_caller_that_passes_the_lambda_compiles() {
    let program = compile_source(ONE_TARGET_SRC);
    let f = func_index(&program, "bench");
    crate::jit::Compiled::compile_unboxed(&program, f)
        .unwrap_or_else(|e| panic!("`bench` must compile after DEC-444, got: {e:?}"));
}

/// …but `applyTwice` compiled STANDALONE still declines, and that is correct rather than a gap.
///
/// This pins DEC-434.2's central insight from the other direction: *a closure only has known operand
/// kinds in the context of its CALL SITE.* Compiled as its own entry, `applyTwice` has no caller, so
/// the fixpoint's `param_over` is empty, its param 0 is `Unknown`, and the `CallValue` cannot resolve
/// a target. The identity is a property of the graph it is compiled INTO — which is exactly why the
/// fix propagates it through the call signature rather than stamping it on the function.
///
/// If this ever starts compiling, something now supplies param kinds without a call site (declared-type
/// seeding would do it) — update the test, do not delete it.
#[test]
fn the_callee_alone_still_declines_because_identity_is_a_call_site_fact() {
    let program = compile_source(ONE_TARGET_SRC);
    let f = func_index(&program, "applyTwice");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect("no call site ⇒ no identity ⇒ the CallValue cannot resolve a target");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("CallValue on Unknown"),
        "expected the unresolved-target decline, got: {msg}"
    );
}

/// FAIL CLOSED: two different targets reaching one param must NOT compile to a direct call. This is
/// the test that would catch the dangerous version of this change — a miscompile here would silently
/// call the wrong lambda and still look plausible.
///
/// The mechanism is `join_kind`'s missing `Fn` arm: `Fn(a) ⊔ Fn(b)` falls to `_ => None`, so the sig
/// merge reports conflicting call argument kinds. `Fn(a) ⊔ Fn(a)` survives via the `a == b` fast path,
/// which is what makes the single-target case above work.
#[test]
fn two_different_lambdas_at_two_sites_fail_closed_rather_than_miscompiling() {
    let program = compile_source(TWO_TARGETS_SRC);
    let f = func_index(&program, "bench");
    let err = crate::jit::Compiled::compile_unboxed(&program, f)
        .err()
        .expect(
        "two different lambdas into one param must DECLINE — compiling would pick one target and \
         silently call the wrong function",
    );
    let msg = format!("{err:?}");
    assert!(
        msg.contains("conflicting call argument kinds"),
        "expected the sig-merge conflict to be the blocker, got: {msg}"
    );
}

/// …and the polymorphic program must still produce the RIGHT answer, by falling back to the VM.
/// Declining is only safe if the fallback is correct, so the decline above is not evidence on its own.
#[test]
fn the_polymorphic_program_still_runs_correctly_on_the_fallback() {
    let oracle = crate::cli::cmd_treewalk(TWO_TARGETS_SRC).expect("interpreter oracle ok");
    let jit_out = crate::cli::cmd_run(TWO_TARGETS_SRC).expect("jit-wired run ok");
    assert_eq!(
        jit_out, oracle,
        "a declined graph must still run correctly on the VM"
    );
}

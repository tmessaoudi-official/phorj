//! The VM dispatch cache (DEC-448): `func → code` cached across instructions.
//!
//! The cache is sound because a function's code slice is IMMUTABLE for the program's lifetime, so the
//! only thing that can go wrong is serving a STALE slice after the top frame changes function. These
//! tests drive exactly that: rapid alternation between two functions (a cache miss on every single
//! op), deep recursion (a cache HIT on a brand-new frame — the case a naive "invalidate on any frame
//! change" would get wrong in the other direction), and a throw unwinding across frames.
//!
//! Each asserts against the TREE-WALKER, which is the reference oracle by Invariant 2 — not against a
//! hardcoded number, so the test says "the backends agree" rather than "the answer is what I typed".
//! Both engines' dispatch loops were changed (`run_to_completion` and `run_until`), so the closure
//! case is exercised too.

/// Run one source on both backends and assert they agree, returning the shared output.
fn agree(src: &str) -> String {
    let vm = phorj::cli::cmd_run(src).expect("vm run ok");
    let oracle = phorj::cli::cmd_treewalk(src).expect("tree-walker oracle ok");
    assert_eq!(vm, oracle, "VM and tree-walker must agree");
    vm
}

/// MUTUAL RECURSION — the worst case for the cache: the top frame's function changes on essentially
/// every call and return, so the cached slice is wrong more often than it is right. A cache that
/// failed to re-check `func` would execute one function's bytecode against another's frame.
#[test]
fn mutually_recursive_functions_thrash_the_dispatch_cache_correctly() {
    let out = agree(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output;\n\
         function isEven(int n): bool { if (n == 0) { return true; } return isOdd(n - 1); }\n\
         function isOdd(int n): bool { if (n == 0) { return false; } return isEven(n - 1); }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
           Output.printLine(\"{isEven(101)} {isOdd(101)} {isEven(200)}\"); }",
    );
    assert_eq!(out.trim(), "false true true");
}

/// DEEP RECURSION — a new frame whose `func` equals the cached one, so the cache legitimately HITS on
/// a frame it has never seen. Sound only because the slice depends on the function, not the frame;
/// this pins that reasoning rather than leaving it as a comment.
#[test]
fn deep_self_recursion_reuses_the_cached_slice_across_new_frames() {
    let out = agree(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output;\n\
         function sum(int n): int { if (n == 0) { return 0; } return n + sum(n - 1); }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{sum(500)}\"); }",
    );
    assert_eq!(out.trim(), "125250");
}

/// A THROW unwinding across frames. `unwind_throw` moves the top frame AND rewrites its `ip`, so the
/// cache must re-derive the slice for whatever frame it lands on — and the pre-incremented `ip` must
/// not leak into the catch landing pad.
#[test]
fn a_throw_unwinding_across_frames_lands_on_the_right_code() {
    let out = agree(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output; import Core.ErrorModule.RuntimeError;\n\
         function deep(int n): int throws RuntimeError {\n\
           if (n == 0) { throw new RuntimeError(\"bottom\"); }\n\
           return deep(n - 1)?; }\n\
         function mid(int n): int throws RuntimeError { return deep(n)?; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
           try { Output.printLine(\"{mid(20)}\"); }\n\
           catch (RuntimeError e) { Output.printLine(\"caught {e.message}\"); } }",
    );
    assert_eq!(out.trim(), "caught bottom");
}

/// The CLOSURE path — `run_until`, the second loop changed. A higher-order native drives a closure
/// re-entrantly per element, and the closure's body is a different function from its caller, so this
/// alternates the cached slice once per element.
#[test]
fn a_closure_driven_per_element_alternates_the_cache_correctly() {
    let out = agree(
        "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
         import Core.Output; import Core.List;\n\
         function twice(int x): int { return x * 2; }\n\
         #[Entry(kind: EntryKind.Cli)] function main(): void {\n\
           List<int> xs = [1, 2, 3, 4, 5];\n\
           List<int> ys = List.map(xs, function(int x) => twice(x) + 1);\n\
           Output.printLine(\"{List.sum(ys)}\"); }",
    );
    assert_eq!(out.trim(), "35");
}

//! DEC-333 Json-ADT JIT slice tests. Built increment-by-increment; this file OPENS with the
//! ENTRY STR-PARAM ABI — the `deepjson` prerequisite. `bench(string doc, int iters)` reaches the
//! compiled body with `doc` marshalled into a FRESH untagged `UbCtx` handle (allocated past
//! `n_pinned`, reclaimed by the next entry's `reset_for_run`); the body borrows it and never
//! releases it (the entry str param compiles `Str(Borrowed)`). The Json-VALUE arms (MakeEnum /
//! MatchTag / parse / map_get / …) land in later increments and add their tests here.

use super::*;

/// The delivery-path assertion the sibling vertical tests use: the JIT-wired run and the manual
/// hook run both match the interpreter oracle, the JIT was actually HIT, and NOTHING redid on the
/// VM (a redo would mean the shape silently fell back — the marshal unproven). Returns the stdout.
fn assert_jit_hits(src: &str, label: &str) -> String {
    let jit_out = crate::cli::cmd_run(src).expect("jit-wired run ok");
    let oracle = crate::cli::cmd_treewalk(src).expect("interpreter oracle ok");
    assert_eq!(jit_out, oracle, "{label}: jit output must match the oracle");
    let program = compile_source(src);
    let cache = std::rc::Rc::new(std::cell::RefCell::new(crate::vm::JitCache::new()));
    let manual = crate::vm::Vm::new(&program)
        .with_jit(cache.clone())
        .run()
        .expect("manual jit-wired run ok");
    assert_eq!(manual, oracle, "{label}: manual jit output must match");
    assert!(
        cache.borrow().hits > 0,
        "{label}: must actually hit the JIT — else the entry marshal is unproven"
    );
    assert_eq!(
        cache.borrow().redos,
        0,
        "{label}: must not redo on the VM (the entry marshal must carry the whole body natively)"
    );
    jit_out
}

#[test]
fn jit_entry_string_param_marshals_and_hits() {
    // The `deepjson` entry SHAPE minus the Json body: `bench(string doc, int iters)`. The str arg
    // is marshalled into a fresh untagged ctx handle at `run_unboxed`; the loop body reads it via
    // the untagged-safe `String.length` slow path. A loop-containing entry compiles EAGERLY on the
    // first call, so the single `bench(...)` from `main` hits the JIT with a REAL `Value::Str` arg.
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(string doc, int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + String.length(doc);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(\\\"hello, marshalled world\\\", 4)}\"); }";
    let out = assert_jit_hits(SRC, "entry str-param marshal");
    // "hello, marshalled world" = 23 bytes (past the 22-byte inline cap, but an entry arg is an
    // untagged handle regardless of length) × 4 iterations = 92.
    assert_eq!(out.trim(), "92", "entry str-param marshal value");
}

#[test]
fn jit_entry_string_param_empty_and_length_edges() {
    // Empty string (len 0) and a >22-byte string, both as entry args — parity + hits, no redo.
    const SRC: &str = "package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;\n\
        import Core.Output;\n\
        import Core.String;\n\
        function bench(string a, string b, int iters): int {\n\
          mutable int acc = 0;\n\
          mutable int i = 0;\n\
          while (i < iters) {\n\
            acc = acc + String.length(a) + String.length(b);\n\
            i = i + 1;\n\
          }\n\
          return acc;\n\
        }\n\
        #[Entry(kind: EntryKind.Cli)] function main(): void { Output.printLine(\"{bench(\\\"\\\", \\\"a fairly long string well past twenty-two bytes\\\", 3)}\"); }";
    assert_jit_hits(SRC, "entry str-param empty + long");
}

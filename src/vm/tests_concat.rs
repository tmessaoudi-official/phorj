//! DEC-431 B — the `s = s + x` accumulator peephole in `Op::Concat`: proven to FIRE by count, and
//! proven to DECLINE when the accumulator is aliased.
use super::tests::compile_source;
use super::*;

// ── DEC-431 B: the `s = s + x` accumulator appends IN PLACE on the VM ────────────────────────────

/// The idiom must take the in-place path — asserted by COUNT, not by wall-clock: a fast path that
/// silently never fires reads exactly like a working one on a green suite.
#[test]
fn accumulator_append_runs_in_place_and_counts() {
    let program = compile_source(
        "package Main; import Core.Output; import Core.String; \
         import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
         #[Entry(kind: EntryKind.Cli)] function main() -> void { mutable string s = \"\"; mutable int i = 0; \
         while (i < 300) { s = s + \"line {i}\\n\"; i = i + 1; } \
         Output.printLine(\"{String.length(s)}\"); }",
    );
    let (out, _exit, in_place) = Vm::new(&program).run_main_counting().unwrap();
    assert_eq!(out, "2590\n");
    // The first appends are inline-sized (no heap `Rc` yet) and must decline; once the string is on
    // the heap every iteration qualifies. 300 lines minus the inline prefix is well over 250.
    assert!(in_place >= 250, "in-place appends = {in_place}");
}

/// The cases where the shortcut must DECLINE and fall through to the copying concat: a
/// self-append (the right operand holds a third reference) and an accumulator whose earlier value is
/// still held by another local. Correctness is the assertion; the count is allowed to be anything.
#[test]
fn accumulator_append_declines_when_aliased() {
    let program = compile_source(
        "package Main; import Core.Output; \
         import Core.Runtime.Entry; import Core.Runtime.EntryKind; \
         #[Entry(kind: EntryKind.Cli)] function main() -> void { \
           mutable string s = \"abcdefghijklmnopqrstuvwxyz0123456789\"; s = s + s; s = s + s; \
           mutable string a = \"abcdefghijklmnopqrstuvwxyz0123456789\"; string kept = a; \
           a = a + \"!\"; a = a + \"?\"; \
           Output.printLine(\"{s}|{a}|{kept}\"); }",
    );
    let (out, _exit, _) = Vm::new(&program).run_main_counting().unwrap();
    let base = "abcdefghijklmnopqrstuvwxyz0123456789";
    assert_eq!(out, format!("{0}{0}{0}{0}|{0}!?|{0}\n", base));
}

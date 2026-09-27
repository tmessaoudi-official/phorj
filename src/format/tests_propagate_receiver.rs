//! Row 4j1 — a propagated call used as a RECEIVER keeps its parentheses.
//!
//! The lexer munches `?.` into the safe-navigation token, so `(fold(s)?).length()` printed as
//! `fold(s)?.length()` is a DIFFERENT program: no propagation at all (`E-CALL-UNHANDLED`), and a
//! nullable receiver where there was none. Idempotence cannot see it — the wrong output is a fixed
//! point — so these assert that meaning is preserved, on the single-link and the chain layout.

use super::tests::{assert_idempotent, assert_meaning_preserved, fmt};

const PRELUDE: &str = "package Main;\nimport Core.Output;\nimport Core.Runtime.Entry;\nimport Core.Runtime.EntryKind;\n\
    import Core.ErrorModule.RuntimeError;\nimport Core.String;\nclass BadError extends RuntimeError {}\n\
    function fold(string s): string throws BadError { if (s == \"\") { throw new BadError(\"empty\"); } return s; }\n";

#[test]
fn a_propagated_receiver_keeps_its_parentheses() {
    let src = format!(
        "{PRELUDE}function size(string s): int throws BadError {{ return (fold(s)?).length(); }}\n\
         #[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ try {{ Output.printLine(\"{{size(\\\"abc\\\")}}\"); \
         Output.printLine(\"{{size(\\\"\\\")}}\"); }} catch (BadError e) {{ Output.printLine(\"caught\"); }} }}\n"
    );
    let out = fmt(&src);
    assert!(out.contains("(fold(s)?).length()"), "{out}");
    assert_idempotent(&src);
    assert_meaning_preserved(&src);
}

#[test]
fn a_propagated_receiver_in_a_call_chain_keeps_its_parentheses() {
    let src = format!(
        "{PRELUDE}function size(string s): int throws BadError {{ return (fold(s)?).trim().length(); }}\n\
         #[Entry(kind: EntryKind.Cli)]\nfunction main(): void {{ try {{ Output.printLine(\"{{size(\\\" ab \\\")}}\"); \
         Output.printLine(\"{{size(\\\"\\\")}}\"); }} catch (BadError e) {{ Output.printLine(\"caught\"); }} }}\n"
    );
    let out = fmt(&src);
    assert!(out.contains("(fold(s)?).trim().length()"), "{out}");
    assert_idempotent(&src);
    assert_meaning_preserved(&src);
}

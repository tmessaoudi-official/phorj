//! Tests for [`super::walk`] — split out of `walk.rs` (Invariant 13, M-Decomp).
//!
//! `walk.rs` sat at 812 lines, 62% over the 500-line hard cap, when DEC-356 replaced its
//! `collect_pattern_bindings` catch-all with named no-op arms. Rather than squeeze comments to hold a
//! grandfathered ceiling, the inline `#[cfg(test)] mod` moved here — the split-as-you-go default, which
//! reduces the debt instead of merely holding it.
use super::uses_concurrency;

fn parse(src: &str) -> crate::ast::Program {
    crate::loader::load_loose_src(src).expect("parse").program
}

#[test]
fn no_spawn_is_false() {
    assert!(!uses_concurrency(&parse(
        "package Main;\nimport Core.Output;\n\
         function main() -> void { Output.printLine(\"hi\"); }\n"
    )));
}

#[test]
fn spawn_in_main_is_true() {
    assert!(uses_concurrency(&parse(
        "package Main;\n\
         function sq(int n) -> int { return n * n; }\n\
         function main() -> void { var t = spawn sq(3); }\n"
    )));
}

#[test]
fn spawn_nested_in_a_helper_body_is_true() {
    // spawn buried in a non-main free function, inside an `if` inside a `for` — exercises the
    // statement recursion (a false negative here would silently route to the eager path).
    assert!(uses_concurrency(&parse(
        "package Main;\n\
         function sq(int n) -> int { return n * n; }\n\
         function work() -> void {\n\
             for (int i in 0..3) { if (i > 0) { var t = spawn sq(i); } }\n\
         }\n\
         function main() -> void { work(); }\n"
    )));
}

#[test]
fn spawn_in_a_method_body_is_true() {
    assert!(uses_concurrency(&parse(
        "package Main;\n\
         function sq(int n) -> int { return n * n; }\n\
         class Runner { function go() -> void { var t = spawn sq(2); } }\n\
         function main() -> void { new Runner().go(); }\n"
    )));
}

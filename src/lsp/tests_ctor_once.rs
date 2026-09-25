//! DEC-524 (scout row 5j): the constructor-once rule reaches the editor through the shared check
//! pipeline — a legal once-per-path assignment publishes nothing, and each refusal publishes its own
//! code (Invariant 17: `phg check` ≡ LSP diagnostics).

use super::tests::did_open;
use super::*;

fn diagnostics(src: &str) -> String {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src)).remove(0)
}

#[test]
fn a_once_per_path_assignment_publishes_no_diagnostic() {
    let out = diagnostics(
        "package Main;
class T { public int n; constructor(bool f) { if (f) { this.n = 1; } else { this.n = 2; } } }
",
    );
    assert!(out.contains("\"diagnostics\":[]"), "{out}");
}

#[test]
fn each_refusal_publishes_its_code() {
    let twice = diagnostics(
        "package Main;\nclass T { public int n; constructor(int v) { this.n = 1; this.n = v; } }\n",
    );
    assert!(twice.contains("E-ASSIGN-IMMUTABLE-TWICE"), "{twice}");
    let read = diagnostics(
        "package Main;\nclass T { public mutable int n; constructor(int v) { int y = this.n; this.n = v; } }\n",
    );
    assert!(read.contains("E-FIELD-READ-BEFORE-INIT"), "{read}");
}

//! LSP tests — DEC-486's test mode. Split out of `tests.rs` by cohesion (Invariant 13: that file sits
//! six lines under the 500-line hard cap), reusing its `did_open`/`req_at` helpers.
//!
//! Invariant 17 (`check ≡ LSP ≡ test`): a document containing a `test` item is checked in test mode by
//! the editors exactly as `phg check` checks it — accepted, body type-checked, and listed in the
//! outline. Before this, every `selftest/*.phg` squiggled `E-TEST-OUTSIDE-TESTS` in VS Code and
//! JetBrains on lines `phg test` accepted (panel C9/K1/K7).

use super::tests::{did_open, req_at};
use super::Server;

const TEST_SRC: &str =
    "package Main;\ntest \"adds\" { var y = 1 + 2; }\nfunction main() -> void { }";

#[test]
fn a_document_with_a_test_item_publishes_no_outside_tests_diagnostic() {
    let mut s = Server::default();
    let out = s.handle(&did_open("file:///t.phg", TEST_SRC));
    let body = out.join("");
    assert!(body.contains("publishDiagnostics"), "{body}");
    assert!(
        body.contains("\"diagnostics\":[]"),
        "a `test` item must diagnose CLEAN in the editor (test mode), got {body}"
    );
}

#[test]
fn a_type_error_inside_a_test_body_is_still_reported() {
    let mut s = Server::default();
    let out = s.handle(&did_open(
        "file:///t.phg",
        "package Main;\ntest \"bad\" { var y = 1 + true; }",
    ));
    let body = out.join("");
    assert!(
        body.contains("\"severity\":1"),
        "the body error must reach the editor: {body}"
    );
    assert!(
        !body.contains("E-TEST-OUTSIDE-TESTS"),
        "test mode must not raise the outside-tests gate: {body}"
    );
}

#[test]
fn a_test_item_appears_in_the_document_outline() {
    // SymbolKind Function = 12 — a named test is a runnable unit, listed beside the functions so the
    // editor's outline (and breadcrumb) can reach it; before DEC-486 the arm returned `None`.
    let mut s = Server::default();
    // `req_at` addresses `file:///x.phg` — open the document under that URI.
    s.handle(&did_open("file:///x.phg", TEST_SRC));
    let out = s.handle(&req_at("documentSymbol", 0, 0));
    let body = &out[0];
    assert!(body.contains("\"name\":\"adds\",\"kind\":12"), "{body}");
    assert!(body.contains("\"name\":\"main\",\"kind\":12"), "{body}");
}

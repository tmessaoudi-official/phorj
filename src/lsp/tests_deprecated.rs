//! LSP tests — DEC-417's `#[Deprecated]` tags. Split out of `tests.rs` by cohesion (Invariant 13: the
//! three tests below took that file over the 500-line hard cap), reusing its `did_open`/`req_at_uri`
//! helpers.
//!
//! These assert at the JSON level on purpose. Invariant 17's 100% rule is about what the EDITOR shows,
//! and a tag that never reaches the wire is invisible no matter how correct the checker is.

use super::tests::{did_open, req_at_uri};
use super::Server;

// ── DEC-417: the LSP half of `#[Deprecated]` ─────────────────────────────────────────────────────
// Invariant 17's 100% rule: the compiler knowing about a feature is not enough — the editor must show
// it. These pin the two tags that make that visible, at the JSON level, because a tag that never
// reaches the wire is invisible no matter how correct the checker is.

const DEPRECATED_SRC: &str = "package Main;\nimport Core.Runtime.Deprecated;\n#[Deprecated(message: \"use shout\")] function yell(): int { return 1; }\nfunction main() -> void { int x = yell(); }";

#[test]
fn a_deprecated_use_publishes_diagnostic_tag_2() {
    // `DiagnosticTag.Deprecated` = 2 — this is what makes the editor strike the CALL through.
    let mut s = Server::default();
    let out = s.handle(&did_open("file:///dep.phg", DEPRECATED_SRC));
    let body = out.join("");
    assert!(
        body.contains("W-DEPRECATED"),
        "no deprecation diagnostic: {body}"
    );
    assert!(
        body.contains("\"tags\":[2]"),
        "the use site must carry DiagnosticTag.Deprecated (2): {body}"
    );
    assert!(
        body.contains("\"severity\":2"),
        "a deprecation is a WARNING, never an error: {body}"
    );
}

#[test]
fn a_deprecated_declaration_is_tagged_in_completion() {
    // `CompletionItemTag.Deprecated` = 1 — shown struck through in the picker. Plus the legacy
    // `deprecated` boolean, which some clients still read instead of `tags`.
    let mut s = Server::default();
    let _ = s.handle(&did_open("file:///dep.phg", DEPRECATED_SRC));
    let out = s.handle(&req_at_uri("file:///dep.phg", "completion", 3, 33));
    let body = out.join("");
    assert!(
        body.contains("\"label\":\"yell\""),
        "no `yell` item: {body}"
    );
    let item_start = body.find("\"label\":\"yell\"").unwrap();
    let item = &body[item_start..(item_start + 140).min(body.len())];
    assert!(
        item.contains("\"tags\":[1]") && item.contains("\"deprecated\":true"),
        "the deprecated declaration must be tagged in completion: {item}"
    );
}

#[test]
fn a_live_declaration_carries_no_deprecation_tag() {
    let mut s = Server::default();
    let src = "package Main;\nfunction fine(): int { return 1; }\nfunction main() -> void { int x = fine(); }";
    let _ = s.handle(&did_open("file:///live.phg", src));
    let out = s.handle(&req_at_uri("file:///live.phg", "completion", 2, 33));
    let body = out.join("");
    let item_start = body.find("\"label\":\"fine\"").expect("no `fine` item");
    let item = &body[item_start..(item_start + 140).min(body.len())];
    assert!(
        !item.contains("\"tags\":[1]") && !item.contains("\"deprecated\":true"),
        "a live symbol must not be tagged: {item}"
    );
}

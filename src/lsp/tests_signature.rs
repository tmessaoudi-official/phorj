//! `textDocument/signatureHelp` — Invariant 17 names it in the 100% RULE, and it was the one named
//! capability the server did not advertise. Two layers: the pure text analysis in `signature.rs`
//! (which call, which argument), and the end-to-end JSON a client actually receives.

use super::signature::{call_at, split_params, CallSite};
use super::tests::{did_open, req_at, PROG};
use super::Server;
use crate::json::Json;

fn site(text: &str, offset: usize) -> Option<CallSite> {
    call_at(text, offset)
}

// ── call_at: which call, which argument ─────────────────────────────────────────────────────────

#[test]
fn cursor_right_after_the_open_paren_is_argument_zero() {
    let t = "helper(";
    assert_eq!(
        site(t, t.len()),
        Some(CallSite {
            callee: "helper".into(),
            active: 0
        })
    );
}

#[test]
fn top_level_commas_advance_the_active_argument() {
    let t = "f(a, b, ";
    assert_eq!(site(t, t.len()).unwrap().active, 2);
}

#[test]
fn a_nested_call_reports_the_innermost_frame_only() {
    // Inside `g(`, so `g`'s first argument — not `f`'s second.
    let t = "f(a, g(";
    let s = site(t, t.len()).unwrap();
    assert_eq!((s.callee.as_str(), s.active), ("g", 0));
    // …and once `g(...)` is closed, the cursor is back in `f`'s second argument.
    let t2 = "f(a, g(x), ";
    let s2 = site(t2, t2.len()).unwrap();
    assert_eq!((s2.callee.as_str(), s2.active), ("f", 2));
}

#[test]
fn a_comma_inside_a_string_literal_does_not_count() {
    // The phantom-comma failure: without string skipping this reports argument 2 of a call that
    // has been closed by the `)` inside the literal.
    let t = "f(\"a, b)\", ";
    let s = site(t, t.len()).unwrap();
    assert_eq!((s.callee.as_str(), s.active), ("f", 1));
    let t3 = "f(\"\"\"x, y)\"\"\", ";
    let s3 = site(t3, t3.len()).unwrap();
    assert_eq!((s3.callee.as_str(), s3.active), ("f", 1));
}

#[test]
fn brackets_and_braces_are_skipped_but_not_treated_as_the_call() {
    // `f(xs[` — still typing `f`'s first argument.
    let t = "f(xs[";
    let s = site(t, t.len()).unwrap();
    assert_eq!((s.callee.as_str(), s.active), ("f", 0));
    // A comma inside an inner `[…]` list literal belongs to the literal, not to `f`.
    let t2 = "f([1, 2], ";
    assert_eq!(site(t2, t2.len()).unwrap().active, 1);
}

#[test]
fn a_dotted_callee_is_kept_whole() {
    let t = "Output.printLine(";
    assert_eq!(site(t, t.len()).unwrap().callee, "Output.printLine");
}

#[test]
fn control_flow_and_declarations_are_not_calls() {
    for t in ["while (", "if (", "function helper(", "return ("] {
        assert_eq!(site(t, t.len()), None, "{t:?}");
    }
    // Outside any argument list at all.
    assert_eq!(site("helper(3);", 10), None);
}

#[test]
fn line_comments_are_skipped() {
    let t = "// f(\ng(";
    assert_eq!(site(t, t.len()).unwrap().callee, "g");
}

// ── split_params ────────────────────────────────────────────────────────────────────────────────

#[test]
fn split_params_keeps_generic_commas_together() {
    assert_eq!(
        split_params("function f(Map<string, int> m, List<int> xs) -> void"),
        vec!["Map<string, int> m", "List<int> xs"]
    );
    assert_eq!(split_params("function f() -> void"), Vec::<String>::new());
    assert_eq!(split_params("function g(int n) -> int"), vec!["int n"]);
}

// ── end to end ──────────────────────────────────────────────────────────────────────────────────

#[test]
fn initialize_advertises_signature_help_with_its_trigger_characters() {
    let mut s = Server::default();
    let out = s.handle(&Json::parse(r#"{"id":1,"method":"initialize"}"#).unwrap());
    assert!(out[0].contains("\"signatureHelpProvider\""), "{}", out[0]);
    assert!(
        out[0].contains("\"triggerCharacters\":[\"(\",\",\"]"),
        "{}",
        out[0]
    );
}

#[test]
fn signature_help_inside_a_user_call_shows_the_declaration_and_active_parameter() {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", PROG));
    // PROG line 2: `function main() -> void { var r = helper(3); }` — char 42 is just after `(`.
    let out = s.handle(&req_at("signatureHelp", 2, 42));
    let body = &out[0];
    assert!(
        body.contains("\"label\":\"function helper(int n) -> int\""),
        "{body}"
    );
    assert!(
        body.contains("\"parameters\":[{\"label\":\"int n\"}]"),
        "{body}"
    );
    assert!(body.contains("\"activeParameter\":0"), "{body}");
}

#[test]
fn signature_help_tracks_the_second_argument() {
    let mut s = Server::default();
    let prog = "package Main;\nfunction add(int a, int b) -> int { return a + b; }\nfunction main() -> void { var r = add(1, 2); }";
    s.handle(&did_open("file:///x.phg", prog));
    // line 2: cursor on the `2` — after the first comma.
    let out = s.handle(&req_at("signatureHelp", 2, 41));
    let body = &out[0];
    assert!(body.contains("\"activeParameter\":1"), "{body}");
    assert!(
        body.contains("\"parameters\":[{\"label\":\"int a\"},{\"label\":\"int b\"}]"),
        "{body}"
    );
}

/// DEC-419: the declaration's doc comment travels with the signature, as it does on hover.
#[test]
fn signature_help_carries_the_doc_comment() {
    let mut s = Server::default();
    let prog = "package Main;\n/** Doubles `n`. */\nfunction helper(int n) -> int { return n; }\nfunction main() -> void { var r = helper(3); }";
    s.handle(&did_open("file:///x.phg", prog));
    let out = s.handle(&req_at("signatureHelp", 3, 42));
    assert!(out[0].contains("Doubles `n`."), "{}", out[0]);
}

/// A stdlib call resolves through `native::registry()` — the same table completion and the checker
/// read — so a new native is signature-helped the moment it is registered.
#[test]
fn signature_help_on_a_native_reads_the_registry() {
    let mut s = Server::default();
    let prog = "package Main;\nimport Core.String;\nfunction main() -> void { var r = String.repeat(\"a\", 3); }";
    s.handle(&did_open("file:///x.phg", prog));
    // line 2: on the `3` inside `String.repeat("a", 3)` — past the comma. (The scan counts bytes
    // BEFORE the cursor, so a cursor sitting on the comma itself is still argument 0.)
    let out = s.handle(&req_at("signatureHelp", 2, 53));
    let body = &out[0];
    assert!(
        body.contains("\"label\":\"function String.repeat(string, int): string\""),
        "{body}"
    );
    assert!(body.contains("\"activeParameter\":1"), "{body}");
}

#[test]
fn signature_help_outside_a_call_is_null() {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", PROG));
    // line 1, char 0 — the `function` keyword of the declaration.
    let out = s.handle(&req_at("signatureHelp", 1, 0));
    assert!(out[0].contains("\"result\":null"), "{}", out[0]);
}

/// Cross-file: the callee is declared in another open buffer (a same-package sibling).
#[test]
fn signature_help_resolves_across_open_files() {
    let mut s = Server::default();
    s.handle(&did_open(
        "file:///a.phg",
        "package Main;\nfunction twice(int n) -> int { return n * 2; }",
    ));
    s.handle(&did_open(
        "file:///x.phg",
        "package Main;\nfunction main() -> void { var r = twice(3); }",
    ));
    let out = s.handle(&req_at("signatureHelp", 1, 41));
    assert!(
        out[0].contains("\"label\":\"function twice(int n) -> int\""),
        "{}",
        out[0]
    );
}

/// The case the feature exists for: the buffer is mid-edit and does NOT parse — the user is inside
/// an unclosed `(`. A parser-only lookup returns nothing here, which is why `same_file_decl` falls
/// back to the `function <name>(` token sequence.
#[test]
fn signature_help_works_while_the_buffer_does_not_parse() {
    let mut s = Server::default();
    let prog = "package Main;\nfunction helper(int n) -> int { return n; }\nfunction main() -> void { var r = helper(";
    s.handle(&did_open("file:///x.phg", prog));
    let line2_len = "function main() -> void { var r = helper(".len() as u32;
    let out = s.handle(&req_at("signatureHelp", 2, line2_len));
    let body = &out[0];
    assert!(
        body.contains("\"label\":\"function helper(int n) -> int\""),
        "{body}"
    );
    assert!(body.contains("\"activeParameter\":0"), "{body}");
}

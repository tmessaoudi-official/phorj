//! Hover tests — TUPLE locals and named-tuple FIELDS (DEC-504, Invariant 17's 100% rule).
//!
//! Split from `tests.rs`, which is at the Invariant-13 hard cap.
//!
//! Both cases here were found by probing the shipped server rather than by reading it: the
//! compiler knew `t.bp`, and the editor showed either nothing or a syntactically broken type.

use super::tests::{did_open, req_at};
use super::*;

/// A local whose type is a TUPLE hovers as the WHOLE type, not the text up to its first comma.
///
/// `local_signature_text` cut the declaration at the first `,`, `)`, `;`, `{` or newline. That is
/// right for a bare `int n`, and wrong for every type that CONTAINS a comma: a named tuple hovered
/// as `(source: string`, and the positional `(int, string)` of DEC-288 as `(int` — an unbalanced
/// paren, presented to the reader as their variable's type. The cut is now depth-aware, so a comma
/// or a close-paren only ends the signature at nesting depth zero.
#[test]
fn hover_on_a_tuple_local_shows_the_whole_type() {
    let src = "package Main;\nfunction main() -> void {\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               (int, string) u = (1, \"a\");\n\
               var a = t.bp; var c = u;\n}";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    // line 4 is `var a = t.bp; var c = u;` — column 8 is the `t` USE, column 22 the `u` use.
    let named = &s.handle(&req_at("hover", 4, 8))[0];
    assert!(
        named.contains("(source: string, bp: int)"),
        "named-tuple local hovered as a truncated type: {named}"
    );
    let positional = &s.handle(&req_at("hover", 4, 22))[0];
    assert!(
        positional.contains("(int, string)"),
        "positional-tuple local hovered as a truncated type: {positional}"
    );
}

/// Hovering a named tuple's FIELD reports that field's type.
///
/// Hover resolves the identifier under the cursor to a DECLARATION, and a tuple field is not one —
/// so `t.bp` hovered as `null`, and worse, an unrelated local named `bp` in the same scope would
/// have answered for it. The field case is resolved against the receiver's tuple type instead,
/// reusing the machinery that already backs `t.` completion.
#[test]
fn hover_on_a_named_tuple_field_shows_its_type() {
    let src = "package Main;\nfunction main() -> void {\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               var a = t.bp;\n}";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    // line 3 is `var a = t.bp;` — column 10 sits inside `bp`.
    let body = &s.handle(&req_at("hover", 3, 10))[0];
    assert!(body.contains("bp"), "field hover lost the name: {body}");
    assert!(body.contains("int"), "field hover lost the type: {body}");
    assert!(
        !body.contains("string"),
        "field hover answered with the WRONG position's type: {body}"
    );
}

/// A field hover must not be answered by an unrelated LOCAL that happens to share the name — the
/// exact misattribution the receiver-keyed lookup exists to prevent.
#[test]
fn a_field_hover_is_not_answered_by_a_same_named_local() {
    let src = "package Main;\nfunction main() -> void {\n\
               string bp = \"decoy\";\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               var a = t.bp;\n}";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    let body = &s.handle(&req_at("hover", 4, 10))[0];
    assert!(
        !body.contains("decoy"),
        "the decoy local answered for the tuple field: {body}"
    );
    assert!(body.contains("int"), "field hover lost the type: {body}");
}

/// Go-to-definition on a tuple field answers NOTHING rather than jumping somewhere wrong.
///
/// A tuple field has no declaration site: the type annotation's label is not one, and the field is
/// not a class member. Falling through to the ordinary lookup made `t.bp` jump to any local named
/// `bp` in scope — a confident jump to an unrelated line. Nothing is the honest answer; hover still
/// reports the field's type. (Jumping to the LABEL inside the receiver's written type would need
/// spans `receiver_tuple_fields` does not carry — an uncertified follow-up, not a claim.)
#[test]
fn go_to_definition_on_a_tuple_field_does_not_jump_to_a_same_named_local() {
    let src = "package Main;\nfunction main() -> void {\n\
               string bp = \"decoy\";\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               var a = t.bp;\n}";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    let body = &s.handle(&req_at("definition", 4, 10))[0];
    assert!(
        body.contains("\"result\":null"),
        "definition jumped somewhere for a tuple field: {body}"
    );
}

/// `t?.bp` is refused on a tuple (`E-TUPLE-SAFE-FIELD`), but it still READS as a field access, so
/// hover answers with the field's type while the diagnostic explains the refusal.
///
/// Pinned because the receiver walk stopped on the `?`: it found no receiver, returned `None`, and
/// fell through to the declaration lookup — the decoy path this module exists to close, reopened by
/// one character.
#[test]
fn hover_on_a_safe_navigated_tuple_field_still_resolves_against_its_receiver() {
    let src = "package Main;\nfunction main() -> void {\n\
               string bp = \"decoy\";\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               var a = t?.bp;\n}";
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", src));
    let body = &s.handle(&req_at("hover", 4, 11))[0];
    assert!(
        !body.contains("decoy"),
        "the decoy local answered for a `?.` tuple field: {body}"
    );
    assert!(body.contains("int"), "field hover lost the type: {body}");
}

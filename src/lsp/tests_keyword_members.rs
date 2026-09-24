//! DEC-531 (scout row 5k): the editor surfaces a reserved-word MEMBER the way it surfaces any other
//! member — no parse diagnostic, a document-symbol range on the NAME, and completion after `.`.
//! Hover/definition resolve top-level and local names only (members are not resolved in v1), so a
//! keyword member answers exactly what an ordinary one does there.

use super::tests::{did_open, req_at};
use super::*;

const SRC: &str = "package Main;
class Rule {
  public int type = 1;
  public function match(int x): int { return match (x) { 1 => 2, default => x }; }
}
function apply(Rule r): int { return r.match(r.type); }
";

#[test]
fn a_keyword_member_publishes_no_diagnostic() {
    let mut s = Server::default();
    let out = s.handle(&did_open("file:///x.phg", SRC));
    assert!(out[0].contains("\"diagnostics\":[]"), "{}", out[0]);
}

#[test]
fn a_keyword_member_symbol_selects_its_name() {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", SRC));
    let body = &s.handle(&req_at("documentSymbol", 0, 0))[0];
    // `type` sits at line 2, character 13; `match` (the METHOD, not the expression) at line 3, 18.
    let field = "\"name\":\"type\",\"kind\":8,\"range\":{\"start\":{\"line\":2,\"character\":13},\"end\":{\"line\":2,\"character\":17}}";
    assert!(body.contains(field), "{body}");
    assert!(
        body.contains("\"selectionRange\":{\"start\":{\"line\":3,\"character\":18},\"end\":{\"line\":3,\"character\":23}}"),
        "{body}"
    );
}

#[test]
fn completion_after_a_dot_offers_keyword_members() {
    let mut s = Server::default();
    // The cursor gets its own line: completion re-parses with the cursor's line blanked.
    let src = SRC.replace("return r.match(r.type); }", "\n  return r.\n}");
    s.handle(&did_open("file:///x.phg", &src));
    let body = &s.handle(&req_at("completion", 6, 11))[0];
    assert!(body.contains("\"label\":\"match\""), "{body}");
    assert!(body.contains("\"label\":\"type\""), "{body}");
}

#[test]
fn a_keyword_in_keyword_position_is_not_a_name() {
    let src = "function f(bool c, int x): int { if (c) { return; } else { return match (x) { default => c ? 1 : 2 }; } }";
    let tokens = crate::tokenizer::lex(src).unwrap();
    let names: Vec<&str> = (0..tokens.len())
        .filter(|&i| !matches!(tokens[i].kind, crate::token::TokenKind::Ident(_)))
        .filter_map(|i| super::names::name_at(&tokens, i))
        .collect();
    assert!(names.is_empty(), "keywords read as names: {names:?}");
    let t = crate::tokenizer::lex("f(c ? true : false, type: 1); x with { match = 2 }; r.open()")
        .unwrap();
    let kw: Vec<&str> = (0..t.len())
        .filter(|&i| !matches!(t[i].kind, crate::token::TokenKind::Ident(_)))
        .filter_map(|i| super::names::name_at(&t, i))
        .collect();
    assert_eq!(kw, ["type", "match", "open"]);
}

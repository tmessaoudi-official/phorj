//! DEC-533 (scout row 5m): a collection constant is an ordinary constant to the editor — no
//! diagnostic, listed in the outline. Hover, go-to-definition and completion on a CLASS-QUALIFIED
//! member (`K.MAX`, `K.twice`) are row 5o's, pinned in `tests_class_members.rs`.

use super::tests::{did_open, req_at};
use super::*;

const SRC: &str = "package Main;
class Accents {
    public const Map<string, List<int>> BANDS = [\"A\" => [1, 2]];
    public const int OFFSET = -5;
}
function f(): int { var b = Accents.BANDS; return Accents.OFFSET; }
";

#[test]
fn a_collection_constant_publishes_no_diagnostic() {
    let mut s = Server::default();
    let out = s.handle(&did_open("file:///x.phg", SRC));
    assert!(out[0].contains("\"diagnostics\":[]"), "{}", out[0]);
}

#[test]
fn the_outline_lists_collection_constants() {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", SRC));
    let body = &s.handle(&req_at("documentSymbol", 0, 0))[0];
    assert!(body.contains("\"name\":\"BANDS\",\"kind\":8"), "{body}");
    assert!(body.contains("\"name\":\"OFFSET\",\"kind\":8"), "{body}");
}

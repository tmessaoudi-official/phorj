//! DEC-532 (scout row 5l): a throw-expression is an ordinary expression to the editor — no parse
//! diagnostic, and a name inside the thrown operand resolves like a name anywhere else.

use super::tests::{did_open, req_at};
use super::*;

const SRC: &str = "package Main;
class BadError implements Error { constructor(public string message) {} }
function reason(): string { return \"r\"; }
function k(string? s): string throws BadError { return s ?? throw new BadError(reason()); }
";

#[test]
fn a_throw_expression_publishes_no_diagnostic() {
    let mut s = Server::default();
    let out = s.handle(&did_open("file:///x.phg", SRC));
    assert!(out[0].contains("\"diagnostics\":[]"), "{}", out[0]);
}

#[test]
fn definition_resolves_a_name_inside_the_thrown_operand() {
    let mut s = Server::default();
    s.handle(&did_open("file:///x.phg", SRC));
    let line = SRC.lines().nth(3).unwrap();
    let col = line.find("reason()").unwrap() as u32 + 1;
    let body = &s.handle(&req_at("definition", 3, col))[0];
    // `reason` is declared on LSP line 2.
    assert!(body.contains("\"line\":2"), "{body}");
}

//! PHP transpiler tests — expressions used as the RECEIVER of a call or field read, where PHP's
//! grammar needs parentheses the Phorj source does not (plan row 4b, TRANSPILE-WITH-CALL).

use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn php(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

#[test]
fn clone_with_used_as_a_receiver_is_parenthesized() {
    // PHP 8.5 rejects `clone($p, ['x' => 5])->d()` with `unexpected token "->"`, so a `with`
    // result that is the receiver of a call or a field read is wrapped as `(clone(…))`. A `with`
    // anywhere else keeps its bare form. (Raw-AST helper: `P(1)` is the pre-checker spelling of
    // `new P(1)`, as in the sibling test modules.)
    let out = php(
        "class P { constructor(public int x) {} function d(): int { return this.x * 2; } } \
             function main(): void { P p = P(1); int a = p with { x = 5 }.d(); \
             int b = (p with { x = 6 }).x; P c = p with { x = 7 }; }",
    );
    assert!(
        out.contains("(clone($p, ['x' => 5]))->d()"),
        "method receiver:\n{out}"
    );
    assert!(
        out.contains("(clone($p, ['x' => 6]))->x"),
        "field receiver:\n{out}"
    );
    assert!(
        out.contains("$c = clone($p, ['x' => 7]);"),
        "plain position unwrapped:\n{out}"
    );
}

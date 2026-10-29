//! PHP transpiler tests — statics and composition across a multi-parent decomposition (plan row 4c,
//! KNOWN_ISSUES STATIC-COMPOSITION-PARITY).

use super::emit;
use crate::parser::Parser;
use crate::tokenizer::lex;

fn php(src: &str) -> String {
    let tokens = lex(src).expect("lex");
    let prog = Parser::new(tokens).parse_program().expect("parse");
    emit(&prog).expect("emit")
}

#[test]
fn a_decomposed_parents_static_method_is_static_in_its_interface() {
    // Item 3: `A` is decomposed (an ancestor of a multi-parent class) into `interface IA` + `trait TA`.
    // The interface declared `fa` non-static while the trait body defines it `static`, and PHP
    // rejects the class: `Cannot make non static method IA::fa() static in class A`.
    let out = php(
        "open class A { static function fa(): int { return 1; } function g(): int { return 2; } } \
         open class B { int j = 2; } class M extends A, B {}",
    );
    assert!(
        out.contains("public static function fa(): int;"),
        "the interface must declare `fa` static:\n{out}"
    );
    // An instance method keeps its non-static signature.
    assert!(out.contains("public function g(): int;"), "{out}");
}

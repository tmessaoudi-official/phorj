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

#[test]
fn an_inherited_static_names_its_owners_slot() {
    // Item 2: `M.sa` is A's ONE slot on the native legs, so the PHP must name `A::$sa`. With `A`
    // decomposed (a parent of a multi-parent class) `M::$sa` would be M's own trait copy, a second slot.
    let out = php(
        "open class A { static mutable int sa = 2; } open class B { function g(): int { return 1; } } \
         class M extends A, B {} \
         function main(): void { M.sa = 7; int v = M.sa; }",
    );
    assert!(
        out.contains("A::$sa = 7;"),
        "the write must name A's slot:\n{out}"
    );
    assert!(!out.contains("M::$sa"), "no second slot through M:\n{out}");
    // A redeclaring subclass keeps its own.
    let own = php(
        "open class A { static mutable int sa = 2; } class M extends A { static mutable int sa = 5; } \
         function main(): void { M.sa = 7; }",
    );
    assert!(own.contains("M::$sa = 7;"), "{own}");
}

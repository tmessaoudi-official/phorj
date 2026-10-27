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

#[test]
fn trait_initializers_run_in_the_using_classs_constructor() {
    // Item 4: a trait never synthesizes an init-only `__construct` (two such traits collide fatally,
    // and a user's own constructor overrode it); the using class's constructor runs the set.
    let out = php(
        "trait T { int k = 9; } trait R { int r = 4; } class U { use T; use R; } \
         class W { use T; int j; constructor(int j0) { this.j = j0; } }",
    );
    let trait_t = out
        .split("trait T {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(
        !trait_t.contains("__construct"),
        "trait T must not construct:\n{out}"
    );
    assert!(
        out.contains("$this->k = 9;\n        $this->r = 4;"),
        "U's constructor runs both trait sets:\n{out}"
    );
    let w = out.split("class W {").nth(1).unwrap();
    assert!(
        w.contains(
            "function __construct(int $j0) {\n        $this->k = 9;\n        $this->j = $j0;"
        ),
        "W's own constructor runs the trait set first:\n{out}"
    );
}

#[test]
fn a_trait_constructor_user_keeps_it_unless_it_has_more_initializers() {
    // Plain: the trait's `__construct` (with its own prelude) is the class's; a synthesized zero-arg
    // `__construct` would override it.
    let plain = php(
        "trait T { int k = 9; int m; constructor(int m0) { this.m = m0; } } class U { use T; }",
    );
    let u = plain
        .split("class U {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(
        !u.contains("__construct"),
        "U keeps T's constructor:\n{plain}"
    );
    // More initializers than the trait constructor runs: the class emits its own, from the plan.
    let more = php(
        "trait T { int k = 9; int m; constructor(int m0) { this.m = m0; } } trait R { int r = 4; } \
         class U { use T; use R; int j = 3; }",
    );
    assert!(
        more.contains(
            "public function __construct(int $m0) {\n        $this->k = 9;\n        $this->r = 4;\n        $this->j = 3;\n        $this->m = $m0;"
        ),
        "{more}"
    );
}

#[test]
fn a_synthesized_constructor_promotes_initializes_then_runs_bodies() {
    // The native order across a multi-parent plan: every promoted param, the initializer set, then the
    // bodies (it interleaved promotion and bodies per parent before). A decomposed parent with no
    // constructor still runs its own initializer.
    let out = php(
        "open class A { mutable int x = 1; constructor(public int a) { this.x = this.a + 10; } } \
         open class B { int y = 2; constructor(public int b) {} } class M extends A, B {}",
    );
    let m = out.split("final class M").nth(1).unwrap();
    assert!(
        m.contains(
            "$this->a = $a;\n        $this->b = $b;\n        $this->x = 1;\n        $this->y = 2;\n        $this->x = $this->a + 10;"
        ),
        "{out}"
    );
    let only_init =
        php("open class A { int x = 1; } open class B { int y = 2; } class M extends A, B {}");
    let a = only_init.split("class A implements IA {").nth(1).unwrap();
    assert!(
        a.contains("public function __construct() {\n        $this->x = 1;"),
        "{only_init}"
    );
}

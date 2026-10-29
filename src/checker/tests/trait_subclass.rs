//! Checker tests — a subclass of a trait-using class sees the trait's members (plan row 4c item 5,
//! KNOWN_ISSUES STATIC-COMPOSITION-PARITY item 5). The runtime already inherits them (the dispatch table
//! and `ast::ctor_plan` reach a parent's trait), and PHP inherits what a class composed; the checker
//! merged traits only into their direct users, so all four member kinds were rejected on a subclass.

use super::support::*;

const T: &str = "trait T { mutable int k = 0; static mutable int s = 1; \
                 function f(): int { return 1; } static function sf(): int { return 3; } } \
                 open class U { use T; } class V extends U {} ";

#[test]
fn a_trait_method_is_callable_on_a_subclass_of_its_user() {
    let src = format!("{T} function main(): void {{ int a = new V().f(); }}");
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

#[test]
fn a_trait_static_method_and_static_field_reach_the_subclass() {
    let src = format!("{T} function main(): void {{ int a = V.sf(); V.s = 4; int b = V.s; }}");
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

#[test]
fn a_trait_field_is_readable_and_writable_on_the_subclass() {
    let src = format!("{T} function main(): void {{ V v = new V(); v.k = 4; int a = v.k; }}");
    assert!(errors_of(&src).is_empty(), "{:?}", errors_of(&src));
}

#[test]
fn a_trait_constructor_is_the_subclasss_inherited_signature() {
    let src =
        "trait T { constructor(public int k) {} } open class U { use T; } class V extends U {} \
               function main(): void { V v = new V(3); int a = v.k; }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
    // Arity is still checked against that signature.
    let bad =
        "trait T { constructor(public int k) {} } open class U { use T; } class V extends U {} \
               function main(): void { V v = new V(); }";
    assert!(
        !errors_of(bad).is_empty(),
        "a missing ctor argument must still be an error"
    );
}

#[test]
fn a_trait_constructor_takes_named_arguments_on_its_user_and_the_subclass() {
    // The trait merge copied the ctor signature but not its parameter names (DEC-297), so `new U(k: 3)`
    // was `E-NAMED-ARG-UNKNOWN` with an empty parameter list — and the second pass carried that to V.
    let src =
        "trait T { constructor(public int k) {} } open class U { use T; } class V extends U {} \
               function main(): void { U u = new U(k: 3); V v = new V(k: 4); }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

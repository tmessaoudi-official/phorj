//! Checker tests — interfaces (M-Decomp W2b, by language feature).

use super::support::*;

#[test]
fn interface_conformance_and_subtyping_ok() {
    // A class providing every interface method type-checks; its instance flows into an
    // interface-typed parameter (nominal subtyping) and an interface-typed local.
    let src = "interface Speaker { function speak() -> string; } \
                   class Dog implements Speaker { function speak() -> string { return \"w\"; } } \
                   function announce(Speaker s) -> string { return s.speak(); } \
                   function main() -> void { Speaker sp = new Dog(); discard announce(sp); }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

#[test]
fn interface_missing_method_is_unimpl() {
    let src = "interface Speaker { function speak() -> string; } \
                   class Mute implements Speaker {} \
                   function main() -> void {}";
    let e = errors_of(src);
    assert!(e.iter().any(|d| d.code == Some("E-IFACE-UNIMPL")), "{e:?}");
}

#[test]
fn interface_wrong_signature_is_sig() {
    // `speak` must return `string`; returning `int` is a signature mismatch.
    let src = "interface Speaker { function speak() -> string; } \
                   class Dog implements Speaker { function speak() -> int { return 1; } } \
                   function main() -> void {}";
    let e = errors_of(src);
    assert!(e.iter().any(|d| d.code == Some("E-IFACE-SIG")), "{e:?}");
}

#[test]
fn implements_a_non_interface_is_impl_error() {
    // `implements` must name a declared interface, not a class.
    let src = "class A {} class B implements A {} function main() -> void {}";
    let e = errors_of(src);
    assert!(e.iter().any(|d| d.code == Some("E-IFACE-IMPL")), "{e:?}");
}

#[test]
fn interface_extends_cycle_is_rejected() {
    let src = "interface A extends B { function a() -> int; } \
                   interface B extends A { function b() -> int; } \
                   function main() -> void {}";
    let e = errors_of(src);
    assert!(e.iter().any(|d| d.code == Some("E-IFACE-CYCLE")), "{e:?}");
}

#[test]
fn interface_is_not_assignable_to_unrelated_class() {
    // A Speaker is not a Dog: interface → concrete class is not a subtype.
    let src = "interface Speaker { function speak() -> string; } \
                   class Dog implements Speaker { function speak() -> string { return \"w\"; } } \
                   function main() -> void { Speaker s = new Dog(); Dog d = s; }";
    let e = errors_of(src);
    assert!(!e.is_empty(), "expected an assignability error, got none");
}

#[test]
fn generic_interface_conformance_ok_and_runs_through_receiver() {
    // DEC-257 slice 1: `implements Producer<int>` substitutes T=int into the interface's
    // signatures for conformance, and an interface-typed receiver substitutes its arguments
    // (`p.produce()` types as `int`, not the raw `T`).
    let src = "interface Producer<T> { function produce() -> T; } \
                   class Ints implements Producer<int> { function produce() -> int { return 7; } } \
                   function consume(Producer<int> p) -> int { return p.produce() + 1; } \
                   function main() -> void { Producer<int> p = new Ints(); discard consume(p); }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

#[test]
fn generic_interface_wrong_impl_type_is_sig() {
    // T=int is substituted before comparison, so a `string` return is a signature mismatch.
    let src = "interface Producer<T> { function produce() -> T; } \
                   class Wrong implements Producer<int> { function produce() -> string { return \"x\"; } } \
                   function main() -> void {}";
    let e = errors_of(src);
    assert!(e.iter().any(|d| d.code == Some("E-IFACE-SIG")), "{e:?}");
}

#[test]
fn generic_interface_missing_args_is_arity_error() {
    let src = "interface Producer<T> { function produce() -> T; } \
                   class NoArgs implements Producer { function produce() -> int { return 1; } } \
                   function main() -> void {}";
    let e = errors_of(src);
    assert!(
        e.iter().any(|d| d.code == Some("E-TYPE-ARG-COUNT")),
        "{e:?}"
    );
}

#[test]
fn generic_interface_assignability_is_argument_invariant() {
    // `Ints implements Producer<int>` — its instance must NOT flow into `Producer<string>`.
    let src = "interface Producer<T> { function produce() -> T; } \
                   class Ints implements Producer<int> { function produce() -> int { return 7; } } \
                   function main() -> void { Producer<string> p = new Ints(); }";
    let e = errors_of(src);
    assert!(!e.is_empty(), "expected an assignability error, got none");
}

#[test]
fn generic_class_implements_generic_interface_through_own_param() {
    // `Boxed<T> implements Producer<T>`: the recorded interface arguments mention the class's
    // own parameter, substituted from the instance (`Boxed(42)` ⇒ `Producer<int>`).
    let src = "interface Producer<T> { function produce() -> T; } \
                   class Boxed<T> implements Producer<T> { \
                     constructor(private T v) {} \
                     function produce() -> T { return this.v; } } \
                   function main() -> void { Producer<int> p = new Boxed(42); discard p.produce(); }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

#[test]
fn error_implementor_must_carry_the_suffix() {
    // DEC-275: a throwable type must read as one — direct implementors, and subclasses of an
    // error base (transitive), need the Error|Exception suffix.
    let bad = "class Oops implements Error { constructor(public string message) {} } \
                   function main() -> void {}";
    let e = errors_of(bad);
    assert!(e.iter().any(|d| d.code == Some("E-ERROR-NAME")), "{e:?}");
    let bad_sub = "open class BaseError implements Error { constructor(public string message) {} } \
                   class Timeout extends BaseError { constructor(string m) { parent.constructor(m); } } \
                   function main() -> void {}";
    let e2 = errors_of(bad_sub);
    assert!(e2.iter().any(|d| d.code == Some("E-ERROR-NAME")), "{e2:?}");
}

#[test]
fn error_and_exception_suffixes_both_pass() {
    let ok = "class OopsError implements Error { constructor(public string message) {} } \
                  class OopsException implements Error { constructor(public string message) {} } \
                  function main() -> void {}";
    assert!(errors_of(ok).is_empty(), "{:?}", errors_of(ok));
}

#[test]
fn instanceof_against_interface_narrows() {
    // `instanceof` accepts an interface RHS, and inside the then-block the operand is
    // smart-cast to the interface so its methods resolve.
    let src = "interface Speaker { function speak() -> string; } \
                   class Dog implements Speaker { function speak() -> string { return \"w\"; } } \
                   function main() -> void { Dog d = new Dog(); \
                     if (d instanceof Speaker) { discard d.speak(); } }";
    assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
}

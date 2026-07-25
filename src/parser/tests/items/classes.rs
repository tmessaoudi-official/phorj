//! Parser tests — classes, members, modifiers, property hooks, implements, interfaces.

use super::super::support::*;

#[test]
fn parses_class_decl() {
    let src = "class Greeter { \
                     private string name; \
                     constructor(private string name) {} \
                     function greet() -> string { return name; } \
                   }";
    match item(src) {
        Item::Class(c) => {
            assert_eq!(c.name, "Greeter");
            assert_eq!(c.members.len(), 3);
            match &c.members[0] {
                ClassMember::Field {
                    modifiers, name, ..
                } => {
                    assert_eq!(name, "name");
                    assert_eq!(modifiers, &vec![Modifier::Private]);
                }
                other => panic!("member 0: {other:?}"),
            }
            match &c.members[1] {
                ClassMember::Constructor { params, .. } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].modifiers, vec![Modifier::Private]);
                    assert_eq!(params[0].name, "name");
                }
                other => panic!("member 1: {other:?}"),
            }
            match &c.members[2] {
                ClassMember::Method(f) => assert_eq!(f.name, "greet"),
                other => panic!("member 2: {other:?}"),
            }
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_mutable_field_and_ctor_param_modifier() {
    // M-mut.6: `mutable` is accepted in field + promoted-ctor-param modifier position.
    let src = "class C { \
                     mutable int count; \
                     constructor(public mutable int total) {} \
                   }";
    match item(src) {
        Item::Class(c) => {
            match &c.members[0] {
                ClassMember::Field {
                    modifiers, name, ..
                } => {
                    assert_eq!(name, "count");
                    assert_eq!(modifiers, &vec![Modifier::Mutable]);
                }
                other => panic!("member 0: {other:?}"),
            }
            match &c.members[1] {
                ClassMember::Constructor { params, .. } => {
                    assert_eq!(
                        params[0].modifiers,
                        vec![Modifier::Public, Modifier::Mutable]
                    );
                }
                other => panic!("member 1: {other:?}"),
            }
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn open_method_modifier_and_final_retired() {
    // S6a.1: `open` parses as a method modifier. (Methods use block bodies, not `=> expr`.)
    match item("class C { open function f() -> int { return 1; } }") {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Method(m) => {
                assert_eq!(m.name, "f");
                assert_eq!(m.modifiers, vec![Modifier::Open]);
            }
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
    // S6a.1: `final` is no longer a keyword — it now lexes as an ordinary identifier.
    let toks = lex("final").expect("lex ok");
    assert!(
        matches!(&toks[0].kind, TokenKind::Ident(s) if s == "final"),
        "expected `final` to lex as Ident, got {:?}",
        toks[0].kind
    );
}

#[test]
fn parses_open_class_with_single_extends() {
    // S6a.2: `open` class prefix + a single `extends` parent.
    let p = prog("package Main;\nopen class Animal {}\nclass Dog extends Animal {}");
    let animal = match &p.items[0] {
        Item::Class(c) => c,
        o => panic!("item 0: {o:?}"),
    };
    assert!(animal.open, "Animal should be open");
    assert!(animal.extends.is_empty(), "Animal extends nothing");
    let dog = match &p.items[1] {
        Item::Class(c) => c,
        o => panic!("item 1: {o:?}"),
    };
    assert!(!dog.open, "Dog is final-by-default (not open)");
    assert_eq!(dog.extends, vec!["Animal".to_string()]);
}

#[test]
fn open_prefix_on_a_non_class_is_an_error() {
    // S6a.2: `open` only applies to classes.
    let msg = prog_err("package Main;\nopen function f() -> void {}");
    assert!(msg.contains("only a class"), "got: {msg}");
}

#[test]
fn parses_static_field_with_initializer() {
    // M-mut.7: `static mutable int total = 0;` — static modifier + field-level initializer.
    let src = "class C { static mutable int total = 0; }";
    match item(src) {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Field {
                modifiers,
                name,
                init,
                ..
            } => {
                assert_eq!(name, "total");
                assert_eq!(modifiers, &vec![Modifier::Static, Modifier::Mutable]);
                assert!(matches!(init, Some(Expr::Int(0, _))));
            }
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_property_hook_get_and_set() {
    // M-mut.7b: `float fahrenheit { get => …; set(float v) { … } }` — a property hook with
    // both a computed-read body and an intercepted-write body.
    let src = "class Temp { \
                     mutable float celsius; \
                     float fahrenheit { \
                       get => this.celsius * 2.0; \
                       set(float v) { this.celsius = v; } \
                     } \
                   }";
    match item(src) {
        Item::Class(c) => match &c.members[1] {
            ClassMember::Hook {
                name, get, set, ty, ..
            } => {
                assert_eq!(name, "fahrenheit");
                assert!(matches!(ty, Type::Named { name, .. } if name == "float"));
                assert!(get.is_some(), "expected a get body");
                let (p, stmts) = set.as_ref().expect("expected a set body");
                assert_eq!(p.name, "v");
                assert_eq!(stmts.len(), 1);
            }
            other => panic!("member 1: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_read_only_property_hook() {
    // A get-only hook (no `set`) is a read-only computed property.
    match item("class C { int doubled { get => 2; } }") {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Hook { get, set, .. } => {
                assert!(get.is_some());
                assert!(set.is_none());
            }
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_class_implements_list() {
    // M-RT S2: `implements A, B` is parsed into ClassDecl.implements.
    match item("class Dog implements Speaker, Pet { function speak() -> string { return \"w\"; } }")
    {
        Item::Class(c) => {
            assert_eq!(c.name, "Dog");
            assert_eq!(c.implements, vec!["Speaker".to_string(), "Pet".to_string()]);
            assert_eq!(c.members.len(), 1);
        }
        other => panic!("got {other:?}"),
    }
    // No `implements` ⇒ empty list.
    match item("class Plain {}") {
        Item::Class(c) => assert!(c.implements.is_empty()),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_interface_decl() {
    // M-RT S2: an interface is method signatures (no bodies) + an optional `extends` list.
    match item("interface Pet extends Speaker, Named { function speak() -> string; function age() -> int; }") {
            Item::Interface(i) => {
                assert_eq!(i.name, "Pet");
                assert_eq!(i.extends, vec!["Speaker".to_string(), "Named".to_string()]);
                assert_eq!(i.methods.len(), 2);
                assert_eq!(i.methods[0].name, "speak");
                assert!(i.methods[0].body.is_empty(), "signature has no body");
                assert_eq!(i.methods[1].name, "age");
            }
            other => panic!("got {other:?}"),
        }
}

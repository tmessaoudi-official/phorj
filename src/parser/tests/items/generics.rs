//! Parser tests — generic type params and `throws` clauses (functions + constructors).

use super::super::support::*;

#[test]
fn parses_generic_function_type_params() {
    // `function id<T>(T x) -> T { … }` records the type parameter list (M-RT S7).
    match item("function id<T, U>(T a, U b) -> T { return a; }") {
        Item::Function(f) => assert_eq!(f.type_params, vec!["T".to_string(), "U".to_string()]),
        other => panic!("expected a generic function, got {other:?}"),
    }
    // A non-generic function has an empty type-param list.
    match item("function plain(int x) -> int { return x; }") {
        Item::Function(f) => assert!(f.type_params.is_empty()),
        other => panic!("expected a function, got {other:?}"),
    }
}

#[test]
fn parses_generic_methods() {
    // M-RT generics-all: a method may declare `<T>` just like a free function.
    let item = parser("class C { function m<T>(T x) -> T { return x; } }")
        .parse_item()
        .expect("generic method should parse");
    match item {
        Item::Class(c) => match &c.members[0] {
            crate::ast::ClassMember::Method(f) => {
                assert_eq!(f.type_params, vec!["T".to_string()]);
            }
            _ => panic!("expected a method"),
        },
        _ => panic!("expected a class"),
    }
}

#[test]
fn parses_fn_throws_clause() {
    // Single declared exception type.
    match &prog("package Main;\nfunction f() -> int throws ParseError { return 1; }").items[0] {
        Item::Function(f) => {
            assert_eq!(f.throws.len(), 1);
            assert!(matches!(&f.throws[0], Type::Named { name, .. } if name == "ParseError"));
        }
        other => panic!("expected function, got {other:?}"),
    }
    // `throws A | B` captures the whole union as one `Type::Union`.
    match &prog("package Main;\nfunction g() -> void throws A | B { return; }").items[0] {
        Item::Function(f) => {
            assert_eq!(f.throws.len(), 1);
            assert!(matches!(&f.throws[0], Type::Union(members, _) if members.len() == 2));
        }
        other => panic!("expected function, got {other:?}"),
    }
    // No throws clause ⇒ empty.
    match &prog("package Main;\nfunction h() -> void {}").items[0] {
        Item::Function(f) => assert!(f.throws.is_empty()),
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn parses_ctor_throws_clause() {
    // DEC-221: a constructor may declare a `throws` clause between its params and body.
    match item("class C { constructor(int x) throws ParseError {} }") {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Constructor { params, throws, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(throws.len(), 1);
                assert!(matches!(&throws[0], Type::Named { name, .. } if name == "ParseError"));
            }
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
    // `throws A | B` captures the whole union as one `Type::Union`, like the fn form.
    match item("class C { constructor() throws A | B {} }") {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Constructor { throws, .. } => {
                assert_eq!(throws.len(), 1);
                assert!(matches!(&throws[0], Type::Union(members, _) if members.len() == 2));
            }
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
    // No throws clause ⇒ empty (byte-identical to the pre-DEC-221 AST).
    match item("class C { constructor(int x) {} }") {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Constructor { throws, .. } => assert!(throws.is_empty()),
            other => panic!("member 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
}

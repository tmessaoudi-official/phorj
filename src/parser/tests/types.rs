//! Parser tests — types (M-Decomp W3.1b, mirrors the source clusters).

use super::support::*;

#[test]
fn parse_type_union_and_single() {
    // A union of three; a single type is returned unchanged (no wrapping).
    match ty("A | B | C") {
        Type::Union(members, _) => assert_eq!(members.len(), 3),
        other => panic!("expected union, got {other:?}"),
    }
    assert!(matches!(ty("A"), Type::Named { .. }));
    // `?` binds to its immediate member: `A | B?` ≡ `A | (B?)`.
    match ty("A | B?") {
        Type::Union(m, _) => assert!(matches!(m[1], Type::Optional { .. })),
        other => panic!("expected union, got {other:?}"),
    }
    // a union nests inside a generic argument.
    assert!(matches!(ty("List<A | B>"), Type::Named { .. }));
}

#[test]
fn parse_type_intersection_and_precedence() {
    // An intersection of three; a single type is returned unchanged.
    match ty("A & B & C") {
        Type::Intersection(members, _) => assert_eq!(members.len(), 3),
        other => panic!("expected intersection, got {other:?}"),
    }
    // `&` binds tighter than `|`: `A | B & C` ≡ `A | (B & C)` — a union whose 2nd member is an
    // intersection.
    match ty("A | B & C") {
        Type::Union(m, _) => {
            assert_eq!(m.len(), 2);
            assert!(matches!(m[0], Type::Named { .. }));
            assert!(matches!(m[1], Type::Intersection(_, _)));
        }
        other => panic!("expected union, got {other:?}"),
    }
    // an intersection nests inside a generic argument and a function param.
    assert!(matches!(ty("List<A & B>"), Type::Named { .. }));
    assert!(matches!(ty("(A & B) -> C"), Type::Function { .. }));
}

#[test]
fn parse_type_pattern_vs_binding() {
    match pat("Circle c") {
        Pattern::Type {
            type_name, binding, ..
        } => {
            assert_eq!(type_name, "Circle");
            assert_eq!(binding.as_deref(), Some("c"));
        }
        other => panic!("expected type pattern, got {other:?}"),
    }
    // `Type _` binds nothing.
    assert!(matches!(
        pat("Circle _"),
        Pattern::Type { binding: None, .. }
    ));
    // A lone LOWERCASE ident is a catch-all Binding (valid).
    assert!(matches!(pat("shape"), Pattern::Binding { name, .. } if name == "shape"));
    // A lone PascalCase ident is REJECTED (DEC-209): it looks like a variant/type but would
    // silently catch everything, so it is `E-MATCH-BARE-VARIANT` rather than a bare binding.
    let err = parser("Circle").parse_pattern().unwrap_err();
    assert_eq!(err.code, Some("E-MATCH-BARE-VARIANT"));
}

#[test]
fn parses_types() {
    match ty("int") {
        Type::Named { name, args, .. } => {
            assert_eq!(name, "int");
            assert!(args.is_empty());
        }
        other => panic!("got {other:?}"),
    }
    match ty("List<Shape>") {
        Type::Named { name, args, .. } => {
            assert_eq!(name, "List");
            assert_eq!(args.len(), 1);
        }
        other => panic!("got {other:?}"),
    }
    match ty("Map<string, int>") {
        Type::Named { name, args, .. } => {
            assert_eq!(name, "Map");
            assert_eq!(args.len(), 2);
        }
        other => panic!("got {other:?}"),
    }
    assert!(matches!(ty("int?"), Type::Optional { .. }));
    // nested generics
    match ty("List<Map<string, int>>") {
        Type::Named { name, args, .. } => {
            assert_eq!(name, "List");
            assert_eq!(args.len(), 1);
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_function_type_annotation() {
    // a function-typed parameter must parse
    let result = parser("package Main; function apply(int x, (int) -> int f) -> int { return x; }")
        .parse_program();
    assert!(
        result.is_ok(),
        "function-typed param should parse: {result:?}"
    );
    // nested + zero-arg
    let result2 = parser("package Main; function f() -> () -> int { }").parse_program();
    assert!(
        result2.is_ok(),
        "zero-arg function type should parse: {result2:?}"
    );
    // direct type parsing
    match ty("(int) -> int") {
        Type::Function { params, ret, .. } => {
            assert_eq!(params.len(), 1);
            assert!(matches!(ret.as_ref(), Type::Named { name, .. } if name == "int"));
        }
        other => panic!("expected Type::Function, got {other:?}"),
    }
}

#[test]
fn parses_fat_arrow_function_type() {
    // A-1: function types now use `=>` (`(int) => int`); `->` stays as a transition alias.
    match ty("(int) => int") {
        Type::Function { params, ret, .. } => {
            assert_eq!(params.len(), 1);
            assert!(matches!(ret.as_ref(), Type::Named { name, .. } if name == "int"));
        }
        other => panic!("expected Type::Function, got {other:?}"),
    }
    // multi-param + nested return, fat-arrow form
    assert!(matches!(ty("(int, string) => bool"), Type::Function { .. }));
    assert!(matches!(ty("() => (int) => bool"), Type::Function { .. }));
    // the old `->` form still parses (alias)
    assert!(matches!(ty("(int) -> int"), Type::Function { .. }));
}

#[test]
fn parses_fixed_length_list_type() {
    match ty("[int; 3]") {
        Type::FixedList { elem, len, .. } => {
            assert_eq!(len, 3);
            assert!(matches!(elem.as_ref(), Type::Named { name, .. } if name == "int"));
        }
        other => panic!("expected Type::FixedList, got {other:?}"),
    }
    // nests inside generic args and is itself optional-able
    assert!(matches!(
        ty("[List<int>; 2]"),
        Type::FixedList { len: 2, .. }
    ));
    assert!(matches!(ty("[int; 2]?"), Type::Optional { .. }));
    // a non-integer / negative length is a parse error
    assert!(parser("[int; x]").parse_type().is_err());
    assert!(parser("[int]").parse_type().is_err()); // missing `; N`
}

#[test]
fn parses_parenthesized_return_position_function_type() {
    // spec #8: a parenthesized function type in return position. `() -> ((int) -> bool)` must parse
    // to the same type as the parens-free `() -> (int) -> bool` — a fn returning a fn.
    for src in ["() -> ((int) -> bool)", "() -> (int) -> bool"] {
        match ty(src) {
            Type::Function { params, ret, .. } => {
                assert!(params.is_empty(), "{src}");
                assert!(
                    matches!(ret.as_ref(), Type::Function { params, .. } if params.len() == 1),
                    "{src}: ret should be a 1-param fn, got {ret:?}"
                );
            }
            other => panic!("{src}: expected Type::Function, got {other:?}"),
        }
    }
    // A grouped type `(T)` ≡ `T` (parens used purely for grouping, no `->`).
    assert!(matches!(ty("(int)"), Type::Named { name, .. } if name == "int"));
    assert!(matches!(ty("(A | B)"), Type::Union { .. }));
    // DEC-288: `(A, B[, …])` without a `=>` is a TUPLE type (2+ members). A unit-paren `()` without
    // a `=>` is still an error (an empty paren list is only a function-type parameter list).
    assert!(matches!(ty("(int, string)"), Type::Tuple(members, _) if members.len() == 2));
    assert!(matches!(ty("(int, string, bool)"), Type::Tuple(members, _) if members.len() == 3));
    assert!(parser("()").parse_type().is_err());
}

#[test]
fn parse_function_type_throws_clause() {
    // DEC-222: a function type may carry a `throws` clause after the return type. Absent ⇒ empty.
    match ty("(int) => string throws MyError") {
        Type::Function { throws, ret, .. } => {
            assert_eq!(throws.len(), 1, "one thrown type");
            assert!(matches!(&throws[0], Type::Named { name, .. } if name == "MyError"));
            assert!(matches!(ret.as_ref(), Type::Named { name, .. } if name == "string"));
        }
        other => panic!("expected function type, got {other:?}"),
    }
    // No clause ⇒ empty throws (byte-identical to the pre-DEC-222 shape).
    match ty("(int) => string") {
        Type::Function { throws, .. } => assert!(throws.is_empty()),
        other => panic!("expected function type, got {other:?}"),
    }
    // Comma / union forms both parse (flattened by the checker).
    match ty("() => int throws A, B") {
        Type::Function { throws, .. } => assert_eq!(throws.len(), 2),
        other => panic!("expected function type, got {other:?}"),
    }
}

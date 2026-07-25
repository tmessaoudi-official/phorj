//! Parser tests — functions: return-type syntax, decls, and variadic params.

use super::super::support::*;

// ── A-1: `:` return-type syntax (PHP/TS); `->` kept as a silent transition alias ──

#[test]
fn parses_colon_return_type_on_function() {
    // A-1: `function f(): T` — the new canonical return-type syntax.
    match item("function area(Shape s): float { return s; }") {
        Item::Function(f) => {
            assert_eq!(f.name, "area");
            assert!(f.ret.is_some());
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_colon_return_type_on_method_and_interface() {
    // A-1: methods (via parse_function) and interface signatures accept `:` too.
    match &prog("package Main;\nclass C { function m(int x): int { return x; } }").items[0] {
        Item::Class(c) => match &c.members[0] {
            ClassMember::Method(f) => assert!(f.ret.is_some()),
            other => panic!("expected method, got {other:?}"),
        },
        other => panic!("expected class, got {other:?}"),
    }
    match &prog("package Main;\ninterface I { function m(): int; }").items[0] {
        Item::Interface(_) => {}
        other => panic!("expected interface, got {other:?}"),
    }
}

#[test]
fn arrow_return_type_still_parses_as_transition_alias() {
    // A-1: `->` is retained (silently) so the ~190 inline test programs keep parsing during the
    // migration; `.phg` sources are codemodded to `:`. (Full `->` removal is a tracked follow-up.)
    match item("function f() -> int { return 1; }") {
        Item::Function(f) => assert!(f.ret.is_some()),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_function_decl() {
    match item("function area(Shape s) -> float { return s; }") {
        Item::Function(f) => {
            assert_eq!(f.name, "area");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "s");
            assert!(f.ret.is_some());
            assert_eq!(f.body.len(), 1);
            assert!(f.modifiers.is_empty());
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_function_no_ret_no_params() {
    // The PARSER stays permissive: a function with no `-> T` parses with `ret == None`. The
    // return-type *mandate* (S0b, `E-MISSING-RETURN-TYPE`) is a CHECKER rule, not a parser one.
    match item("function main() { Output.printLine(1); }") {
        Item::Function(f) => {
            assert_eq!(f.name, "main");
            assert!(f.params.is_empty());
            assert!(f.ret.is_none());
            assert_eq!(f.body.len(), 1);
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn variadic_param_parses_and_sets_the_flag() {
    // DEC-298: `int ...nums` parses (the `...` sits between element type and name) and sets
    // `Param.variadic`. Semantics (List<T> + call-collection) are the checker's job; the parser just
    // records the flag. (Method/lambda variadics parse too but are rejected at check — free-fn only v1.)
    use crate::ast::Item;
    let prog = parser("package Main; function sum(int ...nums) -> int { return 0; }")
        .parse_program()
        .expect("parse ok");
    let f = prog
        .items
        .iter()
        .find_map(|it| match it {
            Item::Function(f) if f.name == "sum" => Some(f),
            _ => None,
        })
        .expect("sum function");
    assert!(f.params.last().unwrap().variadic, "last param is variadic");
}

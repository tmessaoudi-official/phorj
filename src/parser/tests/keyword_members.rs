//! DEC-531 (scout row 5k): a phorj keyword is a legal MEMBER name where nothing else can stand —
//! after `function` in a class-like body, as a field or promoted-parameter name, after `.`/`?.`/`::`
//! and `parent.`, as a `with { f = … }` field and as a named-argument name. Everywhere else it stays
//! reserved, which the second half of this file pins.

use super::support::*;

fn class_members(src: &str) -> Vec<ClassMember> {
    match item(src) {
        Item::Class(c) => c.members,
        other => panic!("expected a class, got {other:?}"),
    }
}

#[test]
fn a_keyword_names_a_method_a_field_and_a_promoted_parameter() {
    let members = class_members(
        "class C { \
           public int class = 0; \
           constructor(public string type, private mutable int match) {} \
           public static function match(int x): int { return x; } \
           function new(): int { return 1; } \
         }",
    );
    let names: Vec<&str> = members
        .iter()
        .map(|m| match m {
            ClassMember::Field { name, .. } => name.as_str(),
            ClassMember::Method(f) => f.name.as_str(),
            ClassMember::Constructor { .. } => "<ctor>",
            other => panic!("unexpected member {other:?}"),
        })
        .collect();
    assert_eq!(names, ["class", "<ctor>", "match", "new"]);
    match &members[1] {
        ClassMember::Constructor { params, .. } => {
            assert_eq!(params[0].name, "type");
            assert_eq!(params[1].name, "match");
        }
        other => panic!("member 1: {other:?}"),
    }
}

#[test]
fn a_keyword_names_an_interface_method() {
    let src = "interface I { function open(): void; function match(int x): int; }";
    match item(src) {
        Item::Interface(i) => {
            let names: Vec<&str> = i.methods.iter().map(|m| m.name.as_str()).collect();
            assert_eq!(names, ["open", "match"]);
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn a_keyword_follows_a_member_separator() {
    assert_eq!(sexpr(&expr("c.match(5)")), "c.match(5)");
    assert_eq!(sexpr(&expr("this.type")), "this.type");
    assert_eq!(sexpr(&expr("C::open()")), "C.open()");
    assert_eq!(sexpr(&expr("c?.class")), "c?.class");
    match expr("parent.match(1)") {
        Expr::ParentCall { method, .. } => assert_eq!(method, "match"),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn a_keyword_names_a_with_field_and_a_named_argument() {
    match expr("c with { type = 2 }") {
        Expr::CloneWith { fields, .. } => assert_eq!(fields[0].0, "type"),
        other => panic!("got {other:?}"),
    }
    let Expr::New(call, _) = expr("new C(type: 1, name: 2)") else {
        panic!("expected `new`");
    };
    match *call {
        Expr::Call { args, .. } => match &args[0] {
            Expr::NamedArg { name, .. } => assert_eq!(name, "type"),
            other => panic!("arg 0: {other:?}"),
        },
        other => panic!("got {other:?}"),
    }
}

// ── still reserved ──────────────────────────────────────────────────────────────────────────

#[test]
fn a_keyword_still_cannot_name_a_top_level_function() {
    assert!(prog_err("function match(): void {}").contains("expected a function name"));
}

#[test]
fn a_keyword_still_cannot_name_a_local_or_a_plain_parameter() {
    let local = prog_err("function f(): void { var type = 1; }");
    assert!(
        local.contains("TypeKw") || local.contains("type"),
        "{local}"
    );
    assert!(
        prog_err("function f(string type): void {}").contains("expected a parameter name"),
        "a plain parameter is a local"
    );
    assert!(
        prog_err("class C { constructor(string type) {} }").contains("expected a parameter name"),
        "an UNPROMOTED constructor parameter is a local too"
    );
}

#[test]
fn constructor_stays_reserved_as_a_member_name() {
    // `parent.constructor(…)` is the parent-constructor call, so a method of that name would be
    // unreachable through it — the keyword stays reserved in member position.
    assert!(prog_err("class C { function constructor(): void {} }").contains("a function name"));
    assert!(prog_err("class C { int constructor = 1; }").contains("a field name"));
}

#[test]
fn a_keyword_in_expression_position_keeps_its_meaning() {
    // The member arm must not swallow a keyword that STARTS an expression.
    assert!(matches!(expr("new C()"), Expr::New(..)));
    match expr("f(true)") {
        Expr::Call { args, .. } => assert!(matches!(args[0], Expr::Bool(true, _))),
        other => panic!("got {other:?}"),
    }
}

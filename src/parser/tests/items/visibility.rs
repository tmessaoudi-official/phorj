//! Parser tests — item visibility prefixes and the `use` dot-lookahead split.

use super::super::support::*;

#[test]
fn parses_private_class_visibility() {
    match &prog("package Main;\nprivate class P {}").items[0] {
        Item::Class(c) => assert_eq!(c.vis, Visibility::Private),
        other => panic!("expected class, got {other:?}"),
    }
}

#[test]
fn parses_internal_function_visibility() {
    match &prog("package Main;\ninternal function f() -> void {}").items[0] {
        Item::Function(f) => assert_eq!(f.vis, Visibility::Internal),
        other => panic!("expected function, got {other:?}"),
    }
}

#[test]
fn parses_internal_enum_and_interface_visibility() {
    match &prog("package Main;\ninternal enum E { A() }").items[0] {
        Item::Enum(e) => assert_eq!(e.vis, Visibility::Internal),
        other => panic!("expected enum, got {other:?}"),
    }
    match &prog("package Main;\nprivate interface I { function m() -> int; }").items[0] {
        Item::Interface(i) => assert_eq!(i.vis, Visibility::Private),
        other => panic!("expected interface, got {other:?}"),
    }
}

#[test]
fn bare_decl_defaults_to_public() {
    match &prog("package Main;\nclass C {}").items[0] {
        Item::Class(c) => assert_eq!(c.vis, Visibility::Public),
        other => panic!("expected class, got {other:?}"),
    }
}

#[test]
fn s8_use_dot_lookahead_splits_trait_from_resolution() {
    // M-RT S8 D9: `use T;` (no dot) is trait composition; `use A.foo` (dot) is an S6b resolution
    // clause. Both can appear in the same class body and must land in the right buckets.
    match &prog(
        "package Main;\nopen class A { open function foo() -> int { return 1; } }\n\
             trait T { function bar() -> int { return 2; } }\n\
             class C extends A { use T; use A.foo }",
    )
    .items
    .last()
    .unwrap()
    {
        Item::Class(c) => {
            assert_eq!(c.uses.len(), 1, "one trait `use`");
            assert_eq!(c.uses[0].name, "T");
            assert_eq!(c.resolutions.len(), 1, "one resolution clause");
        }
        other => panic!("expected class, got {other:?}"),
    }
}

#[test]
fn explicit_public_enum_parses() {
    match &prog("package Main;\npublic enum E { A() }").items[0] {
        Item::Enum(e) => assert_eq!(e.vis, Visibility::Public),
        other => panic!("expected enum, got {other:?}"),
    }
}

#[test]
fn conflicting_visibility_prefix_is_rejected() {
    let err = prog_err("package Main;\npublic private class C {}");
    assert!(err.contains("a single visibility"), "got: {err}");
}

#[test]
fn visibility_on_import_is_rejected() {
    let err = prog_err("package Main;\nprivate import Core.Output;");
    assert!(err.contains("cannot carry a visibility"), "got: {err}");
}

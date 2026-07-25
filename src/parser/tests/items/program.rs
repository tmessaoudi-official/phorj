//! Parser tests — program/package structure and `test "name" { … }` items (M-Test T1).

use super::super::support::*;

#[test]
fn parses_package_declaration() {
    // `package a.b;` is captured on the Program, not as an Item (M5 S1).
    let prog = parser("package app.util; function main() -> void {}")
        .parse_program()
        .expect("parse ok");
    assert_eq!(prog.package, vec!["app".to_string(), "util".to_string()]);
    // A bare file parses with an empty package — the checker, not the parser, enforces presence.
    let bare = parser("function main() -> void {}")
        .parse_program()
        .expect("parse ok");
    assert!(bare.package.is_empty());
    // `package` after another item is a parse error (it must be the first declaration).
    assert!(parser("function main() -> void {} package app;")
        .parse_program()
        .is_err());
}

#[test]
fn parses_program_multiple_items() {
    let src = "import Core.Output; enum E { A, } function main() -> void { return; }";
    let prog = parser(src).parse_program().expect("parse ok");
    assert_eq!(prog.items.len(), 3);
    assert!(matches!(prog.items[0], Item::Import { .. }));
    assert!(matches!(prog.items[1], Item::Enum(_)));
    assert!(matches!(prog.items[2], Item::Function(_)));
}

#[test]
fn empty_program_parses() {
    let prog = parser("").parse_program().expect("parse ok");
    assert!(prog.items.is_empty());
}

// --- M-Test T1: `test "name" { … }` item ---------------------------------------------------------

#[test]
fn parses_test_item() {
    match item("test \"addition works\" { var x = 2 + 2; }") {
        Item::Test { name, body, .. } => {
            assert_eq!(name, "addition works");
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected a test item, got {other:?}"),
    }
}

#[test]
fn parses_empty_test_item() {
    match item("test \"nothing yet\" {}") {
        Item::Test { name, body, .. } => {
            assert_eq!(name, "nothing yet");
            assert!(body.is_empty());
        }
        other => panic!("expected a test item, got {other:?}"),
    }
}

#[test]
fn test_is_a_contextual_keyword() {
    // `test` stays usable as an ordinary identifier (a local variable here), because it is special
    // only at item position when immediately followed by a string literal.
    let p = prog("package Main;\nfunction main() -> void { var test = 3; }");
    assert!(matches!(&p.items[0], Item::Function(_)));
}

#[test]
fn test_item_rejects_visibility_modifier() {
    assert!(parser("public test \"x\" {}").parse_item().is_err());
}

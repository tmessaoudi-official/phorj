//! Parser tests — enum declarations (algebraic + DEC-302 backed enums).

use super::super::support::*;

#[test]
fn parses_enum_decl() {
    let src = "enum Shape { Circle(float radius), Rect(float w, float h), Unit, }";
    match item(src) {
        Item::Enum(e) => {
            assert_eq!(e.name, "Shape");
            assert_eq!(e.variants.len(), 3);
            assert_eq!(e.variants[0].name, "Circle");
            assert_eq!(e.variants[0].fields.len(), 1);
            assert_eq!(e.variants[1].fields.len(), 2);
            assert!(e.variants[2].fields.is_empty()); // bare variant
            assert!(e.backing_type.is_none()); // DEC-302: a normal algebraic enum is not backed
            assert!(e.variants[0].backing_value.is_none());
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_backed_enum_decl() {
    // DEC-302: `enum Name: BackingType { Variant = value, … }` (PHP 8.1 backed enum).
    let src = "enum Suit: string { Hearts = \"H\", Spades = \"S\" }";
    match item(src) {
        Item::Enum(e) => {
            assert_eq!(e.name, "Suit");
            assert!(
                e.backing_type.is_some(),
                "backing type `: string` must parse"
            );
            assert_eq!(e.variants.len(), 2);
            assert_eq!(e.variants[0].name, "Hearts");
            assert!(e.variants[0].fields.is_empty()); // backed variants are payload-less
            assert!(
                e.variants[0].backing_value.is_some(),
                "variant `= \"H\"` value must parse"
            );
            assert!(e.variants[1].backing_value.is_some());
        }
        other => panic!("got {other:?}"),
    }
}

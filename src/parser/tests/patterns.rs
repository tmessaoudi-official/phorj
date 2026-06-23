//! Parser tests — patterns (M-Decomp W3.1b, mirrors the source clusters).

use super::support::*;

#[test]
fn parses_patterns() {
    assert!(matches!(pat("_"), Pattern::Wildcard(_)));
    match pat("x") {
        Pattern::Binding { name, .. } => assert_eq!(name, "x"),
        other => panic!("got {other:?}"),
    }
    assert!(matches!(pat("42"), Pattern::Int(42, _)));
    assert!(matches!(pat("true"), Pattern::Bool(true, _)));
    assert!(matches!(pat("null"), Pattern::Null(_)));
    // variant destructure
    match pat("Circle(r)") {
        Pattern::Variant { name, fields, .. } => {
            assert_eq!(name, "Circle");
            assert_eq!(fields.len(), 1);
            assert!(matches!(&fields[0], Pattern::Binding { name, .. } if name == "r"));
        }
        other => panic!("got {other:?}"),
    }
    match pat("Rect(w, h)") {
        Pattern::Variant { name, fields, .. } => {
            assert_eq!(name, "Rect");
            assert_eq!(fields.len(), 2);
        }
        other => panic!("got {other:?}"),
    }
    // nested variant patterns
    match pat("Wrap(Circle(r))") {
        Pattern::Variant { fields, .. } => {
            assert!(matches!(&fields[0], Pattern::Variant { .. }))
        }
        other => panic!("got {other:?}"),
    }
}

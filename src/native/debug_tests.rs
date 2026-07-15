//! `Core.Debug` renderer unit tests (sibling file per Invariant 13): the v1 format is a versioned
//! contract (the PHP twin renders byte-identically), so these tests PIN it.

use super::*;
use crate::value::{ClassLayout, EnumVal, Instance};
use std::rc::Rc;

fn r(v: &Value) -> String {
    render(v, 0, &mut Vec::new())
}

#[test]
fn scalars_ride_the_canonical_kernel_and_strings_quote() {
    assert_eq!(r(&Value::Int(42)), "42");
    assert_eq!(r(&Value::Bool(true)), "true");
    assert_eq!(r(&Value::Null), "null");
    assert_eq!(r(&Value::Unit), "void");
    assert_eq!(r(&Value::Str("a\"b\\c\nd".into())), "\"a\\\"b\\\\c\\nd\"");
}

#[test]
fn short_containers_inline_long_ones_wrap() {
    let short = Value::List(Rc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
    assert_eq!(r(&short), "[1, 2, 3]");
    let long = Value::List(Rc::new(
        (0..12)
            .map(|i| Value::Str(format!("item-{i}").into()))
            .collect(),
    ));
    let out = r(&long);
    assert!(out.starts_with("[\n    \"item-0\""), "{out}");
    assert!(out.ends_with("\n]"), "{out}");
    let map = Value::Map(Rc::new(vec![(
        crate::value::HKey::Str("k".into()),
        Value::Int(1),
    )]));
    assert_eq!(r(&map), "{ \"k\" => 1 }");
}

#[test]
fn instances_render_class_and_sorted_fields() {
    let layout = ClassLayout::new(vec!["age".into(), "name".into()]);
    let inst = Instance::new("User".into(), layout);
    inst.set_field("name", Value::Str("Ada".into()));
    inst.set_field("age", Value::Int(36));
    let out = r(&Value::Instance(Rc::new(inst)));
    assert_eq!(out, "User { age: 36, name: \"Ada\" }");
}

#[test]
fn enums_render_qualified_with_payload() {
    let bare = Value::Enum(Rc::new(EnumVal {
        ty: "Color".into(),
        variant: "Red".into(),
        payload: vec![],
    }));
    assert_eq!(r(&bare), "Color.Red");
    let with = Value::Enum(Rc::new(EnumVal {
        ty: "Json".into(),
        variant: "Int".into(),
        payload: vec![Value::Int(7)],
    }));
    assert_eq!(r(&with), "Json.Int(7)");
}

#[test]
fn cycles_cut_as_recursion_but_dags_render_twice() {
    // A true cycle is impossible to build from safe constructors here, so simulate the DAG half:
    // the same Rc list appearing twice renders twice (only genuine cycles cut).
    let shared = Rc::new(vec![Value::Int(1)]);
    let dag = Value::List(Rc::new(vec![
        Value::List(shared.clone()),
        Value::List(shared),
    ]));
    assert_eq!(r(&dag), "[[1], [1]]");
}

#[test]
fn bytes_hex_and_truncation() {
    assert_eq!(r(&Value::Bytes(Rc::new(vec![1, 2, 0xff]))), "b\"0102ff\"");
    let big = Value::Bytes(Rc::new(vec![0xab; 40]));
    let out = r(&big);
    assert!(out.ends_with("\" (+8 more)"), "{out}");
}

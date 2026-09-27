//! `Regex.first` / `Regex.all` (DEC-554): a match answers by position (`texts`, group 0 = the whole
//! match), by name (`groups`), and with BYTE offsets (`starts`); a group that did not take part is
//! `null` in both positional lists and absent from `groups`.

use super::captures::{regex_all, regex_first};
use super::natives::regex_compile;
use crate::value::Value;

fn re(pattern: &str) -> Value {
    regex_compile(&[Value::Str(pattern.into())], &mut String::new()).expect("valid pattern")
}

/// A field of a `RegexMatch` instance, rendered: strings bare, ints as digits, `null` as `-`.
fn field(m: &Value, name: &str) -> Vec<String> {
    let Value::Instance(inst) = m else {
        panic!("expected a RegexMatch, got {m:?}")
    };
    assert_eq!(inst.class.as_ref(), "RegexMatch");
    let render = |v: &Value| match v {
        Value::Str(s) => s.as_str().to_string(),
        Value::Int(n) => n.to_string(),
        Value::Null => "-".to_string(),
        other => panic!("unexpected {other:?}"),
    };
    match inst.get_field(name).expect(name) {
        Value::List(xs) => xs.iter().map(render).collect(),
        Value::Map(m) => {
            let mut kv: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        crate::value::HKey::Str(s) => s.as_str().to_string(),
                        other => format!("{other:?}"),
                    };
                    format!("{key}={}", render(v))
                })
                .collect();
            kv.sort();
            kv
        }
        other => vec![render(&other)],
    }
}

const RANGE: &str = r"(?<lo>\d+)-(\d+)(?:/(\w+))?";
const TEXT: &str = "rooms 10-20 and 30-40/b";

#[test]
fn first_answers_by_position_by_name_and_by_byte_offset() {
    let m = regex_first(&[re(RANGE), Value::Str(TEXT.into())], &mut String::new()).unwrap();
    assert_eq!(field(&m, "matched"), ["10-20"]);
    assert_eq!(field(&m, "texts"), ["10-20", "10", "20", "-"]);
    assert_eq!(field(&m, "starts"), ["6", "6", "9", "-"]);
    assert_eq!(field(&m, "groups"), ["lo=10"]);
}

#[test]
fn first_is_null_without_a_match() {
    let m = regex_first(&[re(RANGE), Value::Str("none".into())], &mut String::new()).unwrap();
    assert!(matches!(m, Value::Null), "{m:?}");
}

/// Offsets count BYTES — `é` is two — the unit `String.substring` uses.
#[test]
fn offsets_are_bytes() {
    let m = regex_first(&[re("x"), Value::Str("éx".into())], &mut String::new()).unwrap();
    assert_eq!(field(&m, "starts"), ["2"]);
}

#[test]
fn all_returns_every_match_left_to_right() {
    let v = regex_all(&[re(RANGE), Value::Str(TEXT.into())], &mut String::new()).unwrap();
    let Value::List(ms) = v else {
        panic!("expected a list, got {v:?}")
    };
    assert_eq!(ms.len(), 2);
    assert_eq!(field(&ms[0], "texts"), ["10-20", "10", "20", "-"]);
    assert_eq!(field(&ms[1], "texts"), ["30-40/b", "30", "40", "b"]);
    assert_eq!(field(&ms[1], "starts"), ["16", "16", "19", "22"]);
    let none = regex_all(&[re(RANGE), Value::Str("none".into())], &mut String::new()).unwrap();
    assert!(
        matches!(&none, Value::List(xs) if xs.is_empty()),
        "{none:?}"
    );
}

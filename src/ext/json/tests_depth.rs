//! Nesting depth follows PHP's `json_decode` default (audit 2026-10-07, A3). Measured on php-8.5.11:
//! 511 nested arrays or objects decode, 512 return `null` ("Maximum stack depth exceeded"), because
//! PHP's default `$depth = 512` counts the innermost value's level too. The literals below pin that
//! measurement, independently of the constant the parser uses.

use super::parser::{parse_json, validate_json};

fn nested_arrays(n: usize) -> String {
    format!("{}{}", "[".repeat(n), "]".repeat(n))
}

fn nested_objects(n: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n))
}

#[test]
fn json_accepts_511_nested_containers_like_php() {
    for doc in [nested_arrays(511), nested_objects(511)] {
        assert!(
            parse_json(&doc).is_some(),
            "eager parser rejected depth 511"
        );
        assert!(
            validate_json(&doc).is_some(),
            "lazy validator rejected depth 511"
        );
    }
}

#[test]
fn json_rejects_512_nested_containers_like_php() {
    for doc in [nested_arrays(512), nested_objects(512)] {
        assert!(
            parse_json(&doc).is_none(),
            "eager parser accepted depth 512"
        );
        assert!(
            validate_json(&doc).is_none(),
            "lazy validator accepted depth 512"
        );
    }
}

#[test]
fn json_rejects_a_hostile_depth_without_exhausting_the_stack() {
    // 200 000 levels used to parse (on the 256 MB pipeline thread) — no bound at all.
    let doc = nested_arrays(200_000);
    assert!(parse_json(&doc).is_none());
    assert!(validate_json(&doc).is_none());
}

//! Checker tests — the catch-all arm of a `match` (split out of `matching.rs`, Invariant 13).

use super::support::*;

/// Parity review B-F2: the non-exhaustive diagnostic told the user to add a `_` arm, which the parser
/// rejects (DEC-209: `default` is the sole standalone catch-all; `_` is an ignore-placeholder only).
#[test]
fn non_exhaustive_match_on_a_non_enum_names_the_default_arm_not_underscore() {
    let e = errors_of("function f(int x) -> string { return match x { 1 => \"a\" }; }");
    let msg = e
        .iter()
        .map(|d| d.message.as_str())
        .find(|m| m.contains("non-exhaustive"))
        .unwrap_or_else(|| panic!("no non-exhaustive diagnostic: {e:?}"));
    assert!(
        msg.contains("`default`"),
        "must name the real catch-all keyword: {msg}"
    );
    assert!(
        !msg.contains("`_`"),
        "must not suggest the rejected `_` arm: {msg}"
    );
}

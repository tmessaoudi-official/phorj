//! Named-argument diagnostics (`greet(name: "Ada")`). All four had emit sites and no assertion.
use super::support::*;

const GREET: &str =
    "function greet(string name, string greeting = \"Hi\") -> string { return greeting + name; }";

fn has(body: &str, code: &str) {
    let e = errors_of(&format!("{GREET} function main() -> void {{ {body} }}"));
    assert!(
        e.iter().any(|d| d.code == Some(code)),
        "expected {code}, got {e:?}"
    );
}

#[test]
fn an_unknown_parameter_name_is_rejected() {
    has("string s = greet(nom: \"Ada\");", "E-NAMED-ARG-UNKNOWN");
}

#[test]
fn a_parameter_cannot_be_supplied_twice() {
    has(
        "string s = greet(\"Ada\", name: \"Bo\");",
        "E-NAMED-ARG-DUPLICATE",
    );
}

#[test]
fn a_positional_argument_cannot_follow_a_named_one() {
    has(
        "string s = greet(greeting: \"Hey\", \"Ada\");",
        "E-NAMED-ARG-POSITIONAL-AFTER",
    );
}

#[test]
fn a_required_parameter_left_unsupplied_is_named() {
    has(
        "string s = greet(greeting: \"Hey\");",
        "E-NAMED-ARG-MISSING",
    );
}

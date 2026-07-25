//! Parser tests — imports: plain, multi-segment/aliased, grouped, and wildcard forms.

use super::super::support::*;

#[test]
fn parses_import() {
    match item("import Core.Output;") {
        Item::Import { path, .. } => assert_eq!(path, vec!["Core", "Output"]),
        other => panic!("got {other:?}"),
    }
    match item("import a;") {
        Item::Import { path, .. } => assert_eq!(path, vec!["a"]),
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_multisegment_and_aliased_import() {
    // A variant-path import (DEC-186) — three segments — parses to a full path.
    match item("import Core.Result.Success;") {
        Item::Import { path, alias, .. } => {
            assert_eq!(path, vec!["Core", "Result", "Success"]);
            assert_eq!(alias, None);
        }
        other => panic!("got {other:?}"),
    }
    match item("import Core.Result.Success as MyOk;") {
        Item::Import { path, alias, .. } => {
            assert_eq!(path, vec!["Core", "Result", "Success"]);
            assert_eq!(alias, Some("MyOk".to_string()));
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_grouped_import_expands_to_one_per_member() {
    // `import P.{ a, b as c };` (DEC-186) expands to one `Item::Import` per member, in source order,
    // each with `path = prefix + [leaf]` and the per-item alias. Multi-line + trailing comma allowed.
    let src = "package Main; import Core.Result.{ Success, Failure as Xzs }; \
               import Core.Option.{\n  Some,\n  None,\n}; \
               function main() -> void {}";
    let prog = parser(src).parse_program().expect("parse ok");
    let imports: Vec<(&Vec<String>, &Option<String>)> = prog
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Import { path, alias, .. } => Some((path, alias)),
            _ => None,
        })
        .collect();
    assert_eq!(imports.len(), 4, "two groups of two expand to four imports");
    assert_eq!(imports[0].0, &vec!["Core", "Result", "Success"]);
    assert_eq!(imports[0].1, &None);
    assert_eq!(imports[1].0, &vec!["Core", "Result", "Failure"]);
    assert_eq!(imports[1].1, &Some("Xzs".to_string()));
    assert_eq!(imports[2].0, &vec!["Core", "Option", "Some"]);
    assert_eq!(imports[3].0, &vec!["Core", "Option", "None"]);
    assert_eq!(imports[3].1, &None);
}

#[test]
fn empty_import_group_is_a_parse_error() {
    assert!(parser("package Main; import Core.Result.{};")
        .parse_program()
        .is_err());
}

#[test]
fn parses_wildcard_import() {
    // Q-A: `import X.Y.*;` parses to a single wildcard `Item::Import` whose path is the PACKAGE PREFIX
    // (the loader expands it later). No alias, no except.
    match item("import Acme.Http.*;") {
        Item::Import {
            path,
            alias,
            wildcard,
            except,
            ..
        } => {
            assert_eq!(path, vec!["Acme", "Http"]);
            assert_eq!(alias, None);
            assert!(wildcard, "expected a wildcard import");
            assert!(except.is_empty());
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn parses_wildcard_import_with_except() {
    // Q-A: `import X.Y.* except { A, B };` — the except set is captured (multi-line + trailing comma ok).
    match item("import Acme.Geometry.* except {\n  Circle,\n  Square,\n};") {
        Item::Import {
            path,
            wildcard,
            except,
            ..
        } => {
            assert_eq!(path, vec!["Acme", "Geometry"]);
            assert!(wildcard);
            assert_eq!(except, vec!["Circle".to_string(), "Square".to_string()]);
        }
        other => panic!("got {other:?}"),
    }
}

#[test]
fn wildcard_import_alias_is_a_parse_error() {
    // Q-A: `import X.* as Y;` is E-WILDCARD-ALIAS — a flat wildcard has no single name to bind.
    let d = parser("package Main; import Acme.Http.* as H;")
        .parse_program()
        .expect_err("alias on a wildcard is rejected");
    assert_eq!(d.code, Some("E-WILDCARD-ALIAS"));
    // F2 (round-1 review): the message must read as prose, NOT be mangled by `Parser::error`'s
    // "expected … found …" wrapper.
    assert!(
        d.message
            .starts_with("a wildcard import `X.*` cannot be aliased"),
        "message garbled by the expected/found wrapper: {:?}",
        d.message
    );
    assert!(
        !d.message.contains("expected"),
        "must not be wrapped: {:?}",
        d.message
    );
}

#[test]
fn bare_core_wildcard_is_a_parse_error_with_prose() {
    // Q-A / P-Q-A-1: bare `import Core.*;` is E-WILDCARD-STDLIB-ROOT (would flood the stdlib). F2:
    // the full-sentence rejection must read as prose, not wrapped in "expected … found …".
    let d = parser("package Main; import Core.*;")
        .parse_program()
        .expect_err("bare Core.* is rejected");
    assert_eq!(d.code, Some("E-WILDCARD-STDLIB-ROOT"));
    assert!(
        d.message.starts_with("`import Core.*;` is not allowed"),
        "message garbled: {:?}",
        d.message
    );
    assert!(
        !d.message.contains("expected"),
        "must not be wrapped: {:?}",
        d.message
    );
}

#[test]
fn core_submodule_wildcard_is_a_parse_error_with_prose() {
    // P-Q-A-1: a Core SUBMODULE wildcard (`import Core.Http.*;`) is deferred → E-WILDCARD-STDLIB-ROOT
    // with the "not yet supported" prose (F2: unwrapped).
    let d = parser("package Main; import Core.Http.*;")
        .parse_program()
        .expect_err("Core submodule wildcard is deferred");
    assert_eq!(d.code, Some("E-WILDCARD-STDLIB-ROOT"));
    assert!(
        d.message.contains("not yet supported"),
        "expected the deferred-submodule prose: {:?}",
        d.message
    );
    assert!(
        !d.message.starts_with("expected"),
        "must not be wrapped: {:?}",
        d.message
    );
}

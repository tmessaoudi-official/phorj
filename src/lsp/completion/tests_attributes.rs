//! Attribute-NAME completion tests (the `#[` context). Split into its own file rather than grown
//! onto `completion/tests.rs`, which sits at 439 lines against Invariant 13's 500-line hard cap.
//!
//! The gap this closes: typing `#[` offered NOTHING, uniformly, for every attribute in the language —
//! so `#[Entry]`, `#[Config]`, `#[Route]`, `#[Deprecated]`, `#[Invoke]`, `#[ToString]` and the DI set
//! were all undiscoverable from the editor. Invariant 17's 100% rule makes that an incomplete feature.
use super::complete;

/// Extract every `"label":"…"` value from a completion response (assert on CONTENT, not just count).
/// Local copy — `tests.rs`'s helper is private to that module and this file must not grow it.
fn labels(resp: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = resp;
    while let Some(i) = rest.find("\"label\":\"") {
        rest = &rest[i + 9..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    out
}

/// Complete at the END of `src` (the usual mid-edit cursor position).
fn at_end(src: &str) -> Vec<String> {
    labels(&complete(src, src.len(), None, None, &Default::default()))
}

#[test]
fn bare_open_attribute_offers_every_builtin() {
    // `#[` with nothing typed yet: the full built-in set, by canonical-path leaf.
    let got = at_end("package Main;\n#[");
    for want in [
        "Entry",
        "Config",
        "Route",
        "Deprecated",
        "Invoke",
        "ToString",
        "Attribute",
        "Injectable",
        "Provides",
        "Transient",
        "UncheckedOverflow",
    ] {
        assert!(
            got.contains(&want.to_string()),
            "missing `{want}` in {got:?}"
        );
    }
}

#[test]
fn attribute_prefix_filters_the_set() {
    // `#[Ent` narrows to `Entry` and drops the unrelated names.
    let got = at_end("package Main;\n#[Ent");
    assert!(got.contains(&"Entry".to_string()), "{got:?}");
    assert!(!got.contains(&"Route".to_string()), "{got:?}");
}

#[test]
fn attribute_completion_offers_user_declared_attributes() {
    // DEC-194: a class carrying `#[Attribute]` IS a user attribute type, so it is completable at a
    // use site exactly like a built-in. Without this the 100% rule holds only for the built-in set.
    //
    // Asserted on the DETAIL, not just the label: a bare label assertion passes even with this
    // feature absent, because general symbol completion already offers every top-level class name.
    // Only the `user-defined attribute` detail proves the item came from the attribute context.
    let src = "package Main;\n#[Attribute]\nclass Audited {}\n#[Aud";
    let resp = complete(src, src.len(), None, None, &Default::default());
    assert!(
        resp.contains(r#""label":"Audited","kind":7,"detail":"user-defined attribute""#),
        "user attribute not offered from the attribute context: {resp}"
    );
}

#[test]
fn a_class_without_the_attribute_marker_is_not_offered() {
    // The converse of the above — a PLAIN class is not an attribute type, so it must not appear in
    // the `#[` list. Without this, the feature would offer every class and the checker would then
    // reject the accepted item as `E-UNKNOWN-ATTRIBUTE`.
    let src = "package Main;\nclass Plain {}\n#[Pla";
    let resp = complete(src, src.len(), None, None, &Default::default());
    assert!(
        !resp.contains("\"label\":\"Plain\""),
        "a non-attribute class leaked into the attribute list: {resp}"
    );
}

#[test]
fn qualified_attribute_carries_a_replacing_text_edit() {
    // `.` is a client word boundary, so a dotted candidate offered as a plain label would be
    // inserted AFTER the already-typed `Core.Runtime.`, yielding `Core.Runtime.Core.Runtime.Entry`
    // — the exact trap the import context documents. The item must carry a textEdit whose newText
    // is the FULL path, replacing the typed fragment.
    let src = "package Main;\n#[Core.Runtime.En";
    let resp = complete(src, src.len(), None, None, &Default::default());
    assert!(
        resp.contains(r#""newText":"Core.Runtime.Entry""#),
        "qualified attribute item lacks a replacing textEdit: {resp}"
    );
}

#[test]
fn qualified_attribute_path_is_offered() {
    // The fully-qualified form is self-gating (needs no import), so `#[Core.Runtime.` must offer the
    // canonical paths — otherwise the import-free spelling is undiscoverable.
    let got = at_end("package Main;\n#[Core.Runtime.");
    assert!(
        got.iter().any(|l| l == "Core.Runtime.Entry"),
        "expected a canonical path in {got:?}"
    );
}

#[test]
fn index_bracket_is_not_an_attribute_context() {
    // `[` became a completion trigger character for `#[`, so an ARRAY INDEX now fires completion too.
    // It must not offer attribute names there — a wrong list is worse than none (the module's own
    // conservative-gate doctrine).
    let src = "package Main;\nfunction f(IntList xs): int {\n  return xs[";
    let got = at_end(src);
    assert!(
        !got.contains(&"Entry".to_string()),
        "attribute names leaked into an index context: {got:?}"
    );
}

#[test]
fn attribute_items_are_sorted_and_deduped() {
    // Determinism (Invariant 10): any user-facing list must be stable, and a name must not appear
    // twice when a user attribute shadows nothing.
    let got: Vec<String> = at_end("package Main;\n#[");
    let mut sorted = got.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(got, sorted, "attribute completion must be sorted + deduped");
}

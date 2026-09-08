//! Completion tests — NAMED-TUPLE receivers (DEC-504).
//!
//! Split from `tests.rs` (Invariant 13). A tuple receiver is its own case there: it is the one
//! receiver whose members are not a class's, so it takes a different branch and deserves its own
//! file next to `tests_attributes.rs` / `tests_preludes.rs`.

use super::complete;
use super::tests::labels;

/// DEC-504 — a NAMED-TUPLE receiver completes its FIELDS. Invariant 17's 100% rule: the compiler
/// knowing `t.bp` is not enough, the editor has to offer it.
///
/// A named tuple has no nominal head, so the class path this shares a branch with returns nothing
/// for it — without the tuple case, `t.` is silent. The live buffer ends in `t.`, which does not
/// parse, so this also exercises the repaired-parse fallback.
#[test]
fn a_named_tuple_receiver_completes_its_fields() {
    let src = "package Main;\nfunction main(): void {\n\
               (source: string, bp: int) t = (source: \"cli\", bp: 3);\n\
               var x = t.\n}\n";
    let got = labels(&complete(
        src,
        src.find("t.\n").unwrap() + 2,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(got.iter().any(|l| l == "bp"), "want `bp` in {got:?}");
    assert!(
        got.iter().any(|l| l == "source"),
        "want `source` in {got:?}"
    );
}

/// A POSITIONAL tuple has positions, not names, so there is nothing to complete and the editor must
/// say nothing rather than invent `0`/`1` — the same distinction the checker draws between
/// `E-TUPLE-UNKNOWN-FIELD` and `E-TUPLE-POSITIONAL-FIELD`.
#[test]
fn a_positional_tuple_receiver_completes_nothing() {
    let src = "package Main;\nfunction main(): void {\n\
               (string, int) t = (\"cli\", 3);\n\
               var x = t.\n}\n";
    let got = labels(&complete(
        src,
        src.find("t.\n").unwrap() + 2,
        None,
        None,
        &std::collections::HashMap::new(),
    ));
    assert!(got.is_empty(), "expected no items, got {got:?}");
}

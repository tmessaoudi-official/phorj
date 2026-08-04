//! Unit tests for `attributes.rs` — split out per Invariant 13: adding the SSOT enumeration
//! plus its tests pushed that file from 152 to 304 lines, past the 300-line soft cap, and
//! split-as-you-go is the default rather than growing a file and grandfathering it.
use super::*;

fn attr(name: &str) -> Attribute {
    Attribute {
        name: name.to_string(),
        args: Vec::new(),
        span: Span {
            start: 0,
            len: 0,
            line: 1,
            col: 1,
        },
    }
}

/// Every row in [`BUILTIN_ATTRIBUTE_PATHS`] must be recognized by one of the `is_*` predicates.
/// This is the direction that catches a typo'd const or a row left behind by a removed attribute —
/// completion would otherwise offer a name the checker rejects as `E-UNKNOWN-ATTRIBUTE`.
#[test]
fn every_enumerated_attribute_is_recognized() {
    for (path, detail) in BUILTIN_ATTRIBUTE_PATHS {
        let a = attr(path);
        let recognized = a.is_unchecked_overflow()
            || a.is_di_builtin()
            || a.is_di_provides()
            || a.is_di_transient()
            || a.is_attribute_marker()
            || a.is_entry()
            || a.is_deprecated()
            || a.is_config()
            || a.is_route()
            || a.is_invoke()
            || a.is_to_string();
        assert!(
            recognized,
            "`{path}` is enumerated but no predicate matches it"
        );
        assert!(!detail.is_empty(), "`{path}` has no completion detail");
    }
}

/// The BARE LEAF of every enumerated path must also be recognized — that is the idiomatic
/// use-site spelling (import-gated) and exactly what completion inserts by default. A row whose
/// leaf did not match would make the offered item uncompilable.
#[test]
fn every_enumerated_leaf_is_recognized_and_unique() {
    let mut leaves: Vec<&str> = Vec::new();
    for (path, _) in BUILTIN_ATTRIBUTE_PATHS {
        let leaf = attr_path_leaf(path);
        let a = attr(leaf);
        let recognized = a.is_unchecked_overflow()
            || a.is_di_builtin()
            || a.is_di_provides()
            || a.is_di_transient()
            || a.is_attribute_marker()
            || a.is_entry()
            || a.is_deprecated()
            || a.is_config()
            || a.is_route()
            || a.is_invoke()
            || a.is_to_string();
        assert!(
            recognized,
            "bare leaf `{leaf}` of `{path}` is not recognized"
        );
        leaves.push(leaf);
    }
    let mut sorted = leaves.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        leaves.len(),
        "two built-in attributes share a bare leaf — completion could not disambiguate: {leaves:?}"
    );
}

/// The enumeration is kept in sorted order so the completion list is deterministic
/// (Invariant 10) without the LSP having to re-sort it.
#[test]
fn enumeration_is_sorted_by_leaf() {
    let leaves: Vec<&str> = BUILTIN_ATTRIBUTE_PATHS
        .iter()
        .map(|(p, _)| attr_path_leaf(p))
        .collect();
    let mut sorted = leaves.clone();
    sorted.sort_unstable();
    assert_eq!(
        leaves, sorted,
        "keep BUILTIN_ATTRIBUTE_PATHS sorted by leaf"
    );
}

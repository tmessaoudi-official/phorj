//! The dependency-claim ratchet.
//!
//! `docs/specs/UNIFIED-SPEC.md`'s external-dependency policy warns that understated dependency
//! claims "must not be repeated" — and nothing enforced it, so `FEATURES.md` drifted back to naming
//! **11** crates while claiming "nothing else", omitting `unicode-segmentation` and all three
//! `cranelift` crates. A prose warning is not a ratchet; this is.
//!
//! Deliberately derived from `Cargo.toml` rather than from a list in this file: a hardcoded expected
//! set would be a second copy to drift, which is the bug.

use std::path::Path;

/// Every optional/required crate under `[dependencies]` (and the target-gated block) must be named
/// in `FEATURES.md`'s dependency paragraph, and `THIRD-PARTY-NOTICES.md` must carry it too.
#[test]
fn every_admitted_crate_is_named_in_features_md() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");

    // Collect dependency NAMES: lines of the form `name = …` inside `[dependencies]` or a
    // `[target.'…'.dependencies]` block, stopping at the next section header.
    let mut names: Vec<String> = Vec::new();
    let mut in_deps = false;
    for line in cargo.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]"
                || (t.starts_with("[target.") && t.ends_with(".dependencies]"));
            continue;
        }
        if !in_deps || t.starts_with('#') || t.is_empty() {
            continue;
        }
        if let Some((name, _)) = t.split_once('=') {
            let name = name.trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    assert!(
        names.len() >= 10,
        "parsed only {} dependencies from Cargo.toml — the parser broke, which would make this \
         ratchet pass vacuously: {names:?}",
        names.len()
    );

    let features_all = std::fs::read_to_string(root.join("FEATURES.md")).expect("FEATURES.md");
    // Scope to the dependency BULLET, not the whole file. A file-wide `contains` is worthless here:
    // `unicode-segmentation` and `cranelift` are named in other rows, so deleting them from the list
    // still passed — verified by sabotage, and the same too-weak-assert shape that let a `Time.sleep`
    // guard pass while the PHP leg slept for a real hour.
    let start = features_all
        .find("- **Std-first with a short, vetted, feature-gated dependency list")
        .expect("FEATURES.md must carry the dependency bullet");
    let rest = &features_all[start + 2..];
    let end = rest
        .find("\n- **")
        .map_or(features_all.len(), |i| start + 2 + i);
    let features = &features_all[start..end];
    let notices = std::fs::read_to_string(root.join("THIRD-PARTY-NOTICES.md"))
        .expect("THIRD-PARTY-NOTICES.md");

    let missing_features: Vec<&String> = names
        .iter()
        .filter(|n| !features.contains(n.as_str()))
        .collect();
    assert!(
        missing_features.is_empty(),
        "FEATURES.md's dependency list claims \"nothing else\" but does not name {} admitted \
         crate(s) — an understated dependency claim, which the policy forbids repeating:\n  {:?}",
        missing_features.len(),
        missing_features
    );

    let missing_notices: Vec<&String> = names.iter().filter(|n| !notices.contains(*n)).collect();
    assert!(
        missing_notices.is_empty(),
        "THIRD-PARTY-NOTICES.md is missing {} admitted crate(s):\n  {:?}",
        missing_notices.len(),
        missing_notices
    );

    // And the stated COUNT must match, so "15 crates" cannot rot into a wrong number.
    let n = names.len();
    assert!(
        features.contains(&format!("{n} crates")),
        "FEATURES.md must state the crate count as `{n} crates` (Cargo.toml has {n}: {names:?})"
    );
}

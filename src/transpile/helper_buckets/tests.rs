//! The two ratchets over `HELPER_BUCKETS` — split from the registry file (Invariant 13).

use super::HELPER_BUCKETS;

/// Re-derive the helper set from source and assert it MATCHES the registry exactly (DEC-377).
///
/// This is what stops the audit decaying. DEC-377's classification sat OWED because it was a
/// one-off document with nothing keeping it true; meanwhile DEC-356's inventory drifted 17→26 the
/// same way. Adding a `__phorj_*` helper now fails here until it is classified, and deleting one
/// fails until it is removed. The count itself is asserted by
/// [`the_module_header_matches_the_registry`] below — until 2026-09-04 this comment claimed the
/// count could "not drift again" while NOTHING checked it, and it had drifted to 173-vs-187 with
/// seven helpers missing from the header lists. Do not weaken either test back to prose.
///
/// A **bucket-3 entry is a build failure by design**: bucket 3 means "convenience/DRY only, must be
/// INLINED", so recording one instead of inlining it would be recording the violation.
#[test]
fn the_helper_registry_matches_the_source_exactly() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = std::collections::BTreeSet::new();
    collect(&root, &mut found);

    let registered: std::collections::BTreeSet<&str> =
        HELPER_BUCKETS.iter().map(|(n, _)| *n).collect();
    let found_refs: std::collections::BTreeSet<&str> = found.iter().map(String::as_str).collect();

    let unregistered: Vec<_> = found_refs.difference(&registered).collect();
    assert!(
        unregistered.is_empty(),
        "{} `__phorj_*` helper(s) exist but are NOT classified (DEC-377: a helper may exist ONLY \
         when PHP cannot do natively what phorj does — classify it in `HELPER_BUCKETS`, stating the \
         reason, or inline it):\n  {:?}",
        unregistered.len(),
        unregistered
    );
    let stale: Vec<_> = registered.difference(&found_refs).collect();
    assert!(
        stale.is_empty(),
        "{} classified helper(s) no longer exist in source — drop them from `HELPER_BUCKETS` so the \
         registry cannot rot (this is how the count was wrong three times):\n  {:?}",
        stale.len(),
        stale
    );

    // Bucket 3 must stay empty: recording one is recording the violation.
    let b3: Vec<_> = HELPER_BUCKETS
        .iter()
        .filter(|(_, b)| *b == 3)
        .map(|(n, _)| *n)
        .collect();
    assert!(
        b3.is_empty(),
        "bucket 3 means \"convenience/DRY only — must be INLINED\", so a helper cannot be RECORDED \
         as bucket 3: inline it instead. Offending: {b3:?}"
    );
    for (n, b) in HELPER_BUCKETS {
        assert!(
            *b == 1 || *b == 2,
            "{n} has bucket {b}; only 1 (semantic necessity) and 2 (no single-expression \
             equivalent) are valid recorded values"
        );
    }
}

fn collect(dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            // Comment lines are PROSE, not emitters — skipped, or this file's own documentation
            // (which names `__phorj_trim` precisely to record that it does NOT exist) would register
            // as a definition. The DEC-361 ratchet skips comments for the same reason.
            for line in text.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                // `function __phorj_x(` and the by-reference `function &__phorj_x(` form — missing
                // the `&` is what made a first count read 158 instead of 165.
                for m in line.match_indices("function ") {
                    let rest = &line[m.0 + "function ".len()..];
                    let rest = rest.strip_prefix('&').unwrap_or(rest);
                    if let Some(name) = helper_name(rest) {
                        out.insert(name);
                    }
                }
                // The checked-arith pairs come from a codegen table, not a `function` literal.
                for m in line.match_indices("(\"__phorj_checked_") {
                    if let Some(name) = helper_name(&line[m.0 + 2..]) {
                        out.insert(name);
                    }
                }
            }
        }
    }
}

/// The `//!` module header is DOCUMENTATION of the registry, so it must be DERIVED from it —
/// asserted three ways per bucket: the count declared in the heading, the length of the name
/// list under it, and the registry itself must all agree.
///
/// WHY THREE AND NOT TWO. On 2026-09-04 the header was inconsistent with *itself*: bucket 2's
/// heading said `(105)` while its own list held 112 names, and the registry held 116. Asserting
/// only list-vs-registry would have let the heading stay wrong; asserting only heading-vs-registry
/// would have let the list stay short. Seven helpers (`cs_decode`, `cs_encode`, `cs_name`,
/// `fold_accents`, `sleep`, `wordwrap`, `proc_run`) were missing from the lists, and the total
/// line claimed 173 against a real 187 — for four months, while the comment on the test above
/// asserted the count "cannot drift again". It could. Now it cannot.
#[test]
fn the_module_header_matches_the_registry() {
    let src = include_str!("mod.rs");
    // Only the `//!` module header — stop at the first line that is not one, so the registry
    // tuples and this module's own `///` comments can never leak into the scan.
    let header: Vec<&str> = src
        .lines()
        .take_while(|l| l.starts_with("//!") || l.trim().is_empty())
        .collect();
    let pos = |needle: &str| {
        header
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("module header has no `{needle}` heading"))
    };
    let (h1, h2) = (pos("## Bucket 1"), pos("## Bucket 2"));

    // Backticked `__phorj_x` names in a header slice. The family globs (`checked_*`, `fs_*`) do
    // not match, and starting at the Bucket-1 heading skips the `__phorj_trim` / `__phorj_unwrap`
    // mentions in the prose above it, which are there precisely to say they are NOT helpers.
    fn names(slice: &[&str]) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for line in slice {
            let mut rest = *line;
            while let Some(i) = rest.find("`__phorj_") {
                rest = &rest[i + 1..];
                if let Some(end) = rest.find('`') {
                    let n = &rest[..end];
                    if n.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    {
                        out.insert(n.to_string());
                    }
                    rest = &rest[end..];
                } else {
                    break;
                }
            }
        }
        out
    }
    // The count in `## Bucket N — reason (COUNT)`.
    fn declared(line: &str) -> usize {
        let open = line.rfind('(').expect("bucket heading has no `(count)`");
        let close = line.rfind(')').expect("bucket heading has no `(count)`");
        line[open + 1..close]
            .trim()
            .parse()
            .expect("bucket heading count is not a number")
    }

    let doc1 = names(&header[h1..h2]);
    let doc2 = names(&header[h2..]);
    // Vacuity guard: a slicing bug that yields an empty set must FAIL, not quietly compare two
    // wrong things. This whole file exists because a check that measured nothing looked green.
    assert!(
        doc1.len() > 50 && doc2.len() > 50,
        "header scan found {} / {} names — the slicing is broken, not the docs",
        doc1.len(),
        doc2.len()
    );

    for (bucket, doc, heading) in [(1u8, &doc1, header[h1]), (2, &doc2, header[h2])] {
        let reg: std::collections::BTreeSet<String> = HELPER_BUCKETS
            .iter()
            .filter(|(_, b)| *b == bucket)
            .map(|(n, _)| (*n).to_string())
            .collect();
        let missing: Vec<_> = reg.difference(doc).collect();
        let extra: Vec<_> = doc.difference(&reg).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "bucket {bucket}: the `//!` header list does not match `HELPER_BUCKETS`.\n  \
             missing from the header: {missing:?}\n  in the header but not registered: {extra:?}"
        );
        assert_eq!(
            declared(heading),
            reg.len(),
            "bucket {bucket}: the heading declares {} but the registry holds {} — update the \
             heading (this is the drift that went unnoticed for four months)",
            declared(heading),
            reg.len()
        );
    }

    // The grand total stated in the "count was wrong three times" section.
    let total_line = header
        .iter()
        .find(|l| l.contains("as of DEC-"))
        .expect("header lost its total line");
    let total: usize = total_line
        .split("**")
        .nth(1)
        .and_then(|n| n.parse().ok())
        .expect("total line has no `**N**`");
    assert_eq!(
        total,
        HELPER_BUCKETS.len(),
        "the header states {total} helpers, the registry holds {}",
        HELPER_BUCKETS.len()
    );
}

/// `__phorj_foo(` at the start of `s` → `Some("__phorj_foo")`.
fn helper_name(s: &str) -> Option<String> {
    let s = s.strip_prefix("__phorj_")?;
    let end = s.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))?;
    if s.as_bytes().get(end) != Some(&b'(') || end == 0 {
        return None;
    }
    Some(format!("__phorj_{}", &s[..end]))
}

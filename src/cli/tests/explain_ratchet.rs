//! Test: the diagnostic-coverage ratchet — every emitted code must have a `phg explain`.
use super::super::explain_text;

// ── M-DX S1: the diagnostic-coverage ratchet ──────────────────────────────────────────────────
// Every diagnostic code emitted anywhere in non-test `src/` must self-document via `phg explain`.
// This scans the source tree at test time (no hand-maintained registry to drift), so a NEW code
// added without a matching `explain_text` arm fails CI loudly. Closes the M-DX/W1 audit finding
// that 14 real checker codes had no explanation.

/// Does `s` look like a bare diagnostic code (`E-…` / `W-…`, uppercase-and-dashes)?
fn is_diagnostic_code(s: &str) -> bool {
    (s.starts_with("E-") || s.starts_with("W-"))
        && s.len() > 2
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
}

/// Extract emitted codes from a whole source file's text: standalone quoted codes (`"E-FOO"` — the
/// argument to `err_coded`/`warn_coded`/`with_code`) and bracketed codes (`[E-FOO]` — the loader's
/// plain-`String` errors, which can span multiple lines inside one `format!`). Scanning the whole
/// text (not line-by-line) is deliberate: a per-line scan misses a `[E-FOO]` sitting inside a
/// multi-line string literal. `is_diagnostic_code` filters any non-code pairing.
fn collect_codes_from_text(text: &str, out: &mut std::collections::BTreeSet<String>) {
    // Standalone quoted codes: content of a `"…"` that is exactly a code.
    for (i, c) in text.char_indices() {
        if c == '"' {
            if let Some(end) = text[i + 1..].find('"') {
                let inner = &text[i + 1..i + 1 + end];
                if is_diagnostic_code(inner) {
                    out.insert(inner.to_string());
                }
            }
        }
    }
    // Bracketed codes anywhere: `[E-…]` / `[W-…]`.
    let bytes = text.as_bytes();
    for (i, _) in bytes.iter().enumerate().filter(|(_, &b)| b == b'[') {
        if let Some(end) = text[i + 1..].find(']') {
            let inner = &text[i + 1..i + 1 + end];
            if is_diagnostic_code(inner) {
                out.insert(inner.to_string());
            }
        }
    }
}

#[test]
fn every_emitted_diagnostic_code_has_an_explanation() {
    fn walk(dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let p = entry.expect("dir entry").path();
            if p.is_dir() {
                // Skip test trees and the `explain/` catalog — both hold code literals that DEFINE
                // codes (test assertions / `explain_text` arms), not emission sites.
                if matches!(
                    p.file_name().and_then(|s| s.to_str()),
                    Some("tests") | Some("explain")
                ) {
                    continue;
                }
                walk(&p, out);
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
            // Skip test files (the `explain/` catalog is already skipped as a directory above).
            if name.ends_with("tests.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("read src file");
            // Drop the conventional trailing `#[cfg(test)]` module (test literals aren't emissions).
            let live = text.split("#[cfg(test)]").next().unwrap_or(&text);
            collect_codes_from_text(live, out);
        }
    }

    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut emitted = std::collections::BTreeSet::new();
    walk(&src_root, &mut emitted);

    // Sanity: the scan actually found a representative sample (guards against a broken walker
    // silently passing). The surface has well over a hundred codes.
    assert!(
        emitted.len() > 100,
        "code scan found only {} codes — the walker is likely broken",
        emitted.len()
    );

    let undocumented: Vec<&String> = emitted
        .iter()
        .filter(|c| explain_text(c).is_none())
        .collect();
    assert!(
        undocumented.is_empty(),
        "these diagnostic codes are emitted in src/ but have no `phg explain` entry \
         (add an arm to `explain_text`): {undocumented:?}"
    );
}

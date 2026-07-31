//! DEC-419 transpile tests — doc comments re-emitted as PHP docblocks.
//!
//! A sibling of `tests.rs` rather than an addition to it: that file was 2 lines under the Invariant-13
//! hard cap, so these would have pushed it over.

/// DEC-419 — a `/** … */` doc comment is re-emitted as a PHP DOCBLOCK on the transpiled declaration,
/// and a plain `/* … */` is NOT. PHPDoc is `/** … */` too, so this is a re-emission, not a translation.
#[test]
fn doc_comments_become_php_docblocks() {
    let src = "package Main;\n/**\n * Doubles a number.\n *\n * Second paragraph.\n */\nfunction twice(int n): int { return n * 2; }\n/* just a note */\nfunction plain(): int { return 1; }\n/** A documented class. */\nclass Box { constructor(public int v) {} }\n";
    let prog = crate::cli::parse_program(src).expect("fixture parses");
    let php = crate::transpile::emit_with_source(&prog, Some(src)).expect("transpiles");
    assert!(
        php.contains(" * Doubles a number.") && php.contains(" * Second paragraph."),
        "function docblock missing:\n{php}"
    );
    assert!(
        php.contains(" * A documented class."),
        "class docblock missing:\n{php}"
    );
    assert!(
        !php.contains("just a note"),
        "a plain block comment leaked into the PHP:\n{php}"
    );
}

/// `emit` (no source) must produce EXACTLY what it produced before DEC-419 — the doc-bearing form is
/// opt-in via `emit_with_source`, so a caller holding only a `Program` is unaffected. Asserting the two
/// differ only by the docblock is what pins that.
#[test]
fn emit_without_source_carries_no_docblocks() {
    let src = "package Main;\n/** Documented. */\nfunction f(): int { return 1; }\n";
    let prog = crate::cli::parse_program(src).expect("fixture parses");
    let bare = crate::transpile::emit(&prog).expect("transpiles");
    assert!(!bare.contains("Documented."), "{bare}");
    let with = crate::transpile::emit_with_source(&prog, Some(src)).expect("transpiles");
    assert!(with.contains(" * Documented."), "{with}");
    // Same PHP apart from the docblock: strip the comment lines and the two must be identical.
    let stripped: String = with
        .lines()
        .filter(|l| {
            let t = l.trim();
            t != "/**" && t != "*/" && !t.starts_with("* Documented")
        })
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(stripped, bare, "docblocks changed more than the comments");
}

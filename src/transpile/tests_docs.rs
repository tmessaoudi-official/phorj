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

/// DEC-420 — a free function named after a PHP builtin is MANGLED, at the definition AND every call.
///
/// Before this, `function count(…)` passed `phg check`, ran on both Rust backends, and transpiled to
/// `Cannot redeclare function count()` — verified by running the PHP, which exited 255. The mangle is
/// only useful if BOTH sites agree: mangling the definition alone would swap one fatal for
/// `Call to undefined function count_()`, so the call site is asserted too.
#[test]
fn a_free_function_named_after_a_php_builtin_is_mangled_at_both_sites() {
    let src = "package Main;\nfunction count(int n): int { return n * 2; }\nfunction plain(int n): int { return count(n); }\n";
    let prog = crate::cli::parse_program(src).expect("fixture parses");
    let php = crate::transpile::emit(&prog).expect("transpiles");
    assert!(
        php.contains("function count_(int $n)"),
        "definition not mangled:\n{php}"
    );
    assert!(
        php.contains("count_($n)"),
        "CALL not mangled — the two sites disagree:\n{php}"
    );
    assert!(
        !php.contains("function count(int"),
        "the unmangled definition is still emitted:\n{php}"
    );
}

/// A name that is NOT a builtin must be untouched — the mangle has to be surgical, or every program's
/// PHP output changes for no reason.
#[test]
fn a_non_colliding_function_name_is_left_alone() {
    let src = "package Main;\nfunction tally(int n): int { return n; }\nfunction go(int n): int { return tally(n); }\n";
    let prog = crate::cli::parse_program(src).expect("fixture parses");
    let php = crate::transpile::emit(&prog).expect("transpiles");
    assert!(php.contains("function tally(int $n)"), "{php}");
    assert!(php.contains("tally($n)"), "{php}");
    assert!(
        !php.contains("tally_"),
        "a non-colliding name was mangled:\n{php}"
    );
}

/// A METHOD named `count` is legal PHP and must NOT be mangled — only free functions collide with the
/// global builtin namespace.
#[test]
fn a_method_named_after_a_builtin_is_not_mangled() {
    let src = "package Main;\nclass Bag { constructor(public int n) {} function count(): int { return this.n; } }\n";
    let prog = crate::cli::parse_program(src).expect("fixture parses");
    let php = crate::transpile::emit(&prog).expect("transpiles");
    assert!(
        php.contains("function count()"),
        "a method was mangled:\n{php}"
    );
    assert!(!php.contains("count_"), "a method was mangled:\n{php}");
}

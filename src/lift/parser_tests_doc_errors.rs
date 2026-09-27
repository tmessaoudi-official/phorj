//! Row 4c — a docblock is applied AFTER the declaration it documents has been parsed, so a refusal
//! raised while reading one used to name the parser's CURRENT token: the NEXT member, a whole method
//! further down (scout `RawListing.php`: a ctor `@param array<string, mixed>` at line 29 reported as
//! line 171, `found Ident("public")`). It now names the declaration the docblock documents.

use super::lifter::lift_source;

fn refused(src: &str) -> String {
    match lift_source(src) {
        Err(e) => e,
        Ok(out) => panic!("expected a refusal, lifted:\n{out}"),
    }
}

const CTOR_THEN_METHOD: &str = "<?php
final class R
{
    /**
     * @param array<string, mixed> $fields
     */
    public function __construct(
        public array $fields = [],
    ) {}

    public function text(): string
    {
        return 'x';
    }
}";

#[test]
fn a_docblock_refusal_names_the_declaration_it_documents() {
    let e = refused(CTOR_THEN_METHOD);
    assert!(e.contains("`mixed` in a docblock type"), "{e}");
    assert!(e.contains("docblock of the declaration at line 7"), "{e}");
    assert!(
        !e.contains("found"),
        "the next token is not what was wrong: {e}"
    );
}

#[test]
fn a_function_docblock_refusal_names_its_function() {
    let e = refused(
        "<?php\n/** @return array<string, mixed> */\nfunction f(): array { return []; }\nfunction g(): int { return 2; }",
    );
    assert!(e.contains("docblock of the declaration at line 3"), "{e}");
}

#[test]
fn a_local_var_docblock_refusal_names_its_statement() {
    let e = refused(
        "<?php\nfunction f(): int {\n    /** @var array<string, mixed> $x */\n    $x = [];\n    return 1;\n}",
    );
    assert!(e.contains("docblock of the declaration at line 4"), "{e}");
}

#[test]
fn an_ordinary_error_after_an_applied_docblock_keeps_its_own_line() {
    // The docblock line is scoped to the APPLY: a syntax error in the next function must still name
    // its own token and line, not a docblock that was applied cleanly before it.
    let e = refused(
        "<?php\n/** @param list<int> $xs */\nfunction f(array $xs): int { return 1; }\nfunction g(): int { return 1 +; }",
    );
    assert!(!e.contains("docblock"), "{e}");
    assert!(e.contains("found") && e.contains("(line 4)"), "{e}");
}

/// twes row T2 (2026-09-27) — a PHPStan/Psalm pseudo-type (`numeric-string`, `array-key`,
/// `class-string`, …) fell through to the class-name path and was printed VERBATIM, so
/// `List<(StockLot, numeric-string)>` reached the draft and it did not parse. A PHP class name can
/// never contain `-`, so a hyphenated atom is always a pseudo-type: refused by name.
#[test]
fn a_hyphenated_pseudo_type_is_refused_by_name() {
    for pseudo in [
        "numeric-string",
        "array-key",
        "non-empty-string",
        "class-string",
        "positive-int",
    ] {
        let src = format!(
            "<?php\n/** @param list<array{{string, {pseudo}}}> $xs */\nfunction f(array $xs): int {{ return 0; }}"
        );
        let e = refused(&src);
        assert!(e.contains(&format!("`{pseudo}`")), "{pseudo}: {e}");
        assert!(e.contains("pseudo-type"), "{pseudo}: {e}");
    }
}

/// The two refinements that already lift keep lifting: `non-empty-list<T>` and `non-empty-array<…>`
/// are their base generic (the refinement is a static promise PHP never checks at runtime).
#[test]
fn non_empty_list_and_array_still_lift_as_their_base() {
    let out = lift_source(
        "<?php\n/** @param non-empty-list<int> $xs */\nfunction f(array $xs): int { return 0; }",
    )
    .unwrap_or_else(|e| panic!("lifts: {e}"));
    assert!(out.contains("List<int> xs"), "{out}");
}

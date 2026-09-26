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

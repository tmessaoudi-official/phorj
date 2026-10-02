//! Parity review C2 — the lifter copies a PHP builtin phorj has no counterpart for VERBATIM
//! (`strrev($s)` stays `strrev(s)`), with no marker, so the draft fails `phg check` with a bare
//! "unknown function" the reader must trace back. The lifter's contract: what it cannot do is said
//! loudly, in a `// CANNOT LIFT:` note.

use crate::lift::lifter::lift_source;

fn lifted(src: &str) -> String {
    lift_source(src).unwrap_or_else(|e| panic!("expected a lift, refused: {e}\n{src}"))
}

#[test]
fn an_unmapped_php_builtin_is_named_in_a_cannot_lift_note() {
    let out = lifted("<?php\nfunction f(string $s): string { return strrev($s) . nl2br($s); }\n");
    assert!(
        out.contains("// CANNOT LIFT: PHP function `strrev()`"),
        "{out}"
    );
    assert!(
        out.contains("// CANNOT LIFT: PHP function `nl2br()`"),
        "{out}"
    );
}

#[test]
fn the_empty_language_construct_is_named_too() {
    let out = lifted("<?php\nfunction f(string $s): bool { return empty($s); }\n");
    assert!(
        out.contains("// CANNOT LIFT: PHP function `empty()`"),
        "{out}"
    );
}

#[test]
fn a_mapped_builtin_a_method_call_and_a_user_function_are_not_named() {
    let out = lifted(
        "<?php\nfunction helper(string $s): string { return $s; }\n\
         function f(string $s): string { return strtoupper(helper($s)); }\n",
    );
    assert!(!out.contains("CANNOT LIFT"), "{out}");
}

#[test]
fn each_name_is_noted_once_in_first_seen_order() {
    let out = lifted(
        "<?php\nfunction f(string $s): string { return strrev($s) . nl2br($s) . strrev($s); }\n",
    );
    assert_eq!(out.matches("PHP function `strrev()`").count(), 1, "{out}");
    assert!(
        out.find("`strrev()`").unwrap() < out.find("`nl2br()`").unwrap(),
        "{out}"
    );
}

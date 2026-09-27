//! Row 4j — a user exception class's PHP BASE (`extends \RuntimeException`), which the DEC-421 mapping
//! covered at `throw` / `catch` sites but not at the declaration. Split from `exceptions_tests.rs`
//! (Invariant 13); the helpers are shared.

use super::tests::{check, parse};
use super::unmapped_exception_classes;
use crate::lift::lifter::lift_source;

// ── row 4j: a user exception class's PHP base ──

/// `extends \RuntimeException` names a PHP builtin, which phorj does not have: it lifts to the DEC-421
/// counterpart (`RuntimeError`, an `open` class whose `message` constructor the subclass inherits), and
/// the draft imports it like any other site that names one.
#[test]
fn a_user_exception_extends_the_mapped_phorj_error() {
    let out = lift_source(
        "<?php\nfinal class ParseFailureException extends \\RuntimeException {\n\
             public static function at(string $w): self { return new self('bad ' . $w); }\n\
         }\n\
         try { throw ParseFailureException::at('x'); } catch (ParseFailureException $e) { echo 'caught'; }",
    )
    .expect("lifts");
    assert!(
        out.contains("class ParseFailureException extends RuntimeError"),
        "{out}"
    );
    assert!(
        out.contains("import Core.ErrorModule.RuntimeError;"),
        "{out}"
    );
    assert!(!out.contains("CANNOT LIFT"), "{out}");
    check(&out).unwrap_or_else(|e| panic!("{e}\n{out}"));
}

/// Only a KNOWN PHP exception base is mapped or imported: a user base class is the program's own name,
/// and must not be reported as an unliftable exception class.
#[test]
fn a_user_base_class_is_neither_mapped_nor_reported() {
    for src in [
        "<?php\nclass Base {}\nfinal class Leaf extends Base {}",
        // A base the file does not declare (a vendor class) is not an exception site either.
        "<?php\nfinal class Leaf extends VendorBase {}",
    ] {
        let out = lift_source(src).expect("lifts");
        assert!(out.contains("class Leaf extends"), "{out}");
        assert!(
            !out.contains("CANNOT LIFT") && !out.contains("ErrorModule"),
            "{out}"
        );
        assert!(unmapped_exception_classes(&parse(src)).is_empty(), "{src}");
    }
}

/// phorj requires a throwable's name to end in `Error` or `Exception` (`E-ERROR-NAME`). The lift keeps
/// the PHP name — renaming a class is not the lifter's call (row 4j, a pending question) — so the
/// checker names the fix.
#[test]
fn a_throwable_whose_name_says_nothing_keeps_it_and_the_checker_asks_for_a_rename() {
    let out = lift_source("<?php\nfinal class MalformedText extends \\RuntimeException {}")
        .expect("lifts");
    assert!(
        out.contains("class MalformedText extends RuntimeError"),
        "{out}"
    );
    let err = check(&out).expect_err("E-ERROR-NAME");
    assert!(err.contains("E-ERROR-NAME"), "{err}");
}

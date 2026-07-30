//! DEC-367 (Invariant-1 breach) — `E-FINAL-PARENT-METHOD`.
//!
//! A phorj class implementing `Error` transpiles to one extending PHP's `Exception`, which marks seven
//! methods `final`. Defining one of them produced a program both Rust backends ran happily while the PHP
//! leg died at runtime with `Fatal error: Cannot override final method Exception::getMessage()` — a
//! byte-identity breach that `phg check` reported as clean.
//!
//! Rejected at check time instead. Renaming on emission was explicitly rejected by the ruling: it would
//! keep the program running while silently diverging from what the author wrote.

use super::support::*;

fn final_errs(src: &str) -> Vec<String> {
    errors_of(src)
        .iter()
        .filter(|d| d.code == Some("E-FINAL-PARENT-METHOD"))
        .map(|d| d.message.clone())
        .collect()
}

/// A throwable class (the name must end in `Error`/`Exception` — `E-ERROR-NAME`) defining `method`.
fn throwable_with(method: &str) -> String {
    format!(
        "class CustomError implements Error {{ constructor(public string reason) {{}} \
         function {method}(): string {{ return this.reason; }} }} \
         function main() -> void {{ }}"
    )
}

#[test]
fn every_final_exception_method_is_rejected() {
    // All seven, verified against php-8.5.8 by reflection rather than from memory. `getCode` returns
    // string here only to keep one fixture — the collision is on the NAME, before any signature check.
    for m in [
        "getMessage",
        "getCode",
        "getFile",
        "getLine",
        "getTrace",
        "getPrevious",
        "getTraceAsString",
    ] {
        let src = throwable_with(m);
        assert!(
            !final_errs(&src).is_empty(),
            "`{m}` is final on PHP's Exception and must be rejected; got {:?}",
            errors_of(&src)
        );
    }
}

#[test]
fn the_diagnostic_explains_the_php_reason_and_offers_a_rename() {
    let src = throwable_with("getMessage");
    let d = errors_of(&src)
        .into_iter()
        .find(|d| d.code == Some("E-FINAL-PARENT-METHOD"))
        .expect("no E-FINAL-PARENT-METHOD");
    assert!(
        d.message.contains("final") && d.message.contains("Exception"),
        "the message must say WHY PHP refuses it: {}",
        d.message
    );
    assert!(
        d.hint.as_deref().is_some_and(|h| h.contains("rename")),
        "the hint must offer the way out: {:?}",
        d.hint
    );
}

#[test]
fn a_non_final_exception_method_is_still_allowed() {
    // `__construct` and `__toString` are NOT final on Exception, so a throwable may still carry its own
    // constructor and stringification. Over-rejecting here would make `Error` subclasses unusable.
    let src = "class CustomError implements Error { constructor(public string reason) {} \
               function describe(): string { return this.reason; } } \
               function main() -> void { }";
    assert!(
        final_errs(src).is_empty(),
        "an ordinary method must stay legal: {:?}",
        final_errs(src)
    );
}

#[test]
fn a_non_throwable_class_may_define_get_message_freely() {
    // The guard is scoped to classes that actually implement `Error`. A plain value type never extends
    // `Exception` on the PHP leg, so `getMessage` there collides with nothing.
    let src = "class Envelope { constructor(public string body) {} \
               function getMessage(): string { return this.body; } } \
               function main() -> void { }";
    assert!(
        final_errs(src).is_empty(),
        "a non-throwable class must be unaffected: {:?}",
        final_errs(src)
    );
}

#[test]
fn a_foreign_declare_class_may_declare_a_final_method() {
    // A `declare class` DESCRIBES an existing PHP class instead of defining one, so declaring the
    // signature of a method that is final over there is correct — it is how `examples/interop/`
    // binds PHP's own `DivisionByZeroError`. Only a class phorj EMITS can collide.
    //
    // This regression exists because the first version of the guard rejected it, and the pre-push gate
    // caught it on a shipped example rather than a reviewer catching it in the diff.
    let src =
        "declare class DivisionByZeroError implements Error { function getMessage(): string; } \
               function main() -> void { }";
    assert!(
        final_errs(src).is_empty(),
        "a foreign declaration must not be rejected: {:?}",
        final_errs(src)
    );
}

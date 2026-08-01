//! `Core.ErrorModule` — phorj's STANDARD ERROR TAXONOMY (DEC-421, developer-ruled 2026-07-31).
//!
//! Six error types every program can throw and catch, so code that needs a conventional error does not
//! have to declare its own. It exists because the PHP LIFTER needed somewhere to land: a lifted
//! `catch (\RuntimeException $e)` produced valid phorj syntax that then failed `phg check` with
//! `unknown type RuntimeException`, because phorj had an `Error` marker and user-declared errors and
//! nothing in between.
//!
//! **FLAT on purpose.** No inheritance between these six. PHP's own `Throwable`/`Error`/`Exception`
//! split was considered and REJECTED: mirroring it would make the lifter trivial, but it would import a
//! much-criticised hierarchy into a language that deliberately does not have one — deciding phorj's
//! error model as a side effect of a lift feature. Flat also means `catch` needs no subclass matching,
//! which is the subtlety a hierarchy would have added. The naming matches how phorj already prefixes
//! taxonomies (`FileSystemNotFoundError` and friends).
//!
//! **Three names avoid a PHP builtin CLASS, and that is why they read as they do.** `ArithmeticError`,
//! `TypeError` and `ValueError` — the obvious spellings, and the ones first proposed — are all real PHP
//! builtin classes, so `E-RESERVED-NAME` (DEC-202/213) rejects them and rightly: transpiling
//! `class TypeError extends \Exception` would redeclare PHP's own. They are `MathError`,
//! `TypeMismatchError` and `InvalidValueError` here. `RuntimeError`, `LogicError` and `IoError` collide
//! with nothing and keep their natural names.
//!
//! **Named `ErrorModule`, not `Error`** (DEC-278's suffix rule, applied for a concrete reason here):
//! `Error` is already the built-in marker interface every error implements, so a module whose qualifier
//! leaf was `Error` would bind that name to two different things in the same file.
//!
//! These are ordinary phorj classes — no new `Value`, no new `Ty`, nothing for a backend to learn. Each
//! `implements Error`, so the existing typed-catch machinery handles them unchanged and they transpile
//! to `extends \Exception` like any other phorj error.

/// The `Core.ErrorModule` prelude source, injected when a program imports it.
///
/// The six were chosen to cover what actually appears in PHP code the lifter will meet, without
/// inventing a category phorj has no use for. `IoError` is the one with no PHP builtin counterpart —
/// PHP throws `RuntimeException` for I/O — and it is included because phorj's own surface wants it.
pub(crate) const ERROR_PRELUDE: &str = r#"
open class RuntimeError implements Error {
  constructor(public string message) {}
}
open class LogicError implements Error {
  constructor(public string message) {}
}
open class MathError implements Error {
  constructor(public string message) {}
}
open class TypeMismatchError implements Error {
  constructor(public string message) {}
}
open class InvalidValueError implements Error {
  constructor(public string message) {}
}
open class IoError implements Error {
  constructor(public string message) {}
}
"#;

/// The phorj error type a PHP builtin exception class lifts to, or `None` when there is no honest
/// mapping (DEC-421).
///
/// `None` is a real answer, not a gap to paper over: the lifter emits the original name plus a
/// `// CANNOT LIFT:` note, so a framework or user-defined exception is visibly left for the human
/// rather than silently coerced into the nearest phorj type.
///
/// The mapping is SEMANTIC, not hierarchical, because the target set is flat. That is why
/// `InvalidArgumentException` lands on `ValueError` rather than `LogicError`: PHP files it under
/// `LogicException` for hierarchy reasons, but what it actually reports is a bad argument VALUE, and a
/// flat set should say what a thing means rather than where PHP filed it.
///
/// Case-insensitive, and a leading `\` is tolerated, so `\RuntimeException` and `RuntimeException` agree.
pub(crate) fn phorj_error_for_php_exception(name: &str) -> Option<&'static str> {
    let n = name.trim_start_matches('\\').to_ascii_lowercase();
    Some(match n.as_str() {
        // The bases: anything unspecific becomes a runtime error.
        "throwable" | "exception" | "error" | "errorexception" | "runtimeexception" => {
            "RuntimeError"
        }
        // Programmer-error family.
        "logicexception" | "badfunctioncallexception" | "badmethodcallexception" => "LogicError",
        // Arithmetic. PHP's `DivisionByZeroError` extends `ArithmeticError`; flat, both land together.
        "arithmeticerror"
        | "divisionbyzeroerror"
        | "overflowexception"
        | "underflowexception"
        | "rangeexception" => "MathError",
        "typeerror" => "TypeMismatchError",
        // Bad-value family — see the note above on `InvalidArgumentException`.
        "valueerror"
        | "invalidargumentexception"
        | "domainexception"
        | "lengthexception"
        | "outofrangeexception"
        | "outofboundsexception"
        | "unexpectedvalueexception"
        | "jsonexception" => "InvalidValueError",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::phorj_error_for_php_exception;

    #[test]
    fn the_common_php_exceptions_map_and_the_root_marker_is_tolerated() {
        assert_eq!(
            phorj_error_for_php_exception("\\RuntimeException"),
            Some("RuntimeError")
        );
        // With and without the root `\`, and case-insensitively — PHP class names are.
        assert_eq!(
            phorj_error_for_php_exception("RuntimeException"),
            Some("RuntimeError")
        );
        assert_eq!(
            phorj_error_for_php_exception("runtimeexception"),
            Some("RuntimeError")
        );
        assert_eq!(
            phorj_error_for_php_exception("\\DivisionByZeroError"),
            Some("MathError")
        );
        assert_eq!(
            phorj_error_for_php_exception("\\LogicException"),
            Some("LogicError")
        );
        assert_eq!(
            phorj_error_for_php_exception("\\TypeError"),
            Some("TypeMismatchError")
        );
    }

    /// `InvalidArgumentException` is the deliberate semantic call: PHP files it under `LogicException`,
    /// but it reports a bad VALUE, and a flat set should name the meaning.
    #[test]
    fn invalid_argument_maps_semantically_not_hierarchically() {
        assert_eq!(
            phorj_error_for_php_exception("\\InvalidArgumentException"),
            Some("InvalidValueError")
        );
    }

    /// An unknown class returns `None` rather than a nearest guess — the lifter turns that into a
    /// visible `// CANNOT LIFT:` note so the human sees what was left undone.
    #[test]
    fn an_unmapped_exception_is_none_rather_than_a_guess() {
        assert_eq!(phorj_error_for_php_exception("\\Acme\\PaymentFailed"), None);
        assert_eq!(phorj_error_for_php_exception("MyAppException"), None);
    }
}

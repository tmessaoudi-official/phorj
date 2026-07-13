//! Single source of truth for **PHP builtin class/interface names** — the always-preloaded Zend
//! engine + SPL + json + date classes that exist in every PHP process with no `use`/extension.
//!
//! Two backends consult this same list, and they MUST agree (DEC-213, byte-identity spine):
//!   - the **checker** rejects a top-level class/enum/interface/trait whose name collides with one
//!     of these (`E-RESERVED-NAME`, DEC-202) — a user-chosen API name that would become a
//!     `Cannot redeclare class` fatal in the flat single-package emission;
//!   - the **transpiler** silently mangles an enum *variant* with a colliding name
//!     (`final class <Variant> extends <Enum>` would otherwise redeclare the builtin).
//!
//! Before DEC-213 the transpiler kept its own hand-copied ~17-name subset, so a variant named after
//! an SPL/date/json builtin (e.g. `DateTime`) passed the checker, ran fine, but its transpiled PHP
//! threw `Cannot redeclare class DateTime` — a live byte-identity break. Both paths now read THIS
//! list, so the reject set and the mangle set can never drift apart again.
//!
//! Case-insensitive (PHP class names are). The list is the always-loaded core only — extension
//! classes (mysqli, PDO, …) are intentionally excluded: they are not present without the extension,
//! and gating on them would reject legal names. Kept in sync empirically against the transpile floor
//! (php-8.5.8).

/// The always-preloaded PHP builtin class/interface names (lowercased), the union consulted by both
/// the DEC-202 reject and the DEC-213 variant mangle. See the module doc for the invariant.
const BUILTIN_CLASSES: &[&str] = &[
    // Core
    "stdclass",
    "exception",
    "error",
    "throwable",
    "typeerror",
    "valueerror",
    "argumentcounterror",
    "arithmeticerror",
    "divisionbyzeroerror",
    "errorexception",
    "unhandledmatcherror",
    "closure",
    "generator",
    "fiber",
    "fibererror",
    "weakreference",
    "weakmap",
    "stringable",
    "traversable",
    "iterator",
    "iteratoraggregate",
    "arrayaccess",
    "countable",
    "serializable",
    "unitenum",
    "backedenum",
    "attribute",
    "sensitiveparameter",
    "returntypewillchange",
    "allowdynamicproperties",
    "override",
    "deprecated",
    // SPL exceptions
    "runtimeexception",
    "logicexception",
    "invalidargumentexception",
    "domainexception",
    "lengthexception",
    "outofboundsexception",
    "outofrangeexception",
    "rangeexception",
    "overflowexception",
    "underflowexception",
    "unexpectedvalueexception",
    "badfunctioncallexception",
    "badmethodcallexception",
    // json
    "jsonexception",
    "jsonserializable",
    // date
    "datetime",
    "datetimeimmutable",
    "datetimeinterface",
    "datetimezone",
    "dateinterval",
    "dateperiod",
    "dateerror",
    "dateobjecterror",
    "daterangeerror",
    "dateexception",
    "dateinvalidoperationexception",
    "dateinvalidtimezoneexception",
    "datemalformedintervalstringexception",
    "datemalformedperiodstringexception",
    "datemalformedstringexception",
    // SPL containers / iterators
    "arrayiterator",
    "arrayobject",
    "splobjectstorage",
    "splfixedarray",
    "splstack",
    "splqueue",
    "spldoublylinkedlist",
    "splpriorityqueue",
    "splheap",
    "splminheap",
    "splmaxheap",
    "splfileinfo",
    "splfileobject",
    "spltempfileobject",
    "splobserver",
    "splsubject",
    "directoryiterator",
    "filesystemiterator",
    "recursivedirectoryiterator",
    "recursiveiteratoriterator",
    "iteratoriterator",
    "callbackfilteriterator",
    "recursivecallbackfilteriterator",
    "filteriterator",
    "limititerator",
    "appenditerator",
    "cachingiterator",
    "recursivecachingiterator",
    "infiniteiterator",
    "multipleiterator",
    "norewinditerator",
    "regexiterator",
    "recursiveregexiterator",
    "recursivefilteriterator",
    "recursivetreeiterator",
    "recursivearrayiterator",
    "parentiterator",
    "outeriterator",
    "recursiveiterator",
    "seekableiterator",
    "globiterator",
    "directory",
    "php_user_filter",
    "php_incomplete_class",
    "assertionerror",
    "compileerror",
    "parseerror",
    "closedgeneratorexception",
    "requestparsebodyexception",
];

/// `true` if `name` collides with an always-preloaded PHP builtin class/interface (case-insensitive).
pub fn is_php_builtin_class_name(name: &str) -> bool {
    BUILTIN_CLASSES.contains(&name.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::is_php_builtin_class_name;

    #[test]
    fn core_and_spl_and_date_and_json_all_match_case_insensitively() {
        for n in [
            "Exception",
            "exception",
            "DateTime",
            "RuntimeException",
            "ArrayObject",
            "JsonException",
        ] {
            assert!(is_php_builtin_class_name(n), "{n} should be a builtin");
        }
    }

    #[test]
    fn non_builtins_do_not_match() {
        for n in ["Tok", "MyClass", "Plain", "Widget", ""] {
            assert!(!is_php_builtin_class_name(n), "{n} should not be a builtin");
        }
    }
}

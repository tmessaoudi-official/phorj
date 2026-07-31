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

/// Always-available PHP builtin FUNCTION names (DEC-420). The functions-half of the same problem the
/// class list above solves: a phorj `function count(…)` passed `phg check`, ran on both Rust backends,
/// and transpiled to `Cannot redeclare function count()` — `phg run` fine, PHP leg dead.
///
/// Scope is deliberately the ALWAYS-PRESENT core (no extension-gated functions): gating on, say,
/// `mysqli_query` would reject a legal name on a build where that extension is absent, and the
/// transpile→real-PHP oracle already catches the unbounded extension tail. PHP function names are
/// case-insensitive, so the list is lowercase and the lookup folds case.
///
/// Not exhaustive over all ~1500 core functions on purpose: this covers the names a *phorj* program
/// plausibly wants (short, ordinary verbs and nouns). A miss is not silent — it surfaces as the same
/// `Cannot redeclare` fatal from the oracle, and the fix is one row here.
const BUILTIN_FUNCTIONS: &[&str] = &[
    // array / iteration
    "count",
    "sort",
    "rsort",
    "usort",
    "uasort",
    "uksort",
    "ksort",
    "krsort",
    "asort",
    "arsort",
    "reset",
    "end",
    "next",
    "prev",
    "current",
    "key",
    "each",
    "range",
    "compact",
    "extract",
    "shuffle",
    "implode",
    "explode",
    "join",
    "array",
    "list",
    "in_array",
    "sizeof",
    // string
    "print",
    "printf",
    "sprintf",
    "trim",
    "ltrim",
    "rtrim",
    "chop",
    "strlen",
    "strpos",
    "substr",
    "strstr",
    "strrev",
    "strtolower",
    "strtoupper",
    "ucfirst",
    "lcfirst",
    "ucwords",
    "str_repeat",
    "str_replace",
    "str_split",
    "str_pad",
    "number_format",
    "nl2br",
    "wordwrap",
    "chunk_split",
    "similar_text",
    "levenshtein",
    "soundex",
    "metaphone",
    "quotemeta",
    "chr",
    "ord",
    "bin2hex",
    // math
    "abs",
    "ceil",
    "floor",
    "round",
    "sqrt",
    "pow",
    "exp",
    "log",
    "log10",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "pi",
    "max",
    "min",
    "rand",
    "srand",
    "mt_rand",
    "intdiv",
    "fmod",
    "hypot",
    "deg2rad",
    "rad2deg",
    "base_convert",
    "bindec",
    "decbin",
    "dechex",
    "hexdec",
    // type / var
    "gettype",
    "settype",
    "intval",
    "floatval",
    "strval",
    "boolval",
    "serialize",
    "unserialize",
    "var_dump",
    "var_export",
    "print_r",
    "empty",
    "isset",
    "unset",
    "is_array",
    "is_string",
    "is_int",
    "is_float",
    "is_bool",
    "is_null",
    "is_object",
    "is_callable",
    "is_numeric",
    // fs / io / misc
    "file",
    "fopen",
    "fclose",
    "fread",
    "fwrite",
    "fgets",
    "feof",
    "flock",
    "rename",
    "copy",
    "unlink",
    "mkdir",
    "rmdir",
    "basename",
    "dirname",
    "realpath",
    "glob",
    "header",
    "die",
    "exit",
    "eval",
    "sleep",
    "usleep",
    "time",
    "date",
    "mktime",
    "microtime",
    "hash",
    "md5",
    "sha1",
    "crc32",
    "uniqid",
    "getenv",
    "putenv",
    "setlocale",
    "error_log",
    "trigger_error",
    "assert",
];

/// `true` if `name` collides with an always-available PHP builtin FUNCTION (case-insensitive).
///
/// The FUNCTION twin of [`is_php_builtin_class_name`], and the same DEC-213 rule applies: whatever
/// consults this must be the ONLY thing that decides, so the reject/mangle set cannot drift from the
/// emit set. Today the sole consumer is the transpiler's [`crate::transpile::php_free_fn_name`].
pub fn is_php_builtin_function_name(name: &str) -> bool {
    BUILTIN_FUNCTIONS.contains(&name.to_ascii_lowercase().as_str())
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

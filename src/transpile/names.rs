//! Transpiler — PHP name shaping: namespaces, reserved-name mangling, variant names.

use super::*;

pub(super) fn namespace_of(name: &str) -> String {
    match name.rfind('\\') {
        Some(i) => name[..i].to_string(),
        None => "Main".to_string(),
    }
}

/// The trailing segment of a mangled name (`Acme\Util\compute` ⇒ `compute`), used as the function's
/// declared name inside its `namespace` block. A bare name is returned unchanged.
pub(super) fn last_segment(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// PHP reserves a fixed set of words as **class names** — `int`/`float`/`bool`/`null` etc. — and
/// rejects them as a class name *even inside a namespace* (verified vs PHP 8.5). An enum variant
/// transpiles to `final class <Variant> extends <Enum>`, so a variant named after one of these would
/// be a parse error. We mangle such a variant's PHP class name by appending `_` (`Int`→`Int_`). This
/// is **transpiler-only**: `run`/`runvm` address a variant by its Phorj name (`EnumVal.variant`),
/// never a PHP class name, so program stdout is unaffected. Comparison is case-insensitive (PHP class
/// names are). `array`/`callable`/`list` ARE rejected as class names by PHP 8.5 (verified) and so are
/// mangled; `enum` is the one type-ish word PHP still accepts as a class name, left untouched.
///
/// Three groups (all verified rejected as a `class` name vs PHP 8.5.8):
///   1. **value-type words** (`int`/`float`/… — the original set);
///   2. **keyword-as-class-name words** — reserved words / language constructs PHP forbids in a class
///      position (`empty`/`echo`/`print`/`match`/`fn`/… — e.g. a variant `Empty` ⇒ `final class Empty`
///      is `Parse error: unexpected token "empty"`). This group closes the F-m byte-identity break
///      (`run ≡ run --tree-walker` succeeded while the transpiled PHP failed to parse);
///   3. **always-present PHP builtin class/interface names** (`Exception`/`DateTime`/`ArrayObject`/… — a
///      variant `DateTime` ⇒ `final class DateTime` is a "cannot redeclare class" fatal). Single-sourced
///      via `crate::php_names::is_php_builtin_class_name` (DEC-213) — the SAME list the checker's DEC-202
///      reject uses, so the reject set and this mangle set can never drift (they did before DEC-213: a
///      variant named after an SPL/date/json builtin passed the reject but redeclared in transpiled PHP).
///      The *unbounded* extension-loaded tail (`PDO`, `mysqli`, …) is not enumerable and stays caught by
///      the transpile→real-PHP oracle — an honest, bounded reduction; mangling is invisible and free.
pub(super) fn php_variant_name(variant: &str) -> String {
    // The trailing segment is the actual PHP class name (`\Ns\Int` ⇒ `Int`); mangle only that.
    let leaf = last_segment(variant);
    const RESERVED: &[&str] = &[
        // 1. value-type words
        "int",
        "float",
        "bool",
        "string",
        "true",
        "false",
        "null",
        "void",
        "iterable",
        "object",
        "mixed",
        "never",
        "self",
        "parent",
        "static",
        "array",
        "list",
        "callable",
        // 2. keyword-as-class-name words (PHP rejects these in a class position)
        "empty",
        "echo",
        "print",
        "unset",
        "isset",
        "eval",
        "exit",
        "die",
        "clone",
        "goto",
        "and",
        "or",
        "xor",
        "yield",
        "use",
        "namespace",
        "switch",
        "case",
        "default",
        "foreach",
        "match",
        "fn",
        "readonly",
        // Group 3 (always-present PHP builtin class/interface names) is single-sourced below via
        // `crate::php_names::is_php_builtin_class_name` (DEC-213) — do NOT re-inline it here.
    ];
    if RESERVED.contains(&leaf.to_ascii_lowercase().as_str())
        || crate::php_names::is_php_builtin_class_name(leaf)
    {
        format!("{variant}_")
    } else {
        variant.to_string()
    }
}

/// Property names PHP's `\Exception` already declares (M-faults 2b). A Phorj `Error` subtype
/// transpiles to `extends \Exception`, so a promoted/declared field with one of these names would be
/// a typed redeclaration of an inherited untyped property — a PHP fatal — and must be emitted untyped.
pub(super) fn exception_reserved(name: &str) -> bool {
    matches!(name, "message" | "code" | "file" | "line" | "previous")
}

/// Whether `ty` is the built-in marker `Error` (bare `Error` or optional `Error?`). Used by M-faults
/// 2c to recognize a conventional `cause` field whose value feeds PHP's native exception chain. A
/// type literally named `Error` in PHP would resolve to the unrelated *engine* `Error` class, so an
/// `Error`-typed cause must be emitted as `?\Throwable` (the type of `\Exception::$previous`), which
/// accepts every Phorj `Error` (each transpiles to `extends \Exception`).
pub(super) fn is_error_marker_type(ty: &Type) -> bool {
    match ty {
        Type::Named { name, .. } => last_segment(name) == "Error",
        Type::Optional { inner, .. } => is_error_marker_type(inner),
        _ => false,
    }
}

/// A type *reference* in PHP: a mangled (`\`-bearing) cross-package name becomes an absolute FQN
/// (leading `\`, so it resolves regardless of the surrounding `namespace` block — uniform with
/// function de-mangling, no `use`); a bare same-/`Main`-namespace name stays bare (M-RT cross-package
/// types). Byte-identical to the pre-lift output for a single-package program (no `\` names).
pub(super) fn php_type_ref(name: &str) -> String {
    if name.contains('\\') {
        let leaf = php_class_name(last_segment(name));
        let ns = &name[..name.len() - last_segment(name).len()];
        format!("\\{ns}{leaf}")
    } else {
        php_class_name(name)
    }
}

/// Mangle a PHP-reserved **class/enum** name that a Phorj type name would collide with. The only
/// case today is `RoundingMode`: PHP 8.4+ ships a built-in `enum RoundingMode`, so the injected M-NUM
/// S2 enum (`abstract class RoundingMode` + its variant subclasses) would otherwise fatal with
/// "cannot extend enum RoundingMode". Append `_` (`RoundingMode` → `RoundingMode_`), transpiler-only:
/// `run`/`runvm` address the enum by its Phorj name (`EnumVal.ty`), never a PHP class name, so
/// program stdout is unaffected. Distinct from [`php_variant_name`] (which mangles reserved *variant*
/// names like `Int`); the enum *type* name and its variant names mangle independently.
pub(super) fn php_class_name(name: &str) -> String {
    if name.eq_ignore_ascii_case("RoundingMode") {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

/// Whether a native's PHP erasure is a global function call (`strlen(...)`, `str_replace(...)`) — an
/// identifier immediately followed by `(`. Such calls need a leading `\` inside a namespace block so
/// they resolve to the global PHP builtin, not `CurrentNs\strlen`. A language construct like
/// `echo … . "\n"` (`console.println`) is not a function call and is left alone (M5-8).
pub(super) fn looks_like_global_call(s: &str) -> bool {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for (_, c) in chars {
        if c == '(' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    false
}

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

/// DEC-329.3: the PHP class name of an enum variant, SCOPED by its owning enum —
/// `Shape.Circle` ⇒ `final class Shape_Circle extends Shape`. Two enums sharing a variant name
/// emit distinct classes, so the old flat-name collision (the pre-329.3
/// `E-TRANSPILE-VARIANT-COLLISION` refusal) cannot occur, and the pre-329.3 reserved-word mangle
/// (`Int`→`Int_`) is subsumed: a scoped name always carries the `Enum_` prefix, so it can never be
/// a bare PHP reserved word. The name is **transpiler-only**: interp/VM address a variant by
/// its Phorj name (`EnumVal.variant`), and the PHP debug renderer maps the class back to
/// `Enum.Variant(…)` via the DEC-238 rows — program stdout is unaffected. The always-present PHP
/// builtin class/interface list is still guarded (DEC-213 single source — underscore builtins like
/// `__PHP_Incomplete_Class` exist); the unbounded extension-loaded tail stays caught by the
/// transpile→real-PHP oracle.
pub(super) fn php_scoped_variant_name(enum_name: &str, variant: &str) -> String {
    // Both trailing segments: the enum leaf scopes, the variant leaf names (`\Ns\Shape.Circle`
    // declares inside its namespace block, so the class name itself is namespace-free).
    let name = format!("{}_{}", last_segment(enum_name), last_segment(variant));
    if crate::php_names::is_php_builtin_class_name(&name) {
        format!("{name}_")
    } else {
        name
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
/// interp/VM address the enum by its Phorj name (`EnumVal.ty`), never a PHP class name, so
/// program stdout is unaffected. Distinct from [`php_variant_name`] (which mangles reserved *variant*
/// names like `Int`); the enum *type* name and its variant names mangle independently.
pub(super) fn php_class_name(name: &str) -> String {
    // `Iterator` (DEC-257): the injected `Core.IteratorModule` interface collides with PHP's root
    // builtin interface `Iterator` in the (un-namespaced) transpiled output — same mangle, same
    // rationale as `RoundingMode`. interp/VM never see a PHP class name, so stdout is
    // unaffected; META-7 disclosure lives in the CHANGELOG entry.
    if name.eq_ignore_ascii_case("RoundingMode") || name == "Iterator" {
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

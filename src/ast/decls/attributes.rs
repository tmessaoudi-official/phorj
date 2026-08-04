//! AST — item attributes (`#[Route]`, `#[Entry]`, …) and their built-in recognizers.

use super::*;

/// A PHP-8-style item attribute — `#[Name(arg, …)]` (M6 W2). Parsed generally (any `Name` + any
/// expression args); only `Route` is given semantics this slice (every other name is a hard
/// `E-UNKNOWN-ATTRIBUTE`). Attributes are front-end metadata: validated by the checker and consumed by
/// the `Http.autoRouter()` desugar, never seen by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// Recognize a built-in attribute written in ANY "nothing in the wind" import form: the bare leaf,
/// any trailing partial qualifier, or the full canonical dotted path. For canonical
/// `Core.Runtime.Entry` this matches `Entry`, `Runtime.Entry`, AND `Core.Runtime.Entry` — the two
/// forms the developer ruled must both work (import-to-leaf-then-bare, OR fully qualified). Matching
/// is on segment boundaries (a `.` must precede the matched suffix), so `try` never matches `…Entry`.
/// Import-gating of the bare/partial forms is enforced separately in `enforce_injected` (an unimported
/// bare name is still `E-INJECTED-TYPE-BARE`); the fully-qualified dotted form is self-gating there.
pub(crate) fn attr_path_matches(name: &str, canonical: &str) -> bool {
    name == canonical
        || (!name.is_empty()
            && canonical.len() > name.len()
            && canonical.ends_with(name)
            && canonical.as_bytes()[canonical.len() - name.len() - 1] == b'.')
}

/// The canonical dotted path of every BUILT-IN attribute, as a named const per attribute. Each
/// `is_*` predicate below is defined in terms of exactly one of these, and
/// [`BUILTIN_ATTRIBUTE_PATHS`] lists the same consts — so a new built-in attribute cannot be
/// recognized by the checker while staying invisible to the LSP (the drift that made every attribute
/// in the language uncompletable). Adding one means adding a const, its predicate, and its row in
/// [`BUILTIN_ATTRIBUTE_PATHS`].
///
/// `every_enumerated_attribute_is_recognized` checks ONE of the two directions — that every enumerated
/// row is actually matched by a predicate (so a typo'd or stale const cannot sit in the list). The
/// converse (a predicate with no row, i.e. a built-in that completion would not offer) is NOT
/// mechanically checkable here without a macro over the `impl` block: Rust cannot enumerate methods
/// reflectively. Keeping the consts adjacent to the array is the mitigation, not a proof.
pub mod paths {
    pub const UNCHECKED_OVERFLOW: &str = "Core.Runtime.Integer.UncheckedOverflow";
    pub const INJECTABLE: &str = "Core.DependencyInjection.Injectable";
    pub const PROVIDES: &str = "Core.DependencyInjection.Provides";
    pub const TRANSIENT: &str = "Core.DependencyInjection.Transient";
    pub const ATTRIBUTE: &str = "Core.Runtime.Attribute";
    pub const ENTRY: &str = "Core.Runtime.Entry";
    pub const DEPRECATED: &str = "Core.Runtime.Deprecated";
    pub const CONFIG: &str = "Core.Runtime.Config";
    pub const ROUTE: &str = "Core.Http.Route";
    pub const INVOKE: &str = "Core.Runtime.Invoke";
    pub const TO_STRING: &str = "Core.Runtime.ToString";
}

/// Every built-in attribute's canonical path, with a one-line `detail` for the completion picker.
/// The ONE enumeration — `src/lsp/catalog.rs` reads this rather than re-listing names by hand, the
/// same single-source discipline the catalog already applies to Core modules and natives.
pub const BUILTIN_ATTRIBUTE_PATHS: &[(&str, &str)] = &[
    (
        paths::ATTRIBUTE,
        "marks a class as a user-defined attribute",
    ),
    (paths::CONFIG, "typed-config provider"),
    (paths::DEPRECATED, "deprecation marker (compile-time only)"),
    (paths::ENTRY, "program entry point"),
    (paths::INJECTABLE, "dependency-injection component"),
    (paths::INVOKE, "makes instances callable"),
    (paths::PROVIDES, "static DI factory method"),
    (paths::ROUTE, "HTTP route handler"),
    (paths::TO_STRING, "the method a class stringifies through"),
    (paths::TRANSIENT, "opt out of the shared DI lifetime"),
    (
        paths::UNCHECKED_OVERFLOW,
        "wrapping integer arithmetic (perf opt-in)",
    ),
];

/// The bare leaf of a canonical attribute path — `Core.Runtime.Entry` → `Entry`. The idiomatic
/// spelling at a use site (import-gated), and what completion offers by default.
#[must_use]
pub fn attr_path_leaf(canonical: &str) -> &str {
    canonical.rsplit('.').next().unwrap_or(canonical)
}

impl Attribute {
    /// True iff this is the `#[UncheckedOverflow]` opt-in — whole-function two's-complement WRAPPING
    /// integer arithmetic (the perf escape hatch; canonical `Core.Runtime.Integer.UncheckedOverflow`).
    /// Recognized in every "nothing in the wind" form (bare leaf / partial qualifier / full path) via
    /// [`attr_path_matches`]. SINGLE SOURCE of the recognition — the checker gate, the compiler
    /// `unchecked` flag, the interpreter, and the transpile `E-TRANSPILE-UNCHECKED` gate all consult
    /// this one predicate, so the four can never drift.
    pub fn is_unchecked_overflow(&self) -> bool {
        attr_path_matches(&self.name, paths::UNCHECKED_OVERFLOW)
    }

    /// True iff this is a DI built-in attribute (DI v1). Recognized so the checker does not reject it
    /// as `E-UNKNOWN-ATTRIBUTE` — it is consumed by [`crate::checker::desugar_di`] before any backend,
    /// then inert (like `#[Route]`). SINGLE SOURCE of the recognition; canonical
    /// `Core.DependencyInjection.Injectable`, matched in every import form via [`attr_path_matches`].
    pub fn is_di_builtin(&self) -> bool {
        attr_path_matches(&self.name, paths::INJECTABLE)
    }

    /// True iff this is the DI `#[Provides]` attribute (DI v1 slice 4) — marks a `static` method whose
    /// return type is a provided type: the DI graph constructs that type via the method instead of `new`.
    /// Canonical `Core.DependencyInjection.Provides`, every import form via [`attr_path_matches`].
    pub fn is_di_provides(&self) -> bool {
        attr_path_matches(&self.name, paths::PROVIDES)
    }

    /// True iff this is the DI `#[Transient]` attribute (DI v1 slice 4b) — on a class, opts OUT of the
    /// default-shared lifetime: the DI graph builds a fresh instance at each injection point instead of
    /// sharing one per resolution root. Canonical `Core.DependencyInjection.Transient`, every import form.
    pub fn is_di_transient(&self) -> bool {
        attr_path_matches(&self.name, paths::TRANSIENT)
    }

    /// True iff this is the built-in `#[Attribute]` marker (DEC-194) — a class carrying it IS a
    /// user-defined attribute type. Canonical `Core.Runtime.Attribute`, every import form.
    pub fn is_attribute_marker(&self) -> bool {
        attr_path_matches(&self.name, paths::ATTRIBUTE)
    }

    /// True iff this is the `#[Entry]` program entry-point marker (DEC-191). Canonical
    /// `Core.Runtime.Entry`, recognized in every import form via [`attr_path_matches`] — so
    /// `#[Entry]` (after `import Core.Runtime.Entry;`) AND `#[Core.Runtime.Entry]` (fully qualified,
    /// self-gating) both select the entry point. The single source is [`is_entry_attr`].
    pub fn is_entry(&self) -> bool {
        attr_path_matches(&self.name, paths::ENTRY)
    }

    /// True iff this is the `#[Deprecated(message: "…")]` userland deprecation marker (DEC-417).
    /// Canonical `Core.Runtime.Deprecated`, recognized in every import form via [`attr_path_matches`]
    /// — the `#[Entry]`/`#[Config]` twin, import-gated under the same `Core.Runtime` provider.
    ///
    /// **Compile-time only (DEC-417.2).** No backend ever acts on it: the checker warns at every USE
    /// site with `W-DEPRECATED` and the LSP tags declaration + usages, but nothing is emitted. In
    /// particular it is deliberately NOT mapped to PHP's native `#[\Deprecated]` — that one fires at
    /// runtime onto stdout, which would break Invariant 1 byte-identity against the two Rust legs.
    pub fn is_deprecated(&self) -> bool {
        attr_path_matches(&self.name, paths::DEPRECATED)
    }

    /// The author's `message:` text from `#[Deprecated(message: "…")]`, if given as a PLAIN string
    /// literal. `None` when the attribute carries no message (legal — the warning then names only the
    /// symbol) or when the argument is not a plain literal.
    ///
    /// An INTERPOLATED string (`"use {other}"`) deliberately returns `None`: this is compile-time-only
    /// metadata with no runtime, so there is nothing to evaluate the holes against. The checker rejects
    /// that shape explicitly rather than silently dropping the text. Single source, so the checker
    /// warning, hover text and the lifter cannot drift in how they read it.
    #[must_use]
    pub fn deprecation_message(&self) -> Option<&str> {
        self.args.iter().find_map(|a| match a {
            Expr::NamedArg { name, value, .. } if name == "message" => match value.as_ref() {
                Expr::Str(parts, _) => match parts.as_slice() {
                    [StrPart::Literal(s)] => Some(s.as_str()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
    }

    /// True iff a `message:` argument is present but is NOT a plain string literal (an interpolation,
    /// or any other expression) — the shape the checker rejects. Distinguishes "no message at all"
    /// (fine) from "a message the compiler cannot read" (an error), which
    /// [`Self::deprecation_message`] alone cannot express since both give `None`.
    #[must_use]
    pub fn has_unreadable_deprecation_message(&self) -> bool {
        self.args
            .iter()
            .any(|a| matches!(a, Expr::NamedArg { name, .. } if name == "message"))
            && self.deprecation_message().is_none()
    }

    /// True iff this is the `#[Config]` typed-config provider marker (DEC-318). Canonical
    /// `Core.Runtime.Config`, recognized in every import form via [`attr_path_matches`] — the
    /// `#[Entry]` twin. The single source is [`is_config_attr`].
    pub fn is_config(&self) -> bool {
        attr_path_matches(&self.name, paths::CONFIG)
    }

    /// True iff this is the `#[Route("METHOD", "/path")]` HTTP route handler marker (M6 W2). Canonical
    /// `Core.Http.Route`, every import form via [`attr_path_matches`]. SINGLE SOURCE — the checker
    /// validation and `desugar_router` both consult this, so they cannot drift.
    pub fn is_route(&self) -> bool {
        attr_path_matches(&self.name, paths::ROUTE)
    }

    /// True iff this is the `#[Invoke]` callability marker (DEC-331 D9a) — a class instance carrying
    /// `#[Invoke]` method(s) is callable as `x(args)`. Canonical `Core.Runtime.Invoke`, every import
    /// form via [`attr_path_matches`]; NOT import-gated (bare `#[Invoke]` is legal with no import — the
    /// frozen-spec surface). SINGLE SOURCE for checker validation + the `resolve_invoke_tostring` lowering.
    pub fn is_invoke(&self) -> bool {
        attr_path_matches(&self.name, paths::INVOKE)
    }

    /// True iff this is the `#[ToString]` stringify marker (DEC-331 D9b) — the one method a class
    /// stringifies through (interpolation, `Conversion.toString`); strict zero-param → `string`, one
    /// per class. Canonical `Core.Runtime.ToString`, every import form; NOT import-gated. SINGLE SOURCE.
    pub fn is_to_string(&self) -> bool {
        attr_path_matches(&self.name, paths::TO_STRING)
    }
}

#[cfg(test)]
#[path = "attributes_tests.rs"]
mod tests;

//! AST — enum declarations.

use super::*;

/// One variant of an enum, with optional associated data fields (`Circle(float radius)`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Param>,
    /// DEC-302 backed-enum scalar value — the `= "H"` / `= 1` after a payload-less variant name (PHP
    /// 8.1 backed enum). `Some` iff the enclosing enum has a [`EnumDecl::backing_type`]; a backed
    /// enum's variants are all payload-less (`fields` empty) each with a scalar literal here. Boxed to
    /// keep the common (non-backed) variant small. Checker validates all-or-none / unique / type-match.
    /// `None` for a normal algebraic variant.
    pub backing_value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    /// Declaration-level visibility (default `Public`). Loader-enforced; see [`Visibility`].
    pub vis: Visibility,
    pub name: String,
    /// Generic type parameters, in declaration order — `["T"]` for `enum Option<T>`, `["T", "E"]` for
    /// `enum Result<T, E>` (M-RT generic enums). empty for a non-generic enum — the common case. While
    /// checking the enum, a bare type name in this set resolves to `Ty::Param` in a variant's field
    /// types; a generic value's arguments are inferred at the variant constructor and these parameters
    /// are **erased** (rewritten to `Type::Erased` across every variant) before any backend runs —
    /// the same compile-time-only discipline as generic classes (`Box<T>`).
    pub type_params: Vec<String>,
    /// DEC-211 generic bounds — sparse `(param, Interface)` pairs (see [`FunctionDecl::type_param_bounds`]).
    /// checker-only; erased before any backend.
    pub type_param_bounds: Vec<(String, String)>,
    /// DEC-302 backed-enum scalar backing type — the `: string` / `: int` after the enum name (PHP
    /// 8.1 backed enum). `Some` ⇒ every variant is payload-less with a [`EnumVariant::backing_value`],
    /// enabling `.value` + static `cases()`/`from()`/`tryFrom()`. Mutually exclusive with generics
    /// (a backed enum is payload-less → `type_params` is empty when this is `Some`). `None` for a
    /// normal algebraic enum (the common case).
    pub backing_type: Option<Type>,
    pub variants: Vec<EnumVariant>,
    /// True for a compiler-INJECTED enum (`Json`, `RoundingMode` — added by `cli::inject_*_prelude`
    /// when the matching `Core.*` module is imported), false for a user-declared enum. Its variants
    /// bind ONLY qualified (`Json.Object(…)`, never bare `Object(…)`) — the "nothing in the wind"
    /// rule (variant-qualification B): an injected name a user never wrote must carry its enum.
    pub injected: bool,
    pub span: Span,
}

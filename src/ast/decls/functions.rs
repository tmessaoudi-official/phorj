//! AST — function/method declarations.

use super::*;

/// A function or method declaration. `modifiers` is empty for a free (top-level) function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub modifiers: Vec<Modifier>,
    /// Item-level attributes (`#[Route("GET", "/p")]`, M6 W2) on a free function. **Front-end-only**:
    /// the checker validates them (`E-UNKNOWN-ATTRIBUTE`/`E-ROUTE-*`) and the `Http.autoRouter()`
    /// desugar consumes the `Route` ones; no backend ever reads this field, so it is inert with
    /// respect to the byte-identity spine (like `throws`). empty for a function with no attributes
    /// (the common case) and always empty on a method (attributes are free-function-only this slice).
    pub attrs: Vec<Attribute>,
    /// Declaration-level visibility. Meaningful only for a free (top-level) function; a method or an
    /// interface method signature carries `Visibility::Public` and the loader never checks it.
    pub vis: Visibility,
    pub name: String,
    /// Generic type parameters, in declaration order — `["T", "U"]` for
    /// `function pair<T, U>(T a, U b) -> …` (M-RT S7). empty for a non-generic function. A type
    /// annotation naming one of these (e.g. `T`) resolves to `Ty::Param("T")` while checking this
    /// function, and is erased to `Type::Erased` before any backend runs.
    pub type_params: Vec<String>,
    /// DEC-211 generic bounds — sparse `(param, Interface)` pairs. `<T: Comparable>` → `("T",
    /// "Comparable")`; a bare `<T>` contributes no pair. checker-only (the checker enforces each
    /// bound pre-erasure from the parser AST); erased before any backend, like `type_params`.
    pub type_param_bounds: Vec<(String, String)>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    /// Declared checked-exception set: the `throws T (| T)*` clause (M-faults 2b). empty for a
    /// function that throws nothing. Each member must be a specific subtype of the built-in `Error`
    /// (the bare root is `E-THROWS-TOO-BROAD`). Erased before any backend — the `throws` declaration
    /// is checker-only (PHP has no checked exceptions).
    pub throws: Vec<Type>,
    pub body: Vec<Stmt>,
    /// `declare function …;` — a **foreign** PHP symbol (M8.5 interop): a bodyless signature describing
    /// an existing PHP function. The checker validates calls against `params`/`ret` but skips the
    /// (empty) body; interp/VM refuse to execute a program containing any foreign decl
    /// (`E-FOREIGN-RUNTIME` — foreign code needs the PHP runtime); the transpiler emits references as the
    /// global PHP form (`\name(…)`) and emits no definition. `false` for every ordinary function.
    pub foreign: bool,
    /// `Some(i)` when this (generic) function's declared return type is *exactly* its `i`-th
    /// parameter's type parameter — `id<T>(T x) -> T` ⇒ `Some(0)`, `firstOr<T>(List<T>, T) -> T` ⇒
    /// `Some(1)`. Set by `erase_generics` (computed from the pre-erasure signature, since the type
    /// parameters are cleared there) and read **only** by the VM compiler's `ctype`, which recovers
    /// the erased result's operand type from the argument so `id(7) + 1` specializes on the VM exactly
    /// as the interpreter already evaluates it (S2.1 — closes the documented generic-result interp↔VM
    /// gap for this common shape). Front-end-only and inert to the byte-identity spine (`None` for
    /// every non-generic function and every generic function whose return is not a bare own parameter).
    pub generic_ret_from_param: Option<usize>,
    pub span: Span,
}

/// A synthetic, inert `function main(): void {}` item. The bytecode compiler requires an entry
/// (`ast::entry_point`), but a serve/web program legitimately has none — its entry is `respond`, run
/// via [`crate::vm::Vm::run_entry`], never `main`. Injecting this satisfies the compiler while staying
/// byte-inert: the synthetic `main` is never invoked, exactly as the interpreter's `call_named` never
/// runs `main`. (The future JIT's library/serve compile will reuse it.)
#[must_use]
pub fn synth_empty_main() -> Item {
    Item::Function(FunctionDecl {
        modifiers: Vec::new(),
        // DEC-331 D1: the synthetic inert entry is CLI-shaped — `#[Entry(kind: EntryKind.Cli)]`.
        attrs: vec![crate::ast::entry_attr(
            "Cli",
            Span {
                start: 0,
                len: 0,
                line: 0,
                col: 0,
            },
        )],
        vis: Visibility::Public,
        name: "main".to_string(),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        params: Vec::new(),
        ret: None,
        throws: Vec::new(),
        body: Vec::new(),
        foreign: false,
        generic_ret_from_param: None,
        span: Span {
            start: 0,
            len: 0,
            line: 1,
            col: 1,
        },
    })
}

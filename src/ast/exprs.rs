//! AST — expressions, lambda bodies, params, modifiers, visibility.

use super::*;

/// The built-in generic collection kinds constructible with `new` (DEC-214): `new List<T>()` and
/// `new Map<K,V>()`. `Set` is deferred — the VM has no empty-set construction op (sets are built via
/// `Set.of(...)` natives), so `new Set<T>()` would need a new `Op` (Invariant-3 coupling); a follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollKind {
    List,
    Map,
}

impl CollKind {
    /// The surface type name (`List`/`Map`) used in diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            CollKind::List => "List",
            CollKind::Map => "Map",
        }
    }
    /// The arity of its type arguments (`List<T>` = 1, `Map<K,V>` = 2).
    pub fn arity(self) -> usize {
        match self {
            CollKind::List => 1,
            CollKind::Map => 2,
        }
    }
}

/// Which separator token was written for a member access (DEC-207). Purely **syntactic**: `Dot` for
/// `.`/`?.` (instance access) and `ColonColon` for `::` (class/type-level access). Both parse to the
/// same `Expr::Member` AST; this field only records the surface spelling so the formatter can render
/// it back faithfully. No enforcement in this scaffold — the checker will later reject `.` on statics
/// / `::` on instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberSep {
    Dot,
    ColonColon,
}

/// Expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64, Span),
    Float(f64, Span),
    /// A `decimal` literal — `19.99d` (M-NUM S1). `unscaled`/`scale` are parsed from the literal
    /// **text** so trailing zeros set the scale (`1.50d` ⇒ `{1999? no — 150}, scale 2`; `1.500d` ⇒
    /// scale 3; `100d` ⇒ scale 0). Value = `unscaled × 10^(-scale)`. A literal whose digits overflow
    /// i128 is a lex/parse error (`E-DECIMAL-LITERAL`), known at compile time.
    Decimal {
        unscaled: i128,
        scale: u8,
        span: Span,
    },
    Bool(bool, Span),
    Null(Span),
    /// String literal as interpolation parts; a plain string is a single `Literal` part.
    Str(Vec<StrPart>, Span),
    /// `b"…"` raw byte-string literal — a flat octet sequence, no interpolation.
    Bytes(Vec<u8>, Span),
    Ident(String, Span),
    This(Span),
    /// `[a, b, c]`
    List(Vec<Expr>, Span),
    /// `[k => v, k2 => v2]` — a map literal (M-RT S3). Distinguished from `List` by the `=>` after the
    /// first element; at least one pair (an empty map literal is deferred — `[]` is the empty *list*).
    /// Keys must be `int`/`bool`/`string` (`E-MAP-KEY`); transpiles to a PHP `[k => v]` array.
    Map(Vec<(Expr, Expr)>, Span),
    /// `new List<T>()` / `new Map<K,V>()` / `new Set<T>()` — explicit empty-collection construction
    /// (DEC-214, supersedes the removed empty-`[]` contextual typing of DEC-201). Carries the
    /// collection kind + type arguments; the checker types it directly from the args (self-typed, no
    /// contextual inference), then a pre-backend rewrite (`rewrite_new_coll`) lowers it to an empty
    /// `List`/`Map` so every backend is unchanged. Non-empty literals `[1, 2, 3]` are untouched.
    NewColl {
        kind: CollKind,
        args: Vec<Type>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `value instanceof TypeName` — a runtime type test (M-RT S1). The right operand is a class
    /// *type name* parsed as a type (not an expression), so this is a dedicated variant rather than a
    /// `BinaryOp`. It evaluates to `bool`; in `if (x instanceof C) { … }` the checker smart-casts `x`
    /// to `C` inside the then-block. Transpiles to PHP `$value instanceof TypeName`. (Replaces the
    /// retired value-equality `is` stub.)
    InstanceOf {
        value: Box<Expr>,
        type_name: String,
        span: Span,
    },
    /// `value as TypeName` — a **checked** downcast (M4 casting axis 2). Result type is `TypeName?`:
    /// `value` itself when `value instanceof TypeName` at runtime, else `null` (the Kotlin/Swift `as?`
    /// model — the honest, surprise-free form of TS's unchecked `<T>v`). The RHS is a class/interface
    /// *type name* (parsed as a type, like `InstanceOf`), so this is its own variant. `value` is
    /// evaluated exactly once. Lowers (no new `Op`) to the `IsInstance` predicate + a branch on the
    /// backends; transpiles to a PHP arrow-IIFE `(fn($x) => $x instanceof T ? $x : null)($value)`.
    /// Composes with `??` / if-let smart-cast. (Value *conversion* — `int→float` etc. — is the
    /// separate `Core.Convert` axis; `as` only reinterprets an existing value.)
    Cast {
        value: Box<Expr>,
        type_name: String,
        span: Span,
    },
    /// `callee(args)` — also covers `Circle(2.0)` constructor calls
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        /// DEC-208 slice A: explicit turbofish type arguments written at the call site
        /// (`identity<int>(5)`, `r.queryInto<User>()`). Empty for the common inferred form. The
        /// checker binds these to the callee's generic type parameters (arity + agreement checked);
        /// generics are then erased like any other, so both backends and the PHP output see the
        /// same monomorphic call whether or not turbofish was written (byte-identity). Only the
        /// formatter renders them — every backend match arm ignores this field via `..`.
        type_args: Vec<Type>,
        span: Span,
    },
    /// `object.name` (`safe == false`) or `object?.name` (`safe == true`, nullsafe access:
    /// a `null` receiver short-circuits the whole access to `null` instead of faulting). A
    /// safe *method* call is a `Call` whose `callee` is a `Member { safe: true, .. }` (M3 S2).
    Member {
        object: Box<Expr>,
        name: String,
        safe: bool,
        /// The syntactic separator written (DEC-207): `Dot` for `.`/`?.`, `ColonColon` for `::`.
        /// Records the surface spelling only; both parse identically.
        sep: MemberSep,
        span: Span,
    },
    /// `object[index]`
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// `inner!` — checked force-unwrap of an optional `T?` to `T` (M3 S2.5). The checker requires
    /// `inner: T?` and lints every use (`W-FORCE-UNWRAP`); at runtime a `null` inner is a clean,
    /// byte-identical fault on both backends rather than a crash.
    Force {
        inner: Box<Expr>,
        span: Span,
    },
    /// `parent.m(args)` / `parent(A).m(args)` — a **super/parent dispatch** call (M-RT super/parent).
    /// `ancestor` is `Some("A")` for the qualified `parent(A).…` form (jump to a named ancestor) and
    /// `None` for the immediate `parent.…` form (nearest declaring ancestor). `method` is `"constructor"`
    /// for a parent-constructor call. Resolved (lexically) to a concrete `(declaring_class, method)` by
    /// `ast::resolve_parent_method` — the same single source for the checker and both backends, so it is
    /// NOT front-end-erased (it reaches the backends as a real, non-virtual dispatch).
    ParentCall {
        ancestor: Option<String>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// `<Type>f(args)` — a **return-type overload selector** (M-RT return-overloading, Slice C1). It is
    /// NOT a value cast (`as` is): `ty` names which overload of `call` (a return-overloaded free
    /// function) to select by its return type. Front-end-only — the checker resolves the member and the
    /// `rewrite_ufcs` pass replaces this node with the mangled `Expr::Call` it chose, so no backend ever
    /// sees an `OverloadSelect` (only the `format` printer + AST walk handle it directly).
    OverloadSelect {
        ty: Type,
        call: Box<Expr>,
        span: Span,
    },
    /// `inner?` — error propagation (M-faults Slice 2a). On a `Result<T, E>` operand it unwraps an
    /// `Ok(v)` to `v`, or early-`return`s the `Err(e)` from the enclosing function (which the checker
    /// requires to return `Result<_, E'>` with `E <: E'`). Lowers on both backends to the existing
    /// variant-tag test + `return` (no new `Op`); the `throws`-call mode is added in Slice 2b. Note the
    /// tokenizer munches `??`/`?.` into their own tokens, so a lone `Question` in postfix position is
    /// unambiguously this operator.
    Propagate {
        inner: Box<Expr>,
        span: Span,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `start..end` (exclusive) or `start..=end` (inclusive) — an integer range, materialized to a
    /// `List<int>` by both backends (decision S1-R). Its only role this slice is `for … in`.
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
        span: Span,
    },
    /// `if (cond) { then } else { else }` in **expression** position: both arms are single
    /// expressions and `else` is mandatory (the value flows out). Distinct from the statement
    /// `Stmt::If`; the parser picks expr-vs-stmt by position (M3 S1.3).
    If {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    /// `function(Type param, …) [: RetType] [throws E] => expr` — an expression-body lambda (M3 S3,
    /// Task 3). Block-body lambdas (`function(…) { … }`) are Task 6. `throws` (DEC-222) is the lambda's
    /// declared checked-exception set (empty when absent); its body is checked with these throws in
    /// context (so `throw`/`?` inside discharge against them), and the lambda's function type carries
    /// them. Checker-time only — the backends see an ordinary closure.
    Lambda {
        params: Vec<Param>,
        ret: Option<Type>,
        throws: Vec<Type>,
        body: LambdaBody,
        span: Span,
    },
    /// `obj with { field = expr, … }` — a functional update (M-mut.4a, Fork 2 = B): a fresh instance
    /// copying `object`'s fields with the named ones overridden, **bypassing the constructor**.
    /// `object` must be a concrete class; `fields` names a subset of its (promoted) fields. Lowers to
    /// the existing `Op::MakeInstance` (no new `Op`); transpiles to PHP `clone($obj, ['f' => …])`.
    CloneWith {
        object: Box<Expr>,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// `new <call>` — the mandatory construction keyword (Feature C). Wraps the inner construction
    /// `Expr::Call` (a class instantiation `new Counter()` or an enum-variant construction `new
    /// Some(7)`). The checker validates that the inner callee really is a class/enum variant
    /// (`E-NEW-ON-NONCONSTRUCT`) and that every construction is `new`-wrapped (`E-NEW-REQUIRED`), then
    /// **unwraps** this node to its inner `Call` (`checker::unwrap_new`, alongside `expand_aliases`/
    /// `erase_generics`) **before any backend runs** — so the interpreter/compiler/transpiler never see
    /// it and construction semantics + the byte-identity spine are unchanged. A bare `new` not followed
    /// by a call is a parse error.
    New(Box<Expr>, Span),
    /// `spawn <call>` — start a green task (M6 W4 concurrency, S4.3). `call` is the function /
    /// closure / method call to run as a task; the expression evaluates to a `Task<T>` handle where
    /// `T` is the call's return type. In the **step-2 synchronous-degenerate** model the call runs to
    /// completion immediately at `spawn` (so `join` already has its result); the cooperative scheduler
    /// (build step 4) will instead enqueue it and run it interleaved. `spawn` is a **contextual**
    /// keyword (recognized only when it leads a call — an ordinary identifier everywhere else), per the
    /// [[contextual-var-and-reserved-names]] lesson. Unlike `new`/`html`/aliases this is **NOT** erased
    /// before the backends — it is a real runtime construct (like `Range`). Green threads have no PHP
    /// target: a `spawn` program is quarantined from the PHP oracle and the transpiler emits
    /// `E-CONCURRENCY-NO-PHP`.
    Spawn {
        call: Box<Expr>,
        span: Span,
    },
    /// `html"<h1>{name}</h1>"` — a typed HTML literal (core.html Wave 3). The parser captures it as
    /// interpolation `parts` (literal chunks + `{expr}` holes, exactly like [`Expr::Str`]); the
    /// **checker** resolves each hole by type (an `Html` hole embeds as-is, a `string`/primitive hole
    /// is auto-escaped via `html.text`, anything else is `E-HTML-HOLE`) and rewrites the whole node
    /// into `html.concat([html.raw(chunk), …])` kernel calls, so no backend ever sees this variant —
    /// it is erased to ordinary native calls before the interpreter/compiler/transpiler run, the same
    /// "compile-time sugar, expanded out" treatment as `type` aliases.
    Html(Vec<StrPart>, Span),
    /// `tag"…{expr}…"` — a **tagged-template** literal for ANY tag other than `html` (DEC-212
    /// scaffold). The lexer recognizes any identifier immediately followed by `"` as a tagged
    /// template; the parser routes `html"…"` to [`Expr::Html`] (unchanged) and every other tag to
    /// this variant, carrying the `tag` name plus the same interpolation `parts` model as
    /// [`Expr::Str`]/[`Expr::Html`]. Only the **syntax** is generalized here: the checker currently
    /// rejects every non-html tag with `E-UNKNOWN-TAG` (the `Expr::TaggedTemplate` arm in
    /// `checker/expr/core.rs` is the single hook where the general two-mode protocol/function desugar
    /// is to be added). Because the checker errors, no backend ever sees this variant.
    TaggedTemplate {
        tag: String,
        parts: Vec<StrPart>,
        span: Span,
    },
    /// `inject<T>()` / `inject()` — the compile-time dependency-injection composition root (DI v1,
    /// `docs/plans/di-attributes.plan.md` §1+§6). `ty` is the explicit target type `T` for
    /// `inject<T>()`, or `None` for the annotation-driven bare `inject()` (T comes from the expected
    /// type). Resolved by [`crate::checker::desugar_di`] **before** the checker: the injectable
    /// dependency graph is expanded into plain `new` construction (a synthesized `__di_T()` factory
    /// per root, so per-resolution-root sharing is byte-identical), so no backend ever sees this
    /// variant — the same "compile-time sugar, expanded out before every backend" discipline as
    /// [`Expr::New`]/[`Expr::Html`]. `inject` is a `Core.DependencyInjection` member (import-gated, NOT a keyword —
    /// ruled 2026-07-10 §7): the parser emits this variant only for the explicit turbofish forms
    /// (`inject<T>()`, `DependencyInjection.inject<T>()`); the no-turbofish forms parse as ordinary calls and
    /// [`crate::checker::desugar_di`] converts them to this variant only when `Core.DependencyInjection` is imported.
    Inject {
        ty: Option<Type>,
        /// `true` for the qualified surface `DependencyInjection.inject…`; `false` for bare `inject…`. Determines which
        /// import gates it (`import Core.DependencyInjection;` vs member-import `import Core.DependencyInjection.inject;`).
        qualified: bool,
        span: Span,
    },
    /// `lhs |> rhs` — the pipe operator (DEC-239: PHP-8.5-aligned callable application). Kept as a
    /// real AST node so the FORMATTER round-trips the surface syntax faithfully (`x |> f` must not
    /// reformat to `f(x)`), then expanded out by [`crate::checker::lower_pipes`] **before** the
    /// checker and every backend — the same "compile-time sugar, expanded out" discipline as
    /// [`Expr::New`]/[`Expr::Html`]. Plain form lowers to `rhs(lhs)`; a `%`-placeholder form
    /// (`x |> f(%, 2)`, see [`Expr::PipePlaceholder`]) lowers by whole-argument substitution
    /// (single `%`) or a single-evaluation IIFE (multiple `%`); a contextually-typed pipe lambda
    /// (`x |> (v => v * 2)`) is an `rhs` [`Expr::Lambda`] whose one param has [`Type::Infer`],
    /// resolved by the checker from the piped value's type.
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// A bare `%` in a whole-argument slot of a pipe's top-level RHS call (DEC-239 placeholder
    /// sugar): `x |> f(%, 2)` ≡ `f(x, 2)`. The PARSER produces it only while parsing a pipe RHS and
    /// validates the shape there (`E-PIPE-PLACEHOLDER` anywhere but a whole top-level argument), so
    /// downstream this node exists only as a direct argument of a [`Expr::Pipe`]'s RHS call, and
    /// [`crate::checker::lower_pipes`] substitutes it away before the checker/backends.
    PipePlaceholder(Span),
}

/// The body of a lambda: either a single expression (`=> expr`) or a block of statements
/// (`{ stmts… }`). Only `Expr` is constructed in Task 3; `Block` is added in Task 6.
#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Vec<Stmt>),
}

/// A function/method parameter: `Type name`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub ty: Type,
    pub name: String,
    /// An optional **default value** (M4 default parameters): `bool b = false`. Restricted to a
    /// literal constant by the checker (`E-DEFAULT-PARAM-EXPR`). A parameter with a default is
    /// optional at the call site; the post-check `fill_defaults` pass appends the default expression
    /// to any under-filled call, so the backends only ever see full-arity calls (byte-identity safe).
    /// Defaults must be trailing (`E-DEFAULT-PARAM-ORDER`). **Boxed** so the rare-and-large default
    /// expression does not bloat every `Param` (which is embedded in `ClassMember::Hook`).
    pub default: Option<Box<Expr>>,
    pub span: Span,
}

/// Visibility / binding modifiers on class members and promoted constructor params.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Private,
    Protected,
    /// `private(set)` — asymmetric visibility (DEC-241): the field/static reads at its declared
    /// (read) visibility but may be ASSIGNED only inside the owning class. Only meaningful on a
    /// `mutable` member (an immutable one can't be assigned anywhere); transpiles 1:1 to PHP
    /// 8.4's `private(set)`.
    PrivateSet,
    /// `protected(set)` — as [`Modifier::PrivateSet`], but the owning class AND its subclasses
    /// may assign. Transpiles 1:1 to PHP 8.4's `protected(set)`.
    ProtectedSet,
    Const,
    /// `open` on a class or method (M-RT S6) — opts into extensibility/overridability. Phorj is
    /// **final-by-default** (a non-`open` class can't be `extend`ed; a non-`open` method can't be
    /// overridden), so the `final` keyword is retired. Checker-enforced (`E-EXTEND-FINAL`/
    /// `E-OVERRIDE-FINAL`); the transpiler emits PHP `final` for the *absence* of `open`. The
    /// extensibility axis of the modifier model, orthogonal to `mutable` (mutation) and `static`.
    Open,
    /// `mutable` on a class field or promoted ctor param (M-mut.6) — the field may be reassigned via
    /// `o.f = e`. Immutable by default (a property of the place, not the type); erased in PHP output
    /// (PHP properties are always mutable unless `readonly`). The binding analog of `VarDecl.mutable`.
    Mutable,
    /// `static` on a class field (M-mut.7) — class-level (not per-instance), program-lifetime state
    /// accessed as `ClassName.field`. The Association axis of the modifier model. Transpiles to a PHP
    /// `static` property.
    Static,
    /// `abstract` on a method (M-RT S6b) — a bodyless signature a concrete subclass must implement.
    /// Implicitly `open` (overridable). Legal only in an `abstract class`; the transpiler emits a PHP
    /// `abstract function …;`.
    Abstract,
}

/// Declaration-level visibility on a top-level item (visibility modifiers). A NEW axis, distinct from
/// the member-level `Modifier::{Public,Private,Protected}`. Ordered so `vis >= Visibility::Internal`
/// reads as "at least package-visible": `Private` (this file only) < `Internal` (this package) <
/// `Public` (cross-package; the default). Enforced entirely in the loader; never read by a backend
/// (PHP has no file/package-private declarations), so it is "erased" simply by being ignored
/// downstream — the byte-identity spine is safe by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Visibility {
    Private,
    Internal,
    Public,
}

/// A constructor parameter, which may carry promotion modifiers
/// (`constructor(private string name)`).
#[derive(Debug, Clone, PartialEq)]
pub struct CtorParam {
    pub modifiers: Vec<Modifier>,
    pub ty: Type,
    pub name: String,
    /// DEC-236 — an optional default literal (`public string user = ""`): trailing-only,
    /// literal-only, filled at each `new` site by the checker (the M4 call-fill technique), so
    /// every backend sees a full-arity construction (byte-identity-safe).
    pub default: Option<Box<Expr>>,
    pub span: Span,
}

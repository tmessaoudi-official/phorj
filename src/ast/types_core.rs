//! AST — types, patterns, string parts, match arms, operators.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// `int`, `List<Shape>`, `Map<string, int>` — `args` empty for non-generic.
    Named {
        name: String,
        args: Vec<Type>,
        span: Span,
    },
    /// `T?`
    Optional { inner: Box<Type>, span: Span },
    /// `A | B | C` — a union type (M-RT S4): a value that is *one of* several nominal/primitive types,
    /// the open-composition counterpart to a closed `enum`. Members are in source order here; the
    /// checker normalizes (flatten/dedupe/canonical-sort) into `Ty::Union`. Members are restricted to
    /// classes, interfaces, and primitives (`E-UNION-MEMBER`); transpiles to PHP 8.0 `A|B`.
    Union(Vec<Type>, Span),
    /// `A & B & C` — an intersection type (M-RT S5): a value that satisfies *all* members
    /// simultaneously, the narrowing dual of a union. Members are in source order here; the checker
    /// normalizes (flatten/dedupe/canonical-sort) into `Ty::Intersection`. Members are restricted to
    /// interfaces, plus at most one concrete class (`E-INTERSECT-MEMBER`/`E-INTERSECT-MULTI-CLASS`) —
    /// a value has exactly one class, so two distinct classes are uninhabited. Transpiles to PHP 8.1
    /// `A&B`. `&` binds tighter than `|` in `parse_type`.
    Intersection(Vec<Type>, Span),
    /// `var` — placeholder for an inferred local binding type (resolved by the checker from the
    /// initializer, erased everywhere else). Only valid as a `Stmt::VarDecl` type.
    Infer(Span),
    /// `(int, string) => bool [throws E]` — a first-class function type (M3 S3; DEC-222 adds `throws`).
    /// `throws` is the declared checked-exception set carried by the callable type (empty when the
    /// clause is absent); it is a checker-time discipline (the backends ignore it — a function value is
    /// a plain closure at runtime), so it must be PRESERVED by every rebuild pass (rewrite_alias /
    /// collapse_injected / rewrite_generics), dropped only where the whole type is dropped.
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
        throws: Vec<Type>,
        span: Span,
    },
    /// `[T; N]` — a **fixed-length list** (Phase 1 types slice): a `List<T>` whose length is a
    /// compile-time constant `N`. Distinct from `List<T>` only in the checker (length tracking +
    /// static literal-index bounds + assignability `[T; N] → List<T>`). At runtime it *is* a list
    /// (`Value::List`, erases to a PHP array) — no new `Value`/`Op`; the length is a compile-time-only
    /// guarantee. The backends treat it exactly as `List<T>` (compiler `CTy::List`, transpiler `array`).
    FixedList {
        elem: Box<Type>,
        len: usize,
        span: Span,
    },
    /// An **erased** generic type parameter (M-RT S7). Produced *only* by `checker::erase_generics`,
    /// which rewrites every `Type::Named` that refers to an in-scope type parameter (`T`) into this
    /// after type-checking. No parser ever emits it and no checker pass before erasure sees it; the
    /// backends consume it as the erasure target — the compiler treats it as `CTy::Other`, the
    /// transpiler emits PHP `mixed`. This is the same "compile-time-only, expanded out before any
    /// backend" discipline as `type` aliases (which become their target) and `html"…"`.
    Erased(Span),
}

/// Patterns in `match` arms.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard(Span),
    /// bare identifier — binds the scrutinee (catch-all)
    Binding {
        name: String,
        span: Span,
    },
    Int(i64, Span),
    Float(f64, Span),
    /// A `decimal` literal pattern — `19.99d` in a `match` arm (M-NUM S1). Matches numerically
    /// (scale-insensitive, like `==`): `1.5d` matches a scrutinee of `1.50d`.
    Decimal {
        unscaled: i128,
        scale: u8,
        span: Span,
    },
    Str(String, Span),
    Bool(bool, Span),
    Null(Span),
    /// `Circle(r)`, `Rect(w, h)` — destructure an enum variant
    Variant {
        name: String,
        fields: Vec<Pattern>,
        /// Optional enum qualifier from a `Enum.Variant(binds)` pattern (variant-qualification A2).
        /// `None` for the bare `Variant(binds)` form. The checker validates it names the scrutinee's
        /// enum + the variant belongs; the backends match by `name` alone (the qualifier is a
        /// compile-time check only), so it needs no backend handling.
        enum_qualifier: Option<String>,
        span: Span,
    },
    /// `Circle c` / `Square _` — a **type pattern** for match-over-union (M-RT S4): matches when the
    /// scrutinee is an instance of `type_name` (a class or interface — the same runtime test as
    /// `instanceof`, reusing `Op::IsInstance`), binding it (narrowed to `type_name`) as `binding` for
    /// the arm body. `binding` is `None` for `Type _`. Parsed as two identifiers in pattern position
    /// (`PascalCaseHead lowercaseBinder`); a lone `Circle =>` stays a catch-all `Binding`.
    Type {
        type_name: String,
        binding: Option<String>,
        span: Span,
    },
    /// `Point { x, y }` / `Point { x: px }` / `Line { from: Point { x, y }, to }` — a **struct
    /// pattern** (M-RT pattern cluster S5.2): matches when the scrutinee is an instance of
    /// `type_name` (a class — the same `instanceof` runtime test as a [`Pattern::Type`], reusing
    /// `Op::IsInstance`), then matches each named field's sub-pattern against that field's value.
    /// Each [`FieldPat`] carries the field name and a sub-pattern; shorthand `x` and rename `x: px`
    /// both desugar to a [`Pattern::Binding`] sub-pattern, so all the existing per-backend pattern
    /// recursion (bind / literal / nested struct) is reused without a new field-target enum.
    Struct {
        type_name: String,
        fields: Vec<FieldPat>,
        span: Span,
    },
}

/// One `field: sub-pattern` entry of a [`Pattern::Struct`]. Shorthand `Point { x }` is sugar for
/// `Point { x: x }` — the parser fills `pat` with `Pattern::Binding { name: field }` — so a field
/// target is always a full [`Pattern`] (bind, literal, wildcard, nested struct, …).
#[derive(Debug, Clone, PartialEq)]
pub struct FieldPat {
    pub field: String,
    pub pat: Pattern,
}

/// One segment of a (possibly interpolated) string literal.
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Literal(String),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// Optional arm guard (`pattern when <cond> => …`). The arm matches only when the pattern
    /// matches AND the guard evaluates true; a false guard falls through to the next arm. `None`
    /// for an unguarded arm. A guarded arm does not discharge its shape for exhaustiveness.
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    /// `~` — bitwise NOT on an `int` operand (primitives P2).
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    /// `**` power (Phase 1 operators slice). Type-directed (`int**int→int`, `float**float→float`);
    /// no dedicated `Op` — the compiler lowers it to `Op::CallNative` (`Core.Math.ipow`/`pow`).
    Pow,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Pipe,
    /// `??` null-coalesce (M3 S2).
    Coalesce,
    /// Bitwise operators on `int` operands (primitives P2). PHP-identical integer semantics; shifts
    /// fault on a negative count and yield 0 / sign-fill for a count ≥ 64.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

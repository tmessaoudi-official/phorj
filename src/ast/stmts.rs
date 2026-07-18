//! AST — statements, destructuring, catch clauses.

use super::*;

/// Statements — appear inside function/method bodies.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `Type name = expr;` or `mutable Type name = expr;` (M-mut.1). `mutable` is a *binding*
    /// modifier (a property of the place, not the type) — immutable by default; only a `mutable`
    /// binding may be reassigned via `Stmt::Assign`. Erased in PHP output (PHP locals are always
    /// mutable); checker-only.
    VarDecl {
        ty: Type,
        name: String,
        init: Expr,
        mutable: bool,
        span: Span,
    },
    /// `<lvalue> = expr;` — reassignment (M-mut.1). `target` is an lvalue expression; this slice
    /// accepts only `Expr::Ident` (field/index targets land in M-mut.5/6 and extend this same
    /// statement). The checker enforces the target is `mutable` (`E-ASSIGN-IMMUTABLE`).
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    /// `return;` or `return expr;`
    Return { value: Option<Expr>, span: Span },
    /// `if (cond) { .. } [else { .. } | else if ..]` — else-branch is a block (an
    /// `else if` chain is stored as a single-statement block wrapping a nested `If`).
    ///
    /// `bind` is `Some(name)` for the `if (var name = cond)` form (M3 S2.4): `cond` is the optional
    /// scrutinee, and `name` is smart-cast to the non-optional inner `T` inside `then_block` only.
    If {
        cond: Expr,
        bind: Option<String>,
        then_block: Vec<Stmt>,
        else_block: Option<Vec<Stmt>>,
        span: Span,
    },
    /// `for (Type name in iter) { .. }` — single-binding iteration over a List/Set/string/range.
    /// `for (Type k, Type v in map) { .. }` — two-binding Map iteration (B1): then `ty`/`name` is the
    /// **key** binding and `val` carries the **value** binding `(Type, name)`. `val` is `None` for the
    /// single-binding form.
    For {
        ty: Type,
        name: String,
        val: Option<(Type, String)>,
        iter: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// A condition loop (M-mut.3): `while (cond) { .. }` (`post_cond = false`) or
    /// `do { .. } while (cond);` (`post_cond = true` — the body runs once before the first test).
    /// Lowers to existing `Jump`/`JumpIfFalse` back-edges (F5) — no new loop opcode. while-let
    /// (`while (var x = opt) { .. }`) is desugared by the parser into `while (true) { if (var x = opt)
    /// { .. } else { break; } }`, reusing the if-let lowering, so it needs no representation here.
    While {
        cond: Expr,
        body: Vec<Stmt>,
        post_cond: bool,
        span: Span,
    },
    /// C-style `for (init; cond; step) { .. }` (M-mut.3). Each clause is optional (`for (;;) {}` is
    /// an infinite loop); `init`/`step` are statements (a `VarDecl`/`Assign`/`Expr`), `cond` an
    /// expression. Lowers to the same jump back-edge as `While` with `step` at the continue target.
    CFor {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `break;` — exit the innermost enclosing loop (M-mut.3).
    Break(Span),
    /// `continue;` — skip to the next iteration of the innermost enclosing loop (M-mut.3).
    Continue(Span),
    /// `{ .. }`
    Block(Vec<Stmt>, Span),
    /// `expr;`
    Expr(Expr, Span),
    /// `discard expr;` (M-must-use Slice A) — evaluate `expr` for its side effects and **explicitly**
    /// drop a non-`void`/`empty` result. The escape hatch for the must-use rule: a bare `Stmt::Expr`
    /// of non-`void`/`empty` type is `E-UNUSED-VALUE`, but a `Discard` of any type is accepted. At
    /// runtime and in PHP output it behaves exactly like `Stmt::Expr` (evaluate, drop) — the only
    /// difference is checker-side (the must-use exemption) and in the formatter (prints `discard `).
    Discard(Expr, Span),
    /// `throw expr;` (M-faults 2b). `value` is `never`-typed at the statement level (a `throw`
    /// diverges — it satisfies return-on-all-paths); the thrown value must be `<: Error`.
    Throw { value: Expr, span: Span },
    /// `try { .. } catch (Type name) { .. } [catch …] [finally { .. }]` (M-faults 2b). At least one
    /// `catch` **or** a `finally` is present (parser-enforced). Catches are tried in source order; a
    /// thrown value matches the first clause whose `ty` it is an `instanceof`. `finally_block` runs on
    /// every exit edge (normal, caught, re-propagated, and a `return`/`break`/`continue` escaping the
    /// try). An uncatchable fault (panic) passes straight through every `catch`.
    Try {
        body: Vec<Stmt>,
        catches: Vec<CatchClause>,
        finally_block: Option<Vec<Stmt>>,
        span: Span,
    },
    /// Let-destructuring (Phase 1 slice 5): `var Point { x, y } = p;` (struct, irrefutable) or
    /// `var [a, b] = xs else { … };` (list, refutable). The binders enter the **enclosing** scope (a
    /// binding statement, not a nested block), so they are live for the rest of the block. `else_block`
    /// is present only for the refutable list form and must diverge (Swift `guard let` model — checked
    /// via the totality engine); a present `else` on an irrefutable pattern is a compile error. No new
    /// `Op`/`Value`: the struct form lowers to field reads, the list form to a length-check + indexed
    /// reads (the same ops as an `if`).
    Destructure {
        pat: DestructurePat,
        init: Expr,
        else_block: Option<Vec<Stmt>>,
        span: Span,
    },
}

/// The target of a [`Stmt::Destructure`] (Phase 1 slice 5). A dedicated, flat (no nested sub-patterns)
/// representation — deliberately *not* the match [`Pattern`] enum: a list target is not a match pattern
/// (adding `Pattern::List` would force match-side handling + exhaustiveness), and let-destructuring
/// needs eager binding into the enclosing scope, which the lazy match-binding path does not model.
#[derive(Debug, Clone, PartialEq)]
pub enum DestructurePat {
    /// `Point { x, y }` / `Point { x: px }` — `type_name` is a concrete class; each field binds (with
    /// optional rename). Irrefutable: the init's static type must be assignable to `type_name`.
    Struct {
        type_name: String,
        fields: Vec<DestructureField>,
        span: Span,
    },
    /// `[a, b]` — bind list elements positionally. Refutable on a `List<T>` (mandatory diverging
    /// `else`), irrefutable on a length-matching `[T; N]`.
    List {
        binders: Vec<(String, Span)>,
        span: Span,
    },
    /// `(a, b)` / `(int a, string b)` — bind TUPLE elements positionally (DEC-288). Irrefutable (a
    /// tuple's arity is statically known). Each binder carries an OPTIONAL type: `None` for the
    /// inferred `var (a, b)` form (the checker fills each from the tuple's position type), `Some(T)`
    /// for the explicit `(T a, …)` form (checked assignable-from the tuple's position type). Lowered
    /// by the backends to indexed reads over the erased runtime list (like [`Self::List`]).
    Tuple {
        binders: Vec<(Option<Type>, String, Span)>,
        span: Span,
    },
}

/// One `field` / `field: binding` entry of a struct [`DestructurePat`] (Phase 1 slice 5). Shorthand
/// `Point { x }` fills `binding` with `field`; rename `Point { x: px }` sets `binding = "px"`.
#[derive(Debug, Clone, PartialEq)]
pub struct DestructureField {
    pub field: String,
    pub binding: String,
    pub span: Span,
}

impl DestructurePat {
    /// The variable names this pattern binds, each with its span, in source order. Used by every pass
    /// that introduces the binders into scope (free-var analysis, checker, casing).
    pub fn binders(&self) -> Vec<(String, Span)> {
        match self {
            DestructurePat::Struct { fields, .. } => {
                fields.iter().map(|f| (f.binding.clone(), f.span)).collect()
            }
            DestructurePat::List { binders, .. } => binders.clone(),
            DestructurePat::Tuple { binders, .. } => {
                binders.iter().map(|(_, n, s)| (n.clone(), *s)).collect()
            }
        }
    }

    pub fn span(&self) -> Span {
        match self {
            DestructurePat::Struct { span, .. }
            | DestructurePat::List { span, .. }
            | DestructurePat::Tuple { span, .. } => *span,
        }
    }
}

/// One `catch (Type name) { .. }` clause of a [`Stmt::Try`] (M-faults 2b). `ty` may be a union
/// (`catch (A | B e)`) — `name` is then bound at the union type. Each clause has its own binding,
/// scope, and body.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub ty: Type,
    pub name: String,
    pub body: Vec<Stmt>,
    pub span: Span,
}

//! M-Lift L2 — the PHP AST, expression half (split out of `ast.rs` under Invariant 13, Lane R-5;
//! `ast.rs` re-exports everything here, so `ast::PhpExpr` is still the name).

use super::ast::{PhpParam, PhpType};

/// A PHP expression.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpExpr {
    Int(i64),
    Float(f64),
    /// A safe (non-interpolating) string literal, escapes decoded.
    Str(String),
    /// An interpolating double-quoted string (`"hi $name"`, `"v={$o->total}"`) parsed into a
    /// sequence of literal runs and embedded `$`-rooted access-chain expressions (C-1). The parser
    /// only admits PHP's actual interpolation grammar (a variable followed by `->prop`/`[idx]`/
    /// method-call steps); a top-level operator or dynamic/variable-variable form is rejected loudly.
    Interp(Vec<PhpStrPart>),
    Bool(bool),
    Null,
    /// `$name` — a variable (without the `$`). `$this` arrives as `Var("this")`.
    Var(String),
    /// A bare identifier: a global constant or a function name (when followed by `(`, postfix turns
    /// it into a [`PhpExpr::Call`]).
    Name(String),
    /// `[a, b, k => v]` (and the `array(…)` long form, which parses as a `Call` to `Name("array")`).
    Array(Vec<PhpArrayElem>),
    /// `name: value` — a NAMED argument (PHP 8.0). Needed because `#[Route(path: '/x')]` is the
    /// dominant real-world attribute spelling; phorj accepts named args in the same positions
    /// (DEC-297 for construction, DEC-435 for attributes), so this lifts 1:1 rather than being
    /// reordered away.
    NamedArg {
        name: String,
        value: Box<PhpExpr>,
    },
    Unary {
        op: PhpUnOp,
        expr: Box<PhpExpr>,
    },
    Binary {
        op: PhpBinOp,
        left: Box<PhpExpr>,
        right: Box<PhpExpr>,
    },
    /// An ARROW closure — `fn (T $x): R => expr`, optionally `static` (which changes nothing for a
    /// lift: phorj closures capture lexically, and `static` only forbids `$this`). Lane R, 2026-09-05:
    /// this is the dominant closure shape in real code (every closure in scout's pure modules is
    /// `static fn (…): T =>`), and every one of them was Tier-2 before. A `use (…)` list is accepted
    /// and dropped — phorj captures by value lexically, which is PHP's `use` semantics — except a
    /// by-reference `use (&$x)`, which has no faithful lift and stays refused. Block-bodied closures
    /// (`function (…) { … }`) remain Tier-2 in this slice: a block needs the statement lifter's
    /// scope machinery, which is a second step.
    Closure {
        params: Vec<PhpParam>,
        ret: Option<PhpType>,
        body: Box<PhpExpr>,
    },
    /// `target[]` — the APPEND slot, valid only as the target of `=` (`$xs[] = v`). Lane R-3.
    AppendSlot(Box<PhpExpr>),
    /// `$xs = [];` under a `/** @var list<T> $xs */` docblock (Lane R-6): the empty literal with
    /// the collection type the program itself declared for it.
    EmptyColl(PhpType),
    /// `(int) e` / `(float) e` / `(string) e` / `(bool) e` — a primitive cast (Lane R-3). `ty` is the
    /// canonical phorj primitive (`integer`/`double`/`boolean` are folded by the parser).
    Cast {
        ty: String,
        value: Box<PhpExpr>,
    },
    /// `value instanceof ClassName` (C-46). `class` is a static type name (a dynamic
    /// `$x instanceof $cls` has no Phorj equivalent and is rejected by the parser).
    InstanceOf {
        value: Box<PhpExpr>,
        class: String,
    },
    /// `target = value` (right-associative). `target` is a validated lvalue.
    Assign {
        target: Box<PhpExpr>,
        value: Box<PhpExpr>,
    },
    /// `target op= value` (`+=`, `.=`, `??=`, …). Kept distinct from `Assign` so it round-trips to
    /// Phorj's own compound assignment.
    CompoundAssign {
        target: Box<PhpExpr>,
        op: PhpBinOp,
        value: Box<PhpExpr>,
    },
    /// `++x` / `x++` / `--x` / `x--`.
    IncDec {
        target: Box<PhpExpr>,
        inc: bool,
        prefix: bool,
    },
    /// `cond ? then : els`. `then == None` encodes the elvis form `cond ?: els`.
    Ternary {
        cond: Box<PhpExpr>,
        then: Option<Box<PhpExpr>>,
        els: Box<PhpExpr>,
    },
    /// `callee(args)` — `callee` is typically a `Name` (free function) but may be any expression.
    Call {
        callee: Box<PhpExpr>,
        args: Vec<PhpExpr>,
    },
    /// `recv->name(args)` / `recv?->name(args)`.
    MethodCall {
        recv: Box<PhpExpr>,
        name: String,
        args: Vec<PhpExpr>,
        nullsafe: bool,
    },
    /// `recv->name` / `recv?->name` (property access, no call).
    Member {
        recv: Box<PhpExpr>,
        name: String,
        nullsafe: bool,
    },
    /// `Class::method(args)`.
    StaticCall {
        class: String,
        name: String,
        args: Vec<PhpExpr>,
    },
    /// `Class::CONST`.
    ClassConst {
        class: String,
        name: String,
    },
    /// `Class::$prop`.
    StaticProp {
        class: String,
        name: String,
    },
    /// `base[index]`.
    Index {
        base: Box<PhpExpr>,
        index: Box<PhpExpr>,
    },
    /// `new Class(args)` / `new Class`.
    New {
        class: String,
        args: Vec<PhpExpr>,
    },
    /// `match (subject) { conds => body, …, default => body }`.
    Match {
        subject: Box<PhpExpr>,
        arms: Vec<PhpMatchArm>,
    },
}

/// One segment of an interpolated double-quoted string (C-1): a literal run or an embedded
/// `$`-rooted access-chain expression. Built by the parser from a raw `InterpStr` token.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpStrPart {
    /// Literal text between holes (escapes already decoded).
    Lit(String),
    /// An embedded `$`-rooted access chain (`$name`, `$o->p`, `$a[$k]`, `$o->m()`).
    Expr(Box<PhpExpr>),
}

/// One element of an array literal: `value` or `key => value`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpArrayElem {
    pub key: Option<PhpExpr>,
    pub value: PhpExpr,
}

/// One arm of a `match`: `conds => body`, where `conds == None` is the `default` arm.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpMatchArm {
    /// `None` = the `default` arm; `Some(list)` = one or more comma-separated conditions.
    pub conds: Option<Vec<PhpExpr>>,
    pub body: PhpExpr,
}

/// Binary operators (Tier-1 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhpBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    /// String concatenation `.`.
    Concat,
    /// Loose equality `==`.
    Eq,
    /// Strict equality `===`.
    Identical,
    /// Loose inequality `!=`.
    NotEq,
    /// Strict inequality `!==`.
    NotIdentical,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    /// Null-coalesce `??`.
    Coalesce,
    /// Bitwise `&` `|` `^` and shifts `<<` `>>` (C-47). Map 1:1 to Phorj's bitwise ops.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Prefix unary operators (Tier-1 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhpUnOp {
    /// `!`.
    Not,
    /// `-` (negation).
    Neg,
    /// `~` bitwise NOT (C-47).
    BitNot,
}

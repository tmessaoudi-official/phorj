//! M-Lift L2 — the **PHP AST** (Tier-1 subset) produced by [`super::parser`].
//!
//! Deliberately kept close to PHP semantics, NOT pre-lifted: `array` stays [`PhpExpr::Array`]
//! (its List/Map/Set role is undecided here), `?T` stays [`PhpType::Nullable`]. The lossy mapping to
//! Phorj's typed world (`array` → `List`/`Map`/`Set`, `?T` → `T?`, `??`/`?->` → Phorj equivalents)
//! is **L4's** job (the lifter), not the parser's — separation of concerns keeps each stage honest.
//!
//! Tier boundary: anything outside this AST (closures, references, union types, casts, heredoc,
//! interpolated strings, `try`/`switch`/`namespace`/…) is rejected *loudly* by the parser rather than
//! represented and guessed at. Classes and enums land in L2b (added to [`PhpItem`] then).

/// A parsed PHP source file: a flat sequence of top-level items. PHP interleaves declarations with
/// file-level statements, so [`PhpItem::Stmt`] carries the latter.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpProgram {
    pub items: Vec<PhpItem>,
    /// The file's `namespace A\B;` segments, or empty when the file declares none. Lifts to the phorj
    /// `package` (PascalCase-ized — `E-PKG-CASE` is enforced, so a lowercase PHP namespace segment
    /// cannot be passed through verbatim); an empty namespace keeps the historical `package Main;`.
    ///
    /// Only the SEMICOLON form is represented. PHP's braced form (`namespace A { … }`) can put several
    /// namespaces in one file, which has no phorj analog (one `package` per file) — the parser refuses
    /// it loudly rather than lifting the first and dropping the rest.
    pub namespace: Vec<String>,
    /// `use A\B\C;` / `use A\B\C as D;` imports, in source order. These map to phorj `import` items —
    /// phorj supports import ALIASES natively (`import Core.Output as Out;` [Verified]), so an aliased
    /// PHP `use` lifts to an aliased phorj import instead of being expanded away at lift time.
    pub uses: Vec<PhpUse>,
    /// PHPDoc attached to top-level declarations, keyed by declaration NAME (DEC-419).
    ///
    /// Name-keyed rather than a field on `PhpFunction`/`PhpClass`/`PhpEnum` so exactly one struct
    /// gains a field instead of three plus every construction site — and top-level names are unique in
    /// PHP, so the key is total. Sorted so the lifted output is deterministic (Invariant 10).
    pub docs: std::collections::BTreeMap<String, String>,
}

/// The declared NAME of a top-level item, or `None` for a bare statement (which declares nothing).
///
/// Exhaustive on purpose: a new `PhpItem` variant must say whether it is nameable rather than
/// inheriting `None` from a wildcard and losing its PHPDoc in silence.
pub fn php_item_name(item: &PhpItem) -> Option<&str> {
    match item {
        PhpItem::Function(f) => Some(&f.name),
        PhpItem::Class(c) => Some(&c.name),
        PhpItem::Enum(e) => Some(&e.name),
        PhpItem::Stmt(_) => None,
    }
}

/// A PHP 8 attribute use — `#[Name(args…)]` (LIFT-ATTR).
///
/// `name` is kept VERBATIM as written, including any `\` qualifier and a leading root marker
/// (`\Attribute`, `ORM\Column`). The lifter maps it; the parser stays a faithful reader of the source,
/// the same discipline `parse_qualified_name` already follows for catch types.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpAttribute {
    pub name: String,
    pub args: Vec<PhpExpr>,
    /// 1-based source line, for lift diagnostics.
    pub line: usize,
}

/// A `use A\B\C;` / `use A\B\C as D;` class import.
///
/// `use function …` and `use const …` are deliberately NOT represented: they import a symbol into the
/// current namespace rather than naming a type, and phorj has no equivalent, so the parser refuses them
/// loudly (DEC-166 — never guess) instead of lifting them as if they were class imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhpUse {
    /// The dotted path segments, root marker stripped (`\Doctrine\ORM\Mapping` → `["Doctrine","ORM","Mapping"]`).
    pub path: Vec<String>,
    /// The `as D` alias, if written. `None` means PHP binds the path's LAST segment as the local name —
    /// which is also what an unaliased phorj `import` does, so the two agree without a synthesized alias.
    pub alias: Option<String>,
    /// 1-based source line, for lift diagnostics.
    pub line: usize,
}

/// A top-level item.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpItem {
    Function(PhpFunction),
    Class(PhpClass),
    Enum(PhpEnum),
    Stmt(PhpStmt),
}

/// Member visibility (`public`/`private`/`protected`). A member with no explicit modifier defaults
/// to `Public` (PHP's rule for methods; properties require a modifier or `var`, which we map here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhpVisibility {
    Public,
    Private,
    Protected,
}

/// A class declaration: `[abstract|final] class Name [extends P] [implements I, …] { members }`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpClass {
    /// `#[…]` attributes written above the class, in source order (LIFT-ATTR). Carries PHP's own
    /// `#[\Attribute]` marker, which is what makes a lifted attribute CLASS usable as an attribute.
    pub attrs: Vec<PhpAttribute>,
    pub name: String,
    pub is_abstract: bool,
    pub is_final: bool,
    /// PHP 8.2 `readonly class`: every property (declared or promoted) is readonly. Lifts to phorj's
    /// DEFAULT — fields are immutable unless `mutable` — so the lifter simply emits no `mutable`.
    pub is_readonly: bool,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub members: Vec<PhpMember>,
    pub line: usize,
}

/// A class member: a property, a method, or a class constant.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpMember {
    Prop {
        vis: PhpVisibility,
        /// PHP 8.4 asymmetric visibility (`public private(set) int $x` / bare `private(set)` —
        /// read defaults to public). Lifts 1:1 onto Phorj's DEC-241 `private(set)`/`protected(set)`
        /// modifiers. `None` = symmetric (the common case).
        set_vis: Option<PhpVisibility>,
        is_static: bool,
        is_readonly: bool,
        ty: Option<PhpType>,
        name: String,
        default: Option<PhpExpr>,
    },
    Method(PhpMethod),
    /// `const NAME = value;`.
    Const {
        vis: PhpVisibility,
        /// PHP 8.3 typed class constant (`const string NAME = …`). Lifted as the declared type; an
        /// untyped constant keeps inferring its type from the literal.
        ty: Option<PhpType>,
        name: String,
        value: PhpExpr,
    },
}

/// A method: like a function plus visibility/static/abstract/final. `body == None` for an abstract
/// method (`function f();`).
#[derive(Debug, Clone, PartialEq)]
pub struct PhpMethod {
    pub vis: PhpVisibility,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_final: bool,
    pub name: String,
    pub params: Vec<PhpParam>,
    pub ret: Option<PhpType>,
    pub body: Option<Vec<PhpStmt>>,
    pub line: usize,
}

/// A PHP 8.1 enum: `enum Name [: backing] [implements I, …] { case …; methods… }`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpEnum {
    pub name: String,
    /// Backing type for a backed enum (`enum Suit: string`); `None` for a pure enum.
    pub backing: Option<PhpType>,
    pub implements: Vec<String>,
    pub cases: Vec<PhpEnumCase>,
    pub methods: Vec<PhpMethod>,
    pub line: usize,
}

/// One enum case: `case Name;` or `case Name = value;` (backed).
#[derive(Debug, Clone, PartialEq)]
pub struct PhpEnumCase {
    pub name: String,
    pub value: Option<PhpExpr>,
}

/// A typed top-level function: `function name(params): ret { body }`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpFunction {
    /// `#[…]` attributes written above the declaration, in source order (LIFT-ATTR). Empty is the
    /// overwhelming common case, so this costs a `Vec` header per function and nothing else.
    pub attrs: Vec<PhpAttribute>,
    pub name: String,
    pub params: Vec<PhpParam>,
    /// Declared return type, if any (`: int`). `None` = no return hint.
    pub ret: Option<PhpType>,
    pub body: Vec<PhpStmt>,
    /// 1-based source line of the `function` keyword (for lift diagnostics).
    pub line: usize,
}

/// A function/method parameter. The leading `$` is stripped from `name`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpParam {
    /// Declared type hint, if any.
    pub ty: Option<PhpType>,
    pub name: String,
    /// Default value (`= expr`), if any. Tier-1: a literal or simple constant expression.
    pub default: Option<PhpExpr>,
    /// Constructor-promotion visibility: `Some(vis)` when a `__construct` param carries a
    /// `public`/`private`/`protected` modifier (PHP 8.0 promoted property), else `None`.
    pub promotion: Option<PhpVisibility>,
    /// `readonly` on a promoted constructor parameter (PHP 8.1). Retained so the lifted field is
    /// immutable — phorj's default — rather than `mutable`.
    pub is_readonly: bool,
}

/// A PHP type hint. Tier-1 = a single name or a nullable single name. Union types (`A|B`) can't even
/// be lexed (the lexer has no bare `|`), so they're excluded at the token level by construction.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpType {
    /// `int`, `float`, `string`, `bool`, `void`, `array`, `mixed`, or a class/enum name.
    Named(String),
    /// `?T` — a nullable type.
    Nullable(Box<PhpType>),
}

/// A PHP statement.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpStmt {
    /// `return;` or `return expr;`.
    Return(Option<PhpExpr>),
    /// An expression used for effect: `foo();`, `$x = 1;`, `$i++;`.
    Expr(PhpExpr),
    /// `if (cond) { then } elseif (c) { … } else { els }`. Bodies are statement lists (a single
    /// brace-less statement is parsed into a one-element list).
    If {
        cond: PhpExpr,
        then: Vec<PhpStmt>,
        /// Zero or more `elseif`/`else if` clauses, in source order.
        elifs: Vec<(PhpExpr, Vec<PhpStmt>)>,
        els: Option<Vec<PhpStmt>>,
    },
    /// `while (cond) { body }`.
    While {
        cond: PhpExpr,
        body: Vec<PhpStmt>,
    },
    /// `for (init; cond; step) { body }`. Each clause is optional (`for (;;)`).
    For {
        init: Option<PhpExpr>,
        cond: Option<PhpExpr>,
        step: Option<PhpExpr>,
        body: Vec<PhpStmt>,
    },
    /// `foreach ($array as $value)` or `foreach ($array as $key => $value)`. Names are `$`-stripped.
    Foreach {
        array: PhpExpr,
        key: Option<String>,
        value: String,
        body: Vec<PhpStmt>,
    },
    /// `echo a, b, c;`.
    Echo(Vec<PhpExpr>),
    Break,
    Continue,
    /// A brace block `{ … }` used as a statement.
    Block(Vec<PhpStmt>),
    /// `throw <expr>;` (2026-07-31). The STATEMENT form only — PHP 8's throw-as-an-EXPRESSION
    /// (`$x = $y ?? throw new E()`) stays outside the subset and is refused by the expression parser,
    /// because a wrong lift there would move where the throw happens.
    Throw(PhpExpr),
    /// `try { … } catch (T $e) { … } … finally { … }` (LIFT-TRY, 2026-07-31).
    ///
    /// The lift subset had NO exception handling at all, which is why `using` could not round-trip:
    /// `using` lowers to `try`/`finally`, so raising that shape back needed the whole family to exist
    /// first. PHP allows `catch (A | B $e)` and — since PHP 8 — a catch with NO variable, so both are
    /// represented rather than assumed away.
    Try {
        body: Vec<PhpStmt>,
        catches: Vec<PhpCatch>,
        finally_block: Option<Vec<PhpStmt>>,
    },
}

/// One `catch (T $e) { … }` clause. `types` holds the union members (one entry for the common case);
/// `var` is `None` for PHP 8's variable-less `catch (T)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhpCatch {
    pub types: Vec<String>,
    pub var: Option<String>,
    pub body: Vec<PhpStmt>,
}

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

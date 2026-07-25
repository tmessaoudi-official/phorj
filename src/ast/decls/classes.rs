//! AST — class/trait declarations, members, and multi-inheritance resolutions.

use super::*;

/// A member of a class.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Field {
        modifiers: Vec<Modifier>,
        ty: Type,
        name: String,
        /// A field-level initializer (`static mutable int total = 0;`). Required for `static` fields
        /// (class-level state has no constructor to set it); must be `None` for an instance field
        /// (those are set via the constructor). Restricted to a literal constant this slice (M-mut.7).
        init: Option<Expr>,
        span: Span,
    },
    Constructor {
        /// Modifiers on the `constructor` keyword itself — its *own* visibility
        /// (`private`/`protected`/`public`), distinct from the per-param promotion modifiers in
        /// `params`. Enforced at the construction site (`E-CTOR-VISIBILITY`); non-visibility
        /// modifiers here are rejected (`E-CTOR-MODIFIER`). Previously parsed and dropped.
        modifiers: Vec<Modifier>,
        params: Vec<CtorParam>,
        /// Declared checked-exception set of the constructor — the `constructor(…) throws E (| E)*`
        /// clause (DEC-221). Empty for a constructor that throws nothing. Semantically identical to
        /// [`FunctionDecl::throws`]: each member must be a specific subtype of the built-in `Error`,
        /// the ctor BODY discharges throwing calls against it, and `new X(…)` propagates it to the
        /// construction site (which must `try`/`catch` it or `?`-propagate + declare `throws`). Erased
        /// before any backend — a throwing ctor transpiles to an ordinary PHP constructor whose body
        /// `throw`s (PHP has no checked exceptions).
        throws: Vec<Type>,
        body: Vec<Stmt>,
        span: Span,
    },
    Method(FunctionDecl),
    /// A **property hook** (M-mut.7b) — a member that looks like a field but computes on read and/or
    /// intercepts writes: `T name { get => expr; set(T v) { stmts } }`. v1 is *virtual-only*: it
    /// declares no storage and takes no initializer, so it is never an instance field (no slot in the
    /// instance map, never promoted, invisible to `clone with`). A `get` is an expression evaluated
    /// with `this` in scope (a read-only computed property); a `set` is a block with the assigned
    /// value bound to its parameter `v`, run with `this` in scope (typically writing other `mutable`
    /// fields). A hook may have get-only, set-only, or both. Reading a get-less hook is
    /// `E-HOOK-NO-GET`; writing a set-less one is `E-HOOK-NO-SET`. Lowers on the VM to synthetic
    /// methods `<Class>::<name>$get`/`$set` dispatched via `Op::CallMethod` (no new `Op`);
    /// transpiles 1:1 to a PHP 8.4 property hook.
    Hook {
        ty: Type,
        name: String,
        /// `get => <expr>` — the computed-read body; `None` for a write-only hook.
        get: Option<Expr>,
        /// `set(T v) { <stmts> }` — the intercepted-write body; the `Param` carries `v`'s name+type.
        /// `None` for a read-only computed hook.
        set: Option<(Param, Vec<Stmt>)>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    /// Declaration-level visibility (default `Public`). Loader-enforced; see [`Visibility`].
    pub vis: Visibility,
    /// Leading `#[…]` attributes on the class declaration (DEC-194 user-attribute system, slice 2a).
    /// Inert metadata until a later slice reads them via reflection; the checker validates each attribute
    /// is recognized (a built-in or, later, a declared user attribute) and legal on a class target.
    pub attrs: Vec<Attribute>,
    pub name: String,
    /// Generic type parameters, in declaration order — `["T"]` for `class Box<T>`, `["A", "B"]` for
    /// `class Pair<A, B>` (M-RT generics-all). empty for a non-generic class — the common case. While
    /// checking the class, a bare type name in this set resolves to `Ty::Param`; a generic instance's
    /// arguments are inferred at construction and these parameters are **erased** (rewritten to
    /// `Type::Erased` across every member) before any backend runs.
    pub type_params: Vec<String>,
    /// DEC-211 generic bounds — sparse `(param, Interface)` pairs (see [`FunctionDecl::type_param_bounds`]).
    /// checker-only; erased before any backend.
    pub type_param_bounds: Vec<(String, String)>,
    /// Parent classes this class `extends` (M-RT S6). empty for a root class; one entry for single
    /// inheritance (`class Dog extends Animal`); two or more for multiple inheritance
    /// (`class Duck extends Swimmer, Flyer`). Each parent must be an `open` class
    /// (`E-EXTEND-FINAL` otherwise); a cycle is `E-MI-CYCLE`. The checker flattens the transitive
    /// supertype set (`ast::class_supertypes`) for subtyping/`instanceof`, and inherits the parents'
    /// fields and methods into this class. Multi-parent collisions must be explicitly resolved (S6b).
    pub extends: Vec<String>,
    /// Interfaces this class declares it implements (`class Dog implements Speaker, Named`). The
    /// checker (`E-IFACE-IMPL`/`E-IFACE-UNIMPL`/`E-IFACE-SIG`) validates each name resolves to an
    /// interface and the class provides every method of it and its `extends` chain (M-RT S2).
    pub implements: Vec<String>,
    /// Type arguments per `implements` entry (DEC-257 generic interfaces) — parallel to
    /// [`Self::implements`], one (possibly empty) argument list per name:
    /// `implements Iterator<int>` ⇒ `implements[i] == "Iterator"`, `implements_args[i] == [int]`.
    /// Empty for the common non-generic case. Checker-only (conformance substitution); **erased**
    /// with the rest of the generic machinery before any backend — the transpiler and both engines
    /// only ever read the names.
    pub implements_args: Vec<Vec<Type>>,
    /// `open class` — whether this class may be `extend`ed (M-RT S6). **Final-by-default**: a
    /// non-`open` class is a leaf (`E-EXTEND-FINAL` if a subclass names it). The transpiler emits a
    /// PHP `final class` for a non-`open` class. The extensibility opt-in, orthogonal to `vis`.
    pub open: bool,
    /// `abstract class` (M-RT S6b) — cannot be instantiated (`E-ABSTRACT-INSTANTIATE`); may declare
    /// `abstract` (bodyless) methods that a concrete subclass must implement (`E-ABSTRACT-UNIMPL`).
    /// Abstract implies extensible, so the parser also sets `open` for an abstract class.
    pub is_abstract: bool,
    /// `sealed class` (W5-3) — a closed hierarchy: its permitted subtypes are exactly those declared
    /// in the whole program, so a `match` over this class type is exhaustive with no `_` (DEC-179).
    /// `sealed` implies `open` (a sealed class exists to be subclassed), and is compile-time-only —
    /// it erases in PHP output (rides the `open` = non-`final` emission; PHP has no sealed classes).
    pub sealed: bool,
    /// Explicit multi-inheritance resolution clauses (M-RT S6b), declared in the class body before/among
    /// members: `use P.m` (pick `P`'s `m` for the colliding name), `rename P.m as n` (rebind `P`'s `m`
    /// under a fresh name `n`, removing it from the collision), `exclude P.m` (drop `P`'s `m`). empty
    /// for a single-parent or collision-free class. Consumed by `ast::class_method_origins` (dispatch)
    /// and the transpiler (`insteadof`/`as` emission). An unresolved cross-parent method collision is
    /// `E-MI-CONFLICT`.
    pub resolutions: Vec<Resolution>,
    /// Traits this class composes via `use T;` (M-RT S8). Each names a `trait` whose members are
    /// flattened into this class (methods registered for dispatch, fields/const/static/hooks/ctor
    /// folded in) **before any backend runs** — a trait is reuse, not a supertype, so it never enters
    /// the `instanceof`/subtype tables. Trait-vs-trait collisions reuse the same `resolutions` clauses
    /// as multi-parent collisions (a clause's "parent" may name a `use`d trait). The transpiler emits a
    /// native PHP `trait`/`use`. empty for a class that composes no traits.
    pub uses: Vec<UseTrait>,
    pub members: Vec<ClassMember>,
    /// `declare class …` — a **foreign** PHP class (M8.5 interop): a signature-only description of an
    /// existing PHP class (constructor / methods / static methods / public fields). Checked like a normal
    /// class for member resolution but its methods are bodyless; interp/VM refuse a program using it
    /// (`E-FOREIGN-RUNTIME`); the transpiler emits references as the global PHP form (`new \Name`,
    /// `\Name::s`, `$o->m`) and emits no class definition. `false` for every ordinary class.
    pub foreign: bool,
    pub span: Span,
}

/// A `use T;` trait-composition clause in a class body (M-RT S8) — see [`ClassDecl::uses`]. Named by
/// the trait's bare name (`package Main`-only this slice). Distinguished at parse time from an S6b
/// resolution clause (`use P.m`) by dot-lookahead: a `.` after the name is a resolution clause, a
/// `,`/`;` is trait composition.
#[derive(Debug, Clone, PartialEq)]
pub struct UseTrait {
    pub name: String,
    pub span: Span,
}

/// A trait declaration (`trait T { members }`, M-RT S8) — horizontal code reuse that is **not a type**
/// (a variable can never be typed `T`; `instanceof T` is rejected). Its members use the exact same
/// grammar as class members (methods with any visibility, instance fields with `mutable`/immutable,
/// `const`, `static`, property hooks, a constructor, and `abstract` requirements). A class composes a
/// trait with `use T;`; the trait's members are flattened into the using class before any backend, so
/// the interpreter/VM see ordinary class members. The transpiler emits a native PHP `trait`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

/// A multi-inheritance conflict-resolution clause (M-RT S6b) — see [`ClassDecl::resolutions`]. Each
/// names a **direct parent** and one of its methods; the checker validates the parent/method exist and
/// that every cross-parent collision is resolved (`E-MI-CONFLICT`).
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// `use P.m` — pick parent `P`'s `m` as the winner for the method name `m`; other parents' `m` drop.
    Use {
        parent: String,
        method: String,
        span: Span,
    },
    /// `rename P.m as n` — bind parent `P`'s `m` under the new name `n` (and remove it from the `m`
    /// collision, so a single remaining source resolves `m`).
    Rename {
        parent: String,
        method: String,
        as_name: String,
        span: Span,
    },
    /// `exclude P.m` — drop parent `P`'s contribution to the method name `m`.
    Exclude {
        parent: String,
        method: String,
        span: Span,
    },
}

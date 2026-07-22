//! AST — declarations: functions, attributes, enums, class members/decls, traits,
//! interfaces, items.

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
    /// (empty) body; `run`/`runvm` refuse to execute a program containing any foreign decl
    /// (`E-FOREIGN-RUNTIME` — foreign code needs the PHP runtime); the transpiler emits references as the
    /// global PHP form (`\name(…)`) and emits no definition. `false` for every ordinary function.
    pub foreign: bool,
    /// `Some(i)` when this (generic) function's declared return type is *exactly* its `i`-th
    /// parameter's type parameter — `id<T>(T x) -> T` ⇒ `Some(0)`, `firstOr<T>(List<T>, T) -> T` ⇒
    /// `Some(1)`. Set by `erase_generics` (computed from the pre-erasure signature, since the type
    /// parameters are cleared there) and read **only** by the VM compiler's `ctype`, which recovers
    /// the erased result's operand type from the argument so `id(7) + 1` specializes on the VM exactly
    /// as the interpreter already evaluates it (S2.1 — closes the documented generic-result run↔runvm
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
        // DEC-191: the synthetic entry carries #[Entry] — the compiler resolves by attribute now.
        attrs: vec![Attribute {
            name: "Entry".to_string(),
            args: Vec::new(),
            span: Span {
                start: 0,
                len: 0,
                line: 0,
                col: 0,
            },
        }],
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

impl Attribute {
    /// True iff this is the `#[UncheckedOverflow]` opt-in — whole-function two's-complement WRAPPING
    /// integer arithmetic (the perf escape hatch; canonical `Core.Runtime.Integer.UncheckedOverflow`).
    /// Recognized in every "nothing in the wind" form (bare leaf / partial qualifier / full path) via
    /// [`attr_path_matches`]. SINGLE SOURCE of the recognition — the checker gate, the compiler
    /// `unchecked` flag, the interpreter, and the transpile `E-TRANSPILE-UNCHECKED` gate all consult
    /// this one predicate, so the four can never drift.
    pub fn is_unchecked_overflow(&self) -> bool {
        attr_path_matches(&self.name, "Core.Runtime.Integer.UncheckedOverflow")
    }

    /// True iff this is a DI built-in attribute (DI v1). Recognized so the checker does not reject it
    /// as `E-UNKNOWN-ATTRIBUTE` — it is consumed by [`crate::checker::desugar_di`] before any backend,
    /// then inert (like `#[Route]`). SINGLE SOURCE of the recognition; canonical
    /// `Core.DependencyInjection.Injectable`, matched in every import form via [`attr_path_matches`].
    pub fn is_di_builtin(&self) -> bool {
        attr_path_matches(&self.name, "Core.DependencyInjection.Injectable")
    }

    /// True iff this is the DI `#[Provides]` attribute (DI v1 slice 4) — marks a `static` method whose
    /// return type is a provided type: the DI graph constructs that type via the method instead of `new`.
    /// Canonical `Core.DependencyInjection.Provides`, every import form via [`attr_path_matches`].
    pub fn is_di_provides(&self) -> bool {
        attr_path_matches(&self.name, "Core.DependencyInjection.Provides")
    }

    /// True iff this is the DI `#[Transient]` attribute (DI v1 slice 4b) — on a class, opts OUT of the
    /// default-shared lifetime: the DI graph builds a fresh instance at each injection point instead of
    /// sharing one per resolution root. Canonical `Core.DependencyInjection.Transient`, every form.
    pub fn is_di_transient(&self) -> bool {
        attr_path_matches(&self.name, "Core.DependencyInjection.Transient")
    }

    /// True iff this is the built-in `#[Attribute]` marker (DEC-194) — a class carrying it IS a
    /// user-defined attribute type. Canonical `Core.Runtime.Attribute`, every import form.
    pub fn is_attribute_marker(&self) -> bool {
        attr_path_matches(&self.name, "Core.Runtime.Attribute")
    }

    /// True iff this is the `#[Entry]` program entry-point marker (DEC-191). Canonical
    /// `Core.Runtime.Entry`, recognized in every import form via [`attr_path_matches`] — so
    /// `#[Entry]` (after `import Core.Runtime.Entry;`) AND `#[Core.Runtime.Entry]` (fully qualified,
    /// self-gating) both select the entry point. The single source is [`is_entry_attr`].
    pub fn is_entry(&self) -> bool {
        attr_path_matches(&self.name, "Core.Runtime.Entry")
    }

    /// True iff this is the `#[Config]` typed-config provider marker (DEC-318). Canonical
    /// `Core.Runtime.Config`, recognized in every import form via [`attr_path_matches`] — the
    /// `#[Entry]` twin. The single source is [`is_config_attr`].
    pub fn is_config(&self) -> bool {
        attr_path_matches(&self.name, "Core.Runtime.Config")
    }

    /// True iff this is the `#[Route("METHOD", "/path")]` HTTP route handler marker (M6 W2). Canonical
    /// `Core.Http.Route`, every import form via [`attr_path_matches`]. SINGLE SOURCE — the checker
    /// validation and `desugar_router` both consult this, so they cannot drift.
    pub fn is_route(&self) -> bool {
        attr_path_matches(&self.name, "Core.Http.Route")
    }
}

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
    /// class for member resolution but its methods are bodyless; `run`/`runvm` refuse a program using it
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

/// An interface declaration (`interface Speaker { method-sigs } [extends A, B]`). Methods are
/// signatures only — a `FunctionDecl` with an empty body (M-RT S2). Interfaces are nominal types
/// usable as a variable/parameter type; a class that `implements` one is a subtype of it. PHP-absent
/// at runtime: there are no interface instances, so the backends only use interfaces for the
/// `instanceof` table and (the transpiler) for emitting a PHP `interface`.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    /// Declaration-level visibility (default `Public`). Loader-enforced; see [`Visibility`].
    pub vis: Visibility,
    pub name: String,
    /// Generic type parameters (`interface Iterator<T>`, DEC-257) — same compile-time-only
    /// discipline as generic classes: a bare name in this set resolves to `Ty::Param` in method
    /// signatures; a class's `implements Iterator<int>` substitutes them for conformance; **erased**
    /// before any backend. Empty for the common non-generic case.
    pub type_params: Vec<String>,
    /// Parent interfaces (`interface Animal extends Speaker, Named`) — flattened transitively.
    pub extends: Vec<String>,
    /// Method signatures (each a `FunctionDecl` with an empty body).
    pub methods: Vec<FunctionDecl>,
    /// `sealed interface` (W5-3) — a closed hierarchy: its permitted implementors are exactly those
    /// declared in the whole program, so a `match` over this interface type is exhaustive with no `_`
    /// (DEC-179). Compile-time-only — PHP emits a plain `interface` (no sealed concept).
    pub sealed: bool,
    /// True for a compiler-INJECTED interface (`Iterator` — added by the `Core.IteratorModule` prelude
    /// when imported), false for a user declaration. Injected interfaces are exempt from the
    /// DEC-202 PHP-builtin-name rejection: the transpiled output is namespaced (`namespace Main;
    /// interface Iterator` never redeclares the root `\Iterator` — verified vs PHP 8.5), and the
    /// name is compiler-owned, not user-chosen.
    pub injected: bool,
    pub span: Span,
}

/// A top-level item in a program.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// `import a.b.c;` or `import a.b.c as leaf;` — `alias`, when present, overrides the call-site
    /// qualifier (the bound leaf) so colliding leaves from different packages can coexist (M5 S2c,
    /// design O-9). `None` ⇒ the qualifier is `path`'s last segment.
    Import {
        path: Vec<String>,
        alias: Option<String>,
        span: Span,
    },
    Function(FunctionDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
    /// `trait T { members }` — horizontal reuse composed by a class via `use T;` (M-RT S8). Not a type.
    Trait(TraitDecl),
    /// `type Name = Type;` — a compile-time alias, erased after checking (resolved by the checker
    /// and expanded out of the AST before any backend runs).
    TypeAlias {
        name: String,
        ty: Type,
        span: Span,
    },
    /// `test "name" { stmts }` — a unit test (M-Test T1). `test` is a *contextual* keyword (special
    /// only at item position when immediately followed by a string literal), so it stays usable as an
    /// identifier elsewhere. The body is checked like a `-> void` function body with no `this`. A test
    /// item is valid only under `phg test` (test mode); in a normal build the checker rejects it as
    /// `E-TEST-OUTSIDE-TESTS`. It is never reached by a backend in a normal compile — the `phg test`
    /// runner executes test bodies directly on the interpreter (M-Test T3).
    Test {
        name: String,
        body: Vec<Stmt>,
        span: Span,
    },
}

/// A whole parsed program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// The file's package path (`package App.Util;` ⇒ `["App", "Util"]`). empty only for a
    /// malformed file with no declaration — the checker rejects that as `E-NO-PACKAGE` (M5: every
    /// file is packaged, never inferred). The reserved `["Main"]` is the runnable entry (M5 S1).
    pub package: Vec<String>,
    pub items: Vec<Item>,
    pub span: Span,
}

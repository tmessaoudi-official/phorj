//! Sound static type-checker. Two sub-phases: `collect` (hoist decls + prelude),
//! then `check` (walk bodies). Returns all type errors at once.

use std::collections::HashMap;

use crate::ast::Program;
use crate::diagnostic::{Diagnostic, Stage};
use crate::limits::MAX_EXPR_DEPTH;
use crate::token::Span;
use crate::types::Ty;

// Self-contained post-check AST-rewrite passes (M-Decomp W1.3): each is a pure `Program -> Program`
// expansion run before the backends (alias expansion, generic erasure, `html"…"` hole resolution).
// Re-exported so callers keep using `checker::expand_aliases` etc.
mod collapse_injected;
mod desugar_db;
mod desugar_di;
mod desugar_router;
mod enforce_injected;
mod function_imports;
mod inline_parent_ctor;
mod intrinsic_imports;
mod overloads;
mod resolve_variant_imports;
mod rewrite_alias;
mod rewrite_fills;
mod rewrite_generics;
mod rewrite_html;
mod rewrite_new;
mod rewrite_pipe;
mod rewrite_ufcs;
pub use collapse_injected::collapse_injected_type_qualifiers;
pub use desugar_db::desugar_db;
pub use desugar_di::desugar_di;
pub use desugar_router::desugar_auto_router;
pub use enforce_injected::enforce_injected_discipline;
pub use inline_parent_ctor::inline_parent_ctors;
pub use intrinsic_imports::resolve_intrinsic_imports;
pub use overloads::rename_overload_defs;
pub use resolve_variant_imports::resolve_variant_imports;
pub use rewrite_alias::expand_aliases;
pub use rewrite_fills::apply_default_fills;
pub use rewrite_generics::erase_generics;
pub use rewrite_html::resolve_html;
pub use rewrite_new::{inject_optional_field_defaults, unwrap_new};
pub use rewrite_pipe::{lower_pipes, materialize_pipe_params};
pub use rewrite_ufcs::rewrite_ufcs;

// impl-cluster cohesion split (M-Decomp W2): one `impl Checker` block per cluster
// file; all share the private struct via `use super::*`.
mod assign;
mod calls;
mod casing;
mod collect;
pub(crate) mod common;
mod expr;
mod matches;
mod program;
mod reflect;
mod resolve;
mod stmt;
mod throws;

// Stateless helpers live in `common`; this glob re-exposes them to `mod.rs`'s own bodies AND
// (transitively, via each cluster's `use super::*`) to every sibling cluster file.
use common::*;

#[derive(Clone)]
struct FnSig {
    params: Vec<Ty>,
    /// Per-parameter default value (M4 default parameters): `Some(literal)` for a defaulted (trailing)
    /// parameter, `None` for a required one. Parallel to `params`. The count of leading `None`s is the
    /// call's *required* arity; an under-filled call records a fill of the trailing defaults, applied
    /// by `fill_defaults` before any backend. Empty-of-defaults (all `None`) is the common case.
    defaults: Vec<Option<crate::ast::Expr>>,
    ret: Ty,
    /// Generic type parameters this function declares (`["T"]` for `function id<T>(T x) -> T`).
    /// Empty for a non-generic function — the common case. When non-empty, `params`/`ret` contain
    /// `Ty::Param` occurrences that a call site unifies away (M-RT S7). Free functions AND class
    /// methods may be generic (M-RT generics-all); interface method signatures stay non-generic
    /// (the parser builds them with empty `type_params`), so theirs is always empty.
    type_params: Vec<String>,
    /// Per-type-param bounds (DEC-211): `(param, Interface)` pairs. At a generic call site, after the
    /// arguments bind each `T` (θ), the concrete binding must implement its bound (`E-BOUND-NOT-SATISFIED`)
    /// — the soundness guarantee that makes the def-site's bounded-`T` member resolution safe post-erasure.
    /// Empty for non-generic/unbounded functions and interface methods.
    type_param_bounds: Vec<(String, String)>,
    /// Checked exception types this function declares (`throws A | B` ⇒ `[A, B]`), resolved at
    /// collection time. Empty for the common no-throws case. A call site must *discharge* each
    /// member — catch it in an enclosing `try`, or propagate it with `?` and a matching enclosing
    /// `throws` (M-faults 2b). Free functions and class methods carry their declared set; interface
    /// method signatures keep it empty (dynamic-dispatch throws enforcement is a documented deferral).
    throws: Vec<Ty>,
    /// Whether this declaration is `static` (Statics-B). Only meaningful for methods; always `false`
    /// for free functions and natives. Every overload of one method name must agree on this
    /// (`E-OVERLOAD-STATIC-MIX`) so a static-call site resolves the same set on all backends.
    is_static: bool,
}

struct EnumInfo {
    /// variant name -> field types (in declaration order)
    variants: HashMap<String, Vec<Ty>>,
    /// Generic type parameters this enum declares (`["T"]` for `enum Option<T>`). Empty for a
    /// non-generic enum. When non-empty, `variants`' field types may contain `Ty::Param` occurrences:
    /// construction unifies a variant's fields against the call arguments to bind them, and a `match`
    /// substitutes them with the scrutinee's type arguments (M-RT generic enums).
    type_params: Vec<String>,
    /// True for a compiler-injected enum (`Json`/`RoundingMode`); its variants must be constructed and
    /// matched *qualified* (`Json.Object(…)`), bare use is `E-INJECTED-VARIANT-BARE` (B). Mirrors
    /// [`crate::ast::EnumDecl::injected`], carried through `collect_enum`.
    injected: bool,
}

#[derive(Clone)]
struct ClassInfo {
    fields: HashMap<String, Ty>,
    /// DEC-257 generic interfaces: interface name → the type arguments this class implements it
    /// with (`class Ints implements Producer<int>` ⇒ `{"Producer": [int]}`). Arguments may mention
    /// the class's OWN type parameters as `Ty::Param` (`Boxed<T> implements Producer<T>`) — the
    /// use site substitutes the instance's arguments through them. Absent/empty for non-generic
    /// interfaces. Feeds interface-typed assignability and the foreach element lookup.
    iface_args: HashMap<String, Vec<Ty>>,
    /// DEC-241 asymmetric visibility: instance-field name → its SET visibility + declaring owner
    /// (`private(set)` ⇒ `Private`, `protected(set)` ⇒ `Protected`). Absent = assignable wherever
    /// the field is visible. Only `mutable` fields may carry one (an immutable field has no set
    /// path to gate — `E-SET-VIS-IMMUTABLE`); enforced at every assignment/`with` site.
    set_vis: HashMap<String, (MemberVis, String)>,
    /// DEC-241: as [`Self::set_vis`], for `static` fields (their own namespace).
    static_set_vis: HashMap<String, (MemberVis, String)>,
    /// Names of the `mutable` fields (M-mut.6) — explicit `mutable Type f;` decls and promoted ctor
    /// params carrying `mutable`. Only these may be the target of `o.f = e` (`E-ASSIGN-IMMUTABLE`);
    /// every other field is immutable by default. A subset of `fields`' keys.
    mutable_fields: std::collections::HashSet<String>,
    /// `static` field name → type (M-mut.7). Class-level, accessed as `ClassName.field` — disjoint
    /// from `fields` (statics are never instance members). Each has a literal-const initializer.
    statics: HashMap<String, Ty>,
    /// `const NAME` → its [`ConstEntry`] (Feature A). Class-level, compile-time, immutable, accessed
    /// only `ClassName.NAME` — disjoint from `fields`/`statics`. Inherited consts are merged into a
    /// subclass (own/nearer wins), so `Sub.MAX` resolves an inherited `MAX`. Visibility is enforced at
    /// the access site.
    consts: HashMap<String, ConstEntry>,
    /// The subset of `statics` declared `static mutable` — only these may be the target of
    /// `ClassName.field = e` (`E-ASSIGN-IMMUTABLE`).
    static_mut: std::collections::HashSet<String>,
    /// Method overload sets (M-RT overloading): a name maps to one *or more* signatures (dynamic
    /// multiple dispatch). Length 1 in the common case; >1 when methods share a name with distinct
    /// parameter signatures (all sharing a return type — `E-OVERLOAD-RETURN`).
    methods: HashMap<String, Vec<FnSig>>,
    /// Property hooks (M-mut.7b) — virtual members keyed by name. Disjoint from `fields`/`statics`
    /// (a hook has no storage). The flags record whether the hook is readable (`has_get`) and/or
    /// writable (`has_set`): reading a `!has_get` hook is `E-HOOK-NO-GET`, writing a `!has_set` one
    /// is `E-HOOK-NO-SET`. A member read/write resolves a hook here before the instance-field path.
    hooks: HashMap<String, HookInfo>,
    /// constructor parameter types, for `ClassName(args)` calls. For a class with no own constructor
    /// under single inheritance (M-RT S6c.2a), this is the *inherited* parent constructor's signature.
    ctor: Vec<Ty>,
    /// DEC-236 — the constructor's default literals, parallel to [`Self::ctor`] (`None` = required).
    /// Validated (literal-only, trailing-only, type-assignable) at collection; consumed by the
    /// construction check via the M4 fill (every backend sees a full-arity `new`). Inherited
    /// alongside `ctor`.
    ctor_defaults: Vec<Option<crate::ast::Expr>>,
    /// DEC-221: the constructor's declared checked-exception set (resolved + flattened), read at the
    /// construction site (`new X(args)` routes each through `route_call_throw`, so the caller must
    /// `try`/`catch` or `?`-propagate) and to seed `cur_throws` while checking the ctor body. Inherited
    /// alongside [`Self::ctor`] for a class with no own constructor. Empty for a non-throwing ctor.
    ctor_throws: Vec<Ty>,
    /// Whether the class declares its **own** constructor (vs. inheriting one). Distinguishes a class
    /// with a zero-arg ctor from one with no ctor at all (both leave `ctor` empty) — `merge_inherited`
    /// inherits a single parent's `ctor` only into a class that has none of its own (M-RT S6c.2a).
    has_ctor: bool,
    /// DEC-194 2b: this class carries the `#[Attribute]` marker → it is a user-defined attribute type,
    /// so `#[ClassName(...)]` is a legal attribute use (validated against `ctor`). Set in `collect_class`.
    is_user_attribute: bool,
    /// Constructor member visibility (Soundness Batch A) — `public` (default) unless the `constructor`
    /// keyword carries `private`/`protected`. Enforced at the construction site (`new C(...)`) so a
    /// private/protected ctor blocks external construction (the factory/singleton pattern), the 7th
    /// member-visibility access site. Inherited alongside `ctor` for a class with no own constructor.
    ctor_vis: MemberVis,
    /// The class that *declares* the constructor (Batch A) — itself for an own ctor, the parent for an
    /// inherited one. The owner for the `protected`-scope subtype check and the `E-CTOR-VISIBILITY`
    /// message, mirroring [`ConstEntry`]'s `owner`.
    ctor_owner: String,
    /// Generic type parameters this class declares (`["T"]` for `class Box<T>`). Empty for a
    /// non-generic class. When non-empty, `fields`/`ctor`/`methods` may contain `Ty::Param`
    /// occurrences: construction unifies the ctor against the arguments to bind them, and member
    /// access substitutes them with the instance's type arguments (M-RT generics-all).
    type_params: Vec<String>,
    /// `abstract class` (M-RT S6b) — not instantiable (`E-ABSTRACT-INSTANTIATE`); may carry abstract
    /// (bodyless) methods a concrete subclass must implement.
    is_abstract: bool,
    /// Member visibility for instance fields (incl. promoted ctor params): field name → (vis, owner).
    /// The owner is the *declaring* class, preserved through inheritance so a `private`/`protected`
    /// access is checked against the real owner (mirrors [`ConstEntry`]). Enforced at the
    /// instance-field read/write sites (Wave 1.1) so `run ≡ runvm ≡ transpiled PHP` — which enforces
    /// visibility natively — all reject an out-of-scope access instead of diverging at runtime.
    field_vis: HashMap<String, (MemberVis, String)>,
    /// Static-field visibility + declaring owner, parallel to [`Self::statics`] (like [`Self::field_vis`]
    /// for instance fields). W0-2: a `private`/`protected` static read/write from outside its scope is
    /// rejected here, closing the run≡runvm≡PHP hole (PHP emits a real `private static` property).
    static_vis: HashMap<String, (MemberVis, String)>,
    /// Member visibility for methods: method name → (vis, owner). Per-name (an overload set shares one
    /// visibility — the first-declared overload's modifiers win). Enforced at the method-call site.
    method_vis: HashMap<String, (MemberVis, String)>,
    /// Names of the `static` methods (Batch-1 D / slice B0). A static method is callable via the class
    /// name (`ClassName.method(args)`) with no receiver; a *non*-static method called that way is
    /// `E-STATIC-CALL`. Inherited alongside `methods`. A subset of `methods`' keys.
    static_methods: std::collections::HashSet<String>,
}

/// A property hook's declared type and which accessors it provides (M-mut.7b).
#[derive(Clone)]
struct HookInfo {
    ty: Ty,
    has_get: bool,
    has_set: bool,
}

/// Member-level visibility (Feature A — `const` class constants). Distinct from `ast::Visibility`
/// (declaration/file scope): a *member* is `public` (default), `protected`, or `private`, derived from
/// the `Modifier::{Public,Private,Protected}` set. Const access is the one site Phorj enforces member
/// visibility — required because the transpiler emits a PHP `private const`, which PHP would reject if
/// read from outside the class (a `run`↔PHP byte-identity break otherwise).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MemberVis {
    Public,
    Protected,
    Private,
}

impl MemberVis {
    /// The member visibility carried by a modifier set: `private` > `protected` > `public` (default).
    /// DEC-241: the SET visibility carried by a modifier set — `Some(Private)` for
    /// `private(set)`, `Some(Protected)` for `protected(set)`, `None` when symmetric.
    pub(super) fn set_of(mods: &[crate::ast::Modifier]) -> Option<MemberVis> {
        use crate::ast::Modifier as M;
        if mods.contains(&M::PrivateSet) {
            Some(MemberVis::Private)
        } else if mods.contains(&M::ProtectedSet) {
            Some(MemberVis::Protected)
        } else {
            None
        }
    }

    pub(super) fn of(mods: &[crate::ast::Modifier]) -> MemberVis {
        use crate::ast::Modifier;
        if mods.contains(&Modifier::Private) {
            MemberVis::Private
        } else if mods.contains(&Modifier::Protected) {
            MemberVis::Protected
        } else {
            MemberVis::Public
        }
    }
}

/// A class constant (Feature A): its declared type, member visibility, and the class that *declares*
/// it (preserved through inheritance so a `private`/`protected` access is checked against the real
/// owner, not the accessing subclass).
#[derive(Clone)]
struct ConstEntry {
    ty: Ty,
    vis: MemberVis,
    owner: String,
}

/// An interface's own method signatures plus its declared parent interfaces (`extends`). The
/// flattened method set (own + every parent's) is computed on demand, cycle-guarded (M-RT S2).
struct InterfaceInfo {
    methods: HashMap<String, FnSig>,
    extends: Vec<String>,
    /// Generic type parameters (`interface Iterator<T>`, DEC-257) — declaration order. Method
    /// signatures mention them as `Ty::Param`; a class's `implements Iterator<int>` substitutes
    /// them positionally for conformance. Empty for the common non-generic interface.
    type_params: Vec<String>,
}

impl ClassInfo {
    /// An empty-membered placeholder carrying only the declared type parameters — registered by the
    /// name-binding pre-pass (`prebind_types`) so a forward/cross-file reference to this class resolves
    /// (correct generic arity) before its members are collected. `collect_class` overwrites it with the
    /// fully-populated entry.
    fn placeholder(type_params: Vec<String>) -> Self {
        ClassInfo {
            fields: HashMap::new(),
            iface_args: HashMap::new(),
            mutable_fields: std::collections::HashSet::new(),
            statics: HashMap::new(),
            consts: HashMap::new(),
            static_mut: std::collections::HashSet::new(),
            methods: HashMap::new(),
            set_vis: HashMap::new(),
            static_set_vis: HashMap::new(),
            hooks: HashMap::new(),
            ctor: Vec::new(),
            ctor_defaults: Vec::new(),
            ctor_throws: Vec::new(),
            has_ctor: false,
            is_user_attribute: false, // placeholder; overwritten by `collect_class`
            ctor_vis: MemberVis::Public,
            ctor_owner: String::new(),
            type_params,
            is_abstract: false,
            field_vis: HashMap::new(),
            static_vis: HashMap::new(),
            method_vis: HashMap::new(),
            static_methods: std::collections::HashSet::new(),
        }
    }
}

impl EnumInfo {
    /// See [`ClassInfo::placeholder`] — name-binding placeholder for an enum (`enum Option<T>`).
    fn placeholder(type_params: Vec<String>) -> Self {
        EnumInfo {
            variants: HashMap::new(),
            injected: false,
            type_params,
        }
    }
}

impl InterfaceInfo {
    /// See [`ClassInfo::placeholder`] — name-binding placeholder for an interface.
    fn placeholder(type_params: Vec<String>) -> Self {
        InterfaceInfo {
            methods: HashMap::new(),
            extends: Vec::new(),
            type_params,
        }
    }
}

pub struct Checker {
    /// Free-function overload sets (M-RT overloading): a name maps to one *or more* signatures
    /// (dynamic multiple dispatch). Length 1 in the common case.
    funcs: HashMap<String, Vec<FnSig>>,
    enums: HashMap<String, EnumInfo>,
    classes: HashMap<String, ClassInfo>,
    interfaces: HashMap<String, InterfaceInfo>,
    /// `sealed` class/interface names (W5-3). A `match` over a scrutinee of a sealed type is
    /// exhaustiveness-checked over its whole-program permitted subtypes (every concrete class that is
    /// a subtype), so no `_` is needed. Compile-time-only — never reaches a backend.
    sealed_types: std::collections::BTreeSet<String>,
    /// Trait names (M-RT S8). A trait's members are collected into [`Self::classes`] under its name
    /// (so member lookup while checking the trait body and merging into a using class reuse the class
    /// machinery), but a trait is **not a type**: this set lets `resolve_type`/`instanceof`/construction
    /// reject a trait name where a type is expected.
    traits: std::collections::HashSet<String>,
    /// Type names registered by the name-binding **pre-pass** (`prebind_types`) before any member type
    /// is resolved, so a type reference is **order-independent** — a forward reference (a later type in
    /// the same file) and a cross-file reference (a sibling file merged earlier, after the loader's
    /// alphabetical sort) both resolve. Without this, `collect_class` only pre-bound its *own* name, so
    /// `class Order { … List<OrderLine> … }` failed when `OrderLine` was declared/merged later. The set
    /// also carries duplicate detection (a name seen twice in the pre-pass is the "already defined"
    /// error), which lets the per-item collectors treat an existing-and-prebound name as "fill my
    /// placeholder" rather than a duplicate.
    prebound: std::collections::HashSet<String>,
    /// Transitively-flattened interface set each class implements (the `instanceof`/subtyping table),
    /// computed once via [`crate::ast::class_implements`] and shared verbatim with the backends so
    /// the runtime test can never diverge from the static one (M-RT S2).
    class_implements: std::collections::BTreeMap<String, Vec<String>>,
    /// Transitively-flattened ancestor-class set for every class (M-RT S6), computed once via
    /// [`crate::ast::class_supertypes`]. Drives nominal subtyping (`Dog <: Animal`) alongside
    /// `class_implements`; a class appearing in its own set marks an `extends` cycle (`E-MI-CYCLE`).
    class_supertypes: std::collections::BTreeMap<String, Vec<String>>,
    /// lexical block scopes; last is innermost. Each binding carries its type and whether it is
    /// `mutable` (reassignable) — immutable by default (M-mut.1); only a `mutable` binding may be
    /// the target of `Stmt::Assign`.
    scopes: Vec<HashMap<String, (Ty, bool)>>,
    errors: Vec<Diagnostic>,
    /// Non-fatal lints (e.g. `W-FORCE-UNWRAP`). Surfaced to stderr by the CLI but never fail the
    /// build — the first member of Phorj's warning channel (M3 S2.5).
    warnings: Vec<Diagnostic>,
    /// return type of the function/method currently being checked
    cur_ret: Ty,
    /// Checked-exception set the function/method currently being checked is declared to `throws`
    /// (M-faults 2b). A `throw e` or a `?`-propagated throwing call discharges against this set;
    /// saved/restored exactly like [`Self::cur_ret`] at every body-checking site. Empty inside a
    /// constructor, property hook, or lambda (none may declare `throws`).
    cur_throws: Vec<Ty>,
    /// Whether the function currently being checked is the program entry `main` (M-faults 2b).
    /// `main` may not declare `throws`, and an undischarged throw reaching it is `E-UNCAUGHT-THROW`
    /// (rather than the generic `E-THROW-UNDECLARED`).
    cur_is_main: bool,
    /// Stack of the active enclosing `try` blocks' catch-type sets — innermost last — while checking
    /// a `try` *body* (popped before its own catch/finally are checked, since a throw there is not
    /// caught by the same `try`). A thrown type is *caught* iff it is `<:` some type in any frame.
    /// Cleared around a lambda body (a closure passed to a native does not see the lexical `try`).
    try_catch_stack: Vec<Vec<Ty>>,
    /// One-shot flag set by a throws-mode `?` so the immediately-wrapped throwing call skips its own
    /// call-site discharge check (the `?` propagates instead). Consumed (taken) by the call.
    skip_throws_discharge: bool,
    /// Throws collected from the `?`-operand's OUTERMOST call while `skip_throws_discharge` was
    /// honored (free fns AND methods — the method half closed the documented `free_call_throws`
    /// deferral). Read + validated by [`Checker::try_throws_propagate`]; empty ⇒ the call throws
    /// nothing (Result-mode / position-error fallback, no re-check).
    propagate_sink: Vec<Ty>,
    /// One-shot flag (Feature C) set by `check_new` so the immediately-wrapped construction call
    /// recognizes it was `new`-prefixed and skips `E-NEW-REQUIRED`. Taken (cleared) at the top of
    /// `check_named_call` — before its arguments are checked — so a bare construction *argument* still
    /// requires its own `new`.
    under_new: bool,
    /// Set while type-checking a **field/static initializer** (Phase 1 closures slice). A lambda
    /// default there may not capture `this` (`E-LAMBDA-THIS`): the instance is only partially built
    /// when an initializer runs, so capturing the receiver is the F8 footgun. Outside this context a
    /// method-body lambda *may* capture `this`.
    in_field_init: bool,
    /// Set while type-checking a **static** field initializer (Batch A). The init runs in the class's
    /// own scope (so it may call a `private`/`protected` constructor — the singleton pattern, and
    /// `cur_class` is set to the owner for that visibility check), but there is no instance, so `this`
    /// is forbidden even though `cur_class` is `Some`. Distinct from `in_field_init` (an *instance*
    /// field initializer, where `this` IS in scope).
    in_static_init: bool,
    /// Set when checking a program under `phg test` (M-Test). When true, `test "name" { … }` items
    /// are allowed and their bodies type-checked; when false (every normal build — run/runvm/check/
    /// transpile), a `test` item is rejected as `E-TEST-OUTSIDE-TESTS` so production code cannot
    /// smuggle test blocks. Default `false`; flipped only by [`check_tests`].
    test_mode: bool,
    /// Set while checking a **static method** body (Batch E, finding #5). A static method has no
    /// instance, so `this` and bare instance-field references are rejected (`E-STATIC-THIS`) even
    /// though `cur_class` stays set — static-member access (`Class.field`) and constructing the class
    /// (a static factory, whose ctor visibility is checked against `cur_class`) remain valid.
    in_static_method: bool,
    /// Set while checking a **constructor** body (B1b). `parent.constructor(…)` forwarding is valid
    /// only inside a constructor body (`E-PARENT-CTOR-OUTSIDE` otherwise).
    in_constructor: bool,
    /// Set true by [`check_stmt`] just before checking a bare `Stmt::Expr`/`Stmt::Discard` whose
    /// expression is exactly a `parent.constructor(…)` call, then consumed (taken) by
    /// [`check_parent_ctor_call`] (B1b). Guarantees `parent.constructor(…)` is statement-only
    /// (`E-PARENT-CTOR-STMT` otherwise) so the front-end inline pass catches every occurrence and the
    /// backends never see a `ParentCall{method:"constructor"}`.
    parent_ctor_ok: bool,
    /// class currently being checked (for `this` and bare field refs)
    cur_class: Option<String>,
    /// live `check_expr` recursion depth, bounded by [`MAX_EXPR_DEPTH`]
    depth: usize,
    /// number of enclosing loops being checked (M-mut.3). `break`/`continue` are valid only when
    /// this is `> 0` (`E-BREAK-OUTSIDE-LOOP`/`E-CONTINUE-OUTSIDE-LOOP`).
    loop_depth: u32,
    /// `type Name = Type;` aliases, stored as raw AST types and expanded in `resolve_type`.
    aliases: HashMap<String, crate::ast::Type>,
    /// alias names currently being expanded — detects `type A = B; type B = A;` cycles.
    alias_stack: Vec<String>,
    /// Alias names already reported as part of a cycle (W0-4). Dedupes the collect-time graph walk
    /// against the resolve-time use-site detection so one cycle yields exactly one `E-ALIAS-CYCLE`.
    alias_cycle_reported: std::collections::HashSet<String>,
    /// Active import map (leaf qualifier → full dotted module path; see [`crate::native::import_map`]).
    /// Drives namespaced native-call resolution (`console.println`) and the shadowing guard that
    /// keeps an imported qualifier disjoint from every value binding (M3 Wave 1).
    imports: HashMap<String, String>,
    /// DEC-197: bare call-site name → (native module, real leaf) for member-imported module
    /// FUNCTIONS (`import Core.Output.printLine [as p];` ⇒ `p`/`printLine` → (`Core.Output`,
    /// `printLine`)). Built in `collect` from [`function_imports::function_import_bindings`];
    /// consumed by `check_named_call` to resolve a bare call to its native (after user functions —
    /// `local > user fn > imported native`) and record the qualified rewrite `rewrite_ufcs` applies.
    /// Empty unless the program member-imports a stdlib function.
    fn_imports: HashMap<String, (String, String)>,
    /// Span-keyed node substitutions applied by [`resolve_html`] after a successful check, so the
    /// backend-facing AST is free of front-end-only nodes. Keyed by `Span.start` (byte offset —
    /// unique per source occurrence in a single file). Two producers share it:
    /// (1) type-directed `html"…"` desugarings — each entry is the `html.concat([…])` replacement
    ///     for an [`crate::ast::Expr::Html`] literal (core.html Wave 3);
    /// (2) throws-mode `?` erasure — a throwing call's `?` is a checker-only propagation marker, so
    ///     its [`crate::ast::Expr::Propagate`] node is replaced by its inner call expression (the
    ///     call's own throw already unwinds; M-faults 2b). Result-mode `?` is *not* recorded here —
    ///     it carries real lowering and is left for the backends.
    html_resolutions: HashMap<usize, crate::ast::Expr>,
    /// Span-keyed `Call`-node substitutions applied by [`rewrite_ufcs`] after a successful check
    /// (Slice 6, UFCS). When a member call `x.f(args)` does not resolve to a method but `f` resolves
    /// as a free function or imported native, the checker records the desugared free/native call here,
    /// keyed by the *enclosing `Call`* node's `Span.start` (each call site's `(` token is at a unique
    /// byte offset, so chained UFCS — `xs.filter(p).map(g)` — never collides). The backends never see
    /// the original `Member`-call: it is rewritten to an ordinary call they already handle, so UFCS
    /// adds no new `Op`/`Value` and is byte-identical by construction (the "erase front-end sugar
    /// before any backend" discipline shared with `type` aliases / generics / `html"…"`).
    ufcs_resolutions: HashMap<usize, crate::ast::Expr>,
    /// Span-keyed `Call`-node substitutions for `Reflect.typeName(x)` (Core.Reflect, the precise
    /// static-type pass). Built by [`reflect::check_reflect_type_name`] from `x`'s static type — a
    /// value type → a string-literal `Expr`, an object → a `Reflect.className(x)` call, an optional →
    /// a single-eval `match` null-branch, an erased generic → `Reflect.kind(x)`. Merged into the
    /// combined call-rewrite map alongside [`ufcs_resolutions`] (keys are disjoint — a `typeName`
    /// call site is a native member call, never a UFCS site) and applied by [`rewrite_ufcs`], so the
    /// backends see only ordinary calls/literals — the same "erase front-end sugar before any backend"
    /// discipline. No new `Op`/`Value`; byte-identical by construction.
    reflect_resolutions: HashMap<usize, crate::ast::Expr>,
    /// Span-keyed **default-argument fills** (M4 default parameters): a call that omits trailing
    /// defaulted parameters maps its `Call` node's `Span.start` to the **full replacement `Call`**
    /// (provided args + appended default literals). Merged into the call-rewrite map and applied by
    /// [`rewrite_ufcs`] like a UFCS/reflect substitution — so the interpreter/VM/transpiler only ever
    /// see full-arity calls (the "expand before backends" discipline; byte-identical by construction
    /// since the default literal is the same everywhere). No new pass, no new walker.
    default_fills: HashMap<usize, crate::ast::Expr>,
    /// Span-keyed substitutions for a primitive `as`-cast that is a value CONVERSION (M4 as-matrix),
    /// keyed by the `Cast` node's `Span.start` (the `as` token offset — distinct from any wrapped
    /// call's span). The replacement is a leaf-qualified native call (`Convert.toFloat(v)` etc.) the
    /// backends resolve via `index_of_by_leaf` without an import, exactly like a UFCS rewrite. Merged
    /// into the combined call-rewrite map applied by [`rewrite_ufcs`]. Identity casts (`T as T`) are
    /// NOT recorded (they stay `Expr::Cast`, handled trivially by each backend); only conversions are
    /// rewritten. No new `Op`/`Value` — byte-identical by construction.
    cast_resolutions: HashMap<usize, crate::ast::Expr>,
    /// Set by [`check_named_call`]/[`check_native_call`] when a call legally omits trailing defaulted
    /// arguments: the list of default expressions to append. [`check_call`] consumes it (it holds the
    /// original callee + args + span) to build the replacement `Call` recorded in `default_fills`.
    pending_fill: Option<Vec<crate::ast::Expr>>,
    /// Type parameters in scope while resolving the signature/body of the generic function currently
    /// being checked (`["T", "U"]`). A bare type name in this set resolves to `Ty::Param` rather than
    /// being looked up as an alias/enum/class. Set around each generic function and cleared after;
    /// empty for everything else (M-RT S7).
    active_type_params: Vec<String>,
    /// Per-type-param bounds in scope while checking the current function/method body (DEC-211):
    /// `(param, Interface)` pairs, the union of the class's and the function's own bounds. A bounded
    /// `Ty::Param("T")`'s member access resolves against the bound interface (`bound_of`). Recomputed
    /// per function (empty for a non-generic/unbounded body); mirrors `active_type_params`.
    active_type_param_bounds: Vec<(String, String)>,
    /// Type parameters of the generic *class* whose body is currently being checked (`["T"]` for a
    /// method/constructor inside `class Box<T>`). Unioned with the method's own `type_params` so a
    /// method body sees both. Empty for free functions and non-generic classes (M-RT generics-all).
    cur_class_type_params: Vec<String>,
    /// Bounds of the generic *class* whose body is currently being checked (DEC-211), unioned into
    /// `active_type_param_bounds` for each method. Empty for free functions and non-generic classes.
    cur_class_type_param_bounds: Vec<(String, String)>,
    /// Return-type-overload classification (M-RT Slice C1), built by [`Self::finalize_overloads`]
    /// between collection and body-checking. Maps a free-function name whose overload set is a *pure
    /// return-overload set* (all members share identical parameter signatures, ≥2 distinct return
    /// types) to the list of `(return Ty, mangled name)` members. Consumed by call-checking: a call to
    /// such a name needs a `<Type>` selector (C1) or it is `E-OVERLOAD-NO-CONTEXT`. Empty for the common
    /// case (no return-overloaded function in the program).
    return_overload_sets: HashMap<String, Vec<(Ty, String)>>,
    /// Free-function declaration sites accumulated during collection — `(name, decl span, resolved
    /// params, resolved ret)` — so [`Self::finalize_overloads`] can emit a span-keyed rename for each
    /// member of a return-overload set without threading a span through `FnSig`.
    free_fn_decls: Vec<(String, Span, Vec<Ty>, Ty)>,
    /// Span-keyed **definition renames** for return-overload members (M-RT Slice C1): a
    /// `FunctionDecl`'s `span.start` → its mangled name (`f__ret_int`). Applied by
    /// [`crate::checker::rename_overload_defs`] before any backend, so the backends see distinct,
    /// single-overload functions (no ambiguous identical-`ParamKind` dispatch table). Single-return
    /// names are never renamed ⇒ existing programs byte-identical.
    overload_def_renames: HashMap<usize, String>,
    /// Span-keyed `Call`-node substitutions for resolved overload selectors (M-RT Slice C1): an
    /// [`crate::ast::Expr::OverloadSelect`]'s `span.start` → the mangled `Expr::Call` the checker chose.
    /// Merged into the combined call-rewrite map and applied by [`rewrite_ufcs`] (whose `rexpr` gains an
    /// `OverloadSelect` arm), so no backend sees the selector wrapper. No new `Op`/`Value`.
    overload_resolutions: HashMap<usize, crate::ast::Expr>,
    /// Return-type-overload classification for **methods** (M-RT S2.2), built by
    /// [`Self::finalize_method_overloads`] — the method analog of [`Self::return_overload_sets`],
    /// keyed by `(class, method)`. A `(class, method)` is a *pure return-overload method set* when it
    /// has ≥2 overloads sharing one parameter signature with pairwise-distinct returns. A bare call to
    /// such a method needs a `<Type>` selector (C1, like free functions without a sink) or it is
    /// `E-OVERLOAD-NO-CONTEXT`; the selector path mangles per return before any backend. Empty for the
    /// common case (no return-overloaded method).
    return_overload_methods: HashMap<(String, String), Vec<(Ty, String)>>,
    /// S2.1-broad: per-expression *reified operand type*, keyed by the expression's `span.start`, for
    /// `Call`/`Member`/`Index` nodes whose checker-resolved `Ty` is concrete. The VM compiler's `ctype`
    /// consults this FIRST so a generic method result (`box.get() + 1`), a generic field read
    /// (`box.value + 1`), or a `List<T>`/`Map`-typed return specializes as the arithmetic operand the
    /// checker proved — closing the run↔runvm "CTy-operand trap" for results the static shape erases to
    /// `mixed`. The checker is authoritative on the value's runtime type (erasure doesn't change it), so
    /// overriding `ctype` with it is sound; entries that map to `CTy::Other` are dropped at the compile
    /// boundary, so non-operand results never override `ctype`'s normal (fn-value/class) resolution.
    reified_operands: HashMap<usize, Ty>,
    /// DEC-239: contextual pipe-lambda parameter resolutions — the checker-inferred `Ty` of each
    /// lambda parameter written as `Type::Infer` (the pipe lambda `x |> (v => …)` and the multi-`%`
    /// IIFE), keyed by the parameter's `span.start`. `cli::check_and_expand_reified` materializes
    /// each into the AST param (`checker::materialize_pipe_params`, LAST in the rewrite chain) so
    /// the VM compiler's `resolve_cty` and the transpiler's kind analysis see a concrete type —
    /// leaving `Infer` in a backend-bound param is exactly the run≠runvm CTy-operand trap.
    pipe_param_resolutions: HashMap<usize, Ty>,
    /// Method declaration sites accumulated during collection — `(class, method, decl span, resolved
    /// params, resolved ret)` — so [`Self::finalize_method_overloads`] can emit a span-keyed rename for
    /// each member of a return-overload method set (reusing [`Self::overload_def_renames`], the same map
    /// the free-fn members use; method and free-fn decl spans are disjoint).
    method_fn_decls: Vec<(String, String, Span, Vec<Ty>, Ty)>,
    /// Inheritance tables for `parent`/super resolution (M-RT super/parent), computed once in
    /// [`Self::collect`]: direct parents, transitive ancestors (MRO), and the method-dispatch origins.
    /// Threaded to `ast::resolve_parent_method`, the single resolver shared with both backends.
    parent_parents: std::collections::BTreeMap<String, Vec<String>>,
    parent_mro: std::collections::BTreeMap<String, Vec<String>>,
    parent_origins: std::collections::BTreeMap<(String, String), (String, String)>,
}

mod plumbing;

/// Type-check a whole program. `Ok(())` means it is well-typed; otherwise every
/// detected error is returned.
/// Type-check `program`. On success returns the collected non-fatal warnings (the warning channel,
/// M3 S2.5) — possibly empty; on failure returns the errors. Warnings never gate the build: the CLI
/// renders them to stderr and proceeds.
/// Run the checker over a program and return the populated `Checker` (errors, warnings, and the
/// `html"…"` desugarings collected along the way). The single shared entry behind both [`check`]
/// (gate only) and [`check_resolutions`] (gate + html replacements for the backend pipeline).
fn run_checker(program: &Program) -> Checker {
    run_checker_mode(program, false)
}

/// The shared checker driver. `test_mode` is `true` only under `phg test` — it allows `test` items
/// (M-Test); every other entry runs with it `false`.
fn run_checker_mode(program: &Program, test_mode: bool) -> Checker {
    let mut c = Checker::new();
    c.test_mode = test_mode;
    c.collect(program);
    // M-RT Slice C1: classify return-overload sets (after collection sees every signature, before any
    // body is checked, so call-checking can resolve a `<Type>` selector against the set).
    c.finalize_overloads();
    // M-RT S2.2: classify return-overload *method* sets too (same timing/discipline as free fns).
    c.finalize_method_overloads();
    c.check_program(program);
    c
}

pub fn check(program: &Program) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let c = run_checker(program);
    if c.errors.is_empty() {
        Ok(c.warnings)
    } else {
        Err(c.errors)
    }
}

/// Like [`check`], but in **test mode**: `test "name" { … }` items are accepted and their bodies
/// type-checked (a normal build rejects them as `E-TEST-OUTSIDE-TESTS`). Used by the `phg test`
/// runner (M-Test T3) so a test file is checked as a real program plus its test blocks.
pub fn check_tests(program: &Program) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let c = run_checker_mode(program, true);
    if c.errors.is_empty() {
        Ok(c.warnings)
    } else {
        Err(c.errors)
    }
}

/// Like [`check`], but on success also returns the `html"…"` desugarings keyed by literal
/// `Span.start` — fed to [`resolve_html`] so the backend-facing program is `Expr::Html`-free. Used
/// by the run/runvm/transpile pipeline ([`crate::cli::check_and_expand`]); plain [`check`] (e.g.
/// `phg check`) ignores the map since it never reaches a backend.
#[allow(clippy::type_complexity)]
pub fn check_resolutions(
    program: &Program,
) -> Result<
    (
        Vec<Diagnostic>,
        HashMap<usize, crate::ast::Expr>,
        HashMap<usize, crate::ast::Expr>,
        HashMap<usize, String>,
        HashMap<usize, Ty>,
        HashMap<usize, Ty>,
        HashMap<usize, crate::ast::Expr>,
    ),
    Vec<Diagnostic>,
> {
    let c = run_checker(program);
    if c.errors.is_empty() {
        // Merge the Reflect `typeName` substitutions into the call-rewrite map applied by
        // `rewrite_ufcs`. Keys are disjoint (a `typeName` call site is a native member call, never a
        // UFCS site), and a single combined pass — rather than two ordered passes — is what makes the
        // two kinds of sugar compose correctly when nested (UFCS inside a `typeName` argument, or
        // `typeName` inside a UFCS argument): one walker that knows every replacement re-resolves
        // embedded original subtrees regardless of nesting direction.
        let mut calls = c.ufcs_resolutions;
        calls.extend(c.reflect_resolutions);
        // M4/DEC-249 default-parameter fills are returned SEPARATELY (not merged into the
        // rewrite_ufcs map): a fill is a CHECK-TIME clone, so it must be spliced back FIRST —
        // before resolve_html erases throws-`?` / unwrap_new strips `new` — or a lambda argument's
        // already-erased nodes get restored stale (the db.transaction(fn) regression). The
        // pipeline applies them via `apply_default_fills` ahead of every other rewrite, so the
        // spliced subtrees flow through the whole chain like hand-written code.
        // M4 as-matrix: primitive-cast → native-conversion-call substitutions, keyed by the `Cast`
        // node's span (the `as` token — disjoint from every call/UFCS/fill/reflect span). Applied by
        // the same `rewrite_ufcs` walker (its `Cast` arm now consults this map).
        calls.extend(c.cast_resolutions);
        // M-RT Slice C1: resolved overload-selector call-site rewrites join the same call-rewrite map
        // (keys are the `OverloadSelect` node spans — disjoint from every call/UFCS/fill/reflect/cast
        // span). The definition renames are returned separately (they rename items, not call sites).
        calls.extend(c.overload_resolutions);
        Ok((
            c.warnings,
            c.html_resolutions,
            calls,
            c.overload_def_renames,
            c.reified_operands,
            // DEC-239: contextual pipe-lambda param resolutions, materialized into the AST by
            // `materialize_pipe_params` (LAST in the pipeline's rewrite chain).
            c.pipe_param_resolutions,
            c.default_fills,
        ))
    } else {
        Err(c.errors)
    }
}

#[cfg(test)]
mod tests;

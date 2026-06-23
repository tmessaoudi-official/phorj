# Generic Types / Classes `Box<T>` — Design (M-RT generics-all, sub-slice 3)

> Status: **IMPLEMENTATION-GRADE.** The last generics-all sub-slice. Follows generic methods
> (`bd8782c`) and the E-PKG-TYPE lift / cross-package types (`82dd9df`). Pace: fully autonomous.

## 1. Goal

A class may declare type parameters — `class Box<T> { … }`, `class Pair<A, B> { … }` — used in its
fields, constructor, and method signatures. The parameters are **inferred at construction** from the
constructor arguments (`Box(7)` ⇒ `Box<int>`), and member access through a typed instance recovers
the concrete type (`Box(7).get()` is `int`). As with generic functions/methods there is **no
monomorphization**: every type parameter is *erased* before any backend, so the interpreter, VM, and
transpiled PHP run identical, type-variable-free code — byte-for-byte identical output. An instance
carries **no** type argument at runtime: `instanceof Box<int>` is just `instanceof Box`.

This is the TypeScript model: the **checker reifies** `Box<int>` (full use-site precision), the
**emit erases** it (PHP has no generics; a `T` field becomes `mixed`).

## 2. The pivotal constraint and the decision

`Ty::Named(String)` carries **no** type arguments — so a generic instance type cannot be represented
without change. Two models were weighed:

- **D1 — reified in the checker (chosen).** Give `Ty::Named` type arguments
  (`Ty::Named(String, Vec<Ty>)`). `Box(7)` infers `T=int` and yields `Ty::Named("Box", [Int])`;
  member access substitutes `{T→Int}` into the method/field type. Full TS-grade use-site precision
  (`string s = Box(7).get()` is a **type error**). The `Ty::Named` change is confined to **2 files,
  14 sites** (`types.rs`, `checker.rs`); `Ty` is checker-only (the backends use `CTy`/`Value`).
- **D2 — erased at the use boundary (rejected).** Anything flowing out of a generic instance becomes
  `mixed`/poison. Trivial to build but gives *no* use-site safety — a degenerate "generics" that
  barely earns the name. Rejected: the plan locks "TS-style erasure" and "I want generics all
  options".

**D1 is chosen.** The backends need **zero** changes: `compiler::resolve_cty` already maps a class
`Named{..}` → `CTy::Class(name)` (args dropped) and `transpile::emit_type` already maps it →
`php_type_ref(name)` (args dropped). After `erase_generics`, a class's own `<T>`-typed members become
`Type::Erased` → `CTy::Other` / PHP `mixed`; a use-site `Box<int>` annotation keeps its args but the
backends ignore them. The byte-identity spine is therefore safe **by construction** — the entire
slice is front-end (parser + checker + the erasure pass).

## 3. Representation

`Ty::Named(String, Vec<Ty>)` — args empty for a non-generic nominal (enum/interface/non-generic
class), so all existing behavior is preserved. `assignable_with` becomes invariant on args
(`a == b && aa == ba || subtype(a, b)`), matching `List`/`Map`/`Set` invariance. `unify`,
`apply_subst`, and `ty_has_param` gain a `Named` arm that descends args (so a generic function over a
generic class — `function unwrap<T>(Box<T> b) -> T` — works). `Display`: `Box<int>` (or `Box` when
args empty).

`ClassDecl` gains `type_params: Vec<String>`; `ClassInfo` (checker) gains the same, registered before
member resolution so a self-referential `Box<T> next` field resolves.

## 4. Checker flow

- **`collect_class`**: register `type_params`; set `active_type_params = class.type_params` while
  resolving field/ctor/method **signatures** so a bare `T` → `Ty::Param("T")`. A generic method adds
  its own params on top (`class ∪ method`). `validate_type_params(class)` (builtin-shadow / duplicate,
  PascalCase via casing); a **method** type param that shadows a **class** type param → clean error
  (`E-GENERIC-PARAM`) to keep composition unambiguous.
- **`check_function` (method bodies)**: a new `cur_class_type_params` field is unioned with the
  method's own params so the body sees both. Empty for free functions.
- **`resolve_type`** (the class/enum/iface branch): resolve args; for a **generic class** require
  `args.len() == type_params.len()` (a bare `Box` annotation on a generic class is a clean
  "expects N type arguments" error, like `List`); for a non-generic nominal, args must be empty.
- **Construction** (`try_variant_or_class_call`): for a generic class, unify each stored ctor param
  against the argument type (first-binding-wins, reusing `unify`) → θ; instance type is
  `Ty::Named(name, [θ(p) for p in type_params])`. An un-inferable parameter (not mentioned by the
  constructor, or no constructor) defaults to `Ty::Error` (permissive — documented).
- **`this`**: `Ty::Named(cls, [Ty::Param(p) for p in type_params])`, so inside the body
  `this.value : T`.
- **`check_member` / `check_method_call`**: when the receiver is `Ty::Named(cls, cargs)` and `cls` is
  a generic class, build θ = `{type_params[i] → cargs[i]}`, `apply_subst` it into the field type /
  method (params, ret) before the usual checks. A remaining method-level `<U>` then infers through the
  existing generic-call unifier (composition).
- **instanceof narrowing**: the RHS is a bare name; narrowing a generic class declares
  `Ty::Named(name, [Error; arity])` (the type args are erased by `instanceof` — documented).

## 5. Erasure

`erase_generics`'s `Item::Class` arm fires when the class has **type params *or* any generic method**
(was: generic method only). For a generic class it erases **every** member: field types, constructor
param types, and each method's signature + body, with the param set = `class ∪ method`; the class's
`type_params` (and each method's) are cleared. No `T` survives to a backend. A non-generic class is
returned byte-for-byte.

## 6. Scope / deferrals (→ KNOWN_ISSUES)

- **`package Main` only** this slice (mirrors S2 interfaces shipping main-first). Cross-package
  generic *library* types are not validated/supported yet (the loader leaves a class type param `T`
  unchanged and erasure removes it, so it may "work", but it is untested — deferred).
- No **explicit type arguments at construction** (`Box<int>(7)`) — inference only.
- No **bounds** (`<T: Speaker>`), no **variance** (invariant), no **generic enums/interfaces** (a
  generic interface method signature is still built with empty `type_params`).

## 7. No new `Op`, no `Value` change, no backend change. New diagnostic reuse: `E-GENERIC-PARAM`.

## 8. Example + gate

`examples/guide/generic-types.phg` — `Box<T>` (single param) + `Pair<A, B>` (two params): construction
inference, a method returning `T`/`A`/`B`, a method taking a `T`, field-via-method. Byte-identical
run ≡ runvm ≡ real PHP. Checker unit tests cover: inference, member substitution, wrong-arg rejection,
arity error, method-shadows-class-param error, erasure-strips-class-params.

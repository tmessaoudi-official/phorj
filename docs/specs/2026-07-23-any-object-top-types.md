# SPEC — `Any` + `Object`: the two-tier top types (DEC-335)

> Status: **SPEC RULED (dev, 2026-07-23) — BUILD-READY, QUEUED** (joins the design-slice
> queue; scheduling vs DEC-333 is the one open point, dev slots at pickup). Dev-initiated
> ("a global parent Object that everything derives from, like Java — so generics can span
> primitives or classes"), adjudicated over three AskUserQuestion rounds. Key reframe locked
> during adjudication: generics ALREADY span primitives and classes (erasure + uniform Value
> — `Box(7)` ⇒ `Box<int>`, no boxing cliff); what the top types add is HETEROGENEITY and
> typeable "accepts anything" sinks, not generic capability.

## 1. Surface

```phg
// Any — the top of ALL values
Any a = 42;                                // primitive flows in
List<Any> bag = [1, "two", new User(3)];   // heterogeneous storage
function dump(Any x): void {
    if (x instanceof User u) { Output.printLine(u.name); }   // smart-cast narrowing
    else if (x instanceof int i) { Output.printLine("{i}"); }
}

// Object — the top of REFERENCE values (implicit root class)
function register(Object handler): void { /* ... */ }
register(new UserHandler());   // class instance    ✓
register(Color.Red);           // enum value        ✓ (Java: enums ARE Objects; PHP: objects)
register((e) => log(e));       // function value    ✓ (Java: lambdas ARE Objects; PHP: Closure)
register([1, 2]);              // E-TYPE — collections are Any-only (PHP arrays aren't objects)
register(42);                  // E-TYPE — primitives are Any-only

Object token = new Object();          // legal: identity/sentinel token idiom
class User extends Object {}          // legal: explicit no-op (the edge is implicit anyway)
if (x instanceof Object o) { ... }    // true for ANY reference value
```

## 2. Semantics (locked)

- **Subtyping lattice**: `C <: Object <: Any` for every class `C`; enum types and function
  types `<: Object`; primitives (`int`/`float`/`bool`/`string`) and collections
  (`List`/`Map`/`Set`) `<: Any` only. `null` is in neither — `Any?`/`Object?` as usual
  (DERIVED from phorj's orthogonal nullability, not a separate ruling — flag at build if it
  surprises).
- **Both are MEMBER-LESS** (per the D9b/P2 ruling — `#[ToString]` stays the designator):
  no default `toString`, no universal `equals`/`hashCode`. `E-NO-TOSTRING` stays strict —
  an `Any`/`Object` value in string context is an error until narrowed to a stringifiable
  type. Using a value NEEDS narrowing (`instanceof` smart-cast / `match` type patterns);
  operators on un-narrowed `Any`/`Object` are the standard type errors.
- **`Object` is the implicit ROOT CLASS, erased** (P1b): every class implicitly extends it;
  `new Object()` is legal (featureless instance — sentinel/identity-token idiom); explicit
  `extends Object` is legal as a no-op. No members, so nothing is inherited.
- **`Object` membership matches PHP's `object` exactly** (P1c — also Java's own semantics):
  class instances + enum values + function values. This makes the PHP leg byte-identical
  with ZERO shims by construction.
- **`instanceof`**: `x instanceof Object` is special-cased (like interfaces) — true iff the
  runtime value is a reference value. `x instanceof Any` is an always-true dead test —
  proposed handling: `E-INSTANCEOF-ANY` compile error (PENDING, dev rules at build pickup —
  Invariant 15: a new user-visible diagnostic is not self-decided; alternative is constant-
  folding to `true`).
- **Unions**: any member type of a union flows into `Any` as usual; `Any` inside a union is
  redundant (build detail: warn or fold — fixed at build).

## 3. Backends (Invariant 17: run + transpile + lift in the same change)

- **Checker/expansion**: `Any`/`Object` are type-level; they erase before backends
  (Invariant 5) except two reified points: `new Object()` (a construction of the built-in
  featureless class) and `instanceof Object` (a reference-value test — reuses the existing
  `Op::IsInstance` machinery with a built-in target, no new `Op` expected; verify at build,
  Invariant 3 if one is needed after all).
- **Transpile (tier 1 faithful, zero shims)**: `Any` → `mixed`, `Object` → `object` (both
  native PHP type-hints); `new Object()` → `new \stdClass()`; `x instanceof Object` →
  `is_object($x)`. **NO base class is emitted** — a literal `extends \Phorj\Object` would
  make `instanceof Object` FALSE for vendor/lifted PHP objects (silent divergence); the
  erased root + `is_object` is the only correct emission.
- **Lift**: PHP `mixed` → `Any`; `object` hint → `Object`; `new \stdClass()` →
  `new Object()`; `is_object($x)` → `x instanceof Object` — closes four lift gaps.
- **JIT**: `Any`/`Object`-typed slots are outside the unboxed subset (boxed path; the Kind
  lattice is untouched) — opt-in cost, zero impact on the shipped verticals.
- **Byte-identity watchpoint (differential test in the build commit)**: equality of two
  bare `new Object()` instances — phorj `==` on class instances must match the PHP leg's
  `\stdClass` behavior (PHP `==` on prop-less stdClass is structural-true). Whichever side
  is canonical per the value kernel, the differential case ships with the slice.

## 4. Faults / diagnostics

`E-INSTANCEOF-ANY` (compile-time — PENDING per §2, dev rules the error-vs-fold choice at
build). Everything else reuses existing type-error machinery (`E-TYPE` on bad flows into
`Object`, standard narrowing requirements). No new runtime faults.

**Reserved-name reconciliation (blast radius, `KNOWN_ISSUES.md` §E-RESERVED-NAME):** the
existing guard rejects PHP type words — including `object` — as user class/enum/interface/
trait names, and PHP's reserved-word matching is case-insensitive. `Object` and `Any` now
become BUILT-IN type names: user redeclaration (`class Object {}`, `class Any {}`) must be
rejected (extend `E-RESERVED-NAME` or the existing built-in-shadowing error — exact channel
fixed at build) and the KNOWN_ISSUES row updated in the build commit. Erasure already keeps
the PHP leg safe (no `class Object` is ever emitted).

## 5. Examples & tests (Inv 9)

`examples/any_object.phg` (heterogeneous `List<Any>` walk + an `Object`-keyed registry taking
a class instance, an enum value, and a function value + `instanceof` narrowing +
`new Object()` sentinel) + README row; differential across backends incl. the bare-Object
equality case; checker negatives (`Object x = 42`, `Object y = [1,2]`, un-narrowed use,
`instanceof Any`); transpile snapshot (`mixed`/`object` hints, `is_object`, `\stdClass`);
lift round-trip of a PHP `object`-hinted API. (The `instanceof Any` negative test lands with
whichever handling the dev rules per §2/§4.)

## 6. RULED (dev, 2026-07-23)

- **P1a → BOTH tiers**: `Any` (top of all values) + `Object` (top of reference values).
- **P1b → root class, ERASED** (dev's lean, confirmed after the instanceof clarification):
  `new Object()` legal, explicit `extends Object` legal no-op, emission stays erased.
- **P1c → classes + enums + functions** ("the more correct thing" — matches Java's own
  Object semantics AND PHP's `object` hint; zero shims).
- **P2 → keep `#[ToString]`** (both tops member-less; the frozen invoke-tostring spec
  stands untouched).
- **P3 → spec now, then build**: this spec lands before the DEC-331 cluster builds; the
  build slice itself joins the design-slice queue (scheduling vs DEC-333 open, dev slots).

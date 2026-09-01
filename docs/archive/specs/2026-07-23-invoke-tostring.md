# SPEC — `#[Invoke]` + `#[ToString]` (DEC-331 D9, build slice 1 of 3)

> Status: **SPEC RULED (dev, 2026-07-23). SLICE 1 BUILT + byte-identity-green (2026-07-23);
> a coupled cluster DEFERRED to slice 1b — see §8 BUILD STATUS.**
> Rulings elaborated here: D9a (attribute-marked callability), D9b (`#[ToString]`), D9c
> (overloaded invoke). All open points RULED — see §7.

## 1. Surface

Attributes designate conventional methods (phorj's unified model — no magic method names):

```phg
package Main;
import Core.Output;
import Core.Runtime.Entry;

class Adder {
    constructor(public int bias) {}

    #[Invoke]
    function add(int x): int { return x + this.bias; }

    #[Invoke]
    function addPair(int x, int y): int { return x + y + this.bias; }

    #[ToString]
    function describe(): string { return "Adder(+{this.bias})"; }
}

#[Entry]
function main(): void {
    Adder a = new Adder(10);
    Output.printLine("{a(5)}");        // 15  — #[Invoke] sugar (arity-1 target)
    Output.printLine("{a(1, 2)}");     // 13  — overload resolution (arity-2 target)
    Output.printLine("{a.add(5)}");    // 15  — direct call ALWAYS stays legal
    Output.printLine("{a}");           // Adder(+10) — #[ToString] in string context
}
```

## 2. Semantics (locked)

- **`#[Invoke]`** (D9a): `x(args)` on a class instance statically rewrites (checker) to the
  matching `#[Invoke]` method call. The class is **assignable to a matching function type**
  (route handlers, callbacks): assignability holds iff exactly one `#[Invoke]` signature
  matches the target function type.
- **Overloading** (D9c): multiple `#[Invoke]` methods with DIFFERENT signatures are all call
  targets (arbitrary method names); resolution by arity/types at the call site. Two with the
  SAME signature = compile error `E-INVOKE-DUPLICATE`.
- **`#[ToString]`** (D9b): STRICT signature — zero params, returns `string` (violation =
  compile error `E-TOSTRING-SIGNATURE`); exactly ONE per class (`E-TOSTRING-DUPLICATE`);
  auto-called in string context (interpolation, print); an object WITHOUT `#[ToString]` in
  string context = compile error `E-NO-TOSTRING` (stricter than PHP's runtime warning).
- Both attributed methods **stay normally callable by name** — the attribute adds sugar only.
- Inheritance: attributes are inherited with the method; a subclass override keeps the
  attribute's role. A subclass may NOT add a second `#[ToString]` (the override IS the one).

## 3. Checker rules

1. `#[Invoke]`/`#[ToString]` legal only on instance methods (not statics, not free functions,
   not constructors) — `E-ATTRIBUTE-TARGET`.
2. Call-expression typing: `expr(args)` where `expr: C` (class type) → resolve against C's
   `#[Invoke]` set exactly like the existing overload resolution (same rules as named-method
   overloads); no match = the standard no-overload error naming the candidates.
3. Function-type assignability (`C` where `(T...) -> U` expected): exactly one `#[Invoke]`
   matches → OK (the checker records the chosen method in a side table for backends);
   zero/ambiguous → error.
4. String-context check: interpolation segment / print operand of class type requires
   `#[ToString]` (`E-NO-TOSTRING` otherwise).

## 4. Backends (Invariant 17: run + transpile + lift in the same change)

- **Interp/VM**: the checker rewrite makes `x(3)` a plain (overloaded) method call — the VM
  already dispatches overloads (`Op::CallOverload` + `dispatch::select_overload`), byte-identity
  by construction [Verified in D9c: overload tables exist on both backends]. String context
  lowers to the `#[ToString]` method call before backends (compile-time sugar, Invariant 5).
- **Transpile — LADDER CHECK (owed to dev, Invariant 14/16):** PHP has ONE `__invoke` per class.
  - Single `#[Invoke]` → emit native `__invoke` (faithful, tier 1).
  - MULTI `#[Invoke]` → **RULED (§7): emit `__invoke(...$args)` + the
    `__phorj_invoke_dispatch` arity/type shim** (META-7: `__phorj_*` helpers are an accepted
    tool, trade surfaced and ruled);
  - `#[ToString]` → native `__toString` (faithful).
- **Lift (PHP→phorj)**: `__invoke` lifts to `#[Invoke]` on a method named `invoke`;
  `__toString` lifts to `#[ToString]` on `toString`.

## 5. Faults / diagnostics (canonical strings fixed at build)

`E-INVOKE-DUPLICATE`, `E-TOSTRING-DUPLICATE`, `E-TOSTRING-SIGNATURE`, `E-NO-TOSTRING`,
`E-ATTRIBUTE-TARGET` — all compile-time; no new runtime faults.

## 6. Examples & tests (Inv 9)

`examples/invoke_tostring.phg` (the §1 program) + README row; checker negative tests for all
five errors; differential coverage via the example; transpile snapshot incl. the multi-invoke
shim dispatch.

## 7. RULED (dev, 2026-07-23)

- **P1 → (a) the `__phorj_invoke_dispatch` arity/type shim**: multi-`#[Invoke]` emits
  `__invoke(...$args)` + the shim; single-`#[Invoke]` emits native `__invoke`.
- **P2 → yes, everywhere**: `Conversion.toString(obj)` lowers to the same `#[ToString]` call —
  one stringification story.

## 8. BUILD STATUS (2026-07-23 — autonomous slice)

**SLICE 1 — BUILT + byte-identity-green** (`phg run` ≡ `phg run --tree-walker` ≡ transpiled PHP,
verified on the example + the `a(5) + 1` CTy-operand case):
- `#[Invoke]` direct calls `x(args)` → checker resolves the overload set and rewrites to
  `x.<method>(args)` (new pass `checker::resolve_invoke_tostring`, runs OUTERMOST on the live AST);
  overloaded `#[Invoke]` methods dispatch by arity/type.
- `#[ToString]` in interpolation holes AND `Conversion.toString(x)` → rewritten to `<expr>.<method>()`
  (spec §2 P2, one stringification story). `E-NO-TOSTRING` when an object hits string context without one.
- Guards: `E-ATTRIBUTE-TARGET`, `E-TOSTRING-SIGNATURE`, `E-TOSTRING-DUPLICATE`, `E-INVOKE-DUPLICATE`,
  plus `E-NOT-CALLABLE` (a class value with no `#[Invoke]`) and reused `E-OVERLOAD-NO-MATCH`.
- Transpile emits a native delegating PHP `__toString`; lift maps PHP `__toString` → `#[ToString] toString`.
- Roles inherit with the method (subclass + trait). Example `examples/guide/invoke-tostring.phg`;
  12 checker tests + transpile snapshot + lift test.
- Resolution note (recorded): `#[Invoke]` marks a method NAME callable (all overloads of a marked name
  participate); the call picks the FIRST arity/type match in declaration order (deterministic — there is
  no runtime re-dispatch, the rewrite names one concrete method, so all backends run the checker's pick).

**SLICE 1b — DEFERRED (coupled "instance as a first-class callable VALUE" cluster; recorded so it is
reopenable, dev to schedule):**
- **Function-type assignability** (spec §2 D9a / §3.3): a class with exactly one matching `#[Invoke]`
  assignable to a `(T…) -> U` function type (route handlers/callbacks). Needs a `ty_assignable`
  `(Named, Function)` arm + a coercion mechanism (a lambda-wrap at the coercion site, uniform across
  backends). NOT load-bearing for slice 1 (the example makes no assignability claim).
- **Transpile PHP `__invoke`** (single delegate) + the MULTI-`#[Invoke]` `__phorj_invoke_dispatch`
  arity/type shim (spec §4 / §7 P1) — only observable once an instance is used AS a PHP callable
  (i.e. behind the assignability above); slice-1 direct calls transpile as ordinary `$x->method(args)`.
- **Lift PHP `__invoke` → `#[Invoke]`** (spec §4) — deferred with the emit side to keep transpile/lift
  symmetric; consequence: a phorj `#[Invoke]` class does NOT round-trip through transpile→lift in slice 1
  (`#[ToString]` DOES). The §6 "transpile snapshot incl. the multi-invoke shim dispatch" test is owed to 1b.

String context is lowered EVERYWHERE it is recorded — method/ctor/hook bodies AND field initializers
(`string s = "{obj}";`), on classes and traits — so the byte-identity spine holds uniformly. (The
field-initializer case was a real gap caught by the round-2 review and fixed before ship.)

**Known slice-1 limitations (recorded):** an interface-/enum-typed receiver used as `x(args)` or in
string context reports `E-NOT-CALLABLE`/`E-NO-TOSTRING` with class-oriented wording (over-rejection,
never unsound); class-level `#[ToString]`/`#[Invoke]` uniqueness is enforced for classes and traits.
A class that acquires `#[ToString]` ONLY via a `use`d trait gets no PHP `__toString` delegate emitted
(the emitter scans the class's own members) — phorj-internal string context still works via the
named-method rewrite; only external PHP-host coercion (`echo $obj`) is affected. Slice-1b follow-up.

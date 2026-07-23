# SPEC — `#[Invoke]` + `#[ToString]` (DEC-331 D9, build slice 1 of 3)

> Status: **SPEC FROZEN, awaiting dev ruling (D10b: dev rules on each spec before any code).**
> Rulings elaborated here: D9a (attribute-marked callability), D9b (`#[ToString]`), D9c
> (overloaded invoke). Nothing in this spec re-decides a ruling; PENDING points are marked.

## 1. Surface

Attributes designate conventional methods (phorj's unified model — no magic method names):

```phg
package Main;
import Core.Output;
import Core.Runtime.Entry;

class Adder {
    int bias;
    function construct(int bias) { this.bias = bias; }

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
  - MULTI `#[Invoke]` → **PENDING dev ruling, options**:
    (a) *(recommended)* emit `__invoke(...$args)` + a `__phorj_invoke_dispatch` arity/type shim
        (META-7: `__phorj_*` helpers are an accepted tool, trade surfaced);
    (b) `E-TRANSPILE-MULTI-INVOKE` hard error (tier 2 quarantine);
  - `#[ToString]` → native `__toString` (faithful).
- **Lift (PHP→phorj)**: `__invoke` lifts to `#[Invoke]` on a method named `invoke`;
  `__toString` lifts to `#[ToString]` on `toString`.

## 5. Faults / diagnostics (canonical strings fixed at build)

`E-INVOKE-DUPLICATE`, `E-TOSTRING-DUPLICATE`, `E-TOSTRING-SIGNATURE`, `E-NO-TOSTRING`,
`E-ATTRIBUTE-TARGET` — all compile-time; no new runtime faults.

## 6. Examples & tests (Inv 9)

`examples/invoke_tostring.phg` (the §1 program) + README row; checker negative tests for all
five errors; differential coverage via the example; transpile snapshot incl. the multi-invoke
resolution once ruled.

## 7. PENDING for dev

- **P1**: multi-`#[Invoke]` PHP leg — shim (a) vs hard error (b) above.
- **P2**: does `#[ToString]` auto-apply in `Conversion.toString(obj)` too (recommended: yes,
  same lowering) or only in interpolation/print?

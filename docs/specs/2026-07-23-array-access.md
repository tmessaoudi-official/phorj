# SPEC — ArrayAccess: `#[ArrayGet]` / `#[ArraySet]` (DEC-331 D10c, the "spec tomorrow" hold)

> Status: **SPEC RULED (dev, 2026-07-23) — BUILD-READY, with a REOPEN flag** (dev: adopted "but we might revisit — put it where we can reopen it later"; this header + the DEC-331 register addendum are that reopen hook). D10c candidate being elaborated: attribute-
> designated indexers, consistent with the attribute-conventional model (`#[Invoke]`,
> `#[ToString]`, `#[Entry]`, `#[Config]`).

## 1. Surface

```phg
class Matrix {
    List<float> cells;
    int cols;
    function construct(int rows, int cols) { /* ... */ }

    #[ArrayGet]
    function at(Pair<int, int> rc): float {
        return this.cells[rc.first * this.cols + rc.second];
    }

    #[ArraySet]
    function put(Pair<int, int> rc, float v): void { /* ... */ }
}

Matrix m = new Matrix(3, 3);
m[Pair.of(1, 2)] = 4.5;           // -> m.put(Pair.of(1, 2), 4.5)
float x = m[Pair.of(1, 2)];       // -> m.at(Pair.of(1, 2))
```

## 2. Semantics

- `#[ArrayGet]` on a 1-param method: `obj[k]` (read position) statically rewrites to that
  method call; the KEY TYPE is the parameter type, the ELEMENT TYPE the return type — fully
  typed indexing (vs PHP's `mixed offsetGet(mixed)`).
- `#[ArraySet]` on a 2-param `(key, value): void` method: `obj[k] = v` rewrites likewise.
- **OVERLOADED indexers ARE in v1** (ruled): multiple `#[ArrayGet]`/`#[ArraySet]` with
  DISTINCT key types resolve statically by key type (the D9c `#[Invoke]`-overload machinery);
  two with the SAME key type = `E-ARRAYACCESS-DUPLICATE`. Strict signatures
  (`E-ARRAYACCESS-SIGNATURE`).
- Read-only types: `#[ArrayGet]` without `#[ArraySet]` — `obj[k] = v` is then
  `E-NOT-INDEX-ASSIGNABLE` at compile time (PHP throws at runtime; we reject statically).
- NO `offsetExists`/`offsetUnset` analogs in v1: existence is the key-type's job (`T?` return
  or a normal `has` method); unset has no phorj analog (immutable-leaning collections).
- The attributed methods stay normally callable by name (the house rule).
- Mutation semantics follow the receiver: `m[k] = v` on a CLASS instance is an in-place
  method call (classes are reference-like today) — no COW surprise; collections' built-in
  indexing is untouched (this sugar applies only to user classes, `E-ATTRIBUTE-TARGET`
  keeps it off natives).

## 3. Backends (Invariant 17)

- **Compile-time sugar (Invariant 5)**: both rewrites happen in the checker/expansion
  chokepoint — backends and the PHP output never see indexer attributes, only plain method
  calls. NO new `Op`. (The compiler's `Op::Index`/`SetIndexLocal` paths stay
  collection-only; the CTy-operand trap does not fire — `m[k]` types as the getter's return,
  Invariant 7 note recorded at build.)
- **Transpile (ruled): plain method calls (tier 1) PLUS the PHP `ArrayAccess` glue** —
  emitted classes `implement \ArrayAccess` with all four methods: `offsetGet`/`offsetSet`
  delegate to the attributed methods (multi-key overloads dispatch via the same `__phorj_*`
  shim pattern as multi-invoke), `offsetExists` = getter nullability (exact semantics fixed
  at build), `offsetUnset` throws (phorj has no unset). Gain: PHP-side interop (frameworks
  type-hinting `ArrayAccess`). phorj-compiled output is byte-identical either way.
- **Lift**: PHP classes implementing `ArrayAccess` lift `offsetGet`→`#[ArrayGet]`,
  `offsetSet`→`#[ArraySet]`; `offsetExists`/`offsetUnset` lift as plain methods with a
  disclosure comment (no sugar) — closes a lift gap.

## 4. Examples & tests (Inv 9)

`examples/array_access.phg` (the Matrix above + a typed registry keyed by string) + README
row; differential across backends; checker negatives (duplicate, bad signature, read-only
assignment, attribute on a native/static); lift round-trip of a PHP `ArrayAccess` class.

## 5. RULED (dev, 2026-07-23)

- **P1 → ADOPTED, with a REOPEN flag** ("we might revisit — put it where we can reopen it
  later"): the reopen hook is this spec's header + the DEC-331 register addendum row.
- **P2 → overloads IN v1** (distinct key types, D9c machinery).
- **P3 → EMIT the `\ArrayAccess` glue** (PHP-side interop; costs accepted).

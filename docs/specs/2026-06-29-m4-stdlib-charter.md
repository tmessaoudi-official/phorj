# M4 — Standard Library Charter

> Status: **adopted** (2026-06-29). This is the governing policy for every `Core.*` standard-library
> module. It is **descriptive of the conventions already practised** across the shipped modules
> (`Core.Console`/`Math`/`Text`/`List`/`Map`/`Set`/`Json`/`Bytes`/`Html`/`File`/`Time`/`Convert`/
> `Decimal`/`Regex`/`Hash`/`Encoding`/`Url`/`Csv`/`Random`/`Validate`/`Crypto`/`Reflect`/`Test`) and
> **prescriptive** for everything added next (the M11 breadth push). When a new native disagrees with
> this charter, change the native — or amend the charter in the same change with a rationale.

This charter exists so that breadth stops being decided ad hoc. The five axes below — **naming**,
**argument order**, **optional-vs-fault**, **determinism tiers**, and **native-vs-`.phg`** — are the
recurring decisions every stdlib addition faces.

---

## 1. Naming

- **Modules are `Core.<Pascal>`** — a reserved `Core` root (see the namespace design) plus a PascalCase
  leaf: `Core.List`, `Core.Text`, `Core.Json`. Jargon-free, domain-obvious leaves (`Console` not `Io`,
  `File` not `Fs`).
- **Functions are `camelCase`** — `parseInt`, `splitOnce`, `startsWith`, `isEmpty`. A multi-word name
  reads as a verb phrase or a predicate.
- **Predicates start `is`/`has`/`starts`/`ends`/`contains`** and return `bool` — `isEmpty`, `hasKey`,
  `startsWith`, `containsIgnoreCase`. Never return `0`/`1` or `int?` where a `bool` is meant.
- **A name must not collide with a PHP-reserved symbol after erasure.** A native that transpiles to a
  PHP builtin keeps its Phorj name distinct from the builtin (the function is the *Phorj* surface; the
  PHP mapping is an implementation detail).
- **No abbreviations that aren't already idiomatic** — `length` not `len` *as the public name where the
  established module already uses `length`*; match the sibling module (`Text.length`, `List.length`).
  Consistency within and across modules beats individual preference.

## 2. Argument order — subject-first

Every native takes its **subject (the receiver-like value) first**, then operands, then options:

```
Text.split(s, sep)            List.map(xs, f)            Map.getOr(m, key, default)
Text.replace(s, from, to)     List.reduce(xs, init, f)   Decimal.div(a, b, scale, mode)
```

- The first parameter is the thing the operation is *about* (the string, the list, the map). This is
  the order a future UFCS/method sugar (`s.split(sep)`) would desugar to, and it reads left-to-right.
- A **closure/callback argument goes last** (`List.map(xs, f)`, `List.reduce(xs, init, f)`) — it is the
  longest, most-likely-multiline argument, and last position keeps call sites readable.
- **Options/config go after the required operands** (`Decimal.div(a, b, scale, mode)`).
- Phorj has no named/keyword arguments; default parameters (M4) fill trailing positions, so order the
  *most-likely-omitted* argument last.

## 3. Optional vs. fault — the recoverability rule

The single most important stdlib decision: when an operation can't produce a value, does it return
`T?` (a recoverable absence) or **fault** (an unrecoverable bug)?

- **Return `T?` when absence is an ordinary, expected outcome the caller routinely handles**:
  `List.first(xs) -> T?` (empty list), `Map.get(m, k) -> V?` (missing key by lookup), `Text.parseInt(s)
  -> int?` (bad input), `Json.parse(s) -> Json?` (malformed input), `File.read(p) -> string?` (missing
  file). The caller composes with `??`, `?.`, if-let, or `match`. **This is the default for any
  parse/lookup/IO that can fail on normal input.**
- **Fault when the precondition is a programmer error the caller should never hit**: indexing past the
  end of a list (`xs[i]` OOB), a missing *required* map key via `m[k]` indexing (vs. the `Map.get`
  lookup), division by zero, an i64/i128 overflow, a negative `scale`. A fault aborts with a stack
  trace; it is a *bug*, not a condition.
- **Two surfaces for the same data are allowed and encouraged** when both modes are legitimate:
  `m[k]` (indexing, faults on miss — "I know it's there") **and** `Map.get(m, k) -> V?` (lookup, `null`
  on miss — "it might not be"). Document which is which.
- **`throws E` (checked exceptions) is the third tier** — for a recoverable error that carries
  *information* (not just absence) and should be enforced up the call chain. Reserve it for genuine
  error conditions with a payload; do not use it where `T?` suffices.
- A fault message is a **string literal baked at compile time** and must be **byte-identical across
  `run`/`runvm`** (compared by `FaultKind` in the differential harness). The transpiled PHP `throw`s a
  matching body.

## 4. Determinism tiers — what may enter `differential.rs`

The byte-identical `run ≡ runvm ≡ real PHP` spine is sacred. A native is classified by determinism:

- **Tier 1 — pure & deterministic.** Output is a pure function of inputs, identical on every backend
  and platform. Almost all of `Core.List`/`Text`/`Map`/`Set`/`Math`(integer)/`Json`/`Bytes`. These are
  byte-identity-gated and **must ship with a runnable guide example** under `examples/guide/`.
- **Tier 2 — deterministic but representation-sensitive.** The *value* is deterministic but its
  *printed form* can differ between Rust's shortest-round-trip and PHP's `echo` (irrational floats,
  `NaN`/`inf`, `1e20`). The `run ≡ runvm` spine is always identical (both Rust); only the comparison to
  PHP's *native* formatter differs. Such a native **must not be printed raw in an example** — exercise
  it through a predicate or a formatter that collapses the difference (`numberFormat`, exact IEEE
  points). Documented in `KNOWN_ISSUES.md`.
- **Tier 3 — impure / non-deterministic.** Touches the clock, the filesystem with external state, the
  network, randomness, the environment, or the process. These are **quarantined**: excluded from
  `differential.rs` (`uses_impure_native`), validated by their own dedicated tests
  (`tests/random.rs`, `tests/process.rs`, `time_tests.rs`) with seeded/injected/fixture inputs.
  - **Network is forbidden until M6** — it breaks both zero-dependency (Rust std has no HTTP client)
    *and* determinism. The determinism, not the dependency, is the gate.
  - A clock or RNG used by a server (`--workers > 1`) may share one global stream; document the
    reproducibility caveat.

## 5. Native (`Rust`) vs. `.phg` prelude

- **Write a native (`src/native/<module>.rs`) when** the operation needs Rust-level primitives — string
  algorithms, hashing, regex, IO, numeric kernels — or must be a single op the compiler can type. A
  native single-sources its checker signature, its `eval` (`NativeEval::Pure` | `HigherOrder` |
  `Reflective`), and its `php` mapping in one `NativeFn`.
- **Write an injected `.phg` prelude (`cli::inject_*_prelude`, gated on the import) when** the feature
  is best expressed *in Phorj itself* — a type with methods (the `Json` enum, `RoundingMode`, the
  `Core.Time` `Instant`/`Duration` view) — so it rides the existing backends with no new plumbing and is
  itself byte-identity-gated. Inject before `check`, gated on the module's import.
- **Higher-order natives** (`map`/`filter`/`reduce`) take a closure and run it via the backend-supplied
  `ClosureInvoker` (the re-entrant VM `run_until`); a closure's result *and* any fault it throws are
  byte-identical to the interpreter by construction. No new `Op`.
- **The PHP mapping uses only `-n`-available core** (PCRE, not mbstring) — the oracle runs `php -n`. The
  one documented exception is `decimal` (BCMath, loaded explicitly). A native must not require a
  non-core PHP extension.
- **Erasure-safety:** a native's `Ty::Param` is registry-only (never erased) and safe because the
  compiler types a native call by expression *shape* (→ `CTy::Other`) and the transpiler emits via the
  `php` closure — so no type variable reaches a backend.

## 6. Every native ships complete (the developer rule)

Per the standing "examples ship with features" rule, a Tier-1/Tier-2 native lands in the **same change**
as: (1) a runnable `examples/guide/<topic>.phg` line exercising it (auto byte-identity-gated by the
`tests/differential.rs` glob), (2) a `examples/README.md` coverage-matrix entry, (3) unit tests in the
module's `*_tests.rs`, and (4) a `KNOWN_ISSUES.md` note for any Tier-2 representation caveat. A Tier-3
native ships with its dedicated non-`differential` test instead of an example.

---

## Quick decision tree for a new stdlib function

1. **Name** it `camelCase`; predicate → `is…`/`has…` returning `bool`; module is `Core.<Pascal>`.
2. **Subject first**, operands next, options after, **closure last**.
3. Can it fail on *normal* input? → return **`T?`**. Only on a *programmer bug*? → **fault** (literal
   message, `FaultKind`-parity). Carries error *information* to enforce up-chain? → **`throws E`**.
4. Pure & deterministic? → **Tier 1**, byte-identity-gated, ship a guide example. Representation-
   sensitive? → **Tier 2**, never print it raw. Impure? → **Tier 3**, quarantine + dedicated test.
5. Needs Rust primitives / a typed single op? → **native**. Best expressed in Phorj? → **injected
   `.phg` prelude**. Takes a closure? → **higher-order native**.
6. Ship the example + README entry + tests + any Tier-2 `KNOWN_ISSUES` note in the **same change**.

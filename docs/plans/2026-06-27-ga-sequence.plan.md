# GA Sequence — charter → DX → test → text → breadth-gaps → numerics → lift → release

> Multi-batch autonomous run chosen 2026-06-27. Move GA% / Global% via the highest-leverage
> remaining chunks, in dependency order. Each slice ships byte-identity-gated (run≡runvm≡real
> PHP 8.5) + a guide example, per the standing rules. Commit green; **never push**.

## Decisions Log
- [2026-06-27] AGREED: do **all four candidate batches in sequence** (developer: "do them in
  sequence"), in the **reordered** order below — NOT the M-Test-first framing I led with.
- [2026-06-27] AGREED: **charter-first reorder** (developer chose "Charter-first, as recommended").
  Rationale: M-Test/M-text/breadth all add stdlib surface; minting them before the conventions
  charter risks an API codemod later (the PascalCase-reshape pain). Charter governs all new stdlib.
- [2026-06-27] AGREED: at genuine design forks (Core.Test assertion API, Core.Regex API, Secret<T>
  model) **stop and ask** via AskUserQuestion before committing the public surface (developer choice).
- [2026-06-27] NOTE: roadmap docs were stale — **error model Slice 2 (throws/Result/try-catch) is
  BUILT** (`Op::Throw/PushHandler/PopHandler`, lexer keywords) and **`phg lift` CLI ships**
  (`cmd_lift`, full `src/lift/`). M4 stdlib **breadth is largely built** (sort/map/list/text/set/
  as-cast/parseFloat). So the remaining work is lighter than the milestone titles imply.

## Decision review — autonomous decisions re-confirmed/changed by the developer (2026-06-27)
> Developer asked to review decisions made in prior autonomous sessions, keep-or-change, one by one.
- [2026-06-27] CSV backslash escape → **KEEP** (RFC-4180, no backslash escape). Confirmed.
- [2026-06-27] **CHANGE** Core.Csv.parse empty input `[""]` → **`[]`** (zero fields; matches Python/Rust,
  honest, round-trips). Was: one empty field. **TODO: implement.**
- [2026-06-27] **CHANGE** Core.Random quarantine → **byte-identical parity**: hand-roll xorshift64 in
  emitted PHP (logical vs arithmetic `>>` masking), Random rejoins the oracle, `pure: true`. **TODO.**
- [2026-06-27] **CHANGE** Decimal `/`: E-DECIMAL-DIV compile error → **exact-or-fault** — bare `/` keeps
  the exact value when the quotient terminates, **faults** at runtime when non-terminating or i128
  overflow. `Decimal.div(a,b,scale,mode)` stays for explicit rounded division. **TODO.**
- [2026-06-27] **CHANGE** Decimal `%`: was wrongly lumped with `/` (rejected). **Un-reject** — `%` is
  exact/closed on fixed-point (no rounding), a bare operator like `+ - *`. Developer confirmed Option 1.
  Open follow-up: add named `Math.rem`/`mod`(+`fmod`?) for symmetry with `Math.intdiv`. **TODO.**

### Batch 2 (scope & API)
- [2026-06-27] Math remainder → **operator-only**, no named `Math.rem`/`fmod` (`%` is exact, needs no
  rounding; the operator already covers int/float, decimal being added). Confirmed.
- [2026-06-27] **CHANGE** Core.Hash: digests-only → **add password hashing** (bcrypt/argon2). Non-
  deterministic (random salt) ⇒ must be **quarantined** + a **security design pass** (own module, e.g.
  `Core.Crypto`/`Core.Password`). **TODO: design first.**
- [2026-06-27] Static calls → **KEEP narrow scope** (own-class, non-overloaded) for now, **AND** schedule
  a **research + brainstorm pass** to cover statics comprehensively (inherited, overloaded, late static
  binding). **TODO: research milestone item.**
- [2026-06-27] **CHANGE** `as` operator → **support all types incl. primitives** (`x as int`). Needs a
  cast-vs-convert reconciliation design (don't reintroduce the C-cast surprise; unify with Core.Convert
  semantics — total vs optional). **TODO: design first.**
- [2026-06-27] AGREED (Option 2 build, item a — design fork resolved): `as`→primitives uses the
  **Unified, fallibility-typed** model. `x as T` (T primitive) result type tracks fallibility:
  **lossless/infallible → total `T`** (int→float, int→decimal, *→string, identity);
  **lossy or fallible → `T?`** (float/decimal→int = null unless integral; string→int/float = parse,
  null on non-numeric; primitive-union/erased member = assertion/narrow). **No silent lossy
  conversion** — lossy narrowing is always optional (loud null); `Convert.truncate` stays the named
  tool for "I want truncation". `T as T` = identity (W-redundant-cast lint).

### Batch 4 (minor / technical-constraint items)
- [2026-06-27] **CHANGE** float `/0` → **clean fault** (general principle: ANY division by zero throws —
  int/float/decimal `/` and `%`). Was: `1.0/0.0`→`inf` (IEEE), diverging from PHP DivisionByZeroError.
  Add `Math.fdiv` for explicit IEEE inf if ever wanted. Verify int/0 + decimal/0 already fault. **TODO.**
- [2026-06-27] **CHANGE** lambda bare-field `fn() => v` → fix the silent runtime failure (brainstorm:
  clear `E-LAMBDA-BARE-FIELD` vs auto-capture as `this.v`). **TODO: brainstorm form.**
- [2026-06-27] **CHANGE** overload erasure ambiguity → **reject** at declaration (`E-OVERLOAD-ERASE`)
  when two overloads differ only by string-vs-bytes or only among List/Map/Set. **TODO.**
- [2026-06-27] Map numeric-string-key coercion under PHP → **KEEP documented** (use non-numeric string
  keys when transpiling; run≡runvm always identical). No action.

### Batch 4b
- [2026-06-27] **CHANGE** `opt!`-on-null transpiled message → align emitted PHP message to the Rust
  backends' "force-unwrap of null" text. Cosmetic (fault domain). **TODO.**
- [2026-06-27] Transcendental last-ULP (Rust vs PHP libm) + `gcd(i64::MIN)` overflow-fault → **ACCEPT
  as-is** (physics / correct safety). No action.

- [2026-06-27] **CHANGE** numberFormat → **digit-based rounding on the shortest-round-trip decimal
  string** (same algorithm Rust + emitted PHP; no float×10^n scaling error; matches PHP's intended
  decimal). Closes the common-case money divergence. **TODO.**

## Decision-driven fixes — execution order (Option 1: do these, then resume GA sequence)
Each its own commit, TDD, byte-identity-gated (run≡runvm≡real PHP 8.5), + example where user-visible.
1. **CSV empty → `[]`** ✅ DONE `ea6bc96`.
2. **Division-by-zero cluster** ✅ DONE (float `/0`/`%0` now fault — `value::float_div`/`float_rem`
   → `Result`, wired through both backends + `__phorge_rem` PHP guard; int/0 + decimal-div/0 already
   faulted). `Math.fdiv` for explicit IEEE inf = deferred (add only if requested).
3. **Decimal `%` un-reject** ✅ DONE — exact remainder operator (`Op::RemD` → `value::decimal_rem` →
   `bcmod`; zero divisor faults; result scale = max). Checker allows `%`, keeps `/` rejected.
4. **Decimal `/` exact-or-fault** ✅ DONE — `Op::DivD` → `value::decimal_div_exact` (reduce fraction,
   strip 2s/5s, fault if non-terminating, minimal-form result). Transpiles to `__phorge_dec_div_exact`
   (bcdiv + exactness check + strip) byte-identical under PHP 8.5. `Decimal.div` (rounded) unchanged.
5. **numberFormat digit-based rounding** ✅ DONE — both legs digit-round the shortest-round-trip
   string (`__phorge_float`) by carry, not float-scaling. `0.285→0.29` byte-identical; `.5` divergence gone.
6. **Random → byte-identical parity** ✅ DONE — `pure: true`; transpiler hand-rolls the same xorshift64
   (`__phorge_rng_*`, logical-`>>` mask + signed `GOLDEN`); dice.phg now oracle-gated, seq identical 3-way.
7. **Overload erasure reject** ✅ DONE — `E-OVERLOAD-ERASE` at declaration via a `php_erasure_key`
   (string/bytes→string, List/Map/Set→array, Optional recursive); explain + checker test.
8. **Lambda bare-field fix** ✅ DONE — resolved by the bigger decision: **require `this.field`
   everywhere** (`E-BARE-FIELD`, PHP-faithful; `53dc203`). Migrated 16 examples + tests + Http prelude.
   Additive bonus ✅ DONE (`04ebe63`): the optional `fn(x): int => e` lambda return annotation was
   already built+parser-tested (parser `:`/`->`; checker assignability check); added the missing
   checker tests (match/mismatch, non-vacuous) + showcased it in `guide/lambdas-pipe.phg`
   (byte-identity-gated run≡runvm≡PHP) + README note. Backends ignore the annotation (checker-only).
9. **opt!-on-null PHP message** ✅ DONE — verified the body ALREADY matches across all backends
   (`"force-unwrap of null"`); only the source *location* differs (inherent to PHP exceptions,
   fault-domain). No code change; KNOWN_ISSUES note corrected (it overstated the difference).

**All 9 decision-fixes COMPLETE.** Additive bonus ✅ DONE (`04ebe63` — `fn(x): int => e` lambda
return annotation, coverage+example).

**Option 2 design-first items (each brainstorm + AskUserQuestion on the API before building):**
- (a) **`as`→primitives ✅ COMPLETE** (plan `docs/plans/2026-06-27-as-primitives-matrix.plan.md`;
  `fc60682` S1 + `85c569e` S2 + `bcb6ea7` S3+S4). Unified, fallibility-typed cast over the full
  primitive matrix + union assertion; no new `Op`/`Value`; byte-identical run≡runvm≡PHP 8.5.
  Design forks resolved with the developer (full matrix; honest/loud, not PHP coercion; bool
  conditions already strict everywhere — verified). Deferred edges in KNOWN_ISSUES.
- (b) **password hashing** — IN PROGRESS. **Decision (2026-06-27, after the developer challenged
  hard):** do NOT delegate to PHP and do NOT compromise security. Since secure password hashing
  requires a vetted impl ("never roll your own") and `std` has no crypto, the developer's rules
  *force* the first external crate. **Adopted RustCrypto `argon2`** (Argon2id) behind a written
  **dependency policy** (`docs/specs/2026-06-27-dependency-policy.md` — audited-crypto-only exception
  to `std`-only). `Core.Crypto.hashPassword`/`verifyPassword`/`needsRehash` implemented **natively in
  the Rust backends** (run/runvm), transpiling to PHP `password_hash(ARGON2ID)`/`password_verify` as a
  **peer emission target** (standard PHC `$argon2id$…` ⇒ Rust↔PHP cross-verify). `pure:false`,
  EXCLUDED from the byte-identity oracle (random salt); dedicated `tests/crypto.rs`; a **verify-only**
  example (committed PHC hash) IS gateable (deterministic). argon2 feature-gated OFF for the WASM
  playground. **Principle reaffirmed:** transpile/lift are migration+test bridges, never a runtime
  Phorge depends on — every native has a real Rust impl; PHP is only an emission target.
- (c) **statics research** — inherited/overloaded/LSB statics; research + brainstorm pass.

**Then design-first items** (each: brainstorm + AskUserQuestion on the API before building), slotted
into the GA sequence: `as`→primitives (cast/convert reconciliation) · password hashing (quarantined
`Core.Crypto`) · statics research/brainstorm (inherited/overloaded/LSB).

## Sequence (dependency order)
1. **M4 charter** — codify the *de-facto* conventions from the ~18 shipped native modules into a
   one-page conventions doc + minimal enforcement. Governs items 3–6. (No API rework: descriptive.)
2. **`phg fmt` + lints** — `fmt` reuses the existing `src/lift/printer.rs`; add unused-import /
   unused-local lints on the warning channel. Near-free; speeds all later authoring.
3. **M-Test** — `phg test` runner + `Core.Test` assertions + `assertFaults` + fixtures/selection/skip
   (+ PHPUnit bridge if cheap). Determinism seam (seedable Random + quarantine) already built. **FORK.**
4. **M-text** — `Core.Regex` (PCRE `/u`), codepoint-aware string ops, `\u{…}` escapes, `number_format`.
   **FORK** (regex API surface).
5. **Breadth gaps** — only what `m4-stdlib-breadth.plan.md` left open (most is ✅); `core.json`
   safe-parse hardening, path/log/sprintf if not present.
6. **Close M-NUM S4** — Math breadth + `number_format` (shared with M-text). Flips M-NUM to ✅.
7. **lift L5** — PHP→Phorge→PHP round-trip oracle gate. Flips lift to ✅.
8. **Release-readiness** — M8 security hardening (injection guards, `Secret<T>` **FORK**, `write_atomic`)
   → GA governance docs (semver/BC/conformance corpus/security model) → M2.5 Phase 3 (CI stub registry
   + `--sign`). Docs last: they describe a stable surface.

## Status
- [ ] 1. M4 charter — IN PROGRESS
- [ ] 2. phg fmt + lints
- [ ] 3. M-Test
- [ ] 4. M-text
- [ ] 5. breadth gaps
- [ ] 6. M-NUM S4
- [ ] 7. lift L5
- [ ] 8. release-readiness

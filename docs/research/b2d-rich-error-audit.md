# B-2d — rich-error audit (Wave B error-model completion)

**Date:** 2026-07-05 · **Status:** audit complete; implementation split into (safe/done) + (deferred,
design-heavy). Governs DEC-180 (reclassify faulting natives) + UA-1.8 (fault-message canonicalization,
DEC B2-9). Read alongside `docs/INVARIANTS.md` §1 (backend parity — faults compared run≡runvm by
`FaultKind`) and `tests/differential.rs`.

> **⚠ CORRECTION (2026-07-05, same day) — the "3 latent byte-identity divergences" below were WRONG and
> are RETRACTED.** This audit's first pass used the wrong divergence criterion: it flagged
> `List.chunk`/`Hash.hkdf`/`Conversion.toString` because the Phorj fault text ≠ the transpiled PHP's
> error text. **Text-mismatch is NOT the project's fault-parity criterion.** Verified from primary
> sources: `agree_err` (differential.rs:148) compares **run vs runvm ONLY** (never PHP); `run_php`
> asserts **exit-0** (non-faulting stdout only); faulting programs are not examples (Invariant 9); and
> the `__phorj_clamp` comment (transpile/program.rs:846) states outright *"a fault is never a
> byte-identity example… only that both legs fault"*. **The real rule: where Phorj faults, PHP must
> also FAULT (not silently succeed with a value); the text need not match.** All three cases DO fault in
> PHP (`array_chunk`/`hash_hkdf` → `ValueError`; `(string)$closure` → `Fatal`) → **behaviourally
> consistent, not divergences.** DEC-195's guard-helpers are therefore **cosmetic (PHP-error wording),
> not a correctness fix** — see the retraction + the correct-lens re-frame at the end. The verify-regime,
> bucket taxonomy, and "skip signature guards / value guards are reachable" findings below still stand.

## How native faults work (the verify regime — settle before editing any string)

- A native is `NativeEval::Pure/HigherOrder/Reflective(fn(...) -> Result<Value, String>)`. The `Err`
  **String is the fault body** (`src/native/mod.rs:134`).
- The differential harness (`tests/differential.rs:111 fn classify`) compares faults by **semantic
  `FaultKind`, NOT raw text**. A fault whose message matches a named arm (`integer overflow`,
  `division by zero`, `list index out of range`, force-unwrap, …) classifies to that kind; **anything
  else falls to `FaultKind::Other(full_message)`**, which IS text-compared.
- **What is compared:** `agree_err` compares **run vs runvm ONLY** by `FaultKind`. **The PHP leg is NOT
  compared for faults at all** (`run_php` asserts exit-0; faulting programs are not examples). So a
  native fault's *text* is held to run≡runvm parity (and, for `Other`-classified faults, that text
  compare is exact — hence the W0-5 interpolation-line-skew care), but **against PHP only the
  *behaviour* matters: PHP must also fault, text irrelevant.**
- **Consequence:** "full oracle gate green" is necessary but **not sufficient** per-string for the
  run≡runvm leg — a `FaultKind`-classified or unexercised string can drift between the two Rust backends
  and stay green; attach a per-string `agree_err`/assertion. But there is **no PHP-text obligation**.

## Two lowering regimes for a faulting native — behaviour only (text never compared to PHP)

Traced by `phg transpile` on a fault-triggering program. **NOTE (corrected):** the "Byte-identity"
column below originally read this backwards. What actually matters is the last column — *does PHP
fault?* If PHP faults (any error), the program is behaviourally consistent regardless of text; the only
real hazard is a builtin that **returns a value instead of throwing** (Phorj-faults-but-PHP-succeeds).

| Regime | PHP leg | Fault text source | Byte-identity | Example |
|---|---|---|---|---|
| **Phorj-helper** | `__phorj_*` helper emitted into the PHP | the helper `throw`s the **same** string the Rust native returns | clean two-site; add an `agree_err` case | `Math.clamp`→`__phorj_clamp`, `Random.intBetween`→`__phorj_rng_int_between` |
| **Raw PHP builtin (that THROWS on bad input)** | a bare PHP builtin call (`array_chunk`, `hash_hkdf`) | **PHP's own** `ValueError`/`TypeError` — different text | **CONSISTENT** (both fault; text differs, which is permitted) | `List.chunk`→`array_chunk`, `Hash.hkdf`→`hash_hkdf` |
| **Raw PHP builtin (that RETURNS a value on bad input)** | a bare builtin that does NOT throw | — | **DIVERGENT** — Phorj faults, PHP silently succeeds (the real hazard; pre-helper `Math.clamp` was this) | *unknown — the correct-lens pass below was NOT run* |

## Fault-string buckets

- **(a) SIGNATURE guards** — `Module.func expects (types)` with a *static* type list and NO runtime
  value marker (e.g. `Bytes.slice expects (bytes, int, int)`, `Encoding.hexEncode expects (bytes)`,
  `File.copy expects (string, string)`). Fire only on wrong arity/type, which the **type checker
  prevents** → **checker-unreachable** → differential-blind, cosmetic. **Action: SKIP** (untestable
  dead-path churn with typo risk). Revisit only if a module name is stale (none are — see below).
- **(a′) VALUE guards — REACHABLE, do NOT skip (6C correction, 2026-07-05).** Distinguished from (a) by
  a runtime tell: they interpolate a runtime value (`{}`) or say `cannot`/`found`/`invalid`/`got` — they
  fire on input the checker **admitted**. Confirmed reachable set (grep `{}`/`cannot`/`found`/`invalid`
  in the native strings): `Conversion.toString cannot convert {}` (traced — reachable, see divergence
  #3 below), `Csv.format … found element of type {}`, `List.sum … found element of type {}`,
  `String.join … found element of type {}`, `List.filter`/`Map.filter`/`Option.filter … must return
  bool, got {}`. Each must be **traced individually** (checker-valid trigger + three legs) and bucketed
  as user-facing (b) — several are non-canonical (no colon) AND may be latent divergences. The earlier
  "skip ~40 arity guards" was **over-broad**: reachability, not string shape, is the gate.
- **(b) user-facing faults** — reachable on valid-but-bad input (`List.chunk size…`, `List.fill count…`,
  `Hash.hkdf length…`, `Random.intBetween…`, `String.count`/`pad`, `Math.clamp`, `Conversion.decimalToFloat`,
  `Csv.format` element-type). Parity-affecting. **The 8 that reach the canonical `Module.function:`
  shape are ALREADY canonical (UA-1.8 part-1 did this).**
- **(c) PHP-mirroring** — `integer overflow`, `division by zero`, `modulo by zero`, `list index out of
  range` (value-kernel, `src/value.rs`). Stay byte-exact (DEC B2-9 confirmed). **EXCLUDED from any sweep.**
- **(d) test fixtures** — `assertion failed…`, `d6 roll…`, `a fixed seed must…` (test `.rs`/selftest).
  Not stdlib. Out of scope.

## UA-1.8 (DEC B2-9) — status: DONE except the reachable value-guards (a′)

- Canonical shape = `Module.function: lowercase message`. The **8 faults matching that shape already
  match** (part-1). **No stale module-name fault strings remain** (`Text.`/`Convert.`/`Validate.`/
  `Bytes.from_string` — grep-verified empty; the `bytes_from_string` hit is a Rust fn name, not a fault).
- **CORRECTION (6C):** "8 canonical strings exist" (a `wc -l`) is NOT "all reachable user-facing faults
  are canonical." The bucket-(a′) VALUE guards are reachable AND non-canonical — traced example
  `Conversion.toString cannot convert {}` type-checks clean and faults at runtime. So UA-1.8 is **done
  except** the (a′) set: each must be traced + canonicalized to `Module.function: …` with its own
  `agree_err`/string-assertion, and some are also divergences (below).
- The bucket-(a) SIGNATURE guards stay checker-unreachable / differential-blind → **skip** (cosmetic).
- **Conclusion:** the "~40-string sweep" premise is stale, but the residual is NOT empty — it is the
  (a′) value-guard set + the reclassification below, not the dead-path signature guards. **Caveat: the
  (a′) list was found by marker-grep (`{}`/`cannot`/`found`/`invalid`) — a heuristic, NOT an exhaustive
  reachability sweep.** A reachable non-canonical fault carrying none of those markers can still exist
  (`List.chunk size must be at least 1` is exactly such a string — caught via the builtin-path trace,
  not the grep). Treat (a′) as a **starting set**; a definitive UA-1.8 close needs a reachability pass
  over every native fault (trace, don't pattern-match).

## DEC-180 reclassification — the genuine residual work (design-heavy → developer adjudication)

Ruling (DEC-180): reclassify **normal-input** native failures to `Result`/`throws`/`T?`; faults stay
uncatchable (bugs only).

### RETRACTED — the "3 latent divergences" were a wrong-lens error (see the top banner)

The three cases below were originally listed as "latent byte-identity divergences" because the Phorj
fault text ≠ PHP's error text. **That is not the parity criterion.** All three **fault in PHP** →
behaviourally consistent → **not divergences**:
- `List.chunk(xs, 0)` — PHP `array_chunk($xs, 0)` throws `ValueError`. Both fault. **Consistent.**
- `Hash.hkdf(..., len>8160)` — PHP `hash_hkdf(...)` throws `ValueError`. Both fault. **Consistent.**
- `Conversion.toString(<non-stringable>)` — PHP `(string)$closure` throws `Fatal`. Both fault.
  **Consistent.** (The `__phorj_str`-is-incomplete observation is real but harmless: an incomplete
  helper that still FAULTS is fine; it would only matter if it returned a value.)

**DEC-195 (guard-helper for these 3) is therefore OPTIONAL COSMETIC**, not a correctness fix — it would
only make PHP's error wording Phorj-flavoured, which the spine does not require and which matters little
given transpile is a bridge, not a runtime. The Rust-side string canonicalization (`List.chunk:` colon)
is harmless UA-1.8 tidy but is attached to no bug. **Developer must re-decide DEC-195 on this accurate
basis** (it was adjudicated on the wrong premise).

### The correct-lens residual — NOT YET RUN (the genuinely valuable pass)

The real byte-identity hazard is **Phorj-faults-but-PHP-SUCCEEDS**: a faulting native that lowers to a
PHP builtin which **returns a value instead of throwing** on the bad input (exactly what pre-helper
`Math.clamp` was — `max/min` computed a wrong value silently). Those are true divergences and are
**untested** (faults aren't in the example corpus). **This pass was NOT performed** — the first audit
searched for text-mismatch (the wrong signal) and never enumerated the succeed-vs-fault behaviour.

**Recommended method (fresh context):** for every reachable user-facing native fault, `phg transpile`
the fault-trigger and **run the transpiled PHP** — check the PHP **exit status**: non-zero (faults) →
consistent, ignore text; zero (succeeds) → **real divergence**, needs a `__phorj_*` guard helper (à la
clamp) so PHP faults too. Only that guard-helper work is correctness; text-matching is not.

## What this audit produced (docs-only)
No native/backend/fault string was edited. Net value after the same-day correction: (1) the verify
regime — `agree_err` is run≡runvm-only, PHP faults are behaviour-not-text; (2) the bucket taxonomy
(signature guards skip / value guards reachable); (3) UA-1.8 is done for the live surface bar the small
(a′) value-guard tidy; (4) **the correct divergence lens (succeed-vs-fault) and an explicit note that
that pass has not been run.** The retracted "3 divergences" are a cautionary example of asserting a
criterion (text-match) without checking what the harness actually enforces.

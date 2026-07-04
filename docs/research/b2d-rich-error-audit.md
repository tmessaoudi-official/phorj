# B-2d — rich-error audit (Wave B error-model completion)

**Date:** 2026-07-05 · **Status:** audit complete; implementation split into (safe/done) + (deferred,
design-heavy). Governs DEC-180 (reclassify faulting natives) + UA-1.8 (fault-message canonicalization,
DEC B2-9). Read alongside `docs/INVARIANTS.md` §4 (fault bodies are parity-affecting) and
`tests/differential.rs` (`FaultKind` classification).

## How native faults work (the verify regime — settle before editing any string)

- A native is `NativeEval::Pure/HigherOrder/Reflective(fn(...) -> Result<Value, String>)`. The `Err`
  **String is the fault body** (`src/native/mod.rs:134`).
- The differential harness (`tests/differential.rs:111 fn classify`) compares faults by **semantic
  `FaultKind`, NOT raw text**. A fault whose message matches a named arm (`integer overflow`,
  `division by zero`, `list index out of range`, force-unwrap, …) classifies to that kind; **anything
  else falls to `FaultKind::Other(full_message)`**, which IS text-compared.
- **Consequence (the trap):** for a `FaultKind`-classified fault, changing the Rust string is
  parity-safe but the gate gives **zero signal** — and the PHP-helper string can silently diverge. For
  an `Other`-classified fault, run/runvm/PHP text must match (multi-site) and the differential WILL
  catch a mismatch **iff an example exercises it**. "Full oracle gate green" is necessary but **not
  sufficient** per-string: a classified or unexercised string can be wrong and stay green.
- A user-facing native fault's text lives in **two** places that must agree for byte-identity: the Rust
  native AND the transpiled PHP. Which PHP side emits it depends on lowering (see below).

## Two lowering regimes for a faulting native (the key distinction)

Traced by `phg transpile` on a fault-triggering program:

| Regime | PHP leg | Fault text source | Byte-identity | Example |
|---|---|---|---|---|
| **Phorj-helper** | `__phorj_*` helper emitted into the PHP | the helper `throw`s the **same** string the Rust native returns | clean two-site; add an `agree_err` case | `Math.clamp`→`__phorj_clamp`, `Random.intBetween`→`__phorj_rng_int_between` |
| **Raw PHP builtin** | a bare PHP builtin call (`array_chunk`, `hash_hkdf`) | **PHP's own** `ValueError`/`TypeError` — a *different* string | **DIVERGENT** — Phorj fault text ≠ PHP error text | `List.chunk`→`array_chunk`, `Hash.hkdf`→`hash_hkdf` |

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
uncatchable (bugs only). The audit finds the actionable set is the **raw-PHP-builtin regime** faults —
where the Phorj fault text and PHP's builtin error already **diverge** and are (so far) **untested**, so
they are simultaneously (i) latent byte-identity bugs and (ii) reclassification candidates:

**Confirmed latent divergences (recorded in KNOWN_ISSUES):**
- `List.chunk(xs, 0)` — run/runvm: `"List.chunk size must be at least 1"`; PHP `array_chunk($xs, 0)`:
  a `ValueError`. Different text, `Other`-classified, no example exercises size 0 → green today, wrong.
- `Hash.hkdf(..., len>8160)` — run/runvm: `"Hash.hkdf: length must be 1..=8160"`; PHP `hash_hkdf(...)`:
  a PHP `ValueError`. Same shape of divergence.
- `Conversion.toString(<non-stringable>)` (e.g. a closure) — run/runvm: `"Conversion.toString cannot
  convert function"`; PHP `__phorj_str($v)` falls through to `(string)$v` → **PHP Fatal: "Object of
  class Closure could not be converted to string"**. Worst of the three (an uncatchable PHP Fatal, not a
  `ValueError`). **Important caveat this exposes: helper-regime is NOT automatically clean** — `__phorj_str`
  is an *incomplete* helper (no guard before `(string)$v`). "Lowers to a `__phorj_*` helper" must be
  verified to actually throw the Phorj string, not assumed.

**Recommended method for the next session (fresh context — parity-sensitive):**
1. Enumerate every reachable user-facing native fault; `phg transpile` each fault-trigger and label the
   lowering regime (helper vs raw builtin) — the table above is the template.
2. **Helper-regime** faults: already byte-identical by construction; only need an `agree_err` case if
   one is missing (verify per-string — the gate is text-blind for classified kinds).
3. **Raw-builtin-regime** faults: each is a design question — reclassify to `Result`/`T?`, or route
   through a `__phorj_*` guard helper that throws the Phorj string (making both legs agree), or accept
   PHP's error and match it. **These are §15 forks — surface to the developer, do not self-rule** (they
   change user-visible API / error surface). Ship each with an `agree_err` case.
4. Never infer per-string correctness from aggregate green — attach a named test (agree_err or direct
   string-assertion) to each changed string.

## What did NOT change in this audit
Docs-only. No native, backend, or fault string was edited (the safe, correct action given the buckets
above). The value is the reframing + the two latent-bug findings + the verify-regime that de-risks the
real implementation.

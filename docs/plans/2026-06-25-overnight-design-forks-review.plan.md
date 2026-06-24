# Overnight Design-Forks Review — 2026-06-25

> **Purpose.** During the autonomous overnight session of 2026-06-25, every *genuine design fork*
> (a decision the active master plan did NOT already resolve) is logged here instead of being decided
> silently. For each fork I make a **provisional** best-judgment call so work continues, but it is
> marked `⏳ AWAITING CONFIRMATION` until the developer reviews it in the morning.
>
> **How to use this in the morning:** walk the forks top-to-bottom. For each, either ✅ confirm my call
> or ✏️ change it. If you change one, I revisit the affected commit(s) — every fork entry names the
> exact commit(s) it touched so a reversal is mechanical.

## Decision rule I applied to every fork

Per [[philosophy-of-phorge]] and the project thesis (Phorge : PHP :: TypeScript : JavaScript): I picked
the **most PHP-familiar, most legible, most pragmatic** option that preserves the byte-identity spine
(`run ≡ runvm ≡ real PHP 8.5`) and adds no new `Op`/`Value` unless genuinely required. When two options
were close, I favored the one that removes a surprise without removing capability.

## Status legend
- `⏳ AWAITING CONFIRMATION` — provisional call shipped; needs your review.
- `✅ CONFIRMED` — you approved (fill in the morning).
- `✏️ CHANGED` — you redirected; follow-up commit noted.

---

## Forks log

### F-001 — UFCS fallback also resolves stdlib natives (not just free functions)
- **Slice / context:** Slice 6 (UFCS). The master plan's locked decision says UFCS is *"general (any
  free function), method-first … else free-function fallback."* But the plan's own motivating examples —
  `xs.length()`, `xs.filter(p).map(g)` — are **`Core.List` natives** (`List.length`/`map`/`filter`),
  addressed by `(module, name)`, NOT user free functions. So "free-function fallback" alone would make
  the flagship examples fail to resolve.
- **The fork:** Should `x.f(args)` fall back only to user free functions, or also to stdlib natives?
- **Options considered:**
  - A) Free functions only (literal plan text). Flagship examples (`xs.map`, `xs.filter`) would NOT
    work via UFCS — the guide example would have to define user free fns, gutting the feature's value.
  - B) Free functions **and** natives, mapping the receiver's type to its stdlib module
    (`List→Core.List`, `Map→Core.Map`, `Set→Core.Set`, `string→Core.Text`, `bytes→Core.Bytes`),
    requiring that module to be imported (consistency with "explicit import required").
- **My provisional call:** **B, via a general "callable by first-param" mechanism (no type→module
  map).** Resolution order: **method → user free fn → any *imported* native**, where a candidate is
  *any* callable named `f` whose **first parameter accepts the receiver type** (tested with the
  existing `unify`, so generic natives like `map: (List<T>,(T)->U)` match a `List<int>` receiver).
  `xs.map(g) ≡ List.map(xs, g)`, `s.upper() ≡ Text.upper(s)`, and even `xs.join(", ") ≡ Text.join(xs,
  ", ")` (note: `join` lives in `Text` but takes a `List` first — a rigid receiver-type→module map
  would have missed it; the by-first-param search finds it). Native candidates must be **imported**
  (preserves the namespace invariant); a user free fn wins over a native of the same name; two distinct
  imported natives both matching ⇒ `E-UFCS-AMBIGUOUS` (rare).
  *Rationale for dropping the type→module map:* it's simpler, strictly more general, matches the plan's
  "general — any free function" wording, and avoids the `join`-style mismatch.
- **Byte-identity / Op impact:** none — UFCS is a type-directed post-check AST rewrite (`rewrite_ufcs`,
  mirroring `resolve_html`); the rewritten node is an ordinary free/native call the backends already
  handle. No new `Op`, no new `Value`.
- **Touched commit(s):** `0dc071c` (feat(lang): UFCS — Phase 1 slice 6)
- **Reversal cost if you change it:** low. Dropping native fallback = delete the native branch of
  `try_ufcs`; the free-fn path and the rewrite mechanism stay.
- **Status:** ⏳ AWAITING CONFIRMATION

### F-002 — `?.` (safe-nav) UFCS deferred
- **Slice / context:** Slice 6 (UFCS).
- **The fork:** Should `x?.f(a)` (safe-nav) also UFCS-desugar when `f` is not a method?
- **My provisional call:** **Defer.** Only plain `.` UFCS this slice. `x?.f(a)` would mean
  "if x is null → null, else f(x!, a) wrapped optional" — extra lowering with little demonstrated need.
  A `?.` call with no matching method keeps today's error. (KNOWN_ISSUES.)
- **Byte-identity / Op impact:** none.
- **Reversal cost if you change it:** low-medium (adds an optional-peel + re-wrap to the UFCS path).
- **Status:** ⏳ AWAITING CONFIRMATION

### F-003 — number receivers (`int`/`float`) UFCS-to-`Core.Math` now *enabled* (consequence of F-001's general mechanism)
- **Slice / context:** Slice 6 (UFCS).
- **The fork:** Should `n.abs()` / `x.sqrt()` desugar to `Math.abs(n)` / `Math.sqrt(x)`?
- **My provisional call:** **Enabled (not deferred), as a free consequence of the by-first-param
  mechanism (F-001).** If `Core.Math` is imported, `Math.abs(int)`/`Math.sqrt(float)` have a numeric
  first param, so `n.abs()` resolves like any other native UFCS — no special-casing. Reads acceptably
  (`n.abs()`, `x.sqrt()`). If you'd rather forbid number receivers, that's a small guard to add.
- **Byte-identity / Op impact:** none.
- **Reversal cost if you change it:** low (add a guard rejecting primitive-number receivers in the
  native-UFCS search).
- **Status:** ⏳ AWAITING CONFIRMATION

### F-004 — multi-package UFCS-to-user-free-fn deferred (natives + single-package unaffected)
- **Slice / context:** Slice 6 (UFCS). The loader name-mangles non-`Main` package free fns to their
  PHP FQN and rewrites *qualified/ident* call sites — but a UFCS `x.f()` is a `Member` call the loader
  doesn't rewrite, so in a multi-package program `self.funcs` holds `f` under its mangled name and the
  bare-`f` UFCS lookup misses.
- **My provisional call:** **Defer** multi-package UFCS to a *user* free function. Single-package
  (`package Main`, all examples + the common case) works because there's no mangling; **native** UFCS
  works regardless (natives aren't mangled). (KNOWN_ISSUES.)
- **Byte-identity / Op impact:** none — it's a resolution-reach limitation, not a correctness risk
  (an unresolved UFCS is a clean compile error, never a silent divergence).
- **Reversal cost if you change it:** medium (loader would need to resolve UFCS heads cross-package).
- **Status:** ⏳ AWAITING CONFIRMATION

<!-- TEMPLATE for each fork — copy below the line:

### F-NNN — <short title>
- **Slice / context:** <which slice, what I was building>
- **The fork:** <the genuine decision the plan didn't resolve, stated neutrally>
- **Options considered:**
  - A) <option> — <tradeoff>
  - B) <option> — <tradeoff>
- **My provisional call:** <which + why, tied to the decision rule above>
- **Byte-identity / Op impact:** <none / new Op / etc.>
- **Touched commit(s):** `<sha>` (`<subject>`)
- **Reversal cost if you change it:** <low/medium/high + what changes>
- **Status:** ⏳ AWAITING CONFIRMATION

-->

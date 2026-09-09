# scout as phorj's forcing function — lift, transpile, LSP, speed

> **Developer directive, 2026-09-07:** *"explore /stack/projects/scout … use it to
> test/validate/certify/review/audit/modify/enrich/improve phorj in a way that i will be able to do a
> phorj version of it that does exactly what the php version does but much more interesting! and
> faster!"* — and, mid-session: *"include everything! the phorj version must beat php in everything
> absolutely! every lifted or transpiled thing must! this is the perfect playground to validate
> everything we implemented in phorj!"*
>
> **scout is READ-ONLY.** Nothing in this campaign writes to `/stack/projects/scout`. It is a frozen
> yardstick: 274 PHP files / 80 035 lines (120 source files / ~29 k lines excluding tests and tools),
> PHP 8.5, zero composer runtime dependencies, 1 272 tests, live against eight real sources. A
> measurement never has to ask whether the thing being measured moved.
>
> This plan is the record of truth (Invariant 19). It supersedes nothing; it EXTENDS
> `docs/plans/2026-09-02-php-parity-readiness.plan.md` (whose steps 13 and 14 are hoisted here) and
> continues `docs/plans/2026-09-05-big-wave-2.plan.md` Lane R.

---

## Decisions Log

- [2026-09-09 11:43] AGREED: **DEC-516 — the microbench baseline is RE-EMITTED on dockerised
  `php:8.5-cli`+JIT, and the gate REFUSES a debug PHP.** Found by a full rule-compliance review:
  `bench/micro-baseline.json._baseline_php` names a phpbrew `php-8.5.8` that no longer exists on this
  box, `--emit` rewrites all 53 rows in one run, and the gate's fallback accepts the surviving
  `ZTS DEBUG GCOV` oracle because it JITs — the exact build DEC-507 calls invalid for a perf claim.
  Supersedes DEC-423.1's local-php baseline. Needs a quiet box with docker.
- [2026-09-09 14:45] BUILT: **DEC-516 — the baseline is re-emitted on dockerised `php:8.5-cli`, and the gate
  now refuses both a DEBUG php and a cross-source comparison.** Measured on a quiet box (load 0.63,
  core 7 at 96.99% idle, 0.00% nice), K=3 — the estimator every future gate run uses. `_baseline_php`
  = `docker php:8.5-cli`, plus `_baseline_php_build` (`PHP 8.5.10 (cli) … (NTS)`) and
  `_baseline_php_digest` (`sha256:9ebdf4c2…`) so a moving tag can never again leave a baseline
  unreproducible. 56 features, 10 OWED. **The finding sharpened while building it:** the DEBUG-fallback
  hole is LATENT (its banner appears 0 times in the push logs — no recorded run ever used that oracle);
  what was LIVE on every push is docker-measured runs compared against the phpbrew-recorded baseline.
  A source-consistency check was added for that (disclosed scope extension — Rule 14: the re-emit fixes
  the instance, the check fixes the class). ZTS warns rather than refuses, stated as a choice.
- [2026-09-09 11:43] AGREED: **DEC-517 — Invariant 17's 100% diagnostics rule is REOPENED; fixture the
  four codes and correct the ledger.** The surface ratchet reports 317/321 (98%) and says so out loud;
  the four unasserted codes are all of L2's — `E-ORDER-DECIMAL`, `E-ORDER-OPERANDS`,
  `E-ORDER-TUPLE-SHAPE`, `E-SPACESHIP-OPERANDS` — while SLICE-STATE still says the rule is CLOSED and
  L4's row reads `done` with its LSP references / document symbols / signature help unprobed.
- [2026-09-09 11:43] AGREED: **DEC-518 — the rule-drift pass, all four items, one docs-only commit**,
  ordered by operational bite: purge the retired DEC-387 plain-text protocol from MASTER-PLAN (the
  `never push` half of this item was found ALREADY annotated as superseded and is WITHDRAWN — row 12);
  ratify the certification tier; PREFIX `docs/INVARIANTS.md` as `T-1`…`T-14`
  (amended from renumbering, so nothing existing is renamed); restate Invariant 13 as the ratchet the
  size gate actually enforces (56 grandfathered over the hard cap).
- [2026-09-09 11:43] AGREED: **DEC-519 — L4c's root-cause phase is WIDENED to the carried micro
  losses.** Rule 14 unchanged; the reproduce step now measures `fslines` / `fsforeachline` /
  `queryparse` / `strappend` alongside the `nestedlist` pair BEFORE the hypothesis is formed, because
  they are element-access-heavy loops over the same shape. Lane order unchanged: L4c → L4b → L3.
- ⚠ **Provenance correction (2026-09-09 11:55, read from `date` — see the second half).** The two entries below were stamped `[2026-09-09
  14:05]`, a time that had not yet occurred when they were written: the commit that recorded them,
  `3d496a0e`, landed at **07:18** [Verified: `git log -1 --date=format 3d496a0e`]. The rulings
  themselves are genuine — the developer answered them interactively — but the timestamp was not, and
  Rule 17's resume check exists for exactly this. Both are re-stamped `07:18`.
  **And the four entries above initially repeated the defect** — stamped `11:52` in the very commit
  that fixed it, when the last measured `date` was **11:43** and the status-block collector run that
  FOLLOWED the edit reported `11:45`. Re-stamped `11:43`. The lesson is narrow and now written down:
  a Decisions Log stamp is READ from `date` at write time, never estimated from context.

- [2026-09-09 07:18] AGREED: **DEC-514 — the nested-read cliff is FIXED BEFORE L3, root-cause first.**
  The DEC-507 escalation was posed with the re-measured pair (`namedtuplefield` 0.25×, erased-form
  control `nestedlist` 0.03×, VM legs 0.4% apart) and the developer ruled a new lane **L4c** on
  phorj's two-level list element read — 370 ns/iter against 4.3 one level, unexplained, not a deep
  copy, JIT-resistant. **L3 does not start until it closes.** This DEPARTS from the DEC-513 precedent
  deliberately: that residue was understood and attributed, this cliff is not, and L3's own oracle
  benches the exact `list<array{…}>` shape it appears on.
- [2026-09-09 07:18] AGREED: **DEC-515 — the lift seed is extended NOW, before L3.** The wall fell and
  the ceiling did not move: the rewrite is param-seeded and covers 0 of the 140 rewritable reads.
  Lane **L4b** extends it to `@var` locals and `foreach` binders over a keyed collection (reads) and
  to a keyed array literal in a named-tuple return position (writes), so `Classification.phg` — whose
  `toArray()` IS L3's four-leg contract — checks. No new language ruling; DEC-166 untouched, because a
  declared element type is not an invented one.
- [2026-09-08 L4-BUILT] L4 (DEC-504 named-field tuples) is BUILT — three commits: the core language
  slice, the lift leg + refusals, and format/LSP/editors. **NEXT: L4c then L4b, and L3 is BLOCKED
  behind both** (DEC-514 + DEC-515, ruled 2026-09-09 — see the two entries above). **The wall FELL,
  measured**
  [Verified 2026-09-08, `target/release/phg lift /stack/projects/scout/src/php`]: **57/123 → 64/125**,
  **zero `array{…}`-shape refusals left** (was 16 files), and `Rent/Core/Classification.php` — the
  four-leg contract file L3 is blocked on — lifts, with
  `toArray(): (tenure: string, confidence_bp: int, outcome: string, reasons: List<string>)`. It does
  not yet CHECK: the body still returns the keyed array literal, so the lifter's keyed rewrite is
  short on the WRITE side exactly as the census found it short on the 140 READ sites.
  Three more things a reader must carry forward. (1) **The Invariant-7 trap had THREE consumers,
  not the one the register predicted**: the VM compiler, the TRANSPILER — where it picked the wrong PHP
  *operator*, emitting `$t[1] . 1` and printing `31` where both native legs print `4` — and INFERRED
  locals, which needed `ty_to_ast_type`'s tuple arm fixed (it mapped EVERY `Ty::Tuple` to
  `Type::Erased`, one arm feeding pipe params, destructure binders and foreach binders alike).
  (2) **Writing the Invariant-9 example is what caught `phg format` silently STRIPPING tuple labels**
  — rewriting `(bp: int, source: string)` to `(int, string)` and `(only: 7)` to `(7)` — which is the
  argument for the invariant, not a tax it charges. (3) **The perf verdict is OWED and is NOT the
  feature's**, and the first numbers for it were WRONG in method: one unpinned run per side, on a
  bench whose own harness reports 284% spread. Re-measured at K=7, interleaved and core-pinned:
  `namedtuplefield` **0.25×**, its erased-form control `nestedlist` **0.03×**, VM legs 0.4% apart —
  so named tuples cost ZERO over their erased form, and the loss is phorj's TWO-LEVEL list element
  read (370 ns/iter against 4.3 one level deep). `nestedlist` now SHIPS as a permanent paired
  control, because a claim that rests on a number nobody can re-run is not evidence.
  (4) **The lift ceiling was never counted, and counting it changed the story**: 140 rewritable
  reads across 16 files (not the 1,324 recorded), and the shipped param-seeded rewrite covers **0**
  of them — scout's keyed shapes are COLLECTIONS that get iterated, and it is the foreach BINDER
  that is indexed. Closing the wall is what L4 delivered; the read-rewrite reaches nothing here yet.
  Recorded per DEC-365 — both benches ship UNPROTECTED (not in `micro-baseline.json`), `--emit` NOT
  run, `_owed` NOT hand-edited (it is a derived field).
  **This is a DEC-507 escalation and awaits the developer's ruling**, exactly as L2b's did.

- [2026-09-08 L4-SHAPE] AGREED (developer adjudication, Invariant 15 — asked as ONE question with the
  minimal refused scout shape embedded, before any code): named-field tuples are **order-significant**,
  **positional on the PHP leg**, read via **`t.bp`** with positional destructuring retained, and
  **mixed labelling + mutation refused** while **ordering extends**. Full wording in the DEC-504 register
  row. The four answers interlock: order-significance is what makes positional erasure well-defined, and
  the two together are what make extending `<=>` sound under DEC-512's static-arity argument. Because the
  PHP leg is positional, the interpreter, VM and JIT need no changes at all — erasure does the work — which
  is what keeps a 73-site wall tractable as one slice.

- [2026-09-08 L2b-OWED] AGREED (developer adjudication, escalated per DEC-507): **carry the residual
  `spaceshipsort` loss as a DEC-365 OWED and proceed to L4.** L2b met its structural goal — no tuple
  is materialized, proven by disassembly — but the bar stayed missed: 0.38x -> 0.46x, VM time -19.5%,
  still ~2.2x php (the pre-push microbench-gate independently measured 0.471). The escalation offered
  L2c (a `CmpSeq` specialized for checker-known scalar element types, which is where the residue
  actually is) and declaring 0.46x the accepted floor; the developer ruled OWED + L4, so **L2c is NOT
  authorized and no further work starts on this bench**. Two things stay true and unprotected, and are
  recorded rather than fixed: `spaceshipsort` is NOT in the microbench baseline, so nothing catches
  this loss DEEPENING, and DEC-365 forbids the `--emit` that would admit it. Reopening this is a new
  lane and a new ruling.

- [2026-09-08 L2b-SCOPE] AGREED (Claude-level scope call, not a developer adjudication): the DEC-513
  fusion keys on the **literal tuple shape on BOTH sides**, not on the checked tuple type. The
  allocation DEC-513 removes happens where a tuple is BUILT, not where it is compared — in
  `var lo = (a, b); lo < hi` both tuples are already on the heap before the comparison runs, so
  fusing there would remove a dispatch, not the ~65% materialization. Widening to the checked type
  is therefore a DIFFERENT optimization (scalar replacement at the binding), not a wider version of
  this one, and it is out of this lane. The narrow form needs no AST field and no checker threading.
  Recorded because the alternative was explicitly weighed and rejected, not overlooked.

- [2026-09-07 §1] AGREED: **the 2026-09-02 16:40 "no scout port" ruling STANDS.** phorj's job is to be
  READY; the developer implements the phorj scout himself later. scout is used *"for experiment and
  validation of lift/transpile/lsp/speed … because it works"* — so the validated surface is four, not
  two: **lift, transpile, LSP, speed**.
- [2026-09-07 §2] AGREED: **lifted scout `.phg` lives in `examples/lift/scout/` IN PHORJ.**
  `tests/differential.rs` globs `examples/**/*.phg`, so every lifted file is held to three-leg
  byte-identity permanently. Satisfies Invariant 9. Nothing lands in scout's tree.
- [2026-09-07 §3] AGREED: **depth first, then breadth.** The depth target is the tenure classifier
  cluster driven by `tests/fixtures/rent/tenure/corpus.json` (130 hand-labelled cases, deliberately
  language-neutral). Breadth (whole-tree lift %) follows.
- [2026-09-07 §4] AGREED: **scout's two hard blockers are HOISTED** above the readiness plan's ruled
  order. New order: HTML5 parse + selectors + entity decode (readiness step 13) → `Core.Net` +
  `Core.Mime` + read-only `Core.Imap` (step 14) → then back to the ruled sequence (tz, `.env`/process,
  JSON, HTTP cookies, crypto, money, Intl). Rationale: these two are the only items that make a phorj
  scout POSSIBLE rather than merely nicer.
- [2026-09-07 §5] AGREED — **DEC-504 NAMED-FIELD TUPLES**: build a **structural** named-field tuple
  type, `(tenure: Tenure, source: string, bp: int)`, with field access `t.tenure`. It is the phorj
  form of PHP's keyed `array{…}` shape — 73 uses across scout, the single largest lift wall (16 files
  refused on it). Chosen over lifter-synthesised classes because the lifter must not invent names
  (DEC-166). Structural, not nominal — note the PHP `value class` RFC (readiness §5) is nominal; that
  is a different thing and no phorj ruling collides.
- [2026-09-07 §6] AGREED — **DEC-505 SPACESHIP + ORDERABLE TUPLES**: `a <=> b` yields `int` (-1/0/1)
  for any two operands of one comparable type, AND tuples/lists compare lexicographically element-wise.
  Both halves are missing today (`1 <=> 2` → parse error; `(1,2) < (1,3)` → *"comparison requires
  matching int or float operands"*). Transpiles to PHP's own `<=>` so the leg stays byte-identical.
  `compare_ord` already exists in `src/value/` — this is surface plus tuple ordering, not a new kernel.
- [2026-09-07 §7] AGREED — **DEC-506 BY-REF CAPTURE STAYS REJECTED, AND IS NAMED**: `use (&$x)`
  contradicts the value/handle split (UNIFIED-SPEC:1619) and is not being un-ruled. The lifter learns
  block-bodied closures (mechanical, 23 sites) but emits a SPECIFIC diagnostic for by-ref capture,
  naming the rejected feature and its ruling, instead of today's generic `expected ")"`. The five real
  sites are hand-ported where needed and recorded as documented lift exceptions.
- [2026-09-07 §8] AGREED: **scout is READ-ONLY.** No source edits, no docblock additions to raise
  liftability, no bug fixes. Findings about scout are reported to the developer as text.
- [2026-09-07 §9] AGREED — **DEC-507 THE ABSOLUTE PERF BAR, WITH ESCALATION**: every lifted or
  transpiled artifact is benched against PHP; any ratio ≤ 1.0× must be FLIPPED before its slice
  closes; a loss that cannot be flipped **STOPS the lane and is escalated to the developer with the
  measurement**, never logged as an OWED and passed. **This SUPERSEDES the 2026-07-10 refinement**
  (*"MATCHES-not-beats php on 20-yr-tuned string/array/collection"*) for the whole of this campaign.
  Invariant 18's NO-HIDDEN-LOSS is tightened, not loosened: an unmeasurable bench is still an OWED
  verdict and is still never reported as passed.
- [2026-09-07 §10] AGREED — **THE PERF BASELINE IS DOCKERISED PHP WITH JIT ON**, core-pinned and
  interleaved: `docker run --cpuset-cpus=7 php:8.5-cli` with `opcache.enable_cli=1`,
  `opcache.jit=tracing`; the phorj leg `taskset -c 7`; samples interleaved. **The on-box gate oracle
  is `PHP 8.5.9 (cli) (ZTS DEBUG GCOV)`** — a debug build with coverage instrumentation. It stays the
  CORRECTNESS oracle and is INVALID for any perf claim. Two PHPs, two jobs, never confused.
- [2026-09-07 §11] AGREED: **one long autonomous wave.** Lanes land as individual green commits and
  pushes; the developer is interrupted only for an Invariant-15 language ruling or a perf loss that
  cannot be flipped. `/progress` is the check-in.
- [2026-09-07 §12] AGREED — **STOP CONDITION**: every scout file **either lifts or carries a NAMED
  refusal with a DEC row** (N + M = 121); `phg check` clean as a project over what lifts; three-leg
  byte-identity on everything pure; every benched scenario > 1.0× against dockerised PHP; the LSP and
  both editors carry every construct added. Anything genuinely impossible — the SQLite, SMTP and IMAP
  legs cannot transpile by design — is an Invariant-14 LADDER disclosure, named, never silent.
  **Wording note (2026-09-07):** the developer chose "every scout file lifts, checks, agrees and
  wins"; under §8 (read-only) and DEC-166 (the lifter never guesses) roughly 30 of the 69 refusing
  files have no lift path that does not either edit scout or make the lifter guess. The option's own
  text carries the escape and it is made explicit here rather than discovered later.

---

- [2026-09-07 §12] AGREED — **DEC-510**: PHP array destructuring is UN-DEFERRED. `[$a,$b] = e;` and
  `list($a,$b) = e;` both lift to `var (a, b) = e;`. A KEYED destructure is refused by name.
- [2026-09-07 §13] AGREED — **DEC-511**: PHP's `.` lifts to ONE interpolation, not `+`. A chain
  flattens; arithmetic `+` is untouched.
- [2026-09-07 §14] CLAUDE-LEVEL, overrulable: two more lift-fidelity calls landed with L1c, both
  found by RUNNING the example rather than reading the lifter, and both the same
  "two halves of one draft disagree" class DEC-511 fixed. (a) `echo $x;` of a BARE VARIABLE is now
  an interpolation `Output.print("{x}")`: the previous carve-out kept it bare on the grounds that a
  string variable is the common case, which is a GUESS about a type the lifter cannot see, and wrong
  on the int a lifted positional shape produces. Wrapping is correct for BOTH. (b) a STRICT `$x ===
  null` (either ordering) becomes phorj's `is null` narrowing test, because the checker rejects
  `T? == null` as a cross-type comparison; PHP's LOOSE `== null` is a different question (also true
  for `0`, `""`, `[]`, `false`) and is deliberately left as an equality for the checker to report.

- [2026-09-08 §16] AGREED — **DEC-512, ordering is a TUPLE capability; `List<T>` is not orderable**
  (amends DEC-505). DEC-505's two clauses — lexicographic element-wise, AND a transpile leg emitting
  PHP's own `<=>` — are mutually exclusive for lists, because **PHP's array `<=>` is count-first**
  (`[2] <=> [1,1]` is `-1`, not `+1`; `[9] <=> [1,1,1]` is `-1`, not `+1` — Verified on the gate
  oracle). Tuples are immune: static arity ⇒ equal lengths ⇒ the two orderings coincide. So tuples
  order and transpile to PHP's own operator with no divergence; `List<T>` ordering is a checker error
  naming the ruling; and `phg lift` maps a positional array literal in an ordering-operand position
  to a tuple, which is DEC-166-safe because a literal's arity is syntactically present — that is what
  makes the COMPARATOR on `Classification.php:48` liftable, i.e. the operator on that line and not the
  file: `usort` has no lift mapping at all [Verified 09-08: zero hits in `src/lift/`], so the
  enclosing call still refuses. NaN pinned alongside: PHP gives `1` for every NaN
  comparison, so `compare_ord`'s `Ok(None)` projects to `1` at all three call sites. Full row +
  rejected alternatives: register DEC-512; spec § `DEC-512` under `Ordering, <=>, and named-field
  tuples`.
- [2026-09-08 §15] CLAUDE-LEVEL, overrulable: **the lane order carried a dependency defect, found by
  re-censusing rather than by reading the plan.** L3 (depth oracle) was ordered before L4, but
  `Classification::toArray()` — the four-leg contract L3 diffs against — refuses on the very keyed
  shape L4 builds, so L3 could never have run first. Recorded in `Blocked` and in §1's re-census
  table; the status block's numbering is left alone so evidence shas stay attached to their rows.
  The general lesson is the one §4 already states and this session still had to relearn: a
  first-error-per-file census is a snapshot of WALLS, not of work, and every lane that claims a
  critical path must be re-derived from a fresh run, not from the last one's table.

- [2026-09-08 DEC-513] AGREED: **the DEC-507 loss on `<=>` is FIXED, not carried — by FUSING the
  comparison in the compiler.** `(a, b) <=> (c, d)` lowers element-wise so no tuple is materialized;
  it reuses `value::compare_ord`/`three_way` (Invariant 4 holds) and leaves the transpile leg emitting
  PHP's own `<=>` untouched. Measured cause: tuple `<=>` 169 ms vs scalar `<=>` 60 ms vs subtraction
  56 ms on one pinned core, so ~65% of the cost is tuple CONSTRUCTION, not `Op::Cmp`. Builds as **L2b,
  BEFORE L4.** Rejected: JIT-whitelisting (speeds the allocation rather than removing it) and carrying
  it OWED to L8 (nothing stops the loss deepening, and the fix is already located).

## 1. Measured starting state (2026-09-07, on `target/release/phg` at `6c49816d`)

`phg lift /stack/projects/scout/src/php -o <out>` — a whole-tree pass that writes `LIFT-REPORT.md`
naming every refusal. **This is the durable census** and it replaces the lost scratchpad
`raw/scout-needs.md` (52 rows) the readiness plan calls its yardstick. It is re-runnable by anyone.

**54 of 123 files lift. 69 refuse.** Refusal histogram — **first error per file only**, so fixing a
wall exposes the next one; the breadth lane re-censuses after every lifter change rather than working
a fixed list of 69:

| count | reason | disposition |
|---|---|---|
| 16 | `array{…}` shape has no phorj type | 73 KEYED → DEC-504 named tuples · 12 POSITIONAL → tuples, mechanical |
| 12 | bare `array` needs List/Map/Set inference | no path under §8 — banked question |
| 9 | `mixed` is Tier-2/3 | banked question |
| 5 | expected an expression | this is `<=>` → DEC-505 |
| 3 | `self` outside a class body | enum bodies — mechanical (R-8 extension) |
| 3 | invalid assignment target | investigate in L1 |
| 2 | `(object)` cast | banked question |
| 2 | backtick shell execution | correctly refused (Tier-3), LADDER row |
| 1 each | block closure · `clone` · non-literal `const` · non-literal `match` arm · catch class expr · `foreach` destructuring · … | L1 / banked |

Not in the histogram but present in the tree: **`yield` in 23 files** (DEC-479 generators RULED,
build QUEUED) — it is masked behind an earlier refusal in every one of them.

**The depth target is blocked by exactly four things.** `TenureClassifier` (1 875 lines) needs
`Tenure`, `TenureSignal`, `Classification` and `Text`; only `TenureSignal` lifts today:

| file | refusal today | what it is | scout-wide |
|---|---|---|---|
| `Rent/Core/Tenure.php:70` | `self` outside a class body | `self::PLAI` in an **enum** method | 3 files |
| `Rent/Core/Classification.php:47` | expected an expression, found `>` | `[$a->tier,$a->position] <=> [$b->tier,$b->position]` | 6 |
| `Rent/Core/TenureClassifier.php:344` | expected `)` | block closure **+ `use (&$doubts)`** | 23 block / 5 by-ref |
| `Core/Text.php:293` | `array{…}` shape | `@return array{int, string}\|null` — **positional**, a tuple | 12 positional |

### Re-census 2026-09-08 (at `78317867`) — the cluster MOVED, and the lane order was wrong

L1a/L1b/L1c changed four of those five rows. Re-run, never re-quote — this is first-error-per-file:

| file | 09-07 refusal | 09-08 refusal | lane |
|---|---|---|---|
| `Rent/Core/Tenure.php` | `self` outside a class body | **LIFTS** | closed by L1a |
| `Rent/Core/TenureSignal.php` | lifts | **LIFTS** | — |
| `Rent/Core/Classification.php` | `<=>` (`:48`) | **keyed `array{…}`** (`:57`) | **L4**, then L2 — it needs BOTH |
| `Rent/Core/TenureClassifier.php` | `expected )` | **named by-ref refusal** (`:344`, 1 site) | L1b closed it; hand-port per §3 |
| `Core/Text.php` | positional shape | **`?? throw`** (`:426`) | banked, below |

**L3 IS BLOCKED ON L4 — a dependency the lane list got backwards.** `Classification.php:57` is
`toArray()`'s keyed shape, and `toArray()` *is* the four-leg contract §2 diffs against. Under §8
(read-only) and DEC-166 (never guess) the lifter cannot produce that file until named-field tuples
exist. The depth path is **L2 + L4 → L3**, not L2 → L3 → L4. The status block's `#` order is
retained for row stability; the `Blocked` section carries the truth.

**Whole-tree headline unchanged at 57/123** — the cluster's walls moved without moving the count,
which is exactly what first-error-per-file does and why §4 forbids quoting the histogram as a
worklist.

### Re-census after L2 (`eba09d4e`, 2026-09-08) — headline STILL 57/123, and that is the result

Run read-only over the same 123 files with the L2 binary. **The count did not move**, and the
histogram says exactly why:

| Wall class | after L1c | after L2 | what moved |
|---|---|---|---|
| keyed `array{…}` (DEC-504 → **L4**) | 15 | **16** | `Rent/Core/Classification.php` joined it |
| bare `array` needs a docblock | 14 | 14 | — |
| `mixed` | ~12 | 12 | — |
| `expected an expression` | 5 | **3** | the three `<=>` members are GONE; all 3 survivors are `found Dot`, i.e. PHP spread (Q-0908-2) |
| by-ref capture (DEC-506) | 3 | 3 | named refusals, hand-port |

**The single most useful number here is `Classification.php`'s wall: line 48 → line 68.** L2 cleared
the `<=>` on line 48 — the operator this lane existed for — and the file's first error is now the
KEYED `array{…}` shape on line 68, which is L4's. So L2 delivered exactly what it claimed and moved
zero files, because every file blocked on `<=>` was also blocked on something later. That is
first-error-per-file behaving as §4 describes, and it independently re-confirms the `Blocked`
section: the depth path is **L2 + L4 → L3**.

`usort` did NOT become the next wall, which is worth stating because it was the expected answer: the
docblock shape is reached first. The `usort` gap is real all the same — `phg lift` has no mapping for
it at all [Verified 09-08: zero hits for `usort` under `src/lift/`] — and it is an L7 row rather than
a question, since the honest lowering (`x = List.sortWith(x, f)` for an in-place by-ref sort) needs a
statement rewrite and a mutable binding, not a ruling.

**Answered from the tree, not from the developer** — `/stack/projects/scout/docs/PHORJ-REQUIREMENTS.md`
is stale and its four open questions now resolve as: ③ `sleep` **SHIPPED** (`d0005b65`); Q1 request
headers **YES** (`HttpClient.send(method, url, headerNames, headerValues, body)`); Q2 timeout **YES**
(`.timeout(ms)`, default 30 000); Q3 cookies **NO** (documented "NOT in v1", readiness step 15 /
DEC-266). Its accent-folding section is superseded by `String.foldAccents` (`2b987f15`). scout is
read-only, so this correction lives here and is reported to the developer, not written there.

---

## 2. The depth oracle is FOUR legs, not three

`tests/differential.rs` proves interpreter ≡ VM ≡ transpiled-PHP over `examples/**/*.phg`. It never
touches scout's ORIGINAL PHP — but "the lifted phorj agrees with scout" is precisely the fourth leg,
and it is the claim that matters.

scout built the contract for it. `Rent/Core/Classification::toArray()` is documented *"stable
structure for the cross-language differential test"* and yields
`{tenure: string, confidence_bp: int, outcome: string, reasons: list<string>}`.

The harness: a read-only PHP driver (living in phorj, run against scout's tree, never committed into
scout) emits that structure for each of the 130 corpus cases → the lifted phorj emits the same → byte
diff. The existing three-leg harness then covers the lifted `.phg` on its own.

**`docs/PHORJ-REQUIREMENTS.md` §"The classifier's cross-implementation contract" is the spec** — eleven
properties. Two will bite first: **property 3** (both folded surfaces must agree byte for byte, which
holds only if lowercasing cannot change byte length — ASCII-only, never a Unicode lowercaser) and
**property 5** (a newline is a phrase boundary in five separate rules; PCRE's `$` matches before a
final newline, so narrowing a character class does nothing until the anchor is `\z`).

**Verify before trusting the diff:** phorj's `List.sort` must be STABLE. PHP 8's `usort` is, and the
`[tier, position]` tie is exactly where byte-identity would break silently.

---

## 3. Lanes

**L0 — Invariant 19 docs commit, BEFORE any code.** DEC-504…507 + the nine other rulings into
`docs/research/full-audit/raw/C-decisions.md`; the explicit supersession of the 2026-07-10
"MATCHES-not-beats" refinement; readiness plan Decisions Log + status block re-ordered for the 13/14
hoist; UNIFIED-SPEC amendments (`<=>`, tuple ordering, named-field tuples as QUEUED); SLICE-STATE
cursor; MASTER-PLAN §0 mirror rows.

**L1 — lifter mechanicals + re-census.** enum `self` · block-bodied closures · positional `array{…}`
→ tuples · a NAMED by-ref diagnostic (DEC-506) · the three `invalid assignment target` sites. Re-run
the whole-tree census after each; the delta is the evidence.

**L2 — DEC-505 `<=>` + orderable tuples.** Full slice: parser, checker, interpreter, VM, JIT
eligibility, transpile to PHP's own `<=>`, lift mapping, `phg format`, LSP + both editors, Invariant-9
example, flip-or-flag bench. Smaller than L4 and on the depth critical path, so it goes first.

**L3 — the depth oracle.** Lift the classifier cluster into `examples/lift/scout/`, hand-port the one
by-ref method as a documented exception, build the four-leg harness, drive all 130 corpus cases to
byte-identity, then bench the classifier scenario against dockerised PHP under DEC-507.

**L4 — DEC-504 named-field tuples.** The 73-site wall. Full slice discipline as L2.

**L5 — HTML5 parse + CSS selectors + entity decode** (readiness step 13, DEC-469, `html5ever` +
`selectors` already admitted). scout blocker #2.

**L6 — `Core.Net` + `Core.Mime` + read-only `Core.Imap`** (readiness step 14, DEC-467), including the
file-backed `.eml` transport `PHORJ-REQUIREMENTS` marks *"do not skip"* — without it every alert-parser
test needs a live mailbox. scout blocker #1.

**L7 — breadth census loop.** Re-census → take the largest wall → mechanical fix or banked
Invariant-15 question → repeat, until every one of the 121 files lifts or carries a named refusal with
a DEC row.

**L8 — perf twins.** A benched scenario per lifted module, driven by **scout's own fixtures**
(Invariant 18's "I/O modules via fixtures") rather than "a module" — a library has no entry point.
DEC-507 applies to every one.

## 4. Blast radius / risks

- **A perf loss stops a lane** (DEC-507). Budget for it: the string/collection primitives the 2026-07-10
  refinement carved out are exactly where scout's classifier spends its time.
- **The census is first-error-only** — never quote "69 files, N reasons" as a worklist.
- scout is **public and has three committed-then-scrubbed leak incidents** (its hard rule 7). No scout
  fixture is copied into phorj's tree without checking scrub status first; `.eml` fixtures especially.
- `examples/` is outside `scripts/size-gate.sh` (it walks `src/**/*.rs`), so a 1 875-line lifted `.phg`
  does not trip Invariant 13. Verified 2026-09-07.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------| ------- |
| 1 | L0 — Invariant 19 docs: DEC-504…508 + the 2026-07-10 perf-refinement supersession, readiness re-order, SPEC § `Ordering, <=>, and named-field tuples`, SLICE-STATE cursor, MASTER-PLAN §0.08 mirror | M | done | e42e9a9a | docs/research/full-audit/raw/C-decisions.md docs/plans/2026-09-07-scout-forcing-function.plan.md docs/specs/UNIFIED-SPEC.md |
| 2 | L1a — enums (DEC-509): a case CONSTRUCTS its variant, `self` inside an enum body is the enum, a method LOWERS to a free function reached by UFCS, static call sites lower with it; 54 → **57/123** — `feat(lift): PHP enums` | M | done | 9bd0a43e | src/lift/lifter/enums.rs src/lift/parser/enums.rs src/lift/lifter_tests_enums.rs examples/lift/enums.php examples/lift/enums.phg |
| 2b | L1b — block-bodied closures lift; `use (&$x)` refused BY NAME (DEC-506); `static` and by-value `use` dropped. Headline stays 57/123 and that is the finding: the histogram is FIRST-ERROR-per-file, so the closure files fell through to their next wall (`array` 12 → 14, a new `Closure`-type row) | M | done | f9e26192 | src/lift/parser/closures.rs src/lift/printer/lambda.rs src/lift/lifter_tests_closures.rs examples/lift/closures.php examples/lift/closures.phg |
| 2c | L1c — positional `array{…}` → tuples (type AND the returned literal, at any depth), array destructuring (DEC-510), `.` → one interpolation (DEC-511), `echo $var` → interpolation, strict `=== null` → `is null`. Headline holds at **57/123** (denominator corrected 09-08; "125" was a typo) — first-error-per-file again: the keyed-shape row rises 14 → 15, and the `expected an expression` row stays at 5 but only **3** of those are `<=>` — the other 2 are PHP spread `...`, found 09-08 | M | done | 78317867 | src/lift/lifter/decls/seed.rs src/lift/lifter/decls/statements.rs src/lift/lifter/exprs.rs src/lift/parser/docblock.rs src/lift/parser/exprs.rs src/lift/lifter_tests_shapes.rs examples/lift/shapes.php examples/lift/shapes.phg |
| 3 | L2 — DEC-505 as amended by DEC-512 — ``feat(lang): `<=>` and tuple ordering`` — `Op::Cmp` + tuple `< > <= >=` on all three legs, `List<T>` refused, `decimal` excluded pending Q-0908-3; lift + LSP + 5 `phg explain` entries. Closed by row 3b | L | done | eba09d4e | src/tokenizer/mod.rs src/parser/exprs/climb.rs src/checker/expr/ordering.rs src/value/collections.rs src/vm/exec.rs src/compiler/cty.rs src/transpile/expr.rs src/lift/lifter/exprs.rs tests/differential.rs |
| 3b | L2 closes — `docs(lang): L2 closes` — SSOT quartet amended (the decimal narrowing never reached the SPEC/register/MASTER-PLAN in C1), the missing lift-rule tests with two mutations verified red and a third proving `positional` redundant, the Invariant-9 example, VS Code grammar, KNOWN_ISSUES § DECIMAL-ORDER, the re-census, and the DEC-507 bench — **a CONFIRMED LOSS, escalation OWED** | M | done | ab959542 | docs/specs/UNIFIED-SPEC.md docs/plans/SLICE-STATE.md docs/research/full-audit/raw/C-decisions.md src/lift/lifter_tests_ordering.rs examples/guide/spaceship.phg bench/micro/spaceshipsort.phg bench/micro/spaceshipsort.php editors/vscode/syntaxes/phorj.tmLanguage.json KNOWN_ISSUES.md |
| 3c | L2b — DEC-513: FUSE the tuple comparison in the compiler. `Op::CmpSeq(n, SeqOrd)` pops the `2n` elements and compares them IN PLACE on the operand stack, so a both-sides-literal `(a,b) <=> (c,d)` materializes no tuple; `value::compare_seq` + `project_three_way` extracted so the generic and fused paths share one lexicographic loop and one NaN projection (Invariant 4); interpreter, transpile and lift legs unchanged. MEASURED A/B on one quiet box (K=15, core-pinned, interleaved, both legs proven by disassembly): 0.38x -> 0.46x, VM time -19.5%, STILL A LOSS at ~2.2x php -> OWED per DEC-365. The ruling's ~65% prediction did not hold: construction was ~a fifth of the tuple-vs-scalar gap, and the residue is per-element `compare_ord` dispatch inside the fused Op (compiler-reachable), NOT the `sortWith` callback | L | done | d4365564 | src/chunk/op.rs src/chunk/mod.rs src/chunk/validate.rs src/compiler/emit.rs src/compiler/mod.rs src/compiler/expr/binary.rs src/value/collections.rs src/vm/cmp_seq.rs src/vm/mod.rs src/vm/exec.rs src/jit/tests/tuple_ordering.rs examples/guide/spaceship.phg examples/README.md |
| 4 | L3 — depth oracle: classifier cluster lifted, four-leg harness over the 130-case corpus, byte-identity, docker-PHP bench. **BLOCKED on rows 5b and 5c** (DEC-514 + DEC-515, 2026-09-09) | L | blocked | - | examples/lift/scout/* tests/* bench/* |
| 5 | L4 — DEC-504 named-field tuples (73 sites), all legs + LSP + editors + example + bench. Wall-fall MEASURED on scout: 57/123 → 64/125, zero keyed-shape refusals, `Classification.php` lifts | L | done | 9b3e528c | src/ast/* src/checker/* src/interpreter/* src/vm/* src/transpile/* src/lift/* src/lsp/* editors/* |
| 5b | L4c — DEC-514: phorj's TWO-LEVEL list element read, `nestedlist` 0.03× vs docker php:8.5+JIT and 370 ns/iter against `listindex` 4.3. ROOT-CAUSE FIRST (Rule 14) — no fix until the cliff is explained with measured evidence; not a deep copy, JIT-resistant. **WIDENED by DEC-519**: the reproduce step also measures the carried OWED losses `fslines` 0.118 / `queryparse` 0.225 / `fsforeachline` 0.318 / `strappend` 0.436 as corroborating evidence BEFORE the hypothesis, since they are element-access-heavy loops over the same shape. Blocks L3 | L | todo | - | src/vm/* src/jit/* src/value/* bench/micro/nestedlist.phg bench/micro/fslines.phg bench/micro/queryparse.phg |
| 5c | L4b — DEC-515: extend the lift tuple-field seed past PARAMS to `@var` locals and `foreach` binders over a keyed collection (140 reads / 16 files), and lift a keyed array literal in a named-tuple return position to a tuple literal (the write half), so `Classification.phg` CHECKS. Blocks L3 | M | todo | - | src/lift/lifter/decls/declarations.rs src/lift/lifter/decls/seed.rs src/lift/lifter/exprs.rs src/lift/lifter_tests_shapes.rs |
| 6 | L5 — HTML5 parse + CSS selectors + entity decode (readiness 13, DEC-469) | L | todo | - | src/ext/html/* |
| 7 | L6 — `Core.Net` + `Core.Mime` + read-only `Core.Imap` with the file-backed `.eml` transport (readiness 14, DEC-467) | L | todo | - | src/ext/net/* src/ext/mime/* src/ext/imap/* |
| 8 | L7 — breadth census loop to the stop condition: every file lifts or carries a named refusal with a DEC row | L | todo | - | src/lift/* examples/lift/scout/* |
| 9 | L8 — perf twins: a fixture-driven benched scenario per lifted module, DEC-507 bar on every one | L | todo | - | bench/* |
| 10 | DEC-516 — re-emit all 53 microbench rows on dockerised `php:8.5-cli`+JIT, core-pinned and interleaved on a quiet box (supersedes DEC-423.1's local-php baseline, whose binary no longer exists), and add a `PHP_DEBUG` refusal to the gate's resolution chain so a debug build can never become the baseline; ALSO a source-consistency refusal + `_baseline_php_build`/`_baseline_php_digest` provenance (disclosed scope extension) | M | done | 00fc5904 | bench/micro-baseline.json scripts/microbench-gate.sh scripts/microbench.sh |
| 11 | DEC-517 — fixture the four unasserted L2 ordering codes (`E-ORDER-DECIMAL` `E-ORDER-OPERANDS` `E-ORDER-TUPLE-SHAPE` `E-SPACESHIP-OPERANDS`), re-emit the surface-ratchet floor, correct SLICE-STATE's `311/311 CLOSED` claim to the live number, and restate L4's status row so it names the unprobed LSP surfaces | M | todo | - | conformance/* scripts/surface-baseline.txt docs/plans/SLICE-STATE.md |
| 12 | DEC-518 — the rule-drift pass, one docs-only commit — `docs(rules): DEC-516…519 — a full rule-compliance review, and the drift pass lands`. All four landed: MASTER-PLAN's retired DEC-387 protocol replaced by the live rule with the retirement recorded (the two `never push` lines were ALREADY annotated as superseded — that half of the finding was overstated); the certification tier ratified IN the repo; `docs/INVARIANTS.md` prefixed `T-1`..`T-14`; Invariant 13 restated as the ratchet (`grandfathered=56 fails=0 warns=168`) | M | done | 36fa37d4 | docs/plans/MASTER-PLAN.md docs/INVARIANTS.md CLAUDE.md |
<!-- /progress-block -->

### Blocked
- **L3 depends on L4** (found by the 09-08 re-census, §1). `Classification::toArray()` is the
  four-leg contract itself and refuses on its keyed `array{…}` shape, so the depth oracle cannot be
  built before named-field tuples exist. L2 is still required by the same file (`:48`) and is still
  the right next build — smaller, ruled, unblocked — but it does not on its own unblock L3.
- L6 depends on L5 for alert-email parsing (an alert body IS an HTML document).

### Needs input
Banked Invariant-15 questions, to be asked when L7 reaches them (not before — the mechanical fixes in
L1/L2/L4 will expose better-shaped versions of each):
- **bare `array` with no docblock** (12 files) — scout is read-only, so the annotation route is closed.
- **`mixed`** (9 files) — Tier-2/3 today.
- **`(object)` cast** (2 files).
- **backtick shell execution** (2 files) — correctly refused as Tier-3; needs a LADDER row, not a build.
- **`yield` / generators** (23 files) — DEC-479 RULED, build QUEUED; masked behind earlier refusals.
- **Q-W2-1 / Q-W2-2** carried over from `2026-09-05-big-wave-2.plan.md` (cross-package `Color.Green`;
  `Acme\Color.Green` rendering).
- **Q-0908-1 — `?? throw`: PHP 8 throw-as-expression.** phorj's `throw` is `Stmt::Throw`
  (`src/ast/stmts.rs:90`) — statement-only, so `a ?? throw e` has no phorj form. **Exactly ONE site
  scout-wide**, `Core/Text.php:426`, and it is on the depth critical path. Two dispositions, both
  honest, and the choice is the developer's: **(a)** phorj gains a throw-expression (real surface,
  all legs + LSP + editors); **(b)** the lifter LOWERS `return a ?? throw e;` to
  `var t = a; if t is null { throw e; } return t;` — semantics-preserving, no guessing, and the
  statement form already lifts (`src/lift/parser/stmts.rs:18`). (b) is the DEC-511 class and costs a
  fraction of (a); (a) is warranted only if the construct is wanted in hand-written phorj. Refused
  today at `src/lift/parser/exprs.rs:326`. Note first-error-only: `Text.php` may have further walls.
- **Q-0908-2 — PHP spread `...`.** phorj has NO spread of any kind [Verified 09-08: zero hits for
  `spread|Ellipsis|DotDotDot` across `src/ast/` and `src/lexer/`]. Two scout sites, both first-error:
  `Rent/Adapters/HtmlSource.php:397` `[...$out, ...$rows]` (array spread in a literal) and
  `Rent/Core/CriteriaEngine.php:378` `array_unshift($reasons, ...$c->reasons())` (argument
  unpacking). These are two DIFFERENT features — a literal-spread and a call-site unpack — and the
  second interacts with arity checking, so they may be ruled apart. Build it, or refuse it by name
  with a DEC row; do not leave it as today's generic `expected an expression`.
- **Q-0908-3 — `decimal` ordering: keep the divergence, or spend a helper?** Found while building
  L2, by measuring rather than by reading: PHP compares a decimal's numeric-string carrier **as a
  float**, so `9007199254740993.00 <=> 9007199254740992.00` is `0` in PHP and `1` under the exact
  i128 `compare_ord`, and `9007199254740992.00 < 9007199254740993.00` is `true` natively and `false`
  in PHP [Verified 09-08 on `php-8.5.9`]. **Bare `d1 < d2` has always carried this** — it is a
  pre-existing Invariant-1 bug with a known fix, NOT an Invariant-14 no-analog case, so it does not
  join the two disclosed exceptions; it is recorded in `KNOWN_ISSUES.md`. L2 shipped NARROW rather
  than inheriting it: `<=>` and tuple ordering refuse `decimal` by name (`E-ORDER-DECIMAL`). Three
  dispositions: **(a)** emit a `__phorj_dec_cmp` helper on the PHP leg and admit `decimal`
  everywhere, closing the bare-`<` hole too — an Invariant-16 byte-identity trade, which is exactly
  why it is banked and not self-decided; **(b)** keep the refusal and fix bare `<` alone; **(c)**
  disclose and leave. scout does not exercise decimal ordering, so nothing is blocked on it.
- **Q-0908-4 — `<=>` associativity diverges from PHP, and was built that way without a ruling.**
  PHP makes `<=>` **non-associative**: `1 <=> 2 <=> 3` is a parse error [Verified 09-08 on
  `php-8.5.9`]. Phorj's operator fold is uniformly left-associative, so it ACCEPTS that form as
  `(1 <=> 2) <=> 3`, and the transpiler's `paren_if_compound` writes the parentheses back out, so the
  PHP leg stays valid and every leg agrees. Nothing is broken and nothing is blocked — phorj simply
  accepts strictly more than PHP here — but *what the language accepts* is a user-visible decision
  and Invariant 15 makes it the developer's, so it is banked rather than left implied by an
  implementation detail. Found while writing the L2 example, when `phg format` removed the
  parentheses the example had written by hand and the result still ran. **(a)** keep left-associative
  (nothing to build; disclose in FEATURES, done); **(b)** make it non-associative to match PHP
  exactly — a parser change plus a diagnostic, and the `paren_if_compound` path stops being
  load-bearing for this operator. Pinned either way by
  `spaceship_left_assoc_parenthesized_on_php_leg` in `tests/differential.rs`, which would go red on a
  silent flip.
- **Q-L1c-1 — `phg format` normalizes `is null` to `instanceof null`.** Found while lifting
  `shapes.php`: `x is null` and `x instanceof null` are ONE AST node (DEC-184 makes `is` a full
  synonym), and the canonical formatter picks `instanceof`. So a lifted null test renders
  `maybePair(false) instanceof null`, which reads as a mistake — and this is REPO-WIDE, not a lift
  issue: `examples/guide/is-narrowing.phg`, the example whose SUBJECT is `is`, prints
  `name instanceof null` under a comment that says `is null`. Changing the canonical form is a
  `phg format` decision with an idempotency gate behind it, so it is not L1c's to take. Deliberately
  NOT worked around in the lift printer — that would make `lift_source` and `cmd_lift` disagree about
  the same draft, the two-gates-one-artifact trap.

### DEC-507 bench for L2 — a CONFIRMED LOSS, recorded OWED (2026-09-08)

`bench/micro/spaceshipsort.{phg,php}` is the comparator-heavy sort DEC-507 asks for: 1200 rows,
7 tiers so the SECONDARY key decides most comparisons, sorted 40 times by
`(a.tier, a.position) <=> (b.tier, b.position)` — scout's own shape. Both legs check-summed the same
`180744`, so the pair is output-identical and the micro is honest.

**Measured** with `scripts/microbench.sh` (docker `php:8.5-cli`, JIT on, `--cpuset-cpus=7` against
`taskset -c 7`, interleaved, best-of-K), on a box at ~89% per-core idle:

| run | K | phorj VM | php 8.5 + JIT | ratio |
|---|---|---|---|---|
| 1 | 5 | 125.7 ms | 46.3 ms | **0.37×** |
| 2 | 9 | 127.0 ms | 46.3 ms | **0.36×** |
| 3 | 7 | 135.5 ms | 49.9 ms | **0.37×** |

Three independent runs; the per-sample SPREAD markers were wide (up to 1095%/1611%), so the tails are
untrustworthy, but the best-of-K minima agree to within 8% and all three verdicts are LOSS. **phorj is
~2.7× SLOWER than dockerised PHP with JIT on this scenario.** Per DEC-365 this is recorded OWED and
reported, never re-baselined away.

**Where the OWED record actually lives, because it is not where a reader would look.** It lives in
THIS prose, and **not** in `bench/micro-baseline.json._owed`. That list is DERIVED at `--emit` from
every feature whose ratio is below 1.0, and DEC-365 forbids the `--emit` that would put
`spaceshipsort` there — so the entry cannot be created without doing the very thing the rule bans.
The consequence is worth stating rather than assuming: `microbench-gate.sh:223` prints a
non-blocking `note spaceshipsort: not in baseline (new) — ratio=… ; run --emit to snapshot it` on
**every** run, which is loud, but the deepening check keys off `_owed`, so **nothing stops this loss
getting worse** until a legitimate future `--emit` (one that follows a fix, per the ratchet's own
rule) admits it. That is by design, not an oversight. `identical: true` holds, so the push is not
wedged either way.

**Where the time goes — attributed, not guessed.** Same micro, same pinned core, only the comparator
body changed:

| comparator | phorj VM |
|---|---|
| `(a.tier, a.position) <=> (b.tier, b.position)` | 169 ms |
| `a.tier <=> b.tier` (scalar `<=>`) | 60 ms |
| `a.tier - b.tier` (plain arithmetic) | 56 ms |

So **`Op::Cmp` is not the problem** — a scalar `<=>` costs what an integer subtraction costs (60 vs
56 ms). Roughly **65% of the total is building two tuples per comparison**: each comparison
materializes two `Rc<Vec<Value>>` and throws them away. PHP allocates two arrays per comparison for
the very same reason and is still 2.7× faster, so this is phorj's list-construction path, not an
inherent cost of the idiom. It is also exactly the Op the JIT's boxed tier declines on
(`src/jit/tests/tuple_ordering.rs`), which is why the loop takes the VM.

**Escalated per DEC-507 (stop, do not decide) and now RULED — DEC-513, 2026-09-08: option (a), FUSE
THE COMPARISON IN THE COMPILER.** `(a, b) <=> (c, d)` and the tuple `< > <= >=` forms lower
ELEMENT-WISE at compile time so no tuple is ever materialized — compare position 0, consult position 1
only on a tie. The lowering is exact and total because a tuple's arity is static, the same property
DEC-512 rests on. It reuses `value::compare_ord` / `value::three_way` rather than re-inlining a
comparison, so Invariant 4 holds and the three legs keep ONE kernel, and the transpile leg is
untouched — it still emits PHP's own `<=>`, so DEC-512's byte-identity claim is unaffected. This
builds as **lane L2b, BEFORE L4** (status row 3c).

The two rejected dispositions, recorded with their reasons: **(b)** whitelisting `Op::Cmp` plus a
list-construction Op in the boxed JIT tier — it speeds the allocation up instead of removing it, so
its ceiling is strictly lower than (a), and eligibility is computed over the whole reachable call
graph, so admitting a construction Op widens what compiles far beyond this shape (and the `hits > 0`
proof is unwritable today — `src/jit/tests/tuple_ordering.rs` pins exactly that); **(c)** carrying the
OWED loss to L8 — it unblocks L4/L3 sooner, but nothing protects the loss from DEEPENING until a
post-fix `--emit` admits it to `_owed`, and since the cause is already located, deferring would bank a
KNOWN regression rather than an unknown one.

### Known issues
- **`<` on two large `decimal`s diverges from the PHP leg** (pre-existing, found by measurement while
  building L2, 2026-09-08). `compare_ord` compares the exact i128 carrier; the transpiled PHP compares
  the numeric-string carrier, which PHP treats as a float. So
  `9007199254740992.00 < 9007199254740993.00` is `true` natively and `false` in PHP. **This is a bug
  with a known fix (`__phorj_dec_cmp`, `bccomp`-shaped), not an Invariant-14 no-analog case — it must
  NOT be counted as a third disclosed byte-identity exception.** Recorded in `KNOWN_ISSUES.md`; the
  fix is gated on Q-0908-3 because emitting a helper is an Invariant-16 trade. L2's new surfaces
  refuse `decimal` (`E-ORDER-DECIMAL`) rather than widening the hole.
- **`Stmt::Destructure` carries no `mutable`**, so `[$a, $b] = f(); $a = …;` lifts to
  `var (a, b) = f(); a = …;` and fails `phg check` with `E-ASSIGN-IMMUTABLE` — the same
  two-halves-disagree class L1c fixed elsewhere, and NOT visible in the census (a refusal count is
  not a check run). It surfaces the moment L3 runs `phg check` over the lifted scout tree. Found by
  review, 2026-09-07, not yet reproduced against scout. The honest outcome if scout has the shape is
  a NAMED refusal, not a silent `var` — the lifter cannot see whether a binder is later reassigned
  without a pass that says so.
- `§LIFT-DISCARD` — a call written for effect lifts fine, then fails `phg check` (carried from Lane R).

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
| 1 | L0 — Invariant 19 docs: DEC-504…507 + the 2026-07-10 perf-refinement supersession, readiness re-order, SPEC amendments, SLICE-STATE, MASTER-PLAN mirror | M | todo | - | docs/research/full-audit/raw/C-decisions.md docs/plans/*.md docs/specs/UNIFIED-SPEC.md |
| 2 | L1 — lifter mechanicals: enum `self`, block closures, positional `array{…}` → tuples, named by-ref diagnostic (DEC-506), assignment-target sites; re-census after each | L | todo | - | src/lift/* examples/lift/* |
| 3 | L2 — DEC-505 `<=>` operator + lexicographic tuple/list ordering, all legs + LSP + editors + example + bench | L | todo | - | src/lexer/* src/parser/* src/checker/* src/interpreter/* src/vm/* src/transpile/* src/lsp/* editors/* examples/guide/* |
| 4 | L3 — depth oracle: classifier cluster lifted, four-leg harness over the 130-case corpus, byte-identity, docker-PHP bench | L | todo | - | examples/lift/scout/* tests/* bench/* |
| 5 | L4 — DEC-504 named-field tuples (73 sites), all legs + LSP + editors + example + bench | L | todo | - | src/ast/* src/checker/* src/interpreter/* src/vm/* src/transpile/* src/lift/* src/lsp/* editors/* |
| 6 | L5 — HTML5 parse + CSS selectors + entity decode (readiness 13, DEC-469) | L | todo | - | src/ext/html/* |
| 7 | L6 — `Core.Net` + `Core.Mime` + read-only `Core.Imap` with the file-backed `.eml` transport (readiness 14, DEC-467) | L | todo | - | src/ext/net/* src/ext/mime/* src/ext/imap/* |
| 8 | L7 — breadth census loop to the stop condition: every file lifts or carries a named refusal with a DEC row | L | todo | - | src/lift/* examples/lift/scout/* |
| 9 | L8 — perf twins: a fixture-driven benched scenario per lifted module, DEC-507 bar on every one | L | todo | - | bench/* |
<!-- /progress-block -->

### Blocked
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

### Known issues
- `§LIFT-DISCARD` — a call written for effect lifts fine, then fails `phg check` (carried from Lane R).

# Phorj — Post-Slice-3 Unified Plan: certify the serve arc, consolidate the SSOT, spec the next body of work

> THIS FILE IS THE SINGLE SOURCE OF TRUTH for the post-Slice-3 consolidation and the next body of
> work (Invariant 19). Produced by the 2026-08-31 review session (rulings via AskUserQuestion) and
> persisted 2026-09-01. The executing session's FIRST action is to `git add` + commit this file,
> then follow the Execution order below. Every later step updates THIS file's Decisions Log; other
> docs point at it, never restate it. Where a step says "verbatim" or gives file:line, do exactly
> that; anything ambiguous is a Part 4 developer question — never self-ruled.

## ⭐ TL;DR — READ THIS FIRST (the rest is executable detail)

**Review result**: serve arc (S3.2–S3.5) is built, pushed, gate-green at `cf6875db`. The 3-lens
milestone panel ran 2026-08-31 on that frozen commit: 1 lens CLEAN, **11 findings** — ONE real
code bug, the rest doc drift.

**What will be done, in order:**
1. **Commit this plan** — it becomes the single source of truth; all else points at it.
2. **Fix the 1 real bug (P1)**: transpile bypass via `Core.Native.Http.registerServe` — emits PHP
   that crashes at runtime instead of refusing at transpile time.
3. **Fix ~15 stale doc lines** (exact file:line listed): stale `--help` example, register still
   saying serve "UNBUILT", README claiming the opposite of shipped TLS behavior, stale headers.
4. **Consolidate docs — NOTHING deleted, everything moved to `docs/archive/plans/` and
   `docs/archive/specs/` (developer-ruled location) with pointer READMEs**: archive 6 finished
   plan files (rescuing 2 unique rationale sections first); fold 10 shipped specs into
   UNIFIED-SPEC (8 unbuilt specs STAY canonical) — ⚠ **became 11 folds / 7 live**, see the Decisions
   Log: item 6's own mandated check found DEC-339 BUILT, so block-scope-shadowing left the protected
   list; migrate the existing `docs/specs/archive/`
   into `docs/archive/specs/`; shrink SLICE-STATE (4,288 lines → current cursor only); refresh
   MASTER-PLAN's 6-week-stale cursor.
5. **Two small code slices**: TEST-RAW-CHECKER fix (`phg test` spurious prelude errors); G-8 perf
   verdict on a quiet box.
6. **Ask the developer the 7 queued adjudications** (DEC-455.4/.5/.6, ServeConfig nullable fields,
   gap-programme batch, LSP prelude go-to-def, unruled L-items).
7. **Then the next feature work: DEC-333 perf roadmap** (Json-ADT JIT → AOT → interpreter
   campaign) — developer ruling: unblock first, then perf.

**Guardrails**: a DO-NOT-DELETE list (Part 1 — the executing model cannot over-delete),
verification doctrine (Part 7 — count non-skips, no filtered-green claims), unchanged commit
discipline (master, plain push, developer identity, no trailers).

---

## Context

DEC-331 Slice 3 (the `phg serve` arc: S3.2 Part C, S3.3a–e, S3.4, S3.5) is BUILT and PUSHED —
`origin/master` == `master` == `cf6875db` (verified 2026-08-31, `git rev-list --count` = 0).
The developer asked for: (1) a full review/certification of everything done, (2) a list of unknowns
needing their input, (3) fragile implementations, (4) ONE unified plan as the single source of
truth with divergent docs removed, executed by another model with zero interpretation latitude.

Four shape rulings were made by the developer on 2026-08-31 (AskUserQuestion):
- **SSOT shape**: one new repo plan doc (this file) + the Invariant-19 quartet cleaned — NOT a
  mega-doc replacing the quartet (no Invariant-19 re-ruling).
- **Next body of work**: HYBRID — front-load the adjudication batch + the two small OWED items,
  then the DEC-333 perf roadmap (ruled 2026-07-23: "FIRST the DEC-331 cluster, THEN perf").
- **Archive mode**: archive dirs + pointer READMEs; nothing deleted.
- **Certification tier**: the declared-due 3-lens milestone panel (S3.5 plan §5) was RUN against
  frozen `cf6875db` — verdicts in Part 0 §0.1.

## Decisions Log

- [2026-08-31] AGREED: SSOT shape = new plan doc + cleaned quartet; Invariant 19 unchanged.
- [2026-08-31] AGREED: next work = hybrid (adjudications + OWED small items → DEC-333 perf roadmap).
- [2026-08-31] AGREED: archival convention = archive dirs + pointer READMEs, nothing deleted;
  MASTER-PLAN's "git history" convention line is superseded and gets edited.
- [2026-09-01] AGREED (refinement): archive LOCATION = `docs/archive/plans/` and
  `docs/archive/specs/` — one root; the existing `docs/archive/specs/` content migrates into
  `docs/archive/specs/` in the same pass.
- [2026-08-31] AGREED: milestone 3-lens panel run on frozen `cf6875db` (discharges S3.5 plan §5's
  run-the-panel obligation). Round 1 = 1 CLEAN + 11 findings across two lenses → the milestone
  gate stays OPEN until fixes land and the developer's chosen closing procedure completes (§0.1).
  G-8 microbench verdict remains OWED (quiet box required).
- [2026-09-02] AGREED: next block = finish the A0+A1 RESIDUE the round-2 panel found — the four
  live code findings (playground = 4th ungated PHP-emit path; LSP still on the non-test path;
  `resolve_variant_imports`/`desugar_router` skip `Item::Test`; the differential's headline
  over-reports with no floor assertion). Rationale: each is a hole in something already reported
  done, and the gate cannot honestly close over them. DEC-457/458/459, Phase B and the remaining
  adjudications queue behind it.
- [2026-09-02] AGREED: certification tier for this block = `advisor()` at 3C and 6C, NO panel —
  executable evidence does the refuting between gates (red-first, then a sabotage check proving
  the suite NOTICES each guarantee breaking). The panel stays in reserve for the actual close.
- [2026-09-02] AGREED (certification tier, Part 0 §0.1): **ONE 3-lens panel round against HEAD, then
  close.** The panel's frozen `cf6875db` is no longer HEAD — W1–W4 landed code after it — so a round
  now reviews the NEW tree (the A0 ladder fix, the `ladder.rs` split, the `phg test` front-end change
  and the whole consolidation), none of which any panel has seen. DEC-268's two-consecutive-clean rule
  is therefore **knowingly relaxed once**, with that reason recorded, rather than silently skipped.
- [2026-09-02] AGREED (DEC-455.4): generic config providers **key on the REIFIED type**, so
  `Map<string,string>` and `Map<string,int>` are distinct injection keys. Rejected: a check-time
  ambiguity error (would forbid two legitimate generic configs) and last-wins-with-warning (keeps a
  silent-wrong-value shape one ignored warning away). The checker already proves reified types for
  arithmetic operands, so the information exists.
- [2026-09-02] AGREED (`Core.Database` case-1 step 2): the PHP twin is a **`__phorj_db_stmt` wrapper
  holding `[PDOStatement, sql, params[], nextIndex]`** — `prepare` returns it, the bind family appends,
  `query`/`exec`/`executeMany` call `execute($params)`. It mirrors phorj's accumulate-then-execute
  semantics and is what makes `executeMany` expressible at all. This unblocks the whole DB lift.
- [2026-09-02] AGREED (PRELUDE-ALIAS-COLLISION): **isolate prelude-internal bindings** so a prelude
  fragment resolves its own references in a namespace user imports cannot rebind — killing the class,
  not the instance. Rejected: a precise diagnostic (leaves the fragility) and making the serve fragment
  self-sufficient (the "second, drifting declaration of the same alias" the prelude deliberately
  avoids, and other fragments keep the exposure). Its own slice; adjacent to §span-collision.
- [2026-09-02] AGREED: execution autonomy = W0→W4 straight through (commit plan → A0 + help.rs P1
  → drift fixes → full consolidation → A1/A0b), pushing green commits as they land; the session
  stops only for the Part 4 adjudication batch and the Part 0 §0.1 certification-tier question,
  both of which require the developer.
- [2026-09-02] RESOLVED (by the check Part 3 item 6 itself mandates): DEC-339 is BUILT 2026-07-29
  (`E-SHADOW-LOCAL`, `src/checker/plumbing.rs`), so `2026-07-26-block-scope-shadowing.md` leaves Part 1's
  protected list and joins the Step 4 fold list — **11 folds, 7 live specs remain**, not 10 and 8. Its
  23-row case list moved INTO UNIFIED-SPEC (the register cited it as canonical, and an archive marked
  "not current" cannot hold a canonical rule); the register citation was repointed in the same change.
- [2026-09-02] AGREED: Part 2 consolidation is executed INLINE and sequentially — no subagent
  fan-out for the spec folds or the SLICE-STATE collapse. Rationale: the fold rule (normative
  surface text into UNIFIED-SPEC; per-file BUILD STATUS ledgers NOT copied) is judgment, and a
  mis-fold silently loses normative spec text — the exact loss Part 1 exists to prevent.
- [2026-09-02 16:40] AGREED (scope of the next body of work): **no scout port is built in this repo.**
  The developer will implement a scout-class app themselves later; phorj's job is to be READY for it —
  every capability the `/stack/projects/scout` inventory needs (regex parity, HTML5 parse + selectors +
  entity decode, MIME/RFC822, `Core.Net` TCP+TLS, IMAP, `sleep`, shutdown signals, tz-aware time,
  `.env`, JSON hard errors, injectable `HttpClient`, charset/QP) must exist as stdlib, each with its
  Invariant-9 example, LSP row and flip-or-flag bench. The scout demand inventory
  (session scratchpad `raw/scout-needs.md`, 52 rows) is the yardstick, not a deliverable.
- [2026-09-02 16:40] AGREED: **all three streams are authorised** — (1) the PHP-readiness stdlib
  wave above, (2) the open-bug drain (26 open, `raw/issues.md`), (3) the DEC-333 perf roadmap. The
  ORDER is the next ruling (recommendation: harness-trust bugs → readiness wave with per-feature
  benches → JIT/AOT roadmap). Schema-v12 SQLite compatibility with the PHP scout is MOOT (no port).
- [2026-09-02 16:40] PENDING (regex spine break, found today on the shipped binary):
  `Regex.compile("(?<=a)b")` → `phg check` OK, `phg run` faults (`regex` crate has no look-around),
  `phg transpile` emits `preg_match('/(?<=a)b/')` which PHP runs and matches. Invariant 1 broken on
  check-clean input; `src/ext/regex/natives.rs:307` emits the pattern unvalidated. Developer chose
  "rewrite the patterns app-side" in the first question but asked for a clearer explanation before the
  phorj-side ruling (parity-by-refusal vs opt-in backtracking vs backtracking default) — re-asked.
- [2026-09-02 17:05] AGREED (REGEX, option B — supersedes the PENDING row above): `Regex.compile`
  stays the linear-time `regex` engine (ReDoS-immune, unchanged); a second constructor
  (`Regex.compileBacktracking`, name to be confirmed in its slice) accepts PCRE-class syntax
  (look-around, backreferences, branch reset, atomic/possessive) via a `fancy-regex`-class crate as the
  15th vetted dependency, with a step budget that raises a typed fault instead of hanging. Both
  constructors emit the same `preg_*` PHP. Compile-time validation of LITERAL patterns on the transpile
  leg + a runtime helper for dynamic ones close today's Invariant-1 hole regardless. DEC row owed.
- [2026-09-02 17:05] AGREED (ORDER): (1) **harness trust** — `default_fills` span-collision P0,
  differential floor + explicit expected-skip set, the three ungated PHP-emit paths (playground,
  `phg benchmark`, regex literal gate), the LSP test-mode path, then DEC-459 prelude isolation;
  (2) the **PHP-readiness wave** in leverage order (regex B → `sleep` + DEC-204 signals → DEC-247 tz →
  `.env`/process/stderr → JSON hard errors + list-vs-object → HTML5 parse/decode/select + DEC-382 XML →
  `Core.Net` TCP+TLS → MIME → DEC-413 IMAP → HTTP client cookies/gzip/injectable interface → DEC-271
  charset/QP/accent-fold), every module benched flip-or-flag as it lands; (3) the DEC-333 perf roadmap.
  Remaining open bugs interleave where they block a wave item.
- [2026-09-02 17:05] AGREED (PERF RULINGS SPLIT): the closure-entry JIT ruling (DEC-434), the
  fallible-call de-JIT region (DEC-431) and the `TakeLocal` op for `s = s + x` are IMPLEMENTATION
  choices Claude makes and ships byte-identical with before/after benches; the `Json.getInt`-style
  accessor surface is user-visible and comes back as an Invariant-15 question when its slice starts.
- [2026-09-02 17:05] AGREED (LOST PANEL): the 24-finding panel report (cited only by SHA in
  SLICE-STATE, never landed in the repo) is recovered by **re-running the three-lens panel NOW, on a
  frozen commit, before any new work**. Its findings table is transcribed INTO this plan file (tracked)
  — `var/claude/` is gitignored, which is how the previous report was lost.
- [2026-09-02 18:15] AGREED (Part 4 item 2, DEC-455.5): a repeated config parameter type is
  **memoized per entry** — one type, one instance, the provider runs once per process entry; config
  and `inject<T>()` (default-SHARED) agree. DEC row to be appended.
- [2026-09-02 18:15] AGREED (Part 4 item 3, DEC-455.6): config candidacy is **declined when NO
  parameter resolves to a provider**, and the entry then reports the accurate `E-ENTRY-SIG`; with at
  least one resolvable provider the unresolved ones keep `E-CONFIG-MISSING`. Scalar providers stay.
- [2026-09-02 18:15] AGREED (Part 4 item 4, §SERVE-CONFIG-PROVENANCE root fix): **D4 `ServeConfig`
  fields become nullable** (`int? timeout` etc.; `null` = unset, `0` = no timeout); the D4 spec text
  is amended in the same change; `port`/`maxBodySize`/`timeout` range validation lands with it.
  `new ServeConfig()` keeps today's behaviour byte-for-byte.
- [2026-09-02 18:15] AGREED (Part 4 item 9, `decimal` on the PHP leg): **bind and fetch `decimal` as
  TEXT** via the `__phorj_db_stmt` wrapper, reconstructed exactly by `getDecimal`; SQL arithmetic on
  a decimal column is done in phorj, not SQLite's float engine. Byte-identity holds; documented as
  TEXT-affinity on the PHP leg.
- [2026-09-02 18:15] STANDING DIRECTIVE (developer, verbatim in substance: *"work more on the LSP,
  the editors VSCode and PhpStorm extensions and the transpile/lift"*): the readiness wave and every
  slice after it invests in the LSP + both editor extensions + transpile AND lift as first-class
  deliverables, not the closing checklist row — Invariant 17's 100% rule is to be AUDITED per feature
  (completion, hover, go-to-def, references, symbols, diagnostics with tags, signature help, grammar)
  and the gaps worked as their own slices, and the lift (PHP→Phorj) is kept at parity with transpile.
- [2026-09-02 18:20] STANDING GOAL (developer): **CROSS-LANGUAGE SCAN, again** — as at the start
  (Invariant 16 / META-7), survey what other languages (Rust, Kotlin, Swift, TypeScript, Go, C#,
  Java, Python…) can do that PHP cannot, and what they do BETTER than PHP, and bring it to phorj.
  Output = a delta list against phorj's shipped surface AND its plan (nothing already built or
  already queued is re-proposed); each new item becomes an Invariant-15 question, never self-ruled.
- [2026-09-02 19:10] STANDING GOAL (developer): **PHP-ECOSYSTEM SCAN** — survey the current PHP
  ecosystem (the developer named **TypePHP** and **phpx**, described as PHP→AOT-binary converters;
  plus KPHP, PeachPie, HHVM/Hack, static-php-cli/phpmicro, FrankenPHP, Swoole, ReactPHP/Amp, the
  8.5/8.6 RFC pipeline, PHPStan/Psalm taint analysis, Rector, Pest/Infection, Deptrac…) for anything
  worth discussing, porting, implementing or improving on in phorj. Delta-only against code + plan;
  every claim verified against a fetched primary source; each new item is an Invariant-15 question.
- [2026-09-02 19:10] NOTE (demand inventories, this session, all in the session scratchpad
  `raw/`): `scout-needs.md` (52 rows), `twesin-needs.md` (26 rows + 12 PHP flaws),
  `invoiceninja-needs.md` (~90 rows + 23 PHP flaws), `cross-language-delta.md` and
  `php-ecosystem-delta.md` (pending). Their consolidated delta graduates into the readiness-wave plan
  file, not into this consolidation plan.
- [2026-09-02 21:05] AGREED (certification tier, STEP 1 harness trust): **`advisor()` per gate for
  every slice of step 1, then ONE panel round on the frozen commit at the end** (DEC-481
  fix-then-verify). Red-first tests + a sabotage check per slice remain mandatory.
- [2026-09-02 18:06] AGREED (panel C9/K1/K7, Invariant 17 `check ≡ LSP ≡ test`): **a document containing a
  `test` item is checked in test mode — in the LSP AND `phg check`** (DEC-486). `run`/`transpile`/
  `build` keep the strict flag. Option B (test mode only under `tests/` paths) and option C (an editor
  setting) were rejected: both make the diagnostic depend on something other than the document.

## Execution order (global — Parts are reference material, THIS is the sequence)

1. **Commit this file** (one `docs:` commit — Part 2 Step 1).
2. **Part 5 A0** — fix the live P1 spine break (`E-TRANSPILE-SERVE` bypass). A shipping
   Invariant-1 violation outranks every doc/consolidation step.
3. **Part 3 drift fixes** (items 1–15, including the panel's help.rs P1) + **Part 2 Step 2**
   prerequisites (rationale migration, panel-record addendum).
4. **Part 2 Steps 3–7** — archives, spec folds, SLICE-STATE collapse, MASTER-PLAN refresh.
5. **Part 5 A0b / A1** (TEST-RAW-CHECKER) — code slices; **A3** (adjudication batch, Part 4) can
   be asked any time from step 1 onward, and A2 (G-8) whenever a quiet box exists.
6. **Re-certification gate** — freeze, then ask the developer the tier question (§0.1 closing rule).
7. **Phase B** — DEC-333 perf roadmap.

---

## Part 0 — Certification record: DEC-331 Slice 3 (what is verified, and by what)

**Certified by execution** [Verified]:
- All 24 arc commits (`0c982019..cf6875db`) passed the pre-push FULL gate (all-features nextest,
  clippy `--all-features` AND `--no-default-features`, fmt, release build, PHP-oracle spine) —
  evidence: the push exists; pre-push is the SSOT of its own steps (`scripts/git-hooks/pre-push`).
- S3.4: exercised on a real pty, all four paths, sabotage-verified twice (plan §, DEC-455.15).
- S3.5: end-to-end live TLS traffic (`phg serve` + `curl`) on BOTH accept paths, after a 6C
  finding that the wiring had never carried a byte of TLS (S3.5 plan §8) — the earlier
  name-filtered "9 passed" green was FALSE (§7); CHANGELOG/register/commit corrected.

**Certified by the 2026-08-31 milestone panel** (3 read-only lenses vs frozen `cf6875db`,
range `0c982019..cf6875db`): verdicts in §0.1.

**Explicitly NOT certified — carried OWED**:
- **G-8 microbench ratchet** — skipped since S3.4 (SLICE-STATE:44). Needs a quiet box; per-core
  `mpstat` idle check, NOT load-avg, before any run. NO-HIDDEN-LOSS (DEC-365): record the verdict,
  never re-baseline via `--emit` to make it pass.
- Anything requiring the four ruled-deferred TLS features (KNOWN_ISSUES §SERVE-TLS) — deferred BY
  RULING, not oversight: HTTP→HTTPS redirect, HSTS, cert hot-reload, mTLS; plus passphrase keys
  unsupported and cert paths resolving against process cwd.

**Rulings still PENDING (developer)** — see Part 4: DEC-455.4, DEC-455.5, DEC-455.6
(C-decisions.md:8374-8376, open since S3.2 Part B).

### §0.1 Panel verdicts (2026-08-31, frozen `cf6875db` `feat(serve): HTTPS that refuses rather than falls back — inbound TLS (S3.5, DEC-331 D7)`, ONE round — NOT a two-clean close)

- **security + safety-promises: CLEAN.** All promises verified against diff + source + executed
  tests (10/10 default-feature TLS refusals; 21/21 with `http-server-tls` incl. a real handshake;
  PEM properties pinned; no committed secrets — `git grep '-----BEGIN'` hits only pem sources;
  "no new crate" honest; unsafe island untouched; DEC-363 header guard intact through the
  framing move; prompt requires stdin AND stderr TTY, default NO).
- **completeness + blast-radius: FINDINGS — 8** (2×P1, 2×P2, 4×P3 — all folded into Part 3 and
  Phase A below). Execution evidence: corpus 199 RUN / 19 SKIP + 19 projects (non-skips counted);
  serve 31/31; serve_tls 13/13 with all 4 handshake tests EXECUTED; ratchets PASS (codes 257/312,
  size-gate 0 fails); all 8 new diagnostic codes asserted + `explain`-covered; LSP completes all
  ten ServeConfig members (pinned); lift emits `#[Entry(kind:)]`, zero `respond` remnants.
  Conformance fixtures for the 8 new codes: zero added — inside the ratchet's tracked-debt
  design, recorded as fact.
- **correctness + regression: FINDINGS — 3** (1×P1 spine break = Phase A item A0; 2×P2). Checked
  clean with evidence: Op triad untouched; DEC-455.11 aliasing sound (`use Foo\Exception;` probe —
  no PHP internal-class conflict); role gate single-chokepoint ahead of both engines; all four
  `E-SERVE-TLS-*` shapes present + explained; framing move behavior-preserving.
- Cross-coverage note: what the correctness lens could not execute (corpus counts, handshake
  tier), the completeness lens executed on the same frozen commit, and vice versa.

Range disclosure: the panel read the diffs of `0c982019..cf6875db` only — S3.2 Parts A/B predate
the range, so their surfaces were reviewed at HEAD state by the lenses but their diffs were never
panel-read.

**Milestone gate status: OPEN.** This was round 1 with findings, so the clean counter is at zero.
After the fixes land and the tree is FROZEN, the executing session asks the developer the
certification-tier question per the standing rule (the tier is the developer's choice at EVERY
gate — never carried forward), presenting the genuine tension for them to arbitrate: DEC-268
mandates two consecutive fully-clean panel rounds to close a milestone, while the 2026-08-19
economize ruling says one panel per milestone and calls a second one the waste the rule exists to
prevent. Do not resolve that tension autonomously, and never report Slice 3's milestone as
panel-certified until the developer's chosen closing procedure completes.

---

## Part 1 — DO-NOT-DELETE list (hard constraints on "remove all other docs")

The developer's "remove all divergents" applies ONLY to items Part 2 names. The executing model
MUST NOT delete, merge away, or "simplify" any of the following:

1. **The SSOT quartet** — `docs/plans/MASTER-PLAN.md`, `docs/plans/SLICE-STATE.md`,
   `docs/specs/UNIFIED-SPEC.md`, `docs/research/full-audit/raw/C-decisions.md`. They get
   CLEANED (Part 2), never removed. C-decisions.md is append-only by design — never consolidated,
   never rewritten, supersessions are new rows.
2. **The 8 RULED-NOT-BUILT loose specs** (live SSOTs for unbuilt features):
   `docs/specs/2026-07-23-any-object-top-types.md`, `2026-07-23-array-access.md`,
   `2026-07-23-eval-position.md`, `2026-07-23-labeled-break-continue.md`, `2026-07-23-typed-lsb.md`,
   `2026-07-26-block-scope-shadowing.md` (stale label — see Part 3, but the FILE stays),
   `2026-07-26-capture-write-rejection.md`, `2026-07-26-ufcs-lsp-companion.md`.
   These KEEP their canonical status until their feature ships; each gets a QUEUED mirror row in
   MASTER-PLAN (Part 2 step 6).
3. **Two live plan files**: `docs/plans/product-driven-gap-programme.plan.md` (blocked on the §4
   adjudication batch — it is the INPUT to a future milestone) and
   `docs/plans/claude-bundle-cross-repo-audit.plan.md` (self-described portable artefact for the
   sibling repos — deliberate, not drift).
4. **KNOWN_ISSUES.md, CHANGELOG.md, docs/HISTORY.md, docs/MILESTONES.md, docs/INVARIANTS.md,
   docs/ARCHITECTURE.md, FEATURES.md** — distinct chartered roles; drift-fixes only (Part 3).
5. **`var/phorj-app`** — the developer's live comparison app; never propose deleting (standing rule).
6. **The archived originals + archive READMEs** — `docs/specs/archive/` content (20 folded
   originals + README) MOVES to `docs/archive/specs/` (Step 4) but is never deleted or pruned.

---

## Part 2 — Consolidation (ordered; each step is one self-contained commit)

**Step 1 — commit this plan**: `git add docs/plans/2026-08-31-post-slice3-consolidation.plan.md`,
commit (`docs:`). This is the "one single source of truth" going forward: every later step updates
IT, and other docs point at it.

**Step 2 — discharge the two conditional-archival prerequisites** (BEFORE any file moves):
- 2a. Migrate the rejected-architecture rationale out of
  `docs/archive/plans/2026-08-22-s3-3-http-serve.plan.md` §3/§3b/§3c (lines ~184-253): the
  inverted-loop design, the explicit "Kept, not deleted: a fresh session must not rebuild this"
  warning, and the two killing facts (a native cannot call a method; the invoker does not outlive
  the native call). Destination: a new dated NOTE row appended to the DEC-331 block in
  `C-decisions.md` (after :3046) AND a short module-doc paragraph in `src/serve/mod.rs`.
  This rationale exists NOWHERE else; deleting the file first loses it (survey-verified).
- 2b. Move the S3.5 §5 milestone obligation to its resolution: append to the SLICE-STATE current
  cursor that the panel ran 2026-08-31 on `cf6875db` (verdicts from Part 0 §0.1, gate OPEN) and
  that G-8 remains OWED. The S3.5 plan file then holds nothing unique.

**Step 3 — create `docs/archive/plans/`** (developer-ruled location, 2026-09-01) with a pointer
README modeled on `docs/archive/specs/README.md` (per-file → where-its-content-now-lives table).
`git mv` these six (all survey-verified SHIPPED, register-confirmed):
- `2026-08-22-s3-3-http-serve.plan.md` (after 2a)
- `2026-08-28-s3-4-role-mismatch.plan.md` (DEC-455.15 mirrors it fully)
- `2026-08-29-s3-5-inbound-tls.plan.md` (after 2b; fix its stale header first — Part 3 item 4)
- `2026-08-23-transpile-ns-prelude.plan.md` (DEC-455.11 shipped+certified)
- `2026-08-04-lift-attr-and-hoist.plan.md` (DEC-397/435/436 BUILT)
- `2026-07-28-consistency-audit.plan.md` (one-shot, zero inbound refs)
Then edit MASTER-PLAN.md:4-6 to state the `docs/archive/` convention (supersedes "git history").

**Step 4 — fold the SHIPPED loose specs into UNIFIED-SPEC.md** (⚠ 10 listed here; it became **11** —
see the Decisions Log. The tree matches the log, not this count.), then `git mv` each original to
`docs/archive/specs/` and add its row to the archive README pointer table. First migrate the
EXISTING `docs/specs/archive/` (20 previously-folded originals + its README) into
`docs/archive/specs/` via `git mv`, then `git grep 'docs/archive/specs'` and repoint every inbound
reference (UNIFIED-SPEC header included) — zero stale hits before proceeding. The 10 to fold:
`2026-07-22-transpile-into-project.md`, `2026-07-23-entry-kinds-serve-tls.md`,
`2026-07-23-invoke-tostring.md` (slice 1b deferral noted in the folded section),
`2026-07-23-rich-request.md`, `2026-07-24-visibility-model.md`, `2026-07-24-wildcard-imports.md`,
`2026-07-26-ast-exhaustiveness.md`, `2026-07-26-response-header-injection-guard.md`,
`2026-07-26-transaction-depth-semantics.md` (items 1/2/4/5 built; note item 3's state verbatim),
`2026-07-30-using-scope-guard.md`.
Fold rule: normative surface text → a new/extended UNIFIED-SPEC `##` section; per-file "BUILD
STATUS" ledgers (e.g. entry-kinds-serve-tls.md:7-30) are SLICE-STATE-class content — do NOT copy
them into UNIFIED-SPEC; the archive README pointer row records the final status line instead.
UNIFIED-SPEC's header note ("dated specs are folded at ship-time, never left as parallel SSOTs")
becomes true again. One commit per ~3-4 folds is fine; keep each self-contained.

**Step 5 — SLICE-STATE consolidation**: keep the current cursor (top, 2026-08-29 + the 2026-08-31
panel addendum from 2b) plus at most the two most recent superseded cursors; move EVERYTHING below
(~line 400 down: the ~28 dated cursor/session/handoff blocks, including the four stale
"CURRENT CURSOR" claimants at :393/:443/:490 and the historical "NEXT MAJOR BODY OF WORK
2026-07-20" block at :3144) to `docs/archive/plans/SLICE-STATE-ARCHIVE.md` with a one-line pointer
left at the bottom of SLICE-STATE. Nothing is deleted; `git grep` still finds it.

**Step 6 — MASTER-PLAN refresh** (one commit):
- Rewrite the §0 cursor row (:26) — currently dated 2026-07-17 + parenthetical patches; new row:
  date of the commit, HEAD at the then-current sha, "DEC-331 Slice 3 COMPLETE; next = hybrid per
  this plan". No parentheticals — rewrite the row.
- Reconcile the three "what's next" claimants IN PLACE: §0:33 (build-order) and :293-297 (DEC-333
  perf) each get one line pointing at this plan doc as the arbitrated ordering (hybrid). Do not
  delete either section's content.
- Fold `docs/archive/plans/2026-07-26-ruled-build-order.md` INTO MASTER-PLAN as a section (it is the
  de-facto queue): strike the built Wave-3 rows (DEC-364/348/348.1/347 — BUILT 2026-07-31,
  register-verified), correct its false header ("every item below is RULED and NOT YET BUILT"),
  then archive the original file per Step 3's convention.
- Add QUEUED mirror rows for the 8 live loose specs (Part 1 item 2), each pointing at its spec file.

**Step 7 — drift fixes** (Part 3 checklist; single `docs:` commit unless a panel item escalates).

---

## Part 3 — Drift-fix checklist (all survey/panel-verified at these locations)

1. `KNOWN_ISSUES.md:2941-2951` §SERVE-CONFIG-PROVENANCE — remove `tlsMinVersion` from the owed
   range-validation list (E-SERVE-TLS-MIN-VERSION shipped in S3.5, `src/serve/tls.rs:38`,
   narrowing ruled in DEC-455.16 ruling 3 at C-decisions.md:8421). Residual owed:
   `port`/`maxBodySize`/`timeout` bounds only.
2. `docs/archive/plans/2026-08-22-s3-3-http-serve.plan.md:218-223` §3b — add the same inline
   `SUPERSEDED` marker its siblings §3 (:184) and §3c (:225) carry (shipped behavior is the
   OPPOSITE: flag wins loudly, W-SERVE-CONFIG-OVERRIDDEN, DEC-455.14). Do before Step 3 archival.
3. Same file `:262` build-order table — S3.3d row missing its ✅ (shipped 2026-08-23, `12341ca7`,
   DEC-455.12).
4. `docs/archive/plans/2026-08-29-s3-5-inbound-tls.plan.md:3-5` header — still "RULED, NOT BUILT"; change
   to SHIPPED 2026-08-29, DEC-455.16. Do before Step 3 archival.
5. `C-decisions.md:2915` DEC-331 block label — still "(INTERACTIVE DESIGN, QUEUED; … no side plan
   doc)"; append-only discipline: add a dated correction row (see item 10).
6. `docs/archive/specs/2026-07-26-block-scope-shadowing.md:3` — "not yet built" is FALSE (DEC-339 BUILT
   per register). Fix the status line; file stays live only if any part remains unbuilt — verify
   against the register first; if fully built it joins the Step 4 fold list instead.
7. `docs/archive/plans/2026-07-26-ruled-build-order.md:6` header + un-struck Wave 3 — handled in Step 6.
8. `docs/plans/MASTER-PLAN.md:26` stale §0 cursor — handled in Step 6.
9. **[panel P1 — CODE, not doc]** `src/cli/help.rs:257` — `phg serve --help` example line names
   `examples/web/server.phg`, a file S3.3d DELETED (now the project `examples/web/server/`).
   Fix the example line to a path that exists (`examples/web/server/serve.phg` is test-served,
   tests/serve.rs:1079). Check for a help snapshot test and update it in the same change.
10. **[panel P1]** `C-decisions.md:2915-2932` DEC-331 block — beyond the stale QUEUED label
   (item 5): it still claims "no side plan doc" and "D2/D3/D5/D6/D7 remain UNBUILT" with NO
   forward pointer to DEC-455.11–.16 (rows 8383-8421). Append-only correction row: Slice 3
   COMPLETE, D5/D6/D7 BUILT, forward pointer to the .11–.16 rows and the archived plan docs.
11. **[panel P2]** `examples/README.md:219` (`web/serve_config.phg` row) — re-asserts the exact
   claim S3.5 REJECTED: "(a lone `cert` still serves plain HTTP)". Contradicts row :215 four
   lines up and the example's own corrected header. Security-relevant claim surface — fix the
   parenthetical to "a lone one is `E-SERVE-TLS-INCOMPLETE`".
12. **[panel P2]** `docs/archive/plans/2026-08-22-s3-3-http-serve.plan.md:7` — the header that declares
   itself authoritative says "D7 inbound TLS, still unbuilt. Next slice is S3.4" — both false
   since 2026-08-28/29. Fix before Step 3 archival (same pass as items 2/3).
13. **[panel P3]** `docs/plans/SLICE-STATE.md:50` — the 2026-08-28 S3.4 cursor lacks the
   "⚠ SUPERSEDED" banner every other old cursor carries; its "Next: S3.5" (:91) reads live.
   (Absorbed by Step 5 consolidation — verify it gets the banner or moves to the archive.)
14. **[panel P3, grouped]** Stale flat-path references to files S3.3d converted to projects:
   `docs/MILESTONES.md:299-300` (`examples/web/server.phg`, `examples/web/json-api.phg`);
   `tests/serve.rs:49` (present-tense "is byte-identity-gated" about a deleted file);
   historical-comment citations of `core-http.phg` at `tests/serve.rs:998`,
   `src/transpile/call.rs:26`, `src/native/fs_prelude.rs:13` — reword to past-tense/project paths.
15. **[panel fact, not defect]** Zero conformance fixtures exist for the 8 new diagnostic codes
   (`codes_in_conformance` 25 before and after the arc) — tracked ratchet debt; queue fixture
   additions with the next conformance batch, do not silently absorb.

---

## Part 4 — Adjudication batch (developer inputs — ask at execution start, Invariant-15 shape)

These are the ONLY unknowns requiring developer input. The executing session asks them via
AskUserQuestion, batched ≤4 per call, each with a minimal failing program embedded in the question
and after-states inside each option, recommended-first, with the challenge-the-premise escape.
Never proceed on a default; PENDING answers block only their own items, not the consolidation.

1. **DEC-455.4** (C-decisions.md:8374): generic config types collide under one provider key —
   two `#[Config]` providers whose types erase to the same key. Options to prepare: key on the
   reified type / reject at check-time / last-wins with warning.
2. **DEC-455.5** (:8375): a repeated config parameter calls the provider twice — memoize per
   request, per process, or keep call-per-use (document it)?
3. **DEC-455.6** (:8376): the widened entry arity cost the accurate `E-ENTRY-SIG` diagnostic —
   restore specificity via a dedicated arity check, or accept the generic message?
4. **§SERVE-CONFIG-PROVENANCE root fix** (KNOWN_ISSUES:2912-2953): make the D4 `ServeConfig`
   fields nullable so provenance is real, not approximated-by-value ("field written at its default
   reads as unset"; `timeout: 0` cannot express "no timeout"). This contradicts the D4 spec text
   (non-optional fields) — that is WHY it is an adjudication, not a fix.
5. **Gap-programme §4 batch** (`docs/plans/product-driven-gap-programme.plan.md` §4, :755 §7):
   the 2026-08-07 doctrine items — dates-with-timezones, crypto beyond password hashing, charset
   transcoding, compression, process spawn. Unblocks the programme's spec round.
6. **§LSP-PRELUDE-DEFINITION** (KNOWN_ISSUES:2886-2911): go-to-definition on an injected-prelude
   symbol — three candidate answers already enumerated there, one banned (§span-collision).
7. **Unruled build-order leftovers**: L-22, L-25, L-33, L-86 ("L-22 and L-33 look substantial",
   ruled-build-order:128-130) — surface for ruling or explicit deferral.

**FOUND WHILE EXECUTING (added 2026-09-02) — four more, none of them self-ruled:**

8. **`__phorj_db_stmt` wrapper shape** (`Core.Database` Ladder case-1, step 2). phorj's `Statement`
   binds onto a SHARED raw handle in place and returns it (a deliberate DEC-266 allocation lever),
   while PDO's `bindValue` needs a 1-based index — so `prepare` must return a WRAPPER object carrying
   its own parameter accumulator and positional counter, and that wrapper's shape determines what
   every other emitter can assume. **The spec's own "~20 emitters; mechanical" estimate was retracted
   in writing as false.** Recommended (NOT ruled): `[PDOStatement, sql, params[], nextIndex]`, which is
   also what makes `executeMany` expressible. Blocks the whole DB case-1 lift.
9. **`decimal` mapping on the PHP leg** (same slice, step 3). PDO+SQLite returns native `int`/`float`
   (an earlier assumption otherwise was wrong and is corrected), but a `NUMERIC` column comes back as
   float `19.99` where phorj `decimal` is exact fixed-point. Bind/fetch as TEXT and reconstruct
   exactly, or accept float on the PHP leg and disclose it.
10. **P-Q-B-1 — visibility narrowing on an overloaded interface method.** Pre-existing, open since
    2026-07-25, surfaced by the visibility-model fold; not closed by that build.
11. **PRELUDE-ALIAS-COLLISION** (KNOWN_ISSUES, verified 2026-09-02). Importing `Core.Native.Http` under
    any alias but `NativeHttp` and using it suppresses the prelude's own binding, failing with
    `E-UNKNOWN-IDENT` at prelude lines the user cannot open. The fix — making prelude-internal bindings
    immune to user aliasing — changes the import model and sits in §span-collision territory, so it is
    an adjudication rather than a patch.

---

## Part 5 — Next body of work (hybrid, ruled 2026-08-31)

**Phase A — unblock (small, immediate, parallel-friendly):**
- **A0. [panel P1 — FIRST, it is a live spine break] Close the `E-TRANSPILE-SERVE` bypass.**
  `phg transpile` on a program calling `Core.Native.Http.registerServe(...)` directly (import
  `Core.Native.Http as NativeHttp`) exits 0 and emits `__phorj_http_register_serve(...)` — a stub
  NO helper family defines — so the PHP leg fatals at runtime while native legs run fine.
  Invariant 1 broken; Invariant 14 tier 2 demands a transpile-time hard error. Evidence: panel
  repro executed against the release binary; negative control (`Http.serve` spelling) correctly
  refused. Root cause: the refusal is keyed only on the `Http.serve` member call
  (`src/transpile/call.rs:16-33`), and the native-only refusal list at
  `src/cli/pipeline.rs:649-672` covers `Core.Native.{Database,Session,HttpClient,Mail}` but NOT
  `Core.Native.Http`. The `E-IMPORT-NATIVE-MEMBER` hint (`src/checker/program/imports.rs:148-166`)
  actively recommends the bypass spelling. Fix: add `Core.Native.Http` to the native-only
  transpile refusal path (same mechanism as its four siblings) — test-first: the panel's repro
  program as a failing test asserting exit 1 + `E-TRANSPILE-SERVE` (or the sibling refusal code —
  match whichever the four siblings emit; consistency over novelty), plus the negative control.
  Record a KNOWN_ISSUES row only if any residue is deferred; otherwise fix outright.
- **A0b. [panel P2, latent] Alias-sensitivity of the injected serve prelude.**
  `import Core.Native.Http as NH;` (any alias other than `NativeHttp`) suppresses the prelude's
  own `import Core.Native.Http as NativeHttp;` binding, detonating `E-UNKNOWN-IDENT` at prelude
  lines the user cannot see (reliance documented at `src/cli/http_serve_prelude.rs:43-47`;
  endorsed user surface per `checker/program/imports.rs:158`). Mechanism is pre-existing; this
  arc widened the trigger surface. Minimum: record as a KNOWN_ISSUES row with the repro.
  Preferred: make prelude-internal bindings immune to user import aliasing (read KNOWN_ISSUES
  §span-collision first — same territory). If the immune-binding fix needs a design ruling,
  it joins the Part 4 batch instead of being self-ruled.
- A1. **TEST-RAW-CHECKER fix** (KNOWN_ISSUES:27-61): `phg test` runs the RAW checker, so
  injected-prelude symbols raise spurious `E-UNKNOWN-IDENT`. Fix = route `phg test`'s check
  through the SAME front-end pipeline as `phg check`/LSP (the DEC-252 chokepoint —
  `cli::check_and_expand`; LSP precedent verified at src/lsp/mod.rs:518). Test-first: a failing
  test with a prelude-using program under `phg test`; count non-skips. Invariant-9 example or
  selftest update in the same change.
- A2. **G-8 microbench verdict** — on a quiet box only (per-core mpstat idle, core-pinned,
  interleaved samples, fresh docker php:8.5-cli+JIT baseline per the perf-claim rule). Record
  WIN/FLAG/OWED honestly; never `--emit` re-baseline to green it.
- A3. **Adjudication batch** (Part 4) — ask early; answers feed Phase B and the register.

**Phase B — DEC-333 perf roadmap** (MASTER-PLAN:293-297, ruled 2026-07-23), in its ruled order:
1. Json-ADT JIT slice (flips `jsonround`/`deepjson` — was IN FLIGHT; re-verify current state in
   SLICE-STATE before resuming; arena-Json flip queued as DEC-309, needs quiet box).
2. AOT full M1–M3 (`phg build --native`).
3. Interpreter campaign FULL A+C+D (NaN-boxed Value + register bytecode/typed-op specialization +
   superinstructions).
Each perf slice obeys: Invariant 11/18 (measured before/after, WIN-OR-FLAG, NO-HIDDEN-LOSS),
the perf-claim rule (docker php baseline, core-pin both sides, interleave), and lands examples +
LSP/editor rows per Invariants 9/17 where surface changes.

**Explicitly NOT in this plan** (do not drift into): build-order Waves 4–8 and the gap-programme
build — they queue AFTER Phase B unless the developer re-rules; their content is preserved in
MASTER-PLAN (Step 6) and the live gap-programme plan.

## Part 6 — Fragile implementations register (will-bite; verified in code/docs 2026-08-31)

1. Lone `cert` = `E-SERVE-TLS-INCOMPLETE`, deliberately NOT plain HTTP — do not "fix" toward D7's
   literal "iff BOTH" text (SLICE-STATE cursor; DEC-455.16).
2. `TlsServer` feature-off is an UNINHABITED ENUM — the no-plaintext guarantee is type-level;
   turning it into a struct deletes the guarantee while tests stay green (`Conn::accept`
   discharges with `match *never {}`).
3. Config errors outrank build errors (lone cert on feature-off build → `-INCOMPLETE`, not
   `-DISABLED`) — pinned by test; preserve the ordering.
4. Stream is TLS-wrapped only AFTER blocking mode + timeouts on the raw `TcpStream` — rustls fails
   on non-blocking sockets, and the timeouts bound a TLS slowloris. Both accept paths; never hoist.
5. TLS handshake runs in the WORKER (StreamOwned, first read), never the accept loop — a stalled
   client must not serialize `accept()`.
6. TLS reads the config DIRECTLY, never through `settings::resolve` (no flag ⇒ no precedence; and
   `ServeSettings` derives `PartialEq, Eq` which the TLS types don't have).
7. `src/serve/pem.rs` — hand-rolled by ruling (no 15th crate); its ONLY contract is "fewer blocks
   on malformed input, never a wrong one". Any replacement must preserve exactly that property.
8. `src/cli/role_mismatch.rs` is PURE — TTY-ness and the answer are caller-supplied; keep it that
   way so the suite, not a human, exercises the ruling. Prompt writes to stderr (DEC-220);
   non-TTY mismatch exits 1; exit 2 stays argv/usage-only.
9. The role guard sits at EIGHT run entry points via `pipeline::run_guard` — a new run-shaped verb
   must go through it (S3.4 plan amendment).
10. `E-TRANSPILE-SERVE` is keyed on the CALL (DEC-455.7); the corpus quarantine and the
    by-purpose example split (projects 18, flat RUN 198) depend on it. (A0 extends the keying —
    keep both layers consistent.)
11. §TRANSPILE-NS-REFLECT-TABLES (KNOWN_ISSUES:63-85): namespaced `get_class()`-keyed
    reflect/enum tables are an UNGATED coverage hole — divergence only [Inferred]; the next slice
    touching transpile namespaces writes the PROBE first.
12. Span-keyed rewrite maps: prelude/user SPAN COLLISION latent P1 — read KNOWN_ISSUES
    §span-collision before ANY new span-keyed map.

## Part 7 — Verification doctrine for the executing model (repo lessons, mandatory)

- **Count non-skips, never trust a filtered green**: S3.5's name-filtered run said "9 passed"
  while zero TLS tests executed; the example glob was a silent no-op for weeks (0 RUN read as
  green). Every gate claim states RUN/SKIP counts. Filters that match nothing must fail loudly.
- **Diff inspection**: ALWAYS `git --no-pager -c core.pager=cat diff --no-ext-diff` — the
  difftastic external driver yields zero grep hits and reads as "clean" (burned twice 2026-08-21).
  Use `git grep`, not `grep -rn`, for completeness sweeps.
- **Full gate before claiming done** (phorj CLAUDE.md): `source scripts/toolchain.env &&
  PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features` + clippy both feature sets +
  `cargo fmt --check` + `cargo build --release` + `cargo check --no-default-features`. Heavy runs
  get SIGKILLed under load on this box — targeted `-E` + `NEXTEST_TEST_THREADS=4` while iterating,
  full gate at commit points, one commit at a time (DEC-378: never two concurrent cargo runs).
- **Consolidation-specific checks**: after Steps 3-5, `git grep` every moved filename — every hit
  must be a pointer to the new location or the archive README (zero stale inbound refs); the
  differential corpus counts must be UNCHANGED by doc moves (projects 19 / flat 199 RUN / 19 SKIP
  per the panel's executed counts); after Step 4, `grep -c 'docs/specs/2026-'
  docs/specs/UNIFIED-SPEC.md` returns pointer-note hits only, and the 8 live specs remain in
  `docs/specs/`.
- **Commit discipline**: master only, plain `git push`, developer identity
  (`Takieddine Messaoudi <takieddine.messaoudi.official@gmail.com>`), no trailers, green
  self-contained commits, docs commits `docs:`-prefixed.

---

## Panel round 3 — 2026-09-02, frozen `6a18f71a`, range `0c982019..6a18f71a`

Transcribed IN the repo (developer ruling 2026-09-02: the round-2 report lived only in gitignored
`var/claude/` and was lost). Three fresh-context lenses, no cargo runs by the reviewers; the parent
ran the 12 tests they named (12/12 green). Full lens reports: `var/claude/panel/2026-09-02-round3/`
(gitignored — THIS table is the record).

**Verdicts:** correctness+regression **FINDINGS — 12** (1×P0, 6×P1) · completeness+blast-radius
**FINDINGS — 16** (3×P1) · security+safety-promises **FINDINGS — 7** (0×P0/P1, 6×P2, 1×P3).
**Total 35** (1×P0, 9×P1). Gate status: **OPEN**, clean counter zero.

### Security + safety-promises (7)

| # | sev | where | promise broken | status |
|---|---|---|---|---|
| F1 | P2 | `src/cli/benchmark.rs:80`, `playground/src/lib.rs:103` | `benchmark --vs-php` EXECUTES emitted PHP for `Core.SessionModule` and reports "transpile divergence" instead of `E-TRANSPILE-SESSION`; KNOWN_ISSUES:171's "would fail loudly" reasoning is contradicted for Session | **FIXED** (= C7/C8) |
| F2 | P2 | `src/ext/registry.rs`, `docs/EXTENSIONS.md` | `phg extensions` lists flag-only rows but has NO `http-server-tls` row, although its absence is the runtime refusal `E-SERVE-TLS-DISABLED` | NEW |
| F3 | P2 | `KNOWN_ISSUES.md:2481-2482`, `examples/README.md:133` | "lookaround rejected at compile — never a divergence" is false on the PHP leg | NEW (= REGEX-B class) |
| F3b | P3 | `KNOWN_ISSUES.md:2484` | claims `\d` is ASCII-only in transpiled PCRE; the helper appends `u`, both legs agree | stale doc (= C11) |
| F4 | P2 | `src/serve/framing.rs:118` | invalid `Content-Length` (`abc`, `-1`, 24-digit) served `200` instead of `400`+close; pinned as intended by `content_length_malformed_is_zero` | NEW (= C10) |
| F5 | P2 | `src/serve/settings.rs:119,149` | negative `workers`/`timeout` in `ServeConfig` silently read as unset with no `W-SERVE-CONFIG-OVERRIDDEN` notice | tracked §SERVE-CONFIG-PROVENANCE (ruled: nullable fields + range validation) |
| F6 | P2 | `cli/http_request_prelude.rs:23`, `cli/preludes.rs:64`, `ext/uri/prelude.rs:7`, `ext/debug/prelude.rs:12` | nothing-in-the-wind: `NativeHttp`, `NativeInput`, `NativeUri`, `NativeDebug` resolve in user code with NO import; the containment arm covers only `NativeHttp.registerServe` | tracked KNOWN_ISSUES:125, DEC-459 unbuilt |
| F7 | P3 | `src/cli/ladder.rs:123` | error text renders 14-space runs inside the hint | NEW |

Verified clean: `unsafe` outside `src/jit/` = 0; ladder holds on `transpile` + `build --php` for every
`Http.serve` spelling probed; no new network use outside `src/serve/transport.rs`; TLS cert-only /
no-feature / `tlsMinVersion "1.1"` all refuse with the port never bound; no secrets in the range;
deps 14 admitted / 9 default = UNIFIED-SPEC:1359; all 7 new codes have `phg explain` entries.

### Correctness + regression (12)

| # | sev | where | finding | status |
|---|---|---|---|---|
| C1 | P1 | `src/ext/regex/natives.rs:178-191`, `transpile/runtime_php.rs:1053` | `Regex.replace` replacement syntax diverges silently: `\1-`, `$$`, `$1a`, `${x}` render differently native vs php, all legs exit 0 | KNOWN_ISSUES:2503 names `\1` only; 3 of 4 rows NEW |
| C2 | P1 | `natives.rs:38,307` | possessive `a++a` on `"aaa"`: native `true` (parsed as `(a+)+`), php `false`; check-clean | NEW |
| C3 | P1 | `runtime_php.rs:995-1004` | `a$` on `"a\n"`: native `false`, php `true` — delimiter helper emits `u` without `D` | KNOWN_ISSUES:883 covered Core.Validation only; NEW for Core.Regex |
| C4 | P2 | same helpers | `findAll("a*","baaa")` → native 2, php 3 (empty-match placement) | KNOWN_ISSUES:2505 |
| C5 | P1 | `natives.rs:307` | look-around siblings forwarded unvalidated: `a(?=b)`, `(a)\1`, `\h`, `\R`, `\Z`, `a{,3}b` — native fault, php `true` | class ruled REGEX-B above; sibling list NEW |
| C6 | **P0** | `src/checker/calls/args.rs:256` | `default_fills` two-file collision reproduced at HEAD: `new Cookie(…)` at the same byte offset in two files → wrong arguments on run, --tree-walker AND php | **FIXED `53df9ef1`** — per-file span windows in the loader (`SpanWindows`); three-file differential pin + two sabotages; closes §default_fills and §span-collision |
| C7 | P1 | `src/cli/benchmark.rs:80,244` | `benchmark --vs-php` transpiles a `Core.Database` program with no ladder gate → PHP fatal, "skipping", exit 0 | **FIXED** (step 1) — gated pre-expansion on the PHP leg only |
| C8 | P1 | `playground/src/lib.rs:103` | `transpile_json` → `transpile::emit`, no ladder gate | **FIXED** (step 1) — routed through `cli::transpile_source`, the single-source chokepoint |
| C9 | P2 | `src/lsp/mod.rs:496-511` | LSP diagnostics run `test_mode=false`; `check ≡ LSP ≡ test` false | FIXED (DEC-486, step 1) |
| C10 | P2 | `src/serve/framing.rs:118` | `Content-Length: abc` → `200` body-less instead of `400`; `unwrap_or(0)` has no failure-mode evidence | NEW |
| C11 | P3 | `runtime_php.rs:992`, KNOWN_ISSUES:2485 | the documented `\d\w\s` ASCII-vs-Unicode edge does NOT exist (php `u` ⇒ UCP); the real edges are C1–C3 | NEW (doc wrong) |
| C12 | P3 | `src/cli/pipeline.rs:397-420` | `benchmark --vs-php` on a Web-only program measures handler registration only and reports a speedup | NEW, by-design per comment |

Negatives with controls: CD-31's param-default / attribute-argument gap is NOT reachable (all four
sugars are `E-DEFAULT-PARAM-EXPR`/literal-only in those positions on 3 legs); all 9 item walks carry
`Item::Test`; `phg test selftest` 11/11; the round-2 P1 (`NativeHttp.registerServe` bypass) confirmed
fixed; `import Core.Http as H` offers no alias bypass.

### Completeness + blast-radius (16)

| # | sev | where | finding | status |
|---|---|---|---|---|
| K1 | P1 | `src/lsp/mod.rs:514,534` → `cli/pipeline.rs:295,326` | both LSP diagnostic paths hard-code the non-test checker; all 3 `selftest/*.phg` squiggle `E-TEST-OUTSIDE-TESTS` | FIXED (DEC-486, step 1) |
| K2 | P1 | quartet | the 17:05 rulings (regex B, no scout port, order, perf split) exist only in this plan — no register row; MASTER-PLAN and SLICE-STATE state different "next"; DEC-457/458/459 have no MASTER-PLAN mirror | NEW — Invariant 19 |
| K3 | P1 | `SLICE-STATE.md:137-140` vs `:83-86` | intra-file contradiction on whether `phg test` still checks the raw program | NEW |
| K4 | P2 | `tests/differential.rs:2012-2062` | two `continue` skip arms, no counter, no expected-skip-set; floor is `files.len() >= 3` | tracked |
| K5 | P3 | `UNIFIED-SPEC.md:1209` | "no named arguments" true of natives only, reads as a language claim | NEW |
| K6 | P2 | `UNIFIED-SPEC.md:2117-2136` | spec names none of `E-SERVE-TLS-*`, `W-SERVE-CONFIG-OVERRIDDEN`, `E-TEST-OUTSIDE-TESTS` | NEW |
| K7 | P2 | `src/lsp/symbols.rs:153,223` | document symbols drop `Item::Test` — a named test never appears in either editor's outline | FIXED (DEC-486, step 1) |
| K8 | P2 | `checker/rewrite_pipe/walk.rs:33`, `qualify_variants.rs:67`, `rewrite_new.rs:57`, `cli/rewrite_new.rs:44` | four passes keep an inert `_ => {}` item arm outside the CD-31 ratchet; `rewrite_pipe` is pre-check and the "tests are checker-gated out" comment is false | NEW |
| K9 | P2 | `KNOWN_ISSUES.md:2854-2860` | second section still says `test_runner` calls `check_tests` without prelude expansion — false since 8a83ada6 | NEW |
| K10 | P2 | `CLAUDE.md:207`, `docs/INVARIANTS.md:39` | Invariant 3 still scoped to Expr/Stmt/Pattern; CD-31's `Item` widening absent | NEW |
| K11 | P2 | `src/cli/help.rs:8-30` | `phg --help` omits `add/install/update/remove` | KNOWN_ISSUES:2890 |
| K12 | P2 | `examples/process/README.md`, `explain/members_destructure.rs:90` | `Process.args()` drift, 20 hits; native is `arguments` | KNOWN_ISSUES:2847 |
| K13 | P3 | `examples/README.md:181` | only the html-in-trait CD-31 shape has an example; README row not updated | NEW |
| K14 | P3 | `differential.rs:2011`, `UNIFIED-SPEC.md:1684,1827,1878` | dead citations (deleted design doc, two "retired" plans in no archive) | NEW |
| K15 | P3 | `C-decisions.md:3547` | DEC-362 "build queued" while BUILT | NEW |
| K16 | P2 | `src/lsp/` | no hover/signature-help consumer of `prelude_catalog` — `cfg.port`/`Http.serve` hover empty | KNOWN_ISSUES:3056 |
| K-fact | — | `tests/serve_tls.rs` | handshake tier self-skips without `openssl`, no `PHORJ_REQUIRE_*` analogue | note |

### Disposition (feeds the ORDER ruling's step 1, "harness trust")

- **Regex cluster C1–C5 + C11** is one slice with the REGEX-B build: compile-time literal validation on
  the transpile leg, `D` in the delimiter helper, replacement-syntax normalisation, possessive/`\h`/`\R`/
  `\Z`/`{,n}` rejection or support decided per construct, each with an `agree_out_php` case (the
  reviewer confirms no existing test can go red on C1–C5).
- **C6 P0** — DONE `53df9ef1`. **C7/C8/F1** — DONE `06e9e975`. **C9/K1/K7** — DONE (DEC-486; `check --json` raw-checker and loader `resolve_item` catch-all fixed alongside). **K16** = the prelude hover slice, still owed. **K8** partial: `src/loader/resolve.rs`'s ITEM walk is wildcard-free (DEC-486) but its `resolve_expr` still ends in `leaf => leaf`, swallowing eight `Expr` variants (`Tuple`, `NamedArg`, `NewColl`, …) — a cross-package call inside a tuple literal or a named-argument value in a library file is never mangled; the follow-up commit makes it total and adds the file to the ratchet. The Pass-1 index's `_ => continue` in `assemble.rs` is a separate named gap (KNOWN_ISSUES §LOADER-ALIAS-EXPORT — public type aliases in library packages are not exported)
  (developer directive: LSP/editors are first-class). **K4** = differential floor. **K8** = widen the
  CD-31 ratchet to the four remaining item arms. **K2/K3/K5/K6/K9/K10/K14/K15** = one docs pass, same
  commit as the register rows. **C10** needs a failing-request test before its fix (anti-bandaid).
  **C12/K11/K12/K13** small, batched.

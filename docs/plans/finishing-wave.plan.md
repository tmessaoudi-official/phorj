# THE FINISHING WAVE — consolidate the docs, tell the truth, then drive Phorj to 100%

> **STATUS: AWAITING EXPLICIT GO (2026-07-11).** This plan is presented; the developer gives the
> go or corrects the approach first (their explicit instruction). Nothing in Part A/B/C executes
> until then. This file is the durability spine — it must survive `/compact` and session resets.
> SSOT once approved; supersedes `web-spine.plan.md` / `perf-wave.plan.md` / `di-attributes.plan.md`
> as the active roadmap (those get folded + deleted per Part A).

## Decisions Log
- [2026-07-11] PRESENTED: understanding + finishing-wave structure + 3 blocking decisions surfaced
  via ask-human.
- [2026-07-11] RULED (developer, ask-human):
  - **A — Perf = multi-dimensional "better"; PARK the string/array speed-beat.** Phorj wins on
    safety/correctness/design/organization + already-won numeric; it MATCHES php speed where a beat is
    unreachable. Developer will bring in **Fable** (separate model) to attack the speed problem later.
    **All KNOWN_ISSUES park-items are resolved at the END** — once the language is otherwise complete
    and only parked items remain.
  - **B — Target = 100% VISION** (php-parity + the 35-capability beyond-PHP programme, the "and some").
    Denominator is the vision, not parity-only.
  - **C — Footguns: audit all 49 GAP-by-design one-by-one (Option 1 AND 2).** Governing rule: *do
    everything PHP does, better; take NONE of PHP's weaknesses/problems.* Per row: genuine capability →
    cover it in Phorj's better/safer form (flips toward COVERED); pure footgun/weakness → stays
    excluded by design. The audit sorts capability-vs-weakness with that decision rule.
- [2026-07-11] GO given (developer). Part A started (divergent-plan inventory done via 3 extractors).
- [2026-07-11] RULED (developer, mid-Part-A — GLOBAL DESIGN TENETS, apply to the WHOLE finishing wave):
  - **Prefer INSTANCES + mandatory `new`.** Every stdlib capability is proper `new`-able instances +
    types, not static/module-factory namespaces. (Fixes an Invariant-12 violation in shipped Core.Sql.)
  - **Nothing in the wind — leaf-or-parent import, always.** Every symbol import-gated: `import A.B.C`
    → bare `C`, OR `import A.B` → `B.C`. Never ambient. (Reaffirms [[nothing-in-the-wind-import-discipline]].)
  - **Decoupled / composable / generic / scalable / modular.** Components work together but don't depend
    on each other's construction; easy to add/extend/swap. SOLID throughout.
- [2026-07-11] RULED (developer, ask-human — Core.Sql DBAL instance design; SUPERSEDES shipped slices
  1+2 static-factory surface `Sql.query`/`Sql.select`, which get reworked in Ω-1):
  - **Entry = `new QueryBuilder("users", "u")`** — table-anchored, alias first-class. Under `Core.Sql`,
    imported leaf (`import Core.Sql.QueryBuilder` → `new QueryBuilder`) or parent (`import Core.Sql` →
    `new Sql.QueryBuilder`).
  - **Typed sub-builder per verb:** `.select([...])` → `SelectQuery`, `.insert([...])` →
    `InsertStatement`, `.update(...)` → `UpdateStatement`, `.delete()` → `DeleteStatement`. Each exposes
    ONLY its valid methods (a SELECT can't call `.values()` — compile error). Immutable threading
    (Phorj immutable-by-default: each method returns a new value).
  - **Always-alias + ambiguity error.** Every table (primary + joined) has an alias; columns qualify by
    alias; unqualified column with >1 table in play = build-time `E-SQL-AMBIGUOUS-COLUMN`; single-table
    auto-qualifies (bare `id` still fine). A Phorj upgrade over PHP's silent ambiguity.
  - **Decoupled dialect (auto at execute, NOT build).** Builder is dialect-agnostic; `.toQuery()` → a
    portable immutable `Query` value (sql template + binds); `new Db(SqliteConfig(...)).execute(q)`
    renders `?`-vs-`$1`/LIMIT/quoting automatically per the connection's dialect. Builder stays offline-
    buildable + testable + `new`-able (NOT born from a connection — that coupling was challenged + rejected).
    Dialect-SPECIFIC features (PG `RETURNING`, MySQL `ON DUPLICATE KEY`) = later LADDER item / `.raw()`
    escape, parked — not forced now.
  - **Raw queries:** `new Query(sql, [binds])` (instance-consistent; both paths produce a `Query`).
  - **Joins:** `.join/.innerJoin/.leftJoin("orders","o").on("u.id","=","o.userId")`.

---

## 0. WHAT THE DEVELOPER ASKED (verbatim intent)
1. Merge/include **everything** into MASTER-PLAN + UNIFIED-SPEC. Nothing missing, nothing
   duplicated, everything true.
2. Delete the divergent plans/specs afterward.
3. Update the cursor + the percentage + the perf to be **100% true** (verified, not asserted).
4. Define **ONE very big wave** to actually finish everything — the whole language, the perf,
   everything — to reach **100%**. Don't stop until finished (context auto-compacts;
   session-remember carries continuity).
5. **The absolute rules:**
   - Phorj does **everything PHP does, and some** (superset).
   - Everything Phorj does is **better** — faster / more organized / best-practice / craftsmanship /
     SOLID + all principles. ("Better" is MULTI-DIMENSIONAL: speed is one axis; safety, correctness,
     organization, design are others.)
   - Where PHP does something badly (silent failure, hidden issue) and you **can't autonomously pick
     a better approach** (none easy) **or** it would **sacrifice performance/security/any major
     dimension** → **PARK it in KNOWN_ISSUES** for the developer, and keep going on everything else.
6. First: state understanding + how you'll finish, **then STOP for explicit go**.

---

## 1. CURRENT TRUE STATE (Verified against source this session, 2026-07-11)
- **Parity ≈60% · Vision ≈62% · raw row-floor ≈41%** — HEAD `af3aad3`, M-gap-matrix §4.6 + §11.4.
- **824 verdict rows** (173 SYN + 631 FN + 20 RT; net denom 665 = rows − N/A − GAP-by-design):
  - COVERED 220 · PARTIAL 76 · GAP-planned 110 · GAP-unplanned 259 · GAP-by-design 49 · N/A 110.
  - **Road to 100% parity = close 445 rows** (76 PARTIAL + 110 GAP-planned + 259 GAP-unplanned).
    The 49 GAP-by-design are deliberate PHP-footgun exclusions and are ALREADY out of the 665 denom.
- **The roadmap's own model tops out at ≈75% parity / ≈81% vision at W6 close** — it DEFERS XML,
  intl, and most of the 259-row unplanned stdlib tail. **So "finish to 100%" goes BEYOND the
  scheduled waves** — it must schedule + build the 259 unplanned rows and resolve every parked-hard
  item. This is the honest scale: an order of magnitude more than the 252-commit span that moved
  58%→60%.
- **TOP-20 parity gaps** (M-gap-matrix): #1 DB · #2 HTTP · #3 sessions · #4 sprintf (DONE) ·
  #5 filesystem breadth · #6 Unicode strings (the one inherited PHP defect DEF-016) · #7 named-args
  /variadics/spread · #8 generators/`yield` · #9 date/time breadth · #10 CSPRNG (partly done) ·
  #11 array_* long tail · #12 XML/DOM · #13 subprocess · #14 regex breadth · #15 compression ·
  #16 user attributes+reflection · #17 `__toString`/`__invoke` · #18 structured logging ·
  #19 intl · #20 math long tail + BigInt.
- **jit is a DEFAULT feature** [Verified `Cargo.toml:50`] — the §0 cursor line saying otherwise is
  STALE (one concrete instance of the truth-pass work).
- **KNOWN_ISSUES.md = 1373 lines, 31 sections** — the parking mechanism the developer wants already
  exists and is the target for Rule-2 park-items.

## 2. THE PERF RECKONING (the #1 thing needing developer input)
- The developer's OWN ratified refined mandate (2026-07-10, ask-human): Phorj **matches-not-beats**
  PHP on 20-yr-tuned string/array/collection categories, because a clean speed-WIN there needs
  reimplementing PHP's C engine natively in the JIT — **evidence-proven unreachable at reasonable
  cost** (strings 27.6× / maps 67.1× behind; boxed-value JIT built+measured+REVERTED as not-a-win).
  Phorj already WINS where structurally possible: numeric/recursion/control-flow via the unboxed JIT
  (fibrec 1.7–2.9×, intadd `#[Unchecked]` 2×).
- **"Everything faster than PHP" collides with that ruling. RULED (Decision A):** multi-dimensional
  "better" + PARK the speed-beat. Phorj matches php speed on string/array, wins on every other axis +
  numeric. The speed-beat is a KNOWN_ISSUES park-item; the developer will bring in **Fable** to attack
  it later, and **all park-items are resolved at the END** (once only they remain). The shelved
  speed-beat plan = `[[value-representation-overhaul]]` + `perf-wave.plan.md`.

---

## 3. HOW I'LL FINISH — three parts (Part C is the marathon)

### PART A — CONSOLIDATE (merge, don't concatenate)
"Merge everything" ≠ paste 231K of perf-log into a 162K plan (that would CREATE duplication). The
project's own SSOT convention (CHANGELOG + HISTORY narrative; MASTER-PLAN = live state) governs:
- **Fold DECISIONS + LIVE STATE** into **MASTER-PLAN.md** (roadmap/cursor/ledger) and
  **UNIFIED-SPEC.md** (ratified language surface). One fact, one home.
- **Move HISTORICAL EXECUTION NARRATIVE** (the perf-wave / di / web-spine session logs, the stratified
  `prior:` layers in §0) to **HISTORY.md / CHANGELOG.md**.
- **Reconcile ROADMAP.md** (root) — overlaps MASTER-PLAN → make it a thin pointer or fold.
- **THEN DELETE the divergent files** — strictly: merge → verify nothing lost (grep every decision
  ID / DEC-/UA-/Q- token appears in a surviving SSOT) → `git rm` (recoverable) → **only after the
  merge is verified complete AND the developer approved**. Candidate deletes (final list confirmed
  post-merge): `perf-wave.plan.md`, `di-attributes.plan.md`, `web-spine.plan.md`,
  `cli-name-sync.plan.md`, folded `docs/research/` drafts. NEVER delete before verify.

### PART B — TELL THE TRUTH (verify, don't assert)
- **Rebuild §0 cursor** from scratch: one tight current-state block; kill the stratified `prior:`
  accretion (that's history → HISTORY.md).
- **Recompute the percentage** as a FRESH full 824-row verdict scan at HEAD (the recompute rule —
  not a delta), each COVERED/PARTIAL re-checked against `src/`. Update §11 + cursor in one commit.
- **Verify every perf claim** against a fresh Docker `php:8.5-cli`+opcache.jit interleaved baseline
  (per the perf-claim rule); mark WIN / MATCH / PARKED honestly. No claim above [Inferred] without a
  fresh before/after.
- **Fix stale facts** (jit-default, test counts, feature lists) everywhere they appear.

### PART C — THE FINISHING WAVE (ordered sub-waves; row-detail finalizes AFTER Part B's verified gap inventory)
**Target = 100% VISION** (Decision B: php-parity + the 35-capability beyond-PHP programme). Ordered by
impact (TOP-20 first):
- **Ω-0 Footgun audit** (Decision C): walk all 49 GAP-by-design rows; per row apply *do-everything-
  better / take-no-weakness* — capability → schedule a better/safer Phorj cover; pure footgun → confirm
  excluded. Feeds the row-detail for Ω-1…Ω-6. Runs as part of Part B's verified gap inventory.
- **Ω-1 Web spine** (TOP-20 #1/#2/#3): finish Core.Sql P1 → P2 Core.Db (rusqlite) → PG/MySQL →
  HTTP client → sessions/cookies/auth. (Continues Wave D; `Bytes.format` op — the parked
  `dbc5215` decision — lands here.)
- **Ω-2 Filesystem + subprocess + logging + compression** (#5/#13/#18/#15).
- **Ω-3 String/Unicode correctness** (#6 — the inherited DEF-016 byte-length defect) + regex breadth
  (#14) + array_* long tail (#11) + math long tail + BigInt (#20).
- **Ω-4 Language surface** (#7 named-args/variadics/spread · #8 generators/`yield`+iterators ·
  #17 `__toString`/`__invoke` · #16 attributes v2 + reflection · trait §7 · DI v2).
- **Ω-5 Date/time + intl + XML/DOM** (#9/#19/#12 — the deferred tier-3 domains; intl needs an ICU
  extension story).
- **Ω-6 The 259-row unplanned stdlib tail** — the long march the roadmap never scheduled; this is
  what separates ~75% from 100%. Batched by domain, park-engine per row.
- **Ω-7 Beyond-PHP programme** (the "and some" — required for 100% VISION, Decision B): the 35
  beyond-php capabilities + the DI/reflection/ORM/routing framework stack. DI v2 (from Ω-4) feeds this.
- **Ω-8 Perf hold** — hold the won numeric ground; every string/array speed-beat stays a park-item
  (Decision A) for Fable + end-stage resolution. Re-verify no regression each sub-wave.
- **Ω-9 GA close** — spec freeze, GA-CHECKLIST, final vision-% recompute, showcase; THEN resolve the
  remaining KNOWN_ISSUES park-items (Decision A: "once the language is complete and only those remain").

### PART D — EXECUTION PROTOCOL (the marathon rules)
- **The PARK-ENGINE (wires the absolute rule to the mechanism):** for each gap row, build PHP's
  behaviour in Phorj's **better-or-equal** form. When "better" requires a design decision I can't make
  autonomously, OR would sacrifice perf/security/correctness/any major dimension → **PARK to
  KNOWN_ISSUES** (structured entry: what, why-parked, the fork for the developer) and move on. Never
  silently downgrade (Invariant 14 LADDER).
- **Per-slice gate (unchanged, non-negotiable):** full oracle `PHORJ_REQUIRE_PHP=1 cargo test
  --workspace` + clippy(both feature configs) + fmt + release build + Invariant-9 runnable example +
  byte-identity `run≡runvm≡php-8.5.8`. Commit each green slice. **NEVER push** (developer pushes).
- **Spine-sensitive slices → advisor review before commit** (green gate alone masks byte-identity P0s).
- **Recompute the % at each sub-wave close** (824-row model) → update §0 cursor + §11 in that commit.
- **Continuity:** update this plan's Decisions Log + the MEMORY `⭐⭐⭐ ON "CONTINUE"` pointer at every
  stopping point / compaction boundary, so a resumed session picks up exactly here.
- **Adjudication (Invariant 15):** genuine user-visible design forks → surface via ask-human with a
  concrete failing-program example, don't self-rule; if blocked, park + continue elsewhere.

---

## 4. THE BLOCKING DECISIONS — ALL RULED 2026-07-11 (see Decisions Log)
- **A — RULED:** multi-dimensional "better" + PARK the string/array speed-beat (Fable attacks it later;
  park-items resolved at the end once only they remain).
- **B — RULED:** target = **100% VISION** (php-parity + the 35-capability beyond-PHP programme).
- **C — RULED:** audit all 49 footguns (Ω-0) — do-everything-better, take-no-weakness; capability→cover
  in Phorj's better form, pure footgun→stays excluded by design.
- **Stated, correctable-but-not-a-fork:** merge = fold+move-history-then-delete (§Part A); delete is
  strictly merge→verify→git rm→post-approval; Part C row-detail finalizes after Part B's verified gap
  inventory; the marathon is genuinely many sessions (honest scale).

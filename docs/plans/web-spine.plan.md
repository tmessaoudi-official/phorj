# web-spine Plan (Wave D)

> The web spine is the #1 remaining PHP-parity mover. Sequence is roadmap-locked
> (`MASTER-PLAN.md` §"Next up" + MEMORY ⭐⭐⭐):
> **UA-L2 (loader unification) → W3-1 SQL DBAL → W3-2 HTTP → sessions.**
> This plan freezes the sequence + scope + invariants; each wave's deep design forks
> are adjudicated at that wave's own start (FRESH context — standing byte-identity rule).

## Decisions Log

- [2026-07-10] AGREED: This session pursues the **full web spine (Wave D)** — the perf arc is
  CLOSED (do not reopen ②/value-repr without new evidence, per MEMORY + MASTER-PLAN §0).
- [2026-07-10] AGREED: Entry point = **design pass → then build UA-L2** this session. Deep
  DB/HTTP/session design forks are DEFERRED to each wave's own start in fresh context (standing
  rule: "spine-sensitive slices → FRESH context; advisor review, not the green gate, catches
  masked byte-identity P0s").
- [2026-07-10] AGREED: UA-L2 depth = **registry-unification** (not full loader-unification). One
  data-driven `CORE_MODULES` table ({module, prelude source, member types}); the 8
  `inject_*_prelude` calls + the hand-synced `enforce_injected::module_of` table both DERIVE from
  it. Keeps the proven injection-at-chokepoint mechanism (byte-identical); adding a Core module
  (Db, HTTP expansions) becomes ONE table row. Satisfies B2-2's "reduced to loader rules" without
  rearchitecting resolution on the byte-identity spine. Full loader-unification explicitly
  deferred as higher-risk/multi-session.
- [2026-07-10] VERIFIED (not a decision, a fact confirmed for the record): the dependency-policy
  amendment admitting `rusqlite` (domain #5) + `rustls` (domain #6) is ADOPTED
  (`UNIFIED-SPEC.md` §"External dependency policy", 2026-07-03) — W3-1/W3-2 are dep-unblocked.
- [2026-07-10] ✅ **UA-L2 DONE** (unpushed): `cli::CORE_MODULES` registry + `inject_core_modules` fold
  replace the 8 `inject_*_prelude` fns; `module_of` derives from it. Byte-identity proven (corpus
  structural-equivalence incl. spans, then cut over) + full gate green (1585 unit + 144 differential
  php-8.5.8 + clippy both-configs + fmt + release). `DateTime`-not-gated inconsistency logged to
  KNOWN_ISSUES for separate adjudication. Non-blocking future question (advisor-flagged): `module_of`
  now reaches `cli::core_module_of` (checker→cli edge) — defensible (preludes+`lex_parse` live in cli);
  revisit registry home if it grows. **NEXT = W3-1 SQL DBAL** (fresh context; adjudicate draft forks
  at wave start). Guard shape for W3-1: the corpus-equivalence test was deleted by design, so the
  differential is the SOLE ongoing registry guard — good coverage (`route-constraints.phg` pins
  Http-before-Regex ordering; S2 pins `module_of`) but not total (a reorder of two *independent*
  modules wouldn't be caught — behaviorally harmless, but keep the `Core.Db` row in registry order).

## W3-1 SQL DBAL — design rulings (draft `w3-1-db-access.md` → decisions)

- [2026-07-10] Q1 (dep amendment) RESOLVED = **admit** (rusqlite/rustls adopted, UNIFIED-SPEC 2026-07-03).
- [2026-07-10] Q2 (driver) RESOLVED = **`rusqlite`** (bundled; the amendment names it; unsafe confined to
  the crate, phorj's `#![forbid(unsafe_code)]` intact).
- [2026-07-10] Q4 (param binding) RULED = **ship BOTH positional `?`/`bind` and named `:name`/`bindNamed`;
  named is the documented default** (order-independent, self-documenting, maps to PDO named params).
- [2026-07-10] Q5 (lifecycle) = interim **`Db.close` + `Db.transaction` closure** now; permanent
  `using`/`defer` scoped-release deferred to XL-019 (its own future adjudication). Not separately re-asked.
- [2026-07-10] Q3 (Sql surface) RULED = **FULL fluent builder now** (developer chose the full surface over
  the phased/Query-value options): `Sql.select([...]).from().join().where().groupBy().having().orderBy().limit()`
  + aggregates (`Sql.count`/`as`) + an operator model (`Eq`/`Gt`/`Desc`/…) — designed + tested up front. All
  compiles down to the parameterized `Query` value (injection-safety preserved). ⚠ This is XL scope for P1 —
  the build is a multi-slice effort (operator enum + builder methods + aggregate exprs + `Query` lowering).
- [2026-07-10] Q6 (error model) RULED = **`throws DbError` + try/catch** (CATCHABLE typed exception — my
  "uncatchable fault" rec was wrong and the developer caught it). Maps to PDO `ERRMODE_EXCEPTION` →
  catchable `\PDOException`; checker-enforced (fixes PHP's unchecked `@throws`). `fetchOne` still returns
  `Map?` for the legitimate no-row case (a `null`, not an error).
- [2026-07-10] Q7 (constructor) RULED = **true overload on a typed config**: `Db.open(string dsn)` +
  `Db.open(SqliteConfig cfg)`, dispatched on arg TYPE (Phorj parameter-overloading). Adds a per-driver
  config type (`SqliteConfig`, later `PostgresConfig`). A single `Db.open(string)` for both DSN and path
  was rejected — identical signature = overload collision (won't compile).

## Formal Plan

### Wave-D scope map (frozen sequence; only UA-L2 is build-ready this session)

| Step | Item | Status | Dep | Fork status |
|------|------|--------|-----|-------------|
| 1 | **UA-L2** injected-prelude → registry unification | BUILD NOW | none | RULED (B2-2), depth ruled registry-unification |
| 2 | **W3-1** SQL DBAL (`Core.Sql` Tier-A pure builder → `Core.Db` Tier-B exec; SQLite→PG→MySQL, sync) | designed (draft) | rusqlite (adopted) | design forks → adjudicate at wave start |
| 3 | **W3-2** HTTP (client `rustls`; server = `phg serve` shipped) | designed (draft) | rustls (adopted) | design forks → adjudicate at wave start |
| 4 | **sessions** | not designed | — | design at wave start |

Deferred/adjacent: UA-L7 `Core.Dotenv` (Wave-D adjacent), W4-10 XML.

### UA-L2 — build-ready spec (registry-unification depth)

**Goal:** collapse the hand-synced injected-Core-module machinery into one data-driven registry so
the DB/HTTP waves add a Core module as data, not as edits in ~4 hand-synced places.

**Current state** (verified in code):
- 8 chained `inject_*_prelude` fns: `json, rounding_mode, option, result, http, regex, secret, time`
  (`src/cli/mod.rs:368-1118`), each parsing embedded Phorj prelude source, gated on its `Core.<M>`
  import, injecting items via `Cow<Program>`.
- Hand-synced `enforce_injected::module_of` (`src/checker/enforce_injected.rs:90-109`): 8 entries
  mapping injected member type → owning module leaf; reused by qualified-construction dispatch in
  `calls.rs` + `expr.rs`.
- Downstream special-cases (part of the same discipline): `collapse_injected_type_qualifiers`,
  `resolve_variant_imports` (these stay; they operate post-injection and are not per-module
  hand-synced tables — confirm during build).

**Target:**
- One `CORE_MODULES: &[VirtualModule]` table (`{ module: &str, src: &str, types: &[&str] }`).
- The 8 `inject_*_prelude` calls become a single loop over the table (each row: no-op unless its
  `Core.<module>` is imported / a member-import pulls it — the existing gating logic, factored).
- `module_of` derives from the same table (reverse lookup `type → module`).
- Adding `Core.Db` = one new `VirtualModule` row + the prelude `.phg` source.

**Row schema (advisor-refined — TWO separate per-row concerns, never fused):**
```
struct VirtualModule {
  module:     &[&str],        // e.g. ["Core","Http"] — gate + module_of value
  src:        Option<&str>,   // prelude source; None for attribute-only modules (DI/Runtime*)
  member_gated: bool,         // true = imports_module_or_member; false = exact-module-only (json/regex/secret)
  bare_types: &[&str],        // module_of contribution — EXPLICIT, seeded to today's 8 entries (Time = Duration/Date/Instant, NOT DateTime)
}
```
- **shadow-check names** derive from the PARSED prelude `src` (all four Time classes incl.
  `DateTime`) — so a user's own `DateTime` still shadows. Do NOT reuse `bare_types` for this.
- **`module_of`** derives from `bare_types` only (reverse map type→module leaf). Seeded to reproduce
  the current 8 entries EXACTLY (the "T ≠ module-leaf" derivation is REJECTED — it would wrongly add
  `DateTime`→"Time", newly gating bare `DateTime` = behavior change).
- `injected=true` marking: applied to any injected **Enum** item (json/rm/option/result); classes
  (regex/secret/http/time) are never marked — uniform rule reproduces current behavior.
- **http `respond`-bridge** stays a documented conditional post-hook (inject `respond` iff `handle`
  present and no `respond`) — the one honest residual special-case ("reduced to", not "eliminated").

**Acceptance (B2-2 + G-2 + advisor verification upgrade):**
- **THE GATE = corpus-equivalence** (stronger than the S2 subset): keep the 8 `inject_*_prelude`
  fns, add a throwaway test asserting `old_chain(prog) ≡ new_fold(prog)` structurally (PartialEq or
  `format!("{:?}")`) for EVERY example in the corpus (~146). Injection is early + downstream is
  deterministic ⟹ equal injected Programs ⟹ equal end-to-end. Only AFTER this passes: cut over and
  delete the old fns + the throwaway test.
- Full correctness gate green: `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo test
  --workspace --features jit` + clippy(both feature configs) + fmt + release build.
- No new `Op`; front-end-only change; every backend still sees the same expanded AST.
- Advisor byte-identity review BEFORE commit (spine-sensitive standing rule).

**Risk / worst failure modes (advisor-surfaced):**
1. **ORDER dependency (load-bearing, concrete):** `HTTP_PRELUDE` transitively `import Core.Regex`
   (line 556) and `inject_http` runs BEFORE `inject_regex` — that transitive import is what triggers
   Regex-class injection for `Router.constraintOk`→`Regex.compile`. The fold MUST preserve exact
   chain order json→rm→option→result→http→regex→secret→time. Verify the corpus has an
   http-route-with-regex-constraint example that does NOT explicitly `import Core.Regex`; add one if
   missing (else a future reorder passes every test and breaks real usage).
2. **Fused row concerns:** using one name list for shadow-check + module_of silently changes
   behavior (DateTime is the proof). Mitigated by the two-field schema above.

**Discovered finding (log to KNOWN_ISSUES, separate adjudication — NOT fixed here):** bare `DateTime`
is not gated by `module_of` while its sibling `Date` is — a latent inconsistency in the injected-type
discipline. UA-L2 preserves it byte-identically; whether to gate `DateTime` too is a separate ruling.

### Definition of done (per invariant 9 + standing rules)
- UA-L2: no new example needed (internal refactor, byte-identity-neutral) — but the S2 example
  corpus must stay byte-identical run≡runvm≡php-8.5.8.
- `cargo build --release`; report `target/release/phg`.
- Update `MASTER-PLAN.md` §0 cursor + this plan's Decisions Log on close.
- Commit green (never push).

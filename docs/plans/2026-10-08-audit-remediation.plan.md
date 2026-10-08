# audit remediation (2026-10-08) Plan

> Source: the 2026-10-07 full read-only audit (architecture / craftsmanship / docs+SSOT / Claude-config sync /
> invariants+safety / strictness+neutrality), 6 lenses, 89 raw findings deduplicated to 31 items in 6 themes.
> Report (gitignored): `var/claude/review-2026-10-07/MASTER-REPORT.md`, raw lens files in `raw/`, probes beside them.
> Audited at `ea5cb03a`. Every ruling below was made interactively by the developer via `AskUserQuestion`
> (four rounds); the language rulings are register rows DEC-562..566, the open language questions DEC-567.

## Decisions Log
<!-- Stamps were read from the clock when each ruling was recorded (var/claude/review-2026-10-07/DECISIONS.md). -->
- [2026-10-07 23:55] AGREED: Target architecture for phorj's OWN code = the pragmatic hybrid — pipes-and-filters front end, Clean Architecture's dependency rule (enforced by a workspace crate split, last), hexagonal ports only where phorj touches the world (runtime natives), strategic DDD (bounded contexts, lift/transpile as anti-corruption layers, a glossary). CQRS only as a future query-based incremental LSP; tactical DDD, command buses and event sourcing are do-not-adopt (DEC-562).
- [2026-10-07 23:55] AGREED: Two separate ideas — phorj's source code is held to that architecture; phorj the LANGUAGE stays architecture-neutral and puts its strictness into catching more errors before run/compile. Boundary test: a correctness rule is one whose violation can produce a wrong result, a crash or an ambiguous meaning; an architecture choice rejects a correct, unambiguous program purely for its structure and stays out of the language (DEC-562).
- [2026-10-08 01:57] AGREED: A1 — fix first: validate every header line in `Response.serialize()` (the single chokepoint every leg and construction path reaches); an empty header list yields exactly one blank line; test-first plus web examples.
- [2026-10-08 01:57] AGREED: A2 — close the catch-all class in every total Expr/Stmt walker with explicit arms plus the `src/ast/leaves.rs` macros, and add a ratchet test with a CD-row allow-list; ship a tuple-with-`new` example.
- [2026-10-08 01:57] AGREED: A3 (native `Json.parse` depth limit 512 with PHP's null semantics) and A4 (positive phorj-checkout detection for `phg build --target`, per-stub sha256 re-verified on every cache hit, https-only registry, `install` honours an existing lock) ride in the same fix slice as A1.
- [2026-10-08 01:57] AGREED: A5/A6 — LSP per-message `catch_unwind` with an internal-error reply, a panic hook printing an ICE banner, a `Content-Length` cap, a std-only mutation-smoke test (truncate/splice every example, `check` must not panic); the 38 JIT helpers that take a raw `*mut UbCtx` become `unsafe extern "C" fn` with `# Safety` docs. cargo-fuzz (MASTER-PLAN Tier-3 item 22, beside DEC-246) is left for a dependency-policy ruling.
- [2026-10-08 01:58] AGREED: B — staged architecture roadmap. Stage 1 (S each): move the Core-module registry out of `cli` and fix the three `INJECTED_SPAN_BASE` imports; use-case functions own the ladder and reified-compile gates with `transpile::emit`/`compiler::compile` made `pub(crate)`; a `SpanKey` newtype. Stage 2 (M–L): parser-assigned NodeId, a `CheckedProgram` (core AST plus NodeId→type) fed to all three backends, a separate lowering stage with one pass table and a shared fold, one `RuntimeCx` passed to natives, an LSP checked-document cache. Stage 3: the crate split.
- [2026-10-08 01:58] AGREED: C — one S batch: single-source the re-typed fault bodies (interpreter/VM payload bodies, PHP-leg literals, "map key not found"); one shared PHP-oracle resolver for the 12 test files; the size gate auto-tightens, fails on dead rows and covers `tests/`; delete the `phg rewrite-new` codemod; CI gains `--all-features` and `--no-default-features` jobs. cargo-mutants on the value kernels is queued.
- [2026-10-08 01:58] AGREED: D — fix the facts now (MASTER-PLAN §0 becomes a pointer, DEC-558..561 mirrored with a literal-grammar section in UNIFIED-SPEC, SLICE-STATE rewritten as an overwrite-only cursor, Invariant 1's exceptions restated as three explicit classes, the counts and ARCHITECTURE.md map corrected, a glossary reserving "oracle" for the interpreter, done plans archived); then split each SSOT file into a live head plus archive and amend Invariant 19 to "read the heads, grep the archives".
- [2026-10-08 01:58] AGREED: E — this repo fixes all of its own Claude config in one commit (the 3 reviewer agents, phg-qa-sweep, phg-lenses, unparseable Status rows, memory index, expertise overrides, leftover files); agent-definition edits are shown as a diff for the developer's OK first. The framework-health session was told to drop the phorj items from its step 9.
- [2026-10-08 02:00] AGREED: Language rulings Q1, Q8, Q2, Q10 — register rows DEC-563, DEC-564, DEC-565, DEC-566. Q3–Q7 and Q9 stay PENDING (DEC-567).
- [2026-10-08 02:04] AGREED: Persist this plan, the DEC rows and the MASTER-PLAN/SLICE-STATE pointers in one docs-only commit; keep the report in `var/claude/review-2026-10-07/`; then start the P0 fix slice (rows 1–4) test-first.
- [2026-10-08 07:29] ASSUMED (review): Row 3 (A4) shipped as two commits, bundle side then package-manager side — because each guarantee gets its own red, green and sabotage pass and a smaller hook run. Alternatives: one commit for the row.
- [2026-10-08 07:29] ASSUMED (review): A source cross-build requires the running phg to sit under <dir>/target/ of a Cargo.toml whose [package] is named phorj; an installed copy (cargo install, or CARGO_TARGET_DIR elsewhere) takes the download branch — because only the checkout's own build guarantees the stub matches the host binary. Alternatives: the name check alone; an explicit --from-source flag.
- [2026-10-08 07:29] ASSUMED (review): phg install follows the cargo rule: a git/registry dependency stays at its locked commit while its phorj.json entry still accepts it, a changed entry re-resolves that dependency alone, path dependencies are not pinned, and only phg update resolves without the lock — because a drift-check alone would fail install on every newer registry release. Alternatives: hard-fail on any drift; a --locked flag.
- [2026-10-08 07:29] ASSUMED (review): CD-32: a classifier whose fallback is the safe answer keeps its catch-all only under a "// catch-all exempt (CD-<n>): <reason>" comment block, which the ratchet checks; expr_is_never carries the only two — because listing 34 non-diverging variants adds noise without adding a decision. Alternatives: explicit arms for every variant; leaving totality.rs off the ratchet.
- [2026-10-08 07:29] ASSUMED (review): The slice-close panel runs on opus, the model pinned in the three reviewer agent definitions (no override-table entry) — per the autonomous-mode model policy. Alternatives: sonnet.
- [2026-10-08 08:39] ASSUMED (review): panel round 1, cty.rs: `null` and `new List<T>()` resolve to CTy::Other, `a ?? b` takes the RHS type when the LHS resolves to Other, and the nine pre-expanded forms keep their error under named arms — because the catch-all Err split the VM from the tree-walker on `(null ?? 4) + 1`, and Other alone is still refused (arithmetic lowers only to typed ops; measured). Alternatives: leave the catch-all and add only the Null arm (does not fix it).
- [2026-10-08 08:39] ASSUMED (review): panel round 1, transpile/kinds.rs keeps `_ => OpKind::Other` — because Other routes to the runtime helper, so an unknown form is slower but never wrong (kinds.rs is also at exactly 300 lines). Alternatives: explicit arms (soft-cap breach, no behaviour change).
- [2026-10-08 08:39] ASSUMED (review): panel round 1, a `target` that is a symlink is not a checkout (download branch) — because the resolved target must sit directly inside the resolved cwd, and a link could point into another checkout. Alternatives: allow a link that resolves inside the same cwd (rare, more code).
- [2026-10-08 08:39] ASSUMED (review): panel round 1, the pm JSON parser shares limits::MAX_JSON_DEPTH (511 nested containers) — because one limit is easier to reason about than a pm-specific one, and phorj.json/lock/index never nest past 3. Alternatives: a smaller pm-only limit.
- [2026-10-08 08:39] ASSUMED (review): panel round 1, the serve body spill uses ONE owner-only directory per process (OnceLock) with create_new files, not a directory per spill — because spills are per-request and frequent, and the leak-until-exit lifetime is unchanged (KNOWN_ISSUES). Alternatives: a directory per spill; tempfile-style unlink-on-drop.
- [2026-10-08 08:39] ASSUMED (review): panel round 1, the three header shapes outside DEC-363 (leading whitespace, no colon, handler-supplied Content-Length/Transfer-Encoding) are disclosed and recorded PENDING as DEC-568, NOT built — because widening the rejected set is a language/API decision (Invariant 15). Alternatives: fault on all three now.

## Formal Plan

Order is the developer's: the defect slice first, then the S batches, then the staged architecture work; the language
rulings queue behind their prerequisites. Every code row is test-first (red for the stated reason → green → sabotage
check that proves the gate runs) and carries an Invariant-9 example where it changes behaviour.

1. **A1 — Response header guard at `serialize()`** (`src/cli/http_prelude.rs`): `HeaderSafety.requireName/requireValue`
   over every `headerLines` entry; head built so an empty list yields one blank line; failing cases first (CRLF via the
   public constructor, empty list); KNOWN_ISSUES' "FIXED" entry corrected to name the constructor path.
2. **A3 — JSON depth** (`src/limits.rs`, `src/ext/json/parser/mod.rs`, the JIT twin): depth 512, `null` past it like
   `json_decode`; example at depth 513 on all three legs.
3. **A4 — cross-build + package manager trust** (`src/bundle/cross.rs`, `src/pm/registry.rs`, `src/pm/ops.rs`).
4. **A2 — catch-all walkers** (`src/checker/rewrite_new.rs`, `src/checker/program/walk.rs`,
   `src/checker/qualify_variants.rs`, `src/cli/rewrite_new.rs` or its deletion in row 9, the Stmt walkers) + ratchet test.
5. **A5/A6 — panic net + JIT signatures** (`src/lsp/mod.rs`, `src/main.rs`, `src/jit/handles/`).
6. **E — Claude config sync** (agents diff shown first).
7. **D — fact fixes** (SSOT quartet, README, Cargo.toml comment, ARCHITECTURE.md, INVARIANTS.md, CLAUDE.md counts).
8. **D — live-head/archive split + Invariant 19 amendment.**
9. **C — craftsmanship batch.**
10. **B stage 1**, then **B stage 2** rows (opened when stage 1 lands), then **B stage 3**.
11. **Language rulings**: uncoded-diagnostic coding (prerequisite of DEC-565) → DEC-563 → DEC-564 → DEC-565 → DEC-566.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | A1 Response header guard at serialize, empty-list head, KNOWN_ISSUES correction — `fix(http): check every Response header line in serialize(), so the public constructor cannot inject` | S | done | 2a20ee85 | src/cli/http_prelude.rs examples/web/** KNOWN_ISSUES.md |
| 2 | A3 native Json.parse depth 512 with PHP null semantics (511 parse, 512 None, as json_decode) — `fix(json): Json.parse rejects 512 nested arrays/objects like json_decode, bounding the recursion` | S | done | 5794d297 | src/limits.rs src/ext/json/** examples/guide/json.phg |
| 3 | A4 in two commits — 395b52ef `fix(build): cross-build from source only in a phorj checkout running its own phg, and re-verify every cached stub`, then `fix(pm): phg install reproduces phorj.lock, so a moved tag or a vanished version stops until phg update` | M | done | af389c5b | src/bundle/** src/pm/** src/cli/pm.rs src/cli/help.rs tests/pm.rs |
| 4 | A2 explicit arms in every total Expr/Stmt walker plus catch-all ratchet test (CD-32) — `fix(checker): `new` inside a tuple panicked every backend; close the last six catch-all walks under the ratchet` | M | done | 691f191b | src/checker/** src/cli/rewrite_new.rs src/cli/rewrite_new_tests.rs src/ast/leaves.rs examples/guide/tuples.phg |
| 5 | A5/A6 LSP panic net, panic hook, Content-Length cap, mutation-smoke test, unsafe extern JIT helpers | M | todo | - | src/lsp/** src/main.rs src/jit/** tests/** |
| 6 | E Claude config sync (agents, phg-qa-sweep, phg-lenses, Status rows, memory, expertise, leftovers) | S | todo | - | .claude/** docs/plans/** |
| 7 | D fact fixes across the SSOT quartet and reference docs | S | todo | - | docs/** README.md Cargo.toml CLAUDE.md FEATURES.md KNOWN_ISSUES.md |
| 8 | D live-head/archive split of the SSOT quartet, Invariant 19 amended | L | todo | - | docs/** CLAUDE.md |
| 9 | C craftsmanship batch (fault bodies, shared PHP resolver, size gate, rewrite-new deletion, CI feature jobs) | M | todo | - | src/value/** src/interpreter/** src/vm/** src/transpile/** tests/** scripts/** .github/** |
| 10 | B stage 1 (Core-module registry out of cli, use-case gates, SpanKey) | M | todo | - | src/cli/** src/checker/** src/loader/** src/transpile/** src/compiler/** |
| 11 | B stage 2 (NodeId, CheckedProgram, lowering stage, RuntimeCx, LSP cache) | L | todo | - | src/** |
| 12 | B stage 3 workspace crate split | L | todo | - | Cargo.toml src/** |
| 13 | Code every uncoded checker and parser diagnostic plus a ratchet (prerequisite of DEC-565) | M | todo | - | src/checker/** src/parser/** src/cli/explain/** |
| 14 | DEC-563 literal and const native precondition table | M | todo | - | src/native/** src/checker/** |
| 15 | DEC-564 demote the three file-layout laws to W- lints | S | todo | - | src/loader/** src/cli/explain/** |
| 16 | DEC-565 site attribute Allow(code, reason) plus DEC-360 strict flag and explain text | M | todo | - | src/checker/** src/cli/** |
| 17 | DEC-566 opt-in in-source layer rules | M | todo | - | src/loader/** src/checker/** |
<!-- /progress-block -->
### Blocked
### Needs input
- DEC-567 (a)–(f): six language questions recorded PENDING under Invariant 15, each with its probe program in
  `var/claude/review-2026-10-07/raw/6-strictness-neutrality.md` — ask before building any of them.
- cargo-fuzz as a dev-only out-of-workspace crate (MASTER-PLAN Tier-3 item 22): needs a dependency-policy ruling.
### Needs research
### Fragile
- Rows 10–12 change module paths that `tests/*.rs` and `playground/` import; each stage lands green on its own.
### Known issues
- The audit itself executed probes only on the release binary built 2026-10-06 16:52 (current for `src/` at
  `ea5cb03a`); no cargo build, test suite or differential run was part of it.

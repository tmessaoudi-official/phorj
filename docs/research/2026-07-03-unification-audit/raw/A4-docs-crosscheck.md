# A4 — Docs ↔ Code Bidirectional Cross-Check (unification audit)

Date: 2026-07-03 · HEAD: `0691228` (clean tree) · Auditor: A4 (docs/code consistency)
Method: two independent inventories (Appendix I = doc claims, Appendix II = code ground truth,
each built before comparison), then diffed both directions. Every finding carries an evidence
grade. Binary tests ran against `target/release/phg 0.5.1-alpha.1` (matches HEAD's Cargo.toml
version; parser source at HEAD agrees with observed binary behavior on every tested point).

Executive counts: **12 false-claim clusters (≈45 doc:line sites)** · **7 undocumented-feature
gaps** · **16 contradiction pairs** · **3 prior-research corrections** · percentage model:
denominator EXISTS and is well-defined, but the ≈58% baseline is now a stale lower bound.

---

## A. FALSE CLAIMS — docs assert X, code/binary refutes it

### A1. "Zero external dependencies / std-only / no external crates" — FALSE (P0 for the merge)
Sites presenting it as current:
- `README.md:5-6` — "std-only, with **zero external crates**"
- `README.md:89` — "std-only — **no external crates** to download"
- `README.md:311-312` — "**no third-party runtime dependencies** — see THIRD-PARTY-NOTICES.md" — this sentence links to the very file that lists the deps
- `FEATURES.md:84` — "**Zero external runtime dependencies** — std-only Rust"
- `VISION.md:59` — "Std-only, zero-dependency core … no supply-chain surface"
- `CONTRIBUTING.md:15` — "`cargo build` # std-only — no dependencies to fetch"
Refutation: `Cargo.toml` `[features] default = ["crypto","regex","signals","green"]` pulling
`argon2`, `regex`, `ctrlc`, `corosensei` by default; `THIRD-PARTY-NOTICES.md:15-18` lists all 4
plus transitive deps (`:20-21`). — **[Verified: read Cargo.toml + THIRD-PARTY-NOTICES at HEAD]**
Note: `MASTER-PLAN.md:1012` already rules "'zero deps' is FALSE and stays deleted" (W6-1 rewrite
plan) — the plan knows; the shipping docs still lie today.

### A2. `import type` presented as current/stable syntax — FALSE
The grammar was removed 2026-07-03 (S0, `11a6c71`). Live binary test:
`import type Acme.Geometry.Rect;` → `parse error at 2:8: expected a module path segment, found
TypeKw`. — **[Verified: ran phg check on a probe file]**
Stale sites presenting it as the current form:
- `FEATURES.md:46` — ✅ row "Cross-package types — `import type Pkg.Path.Type [as A]`"
- `STABILITY.md:20` — lists `import type` under **stable**
- `docs/INVARIANTS.md:120,133/135` — "User / library types — `import type Pkg.Path.Name [as Alias];`" (INVARIANTS is a read-before-backend-work doc — high blast radius)
- `KNOWN_ISSUES.md:211, 254, 333, 335` ("the terminal `import type` is the shipped form"), `615`
- `docs/MILESTONES.md:74`
- `examples/README.md:154` — "consumed … via `import type Acme.Geometry.Rect;`" — the actual file `examples/project/shapes/src/main.phg:11` now uses plain `import Acme.Geometry.Rect;` **[Verified: grep]**
- Stale prose comments in src (flagged, lower priority): `src/parser/items.rs:163` doc-comment, `src/native/mod.rs:428`, `src/loader/resolve.rs:8,97,121,544`, `src/loader/mod.rs:25,303,516,570,584,617`, `src/cli/explain.rs:179`
CHANGELOG/HISTORY occurrences are historical records — acceptable, keep.

### A3. `fn` lambdas listed as stable — FALSE
- `STABILITY.md:24` — "lambdas (`fn`)". Binary test: `var f = fn(int x) => x + 1;` → parse
error (`fn` is an ordinary identifier, keyword removed in the naming overhaul, cf.
`docs/HISTORY.md:94` "`fn`→`function`"). — **[Verified: ran phg check on a probe file]**

### A4. Dead CLI verb names (`lex`, `disasm`, `bench`, `fmt`) — FALSE
Real verb set (from `phg --help` at HEAD): run runvm check parse tokenize transpile lift
disassemble benchmark build vendor serve test format explain — plus `lsp` and `debug` which are
accepted (short usage line + `src/main.rs:74-75,244,256`) but omitted from the long help.
`phg fmt <file>` → usage error. — **[Verified: ran --help + phg fmt]**
Non-historical sites using dead names:
- `README.md:128` `lex` · `:130` `disasm` · `:131` `bench` · `:136` `fmt` (while `README.md:203-209` correctly says `phg format` — self-contradiction)
- `FEATURES.md:67` — "check/parse/lex"
- `STABILITY.md:69` — declares `fmt`, `bench`, `disasm`, `lex` **stable** (they don't exist under those names)
- `docs/GA-CHECKLIST.md:16,33,42,44` — `bench`/`disasm`/`fmt`
- `ROADMAP.md:111,147` — "`phg fmt`"
- `CONTRIBUTING.md:66` — "`phg bench <file>`" (actively wrong contributor instructions)
- `docs/ARCHITECTURE.md:54` — lex/disasm/bench
- `docs/MILESTONES.md:19` lex · `:109` bench · `:112` disasm · `:126` "bench --vs-php"
- `examples/README.md:136` — "`phg bench --vs-php`" (the flag exists; the verb is `benchmark`)
- `editors/vscode/README.md:21`, `editors/phpstorm/README.md:12,41,45`, `editors/README.md:4` — "`phg fmt`"
- `examples/bench/workload.phg` comments (propagated into `playground/web/examples.js:10`) — "`phg bench …` / `phg disasm …`"
CHANGELOG occurrences are historical — acceptable.

### A5. `E-TRANSPILE-CONCURRENCY` does not exist — FALSE code-name in shipping docs
- `README.md:277-279` — "the PHP leg is a hard error (`E-TRANSPILE-CONCURRENCY`)"
- `MASTER-PLAN.md:84, 1160, 1271` — treats it as the ruled, in-force code
Refutation: `grep -r 'E-TRANSPILE-' src/` → **zero** matches; the actual code is
`E-CONCURRENCY-NO-PHP` (`src/transpile/expr.rs`, `src/cli/explain.rs`, `src/chunk.rs`,
`src/ast/mod.rs`, `src/value.rs`); `KNOWN_ISSUES.md:757` uses the correct name. —
**[Verified: grep both codes across src]**
Related: `E-TRANSPILE-DB` / `E-TRANSPILE-MONGO` / `E-TRANSPILE-CASE-UNICODE` appear only in
plans/drafts as *future* names (fine); `E-RETIRED-SYNTAX` (`MASTER-PLAN.md:1264`) has **zero**
src occurrences — it is the *planned* code for the pending `->` reject and must be labeled as
planned, not existing. **[Verified: grep]**

### A6. FEATURES.md 🔲/🚧 markers on SHIPPED features — FALSE pendings
- `FEATURES.md:53` — traits "🔲 future … not yet a user-facing surface". Binary test:
  `trait Greeter { function hi(): string {…} } class A { use Greeter; }` → **type-checks clean**;
  lexer keyword `src/lexer/mod.rs:835`. — **[Verified: ran phg check]** (MASTER-PLAN §7-OPEN keeps
  trait *disposition* as the one open ruling — the construct exists regardless; FEATURES stating
  "not yet a user-facing surface" is factually wrong.)
- `FEATURES.md:55` — concurrency "🔲 M6". Shipped: `green` feature + `corosensei` dep
  (Cargo.toml), spawn/channels entries CHANGELOG:117/131, README:277 documents the shipped
  surface + its oracle quarantine. — **[Verified: Cargo.toml + CHANGELOG; construct not binary-tested this session]**
- `FEATURES.md:79` — PHP→Phorj migration "🔲 M8". Shipped: `phg lift` in `--help`, `src/lift/`
  (9 files), `examples/lift/`. — **[Verified: --help]**
- `FEATURES.md:80` — "Editor/LSP, formatter 🔲 M7". Shipped: `phg lsp` (`src/main.rs:244`),
  `src/lsp/`, `editors/` integrations; `phg format` in help; FEATURES:74 itself marks formatter ✅
  (self-contradiction in the same table). — **[Verified: --help + main.rs]**
- `FEATURES.md:38` — 🚧 row including "Set union/intersection" as not-done. Shipped:
  `src/native/set.rs:180/195/210/251` (`union`, `intersection`, `difference`, `isSubset`) +
  `examples/guide/set-ops.phg`. (The P3 *additive* set: isSuperset/symmetricDifference/isDisjoint/
  map/filter is genuinely absent — grep 0 — matching the approved-TODO plan.) — **[Verified: grep native/set.rs]**
- `FEATURES.md:54` — "🚧 M5" modules/packages vs `docs/MILESTONES.md:255` "M5 ✅ COMPLETE".

### A7. GA-CHECKLIST "Missing: an LSP" — FALSE, and it skews the GA %
`docs/GA-CHECKLIST.md:16` — rock 2 at 70% justified by "**Missing: an LSP**". The LSP exists
(`phg lsp`, `src/lsp/`, both editor READMEs route through it). Rock-2's basis is stale → the
≈57% GA figure is computed from a false premise (understates). — **[Verified: --help + src/lsp]**

### A8. ARCHITECTURE trait-grep claim — stale verification statement
`docs/ARCHITECTURE.md:88` — "There is no Backend trait yet (grep 'trait ' src/ = 0)". The grep
is no longer 0: `src/serve.rs:59 pub trait Transport`, `src/debug.rs:62 pub trait DebugFrontend`,
`src/green/exec.rs:40 pub trait Suspend` (+ test files). The substantive claim (no *Backend*
trait) remains true; the embedded verification command is now false. — **[Verified: grep]**

### A9. KNOWN_ISSUES stale points
- `KNOWN_ISSUES.md:150` — "reference them as `\Main\Obj`" — variant renamed `Object`
  (Obj→Object/Arr→Array/Str→String rename shipped with variant-qualification A1/A2/B). —
  **[Inferred: rename commits + E-INJECTED-VARIANT-BARE present in src; PHP emission not re-run this session]**
- `KNOWN_ISSUES.md:338-341` — flat "not yet implemented" list contradicted by the same file:
  exceptions (L338 vs L457-460 shipped), overloading/traits/accessors (L339 vs L530/L248/L383),
  expression-`match` (L341 vs L1059-1063 "completed in M11"). Only operator overloading,
  sized ints, statement-position `match` remain genuinely pending. — **[Verified for traits (binary); Inferred for the rest from same-file sections]**
- `KNOWN_ISSUES.md:678` — function type written `(int) -> int` (canonical `=>`); return-arrow
  `-> T` signature prose at L62, 116, 580, 590, 597, 675, 869, 896-897, 1125 — legal-but-retired
  form used as the canonical presentation. — **[Verified: grep]**

### A10. Line-count / count claims gone stale
- `MASTER-PLAN.md:264` — KNOWN_ISSUES "1125 lines" → actual **1133** (`wc -l`). W0R:227 says 1133 (right).
- `MASTER-PLAN.md:319` — differential.rs "2966 lines" → actual **3308**.
- `MASTER-PLAN.md:232` — explain "registry count 270 (not ~166)" → measured **200 unique E-/W- code
  strings** in `src/cli/explain.rs`; 270 not reproduced (counting method unstated).
- `docs/GA-CHECKLIST.md:18` — "~22 modules" → **26** native module files (Appendix II).
— **[Verified: wc/grep; the 270 delta is Inferred pending its counting method]**

### A11. wave0-remainder internal staleness
- `wave0-remainder.plan.md:112` — "S1 ✅ BUILT (green, **uncommitted**)" — stale: S1 committed
  (`cd29f3c` exists; tree clean at HEAD). — **[Verified: git cat-file + git status]**
- `wave0-remainder.plan.md:145` — Stage C "⏳ TODO" vs `:130-137` "✅ FEATURE-COMPLETE" — the file
  self-labels the TODO block historical, but both states coexist in one doc.

### A12. MASTER-PLAN staleness vs shipped work
- W3-4 (`MASTER-PLAN.md:633-646`) has no ✅ though shipped (`f4c4c1d` exists; `src/native/hash.rs`,
  `src/native/random.rs` present; W0R:8 records it). MP predates the ship — needs the marker at merge.
- `MASTER-PLAN.md:426-429` (W2-4 `->` retirement "not started") vs W0R:250-253 (corpus purged,
  formatter fixed, reject pending) — MP stale on Phase-1 progress. — **[Verified: commits exist]**

---

## B. UNDOCUMENTED FEATURES — code has it, docs don't say so

1. **`phg debug`** (REPL + `--dap`): dispatched at `src/main.rs:256`, in the short usage line, but
   absent from the long `--help` commands list and from FEATURES' tooling table (only
   KNOWN_ISSUES:219-220 + CHANGELOG:9 mention it). **[Verified]**
2. **`phg lsp`**: dispatched at `src/main.rs:244`, absent from long `--help`; FEATURES:80 still
   marks LSP 🔲. README:218-239 does document it (inconsistently with FEATURES). **[Verified]**
3. **W3-4 security stdlib** — `Core.Hash.hmac/equals/hkdf/pbkdf2`, `Core.Random.secureBytes/secureInt`:
   in examples/README + CHANGELOG only; no FEATURES row. **[Verified: src/native/hash.rs, random.rs exist]**
4. **Import S1/S2 discipline** — member-imports (`import Core.Http.Router;`),
   `E-INJECTED-TYPE-BARE` enforcement, qualified `new Http.Router()` / `#[Http.Route]`
   (`src/checker/enforce_injected.rs:77 module_of`, `src/checker/collapse_injected.rs`): no
   user-facing doc (FEATURES/STABILITY untouched by the redesign; spec exists at
   `docs/specs/2026-07-03-unified-import-and-injected-type-discipline.md`). **[Verified: files exist]**
5. **`ctrlc` and `corosensei`** never named in README/FEATURES/VISION/STABILITY;
   `STABILITY.md:64` names only regex+argon2 → the doc surface understates the dep set even where
   it admits deps exist. **[Verified: grep]**
6. **E-codes emitted but not explainable**: `E-STATIC-INIT-CONST`, `E-TYPE-IMPORT-BUILTIN`,
   `E-TYPE-IMPORT-SHADOW` exist in src with **zero** entries in `src/cli/explain.rs`.
   **[Verified: grep both sides]**
7. **`E-OVERLOAD-SELECT-CONFLICT`** — registered in explain (2 refs) but never raised anywhere in
   src → dead registry entry (confirms MASTER-PLAN:567). **[Verified: grep]**

---

## C. CONTRADICTIONS — doc A vs doc B (or self)

| # | Topic | Says done/current | Says pending/other | Verdict |
|---|-------|-------------------|--------------------|---------|
| 1 | traits | KNOWN_ISSUES:248 "S8 shipped"; STABILITY:33 stable | FEATURES:53 🔲 future; MASTER-PLAN §7-OPEN (disposition) | construct WORKS [Verified] — FEATURES wrong |
| 2 | concurrency | README:277 shipped surface; INVARIANTS:131 builtin types | FEATURES:55 🔲 M6 | shipped [Verified deps/CHANGELOG] |
| 3 | M5 packages | MILESTONES:255 ✅ COMPLETE; HISTORY:53 closed | FEATURES:54 🚧; ROADMAP:81 planned | complete |
| 4 | LSP | README:218-239 working `phg lsp`; STABILITY:73 experimental | FEATURES:80 🔲 M7; GA-CHECKLIST:16 "Missing" | exists [Verified] |
| 5 | formatter | FEATURES:74 ✅; GA-CHECKLIST:44 M-fmt COMPLETE | FEATURES:80 🔲 (same file!) | exists (`phg format`) |
| 6 | lift / PHP→Phorj | STABILITY:73 + GA-CHECKLIST:16 exists | FEATURES:79 🔲 M8; README:287 "separate future milestone" | exists [Verified] |
| 7 | serve / M6 | FEATURES:75 ✅ | README:134 "(M6)" future-tagged; STABILITY:73 "M6 in progress" | exists; status prose skew |
| 8 | HTML templating | FEATURES:14 ✅ | STABILITY:44 experimental | tier skew (decide once at merge) |
| 9 | GC | README:73/ROADMAP:62/MILESTONES:67 "no tracing GC, deferred v2" | VISION:68 "with a real garbage collector" | VISION aspirational wording — align |
| 10 | zero-deps | README:5/89/311, FEATURES:84, VISION:59 | STABILITY:64 + HISTORY:92 name the deps; THIRD-PARTY-NOTICES lists 4 | deps exist [Verified] |
| 11 | `fn` vs `function` | HISTORY:94 renamed | STABILITY:24 `fn` stable | `fn` rejected [Verified] |
| 12 | fn-type arrow | FEATURES:33/43 `(A) => B` | INVARIANTS:131 `(A,B) -> R` | canonical `=>` |
| 13 | `fmt` vs `format` | README:203-209 format | README:136 fmt (same file) | `format` [Verified] |
| 14 | M7/M8 numbering | MILESTONES:291/300 (M7=correctness, M8=trust) | ROADMAP:91-92 + FEATURES:79-80 (M7=tooling, M8=migration) | MILESTONES:309-311 flags it; unify at merge |
| 15 | decimal | STABILITY:27, INVARIANTS:52, M-NUM shipped (`src/native/decimal.rs`) | FEATURES language table omits it entirely | add row |
| 16 | E-TRANSPILE-CONCURRENCY | MASTER-PLAN:84/1271 as in-force | W0R:262 "non-existent"; src has E-CONCURRENCY-NO-PHP | W0R right [Verified] |

---

## D. PERCENTAGE CLAIMS — denominator audit

- **≈58% parity / ≈60% vision** (`MASTER-PLAN.md:60-63`, model at `:1119-1121`): the denominator
  IS visible and reproducible — `docs/research/full-audit/raw/M-gap-matrix.md` §4: 824 verdict
  rows (173 SYN + 631 FN + 20 RT); `Coverage=(COVERED+0.5·PARTIAL)/(rows−N/A−GAP-by-design)`;
  SYN 103/129=79.8%, FN usage-weighted 410.5/1264=32.5% (tier weights ×3/×2/×1, §4.3), RT
  12.5/18=69.4%; domain weights 35/40/25 → ≈58; Vision = 0.70·parity + 0.30·programme(64.4).
  **[Verified: read the model + arithmetic checks out]**
- **Staleness**: computed 2026-07-02. Shipped since: S0/S1/S2 import unification, W3-4
  crypto MAC/KDF + CSPRNG, NDJSON, INI — these move FN rows in the HASH/CRYPT/JSON tiers →
  **≈58% is now a stale lower bound**. A row-level re-score of the FN matrix is needed at merge
  (out of A4 scope). — **[Inferred: shipped features map onto scored row families]**
- **GA-CHECKLIST ≈57% / "vibe-77%"** (`:21-23`): a *different* model (6 weighted rocks), whose
  rock-2 input is factually stale (A7: "Missing: an LSP" false; dead verb names in its evidence
  cell). Number not trustworthy as printed. — **[Verified premise-failure; % itself Unverified]**
- **Ledger consistency**: W0R:192 "W3 59%→65%" matches MP §10 (W2-end ≈59 → W3-end ≈65). No
  contradiction. **[Verified: both tables]**

---

## E. wave0-remainder Phase-1 claims vs actual corpus — VERIFIED with caveats

Claim (`wave0-remainder.plan.md:250-253`, commit `479dee4`): formatter canonicalized; "all 121
.phg files, code `->` gone; gate green (1660)".
- Corpus today: **236 `.phg`** files (examples 174, conformance 58, selftest 2, tests-fixtures 2).
  `->` appears in 55 of them — **every occurrence is inside a `//` comment; zero code-syntax
  arrows remain**. The purge claim holds in substance. — **[Verified: grep -rn --include='*.phg' across all dirs; all matches eyeballed]**
- Caveat 1: ~50 example files' comments still *teach* the retired syntax (worst:
  `examples/guide/functions.phg:6` "A function with no `-> T` … returns nothing";
  `guide/lambdas-pipe.phg:19` explains `(int) -> int` as a current parse) — the purge did not
  cover prose, so the corpus documents a form slated for rejection.
- Caveat 2: the parser still ACCEPTS `->` at all 6 sites (`src/parser/types.rs:109`,
  `items.rs:240/296/370/735`, `exprs.rs:546`); probe programs with `-> int` returns and
  `(int) -> int` fn-types parse, check, and run. Matches the plan's "remainder pending" — any doc
  implying the retirement is complete is wrong. — **[Verified: grep + binary probes]**
- "121 files" = the subset that contained arrows at purge time; not re-derivable now
  **[Unverified: historical count]**. "1660 tests green" **[Unverified this session: suite not
  re-run; nextest listing not completed]**.
- `phg --help` itself still prints `->` in prose twice (lift, serve lines) — cosmetic (P3).

---

## F. Prior-research corrections (leads re-verified)

1. `docs/research/2026-07-03-corpus-audit.md` C4 lists **`E-FIELD-INIT`** among codes "absent from
   `phg explain`" — **REFUTED**: `src/cli/explain.rs` contains 4 `E-FIELD-INIT` occurrences. The
   other three (E-STATIC-INIT-CONST, E-TYPE-IMPORT-BUILTIN, E-TYPE-IMPORT-SHADOW) are confirmed
   absent. **[Verified: grep]**
2. Corpus-audit A1's "177 arrows in 75 files" described the pre-purge state — superseded by §E
   above (0 code arrows in .phg now).
3. `docs/research/roadmap-completeness/raw/` (20 track files, 2026-06-21): heavily stale as a
   whole — C.md ("no LSP, no formatter, no REPL"), F.md (formatter/test/playground/LSP missing),
   H.md (no totality check), N.md ("no decimal, no date/time"), O.md ("zero first-class testing")
   are all contradicted by since-shipped features. Treat the whole directory as historical input
   to the 555-triage, never as current state. **[Verified: each named feature exists per Appendix II]**
4. `M-gap-matrix.md` §4 remains the only defensible % model — keep it as the recompute base at
   merge; `P-plan-verdicts.md` (48 DELETE-VERIFIED / 15 MERGE / 2 RECORD / 1 ACTIVE) already
   executed per MASTER-PLAN §12. C-decisions.md: 141 DEC rows; open conflicts C-2, C-8, C-9 still
   listed as open — carry into the merged SSOT.

---

## G. Merge guidance (for the SSOT owner — not rulings, §15 respected)

P0 at merge: A1 (zero-deps), A2 (import type), A4 (dead verbs incl. STABILITY declaring
nonexistent verbs *stable*), A5 (E-TRANSPILE-CONCURRENCY in README), A6/A7 (🔲-on-shipped +
GA-basis). P1: B1-B4 (undocumented debug/lsp/W3-4/S2), C14 (M7/M8 numbering), C15 (decimal row),
A10 (counts), §D recompute. P2: comment-level `->` prose in examples (fold into the P1-remainder
reject pass so comments and syntax flip together), src stale comments (A2 tail), help-text `->`.
FEATURES.md L38 🚧 row needs splitting: tuples/map-iteration genuinely pending [Unverified this
session] vs Set algebra shipped.

---

# Appendix I — Inventory A: doc-side claims (source-anchored)

Scope read: MASTER-PLAN.md (1296 l), wave0-remainder.plan.md (288 l), FEATURES.md (89 l),
README.md (312 l), ROADMAP.md, VISION.md, STABILITY.md, KNOWN_ISSUES.md (1133 l),
CHANGELOG.md (2719 l, structure + recent + targeted scans), docs/{ARCHITECTURE,HISTORY,
MILESTONES,INVARIANTS,GA-CHECKLIST,DEPRECATION}.md, examples/README.md (targeted),
CONTRIBUTING.md, THIRD-PARTY-NOTICES.md, editors/*/README.md, prior research
(full-audit raw M/P/C, roadmap-completeness raw ×20, 2026-07-03-corpus-audit, wave3-4-drafts).

**Done/shipped claims (representative, full refs in §A-C):** FEATURES ✅ rows L12-52 minus flagged;
MILESTONES ✅: M1(14), M2(25), M-Decomp(138), pattern(160), M-TIME(181), M-mut(200),
visibility(219), M5(255), M7-correctness(291), audit(317); GA-CHECKLIST rocks 95/70/15/70/40/10%;
W0R shipped: W3-4 f4c4c1d(8), S0 11a6c71(107), S1(112), S2 0cedcb8/202ec2b/20ecfe0/bc523c1(130),
Phase-1 purge 5fe4e90+479dee4(250-253); MP §12 executed ledger (1263-1296).
**Pending claims:** FEATURES 🔲/🚧 L38/53/55/77/78/79/80; MP DESIGN-NEEDED W3-1/2/3, W4-1/2/4/10/11/13,
W5-16; W0R phases 2-6 TODO (256-269); MP §7-OPEN traits; W3-5 blocker (W0R:15).
**Percentages:** MP:60-66, MP §10 ledger 1125-1131, GA-CHECKLIST:4/15-24, W0R:192.
**Syntax presented as current:** colon returns (README:29/36/180/249, FEATURES:18/32);
`=>` fn-types (FEATURES:33/43); `import type` (A2 list); `->` fn-type (INVARIANTS:131,
KNOWN_ISSUES:678); `fn` (STABILITY:24).
**CLI verbs:** canonical uses (FEATURES:62-76, HISTORY:94, INVARIANTS:111/118) vs dead names (A4 list).
**Deps:** zero-claims (A1 list) vs partial admissions (STABILITY:64, HISTORY:92) vs full list
(THIRD-PARTY-NOTICES:15-18).

# Appendix II — Inventory B: code-side ground truth (all [Verified] at HEAD 0691228)

- **Binary**: `phg 0.5.1-alpha.1`. Long-help verbs (15): run runvm check parse tokenize transpile
  lift disassemble benchmark build vendor serve test format explain. Short-usage adds: **lsp,
  debug** (main.rs:74-75; dispatch :244/:256). No fmt/bench/disasm/lex aliases.
- **Deps** (Cargo.toml): default features crypto+regex+signals+green → argon2 0.5, regex 1,
  ctrlc 3, corosensei 0.3 (non-wasm). `#![forbid(unsafe_code)]`, `warnings = "deny"`.
- **Syntax acceptance (probe-tested)**: `-> T` return ACCEPTED+runs; `(A) -> B` fn-type ACCEPTED;
  `import type` REJECTED (parse error); `fn` lambda REJECTED; `trait T {…}` + `class A { use T; }`
  ACCEPTED (checks clean). Parser Arrow sites: types.rs:109, items.rs:240/296/370/735, exprs.rs:546.
- **Error codes**: 206 unique `E-*` strings in src (incl. test fakes E-FOO/E-NOPE); explain.rs
  carries 200 unique E-/W- codes. Present: E-CONCURRENCY-NO-PHP, E-INJECTED-TYPE-BARE,
  E-INJECTED-VARIANT-BARE, E-FOREIGN-RUNTIME, E-UFCS-AMBIGUOUS. Absent everywhere:
  E-TRANSPILE-* (all), E-RETIRED-SYNTAX, E-UNIMPORTED. In-src-not-in-explain:
  E-STATIC-INIT-CONST, E-TYPE-IMPORT-BUILTIN, E-TYPE-IMPORT-SHADOW. In-explain-never-raised:
  E-OVERLOAD-SELECT-CONFLICT.
- **Native modules (26 files, src/native/)**: bytes convert crypto csv decimal encoding file hash
  html ini json list map math path process random reflect regex runtime set test text time url
  validate. Set algebra present (union/intersection/difference/isSubset); P3 additions absent.
  `Bytes.find` (bytes.rs:110) and `Map.has` (map.rs:194) still pre-rename (P2 pending — correct).
- **Injected-type discipline**: src/checker/enforce_injected.rs (module_of :77),
  collapse_injected.rs — S1/S2 enforced in code.
- **Corpus**: 236 .phg (examples 174, conformance 58, selftest 2, tests 2); `->` only in comments
  (55 files), zero code arrows.
- **Commits claimed by plans — all exist**: f4c4c1d 11a6c71 0cedcb8 202ec2b 20ecfe0 bc523c1
  5fe4e90 479dee4 c66bde5 60540fc ccb2403 cd29f3c d0cdc77 020d98f 25b2ef0 4dbd360 4f4f271
  297229f eedc8f2.
- **Counts**: KNOWN_ISSUES 1133 lines; differential.rs 3308 lines; MASTER-PLAN 1296; FEATURES 89.
- **Rust traits in src (non-test)**: serve.rs:59 Transport, debug.rs:62 DebugFrontend,
  green/exec.rs:40 Suspend.
- **Not verified this session**: test-suite count (1656/1660 claims), PHP-leg byte-identity,
  playground example count, tuples/map-iteration status.

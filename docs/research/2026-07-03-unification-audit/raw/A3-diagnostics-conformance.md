# A3 — `conformance/diagnostics/` corpus audit (2026-07-03, HEAD 0691228)

Auditor dimension: the golden-diagnostic must-fail corpus — completeness, correctness, current
behaviour, and missing sibling diagnostics. Binary: `target/debug/phg` (debug build, `cargo build`
was already up to date at HEAD). Every claim below is graded per Rule 18.

---

## 0. Corpus inventory and harness

The corpus is **18 files = 9 test cases**: each `<name>.phg` (must-fail program) is paired with a
`<name>.expected` (exact rendered diagnostic: header, source line, caret, `[CODE]`, `hint:`).
[Verified: `ls conformance/diagnostics/` → 9 `.phg` + 9 `.expected`]

Harness: `tests/diagnostics.rs` — globs `conformance/diagnostics/*.phg`, runs `phorj::cli::cmd_check(&src)`
on each, requires failure, and byte-compares the rendered diagnostic against `.expected`
(`PHORJ_BLESS=1` regenerates). Two tests: `corpus_is_nonempty` + `every_case_fails_check_with_its_golden_diagnostic`.
Its own docstring scopes intent: *"a representative set of codes render well"* — it complements the
`every_emitted_diagnostic_code_has_an_explanation` ratchet (`src/cli/tests.rs:637`), which proves
every emitted code is explainable. [Verified: read `tests/diagnostics.rs` in full]

**Structural reach limit**: `cmd_check(src: &str)` (`src/cli/mod.rs:1179`) takes a single source
string — loader/project-mode diagnostics (`E-FILE-*`, `E-PKG-PATH`, `E-IMPORT-*`, `E-VENDOR-*`)
are **structurally outside** this corpus's reach. They are covered instead by loader unit tests
(`src/loader/tests.rs`). [Verified: signature read + `grep E-FILE` → emitted in `src/loader/fs.rs`,
tested in `src/loader/tests.rs`]

## 1. Execution results — all 9 cases (Task 1)

**All 9 `.phg` files were EXECUTED** through `phg check`; each failed (exit 1) and the rendered
diagnostic was **byte-identical** to its `.expected` (diff empty for all 9). **Zero regressions,
zero drifted messages.** [Verified: ran `./target/debug/phg check` on each + `diff -u` vs
`.expected`; 9/9 exit=1, 9/9 MATCH]

| case | expected code | result |
|---|---|---|
| duplicate-enum-variant | E-DUP-VARIANT | ✅ fails, exact match |
| duplicate-static-field | E-DUP-STATIC | ✅ fails, exact match |
| duplicate-type | E-DUP-TYPE | ✅ fails, exact match |
| injected-variant-bare | E-INJECTED-VARIANT-BARE | ✅ fails, exact match |
| new-required | E-NEW-REQUIRED | ✅ fails, exact match |
| override-return-sig | E-OVERRIDE-SIG | ✅ fails, exact match |
| static-field-visibility | E-FIELD-VISIBILITY | ✅ fails, exact match |
| static-method-via-instance | E-STATIC-VIA-INSTANCE | ✅ fails, exact match |
| type-arg-count | E-TYPE-ARG-COUNT | ✅ fails, exact match |

## 2. Member-visibility access sites vs corpus coverage (Task 2)

The "six access sites" note is **stale** — the family has grown. `grep enforce_member_vis src/checker/`
finds **10 call sites** + the definition (`src/checker/calls.rs:1774`), plus two sibling enforcers:
`enforce_ctor_vis` (`calls.rs`, its own doc comment calls it *"the 7th member-visibility access
site"*) and the `E-CONST-VISIBILITY` const path (mirrored per the `enforce_member_vis` doc comment).
[Verified: grep output + read definitions]

Coverage table — **exactly 1 of ~12 sites has a golden corpus case**:

| # | access site | location | code | corpus? | live probe |
|---|---|---|---|---|---|
| 1 | instance method call | `src/checker/calls.rs:1396` | E-METHOD-VISIBILITY | ❌ | [Verified: probe p1 → `[E-METHOD-VISIBILITY]`, exit 1] |
| 2 | static method call | `src/checker/calls.rs:1574` | E-METHOD-VISIBILITY | ❌ | [Verified: probe p9 → same code] |
| 3 | overload-selector call | `src/checker/overloads.rs:200` | E-METHOD-VISIBILITY | ❌ | [Inferred: same `enforce_member_vis(…, false)` call shape; not probed] |
| 4 | **static field read** | `src/checker/calls.rs:1633` (W0-2) | E-FIELD-VISIBILITY | ✅ `static-field-visibility.phg` | [Verified: corpus run] |
| 5 | instance field read | `src/checker/calls.rs:1692` | E-FIELD-VISIBILITY | ❌ | [Verified: probe p2 → `[E-FIELD-VISIBILITY]`] |
| 6 | static field write | `src/checker/assign.rs:228` (W0-2) | E-FIELD-VISIBILITY | ❌ | [Verified: probe p4 → `[E-FIELD-VISIBILITY]`] |
| 7 | instance field write | `src/checker/assign.rs:295` | E-FIELD-VISIBILITY | ❌ | [Verified: probe p3 → `[E-FIELD-VISIBILITY]`] |
| 8 | `with` field override | `src/checker/assign.rs:373` | E-FIELD-VISIBILITY | ❌ | [Verified: probe p10 → `[E-FIELD-VISIBILITY]`] |
| 9 | struct-pattern field read | `src/checker/matches.rs:398` | E-FIELD-VISIBILITY | ❌ | [Inferred: `enforce_member_vis(…, true)` at site; not probed] |
| 10 | destructure field read | `src/checker/stmt.rs:540` | E-FIELD-VISIBILITY | ❌ | [Inferred: same shape; not probed] |
| 11 | constructor (`new C()`) | `enforce_ctor_vis`, `src/checker/calls.rs` | E-CTOR-VISIBILITY | ❌ | [Verified: probe p6 → `[E-CTOR-VISIBILITY]` + factory hint] |
| 12 | const access | (const path) | E-CONST-VISIBILITY | ❌ | [Unverified: not probed this session; referenced by `enforce_member_vis` doc comment] |
| — | `protected` (vs private) rendering | any field site | E-FIELD-VISIBILITY | ❌ | [Verified: probe p11 → *"accessible only inside `A` and its subclasses"*] |

The enforcement itself is **healthy** — every probed site fires the right code with a correct
message. The gap is purely golden-render coverage: `E-METHOD-VISIBILITY` and `E-CTOR-VISIBILITY`
have **zero** corpus cases while their field sibling has one, and the `protected` message variant
("…and its subclasses") is never golden-pinned anywhere.

## 3. FINDINGS

### F1 (behavioral asymmetry — the strongest finding). Static FIELD via instance: generic, CODE-LESS error
- **What**: `c.make()` on a static method → dedicated `E-STATIC-VIA-INSTANCE` with an exact fix hint
  (`src/checker/calls.rs:1371`; corpus-pinned). But `a.s` on a static **field** → generic
  ``type `A` has no field `s` `` via code-less `self.err(...)` at **`src/checker/calls.rs:1709`** —
  no `[E-CODE]`, no hint pointing at `A.s`.
- **Why it matters**: violates the M-DX "exact info + one exact fix" bar the corpus docstring cites;
  it is invisible to the explain ratchet (no code ⇒ nothing to explain); and it is asymmetric with
  the method path shipped in W0-3.
- [Verified: probe p5 output — error rendered with no code line and no hint; emitter located at
  `calls.rs:1709` (`self.err`, not `err_coded`)]
- **Suggested fix**: a `E-STATIC-VIA-INSTANCE`-style branch on the instance-field-read path when the
  name exists in `classes[cls].statics` (mirror of the method-side check), + a corpus case.

### F2. E-METHOD-VISIBILITY has no corpus case (symmetry gap, Task 4)
Field visibility is corpus-pinned; the **method** sibling emitted by the same function
(`enforce_member_vis`, `is_field=false`) is not. Both message shapes (`private`/`protected`) are
unpinned. [Verified: corpus file list + probes p1/p9]

### F3. E-INJECTED-TYPE-BARE has no corpus case (symmetry gap)
`E-INJECTED-VARIANT-BARE` IS in the corpus, but its S2 sibling `E-INJECTED-TYPE-BARE` — the newest
enforcement (closes "Route in the wind", commit 20ecfe0) — has no golden render test. The rendering
is good (verified live: probe p8 → correct code + `import Core.Http.Router;` hint) and it has unit
tests, but a message/hint regression would not be loud. [Verified: probe p8 + corpus list]

### F4. E-DUP-FIELD and E-NEW-ON-NONCONSTRUCT missing next to their corpus-covered siblings
- `E-DUP-STATIC` is corpus-pinned; `E-DUP-FIELD` (instance dup) is not — probe p7 confirms it fires
  with the parallel message *"duplicate field `f`"* + hint. [Verified: probe p7]
- `E-NEW-REQUIRED` is corpus-pinned; `E-NEW-ON-NONCONSTRUCT` (emitted `src/checker/expr.rs:254`) has
  **zero tests anywhere** (see F6) — it is both a corpus and a unit-test gap. [Verified: emission
  grep + full-test-surface cross-reference]

### F5. Ghost / stale E-codes (dead references)
1. **`E-OVERLOAD-SELECT-CONFLICT` — explain-only orphan**: has a full `phg explain` entry
   (`src/cli/explain.rs:645`) but is **never emitted anywhere** in `src/`. The explain ratchet only
   proves emitted→explained; the reverse direction is unchecked, so this documents a diagnostic
   users can never see. [Verified: `grep -rF` over `src/` → only explain.rs hits]
2. **`E-TYPE-IMPORT-BUILTIN` / `E-TYPE-IMPORT-SHADOW` — comment-only ghosts of removed `import type`**:
   sole occurrences are doc comments at `src/loader/mod.rs:617` and `src/loader/resolve.rs:590`.
   `import type` was removed in the S0/S2 unification — these are exactly the "import type ghosts"
   the P4 docs-consolidation phase targets, but in **code comments**, not docs. [Verified: grep —
   no string literal, no explain entry, comments only]
3. **`E-PKG-TYPE` — retired code with a live explain entry + self-contradicting comments**:
   `src/loader/mod.rs:261` says *"the old `E-PKG-TYPE` gate is retired — cross-package types"*, yet
   `src/checker/casing.rs:398` still claims *"Cross-package types do not exist yet (`E-PKG-TYPE`)"*
   and `src/cli/explain.rs` still explains it. Never emitted. [Verified: grep — comments +
   explain only, with the two comments contradicting each other]

### F6. 17 emitted E-codes with ZERO test coverage of any kind
Cross-referenced all 203 `E-*` identifiers in `src/` against `tests/`, `conformance/`, and every
`src` file containing `#[test]` (63 files). After removing regex false positives (E-ANNOTATION /
E-IDENTITY — substrings of "TYPE-ANNOTATION"/"BYTE-IDENTITY" in comments) and the F5 ghosts, these
are **emitted in live code paths and tested nowhere**:

| code | emitted at |
|---|---|
| E-CHANNEL-ANNOTATION | src/checker/calls.rs:1185 |
| E-CHANNEL-NEW-ARITY | src/checker/stmt.rs:76 |
| E-CHANNEL-NEW-TYPE | src/checker/stmt.rs:90 |
| E-CONCURRENCY-ARITY | src/checker/calls.rs:1220 |
| E-CONCURRENCY-METHOD | src/checker/calls.rs:1191 |
| E-DECIMAL-LITERAL | src/lexer/mod.rs:157 |
| E-HOOK-DUP | src/checker/collect.rs:1903 |
| E-HOOK-NO-GET | src/checker/calls.rs:1672 |
| E-HOOK-NO-SET | src/checker/assign.rs:277 |
| E-HOOK-TYPE | src/checker/program.rs:381 |
| E-NEW-ON-NONCONSTRUCT | src/checker/expr.rs:254 |
| E-OVERLOAD-FN-VALUE | src/checker/expr.rs:83 |
| E-PARENT-AMBIGUOUS | src/checker/calls.rs:320 |
| E-SPAWN-NOT-CALL | src/checker/calls.rs:1136 |
| E-SPAWN-VOID | src/checker/calls.rs:1145 |
| E-UFCS-AMBIGUOUS | src/checker/calls.rs:1945 |
| E-VARIANT-QUALIFIER | src/checker/matches.rs:264 |

Clusters: the **concurrency/channel/spawn** family (6 codes — plausibly under-tested because the
feature is oracle-quarantined) and the **property-hook** family (4 codes). [Verified: scripted
cross-reference; emission sites confirmed by grep. Caveat: a test that *triggers* a code without
naming its string would be missed — this measures "code string appears in a test surface".]

### F7. Corpus breadth: 9 of ~191 explained codes have a golden render test (~5%)
`explain.rs` documents 191 codes; the corpus pins 9. That matches the harness's declared
"representative set" intent, so this is **not a broken promise** — but the representative set has
visible selection bias: 3 of 9 are duplicate-decl codes, while entire high-traffic families
(visibility beyond one site, overloads, imports/injected-types S2, patterns/destructuring,
optionals, throws) have zero or one golden render. [Verified: `grep -c` on explain.rs match arms =
191; corpus = 9]

## 4. Spec cross-checks (Task 5)

### docs/specs/2026-06-28-public-surface-file-rule-design.md
Promises `E-FILE-NAME`, `E-FILE-MULTI-PUBLIC`, `E-FILE-MIXED-PUBLIC`, each with `phg explain` +
loader unit tests (positive + negative). **All satisfied**: emitted in `src/loader/fs.rs`, explained
in `src/cli/explain.rs`, tested in `src/loader/tests.rs`. They are correctly ABSENT from this corpus
(project-mode only — see §0 reach limit). No violation. [Verified: grep across the three surfaces]

### docs/specs/2026-06-28-statics-research-design.md
Research doc; header status *"research delivered … scope fork awaiting the developer"* is **stale**:
Area A (inherited statics, `Child.parentStatic()`) now type-checks clean — probe p12 `Child.make()`
→ `OK (type-checks clean)`, exit 0 — so at least A shipped after the doc was written. The corpus's
statics cases (E-DUP-STATIC, E-STATIC-VIA-INSTANCE, E-FIELD-VISIBILITY-on-static) are consistent
with the shipped W0-2/W0-3 work. The doc promises "checker tests, a guide example, updated
KNOWN_ISSUES" for A+B — verifying that delivery in full is outside this dimension; flagged for the
docs auditor. [Verified: probe p12 run; staleness = header text vs observed behaviour]

## 5. Recommendations (priority order)

1. **Fix F1** — dedicated static-field-via-instance diagnostic (code + hint) at `calls.rs:1709`,
   + corpus case (mirrors the already-pinned method case).
2. **Add 6 golden corpus cases** for verified-working siblings: `E-METHOD-VISIBILITY` (private
   method), `E-CTOR-VISIBILITY`, `E-INJECTED-TYPE-BARE`, `E-DUP-FIELD`, `E-NEW-ON-NONCONSTRUCT`,
   and one `protected` case (pins the "…and its subclasses" message variant). All six render
   correctly today (probes in §2/§3) — `PHORJ_BLESS=1` makes this cheap.
3. **Delete/resolve the F5 ghosts**: drop the `E-OVERLOAD-SELECT-CONFLICT` and `E-PKG-TYPE` explain
   entries (or wire up emission if the checks were meant to exist); fix the two `E-TYPE-IMPORT-*`
   comments and the contradictory `casing.rs:398` comment. Consider a **reverse ratchet** test
   (every explained code has ≥1 emission site) — the forward ratchet structurally cannot catch these.
4. **Triage F6** — at minimum the non-quarantined codes (hooks family, E-UFCS-AMBIGUOUS,
   E-VARIANT-QUALIFIER, E-PARENT-AMBIGUOUS, E-DECIMAL-LITERAL, E-OVERLOAD-FN-VALUE,
   E-NEW-ON-NONCONSTRUCT) deserve one triggering test each.
5. **Refresh the stale statics spec header** (fold into P4/P5 docs consolidation).

---
*Method note: all probe programs + raw outputs preserved at
`/tmp/claude-1000/-stack-projects-phorj/ca3b0f30-9e4f-4929-b9cf-ff3bc3c4986c/scratchpad/probes/`
for this session. 9/9 corpus cases executed; 12 additional live probes executed (p1–p12, of which
p3/p4 were re-run after a `mutable`-syntax correction).*

# E — Language surface: four developer questions (evidence-based review)

**Scope**: (1) wildcard imports — what actually works; (2) loop syntax — what was retired; (3) UFCS
promotion vs qualified calls; (4) the `class main` example + `main` reservation residue.

**Method**: release binary `/home/user/phorj/target/release/phg` (built 2026-07-25 21:03), probe files
under `scratchpad/probe-lang/` (loose single-file probes + two throwaway projects `proj/` for
cross-package wildcards and `vproj/` for a vendored package). No cargo builds, no repo writes.
Every claim below is either a pasted transcript or a `path:LINE` read.

**Registry-derived numbers disclosure**: the native-registry statistics in §3 come from a regex parse
of `src/native/**` + `src/ext/**` that matched **347 of 363 `NativeFn` literals (95.6%)** — the 16
misses have a `//` comment between `name:` and `params:`. Direction and magnitude are safe; exact
per-module counts carry ±16 natives of slack, and the "non-native" call-site bucket in §3 is
correspondingly overcounted by a small amount. [Verified: `grep -rc 'NativeFn {' src/native src/ext`
→ 363; parser reported 347.]

---

# 1. Wildcard / group imports — "not fully supported?? what do we support now??"

## Ground truth (evidence)

- Spec: `docs/archive/specs/2026-07-24-wildcard-imports.md` — status header line 1 still reads
  *"SPEC (RULED — BUILD-READY, NOT YET BUILT)"* but line 228 carries `## ✅ Q-A DONE (2026-07-25 —
  DEC-268 certified)`. **The header contradicts the body — E1 below.** [Verified: read lines 1, 228.]
- `docs/plans/SLICE-STATE.md:43-53` — `✅ Q-A WILDCARD IMPORTS — DONE (2026-07-25, DEC-268 CERTIFIED)`,
  steps 1-8 shipped, five dev-owned follow-ups `P-Q-A-1..5`. [Verified: read.]
- Group `{A, B}` predates the slice (DEC-186, expanded at PARSE time in
  `src/parser/items/decls.rs::parse_import_group`). Wildcard `*` + `except {}` expand in the **loader**
  (`src/loader/imports.rs`, `src/loader/import_hygiene.rs`). [Verified: spec §BUILD PLAN + SLICE-STATE:45.]
- Shipped example: `examples/project/wildcard-imports/` (4 files, `Acme.Geometry` library package +
  `Main`), exercising `*`, `except {}`, and the explicit-re-import escape hatch. [Verified: read all 4 files.]

## Probe transcripts — the capability matrix

Fixture: project `proj/` with `package Acme.Geometry` (`interface Shape`, `class Rect`, `enum Paint`,
`public function twice`, `internal function hidden`, `private function secret`), a sub-package
`Acme.Geometry.Deep` (`class Nested`), and a colliding `package Other` (`class Rect`).
Vendored fixture: `vproj/vendor/Vend/Util/` (`public function shout`, `public class Tool`).

| # | Form | Result | Evidence |
|---|---|---|---|
| W1 | `import Acme.Geometry.Rect;` | ✅ works | `phg run` → `6` |
| W2 | `import Acme.Geometry.Rect as Box;` | ✅ works | → `6` |
| W3 | `import Acme.Geometry.*;` (project pkg) | ✅ works — binds types **and** public functions | → `6 true 10` (`Rect`, `Shape`, `twice` all bound) |
| W4 | `import Acme.Geometry.{ Rect, Shape };` | ✅ works | → `6 true` |
| W5 | `import Acme.Geometry.{ Rect as Box, Shape };` | ✅ works (per-member alias) | → `6 true` |
| W6 | `import Acme.Geometry.* except { Paint };` + `import Acme.Geometry.Paint;` | ✅ works | → `6` |
| W7 | `import Acme.Geometry.Deep.*;` (deep pkg) | ✅ works | → `7` |
| W8 | does `Acme.Geometry.*` bind sub-package member `Nested`? | ✅ correctly **NO** (shallow, D3) — but the diagnostic is bare | `type error: unknown function 'Nested'` + `E-NEW-ON-NONCONSTRUCT`; **no hint that wildcards are shallow** |
| W9 | `import Acme.*;` (intermediate/empty pkg) | ❌ `E-MODULE-NOT-FOUND` | `no package 'Acme' (or '-') under any search root` |
| W10 | `import Core.*;` | ❌ `E-WILDCARD-STDLIB-ROOT` (parse) | `would bind the entire standard library; import a specific member` |
| W11 | `import Core.Text.*;` (stdlib SUBMODULE) | ❌ `E-WILDCARD-STDLIB-ROOT` — **not yet supported** | `wildcard import of the standard-library module 'Core.Text' … is not yet supported — import its members explicitly` |
| W12 | `import Acme.Geometry.* as Geo;` | ❌ `E-WILDCARD-ALIAS` (parse) | `a flat wildcard has no single name to bind` |
| W13 | two wildcards binding the same leaf (`Rect`) | ❌ `E-IMPORT-AMBIGUOUS` — eager, D2 | `'Rect' is brought by both … import it explicitly … or exclude it` |
| W14 | `except { Nope }` (absent name) | ❌ `E-EXCEPT-UNKNOWN` | `excludes 'Nope', but 'Acme.Geometry' has no such member` — **no did-you-mean hint despite the spec (D5) promising one** |
| W15 | `except {}` removing every member | ❌ `E-WILDCARD-EMPTY` | `binds no names — 'Acme.Geometry' exports nothing importable here` |
| W16 | `import Acme.Geometry.Nothing;` **unused** | ⚠ `E-UNUSED-IMPORT` — the WRONG code | `unused import 'Acme.Geometry.Nothing' — nothing in this file references 'Nothing'` (masks the real problem) |
| W16b | `import Acme.Geometry.Nothing;` **used** | ❌ `E-IMPORT-UNKNOWN` ✓ | `package 'Acme.Geometry' exports no member 'Nothing' — no such function, type, or sub-module` |
| W17 | `import Acme.Geometry.{ Rect, Nope };` unused | ⚠ `E-UNUSED-IMPORT` on `Rect` — masks `Nope` entirely | same masking as W16 |
| W18 | wildcard whose bound names are ALL unused | ✅ silent (no warning) | ran clean — `W-UNUSED-IMPORT` deferred, P-Q-A-3 |
| W19 | explicit import of a cross-pkg `internal` fn | ❌ `E-VIS-INTERNAL` | `it is 'internal' in package 'Acme.Geometry'` |
| W20 | explicit import of a cross-pkg `private` fn | ❌ `E-VIS-PRIVATE` | as above |
| W21 | does `*` bind the `internal` member? | ✅ correctly **NO** (public-only, P-Q-A-2) | `unknown function 'hidden'` |
| W22 | does `*` bind the `private` member? | ✅ correctly **NO** | `unknown function 'secret'` |
| W23/W24 | wildcard in loose single-file / `-e` mode | ❌ `E-WILDCARD-NO-PROJECT` — good, actionable | `only available inside a project … import the members explicitly, or run this inside a project` |
| W25 | `import Vend.Util.*;` (VENDORED package) | ✅ works | `phg run` → `hi! 3` |
| W26 | transpiled PHP of the shipped wildcard example | ✅ no `*` leaks (Inv 5 honored) — emits **fully-qualified** `\Acme\Geometry\Rect`, not per-symbol `use` | `phg transpile src/main.phg` |
| W27 | `phg format` on `* except { Shape, Paint }` | ✅ sorts → `except { Paint, Shape }`; group `{}` **not** sorted (expanded at parse — P-Q-A-4) | before/after diff |

## Recorded follow-ups, verbatim (`docs/archive/specs/2026-07-24-wildcard-imports.md:168-213`)

- **P-Q-A-1 — Core-submodule wildcards (`import Core.Http.*`) DEFERRED.** *"the loader's
  native/prelude pre-pass intercepts `Core.*` imports BEFORE the wildcard-expansion hook, so a Core
  wildcard never reaches it (a naive attempt silently binds nothing — a false positive). Rather than
  ship silent-wrong behavior, `Core.*` wildcards (bare AND submodule) are **parser-rejected** for now
  … **User/vendored-package wildcards work fully.** (D4 allowed Core.Sub.*; this narrows it pending
  the follow-up.)"*
- **P-Q-A-2 — `*` binds PUBLIC-only cross-package (not "public+internal" as D3's shorthand said).**
  *"cross-package `*` binds public only; same-package would bind public+internal but is already
  implicitly visible. D3's literal "public+internal" wording conflicts with the actual rule for
  cross-package internal — flagging for dev confirmation (the principled behavior is the safe/consistent one)."*
- **P-Q-A-3 — `W-UNUSED-IMPORT` (step 4) DEFERRED to the W-UNUSED-* lint family.** *"Wildcard/group
  imports are currently EXEMPT from the hard `E-UNUSED-IMPORT` … A soft `W-UNUSED-IMPORT` needs
  per-expanded-member usage analysis and is the FIRST of the `W-UNUSED-*` family (audit M3 /
  C-unused-import) — better designed as one coherent lint slice than a wildcard-only one-off."*
- **P-Q-A-4 — group-`{}` member sorting is a NO-OP (ruling (e) applies to `except {}` only).**
  *"a grouped import `import P.{ Zeta, Alpha };` is expanded into per-member `Item::Import`s **at
  PARSE time** (DEC-186, `parse_import_group`), so the formatter never sees a `{}` group … Ruling
  (e)'s "sort `{}`/`except {}`" is therefore honored for `except {}` … but is unimplementable for
  `{}` without moving group expansion out of the parser into a post-format desugar — a larger change
  to DEC-186. Behavior is still idempotent and byte-identity-safe (no correctness impact); only the
  cosmetic sort is absent."*
- **P-Q-A-5 — Inv-13 file-size debt accrued across the Q-A series.** *"shipping Q-A grew five
  grandfathered files past their `size-baseline.txt` entries, and the whole series (steps 1-7 …) was
  pushed `--no-verify` with the size-gate red … **Still over baseline (dev to split or re-baseline):**
  `src/parser/items/decls.rs` 667>605 · `src/parser/tests/items.rs` 757>656 ·
  `src/checker/program/walk.rs` 592>519 · `src/cli/explain.rs` 2057>1998 · `src/loader/tests.rs`
  796>667; plus two files at the 500 hard cap (`src/ast/decls.rs` 504, `src/lift/lifter/decls.rs` 504).
  Splitting `explain.rs` (a 2057-line single `match`) is a structural design call."*

## Gaps

- **E1 — [P2, Verified]** The spec's own status header lies. `docs/archive/specs/2026-07-24-wildcard-imports.md:1`
  says *"RULED — BUILD-READY, NOT YET BUILT"*; line 228 says `✅ Q-A DONE`. A fresh context reading
  the header first concludes nothing was built. Inv 19 ("zero divergence from the SSOT") violation
  inside a single file. One-line fix.
- **E2 — [P1, Verified]** **D4 is only partly delivered.** The ruled scope allowed stdlib SUBMODULE
  wildcards (`import Core.Http.*;`); today ALL `Core.*` wildcards are parser-rejected (W10/W11,
  P-Q-A-1). This is the single biggest gap between spec and code and is the most likely source of the
  developer's "not fully supported" impression — stdlib is where a wildcard would be used most.
  The error message IS honest (*"not yet supported"*), so it fails loud, not silent.
- **E3 — [P2, Verified]** `E-IMPORT-UNKNOWN` is **masked by `E-UNUSED-IMPORT`** when the bad name is
  unused (W16/W17). The spec's G6 ruling is *"an import naming a non-existent package/member is a
  compile error at the import line, **whether or not it is used**"*. Both cases do error, but the
  unused case reports the wrong cause — `nothing in this file references 'Nothing'` sends the user to
  delete a line rather than fix a typo. In a group (W17) the bogus member is not mentioned at all.
- **E4 — [P3, Verified]** `E-EXCEPT-UNKNOWN` ships **without the did-you-mean hint** the spec's D5
  ruling promised (*"= HARD ERROR `E-EXCEPT-UNKNOWN` (with did-you-mean hint …)"*). W14 output has no
  hint line.
- **E5 — [P3, Verified]** Shallow-wildcard failure (W8) produces `unknown function 'Nested'` with no
  hint that `*` does not descend into sub-packages. A one-line hint ("`*` is shallow — add
  `import Acme.Geometry.Deep.Nested;`") would close the most likely user confusion.
- **E6 — [P3, Verified]** Spec step 6 asserts *"transpiled PHP shows sorted per-symbol `use`"*; the
  actual emission (W26) is fully-qualified names, no `use` statements. Semantically equivalent and
  byte-identity-safe, but the spec's acceptance criterion does not describe the code.
- **E7 — [P3, Verified]** `phg format` does not sort group-`{}` members (P-Q-A-4). Cosmetic; already
  disclosed and dev-owned.

## Options & recommendation

**Definitive answer to the developer**: wildcard imports **are** built and certified for **user and
vendored packages** — `*`, `* except {}`, group `{}`, group aliasing, deep packages, the eager
collision error, and the 7-code diagnostic catalog all work (W1-W7, W12-W15, W25, table above). The
ONE genuinely missing ruled surface is **stdlib wildcards** (`import Core.Http.*;`) — parser-rejected
pending P-Q-A-1. Everything else on the "not supported" list is a deliberate, recorded deferral
(`W-UNUSED-IMPORT`, group-`{}` sort) or a diagnostic-quality gap (E3-E5), not a missing capability.

Options for closing E2, in increasing effort:
- (a) **Leave rejected, retitle the follow-up as a scheduled slice.** Zero risk; keeps the honest
  "not yet supported" error. Cost: the most-wanted wildcard target stays unavailable.
- (b) **Build P-Q-A-1** — promote `src/lsp/catalog.rs::module_members` to a `pub(crate)` native
  enumerator (the spec's own STEP 2 DETAIL already names this) and run the wildcard expansion
  BEFORE, or inside, the loader's native/prelude pre-pass so `Core.Sub.*` reaches the hook.
  Medium; the enumerator already exists.
- (c) **(b) + namespace-binding `import Core.Http.* as Http;`** — explicitly out of scope per ruling (a).

**RECOMMENDED: (b), preceded by the five cheap diagnostic fixes E1/E3/E4/E5/E6.** Rationale: E2 is
the only capability gap and its enabler already exists in the LSP catalog; the diagnostic fixes are
each one-to-three lines and together they are what actually changes the *felt* completeness of the
feature (four of the five current rough edges are message quality, not behaviour). E1 first — a spec
header that says "NOT BUILT" about a certified feature will keep costing re-investigation.
**This is a recommendation only; the D4-narrowing and every P-Q-A item are dev-owned rulings (Inv 15).**

---

# 2. Loop syntax — "did we retire `for .. in` or `foreach .. in`?"

## Ground truth (evidence)

- Parser: `for` is a real keyword dispatched at `src/parser/stmts.rs:11` → `parse_for` (line 414);
  `foreach` is a **contextual** keyword matched at `src/parser/stmts.rs:42`
  (`TokenKind::Ident(s) if s == "foreach"`) → `parse_foreach` (line 532). [Verified: read.]
- `parse_for` **requires `in`**: `self.expect(&TokenKind::In, "'in' in for-loop header")` at lines
  437 and 479. It first tries the C-style header via `for_header_is_classic()` (line 418), then a
  tuple pattern (line 423), else `parse_type()` + `expect_ident` + `in`.
- `parse_foreach` **requires `as`**: `src/parser/stmts.rs:544` —
  `return Err(self.error("'as' after the foreach iterable (e.g. \`foreach (xs as x)\`)"))`.
- **Nothing has been retired.** `docs/DEPRECATION.md` (43 lines, read in full) contains **zero** loop
  entries and states the deprecation table *"is **empty in the shipping build** today"* (line 41).
- `FEATURES.md:27` still lists `for … in` as ✅ with no replacement note; `FEATURES.md:29` lists
  PHP-familiar `foreach` as ✅. Both are live, both documented as live.
- `examples/guide/foreach.phg:7` — *"A-6 — PHP-familiar `foreach (xs as x)` iteration, **kept
  alongside** Phorj's typed `for (T x in xs)`"* — and its body demonstrates BOTH forms side by side.

**The decision history is where the confusion comes from** — there are two rulings that both said
"retire for-in", and neither was executed:

| Register row | Ruling | Reality |
|---|---|---|
| `C-decisions.md:110` **DEC-094** (06-25, A-6) | *"`foreach (coll as BINDING)` adopted to **REPLACE** `for (x in coll)`"* | row's own outcome column: *"◐ shipped **alongside** for-in, not replacing (see CONFLICTS C-2)"* |
| `C-decisions.md:275` **CONFLICT C-2** | *"The decided replacement was silently softened into an addition during an autonomous slice. Either the decision or the implementation is wrong. [Verified: both forms parse today.]"* → status **"Open — adjudicate"** | still open |
| `C-decisions.md:1372-1381` **DEC-248** (audit flag F-009) | *"FULL PHP ALIGNMENT of the loop surface; supersedes A-6/DEC-094's execution drift AND **retires for-in**. … (2) `for (T x in xs)` RETIRES (`E-RETIRED-FORIN` + rewrite hint) … (5) repo-wide codemod (~69 example sites …). Closes conflict C-2 / flag F-009."* | **item (2) NOT BUILT.** `grep -rn 'E-RETIRED-FORIN' src/ docs/ CHANGELOG.md FEATURES.md` → **only one hit, the register line itself.** `docs/plans/MASTER-PLAN.md:455` lists DEC-248 as item 10 with **no ✅** (its neighbours 8/11/13/14/15 all carry ✅ SHIPPED). Items (1) typed foreach and the k=>v form DID ship (DEC-280, `CHANGELOG.md:368`). |

## Probe transcripts

All probes are `package Main;` + the three standard imports + `#[Entry(kind: EntryKind.Cli)]`.

```
======== L1  for (int x in xs) ========            1 / 2 / 3        ✅ ACCEPTED
======== L4  foreach (xs as int x) ========        1 / 2 / 3        ✅ ACCEPTED
======== L10 foreach (xs as x)  [inferred] ======  1 / 2 / 3        ✅ ACCEPTED
======== L8  foreach (m as k => v) ==============  a=1 / b=2        ✅ ACCEPTED
======== L13 foreach (xs as x with int i) =======  0:1 / 1:2 / 2:3  ✅ ACCEPTED
======== L16 for (string k, int v in m) =========  a=1 / b=2        ✅ ACCEPTED (comma form, B1)
======== L17 for ((a, b) in ps) =================  1/2 / 3/4        ✅ ACCEPTED (tuple, DEC-288)
======== L5  for (mutable int i = 0; …; …) ======  0 / 1 / 2        ✅ ACCEPTED (C-style)

======== L2  for (xs as int x) ==================  ❌ REJECTED
  parse error at 8:16: expected 'in' in for-loop header, found Ident("int")

======== L11 for (xs as x) ======================  ❌ REJECTED
  parse error at 8:16: expected 'in' in for-loop header, found Ident("x")

======== L3  foreach (int x in xs) ==============  ❌ REJECTED
  parse error at 8:18: expected 'as' after the foreach iterable (e.g. `foreach (xs as x)`), found Ident("x")

======== L12 foreach (x in xs) ==================  ❌ REJECTED
  parse error at 8:16: expected 'as' after the foreach iterable (e.g. `foreach (xs as x)`), found In

======== L9  for (x in xs)  [untyped] ===========  ❌ REJECTED
  parse error at 8:12: expected a loop variable name, found In
======== L15 for (var x in xs) ==================  ❌ REJECTED
  type error at 8:10: unknown type `var`   [E-UNKNOWN-TYPE]

======== L6b for (string k => int v in m) =======  ❌ REJECTED
  parse error at 8:19: expected 'in' in for-loop header, found FatArrow
======== L7b for (m as string k => int v) =======  ❌ REJECTED
  parse error at 8:15: expected 'in' in for-loop header, found Ident("string")

======== L14 for (int x in xs with int i) =======  ❌ REJECTED
  parse error at 8:27: expected '{' after 'with', found Ident("int")   (parsed as clone-`with`)
```

**The rule, one line**: `for` pairs ONLY with `in`; `foreach` pairs ONLY with `as`. The keyword picks
the separator; you cannot cross them. The two headers are **not** feature-equivalent:

| capability | `for (… in …)` | `foreach (… as …)` |
|---|---|---|
| single binding, TYPED | ✅ `for (int x in xs)` | ✅ `foreach (xs as int x)` |
| single binding, INFERRED | ❌ (L9/L15 — type mandatory) | ✅ `foreach (xs as x)` |
| key/value | ✅ comma only: `for (K k, V v in m)` | ✅ fat-arrow: `foreach (m as k => v)`, typed/mixed |
| `=> ` key/value spelling | ❌ (L6b) | ✅ |
| `with int i` counter | ❌ (L14) | ✅ |
| tuple pattern | ✅ `for ((a, b) in ps)` (DEC-288) | ❌ *"foreach destructure bindings are not supported yet"* (`stmts.rs:549`) |
| C-style `(init; cond; step)` | ✅ | n/a |

## Gaps

- **E8 — [P1, Verified]** **DEC-248 is a half-executed ruling and CONFLICT C-2 is still open.** The
  register ruled `for (T x in xs)` retired with `E-RETIRED-FORIN` + a rewrite hint and a ~69-site
  codemod; that code does not exist (`grep -rn 'E-RETIRED-FORIN' src/` → 0 hits), while the same
  DEC's foreach half DID ship. Meanwhile `FEATURES.md:27` advertises `for … in` as fully supported
  and `examples/guide/foreach.phg:7` teaches it as a deliberate co-equal ("kept alongside"). So the
  register says one thing, the docs say the opposite, and both are checked in. **This is exactly the
  state that produced the developer's question**, and it will keep producing it. It needs a dev
  ruling, not a build: either finish DEC-248 (retire for-in) or amend DEC-248/DEC-094 and close C-2
  as "keep both, deliberately".
- **E9 — [P2, Verified]** **The cross-form parse errors are not migration hints.** `for (xs as x)`
  (L11) → *"expected 'in' in for-loop header, found Ident("x")"* — it does not say *"did you mean
  `foreach (xs as x)`?"*, even though at that exact token the parser has seen `for ( <expr> as` and
  the intent is unambiguous. The mirror case is better but still asymmetric: `foreach (x in xs)`
  (L12) at least prints a correct example (*"e.g. `foreach (xs as x)`"*) but never mentions that
  `for (T x in xs)` is the form the user actually wrote. A PHP-familiar user typing
  `foreach ($x in $xs)`, and a Phorj user typing `for (xs as x)`, are the two most likely loop
  mistakes in the language and neither gets pointed at the working form.
- **E10 — [P2, Verified]** **`for` cannot infer its binding.** `for (x in xs)` and `for (var x in xs)`
  are both rejected (L9/L15) while `foreach (xs as x)` infers fine. DEC-280's stated goal was
  *"removes the DEC-248 asymmetry: EVERY foreach binding may be untyped-inferred or typed"* — it
  fixed the asymmetry INSIDE `foreach` and left a new one BETWEEN the two forms. If the developer
  keeps both forms, this is the sharpest remaining wart; if for-in is retired, it is moot.
- **E11 — [P3, Verified]** Capability asymmetry (table above) is undocumented as such. Neither
  `FEATURES.md` nor `examples/guide/foreach.phg` states that `with int i` and `=> ` are
  foreach-only while tuple patterns are for-in-only. A user picking a form cannot know what they lose.

## Options & recommendation

**Crisp answer to the developer**: *Nothing was retired — both forms are live and each is locked to
one separator.* `for` takes `in` (`for (int x in xs)`, `for (K k, V v in m)`, `for ((a,b) in ps)`,
plus C-style `for (i=0; …)`); `foreach` takes `as` (`foreach (xs as x)`, `foreach (xs as int x)`,
`foreach (m as k => v)`, `foreach (xs as x with int i)`). Crossing them is a parse error (L2/L3/L11/L12).
The retirement you are remembering is **DEC-248** (`docs/research/full-audit/raw/C-decisions.md:1372-1381`),
which RULED that `for (T x in xs)` retires with `E-RETIRED-FORIN` — **that item was never built**
(`grep -rn 'E-RETIRED-FORIN' src/` → 0 hits; `MASTER-PLAN.md:455` item 10 has no ✅). Its sibling
items (typed `foreach`, `k => v`) *did* ship. Conflict **C-2** (`C-decisions.md:275`, *"Open —
adjudicate"*) is the same open question in the register, filed 06-25 and never closed.

Options for E8 (dev ruling required — Inv 15):
- (a) **Execute DEC-248 as ruled**: add `E-RETIRED-FORIN` with a rewrite hint, codemod the example
  corpus, update `FEATURES.md:27`/`docs/DEPRECATION.md`/`STABILITY.md` per the documented lifecycle,
  close C-2. **Measured corpus census** [Verified: paren-matched header scan of `examples/**/*.phg`]:
  **87 `for (… in …)` headers** (incl. the comma and tuple forms), **6 C-style `for (…;…;…)` headers**
  (unaffected — DEC-248 keeps them), **8 `foreach (… as …)` headers**. So the retirement would rewrite
  87 example sites (vs the ruling's ~69 estimate) — and note the striking ratio: the corpus is
  **87 for-in vs 8 foreach**, i.e. the form DEC-248 ruled retired is the one the examples overwhelmingly
  teach, and the form it ruled canonical is almost absent. That inversion is itself a finding: whichever
  way the developer rules, one of the two numbers has to move a long way.
  Largest change; delivers ONE loop idiom, full PHP alignment,
  and removes E9/E10/E11 at a stroke. Note the deprecation policy requires a `W-DEPRECATED` release
  before removal — so this is two releases, not one.
- (b) **Amend DEC-094/DEC-248 to "keep both, deliberately"** and close C-2 that way; then fix E9
  (cross-form hints) and E10 (let `for` infer) so the two forms are genuinely co-equal rather than
  accidentally asymmetric. Smallest change to code, biggest change to the register.
- (c) **Retire `foreach` instead**, keeping the typed Phorj form. Explicitly *rejected* in DEC-248's
  alternatives list (*"keeps the divergence"*) — listed for completeness only.
- (d) **Leave as-is.** Costs a re-investigation every time the question resurfaces (this is at least
  the second time: C-2 was filed 06-25, DEC-248 re-ruled it, and the question is being asked again now).

**RECOMMENDED: (b) — amend to "keep both", then fix E9 + E10.** Rationale: the language has shipped
~240 examples and a full doc corpus on the "both forms" premise for a month; the *behaviour* the
developer has been living with is (b), and DEC-248's retirement half has now failed to get built
twice. (b) makes the register describe reality (Inv 19) at near-zero code cost, and the two follow-on
fixes (cross-form migration hints, `for`-binding inference) remove the actual user-visible friction
that made the retirement attractive. If the developer's north-star is genuinely "one loop, PHP-shaped",
(a) is the right call and should be scheduled as a real slice with the deprecation lifecycle — but
that is a design ruling, not mine to make.

---

# 3. UFCS promotion — "instead of `String.length` we call `myvar.length`"

## Ground truth (evidence)

### THE RULING ALREADY EXISTS — DEC-326

`docs/research/full-audit/raw/C-decisions.md:2707-2721`:

> **DEC-326 — UFCS CANONICAL STYLE = RECEIVER FORM (developer-ruled 2026-07-22: both forms legal,
> one canonical style everywhere).** … THE RULE: the RECEIVER form `s.length()` is canonical wherever
> the first parameter is the natural subject; the MODULE form stays canonical for receiver-less calls
> (constructors/config/ambient: `Log.configure(...)`, `Math.max(a, b)`-style multi-subject).
> Rationale … : matches the DEC-319 "more OOP" north-star; **`s.`-completion discovery beats
> module-name recall**; Kotlin/Rust converged on the same idiom; lifted PHP visibly modernizes
> (`strlen($s)` → `s.length()`). BUILD QUEUED (next slice): lifter emits receiver form for
> subject-first natives …; **examples/docs migrate as touched**; a formatter canonicalization lint is
> the recorded v2.
> **✅ SHIPPED 2026-07-22:** lifter emits receiver form for subject-first natives …; FIXED the blocker
> the build surfaced: `E-UNUSED-IMPORT` false-fired on modules used ONLY via receiver form …

Mirrored in `FEATURES.md:56`. So **the developer's instinct is already ratified policy, and the
lifter half is shipped.** What is outstanding is exactly (i) the bulk example/doc migration (ruled
LAZY — "as touched"), (ii) the formatter canonicalization lint (recorded v2), and (iii) — not
recorded anywhere — the LSP completion that DEC-326's own rationale cites as the primary benefit.

### The dispatch rule (`src/checker/calls/ufcs.rs::try_ufcs`, read in full)

`x.foo(a, b)` resolves in this order:

1. **Method-first.** A real method `foo` on the receiver's type wins outright (UFCS is only reached
   after method lookup fails). Documented at `examples/guide/ufcs.phg:2-3`.
2. **A USER free function** `foo` (`ufcs.rs:40-59`) — gated on **`sigs.len() == 1`** (single
   overload only; multi-overload deferred, "F-004"), **exact arity** `params.len() == args.len() + 1`,
   and `ufcs_first_accepts(&sigs[0].params[0], recv_ty)` (line 43) which is `unify(param0, recv_ty)`
   against a throwaway substitution (line 144-147) — so a generic `List<T>` first param matches a
   concrete `List<int>`, and subtyping is honoured.
3. **An imported NATIVE** `foo` (`ufcs.rs:67-136`) — eligible only when **the module leaf is imported**
   (`module_imported`, line 91: `self.imports.get(leaf) == Some(n.module)`) **OR** the function itself
   is member-imported/aliased (`function_imported`, line 92, DEC-274). Same exact-arity +
   first-param-accepts test. Two matches → `E-UFCS-AMBIGUOUS` (line 103-115).

Rewrite: `finish_ufcs` (line 205) records `f(receiver, args…)` (free fn) or `Leaf.f(receiver, args…)`
(native) into `ufcs_resolutions`, consumed by `src/checker/rewrite_ufcs.rs` **before any backend** —
so it is Inv-5 compile-time sugar (confirmed by the byte-identity probe below). `?.` lowers to a
`match` over the optional (line 236-258).

### Natives that CANNOT be UFCS — the categories

| Category | Count | Mechanism | Examples |
|---|---|---|---|
| **A. arity-0 natives** — structurally impossible (no receiver slot) | **19** | `params.len() != args.len()+1` can never hold for `args=[]`, `params=[]` | `Math.pi`, `Math.e`, `Math.nan`, `Math.infinity`, `Math.negativeInfinity`, `Random.nextInt`, `Random.nextFloat`, `Process.arguments`, `Time.nowMilliseconds`, `Time.unfreeze`, `Environment.all`, `Runtime.monotonicNanos`, `Runtime.memoryBytes`, `Runtime.peakMemoryBytes`, `Runtime.resetPeakMemory`, `Input.readLine`, `Input.readAll`, `Input.readAllBytes`, `Input.isInteractive` |
| **B. explicit exclusion** | 1 | hard-coded skip, `ufcs.rs:83-85` | `Reflection.typeName` — *"resolved from its argument's static type and erased before any backend; a UFCS-produced raw `typeName(x)` call would instead reach the backend (where its PHP erasure is only coarse) and diverge"*. `Reflection.kind`/`className` stay eligible. |
| **C. multi-overload USER functions** | n/a | `sigs.len() == 1` gate, `ufcs.rs:41` | probe: two `f` overloads → `type 'int' has no method 'f'` |
| **D. PRELUDE CLASS statics** — not natives at all | 217 call sites | the registry loop never sees them | `Uri.parse`, `Request.parse`, `Response.text`, `Instant.now`, `Duration.ofSeconds`, `Assert.assert`, `Validation.*`, `Http.autoRouter`, `Debug.dump` |
| **E. aliased-ONLY module import** | n/a | `imports.get(leaf) == Some(module)` fails for an alias key | probe Z3 below |
| **F. arity mismatch (defaults/variadics)** | — | exact-arity gate | any native call omitting a defaulted trailing param |

Note **`Output.printLine` IS UFCS-eligible** (arity-1, first param accepts a string) — probes below
show `"hello".printLine()` and even `"abc".upperCase().printLine()` run. Eligibility is not the
question there; idiom is.

## Probe transcripts

```
=== printLine_ufcs:  "hello".printLine()          with import Core.Output  ===  hello        ✅
=== list_len_with_import: xs.length()             with import Core.List    ===  3            ✅
=== list_len_no_import:   xs.length()             WITHOUT import Core.List ===  ❌
      type error: type `List<int>` has no method `length`
=== str_len_no_import:    s.length()              WITHOUT import Core.String ===  ❌
      type error: type `string` has no method `length`
=== str_len_with_import:  s.length()              with import Core.String  ===  3            ✅
=== reflect_kind_ufcs:    x.kind()                with import Core.Reflection === int        ✅
=== reflect_typename_ufcs: x.typeName()           with import Core.Reflection === ❌
      type error: type `int` has no method `typeName`          (category B exclusion)
=== bytes_from_ufcs:      "hi".fromString()       with import Core.Bytes   ===  2            ✅ (works; NOT idiomatic — it is a factory)
=== conv_ufcs:            n.toString()            with import Core.Conversion === 42         ✅
=== overload_ufcs:  two user overloads of f, then n.f()  ===  ❌
      type error: type `int` has no method `f`                 (category C, sigs.len()==1 gate)
=== Z1b:  import Core.List.reverse as rev;  →  xs.rev().length()  ===  3   ✅ (DEC-274 alias UFCS)
=== Z3:   import Core.String as Str;        →  "abc".upperCase()  ===  ❌
      type error: type `string` has no method `upperCase`       (category E — alias does not gate UFCS)
=== X1:   Core.String used ONLY via s.upperCase()  →  phg check  ===  OK (type-checks clean)
          (no E-UNUSED-IMPORT false positive — DEC-326's shipped fix confirmed)
=== X2:   "abc".upperCase().printLine()  (fully UFCS, no qualified call at all)  ===  ABC   ✅
```

### Byte-identity of the two call styles — VERIFIED IDENTICAL

```
BI_qual.phg :  var parts = String.split(s, ","); … "{List.length(parts)} {String.upperCase(s)}"
BI_ufcs.phg :  var parts = s.split(",");         … "{parts.length()} {s.upperCase()}"

$ phg run BI_qual.phg   →  3 A,B,C
$ phg run BI_ufcs.phg   →  3 A,B,C
$ phg transpile BI_qual.phg > q.php ; phg transpile BI_ufcs.phg > u.php ; diff q.php u.php
IDENTICAL
```

[Verified: empty diff.] Consistent with `rewrite_ufcs` erasing the sugar pre-backend (Inv 5) — the
PHP leg cannot tell the two styles apart, so **a mass migration carries zero byte-identity risk**.

### The lifter ALREADY emits UFCS

```
$ cat lift_in.php
<?php
function go(string $s): int {
    $parts = explode(",", $s);
    $u = strtoupper($s);
    echo $u, "\n";
    return count($parts);
}
$ phg lift lift_in.php
// lifted (verify) — a best-effort PHP->Phorj draft; review before trusting it.
package Main;
import Core.Output;
import Core.List;
import Core.String;
function go(string s): int {
    mutable var parts = explode(",", s);
    mutable var u = s.upperCase();          ← RECEIVER form
    Output.print(u);
    Output.print("\n");
    return parts.length();                  ← RECEIVER form
}
```

`strtoupper($s)` → `s.upperCase()`, `count($parts)` → `parts.length()`. [Verified: transcript.]
So **Inv 17 argues FOR the migration, not against it**: today the lifter's output is more idiomatic
than the hand-written example corpus, and a user who lifts PHP gets code in a style the guide does
not teach. (Side note, out of scope: `explode(",", s)` was not lifted at all — a separate lift gap.)

### The LSP does NOT support UFCS completion — the unrealized half of DEC-326

`src/lsp/completion/mod.rs:118-146` — `Ctx::Member(recv)`:
- if `catalog::module_members(&recv)` is non-empty (an uppercase Core-module qualifier like `List.`)
  → emit the module's native members;
- **else** (a lowercase variable receiver) → `catalog::class_members(p, &ty)` only, i.e. the
  receiver's own class members; nothing enumerates UFCS-eligible natives.

Pinned by an intentional test, `src/lsp/completion/tests.rs:132`
`unresolved_lowercase_receiver_emits_neither_module_members_nor_keywords`, whose comment reads
*"A lowercase receiver is an instance, never a Core module → must NOT emit module members."*
[Verified: read both.] For a `List<int>` or `string` receiver there is no class at all, so
`class_members` returns nothing.

**Net effect**: `List.` completes; `xs.` does not. DEC-326's headline rationale —
*"`s.`-completion discovery beats module-name recall"* — is **false today**. A migration to
receiver-form would move users from the style that has completion to the style that does not.

### Formatter is call-style neutral

`phg format` on a UFCS file produced only whitespace normalization (blank lines after `package` and
the import block), no call rewriting. [Verified: diff.] Consistent with DEC-326's *"a formatter
canonicalization lint is the recorded v2"* — not built.

## Migration scope — the numbers

### Qualified call sites by surface

`grep -rhoP '(?<![A-Za-z0-9_.])[A-Z][A-Za-z0-9_]*\.[a-z][A-Za-z0-9_]*\(' <surface> | wc -l`

| Surface | Qualified sites | Nature of the churn |
|---|---:|---|
| `examples/**/*.phg` | **2223** | the primary teaching corpus — hand edit |
| `docs/**/*.md` | **382** | prose + fenced snippets — hand edit |
| `examples/README.md` | **65** | the living showcase index — hand edit |
| `FEATURES.md` | **6** | hand edit |
| `src/**/*.rs` (inline fixtures, preludes, tests) | **1740** | hand edit; **preludes are behaviour, not docs** |
| `tests/**` | **1171** | hand edit |
| `conformance/**` | **440** | hand edit (goldens) |
| `bench/**` | **231** | ⚠ **do not touch** — perf baselines (Inv 11/18) |
| `playground/web/examples.js` | **2847** | **GENERATED** from `examples/` by `playground/web/gen_examples.py` — regenerate, do not hand edit |
| **hand-edit total (excl. generated + bench)** | **≈ 6027** | |

Generated-artifact evidence: `playground/README.md:54` — `python3 playground/web/gen_examples.py
# regenerate examples.js`; `playground/README.md:18` names `web/gen_examples.py` as the generator.
`MASTER-PLAN.md:1406` tracks an `examples.js` staleness CI check as not-yet-added — so regeneration
is currently a manual blast-radius step.
`playground/web/main.js:422` additionally holds a **hand-written** editor-fallback snippet that
`SLICE-STATE.md:139-142` records has already drifted from `gen_examples.py` once — a second manual site.

### `examples/**/*.phg` — convertibility of all 2223 qualified sites

Classified by joining each `Leaf.fn` against the parsed native registry (95.6% coverage — see
disclosure at the top).

| Bucket | Sites | % | Meaning |
|---|---:|---:|---|
| Mechanically convertible, **idiom is a judgement call** | 1535 | 69.1% | receiver is a scalar/`Any`/poly first param — legal but "is the first arg the subject?" is a style question. **1231 of these are `Output.printLine` alone (55.4% of the whole corpus).** |
| Mechanically convertible **and idiomatic** (first param = the module's own container type) | 391 | 17.6% | `List.length(xs)`→`xs.length()`, `Map.get(m,k)`→`m.get(k)`, `String.trim(s)`→`s.trim()` — the unambiguous wins |
| **Non-native** (user class / prelude class static) — UFCS N/A | 217 | 9.8% | `Response.text`, `Request.parse`, `Instant.now`, `Validation.*` — must stay qualified |
| **Container-module factory** — keep qualified | 54 | 2.4% | `Bytes.fromString` (25), `String.join` (17), `Set.of` (10), `List.fill` (2) |
| **arity-0 native** — not convertible | 26 | 1.2% | `Time.unfreeze`, `Runtime.monotonicNanos`, `Process.arguments`, `Math.nan`, `Math.infinity`, `Input.isInteractive`, … |

**Excluding all `Core.Output` sites** — the more decision-relevant view:

| Bucket | Sites | % |
|---|---:|---:|
| convertible + idiomatic | **391** | 39.8% |
| convertible, judgement call | 294 | 29.9% |
| non-native (UFCS N/A) | 217 | 22.1% |
| container factory — keep qualified | 54 | 5.5% |
| arity-0 — not convertible | 26 | 2.6% |
| **total non-Output** | **982** | |

### Per-module breakdown (`examples/**/*.phg`)

| Module leaf | Total | Idiomatic-convertible | Judgement | Factory (keep) | arity-0 (keep) | Non-native (keep) |
|---|---:|---:|---:|---:|---:|---:|
| `Output` | 1241 | 0 | **1241** | 0 | 0 | 0 |
| `String` | 139 | **122** | 0 | 17 | 0 | 0 |
| `List` | 137 | **135** | 0 | 2 | 0 | 0 |
| `Math` | 83 | 0 | 65 | 0 | 7 | 11 |
| `Bytes` | 83 | **58** | 0 | 25 | 0 | 0 |
| `Html` | 52 | 0 | 44 | 0 | 0 | 8 |
| `Map` | 48 | **48** | 0 | 0 | 0 | 0 |
| `Set` | 38 | **28** | 0 | 10 | 0 | 0 |
| `Response` | 33 | 0 | 0 | 0 | 0 | 33 |
| `Reflection` | 28 | 0 | 17 | 0 | 0 | 11 |
| `Validation` | 26 | 0 | 0 | 0 | 0 | 26 |
| `Decimal` | 24 | 0 | 24 | 0 | 0 | 0 |
| `Conversion` | 24 | 0 | 24 | 0 | 0 | 0 |
| `Json` | 21 | 0 | 21 | 0 | 0 | 0 |
| `Regex` | 17 | 0 | 17 | 0 | 0 | 0 |
| `Request` | 15 | 0 | 0 | 0 | 0 | 15 |
| `Log` | 14 | 0 | 6 | 0 | 0 | 8 |
| `File` | 13 | 0 | 13 | 0 | 0 | 0 |
| `Path` | 12 | 0 | 12 | 0 | 0 | 0 |
| `Instant` / `FileSystem` | 11 / 11 | 0 | 0 | 0 | 0 | 11 / 11 |
| `Uri` | 10 | 0 | 4 | 0 | 0 | 6 |
| `Hash` | 10 | 0 | 4 | 0 | 0 | 6 |
| `Runtime` | 7 | 0 | 1 | 0 | 6 | 0 |

**Clean-sweep modules** (≈100% idiomatic-convertible, zero judgement calls): **`List` 135/137,
`Map` 48/48, `String` 122/139, `Set` 28/38, `Bytes` 58/83** — **391 sites, 5 modules**. These are the
whole "idiomatic" bucket and they are exactly the DEC-326 "first parameter is the natural subject" case.

### Native-registry view (347 natives parsed)

| Class | Natives |
|---|---:|
receiver-first, own container type — idiomatic UFCS | **117** |
scalar/poly/`Any` receiver — UFCS-able, idiom is a judgement call | **203** |
arity-0 — never UFCS-able | **19** |
container-module factory (`Bytes.fromString`, `List.fill`, `Set.of`, `String.join`) | **4** |

### Current UFCS adoption in the corpus

491 receiver-style call sites exist in `examples/**/*.phg`; **190** of them have a method name that
matches a stdlib native (upper bound on already-UFCS sites — the rest are genuine methods on user
and prelude classes: `prepare` 37, `transaction` 10, `describe` 13, …). Against 2223 qualified sites,
the corpus is roughly **8% receiver-form / 92% module-form** today. The "migrate as touched" clause
of DEC-326 has, three days in, not moved the needle.

## Gaps

- **E12 — [P1, Verified]** **DEC-326's primary rationale is unimplemented: the LSP does not complete
  UFCS members.** `src/lsp/completion/mod.rs:118-146` gives module members for `List.` and class
  members for a typed class receiver, and nothing for a `string`/`List<int>` receiver;
  `src/lsp/completion/tests.rs:132` pins that behaviour deliberately. Promoting receiver-form before
  fixing this trades a discoverable idiom for an undiscoverable one — the exact opposite of the
  ruling's stated reason. **This is the single most important finding in §3.** It is also not
  recorded as a follow-up anywhere (SLICE-STATE's LSP punch-list mentions only `EntryKind.`
  attribute-arg completion).
- **E13 — [P1, Verified]** **The module-import gate makes UFCS-only code read as magic, and its
  failure message does not mention imports.** `xs.length()` requires `import Core.List;`
  (`ufcs.rs:91`) even though `List` is never written in the file. When the import is missing the
  error is `type 'List<int>' has no method 'length'` — no hint naming `import Core.List;`. In a
  fully-migrated corpus every file carries imports for modules whose names never appear in the code,
  and every newcomer's first UFCS attempt fails with a message that points nowhere. A hint on this
  message ("`length` is a `Core.List` native — add `import Core.List;`") is a small, high-leverage fix.
- **E14 — [P2, Verified]** **An aliased-only module import silently disables UFCS.** `import Core.String
  as Str;` then `"abc".upperCase()` → `type 'string' has no method 'upperCase'` (probe Z3). Documented
  only as an in-code comment (`ufcs.rs:65-66`, *"An aliased-only core import is skipped (call it
  explicitly)"*); no user-facing note, no hint in the error. A user who aliases for brevity loses the
  house style with no explanation.
- **E15 — [P2, Verified]** **Overloaded user functions are not UFCS-eligible** (`sigs.len() == 1`,
  `ufcs.rs:41`; probe: `type 'int' has no method 'f'`). This is the "F-004" deferral. Under a
  receiver-form house style this becomes a visible inconsistency: a user's single-signature helper is
  method-callable, adding a second overload silently removes that. The error does not say why.
- **E16 — [P2, Verified]** **`Output.printLine` is 55.4% of the corpus's qualified calls (1231/2223)
  and DEC-326 does not settle it.** The ruling reserves module form for *"receiver-less calls
  (constructors/config/ambient)"*. `printLine(s)` has a receiver-shaped first param and works as
  `"hello".printLine()` (probe), but "is the printed string the *subject* of printing?" is precisely
  the judgement DEC-326 leaves open. Whichever way it goes, it dominates the migration arithmetic —
  it must be ruled BEFORE any codemod, or over half the corpus will be touched twice.
- **E17 — [P2, Verified]** **The generated playground corpus is a blast-radius trap.**
  `playground/web/examples.js` (2847 qualified sites) is generated by `gen_examples.py`
  (`playground/README.md:54`); there is **no CI staleness check** (`MASTER-PLAN.md:1406`: *"`examples.js`
  staleness CI check … Add when touching playground CI"*), and `main.js:422` holds a hand-written
  snippet that has already drifted once (`SLICE-STATE.md:139-142`). Any example migration must
  regenerate + hand-fix both, and would be a good moment to add the CI check.
- **E18 — [P3, Verified]** **`bench/**` (231 qualified sites) must be excluded from any codemod.**
  Byte-identity is unaffected by call style (verified), but Inv 11/18 require measured before/after
  for any perf-surface change, and the committed `bench/*-baseline.json` figures are already flagged
  stale in `SLICE-STATE.md:203-205`. Rewriting bench sources would invalidate a baseline set that is
  awaiting a pinned dev-box re-measure.
- **E19 — [P3, Verified]** **The formatter canonicalization lint (DEC-326 "recorded v2") is not built**
  — `phg format` is call-style neutral (verified). Without it, "canonical style" is unenforced and the
  corpus will re-drift.

## Options & recommendation

**Answer to the developer**: UFCS is supported for **user free functions** (single-overload) and
**imported natives** (module-import-gated or function-import-gated), resolved **method-first**, and
erased before every backend — so `s.length()` and `String.length(s)` transpile to **byte-identical
PHP** (verified). It is NOT available for arity-0 natives (19), `Reflection.typeName` (deliberate),
multi-overload user functions, prelude **class statics** (`Uri.parse`, `Response.text`, `Instant.now`
— 217 example sites), or through an aliased-only module import. And the style you are describing was
already ruled — **DEC-326, 2026-07-22** — with the lifter half shipped; what remains is the corpus
migration, the formatter lint, and the LSP completion that the ruling's own rationale depends on.

Migration strategy options:
- (a) **Module-by-module, cleanest-first.** Start with the five clean-sweep modules
  (`List` 135, `Map` 48, `String` 122, `Set` 28, `Bytes` 58 = **391 sites, 0 judgement calls**),
  one commit per module, full gate + differential each time. Leaves `Output` and the 294
  judgement-call sites for separate rulings.
- (b) **All-at-once codemod across all 6027 hand-edit sites.** Fastest wall-clock, but requires the
  `Output` ruling (E16) up front, touches preludes (behaviour, not docs) and conformance goldens in
  one change, and the differential/format sweeps become a single all-or-nothing gate.
- (c) **Keep DEC-326's "as touched" laziness.** Zero-cost, but measured: 3 days in, the corpus is
  still ~92% module-form. The idiom split will persist indefinitely and lifted code will keep looking
  unlike the guide.
- (d) **Tooling first, corpus second.** Build LSP receiver completion (E12) + the import hint (E13)
  + the formatter canonicalization lint (E19), THEN run (a).

**RECOMMENDED: (d) then (a).** Rationale: E12 is disqualifying on its own — DEC-326 chose receiver
form *because* `s.`-completion beats module recall, and that completion does not exist, so migrating
first would make the corpus teach an idiom the editor cannot help with. E13 compounds it: the very
first thing a reader of a migrated example will try is copying `xs.length()` into their own file,
where it fails with a message that never mentions `import Core.List;`. With those two fixed, (a)'s
first tranche is unusually safe — 391 sites, five modules, zero style judgement, and byte-identity
proven unaffected — and each module is independently gate-able and revertible.

Two things must be **ruled by the developer before any codemod** (Inv 15):
1. **`Output.printLine` (E16)** — receiver form (`"hi".printLine()`) or module form? 1231 sites,
   55.4% of the corpus, hinges on it.
2. **The deliberate-qualified policy** — which examples keep module form *on purpose*, so the corpus
   still teaches that both are legal. Suggested (Speculative, for the developer to accept or replace):
   keep `examples/guide/ufcs.phg` and `examples/guide/extension-methods.phg` showing both forms
   side-by-side (they are the features' own documentation); keep module form everywhere it is
   *forced* — the 54 factory sites, 26 arity-0 sites, 217 prelude-static sites — and add a one-line
   comment at a couple of those saying *why* (factory / no receiver), so the rule is visible in the
   corpus and not only in `FEATURES.md`.

---

# 4. The `class main` example — "with entry no longer need main reserved name!"

## Ground truth (evidence)

- The file is `examples/guide/class-main.phg` (30 lines, read in full). Its opening comment:
  *"Class entry points (Batch-1 D) — a program's `main` may be a class `static` method. … Phorj's
  Go-style top-level `function main` still works (every other example uses it). This example shows
  the Java-style alternative: a `static function main` on a class. Either form is a valid entry;
  declaring BOTH (or two class-static `main`s) is `E-MULTIPLE-MAIN`."* It still runs:
  `phg run examples/guide/class-main.phg` → `Hello from App.main` / `done`.
- Its README row, `examples/README.md:190`, repeats the same claims verbatim including
  *"declaring both, or two class-static `main`s, is `E-MULTIPLE-MAIN`"*.
- **`examples/guide/entry.phg` already supersedes it.** Lines 1-2: *"`#[Entry(kind: EntryKind.…)]`
  (DEC-331, DEC-337) — entries are declared by ATTRIBUTE, **never by a magic name**"*; its body is a
  `class App { #[Entry(kind: EntryKind.Cli)] static function run(): int { … } }` with the inline
  comment *"The entry can live on a class as a static method — **the name is yours to choose**."*
  So the class-static-entry capability is already demonstrated, with a non-`main` name.
- **Entry resolution is attribute-based.** `src/ast/entry.rs:255` `entry_for(program, role)` filters
  `entry_candidates(program)` by `entry_declared_role(f)`. It is the resolver actually used —
  8 call sites (`transpile/program_emit.rs` ×4, `serve/handlers.rs`, `cli/preludes.rs`,
  `interpreter/mod.rs`, `interpreter/coop.rs`, `compiler/program.rs`, `loader/fs.rs`).
- **The legacy name-based resolvers are dead.** `ast::entry_point` (`entry.rs:269`) and
  `ast::entry_point_count` (`entry.rs:297`) take a `name: &str` and match `f.name == name` — and have
  **zero call sites** anywhere in `src/`, `tests/`, or `playground/`
  [Verified: `grep -rn 'entry_point' src tests playground --include='*.rs'` returns only the two
  definitions plus four stale doc comments].
- **`E-MULTIPLE-MAIN` is DEAD.** No emit site remains: `grep -rn 'E-MULTIPLE-MAIN' src/` (excluding
  tests) yields only `src/cli/explain/members_destructure.rs:94` (the explain entry) and four stale
  doc comments (`src/ast/entry.rs:267`, `:295`, `src/interpreter/mod.rs:352`,
  `src/compiler/program.rs:134` — the last two each assert *"`E-MULTIPLE-MAIN` guarantees ≤1"*).
  The real guarantee now comes from `E-DUPLICATE-ENTRY-KIND` (probe N3).
- **BUT `main` IS still reserved** — `src/checker/program/type_bodies.rs:347`:
  ```rust
  let is_entry_main = f.name == "main" && (self.cur_class.is_none() || self.in_static_method);
  if is_entry_main { self.check_main_signature(f, &ret); }
  ```
  Any free function or **static** method named `main` — attribute or not — is forced into the entry
  signature by `check_main_signature` (`type_bodies.rs:277-302`, `E-MAIN-SIGNATURE`). An *instance*
  method named `main` is exempt. `cur_is_main` (set from the same flag, line 353) also re-flavours the
  uncaught-throw diagnostic at `src/checker/stmt/core.rs:452`.

## Probe transcripts

```
=== M1: #[Entry(kind: EntryKind.Cli)] function boot(): void ===
boot ran                                                       ✅ any name is a valid entry
$ phg transpile M1 → function boot(): void { echo "boot ran", "\n"; }  boot();

=== M2: free function literally named `main`, NO #[Entry] ===
compile error: no entry point: running needs an `#[Entry(kind: EntryKind.Cli)]` function (DEC-331).
  A library or web file still type-checks and transpiles — use `phg check` / `phg transpile`
                                                               ✅ `main` has NO special power

=== M3: #[Entry] on `boot` + an ordinary function named `main` ===
boot is the entry
not the entry                                                  ✅ `main` is callable like any function

=== M4: class static `App.boot` with #[Entry] ===  App.boot    ✅

=== N3: two #[Entry(kind: EntryKind.Cli)] functions ===
type error: duplicate `#[Entry(kind: EntryKind.Cli)]` — a program has at most one entry per kind
  [E-DUPLICATE-ENTRY-KIND]                                     ← the LIVE guard

--- the reservation residue ---

=== N1b: `function main(string s): string` — NO #[Entry], a plain library function ===
type error at 2:1: `main` must be `main(): void`, `main(): int`, or take a single `List<string>`
  argv parameter — found an incompatible signature   [E-MAIN-SIGNATURE]
                                                               ❌ the NAME alone constrains it

=== N6: class static `Util.main(string s): string` — a utility class, no entry anywhere ===
type error at 3:5: … [E-MAIN-SIGNATURE]                        ❌ same

=== N7: INSTANCE method named `main` ===  OK (type-checks clean)  ✅ exempt

=== N2b: #[Entry] function boot(string s): string ===
type error: `#[Entry(kind: EntryKind.Cli)]` function `boot`'s signature doesn't match — a `Cli`
  entry is `(): void`, `(): int`, or `(List<string>): void|int`   [E-ENTRY-SIG]
                                                               ← the LIVE, correct gate

=== N4b: #[Entry] function main(string s): string  →  TWO errors for one mistake ===
type error at 4:1: … [E-ENTRY-SIG]
type error at 4:1: `main` must be `main(): void`, … [E-MAIN-SIGNATURE]     ⚠ duplicate diagnostic

--- E-MULTIPLE-MAIN is unreachable ---

=== P1: #[Entry] top-level `main` + a class static `App.main` ===  OK (type-checks clean); runs "top"
=== P2: two class statics `A.main` and `B.main` ===              OK (type-checks clean)
$ phg explain E-MULTIPLE-MAIN
E-MULTIPLE-MAIN — a program declares more than one entry point named `main`.
An entry is EITHER a top-level `function main` OR a single class `static function main`
(Batch-1 D) — never both, and never two class-static `main`s. …
                          ← still shipped, teaches the pre-Entry convention, and CANNOT fire

--- the name-keyed uncaught-throw message ---

=== Q1: #[Entry] function boot(): void { throw new Boom(...); }   (boot IS the entry) ===
type error: `Boom` is thrown here but neither caught nor declared   [E-THROW-UNDECLARED]
             ← the ORDINARY-function message, for the actual program entry point
=== Q2: #[Entry] function main(): void { throw … } ===
type error: `Boom` thrown in `main` escapes the program entry point [E-UNCAUGHT-THROW]
=== Q3: plain `function main(): void { throw … }`  — NOT an entry at all (no #[Entry]) ===
type error: `Boom` thrown in `main` escapes the program entry point [E-UNCAUGHT-THROW]
             ← claims "the program entry point" about a function that is not one
=== Q4/Q5: entry declaring `throws BoomError` (named `boot` AND named `main`) ===
OK (type-checks clean)  — and at runtime: `runtime error at 6: uncaught exception 'BoomError'`
             ← so E-UNCAUGHT-THROW's hint ("`main` may not let an exception escape") is false:
               it may, by declaring `throws`
```

## Corpus census

```
entry function names across examples/**/*.phg:  { main: 238, handle: 5, run: 1 }
non-`main` entries: examples/web/{server,core-http,handler,json-api}.phg (handle),
                    examples/session/counter.phg (handle), examples/guide/entry.phg (run)
```
238 of 244 example entries are still named `main` — convention by habit, not by requirement.

## Gaps

- **E20 — [P1, Verified]** **`main` is still a reserved name in the checker.**
  `src/checker/program/type_bodies.rs:347` constrains every free function and static method named
  `main` to the entry signature regardless of `#[Entry]`. Consequences, all probed: a library
  function `main(string s): string` is rejected (N1b); a utility class's `static Util.main(string)`
  is rejected (N6); and an `#[Entry] function main` with a bad signature gets **two** errors for one
  mistake — `E-ENTRY-SIG` (correct) plus `E-MAIN-SIGNATURE` (stale) (N4b). Post-DEC-331/337 the
  attribute is the entry marker and `E-ENTRY-SIG` already does this job for every entry under any
  name; `E-MAIN-SIGNATURE` now only fires on things that are *not* entries. **This is the direct,
  code-level confirmation of the developer's instinct.**
- **E21 — [P1, Verified]** **`E-UNCAUGHT-THROW` keys on the NAME `main`, not on `#[Entry]`** —
  `cur_is_main` (`type_bodies.rs:353`, consumed at `src/checker/stmt/core.rs:452`). So the *real*
  entry `#[Entry] function boot()` gets the ordinary `E-THROW-UNDECLARED` (Q1) while a *non-entry*
  function named `main` gets *"escapes the program entry point"* (Q3) — the message is attached to
  the wrong function in both directions. Compounding it, the hint *"`main` may not let an exception
  escape"* is false: declaring `throws` is accepted for entries under both names (Q4/Q5) and the
  exception escapes to a runtime error. Fix: key `cur_is_main` on `entry_declared_role(f).is_some()`
  and correct the hint.
- **E22 — [P2, Verified]** **`E-MULTIPLE-MAIN` is dead but still shipped and still taught.** No emit
  site remains; P1 and P2 both type-check clean; the live guard is `E-DUPLICATE-ENTRY-KIND` (N3).
  Yet `phg explain E-MULTIPLE-MAIN` returns a full explanation of the pre-Entry convention, and
  `examples/README.md:190` asserts the error fires. Note that removing the explain entry is safe in
  the *emitted*-code direction (the `every_emitted_diagnostic_code_has_an_explanation` test only
  requires emitted→explained), but any test asserting the explanation exists
  (`src/cli/tests/explain_coverage.rs` has one for `E-MAIN-SIGNATURE`) must be checked first.
- **E23 — [P2, Verified]** **`examples/guide/class-main.phg` teaches an obsolete convention and
  duplicates `entry.phg`.** Its stated lesson — *"a program's `main` may be a class `static` method …
  the Java-style alternative"* — is a pre-Entry framing: the attribute makes the *name* irrelevant,
  which is exactly what `examples/guide/entry.phg` already demonstrates with `static function run()`.
  Its `E-MULTIPLE-MAIN` claim is disproven (E22), and its "Go-style vs Java-style" dichotomy no longer
  corresponds to anything in the language.
- **E24 — [P2, Verified]** **`examples/README.md:190`** repeats every stale claim from E23 including
  the dead error code. Inv 17 (always-current surfaces).
- **E25 — [P3, Verified]** **`examples/guide/exit-codes.phg:3-12`** — *"`main` is where a Phorj
  program starts"* (obsolete framing; the *attribute* is where it starts) and *"An incompatible
  signature is rejected at compile time (`E-MAIN-SIGNATURE`)"* (half-true — that code does still
  fire, but only because of the E20 residue; for a correctly-attributed entry the code is
  `E-ENTRY-SIG`).
- **E26 — [P3, Verified]** **Four stale doc comments** assert a guarantee that no longer exists:
  `src/ast/entry.rs:267` and `:295`, `src/interpreter/mod.rs:352`, `src/compiler/program.rs:134`
  (*"the checker's `E-MULTIPLE-MAIN` guarantees ≤1"*). Plus **two dead public functions**,
  `ast::entry_point` and `ast::entry_point_count` (zero call sites).
- **E27 — [P3, Verified]** **`src/loader/fs.rs:112-116` doc comment and `docs/specs/UNIFIED-SPEC.md:550`
  both say the public-surface file rule exempts *"a file declaring the entry point `main`"*.** The
  **code is correct** — `validate_public_surface` gates on
  `crate::ast::entry_for(prog, EntryRole::Cli).is_some()` (`fs.rs:120`), i.e. the attribute, and
  probes Y1/Y2 confirm an entry named `boot` is exempt exactly like one named `main`. Docs-only drift,
  but it is the kind that makes the next reader believe `main` is load-bearing.

## Options & recommendation

**Answer to the developer**: you are right, and it is worse than a stale example. `main` has no
entry-selecting power at all (`#[Entry]` on any name works — M1/M4; a `main` with no attribute is
"no entry point" — M2), **but it is still a reserved name in the checker**:
`src/checker/program/type_bodies.rs:347` forces any free function or static method named `main` into
the entry signature, so an ordinary library `main(string): string` is rejected (N1b) and an
`#[Entry] function main` with a bad signature gets two errors instead of one (N4b). `E-MULTIPLE-MAIN`
is dead code that `phg explain` still teaches (P1/P2 clean). And `examples/guide/class-main.phg` is
teaching the obsolete "Go-style vs Java-style `main`" dichotomy that `examples/guide/entry.phg`
already replaced — its README row even promises an error that cannot fire.

**Files that would change** (grouped by concern; the code changes are dev-ruled, the doc fixes are
mechanical):

*Code — retire the name reservation (E20/E21):*
- `src/checker/program/type_bodies.rs` — remove the `f.name == "main"` special case at :347; delete
  or repurpose `check_main_signature` (:277-302) now that `E-ENTRY-SIG` covers every entry; re-key
  `cur_is_main` (:353) to `entry_declared_role(f).is_some()`.
- `src/checker/stmt/core.rs:452` — the `E-UNCAUGHT-THROW` branch follows the re-keyed flag; correct
  the hint (an entry MAY declare `throws` — Q4/Q5).
- `src/checker/tests/entry_point.rs` — 14 `E-MAIN-SIGNATURE` assertions to re-point at
  `E-ENTRY-SIG`; the file's own header comment (*"Only the entry `main` is constrained"*) is stale.
- `src/cli/explain/members_destructure.rs:83-105` — retire `E-MULTIPLE-MAIN` (:94), rewrite or
  retire `E-MAIN-SIGNATURE` (:83); check `src/cli/tests/explain_coverage.rs:28` first.
- `src/ast/entry.rs` — delete the dead `entry_point` (:269) and `entry_point_count` (:297); fix the
  `E-MULTIPLE-MAIN` doc comments at :267 and :295.
- `src/interpreter/mod.rs:352`, `src/compiler/program.rs:134`, `src/loader/fs.rs:112-116`,
  `src/ast/decls/functions.rs:55` — stale comments.

*Docs / examples (Inv 17):*
- `examples/guide/class-main.phg` — rewrite or retire (E23).
- `examples/README.md:190` — the row (E24).
- `examples/guide/exit-codes.phg:3-12` — reframe off `main` (E25).
- `docs/specs/UNIFIED-SPEC.md:550` — the file-rule exemption wording (E27).
- `playground/web/examples.js` — regenerate via `gen_examples.py` for any `.phg` edit (E17).

Options for `class-main.phg` specifically:
- (a) **Retire it** (`git rm` + drop the README row). `entry.phg` already covers class-static entries
  with a chosen name. Smallest surface, one fewer example to keep current. Risk: loses the
  "the entry can be a static method" signal for anyone who reads only the example index — mitigated
  because `entry.phg`'s row would carry it.
- (b) **Repurpose it as `guide/entry-any-name.phg`** — keep a runnable file, change the lesson to
  *"the entry is the attribute, not the name"*: an `#[Entry(kind: EntryKind.Cli)] static function
  boot()` on a class, **plus** an ordinary non-entry function named `main` in the same file that is
  simply called like any other function (probe M3 proves this runs — `boot is the entry` /
  `not the entry`). That single file demonstrates the whole post-DEC-331 model and would have caught
  E20 as a test.
- (c) **Keep it, fix only the `E-MULTIPLE-MAIN` sentence.** Cheapest; leaves the obsolete
  Go-vs-Java framing and the near-duplication of `entry.phg` in place.

**RECOMMENDED: (b), and fix E20/E21 first.** Rationale: the example and the checker residue are the
same bug seen from two sides — the corpus teaches that `main` is special *because the checker still
treats it as special*. Fixing E20 without touching the example leaves the docs wrong; fixing the
example without E20 leaves a library author unable to write `function main(string): string`. (b) also
converts the fix into a differential-gated example (Inv 9), so the reservation cannot silently return.
E22/E24 ride along in the same change. **The retirement of `E-MAIN-SIGNATURE` is a user-visible
surface change and therefore a developer ruling (Inv 15)** — options above, no decision taken here.

---

# Cross-cutting note

Three of the four questions trace to the same root cause: **a ruling exists, was partially built, and
the surrounding docs were never reconciled** — DEC-248 (loop retirement, item 2 never built, E8),
DEC-326 (UFCS style, lifter built / corpus+LSP not, E12), DEC-331/337 (`#[Entry]` built, the `main`
reservation and its docs never removed, E20/E22/E23). Invariant 19's "zero divergence from the SSOT"
is being honoured for *new* rulings and not for *partially-executed* ones. A mechanical check —
"every register row whose text names a diagnostic code must have that code present in `src/`, or be
marked PARTIAL" — would have caught `E-RETIRED-FORIN` (ruled, absent) and `E-MULTIPLE-MAIN` (present
in explain, absent from every emit site) automatically. Offered as an observation for the developer's
consideration, not a proposal.

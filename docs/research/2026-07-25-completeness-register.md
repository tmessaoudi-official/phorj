# COMPLETENESS REGISTER — 2026-07-25 (the DV-5 ranked research pass)

**Status:** RESEARCH COMPLETE — **0 decisions taken** (Invariant 15). Every fork below is PENDING and
carries a recommendation + the why. Ruling rows live in the decision register
(`docs/research/full-audit/raw/C-decisions.md`, **DEC-339 … DEC-365** — 27 rows); this file holds the *analysis* and
the ready-to-ask question text. One canonical home each — Invariant 19, no duplicated content.

**What produced it.** The developer reviewed the project himself, produced ~15 findings/questions, and
asked for them to be challenged and verified against real code, widened into a global project review, and
turned into an agenda he can rule on one item at a time — while he slept. This fulfils the already-RULED
**DV-5** pass (`docs/specs/2026-07-24-visibility-model.md`: *"global completeness sweep is its OWN research
pass … synthesized into ONE ranked completeness register"*).

**Evidence base:** `docs/research/2026-07-25-global-review/` (13 per-topic reports, every claim cited to
`file:line` and evidence-graded). Read that directory for detail; read this file to decide.

---

## 0. THE ONE THING THAT MATTERS MOST

**A P0 byte-identity break was found — new, unrecorded, and silently produces wrong output.**

Shadowing a live outer local **or parameter** inside *any* nested block mistranspiles, because phorj has
true lexical block scoping and PHP has none:

```phorj
int a = 1;
if (true) { int a = 2; Output.printLine("in={a}"); }
Output.printLine("out={a}");
```

`phg run` → `out=1` · `phg run --tree-walker` → `out=1` · transpiled PHP → **`out=2`**

Six shapes verified on all three legs (bare block, `if`, `for`, `while`, parameter shadow, 3-deep nesting).
It escaped the gate because `tests/differential.rs` globs `examples/**/*.phg` and **no example shadows a
variable** — so the language's block-scoping semantics have zero spine coverage. Details:
`2026-07-25-global-review/P0-block-shadow-byte-identity.md`. Decision → **GR-1** below.

---

## 1. STATUS OF THE DEVELOPER'S OWN 15 FINDINGS

Verdict column is the honest answer, not a restatement of the question.

| # | His finding | Verdict | Detail |
|---|---|---|---|
| 1 | "`package X.X` is not enforced" | **PARTLY RIGHT — better root cause found** | Enforcement follows the **import graph, not the file**: `src/loader/entry.rs:53-66` returns on a "no user imports" fast path *before* `assemble()`, so all three validators are skipped. `package Foo.Bar;` alone → OK; **the same file plus one user import** → `E-PKG-PATH`. `Core.*` imports don't count, so the shape hand-tested most is the unvalidated one. ⚠ The loose-file-must-be-`Main` rule he remembers was **deliberately retired by DEC-282** — not a regression. |
| 2 | "attribute for the file, so it starts with it?" | **NOT POSSIBLE TODAY** | No file-level/inner attribute grammar exists at all (`#[…]` only before a top-level `function`/`class`; verified `E-ATTR-TARGET`). `#![Package(Main)]` at byte 0 is **silently eaten** by the DEC-282 shebang skip. Recommended shape: `#[Loose] package Foo.Bar;` |
| 3 | "UFCS gives no autocomplete" | **CONFIRMED — but already on record** | `line.` on a `string` → **0 items**. Already the LSP audit's punch-list rows #1/#2 (P1) with the same root cause + same fix sketch. This run corroborated it; it is **known and unbuilt**, not new. |
| 4 | "iterable `Core.Input` for any file size" | **HALF ALREADY DONE** | `Input.lines()` **already streams** — 88 MB / 2 M lines in **23.7 MB peak RSS** (measured), byte-identical on all 3 legs. The `Iterator<T>` protocol exists (DEC-257). **The gap is files only**: `FileSystem`/`File` are whole-slurp with no offset primitive, so users can't build the iterator. |
| 5 | "wildcard import not fully supported?" | **BUILT + CERTIFIED; one gap** | `*`, `* except {}`, group `{}`, group aliasing, deep packages all work for user *and* vendored packages (27-row probe matrix). Missing ruled surface: **stdlib wildcards** — `import Core.Text.*;` is parser-rejected "not yet supported" (P-Q-A-1). |
| 6 | ".phg parsed wrong in editors, light blue like a comment/string" | **CONFIRMED — root cause found (NEW)** | `phorj.tmLanguage.json:34` `"begin": "\\b(b\|r)?\""` — `\b` before an *optional* group fails at opening quotes and matches closing ones, so **every plain string starts at its CLOSING quote**. **81/383** `.phg` files end inside an unterminated span; 188/266 examples have code punctuation scoped as string. A verified 5-rule rewrite takes leakage to **0/383**. (His `#`-as-comment guess was explicitly disproved.) |
| 7 | "did we retire `for..in` or `foreach..in`?" | **NOTHING was retired; both live** | `for`…`in` ✅ and `foreach`…`as` ✅; only the *crossed* forms error. The retirement he remembers is **DEC-248** (ruled `for (T x in xs)` retired behind `E-RETIRED-FORIN`) — **never built** (`grep E-RETIRED-FORIN src/` → 0). Conflict **C-2** open since 06-25. Census: **87 `for…in` vs 8 `foreach…as`** — the corpus overwhelmingly teaches the form that was ruled retired. |
| 8 | "visibility/access in blocks inside a function?" | **AMBIGUOUS — 5 distinct features** | Bare blocks with real scoping already exist (and are the P0). Local functions, local classes, and visibility-on-locals are all parse errors. Every peer language with local functions **forbids access modifiers on them**. Needs disambiguation → **GR-14**. |
| 9 | "should `Database` be `Connection`?" | **YES — and provably** | The object is **one connection**, not a pool or façade: single `Box<dyn DriverConn>`, connection-scoped `tx_depth`/`hook`/`timeout_ms`, `grep pool` empty, pooling listed "out of scope". 8 of 10 ecosystems call this `Connection`/`Conn`/`Client`; `Database`/`DB` is what Go and Laravel use for the *pool/manager* phorj does **not** have. Bonus: DEC-278's `Module` suffix exists only because module leaf and type were namesakes — renaming dissolves that, so `Core.Database` can go bare. |
| 10 | "`.prepare()` — do we have a PreparedStatement?" | **YES, but his exact loop FAILS** | `Statement` exists with `bind`/`bindNamed`/`bindList`/`exec`/`executeMany`/`query`/`stream`. But **binds append and are never reset**, so iteration 2 dies: `2 bound value(s) but 1 ? placeholder(s)`. The same loop "works" with `bindNamed` (silent last-wins) and is **quadratic** — 8000 iters = 4.469s vs 0.059s for re-preparing (**~75× slower**). DEC-208 rejected the one-shot shape *because* it had "no Statement reuse" — that promise is unfulfilled and undocumented. |
| 11 | "savepoints — rollback all and discard?" | **NO — and a real data-loss bug found** | No `rollbackAll`, no `rollbackTo`, no way to observe depth; `rollback()` pops exactly one level. Worse: `db.transaction(fn)` auto-rollback **also pops one level only**, so a closure that leaked an inner `begin()` leaves the outer transaction OPEN with partial writes live — the error reads "rolled back", a row is still visible, and a later `commit()` **persists it**. His SQL instinct is right: a bare top-level `ROLLBACK` discards all savepoints, so `rollbackAll()` is one statement at any depth. |
| 12 | "filesystem lock when available?" | **NOTHING today — and the blocker is FALSE** | Zero locking code; writes unlocked on both legs. The presumed dependency-policy blocker **does not apply**: `std::fs::File::{lock, try_lock, unlock}` compile and run on the pinned rustc 1.97.1 (verified), so **no crate need be admitted**. Rust std locks and PHP `flock()` **block each other bidirectionally** (verified) — ladder case 1 with unusually strong evidence. Open risk: Windows is a shipped target and its lock semantics may be mandatory; no Windows CI. |
| 13 | "promote UFCS over qualified calls" | **ALREADY RATIFIED — DEC-326** | Ruled 2026-07-22; lifter half shipped. But DEC-326's stated rationale (*"`s.`-completion discovery beats module-name recall"*) **is false today** — that completion doesn't exist (finding #3). Corpus is **~8% receiver-form / 92% module-form**; "migrate as touched" hasn't moved. **2223** qualified sites in examples, ~6027 hand-edit total; **391 are zero-judgement conversions**, and **1231 are `Output.printLine` — 55.4% of the corpus**, which must be ruled *before* any codemod or half the corpus gets touched twice. |
| 14 | "example called class main, but Entry frees the name" | **RIGHT — `main` IS still reserved** | `checker/program/type_bodies.rs:347` forces any free function or static method named `main` into the entry signature **regardless of `#[Entry]`**; a library `main(string): string` is rejected. `E-MULTIPLE-MAIN` is **dead code** yet `phg explain` still teaches it. `class-main.phg` duplicates `entry.phg`, which already says entries are declared by attribute, never by a magic name. |
| 15 | "clone without modifying anything?" | **ALREADY WORKS** | `p with { }` → shallow clone, source untouched, transpiles to bare **`clone($p)`**, identical on both backends. Empty braces parse by construction and are test-pinned. Rejected spellings: `clone p`, `p.clone()`. Real debt is elsewhere: the **lifter refuses** `CloneWith` and PHP `clone` (live Invariant-17 gap), no DEC row, no spec grammar, formatter prints `with {  }` (double space). |

**Scorecard:** 6 confirmed as stated · 4 confirmed with a *better* root cause than suspected · 3 already
built or already ruled (so the ask is follow-through, not new work) · 1 inverted recollection (#7) ·
1 ambiguous (#8). **Net: his instincts were sound on 13 of 15.** The two misses (#1's retired rule, #7's
inverted loop memory) are both cases where a past *ruling* was never *built*, which is this project's
dominant failure mode — see §3.

---

## 2. TOMORROW'S AGENDA — **27 rulings** (`GR-1`…`GR-27` ⇄ DEC-339…DEC-365), ranked by (unblocking value ÷ decision cost)

> **FULL AGENDA INDEX — the 27 items are split across four sections of this file.** Read this index first so
> none is dropped: **§2 → GR-1…GR-17** (the developer's own findings) · **§6.4 → GR-18…GR-24** (the global
> sweep; GR-18 is the single highest-value structural item found) · **§7.3 → GR-25, GR-26** (GR-25 is the
> **P1 security** item) · **§8.4 → GR-27** (the push-blocking microbench gate).

Each item: one-sentence question · minimal current-syntax repro · options with **recommended first** ·
the why. Ready to paste into `AskUserQuestion` one at a time.

### GR-1 — P0 block-shadow fix *(DEC-339)* — **decide first, everything else is cosmetic next to a wrong-output bug**
**Q:** How should shadowing in nested blocks be made byte-identical?
**Repro:** `int a = 1; if (true) { int a = 2; } Output.printLine("out={a}");` → vm/tw `out=1`, php `out=2`.
- **(A) Alpha-rename shadowed locals in the transpiler (RECOMMENDED)** → emits `$a__b1` for the inner
  binding; `out=1` on all three legs. Language surface unchanged, zero runtime cost, standard technique for
  targeting a scope-less language, and the transpiler already tracks a `locals` scope stack.
- (B) Reject shadowing with a new `E-SHADOW-LOCAL` → sound and simple, but *removes* a capability the Rust
  backends implement correctly, and Rust/C#/Kotlin all permit shadowing.
- (C) Warn and keep diverging → **forbidden** by Invariant 14 ("silent semantic downgrade: FORBIDDEN").
**Either way:** add a differential example shadowing in every block form, so block scoping gains permanent
spine coverage. *Why A:* it fixes the bug without paying a language-surface tax for a compiler limitation.

### GR-2 — Database auto-rollback data loss *(DEC-340)* — **P1, silent data persistence**
**Q:** Fix `db.transaction(fn)` auto-rollback to unwind ALL depth, and add an explicit abort?
- **(A) Auto-rollback unwinds to depth 0 + add `rollbackAll()` (RECOMMENDED)** → a leaked inner `begin()`
  can no longer leave writes live; `rollbackAll()` is one SQL statement at any depth.
- (B) Fix auto-rollback only; no new API.
- (C) Also add `transactionDepth()`/`inTransaction()` observability (D9).
*Why A:* an error path that reports "rolled back" while persisting a row on the next commit is the worst
failure class in the module — data loss, silent, and reachable from ordinary code.

### GR-3 — TextMate grammar rewrite *(DEC-341)* — **highest visible win per unit of effort**
**Q:** Ship the verified 5-rule string section + a grammar regression gate?
**Repro:** any `.phg` with a plain string renders code as `string.quoted` and string bodies as
`entity.name.type` (light blue) — 81/383 files end inside an unterminated span.
- **(A) Full 5-rule section (raw/bytes/textblock/tagged/plain + corrected escapes + nested interp) +
  `vscode-textmate` pre-push gate (RECOMMENDED)** → leakage 81/383 → **0/383**; fixes B11–B18 together.
- (B) Minimal `\b`→lookbehind fix only → **regresses tagged templates; 29 files still leak** (measured).
- (C) Grammar fix without the gate → the grammar has **zero** automated coverage today, which is the
  structural reason this shipped at all.
*Why A:* both IDEs share this grammar, it has no byte-identity surface, and (B) is measurably insufficient.

### GR-4 — UFCS LSP completion + import gating *(DEC-342)*
**Q:** Add registry-driven UFCS member completion, and should it be gated on the module import?
**Repro:** `import Core.String; string line = "x"; line.` → 0 items. Meanwhile `String.` **without** the
import returns 45 items, while the runtime *requires* the import.
- **(A) Add `catalog::ufcs_members(recv_ty)` (first-param unification) AND import-gate both directions
  (RECOMMENDED)** → `line.` suggests String natives when imported; `String.` stops over-suggesting when not.
- (B) Add UFCS completion ungated → suggests members that won't compile.
- (C) Add UFCS completion + auto-import code action on accept → best UX, more work.
*Why A:* it makes completion agree with the checker, which is the only defensible contract; it also repairs
DEC-326's rationale, which currently rests on a capability that doesn't exist. **B7 and B1 must be ruled
together** — they are the same asymmetry seen from two sides.

### GR-5 — Loop forms: amend DEC-248 or execute it *(DEC-343)*
**Q:** Keep both `for…in` and `foreach…as`, or execute DEC-248's retirement of `for (T x in xs)`?
**Repro:** both run today; `E-RETIRED-FORIN` does not exist in `src/`; census 87 `for…in` vs 8 `foreach…as`.
- **(A) Amend DEC-248 to "keep both", close Conflict C-2, add cross-form migration hints (RECOMMENDED)** →
  matches what the corpus already teaches and what users have; retirement has now failed to get built twice.
- (B) Execute the retirement → must rewrite 87 example sites and re-teach the dominant form.
- (C) Retire `foreach…as` instead (the PHP-flavoured one) → smallest corpus churn (8 sites) but discards
  the deliberate PHP-familiarity affordance.
*Why A:* a ruling unbuilt for a month while the corpus votes 87:8 the other way is evidence the ruling, not
the corpus, is wrong. **This is a language-surface decision — entirely yours.**

### GR-6 — `main` de-reservation *(DEC-344)*
**Q:** Stop forcing the entry signature on functions named `main`?
**Repro:** a library `function main(string s): string` with **no** `#[Entry]` is rejected
(`type_bodies.rs:347`); `#[Entry] function startHere()` works fine, proving the attribute already frees the name.
- **(A) Remove the name-based special case; entry-ness comes only from `#[Entry]` (RECOMMENDED)** → also
  delete dead `E-MULTIPLE-MAIN` + its stale `phg explain` entry, and repurpose `class-main.phg` into a
  differential-gated regression test so the reservation can't silently return.
- (B) Keep the reservation as a guard-rail, but fix the error message to explain it.
*Why A:* DEC-331 already declared entries attribute-declared, "never by a magic name" — the checker simply
never caught up, and `phg explain` currently teaches a code that can never fire.

### GR-7 — Package validation fast path + loose-file hatch *(DEC-345)*
**Q:** Run the three validators before the fast-path return, and add a structure-free file marker?
**Repro:** `package Foo.Bar;` alone → OK; add one user import → `E-PKG-PATH`.
- **(A) Fix A6 first (pure bug: entry root is always `entry_local`, so a *correct* `src/App/Cmd/Runner.phg`
  + `package App.Cmd;` is rejected with a self-contradicting message), THEN run the validators on the fast
  path, THEN fix the `validated (every file…)` message + give the loose-`Main` error a code; hatch =
  `#[Loose] package Foo.Bar;` (RECOMMENDED)** → all surfaces funnel through `load_unified_src`, so
  `run`/`check`/`transpile`/`build`/`test`/**LSP** land together (Invariant 17 by construction).
- (B) Validators only, no hatch.
- (C) `phorj.json` opt-out → **not recommended**: the loader never reads it, and it contradicts DEC-282's
  "no manifest, no marker file".
*Why A ordering matters:* closing the fast path **before** fixing A6 would start emitting the wrong error
for correct layouts. `#![…]` is rejected because byte 0 is contested by the shipped shebang skip.
*Cross-language note:* **Go does not enforce package-name == directory-name** (only one package per
directory) — phorj's `folder = package` is *stricter than Go*, closer to Java; Kotlin and C# deliberately
decoupled; Rust is the closest analogue to your instinct (strict default + `#[path]` escape).

### GR-8 — UFCS migration execution *(DEC-346)* — DEC-326 follow-through
**Q:** How to execute the already-ruled UFCS promotion, and what happens to `Output.printLine`?
**Numbers:** 2223 qualified sites in examples (~6027 hand-edit total); 391 zero-judgement conversions
(List 135, String 122, Map 48, Set 28, Bytes 58); **1231 are `Output.printLine` = 55.4%**.
- **(A) Tooling first (GR-4 completion + import hint + formatter lint), then migrate the 391
  zero-judgement sites module-by-module, and rule `Output.printLine` BEFORE any codemod (RECOMMENDED)** →
  avoids touching 55% of the corpus twice.
- (B) Corpus-wide codemod now → churns everything before the discovery story works.
- (C) Keep "migrate as touched" → it has moved the needle 0% in three days.
**Sub-question to rule explicitly:** is `"text".print()` desirable, or does `Output.printLine(x)` stay
qualified as a deliberate exception? *Why A:* byte-identity is verified unaffected by call style, so this is
purely an idiom/tooling decision — and doing it in the wrong order doubles the work.

### GR-9 — Streaming file reads *(DEC-347)*
**Q:** Add `FileSystem.lines(path): Iterator<string>`?
**Repro:** `Input.lines()` streams (23.7 MB RSS on 88 MB), but every file API is whole-slurp — 200 MB for
`readAll`, and `limits.rs` has no I/O or memory cap.
- **(A) `FileSystem.lines(path)` over an offset-chunk native, no file handle (RECOMMENDED)** → zero new
  Value/type/transpile machinery, identical user syntax, O(1) memory, non-breaking later swap to a real
  handle. Ladder **case 1** (`fgets` maps).
- (B) Full `FileHandle` type → blocked by C4: no transpiling precedent for an opaque handle;
  `emit_type` would emit an unsatisfiable PHP class hint, and both sibling handles are `E-TRANSPILE-*` quarantined.
- (C) Defer (status quo) → the deferral predates the measurement showing slurp is a real memory risk.
*Why A:* it delivers the capability without inventing a handle abstraction the PHP leg can't express.

### GR-10 — Filesystem locking *(DEC-348)*
**Q:** Add advisory file locking, and with what shape?
- **(A) Scoped `withLock(path, fn)` + `tryWithLock` — whole-file, advisory (RECOMMENDED)** → release is
  guaranteed by construction (no leak path); ladder case 1 (Rust std locks and PHP `flock()` are literally
  the same OS lock, verified to block each other).
- (B) Manual `lock`/`unlock` → leak-prone; the pattern every language regrets.
- (C) Add byte-range or timeout → byte-range needs `fcntl` (blocked); timeout is unsupported by the
  primitive and would need a spin-sleep **bandaid**.
**Must be surfaced in the ruling:** Windows is a shipped target whose lock semantics may be *mandatory*
rather than advisory, and there is **no Windows CI** — so any documented cross-platform guarantee is
currently `[Unverified]`. Also needs a `try/finally` PHP helper to keep the release guarantee.

### GR-11 — Bless the no-op clone *(DEC-349)*
**Q:** Make `p with { }` the canonical no-modification clone and fix its debt?
- **(A) Bless + document the existing `p with { }`; add NO new syntax (RECOMMENDED)** → C# records
  (`with { }`) and Kotlin (`copy()`) both make the empty form canonical. Then fix the real debt: **lifter
  refuses `CloneWith` and PHP `clone`** (live Invariant-17 violation), no DEC row, no spec grammar, no
  example documenting the **shallow** boundary, formatter's double space.
- (B) Also add `clone x` or `.clone()` → creates two spellings for one operation, the dual-API mode DEC-257
  rejected.
⚠ **Lift constraint:** lifting PHP `clone` must **refuse loudly** when `__clone` is present — dropping it
silently would be ladder case 3 (forbidden). *Also:* the general `clone($p, [...])` mapping needs **PHP 8.5**
(verified: works on 8.5.8, parse error on 8.4.19); the *empty* case emits bare `clone($p)`, safe everywhere.

### GR-12 — Database naming *(DEC-350)*
**Q:** Rename the type `Database` → `Connection`, and drop the module's `Module` suffix?
- **(A) `Core.Database.Connection` — rename type AND unsuffix the module (RECOMMENDED)** → the object is
  provably one connection; DEC-278's suffix rationale dissolves once the namesake collision is gone.
- (B) Rename the type only, keep `Core.DatabaseModule`.
- (C) Keep `Database` → misnames a connection as a pool/manager, the exact confusion Go's `sql.DB` causes.
*Cost:* user-visible rename → examples, docs, lift, transpile, tests. No register row exists yet.

### GR-13 — Statement bind lifecycle *(DEC-351)*
**Q:** Should binds reset per execution, and should the two bind styles behave alike?
**Repro:** loop `bind` → iteration 2 fails `2 bound value(s) but 1 ? placeholder(s)`; same loop with
`bindNamed` silently last-wins and is **~75× slower** at 8000 iterations.
- **(A) Reset binds after each `exec`/`query`; make positional and named behave identically; fix the
  quadratic path (RECOMMENDED)** → makes your exact loop work, honours DEC-208's stated reuse promise.
  Cheap: the SQLite driver already uses `prepare_cached` and resets per execute — this is bind lifecycle,
  not a driver rewrite.
- (B) Keep append semantics, add explicit `reset()` → more ceremony, keeps the footgun as default.
- (C) Document the limitation only → leaves a 75× perf trap and a silent last-wins asymmetry.
Also open: **D5** nested-savepoint SQL isn't MySQL-portable (bare `RELEASE id`, and a `;`-joined pair
through single-statement `query_drop`) while the module's own `mysql.rs` uses the correct forms — with
**zero** nested-savepoint coverage on MySQL or Postgres.

### GR-14 — Block-body visibility: disambiguate *(DEC-352)*
**Q:** Which of these did you mean by "visibility/access in blocks inside a function"?
- **(A) Named local functions — `function helper(): int { … }` inside a body, WITHOUT access modifiers
  (RECOMMENDED reading)** → today a parse error; a real ergonomic gap (only lambdas-in-variables exist, which
  can't self-recurse cleanly). Lowers to a PHP closure variable → ladder case 1. Every peer language with
  local functions (Rust/C#/Kotlin/Swift/Python/JS) **forbids access modifiers on them**; C# rejects
  `private` explicitly.
- (B) Bare blocks that scope locals → **already exists** (and is GR-1's bug).
- (C) Access modifiers on locals (`private int a = 1;`) → recommend **NO**: a local is already narrower than
  `private` can express; no mainstream language offers it.
- (D) Local class/type declarations → recommend **NO for now**: low payoff, high transpiler cost (hoisting +
  mangled names + reflection tables).
- (E) Explicit closure capture lists → recommend **NO**: capture is already implicit-by-value and
  **verified byte-identical** on all three legs; mandatory capture lists would be pure ceremony.
*Principle worth recording either way:* **"visibility" is a top-level/member-axis concept; inside a function
body the axis is lifetime/scope, not access.** The visibility spec already caught this exact conflation once (G3).

### GR-15 — `#[Entry]` import ceremony *(DEC-353)*
**Q:** Should the compiler-injected `Entry`/`EntryKind` still require explicit imports?
**Repro:** a minimal runnable program is **6 lines, 4 of them ceremony** (vs PHP's 2); omitting either
import is a separate hard error, and the error text itself calls `Entry` *"an injected `Core.Runtime` type"*.
- **(A) Auto-provide the injected `Core.Runtime.{Entry,EntryKind}` symbols (RECOMMENDED)** → requiring an
  explicit import for a **compiler-injected** symbol is self-contradictory; removes 2 lines from every
  runnable file.
- (B) Allow one combined `import Core.Runtime;` to cover both.
- (C) Keep as-is — the ceremony is the price of DEC-337's explicitness.
*Caveat:* (A)/(B) interact with the `E-UNIMPORTED` / `E-INJECTED-VARIANT-BARE` machinery DEC-337 just built,
so this is a real design question, not a tweak.

### GR-16 — Claude bundle import *(DEC-354)*
**Q:** Approve the 14-item import from the global bundle (out of 199 files)?
Full per-file audit in `2026-07-25-global-review/J-claude-bundle.md` — **every one of the 199 files has an
explicit IN/OUT verdict with a reason**, per your "no silent omits" instruction. This re-opens the
2026-07-22 bulk ruling that dropped "the other ~43 machine-specific skills" and the permission lists.
- **(A) Approve the recommended package (RECOMMENDED):** 11 skills (`converge`, `sweep`,
  `expanding-context`, `sleuth`, `forge`, `inspect`, `aggregate-findings`, `qa-sweep`, `validate-infra`,
  `cross-check`, `recent`) + `precompact-handoff` hook + adapted `refs/SKILLS.md` + hand-filtered
  **deny/ask** permission tiers + a new `scripts/disk-reclaim.sh`.
- (B) Skills only, no hooks/permissions.
- (C) `/converge` alone → it is the single highest-value item: it *is* the DEC-268 ladder, currently
  hand-rolled from memory at every 3C/6C gate.
**Highest-value single fact:** `precompact-handoff.sh` addresses a pain that hit **twice in this session**.
**Hard OUT regardless:** all 57 `mcp/**` files — three corporate service `.env` files plus desktop-automation drivers, with zero relevance, and `phorj` is a **public** repo.
**Sub-decisions:** the 4 `ask-human`/gate **Stop hooks** are held back (the container already runs its own
`stop-hook-reply-gate.py` — double-gating risk); recommend instead rewording the framework's **false**
claim that the question guard is "mechanically" enforced here.

### GR-17 — `->` return-syntax retirement (W2-4) *(DEC-355)*
**Q:** Schedule W2-4 now that its cost is known?
**Numbers:** `->` still parses clean; **87** `.phg` + **2068** `.rs` fixture occurrences across **90** files.
**Key enabler nobody recorded: `phg format` ALREADY normalizes `->` → `:`** (verified), and pre-commit
already runs `.phg format --check`.
- **(A) Sequence: scripted `.rs` fixture rewrite → parser-reject → un-ignore dormant tests → add a grep
  gate blocking new `->` (RECOMMENDED)** → the `.phg` half is a formatter sweep; only the Rust string
  fixtures need scripting.
- (B) Defer again → this is its second failure to land.
⚠ **Must not be a naive sweep:** some `->` occurrences are **fn-type/prose** arrows in comments
(`(Request, next) -> Response`, `Test.assertFaults(() -> T)`), a *different* use than the return
annotation. Separate them first or the sweep corrupts documentation.

---

## 3. THE CROSS-CUTTING ROOT CAUSE (the most important structural finding)

**Ruled → partially built → docs never reconciled.** This single pattern explains a majority of tonight's
findings, independently reached by two agents:

| Ruling | Ruled | Built? | Symptom today |
|---|---|---|---|
| DEC-248 `for…in` retirement | ✅ | ❌ `E-RETIRED-FORIN` absent | developer's #7 confusion; C-2 open a month |
| DEC-326 UFCS promotion | ✅ | ½ (lifter only) | corpus still 92% module-form; rationale rests on absent completion |
| DEC-331 entry-by-attribute | ✅ | ½ (checker still reserves `main`) | developer's #14 |
| DEC-208 "Statement reuse" | ✅ | ❌ binds never reset | 75× perf trap |
| DEC-282 loose-file rules | ✅ | bypassed by fast path | developer's #1 |
| `E-MULTIPLE-MAIN` | — | dead code | `phg explain` teaches an unreachable code |

**Recommended systemic fix (cheap, high leverage):** a mechanical gate asserting that **every diagnostic
code named in a decision-register row exists in `src/`, or the row is marked PARTIAL**. That one check
would have auto-caught `E-RETIRED-FORIN` (ruled, absent) *and* `E-MULTIPLE-MAIN` (explained, never emitted),
and it generalises to the whole class. Proposed as part of GR-5/GR-6 rather than a separate slice.

**Second systemic gap:** the differential harness's coverage **is** the example corpus
(`examples/**/*.phg`). Any language feature without an example has **zero** spine coverage — which is
exactly how the GR-1 P0 survived. Worth stating as an explicit corollary to Invariant 9.

---

## 4. READY FOR AUTONOMOUS EXECUTION (no ruling needed)

Work that is unambiguous once `GR-1`…`GR-27` are ruled, or already needs no decision:

1. **Grammar fix + gate** (GR-3 A) — mechanical, verified, no byte-identity surface.
2. **Stale-label corrections** — e.g. a spec header saying "NOT BUILT" about a certified feature (E1);
   SLICE-STATE's *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim is measurably false for UFCS.
   *(Cited by quoted subject, not line number — this file's own edits have already drifted that anchor
   from `:1022` to `:1083`, which is exactly the doc-rot GR-24's third guard addresses.)*
3. **Diagnostic span fix** — some UFCS type errors anchor at `1:9` (`package Main;`) instead of the call
   site; the certification pass reproduced it and identified the trigger as **string interpolation**.
4. **Differential example for block scoping** — required by GR-1 under either option.

*(The `CLAUDE.md` dependency correction that used to head this list was **applied** in part 3 — see §6.5.)*

---

## 5. HONEST LIMITS OF THIS PASS

- **No fix was implemented and nothing was ruled** — by design (Invariant 15 + your "no questions
  tonight" instruction).
- **Two of this run's own inline conclusions were wrong** and are corrected in place in
  `K-inline-findings.md` (loop syntax; `var` usage). Both stemmed from over-reading a single probe — a
  zero-result grep from the wrong directory, and one failing spelling treated as proof its alternative was
  "the survivor". Kept visible rather than edited away.
- **Cross-language naming claims in the database report are `[Unverified]`** — both vendor doc fetches
  returned HTTP 403. The MySQL savepoint-grammar claim is `[Inferred-strong]`, resting on the module's own
  internal self-contradiction.
- **Windows lock semantics are `[Unverified]`** and cannot be verified without a Windows runner.
- The `--all-features` correctness gate was **not** re-run tonight (no source changed); disk headroom was
  ~6 GB, which is why heavy builds were avoided.

---

## 6. GLOBAL SWEEP RESULTS (beyond the developer's 15 findings)

Three additional sweeps ran: **G** Rust source quality/naming/structure/docs, **H** docs consistency +
Invariant-19 divergence, **I** missing enforcement + incompleteness + better-than-PHP gaps. Full detail in
`2026-07-25-global-review/{G-rust-quality,H-docs-consistency,I-gaps-enforcement}.md`.

### 6.1 — Honest verdict on code quality (verified, not assumed)

**The engineering core is genuinely high quality.** Independently checked and confirmed: **zero `unsafe`
outside `src/jit/`** (the documented island), zero `todo!`/`unimplemented!`, zero production `panic!`,
**26 production `unwrap()` in 154,817 lines across 566 files**, all three Invariant-3 exhaustive matches truly wildcard-free, zero
`checked_*` arithmetic outside `src/value/`, a clean Rust-API-guidelines sweep across all 566 files,
uniform `set -eEuo pipefail` in scripts, and the size gate honestly green **with the baseline untouched**.
The recent M-Decomp was real structural work, not accounting.

**The dominant defect class is documentation rot, not code.** A newcomer's first three documents
(ARCHITECTURE, INVARIANTS, ADR-0001) contained **6 verifiably false statements** between them, and ~50
comment pointers led nowhere. Estimated cost to clear: ~4 hours mechanical + one CI check.

**Second weakness — the discipline is asymmetric.** The `Op` set and the 305-code diagnostic registry are
*mechanically* enforced (`explain_coverage.rs`/`explain_ratchet.rs` is the best fan-out gate in the repo).
But the `Expr` set, the `Type` walk, the primitive-type table, and the cross-backend fault strings are
enforced only by **comments asserting agreement** — and three fault strings have **already drifted** while
the oracle cannot see it.

### 6.2 — NEW parity/correctness findings (add to the P0 in §0)

| ID | Sev | Finding |
|---|---|---|
| **I8** | **P1** | **A SECOND exception to Invariant 1**, which claims exactly one. A self-referential property hook diverges `run` vs `run --tree-walker`: **line 9 vs 17, 4099 vs 4 trace lines**. Hook-specific (plain recursion is byte-identical) and **invisible to `agree_err`** because that oracle matches on the fault *body* substring only. |
| **I19** | **P1** | **The only wrong-answer-with-no-error finding**: a lambda's write to a by-value-captured variable is **silently lost**. Needs a ruling (error vs. document). |
| **G27a/b** | **P1** | A canonical `FaultMsg` exists and two backends re-inline it (**Invariant 4** breach). `"non-exhaustive match at runtime"` has **already drifted** — PHP throws `UnhandledMatchError()` with *no message*. `transpile/call.rs:12-39` already does it correctly, so the right shape exists in-tree. |
| **G26** | **P1** | `tests/differential.rs::classify` **re-types all 12 canonical fault bodies as its own literals**; anything unclassified falls to `Other(…)` including the VM's `"at N:"` prefix, so it **can never be asserted equal**. Fault-string drift is therefore *invisible*, not merely untested. |
| **I1 / I7** | **P1** | **Invariant-10 (determinism) breaches:** `phg disassemble` is unstable across runs (≥5 distinct outputs; 5–6 observed per 20-run batch) (`CallOverload` set ids from HashMap order); the flagship `did you mean` hint gives **3 different answers across 20 runs** (`nearest_name`'s `min_by_key` tie-break follows HashMap iteration order — `src/checker/plumbing.rs:160-167`, confirmed by reading it). Program output and transpiled PHP are **verified stable**, so the spine itself is intact. |
| **H2** | **P1** | `INVARIANTS.md:74` says "**never** SIGABRT/panic" — reproduced **exit 134, stack overflow**. The stated 256 MB-worker mechanism doesn't cover that path, and unlike §7 there is no disclosed carve-out. |
| **H9** | **P1** | **Invariant 17 is currently unsatisfiable**: `p with { y = 9 }` runs, transpiles to `clone($a,[…])`, and `phg lift` on *the transpiler's own output* fails. Lift has no `E-TRANSPILE-*`-style escape hatch to legitimise a gap. |

### 6.3 — Previously-known findings now CONFIRMED FIXED (close these records)

Verified fixed, so the open-item lists that still name them are stale: **P0 private/protected static-field
visibility** (now `E-FIELD-VISIBILITY`) · **P1 static-method-via-instance — the `G5` the visibility spec
still lists as open** (now `E-STATIC-VIA-INSTANCE`; the whole static/instance matrix is closed) · **P1
package-decl casing dead on CLI paths** (`E-PKG-CASE` fires) · **P2 `E-ALIAS-CYCLE` uncoded + unused cycle
passes** (both halves) · **P3 `E-OVERLOAD-SELECT-CONFLICT`** (entry removed) · and **all 9** findings of the
earlier same-day plans-divergence audit. Separately, `DV-4`'s G4 was already verified fixed. **Recommend
closing `G5` in `docs/specs/2026-07-24-visibility-model.md` and dropping the fixed rows from
`KNOWN_ISSUES.md`.**

### 6.4 — Seven MORE rulings (GR-18 … GR-24), same Invariant-15 shape

- **GR-18 (DEC-356) — Extend mechanical exhaustiveness from `Op` to `Expr`/`Stmt`/`Pattern`.**
  37 `Expr` variants, 13 hand-rolled total rewriters in `src/checker/`, and **17 named catch-alls**
  (`leaf => leaf`, `other => other`) that compile cleanly and silently pass a new variant through.
  `desugar_db.rs:67-69` literally *declares* "keep the rewriter TOTAL — a new expression-bearing AST node →
  add its arm here" and then closes with `leaf => leaf`. Also `src/ast/walk.rs:748` has `_ => {}` in
  `collect_pattern_bindings`, three lines under a comment recording that this exact bug **already fired
  twice** and that under-reporting a capture "is a correctness bug".
  Options: **(D) fix the known catch-alls now (one file, today) — RECOMMENDED first step** · (C) a
  dummy-variant CI check · (B) one shared total visitor (a real slice). *Why:* this is the single
  highest-value structural improvement found, and the project has already proven it can build this class of
  gate.
- **GR-19 (DEC-357) — Lambda capture-write (I19).** Silently lost today. **Recommended: reject with a new
  diagnostic** (a silent wrong answer is the worst outcome; Invariant 14 forbids silent downgrades).
  Alternative: support by-reference capture — but that re-opens a PHP-parity question (`use (&$x)`).
- **GR-20 (DEC-358) — Uncoded-diagnostic ratchet (I14).** Type mismatch, arity, unknown method,
  non-exhaustive match, **every** parse/lex error and **every** runtime fault carry `code == None`, so
  `phg explain` is unreachable for them — and all 9 `conformance/diagnostics/` cases assert a code, so the
  corpus is blind to it. **Recommended: a `code == None` ratchet with a shrinking allowlist**, mirroring
  the existing `explain_ratchet`. Makes the backlog CI-visible instead of invisible.
- **GR-21 (DEC-359) — Compile-time rejection of impossible literals (I17).** `10/0`, literal overflow, and
  literal index-OOB all pass `check` today — PHP parity where a **win** is available. Recommended: reject at
  check time (a clear better-than-PHP gain); needs a ruling because it is a surface change.
- **GR-22 (DEC-360) — A `W-UNUSED-*` warning tier (I20).** Today an unused *import* is a **hard error**
  while an unused *local* is **silent** — an inconsistency in both directions. Recommended: introduce a
  warning tier and move unused-import into it (Go's hard-error choice is its most complained-about
  feature). **This is a language-UX decision — yours.**
- **GR-23 (DEC-361) — Fault-string single-sourcing + a drift-visible oracle (G26/G27).** Recommended: make
  the backends consume the canonical `FaultMsg` (Invariant 4) **and** make `classify` derive from those
  same consts rather than re-typing them, so drift fails a test instead of silently degrading.
- **GR-24 (DEC-362) — Three mechanical doc guards (H).** Recommended: (1) a markdown reference-checker in
  `pre-push` (would close G4/G16/G18/H48 — **60+ dangling `src/` refs** — permanently); (2) one-register-row-per-DEC
  check (**13 DEC ids have no register row**; DEC-190's ruling exists on exactly one line repo-wide);
  (3) cursors must record `origin/master` + subject, never a bare SHA.

### 6.5 — Additional no-ruling items (extends §4)

**Already fixed tonight in this pass** (committed): `INVARIANTS.md` §1 corruption + inversion · dangling
module paths in `INVARIANTS.md` and `CLAUDE.md` Invariants 3/4 · the `CLAUDE.md` dependency understatement
(4 → the real 14) · the orphaned push SHAs in both cursors.

**Still outstanding, no decision required:**
1. **H1 (P0 for first impressions) — the README's hero snippet and BOTH quickstart commands do not run**
   (`no entry point: … #[Entry(kind: EntryKind.Cli)]`, exit 1). The very first thing a newcomer types fails.
2. **I7 — ~1 line**: give `nearest_name`'s `min_by_key` a lexicographic tie-break. Removes user-visible
   non-determinism from the flagship diagnostic surface.
3. **I4 — 1-line swap**: `src/vm/coop.rs:123` → `parse_checked_program_reified` + `compile_with`
   (**Invariant 6** breach; the doc comment there falsely claims it mirrors `cmd_run`), then delete the old
   entry point so regression is impossible.
4. **I2 / I3 — wrong diagnostics**: a nonexistent import is reported as `E-UNUSED-IMPORT` with the
   unachievable advice "…or use it"; the reserved `Core.` root is still unenforced *and* now produces two
   wrong errors, while `phg explain E-RESERVED-PACKAGE` teaches a lowercase root, the nonexistent
   `Core.Console`, and an `E-PKG-CASE`-illegal remedy.
5. **I15 — `phg check --json` emits plain text** for parse/lex/runtime errors, so a consumer gets a
   `JSONDecodeError`. Breaks the interface `FEATURES.md:85` documents.
6. **Stale status records**: `GA-CHECKLIST` scores rock 3 at **15%** citing three things that all shipped
   (conformance corpus = 64 `.phg`; SEMVER/STABILITY/DEPRECATION.md) — **17 points of headroom on dead
   premises**; MASTER-PLAN lists all six **shipped** security DECs as "do first", un-✅'d; ADR-0005 is still
   `Accepted` describing the retired `phg vendor`, with no ADR-0006, breaking the ADR README's own
   supersession rule; `KNOWN_ISSUES:675` still warns against a naming pattern the compiler already guards
   (flagged 9 days ago by a prior audit and still there).
7. **Invariant text vs reality**: Invariant 10 names the retired `phg vendor`; Invariant 14 names
   `--sequential-concurrency` with **zero hits in `src/`**; `phg install/add/update/remove` ship but are
   absent from `--help`.
8. **G3 — ~500 lines of phorj live as Rust string literals** in `cli/preludes.rs`, and the fmt sweep globs
   only `examples/` + `selftest/` — so the **stdlib's own public surface is the only phorj never
   format-checked or `phg check`-ed**.
9. **G5 — ARCHITECTURE.md omits `src/jit/` (21k lines, the 4th backend), `lsp/`, `lift/`, `pm/`, `bundle/`,
   `format/`** — **31% of the codebase absent from the "one-page map"**, which also contradicts
   `jit/mod.rs:63` on the backend count.
10. **G31 — the primitive type-test table is written 5× across 4 modules**, held together by 5
    cross-referencing comments. Adding `decimal` would diverge VM from interpreter **with no compile error**.
11. **G19 — false module docs**: `green/mod.rs:13` says the executor hasn't shipped, two lines above
    `pub mod exec;`; `serve/mod.rs:14-16` says green threads were superseded and need `unsafe` (both false).
12. **I13 / I12 / I16 — coverage gaps**: shebang (DEC-336) shipped with **no example**; 11 README rows point
    at a nonexistent `examples/db/`; `Core.UriModule` is 15/36 documented; FEATURES.md omits shipped
    named-args, variadics and `#[Invoke]`; spread is **undisclosed** though both W4-1 siblings shipped.

### 6.6 — Why no code was changed tonight

Only **documentation** was edited. The trivial fixes above (I7 ~1 line, I4 1 line) were deliberately left
un-applied: they touch `src/`, which requires the full `--all-features` correctness gate to re-run, and the
night's mandate was to *prepare* decisions, not to sequence code work. Disk headroom was ~6 GB, which is
also why heavy builds were avoided. **They are ready to apply on your word.**

---

## 7. THE ON-HOLD INVENTORY — everything awaiting you, deduplicated

Full table in `2026-07-25-global-review/L-onhold-inventory.md` (95 rows, each with citations, a reality
check, and a recommendation). This answers the "**all the specs we put on hold**" part of the ask.

**Counts:** **95** deduplicated items — ~46 need a ruling (the 24 above + ~22 smaller) · **~30 ruled but
not built** · 17 deferred-with-a-reason (**3 rationales are now obsolete**) · ~22 known limitations ·
and **40 stale labels**.

### 7.1 — The headline: 40 stale status labels

This is the single largest waste surface found, and it directly wastes *your* decision time.

**26 items are recorded as OPEN but are actually BUILT** — so any agenda built from the current docs would
have you re-deciding settled work: named args for ctors+methods · **tuples (DEC-288)** · backed enums ·
`lift_from` (DEC-312) · **P-Q-A-5 file-size debt** (the size gate now reports `fails=0`) · CRAFT-2 ·
DEC-255 helpers · DEC-223 Mail · DEC-238 debug twin · DEC-216 · `db.transaction` closure · the retry
surface · empty-`[]` PART-2 · shebang `--lang` · DEC-336 · LSP find-usages · 2 of 3 lift-catch-up items ·
the naming mega-slice · DEC-313 FS · DEC-257 `Iterator` · Deque/PriorityQueue · HOF `List.map/filter/reduce`
· `E-TRANSPILE-FS` (retired) · Log-v2 processors · Totality's discharged gate · 8 of 9 plans-divergence findings.

**14 are recorded as DONE but are NOT** — the more dangerous direction: the wildcard spec header says
"NOT YET BUILT" beside its own "✅ DONE" · **DEC-331 D2/D3/D5/D6/D7 marked "LOCKED" but unbuilt** ·
DEC-247 DateTime never built · "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE" (false for UFCS) ·
`E-MULTIPLE-MAIN` explained but never emitted · `E-RETIRED-FORIN` absent · W2-4 · CRAFT-1's 90→66 ·
a citation pointing at the deleted `explain.rs` · a KNOWN_ISSUES 🔴 P0 heading for a fixed issue ·
CLAUDE.md's dependency count · UNIFIED-SPEC "JIT not wired" · ADR-0005 naming the retired `phg vendor`
with no ADR-0006. *(Two of these — the CLAUDE.md count and the KNOWN_ISSUES heading — were fixed tonight.)*

**Recommendation:** before working the agenda, spend one pass flipping these 40 labels. It is mechanical,
needs no rulings, and every subsequent decision you make gets more trustworthy inputs. This is also
exactly what DEC-362/GR-24's one-row-per-DEC guard would prevent recurring.

### 7.2 — Ready to build, no ruling needed (your autonomous hand-back batches)

Ranked by value ÷ effort, and deliberately precise about what is genuinely unambiguous:

1. **L-82 — `Core.Validation` trailing-`\n` divergence: reproduced, ONE-LINE `/D` fix.** The cheapest real
   bug in the whole sweep.
2. **L-74 — an 8-item diagnostic-quality cluster.** Best overall value/effort ratio found.
3. **GR-3 / DEC-341 — the grammar rewrite** (pre-verified: 81/383 files leaking → 0/383).
4. **L-41 — resume task #33** (Json-ADT JIT) at the write-path helpers.
5. **L-37…L-40 — the S3.2–S3.5 chain.** Note `ServeConfig` currently exists only as a *comment*, and
   `respond` is still live.
6. **L-53** stdlib companions · **L-43** labeled break/continue · **L-44** typed LSB · **L-36**
   `Core.DateTime` (its dependency ruling is already done) · **L-42** spread legs (a)+(b) only.

### 7.3 — Two more rulings (GR-25, GR-26)

- **GR-25 (DEC-363) — Response-side CRLF guard — `P1` SECURITY, treat as a top-10 item, not a small one.**
  **Verified mechanism, end-to-end:** `src/cli/http_prelude.rs:71-73` `Response.withHeader(name, value)` and
  `:74-77` `withCookie(c)` interpolate straight into a header line (`"{name}: {value}"`, `"Set-Cookie: {line}"`) with
  **zero validation**, `serialize()` (`:91-99`) CRLF-joins them into the response head, and `src/serve/handlers.rs:189`
  `respond_once` returns the handler's bytes **verbatim** — so there is no Rust-side serializer that could
  re-validate. That is textbook **HTTP response splitting**, reachable from ordinary handler code on a
  shipped `phg serve`, and `withCookie` commonly carries user-derived values.
  **Reproduced live by the certification pass** (`phg run`): `Response.text(200,"ok").withHeader("X-User",
  "x\r\nX-Injected: yes\r\n\r\n<html>pwned</html>")` serialises to a head that terminates early and injects
  both an extra header **and a second body** — no error, no validation.
  **The fix is already in-tree on the sibling path:** the *request* side rejects this at the gate
  (`src/ext/http_client/natives.rs:116` — *"header `{n}` contains a forbidden character"*, pinned by
  `src/ext/http_client/tests.rs:450 header_injection_is_rejected_at_the_gate`, which feeds `"a\r\nHost: evil"`).
  Recommended: copy that guard to the response side. *(Initially filed as "small" and ranked 25th — corrected
  after the certification pass re-derived the mechanism; the request/response asymmetry is the aggravating fact.)
- **GR-26 (DEC-364) — `using` / `defer` scope-guard surface.** Flagged because **every** open slice in
  this review — DB transactions, file locking, streaming handles — keeps bumping into the same missing
  primitive. DEC-203 already ruled `using` + `Closable`; this is about finishing it. Recommended: treat it
  as an enabler and sequence it *before* GR-9/GR-10, so those land on top of a real guarantee rather than
  hand-rolled `try/finally`.

### 7.4 — Three deferrals whose stated reason no longer holds

Flagged for re-examination rather than re-deferral (detail in the L report): notably **file-locking's
dependency blocker** (std file locks are stable on the pinned toolchain — the policy clause was never met)
and the **slurp-only file APIs** (deferred before the measurement showing whole-file reads cost 200 MB).

### 7.5 — Honest limits of the inventory

~14 KNOWN_ISSUES "deferred refinements" sections are catalogued at *section*, not bullet, granularity
(~60–100 sub-bullets not lifted); `full-audit/raw/*` beyond `C-decisions.md` was not swept; and **the
pinned dev-box microbench is still owed and only you can run it** — it decides whether the perf-flip
campaign has 3 losses left or 1.

---

## 8. BLOCKER FOUND WHILE PUSHING: the pre-push microbench gate cannot pass in a remote container

Discovered 2026-07-26 while pushing this **docs-only** series. Recorded because it blocks *every* push from
a container and because it casts doubt on a recent perf claim.

### 8.1 — What happened [all Verified]

`git push` failed the `pre-push` hook at `microbench-gate` (the G-8 mandate ratchet). Every other gate leg
passed: full test suite, clippy, fmt, `cargo build --release`. The blocking row:

```
FAIL floatloop: WIN->LOSS flip — baseline ratio 1.011 (WIN) now 0.803 (< 0.95 band): a G-8 mandate regression
microbench-gate: 41 WIN / 8 loss vs release-php+JIT; 1 blocking regression(s)
```

**This cannot be caused by the series being pushed:** `git diff --stat origin/master..HEAD -- src/ tests/
Cargo.toml` is **empty** — the five commits change only `.md` files.

### 8.2 — Root cause: the harness's CPU pinning silently did not apply

The harness emits, on this box:

```
WARNING: Your kernel does not support cpuset or the cgroup is not mounted. Cpuset discarded.
```

`microbench-gate.sh` compares **absolute** native-VM-vs-docker-php ratios, and its own header documents
that this is the load-*sensitive* gate ("empirically 3-4x swings at load average ~7, with NO code change";
"the pinned core is not isolated via `isolcpus`"). With the cpuset **discarded**, the pinning the harness
relies on is gone, so absolute ratios are not comparable to a baseline captured on the dev box.

**Corroborating evidence that this is measurement bias, not a regression:** the *entire* near-parity cluster
drifted **down together** in the same run — `dbwork` 1.004→0.960, `floatmul` 1.002→0.980, `mapget`
1.152→0.996, `setcontains` 1.129→0.954 (all reported as "near-parity wobble … within 0.95 noise band; not
blocking"). `floatloop`'s baseline is **1.011** — a hair over 1.0 — so it is simply the cluster member whose
margin was thinnest, and it tipped past the 0.95 threshold while its neighbours stopped just inside it.
A genuine code regression would not move five unrelated micros in lockstep.

### 8.3 — A perf claim that this harness does NOT corroborate

The same run reports:

```
note queryparse: not in baseline (new) — ratio=0.146 (loss); run --emit to snapshot it
```

**DEC-338** recorded queryparse at **~0.88× ("NEAR-PARITY, NOT yet a WIN")** from an *in-container,
direction-only* measurement, and explicitly deferred the canonical figure to "the dev-box docker microbench
harness". This **is** a docker microbench, and it reads **0.146** — i.e. still a ~7× loss, a ~6× discrepancy
far outside any noise band. Two readings are possible and I cannot separate them here: (a) the harness micro
and DEC-338's ad-hoc program are not the same workload, or (b) the 0.88× reading was optimistic.
**Either way, DEC-338's near-parity claim is NOT reproduced by the canonical harness, and the WIN remains
un-certified.** The register's §5 already flags the dev-box microbench as owed; this sharpens it from
"owed" to "owed and currently contradicted". [Verified: both numbers read from the harness output;
the *interpretation* is [Inferred] because the two workloads were not diffed.]

### 8.4 — GR-27 (DEC-365): what to do about the gate

**Q:** How should the microbench gate behave where CPU pinning is unavailable?
- **(A) Detect the discarded cpuset and SKIP-LOUD, exactly as the gate already does for absent docker
  (RECOMMENDED)** → the gate already has the right precedent: *"docker absent — SKIP the G-8 mandate gate
  (infra, not a regression)"*. A discarded cpuset is the same class of fact: infra, not a regression.
  Cheap, honest, and keeps the gate meaningful where it *can* measure.
- (B) Skip the microbench gate for commits that touch no `src/`/`tests/`/`Cargo.toml` → also correct, and
  complementary to (A), but narrower: it wouldn't help a real code change made from a container.
- (C) Raise the noise band below 1.05 → **not recommended**: it would mask genuine thin-margin regressions,
  which is the opposite of the G-8 mandate's purpose.
- (D) Re-baseline via `--emit` → **explicitly rejected.** That accepts a suspect measurement as truth and is
  the same "just bump the baseline" move the developer forbade for the size gate ("don't cheat"). Not done.

**What I did NOT do:** I did not push with `--no-verify` (classifier-blocked, and the project rule is to
present the command rather than bypass), did not re-baseline, and did not hide the docker binary to trigger
the existing skip path. The five commits are therefore **committed locally and NOT pushed**; the exact push
command is in the handover.

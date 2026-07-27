# RULED BUILD ORDER — the 2026-07-26 agenda, fully ruled

> **Status:** every item below is **RULED and NOT YET BUILT.** This file is the single ordering of the
> ruled set (Invariant 19). Each row's canonical rule lives in its own spec or its register row; nothing
> is re-explained here. Identity + status: `docs/research/full-audit/raw/C-decisions.md`.
>
> **Provenance:** the developer ruled all 27 agenda items (`GR-1`…`GR-27` ⇄ DEC-339…365) plus 13 more that
> the ruling session's own probing surfaced (DEC-366…378), in one interactive pass on 2026-07-26.

## How the order was chosen

1. **Wrong output first.** A program that silently computes the wrong answer outranks everything.
2. **Enablers before dependents.** `using` before locking/streaming; tooling before migrations; explicit
   arms before the shared visitor.
3. **Cheap gates early.** A guard that prevents a whole defect class is worth more than any single fix,
   and the docs-only fast path pays for itself immediately.
4. **Renames before users exist.** Breaking renames are cheap now and expensive later.

## Wave 0 — unblock the workflow (do first, hours not days)

| # | Item | Why first |
|---|---|---|
| 0.1 | **DEC-378** docs-only `pre-commit` fast path + no-concurrent-commits rule | Every later commit pays the ~4 min tier otherwise; this session lost ~45 min to it and its only test failure to the race |
| 0.2 | **DEC-365** microbench gate: SKIP-LOUD on a discarded cpuset, meaning *"unmeasurable, verdict OWED"* — never *"passed"* | Pushes are blocked from the container without it. Carries the **no-hidden-loss** standing rule |
| 0.3 | **DEC-362** three `pre-push` doc guards, incl. *every diagnostic code named in a decision row must exist in `src/`* | Would have caught three separate phantoms found this session |
| 0.4 | Flip the **40 stale status labels** (task #43) | Zero-ruling cleanup; stops future sessions acting on false state |

## Wave 1 — correctness (the reason the agenda existed)

| # | Item | Notes |
|---|---|---|
| 1.1 | **DEC-339** reject redeclaration of a live local/param binding (+ **DEC-366** lifter hoist, same slice) | The P0. 10 divergent shapes, one of which changes iteration count. Measure the `examples/`+`tests/` migration cost and report **before** migrating |
| 1.2 | **DEC-340** transaction auto-rollback unwinds to the **entry depth** + `rollbackAll()` + `transactionDepth()` + the PHP savepoint helper | P1 silent data loss, reproduced live |
| 1.3 | **DEC-363** response-header CRLF/NUL guard in the prelude + NUL on the request side + `isValidHeaderName`/`…Value` | P1 security on a shipped `phg serve`, reproduced live |
| 1.4 | **DEC-367** extend the builtin-collision guard to final methods of the mapped PHP parent | Invariant-1 breach: PHP fatal where both Rust legs run |
| 1.5 | **DEC-361** single-source the fault strings **and** make `differential.rs::classify` derive from them | The test that should catch drift is what hides it |
| 1.6 | **DEC-351** reset `Statement` binds per execution, unify positional/named, fix the quadratic path, portable savepoint SQL + MySQL/Postgres coverage | Broken reuse + ~75× |

## Wave 2 — structural gates

| # | Item | Notes |
|---|---|---|
| 2.1 | **DEC-356** fix all 18 catch-all sites **and** land the probe-variant gate, one slice; widen Invariant 3 to `Expr`/`Stmt`/`Pattern` | The class, not the instances. `B` (shared total visitor) stays a separate later ruling |
| 2.2 | **DEC-377** audit and classify all **168** `__phorj_*` helpers into the 3 buckets; inline the convenience-only ones | Nobody currently knows which bucket each is in |

## Wave 3 — the enabler, then what needs it

| # | Item |
|---|---|
| 3.1 | **DEC-364** build `using` (`defer` stays rejected on its real merits) |
| 3.2 | **DEC-347** `FileSystem.lines(path): Iterator<string>` over an offset-chunk native |
| 3.3 | **DEC-348** scoped `withLock`/`tryWithLock` — **Windows semantics `[Unverified]`, no Windows CI, must be disclosed** |

## Wave 4 — language surface

| # | Item |
|---|---|
| 4.1 | **DEC-357** reject capture-writes + **DEC-368** prelude `Mutable<T>` with `.value` (no `__phorj_` helper) |
| 4.2 | **DEC-373** `lift` reads `&$param` · **DEC-374** `declare function` by-ref out-params (`preg_match`) |
| 4.3 | **DEC-344** de-reserve `main` · **DEC-353** auto-provide injected `Entry`/`EntryKind` · **DEC-372** top-level statements stay rejected |
| 4.4 | **DEC-345** package-validator fast path — **A6 first**, then validators, then the message; hatch `#[Core.Runtime.FreePath]` |
| 4.5 | **DEC-352** local functions (capture by value) + local classes (**non-capturing**); visibility on either permanently rejected with an explaining diagnostic |
| 4.6 | **DEC-359** reject `10/0`, literal overflow, literal index-OOB (only when statically provable) |
| 4.7 | **DEC-360** `W-UNUSED-*` family + move unused-import into it; **`--strict` promotes warnings, `run`/`check` never fail on them** |
| 4.8 | **DEC-343** amend DEC-248 — keep **both** loop forms, close Conflict C-2 |
| 4.9 | **DEC-355** retire the `->` **return-type** spelling (the `=>` lambda arrow is untouched) |
| 4.10 | **DEC-349** bless `p with { }`; `lift` refuses loudly only when `__clone` exists |
| 4.11 | **DEC-376** foreign PHP file-return interop (PHP-target-only, `E-FOREIGN-RUNTIME`) |

## Wave 5 — editors and migrations

| # | Item |
|---|---|
| 5.1 | **DEC-342** UFCS receiver completion + import-gating both ways + the "exists in `Core.X` — add the import" diagnostic **with a quick-fix** + call-site spans + the ambiguity error. Measured against **DEC-375** (the LSP is the expert companion) |
| 5.2 | **DEC-341** the pre-verified 5-rule TextMate string section (81/383 → 0/383) **plus** the `vscode-textmate` pre-push gate |
| 5.3 | **DEC-346** migrate the 391 zero-judgement UFCS sites — **`Output.printLine` stays qualified** |
| 5.4 | **DEC-350** rename to `Core.Database.Connection`, drop the `Module` suffix |
| 5.5 | **DEC-354** the narrowed Claude bundle: 7 skills, **allow-list-only** permissions, `precompact-handoff` only, no session-remember, no MCP |
| 5.6 | **DEC-369** vocabulary sweep: "cooperative tasks"; `uses_concurrency` → `uses_tasks`; delete Invariant 14's phantom flag; **"concurrent"/"parallel" reserved** |
| 5.7 | **DEC-371** strike PHP-absence from the four contaminated rationales; mark DEC-037 superseded; re-open `defer` inside DEC-364; add the standing rule beside Invariant 16 |

## Wave 6 — the big one

| # | Item |
|---|---|
| 6.1 | **DEC-370** real parallelism: **isolated tasks + copying channels** as the target, **data-parallel stdlib combinators** as the first shippable slice, `E-TRANSPILE-PARALLEL-NO-PHP` per DEC-133's precedent. **Owed measurements first:** copy-at-boundary cost, and per-thread instantiability of interpreter/VM state |

## Owed measurements (none of these are optional)

1. **DEC-339** — how many `examples/`+`tests/` sites the redeclaration rule breaks. Needs the diagnostic to exist; report before migrating.
2. **DEC-357** — whether anything in-tree writes to a capture. Any hit is a **bug found**, not migration burden.
3. **DEC-365** — the two **owed verdicts**: `floatloop` (WIN→LOSS on a discarded-cpuset run) and `queryparse` (0.146 here vs DEC-338's ~0.88×, so **DEC-338's near-parity claim stays un-certified**). Both need a dev-box run.
4. **DEC-370** — copy-at-boundary cost + per-thread runtime instantiability.
5. **DEC-377** — the 168-helper classification.

# RULED BUILD ORDER — the 2026-07-26 agenda, fully ruled

> **Status:** every item below is **RULED and NOT YET BUILT.** This file is the single ordering of the
> ruled set (Invariant 19). Each row's canonical rule lives in its own spec or its register row; nothing
> is re-explained here. Identity + status: `docs/research/full-audit/raw/C-decisions.md`.
>
> **Provenance:** the developer ruled all 27 agenda items (`GR-1`…`GR-27` ⇄ DEC-339…365) plus 21 more that
> the ruling session's own probing and the on-hold tail surfaced (DEC-366…386), in one interactive pass on
> 2026-07-26; **DEC-387** was ruled during the 2026-07-27 build. Waves 0-6 cover DEC-339…378;
> **Wave 7 covers DEC-379…386**; Wave 8 is the two research slices. **Wave 5.5 (DEC-354 + DEC-387) is
> the only row BUILT so far** — built out of order at the developer's request; everything else is unbuilt.

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
| 0.1 | ~~**DEC-378** docs-only `pre-commit` fast path + no-concurrent-commits rule~~ — **BUILT 2026-07-29** (fast path routes on staged paths; the rule is now an enforced `flock`, not a remembered convention) | Every later commit pays the ~4 min tier otherwise; this session lost ~45 min to it and its only test failure to the race |
| 0.2 | ~~**DEC-365** microbench gate SKIP-LOUD — was already built for *load* and *missing binary*; **2026-07-29 fixed a real hole: it probed the docker BINARY, not the DAEMON, so an unreachable daemon returned setup-error 2 and ABORTED the push** (the cause of every `--no-verify` that session). Remaining: the discarded-cpuset case~~ — **BUILT 2026-07-29** | Pushes are blocked from the container without it. Carries the **no-hidden-loss** standing rule |
| 0.3 | ~~**DEC-362** three `pre-push` doc guards, incl. *every diagnostic code named in a decision row must exist in `src/`*~~ — **BUILT 2026-07-29** as `scripts/doc-guards.sh` (G1 paths / G2 DEC rows / G3 bare SHAs / G4 diagnostic codes; G2 hard, the rest ratcheted against a 142-entry baseline). Found and fixed 3 DEC ids with no register row on its first run | Would have caught three separate phantoms found this session |
| 0.4 | ~~Flip the **40 stale status labels** (task #43)~~ — **BUILT 2026-07-30** (task #43; and this very row was one of the stale labels it missed — see the note below) | Zero-ruling cleanup; stops future sessions acting on false state |
| 0.5 | ~~**Q28 / DEC-414** re-port the P6 git-argument hardening to `src/pm/fetch.rs`~~ — **BUILT 2026-07-29**: `ext::`/`file::` helper rejection (case-insensitive), leading-dash + empty rejection, `--` on clone, `-c protocol.ext.allow=never` everywhere, `GIT_*` env scrubbed; 6 tests each verified to fail with the guard neutered. `KNOWN_ISSUES` 4b closed | Was the only LIVE security regression on the audit tail |

## Wave 1 — correctness (the reason the agenda existed)

| # | Item | Notes |
|---|---|---|
| 1.1 | ~~**DEC-339** reject redeclaration of a live local/param binding (+ **DEC-366** lifter hoist, same slice — ratified by **DEC-397**)~~ — **BUILT 2026-07-29** | The P0. 10 divergent shapes, one of which changes iteration count. **Migration cost MEASURED 2026-07-29 (DEC-412): exactly ONE in-tree site** — `examples/guide/math.phg:54` re-declares `l1` (`int` at :46, `float` at :54 — same scope, different type = case 11). One rename; nothing else in 270 `.phg` files. Also lands **DEC-396**'s matrix additions (3 ACCEPTED rows, the lambda-own-param hygiene rejection, `using`/local-fn scope forms) and **DEC-404**'s captured-name-is-live rule, and the **DEC-410** `enum extends` diagnostic |
| 1.2 | ~~**DEC-340** transaction auto-rollback unwinds to the **entry depth** + `rollbackAll()` + `transactionDepth()` + the PHP savepoint helper~~ — **BUILT 2026-07-29** | P1 silent data loss, reproduced live |
| 1.3 | ~~**DEC-363** response-header CRLF/NUL guard in the prelude + NUL on the request side + `isValidHeaderName`/`…Value`~~ — **BUILT 2026-07-30** | P1 security on a shipped `phg serve`, reproduced live |
| 1.4 | ~~**DEC-367** extend the builtin-collision guard to final methods of the mapped PHP parent~~ — **BUILT 2026-07-29** | Invariant-1 breach: PHP fatal where both Rust legs run |
| 1.5 | ~~**DEC-361** single-source the fault strings **and** make `differential.rs::classify` derive from them~~ — **BUILT 2026-07-30**: `src/value/faults.rs` + two ratchets (no-literal-outside-its-definition, and every const must be classified); **38 re-inlined sites** converted, incl. a second `pub const` in the JIT whose comment admitted the body wasn't single-sourced; the predicted PHP-leg match drift found in **TWO** lowerings (empty `\UnhandledMatchError` + PHP's native "Unhandled match case") and fixed in both | The test that should catch drift is what hides it |
| 1.6 | ~~**DEC-351** reset `Statement` binds per execution, unify positional/named, fix the quadratic path, portable savepoint SQL + MySQL/Postgres coverage~~ — **BUILT 2026-07-30**: binds execution-scoped (`take_binds`, reset BEFORE the driver call at all four sites); **8000 named binds 4.469s → 0.054s measured**, at the report's own re-prepare baseline of 0.059s; D5 single-sourced in `natives/savepoint.rs` (three-dialect intersection only) with a source-scan ratchet over every emitter incl. the PHP leg; the nested `RELEASE SAVEPOINT` branch NO test had ever run is now covered. MySQL/PG live tests written but SKIP (no server in-container — **CD-22**, stated gap) | Broken reuse + ~75× |

## Wave 2 — structural gates

| # | Item | Notes |
|---|---|---|
| 2.1 | ~~**DEC-356**~~ — **BUILT 2026-07-30**: D + C + Invariant 3 widened. Found a VERIFIED compiler PANIC (`html"…"` in a tuple → `unreachable!`), plus `Item::Test` and `Stmt::Destructure` gaps. Leaf sets single-sourced as or-pattern macros; six cohesion splits left Invariant 13 net-negative (4 files dropped under the hard cap). Follow-up B QUEUED. |
| 2.2 | ~~**DEC-377**~~ — **BUILT 2026-07-30**: `src/transpile/helper_buckets.rs` classifies all **165** helpers (68 bucket-1 / 97 bucket-2 / **0 bucket-3**) with a both-directions ratchet. All 17 bucket-3 candidates REFUTED by reading them; both attached findings were wrong (`uri_*` already uses PHP 8.5's URI extension, `text_*` exists because PHP is byte-oriented — verified against php-8.5.8); `__phorj_trim` is a phantom. The count was wrong three times (168 → "149 real" → 165) and is now asserted, not claimed. |

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
| 5.1 | **DEC-342** UFCS receiver completion **+ WILDCARD-IMPORT completion** (`Ctrl+Space` on an empty line lists every wildcard-imported symbol and filters as you type) + import-gating both ways + the "exists in `Core.X` — add the import" diagnostic **with a quick-fix** + call-site spans + the ambiguity error. Measured against **DEC-375** (the LSP is the expert companion) |
| 5.2 | **DEC-341** the pre-verified 5-rule TextMate string section (81/383 → 0/383) **plus** the `vscode-textmate` pre-push gate |
| 5.3 | **DEC-346** migrate the 391 zero-judgement UFCS sites — **`Output.printLine` stays qualified** |
| 5.4 | ~~**DEC-350**~~ — **BUILT 2026-07-29** (verified: `src/cli/preludes.rs:777` says `Core.Database`; its stale doc references were swept 2026-07-30). DEC-394's prefix drop still OPEN. Rename to `Core.Database.Connection`, drop the `Module` suffix — ALSO lands **DEC-394**'s prefix drop in the same sweep (`HttpTimeoutError`/`MailTimeoutError` → `TimeoutError`, module-scoped injected classes + the hard collision error): both are stdlib-wide error/type renames, so one codemod, one re-baseline |
| 5.5 | ~~**DEC-354** the narrowed Claude bundle: 7 skills, **allow-list-only** permissions, `precompact-handoff` only, no session-remember, no MCP~~ — **BUILT 2026-07-27** (out of order, at the developer's request). Also produced **DEC-387** (`AskUserQuestion` FORBIDDEN — plain-text questions). One residual: `settings.json.pending` awaits the developer's local `apply-pending-settings.sh` run — see SLICE-STATE. **DEC-388 (2026-07-27) reopened and ruled four bundle items DEC-386 closed too broadly:** 388.1 disk-reclaim BUILT · 388.2 `/forge` import REVERSED-and-BUILT (partial reversal of the DEC-354 drop) · 388.3 `backend-parity-reviewer` agent BUILT · 388.4 validate-infra in pre-push BUILT · 388.5 `/qa-sweep` queued after Wave 0 |
| 5.6 | **DEC-369** vocabulary sweep: "cooperative tasks"; `uses_concurrency` → `uses_tasks`; delete Invariant 14's phantom flag; **"concurrent"/"parallel" reserved** |
| 5.7 | **DEC-371** strike PHP-absence from the four contaminated rationales; mark DEC-037 superseded; re-open `defer` inside DEC-364; add the standing rule beside Invariant 16 |

## Wave 6 — the big one

| # | Item |
|---|---|
| 6.1 | **DEC-370** real parallelism: **isolated tasks + copying channels** as the target, **data-parallel stdlib combinators** as the first shippable slice, `E-TRANSPILE-PARALLEL-NO-PHP` per DEC-133's precedent. **Owed measurements first:** copy-at-boundary cost, and per-thread instantiability of interpreter/VM state |

## Wave 7 — the on-hold tail (DEC-379…386, ruled the same session)

| # | Item |
|---|---|
| 7.1 | ~~**DEC-379**~~ — **BUILT 2026-07-30** (reproduced: all three legs called a `private` method through an interface receiver; per-overload visibility keyed on the CONFORMING overload; F-032 closed, CD-28 opened) close the `E-IFACE-VIS` overload bypass — a soundness hole, do it early |
| 7.2 | **DEC-385** merge `Core.Text` into `Core.String`, deprecate the module — **must land BEFORE DEC-342's UFCS completion**, or `line.length()` fires the ambiguity error on ordinary code |
| 7.3 | **DEC-384** stdlib submodule wildcards (`import Core.Http.*;`) — order the native pre-pass against the wildcard hook. `import Acme.*` already works; bare `Core.*` stays rejected |
| 7.4 | **DEC-386** the cheap tail: close DEC-200 as already-ruled · `DateTime` gating consistent with DEC-353 · delete the group-`{}` sort no-op · deprecate `Core.File` · close the bundle's Q-J1…8 as superseded |
| 7.5 | **BUILD the lifetime pair** — **DEC-205** (`Rc` cycle leak: PHP-style threshold collector first, `Weak<T>` second) + **DEC-204** (`Runtime.onShutdown(fn)`, SIGINT/SIGTERM, lands with Ω-2 `Core.Process`). Nothing to rule: **DEC-390** (developer, 2026-07-29) closed DEC-383 as bookkeeping — its forks (a)/(c) *are* DEC-205/DEC-204, both ruled 2026-07-12 |

## Wave 8 — the two research slices (do NOT start these at a low token budget)

| # | Item |
|---|---|
| 8.1 | **DEC-380** chase the `jsonround` win. Name the blocking constraint, re-examine the proxy-based no-win verdict, cost a real `Value::JsonArena` / lazy-materialise / index-handle-instead-of-`Rc`, and revise a blocking invariant if that is what it takes. WIN-OR-FLAG + no-hidden-loss both apply |
| 8.2 | **DEC-382** XML/DOM/XPath via a vetted crate — the 15th dependency, with the `Cargo.toml` + UNIFIED-SPEC policy row updated in the same change. **Best parity-per-effort item left** |

## Still OPEN, deliberately unruled

**L-19 · L-22 · L-25 · L-28 · L-31 · L-33 · L-86** — Claude had titles only and refused to invent
recommendations; developer-approved to defer. L-31/L-19 look mechanical; **L-22 and L-33 look substantial.**

## Owed measurements (none of these are optional)

1. **DEC-339** — how many `examples/`+`tests/` sites the redeclaration rule breaks. Needs the diagnostic to exist; report before migrating.
2. **DEC-357** — whether anything in-tree writes to a capture. Any hit is a **bug found**, not migration burden.
3. **DEC-365** — the two **owed verdicts**: `floatloop` (WIN→LOSS on a discarded-cpuset run) and `queryparse` (0.146 here vs DEC-338's ~0.88×, so **DEC-338's near-parity claim stays un-certified**). Both need a dev-box run.
4. **DEC-370** — copy-at-boundary cost + per-thread runtime instantiability.
5. **DEC-377** — the 168-helper classification.


## Stale-status postmortem (2026-07-30)

Row **0.4** was "flip the 40 stale status labels", and it was marked done — yet on 2026-07-30 a count of
this very file found **seven** rows still unflipped (0.2, 0.4, 1.1, 1.2, 1.3, 1.4, 5.4) and the decision
register still saying *"RULED — build queued"* for **four shipped features** (DEC-339/340/363/367). A
fresh session reading either SSOT would have concluded Wave 1.1–1.4 was unbuilt.

The lesson is the one DEC-361 and DEC-377 both landed on: **a status label with nothing asserting it goes
stale, and a sweep that fixes labels by hand fixes them once.** 0.4 swept prose and missed status columns.
The countable check that caught this — parse every `N.N` row, compare its BUILT tag against the register
and against whether the code exists — is cheap and should be run at the start of any status question rather
than trusting either file.

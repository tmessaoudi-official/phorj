# Full Reopen Audit — 2026-07-16

> **Mandate (developer, 2026-07-16, at-desk):** full complete rich deep audit/review of everything
> done. ALL KNOWN_ISSUES and ALL decisions reopened. The bar: **phorj must be conceptually,
> theoretically, and practically better / faster / safer / more secure / more intuitive than PHP**.
> Every deviation from PHP is justified strictly or FLAGGED for the developer. Anything
> non-generic or opinionated is flagged too. Architecture bar: clean, structured, decoupled,
> no fat files, better folder structure. Phorj must also stay AHEAD of PHP (8.6 plans in scope).
>
> **Protocol (ruled via AskUserQuestion, recorded in MASTER-PLAN §0.2):** audit-first ZERO source
> changes (doc-only consolidation commits allowed) · full external PHP re-sweep incl. 8.6
> RFCs/roadmap · FULL depth on every row (~180 register+issue rows, each gets a written verdict) ·
> checkpoint triage per dimension, flags brought one-by-one · everything unified into
> MASTER-PLAN/UNIFIED-SPEC.
>
> Baseline: HEAD `6b9256ba` (== origin/master, pushed). Verdict vocabulary:
> `JUSTIFIED(why)` / `FLAGGED(F-###)` / `OBSOLETE` / `SUPERSEDED(by)`.

## Dimension cursor

| Dim | Scope | Status |
|-----|-------|--------|
| D0 | PHP 8.4/8.5 surface re-sweep + 8.6 RFC ahead-watch vs the 824-row matrix | ▶ IN PROGRESS |
| D1 | Decision register full reopen (149 DEC rows) | pending |
| D2 | KNOWN_ISSUES full reopen (every row) | pending |
| D3 | Architecture / clean code / folder structure | pending |
| D4 | Security (every native surface vs PHP's equivalent) | pending |
| D5 | Perf-claim re-verification (WIN/HOLD/LOSS ledger) | pending |
| D6 | Docs drift + SSOT unification (runs throughout) | continuous |

## Flag ledger (grows monotonically; triage rulings recorded in C-decisions.md)

| Flag | Dim | Severity | One-liner | Ruling |
|------|-----|----------|-----------|--------|

---

## D0 — PHP surface re-sweep + 8.6 ahead-watch

### D0 sources (fixed list, per protocol)

- php.net PHP 8.5 release page + UPGRADING/migration guide
- php.net PHP 8.4 release page + migration guide (matrix predates parts of it)
- wiki.php.net/rfc index — accepted / under-discussion / draft targeting 8.6
- SPL + function-category inventory spot-checks against the 824-row matrix

### D0.1 Source A inventory (PHP surface — external)

<!-- filled during sweep -->

### D0.2 Source B inventory (phorj coverage — matrix + repo)

<!-- filled during sweep -->

### D0.3 Delta list

<!-- every item appearing in only one inventory is an automatic finding -->

### D0.4 PHP 8.6 ahead-watch

<!-- accepted/likely RFCs phorj should pre-empt or already beats -->

---

## D1 — Decision register reopen

<!-- one verdict line per DEC row -->

## D2 — KNOWN_ISSUES reopen

<!-- one verdict line per issue row -->

## D3 — Architecture

## D4 — Security

## D5 — Perf ledger

## D6 — Docs unification log

# Archived plan documents — finished work, kept for provenance

These are **frozen per-slice plan files whose work has SHIPPED**. Nothing here was deleted: the
2026-08-31 consolidation ruled that finished plans move to an archive directory with a pointer table
rather than being removed, so `git grep` still finds them and their reasoning stays readable.

**Do not treat these as current.** Where an archived plan and a live SSOT disagree, the live SSOT
wins. The live SSOTs are the Invariant-19 quartet:

- `docs/plans/MASTER-PLAN.md` — the roadmap
- `docs/plans/SLICE-STATE.md` — the current cursor
- `docs/specs/UNIFIED-SPEC.md` — the language surface
- `docs/research/full-audit/raw/C-decisions.md` — the decision register (append-only)

Each plan below states a design as it was understood *at authoring*. Several contain sections that
were later superseded — some carry inline `⚠ SUPERSEDED` markers, and the shipped behaviour is
occasionally the OPPOSITE of what the surrounding paragraph proposes. Read the register row, not the
plan, for what was actually ruled.

**One consequence worth knowing.** The DEC-362 doc-guards ratchet globs `docs/plans/*.md`, so files
in here are OUTSIDE its checks — G3's "a SHA must carry a ref or a subject" no longer applies to them.
That is deliberate: these are frozen historical documents that explicitly are not current, and
enforcing live-document hygiene on them would add noise without protecting anything. It cost nothing
at the time of the move — the baseline held zero frozen violations for any of these six, so no
existing violation was laundered out of enforcement by archiving. If a future archive move would take
a *baselined* violation out of scope, say so in that commit rather than letting the ratchet shrink
quietly.

| Archived plan | What shipped, and where its content now lives |
|---|---|
| `2026-08-22-s3-3-http-serve.plan.md` | **S3.3a–e SHIPPED** 2026-08-23 (DEC-455.12). `Http.serve(cfg, handler)` registration, `respond` retired, the web examples converted to projects. Its §3/§3c rejected-architecture reasoning — *why the accept loop is NOT inside the native* — was rescued BEFORE archiving into `src/serve/mod.rs`'s module doc and a dated NOTE row on the DEC-331 register block, because it existed nowhere else and the file itself warned "a fresh session must not rebuild this". Its §3b flag-vs-config paragraph is SUPERSEDED by DEC-455.14: the flag wins loudly, it does not hard-error |
| `2026-08-28-s3-4-role-mismatch.plan.md` | **S3.4 SHIPPED** 2026-08-28 (DEC-455.15) — `E-NO-ENTRY-FOR-ROLE`, symmetric across `phg run`/`phg serve`, prompting only on a real TTY and defaulting to NO. The register row mirrors the plan in full; nothing unique remained |
| `2026-08-29-s3-5-inbound-tls.plan.md` | **S3.5 SHIPPED** 2026-08-29 (DEC-455.16) — inbound TLS behind `http-server-tls`, every misconfiguration a startup refusal rather than a quiet fall back to plaintext. Its §5 obligation to run the milestone panel was discharged 2026-08-31; the verdicts and the still-OPEN gate live in the `SLICE-STATE.md` 2026-09-02 addendum |
| `2026-08-23-transpile-ns-prelude.plan.md` | **SHIPPED + certified** (DEC-455.11) — the namespaced-emit blocker fixed centrally with one alias block rather than five qualified emitters |
| `2026-08-04-lift-attr-and-hoist.plan.md` | **BOTH SLICES BUILT** — Slice A (#48 lifter hoist) 2026-08-04 as a narrowed sound subset after measurement refuted the agreed literal-hoist shape (DEC-397); Slice B (LIFT-ATTR, #46) 2026-08-05 (DEC-435/436), code at `src/lift/lifter/attrs.rs`. Its header's "remain OPEN and must be re-planned" was corrected before archiving — false since 2026-08-05. The 31-finding 3C panel transcript it carries is kept as a worked example of a plan being refuted |
| `2026-07-28-consistency-audit.plan.md` | **One-shot audit, completed.** Its findings landed as corrections across the quartet; the register carries the rulings it produced |
| `2026-07-26-ruled-build-order.md` *(folded 2026-09-02)* | **FOLDED into `docs/plans/MASTER-PLAN.md` §0.1**, because while it lived outside MASTER-PLAN it was the de-facto queue — the parallel-SSOT shape Invariant 19 forbids. Its header claimed "every item below is RULED and NOT YET BUILT … Wave 5.5 is the only row BUILT", false since 2026-07-29; the folded section corrects it and marks Wave 0, Wave 1 and Wave 3's DEC-364/347/348 as built |

---
name: phg-ask-human
description: >
  phorj's additions to the global /ask-human question protocol — its mandatory cases
  (language/design decisions under Invariant 15, the DEC-268 cap), the DEC-row rule and a worked
  example, and the every-mode Invariant-15 carve-out (autonomous: record PENDING, never ASSUMED).
  The protocol itself is global ask-human § Question quality.
user-invocable: true
---

<!-- THINNED 2026-09-28 (review-remediation 7.6): the shared question-quality protocol (five parts,
  shape, non-negotiable rules, when mandatory / not) moved to the global `ask-human` skill,
  § "Question quality"; this file keeps only what is specific to phorj. History: AskUserQuestion was
  banned (the 2026-07-27 cross-repo ruling; developer-ruled here 2026-07-30) in the cloud-container era (it failed silently there) and re-inverted 2026-08-18
  (de-containerization ruling); renamed ask-human → phg-ask-human the same day (a repo skill may not share
  a global skill's name). The full pre-thinning text is in git history. -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /phg-ask-human — phorj's additions to the global /ask-human question protocol:
>     its mandatory-question cases and a worked example.
>
> No flags — invoked automatically by Claude whenever a decision belongs to the developer.
> ```

---

# Question protocol — phorj additions

The protocol — the five required parts, the non-negotiable rules, when a question is mandatory and
when it is not — is the global `/ask-human` skill, § "Question quality". Whether a question stops
the turn or is shown and answered with the recommended option is the GLOBAL `~/.claude/CLAUDE.md` § "Mode — spec or autonomous" — **except the
Invariant-15 decisions below, which are never shown-and-answered in any mode**. This file adds only what is specific to this repo.

## Repo notes

- **Part 2 (the minimal example) here:** A **minimal concrete example** of the problem — for a language question, a runnable current-syntax program and its actual current output/error. Not a description of the program: the program. It goes **IN the `question` text** of the `AskUserQuestion` call (Invariant 15), not in prose above the call —
  prose outside the option list is missed while options are being compared.
- **Ruled decisions** (a DEC row) — never re-open one without new evidence.
- **Never re-ask a different question because the first one went unanswered** (kept from the pre-thinning protocol; the
  global skill does not carry it).
- **The escape**: Invariant 15 wants a visible *"none of these / challenge the premise"* escape. Prefer it as an option; when
  four real options already fill the tool (its cap is 4), a sentence in the `question` text pointing at the auto-appended
  "Other" is the escape.
- **RETIRED — do not reinstate:** the container-era plain-text protocol and its `❓ QUESTION` / `⏹ NO QUESTION` end-of-reply
  markers (DEC-418, developer-ruled 2026-07-30; retired 2026-08-18 with the de-containerization ruling, `c5cded43`). The
  retirement is recorded on the register's DEC-418 entry and in MASTER-PLAN G-4 beside DEC-387.

## When a question is mandatory here

- Any **user-visible language or design decision** (project CLAUDE.md Invariant 15 — the
  ADJUDICATION RULE: those are the developer's, made interactively, never ruled alone). This overrides the
  global § Mode "take the recommended option" in EVERY mode — the same override as the DEC-268 cap below, with
  a different effect: the cap STOPS the turn; this one, in autonomous mode, **records and moves on**. Neither
  option is built and it is never `ASSUMED (review)`. The record is a `PENDING (Invariant 15)` entry in the
  decision register (`docs/research/full-audit/raw/C-decisions.md`, the established form — KNOWN_ISSUES:
  "Recorded as a PENDING question in the register") plus a line in the `docs/plans/SLICE-STATE.md` cursor,
  and the closing check-in names it. Not a plan Decisions-Log bullet: the global `/next` and `/progress`
  surface `ASSUMED` entries only, so a PENDING written there is never brought back. In spec mode: ask, then stop.
- Any **destructive or hard-to-reverse action** — force-push and history rewrites above all. Note
  that ordinary `git add` / `git commit` / `git push` are **autonomously authorised** here
  (CLAUDE.md § "Git autonomy", DEC-417) and must NOT be asked about.
- A **certification loop that hits its cap** (DEC-268: 5 rounds with findings still open → ask, never
  silently proceed).

The global cases (two readings leading to materially different work, …) apply as well.

## Worked example

The block shows the CONTENT at full quality — deliver it through `AskUserQuestion` (the question text and the options),
never as the prose below. It is a spec-mode delivery; in autonomous mode the same content is the PENDING record.

```
## Question — should `10 / 0` be a compile error?

`phg check` currently accepts `10 / 0` and the fault only surfaces at runtime, identically on
all three legs. PHP behaves the same way, so this is parity — but it is a free win we are
leaving on the table, and it blocks nothing else.

Today:

    int x = 10 / 0;        // phg check: OK
    // phg run:  fault: division by zero

**Option 1 — reject it at check time (recommended).** A literal zero divisor is statically
   provable, so this is a pure win with no false positives, and DEC-058 says equal-or-better
   than PHP is the bar.
   After: `phg check` → `error: division by zero [E-DIV-ZERO]`, caught before the program runs.

**Option 2 — leave it as a runtime fault.** Keeps exact PHP parity and costs nothing to build.
   After: unchanged — the bug ships and fires in production instead.

**Option 3 — none of these / challenge the premise.** If you would rather this be a warning
   than an error, or want it grouped with the other literal-fault checks, say so.

I'll wait for your answer before doing anything else.
```

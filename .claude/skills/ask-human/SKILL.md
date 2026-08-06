---
name: ask-human
description: >
  PLAIN-TEXT question protocol — never AskUserQuestion. Context, a minimal failing example,
  clear numbered options, a recommended option first with its reason, then STOP and wait.
user-invocable: true
model: sonnet
disallowed-tools: AskUserQuestion
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  REWRITTEN 2026-07-27 (developer ruling, recorded under DEC-354). This skill previously
  mandated `AskUserQuestion` and forbade prose questions. That is now INVERTED:

    `AskUserQuestion` is FORBIDDEN in this project. It silently fails here — it returned
    "the user did not answer" four separate times on 2026-07-26 while the developer was
    actively at the keyboard, so a question asked that way can be lost with no trace and
    the turn ends as if nothing was asked. A gate that cannot fire is worse than none.

  The developer's instruction, verbatim: *"never use askUserQuestion — you must put the
  context clearly with clear options and clear examples with a recommended option"*.

  IT IS MECHANICALLY ENFORCED — AND A PRIOR CLAIM HERE THAT IT WAS NOT WAS WRONG (corrected
  2026-08-06, same day it was written). Every skill in this repo now carries
  `disallowed-tools: AskUserQuestion` in its frontmatter, matching all four sibling repos, and the
  running Claude Code DOES read that key: its SKILL.md frontmatter schema documents it as *"Tools
  removed from the model while this file is active. Comma-separated string or YAML list."*, and the
  loader destructures `a["disallowed-tools"] ?? a.disallowedTools` through the same normaliser it
  uses for `allowed-tools`, inside the same function that performs the `${CLAUDE_SKILL_DIR}`
  substitution. So stack's CLAUDE.md is right to call this mechanical backing.

  Earlier on 2026-08-06 this block asserted the opposite — that the key is inert and the siblings'
  declarations decoration — and that claim was propagated to seventeen places before the DEC-268
  panel refuted it. The cause is worth keeping, because it is the exact failure this repo added a
  rule about in the same commit: the check grepped a STALE npm copy of the CLI
  (`/opt/node22/.../claude-code/cli.js`, version 2.1.42) which has no skill-frontmatter loader at
  all, instead of the binary actually running (2.1.220). A probe that cannot fail is worse than no
  probe — and the right path had already been printed earlier in that same session. Never grade a
  claim `[Verified]` against an artefact you have not confirmed is the live one.

  What is NOT mechanical is the SHAPE of the question — context, example, numbered options,
  recommendation first, escape hatch, STOP. Nothing enforces that; it is yours to keep.

  The remaining container deltas that apply to this skill (the canonical set the other skills carry —
  `ls .claude/skills/` is the authoritative list, never a count in prose; delta 1 is this skill's
  entire subject, so it is not restated):
    • REPORTS/notes go to `var/claude/…` in the repo — never `~/.claude/projects/…`, wiped when
      the container is reclaimed. Never `git add` one.
    • `~/.claude/` here is GENERATED from `scripts/claude-bootstrap/` and overwritten
      unconditionally each SessionStart — a fix belongs in the repo copy, never in `~/.claude`.
    • EVERY REPLY ENDS WITH A MARKER LINE — `❓ QUESTION — …` or `⏹ NO QUESTION — …` as its literal
      last line (project CLAUDE.md § "Reply convention"). A question asked WITHOUT the `❓` marker
      is the exact failure this protocol exists to prevent: indistinguishable from a pause.
    • PROJECT RULES WIN on any conflict: `/home/user/phorj/CLAUDE.md`.
═══════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /ask-human — Plain-text question protocol: context + example + numbered options,
>              recommended first with its reason, then stop and wait.
>              AskUserQuestion is forbidden — it silently fails in this container.
>
> No flags — invoked automatically by Claude whenever a decision belongs to the developer.
> ```

---

# Plain-text question protocol

Every question to the developer is **ordinary text in the response**. No tool call, no dialog, no
hidden state. Then **STOP**: end the turn and wait. Never assume an answer, never proceed on a
default, never re-ask a different question because the first one went unanswered.

## The five required parts

| # | Part | Requirement |
|---|---|---|
| 1 | **Context** | What is being decided and *why it is being asked now* — one short paragraph. Enough that the developer needs no scrollback. |
| 2 | **Example** | A **minimal concrete example** of the problem — for a language question, a runnable current-syntax program and its actual current output/error. Not a description of the program: the program. |
| 3 | **Options** | Numbered, mutually exclusive, each with its own consequence. Ordinarily 2–4. |
| 4 | **Recommendation** | **Option 1 is the recommended one**, marked `(recommended)`, with the reason it wins stated in the same breath. |
| 5 | **Escape hatch** | A visible final option — *"none of these / challenge the premise"* — plus an explicit invitation to tweak any option. The developer must be able to answer *and* amend in one reply. |

## Shape

```
## Question — <one-line subject>

<Context: what is being decided, why now, what is blocked on it.>

Today:

    <minimal example — actual code, actual output/error>

**Option 1 — <name> (recommended).** <What it does.> <Why it wins.>
   After: <the after-state — the same example under this option>

**Option 2 — <name>.** <What it does.> <Cost or risk that makes it second.>
   After: <after-state>

**Option 3 — none of these / challenge the premise.** <What you would want to hear.>

I'll wait for your answer before doing anything else.
```

## Non-negotiable rules

- **Never `AskUserQuestion`.** Not as a fallback, not "just to try", not for a yes/no.
- **Never a bare `?` with no options.** If a real choice exists, enumerate it. An unstructured
  question makes the developer do the work of designing the options.
- **Always a recommendation.** "What do you prefer?" with no lean is an abdication. State the
  recommendation and why — the developer can then disagree cheaply.
- **The after-state goes in the option.** Prose written *outside* the option list is easy to miss
  while comparing options; put each option's consequence *inside* that option.
- **One STOP per question set.** Batch related questions (3–4 is fine when the developer asked to
  move faster), but end the turn after the batch — never answer your own question and continue.
- **Never re-open a ruled decision** without new evidence, and say what the new evidence is.
- **Challenge before accepting.** If the developer's proposal has a failure mode, say so in one or
  two sentences *and still deliver what was asked* under a stated assumption if they reaffirm it.

## When this protocol is mandatory

- Any **user-visible language or design decision** (project CLAUDE.md Invariant 15 — the
  ADJUDICATION RULE: those are the developer's, made interactively, never ruled alone).
- Any **destructive or hard-to-reverse action** — and `git push`, which project rules keep behind an
  explicit request even though `add`/`commit` are autonomous.
- A **certification loop that hits its cap** (DEC-268: 5 rounds with findings still open → ask, never
  silently proceed).
- Any point where two readings of the request lead to **materially different work**.

## When it is NOT needed

Routine judgement calls with an obvious default, and pure information questions. Asking about
everything is its own failure — it converts the developer into a decision queue. Decide what you can
defend, state the assumption, and keep moving.

## Worked example

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

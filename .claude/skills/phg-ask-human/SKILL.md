---
name: phg-ask-human
description: >
  Question protocol — AskUserQuestion with this repo's extra rules. Context, a minimal
  concrete example, clear options, the recommended option first with its reason, a visible
  "none of these / challenge the premise" escape, then STOP and wait.
user-invocable: true
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  RE-INVERTED 2026-08-18 (de-containerization ruling, recorded in /stack's
  docs/plans/decontainerization.plan.md § Decisions Log). The 2026-07-27 ruling banned
  `AskUserQuestion` because it silently failed in the Claude Code CLOUD CONTAINER ("the user
  did not answer" 4× on 2026-07-26). That environment is dead. On the developer's own machine
  the tool WORKS — `askUserQuestionTimeout` is `"never"` globally and the global
  ask-human-question-guard Stop hook mechanically REQUIRES it. Questions therefore use
  `AskUserQuestion` again. Everything below that is about question QUALITY (five parts,
  recommendation first, after-states, escape hatch, when-mandatory list) survives unchanged —
  only the delivery mechanism inverted back. Renamed ask-human → phg-ask-human the same day
  (global-is-reference ruling: a repo skill may not share a global skill's name). Invariant 15's
  ADJUDICATION RULE (question shape, options, after-states) is unchanged by the re-inversion.

  phorj ADAPTATION: the protocol itself is UNCHANGED from the cross-repo port — five parts,
  the shape template, and every non-negotiable rule are exactly as ported. Only the
  illustrations are phorj's own (language-design adjudication, DEC rows, the div-by-zero
  worked example).
═══════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /phg-ask-human — Question protocol: AskUserQuestion with context + a minimal example,
>              recommended option first with its reason, a visible "none of these /
>              challenge the premise" escape, then stop and wait.
>
> No flags — invoked automatically by Claude whenever a decision belongs to the developer.
> ```

---

# Question protocol

Every question to the developer goes through **`AskUserQuestion`** — context in the question text,
2–4 options with the recommended one FIRST (label it `(Recommended)`), and a visible
*"none of these / challenge the premise"* option (the built-in "Other" is the free-text escape, but
the challenge path must be a VISIBLE option, not only "Other"). Then **STOP**: end the turn and
wait. Never assume an answer, never proceed on a default, never re-ask a different question because
the first one went unanswered.

## The five required parts

| # | Part | Requirement |
|---|---|---|
| 1 | **Context** | What is being decided and *why it is being asked now* — one short paragraph. Enough that the developer needs no scrollback. |
| 2 | **Example** | A **minimal concrete example** of the problem — for a language question, a runnable current-syntax program and its actual current output/error. Not a description of the program: the program. |
| 3 | **Options** | Numbered, mutually exclusive, each with its own consequence. Ordinarily 2–4. |
| 4 | **Recommendation** | **Option 1 is the recommended one**, marked `(recommended)`, with the reason it wins stated in the same breath. |
| 5 | **Escape hatch** | A visible final option — *"none of these / challenge the premise"* — plus an explicit invitation to tweak any option. The developer must be able to answer *and* amend in one reply. |

## Shape

The five parts map onto the tool call: context → the `question` text (with the minimal example);
options → `options[]`, recommended first, each `description` carrying its own consequence AND
after-state; escape hatch → a visible final option. The worked example at the bottom shows the
CONTENT at full quality — deliver that content through the tool, not as prose. Prose layout for
reference:

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

- **Never a free-text prose question.** The global ask-human-question-guard Stop hook blocks a
  turn that ends on a bare `?` without an `AskUserQuestion` call — and it is right to.
- **Never a bare `?` with no options.** If a real choice exists, enumerate it. An unstructured
  question makes the developer do the work of designing the options.
- **Always a recommendation.** "What do you prefer?" with no lean is an abdication. State the
  recommendation and why — the developer can then disagree cheaply.
- **The after-state goes in the option.** Prose written *outside* the option list is easy to miss
  while comparing options; put each option's consequence *inside* that option.
- **One STOP per question set.** Batch related questions (3–4 is fine when the developer asked to
  move faster), but end the turn after the batch — never answer your own question and continue.
- **Never re-open a ruled decision** (a DEC row) without new evidence, and say what the new
  evidence is.
- **Challenge before accepting.** If the developer's proposal has a failure mode, say so in one or
  two sentences *and still deliver what was asked* under a stated assumption if they reaffirm it.

## When this protocol is mandatory

- Any **user-visible language or design decision** (project CLAUDE.md Invariant 15 — the
  ADJUDICATION RULE: those are the developer's, made interactively, never ruled alone).
- Any **destructive or hard-to-reverse action** — force-push and history rewrites above all. Note
  that ordinary `git add` / `git commit` / `git push` are **autonomously authorised** here
  (CLAUDE.md § "Git autonomy", DEC-417) and must NOT be asked about.
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

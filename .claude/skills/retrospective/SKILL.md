---
name: retrospective
spotlight: true
description: Use at the end of a long or complex session for deliberate end-of-session learning extraction and memory capture across hidden dependencies, naming surprises, behavioral quirks, and decision rationale.
user-invocable: true
disallowed-tools: AskUserQuestion
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  phorj CONTAINER ADAPTATION (DEC-354, built 2026-07-27; deltas 2/4/5/6 added or widened
  2026-08-06 by the second cross-repo bundle round). Imported from the developer's machine bundle
  `claude-setup-global-20260722`; DEC-354 ruled 7 of its 48 skills IN, each ADAPTED.
  These deltas OVERRIDE the body below wherever they conflict:

  1. QUESTIONS ARE PLAIN TEXT. `AskUserQuestion` silently fails in this container (observed 4x on
     2026-07-26), so a gate that "asks" cannot fire. Every "invoke ask-human" below means: print the
     question, a minimal concrete example, numbered options, and the recommended option FIRST with
     its reason, as ordinary prose — then STOP and wait. Protocol:
     `.claude/skills/ask-human/SKILL.md`. This file's frontmatter carries
     `disallowed-tools: AskUserQuestion`, which IS honoured: the running Claude Code reads that key
     from SKILL.md frontmatter and removes the tool while this file is active. So the forbidden tool
     is mechanically unavailable here, not merely discouraged — but the plain-text SHAPE of the
     question is still discipline, and that part nothing enforces.
  2. NO `advisor()` HERE. Independent certification = fresh-context read-only reviewer subagents
     (project CLAUDE.md, DEC-268 MAXIMAL ladder), run as the three phorj lenses:
     `backend-parity-reviewer` (correctness+regression), `safety-promises-reviewer`
     (security+safety-promises), `completeness-reviewer` (completeness+blast-radius). All three are
     REAL agent definitions in `.claude/agents/` as of 2026-08-06 — spawn them BY NAME via the Agent
     tool, in ONE message so they run concurrently, rather than re-describing their charter inline,
     so each lens's attack surface stays in one place. Self-grading is the LAST rung and must be
     DISCLOSED as self-graded in the output.
  3. REPORTS GO TO `var/claude/…` in the repo — gitignored (`/var`), survives compaction inside the
     session, never committed. NOT `~/.claude/projects/…`: that is wiped when the container is
     reclaimed, so a report written there is lost (Invariant 19 — only committed repo state survives).
     Never `git add` a report regardless — being ignored is what keeps them out of history, not what
     makes staging them harmless.
  4. `--scope=global|both` IS REMOVED wherever it appears. `~/.claude/` in this container is
     GENERATED from repo files by `scripts/claude-bootstrap/install.sh`, which since 2026-08-06
     overwrites them UNCONDITIONALLY on every SessionStart — so auditing `~/.claude` audits a copy,
     and a finding against it is fixed in `scripts/claude-bootstrap/` or not at all.
  5. ≤5 CONCURRENT SUBAGENTS (10 caused ~50% rate-limit failures upstream), and every pipeline agent
     writes its raw output to `var/claude/<stage>/raw/` BEFORE returning — in-conversation results do
     not survive autocompact, only disk files do. `Explore` cannot Write: use `general-purpose` for
     any agent that must persist a file.
  6. EVERY REPLY ENDS WITH A MARKER LINE — `❓ QUESTION — …` or `⏹ NO QUESTION — …` as its literal
     last line (project CLAUDE.md § "Reply convention"). This skill's output is a reply like any other.
  7. PROJECT RULES WIN on any conflict: `/home/user/phorj/CLAUDE.md` — the invariants, the full
     correctness gate, the git-autonomy override (`master` only, plain `git push`, no trailers),
     Invariant 19's canonical plan/decision homes.
═══════════════════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /retrospective — End-of-session deliberate learning extraction and memory capture across hidden dependencies, naming surprises, behavioral quirks, and decision rationale.
> ```
>
> Then output the complete flag table from the **"Flags"** section below. Then STOP.

---

# /retrospective — Session Learning Capture

Manual trigger for end-of-session learning extraction. Companion to the automatic Phase 8 learning prompt — use this for a deliberate sweep after long or complex sessions.

**Flags**:

| Flag | Behavior |
|------|----------|
| `--quick` | Skip to the 2 highest-signal lenses only (Failure pattern + Decision rationale); skips all 6-lens scan; output is a compact 2-question pass. |
| `--source=…` | **REMOVED (adaptation).** There is no cross-project memory index in this container — no `~/.claude/projects/*/memory/`, no `MEMORY.md`. See the note below. |

---

## Step 1: Reconstruct what happened

Review the session by scanning:
```bash
git diff --stat
git log --oneline -10
```

If git shows nothing (e.g. session worked on `~/.claude/` or other untracked paths), fall back to:
```bash
# ADAPTATION: there is no ~/.claude memory tree here, and ~/.claude itself is GENERATED from
# scripts/claude-bootstrap/ (overwritten every SessionStart), so scanning it tells you nothing about
# the session. Look at the repo instead — that is the only thing that survives the container:
git -C "${CLAUDE_PROJECT_DIR:-$PWD}" status --porcelain; ls var/claude/ 2>/dev/null
```
Also check the conversation context directly — it is the authoritative record of what was done.

Summarize in one paragraph: what was the core task, what approach was taken, what changed.

---

## Step 2: Extract non-obvious discoveries

**If `--quick` flag was passed**: scan only "Failure pattern" and "Decision rationale" lenses. Skip all others and jump directly to Step 3 with those 2 results.

For each of these lenses, ask the question and answer honestly — skip any where the answer is "nothing surprising":

| Lens | Question |
|------|----------|
| **Hidden dependency** | Did anything turn out to depend on something that wasn't documented? |
| **Naming surprise** | Was anything named differently than expected (script, var, path, command)? |
| **Behavioral quirk** | Did a tool, command, or system behave in a non-obvious way? |
| **Failure pattern** | What broke, and why — and would it be easy to repeat the mistake? |
| **Workaround** | Was something fixed with a workaround that future sessions should know about? |
| **Decision rationale** | Was a design choice made that isn't obvious from the code alone? |

---

## Step 2.5: Cross-project index enrichment (skip if `--source=project` or `--quick`)

**Compute current project slug:**
```bash
CURRENT_SLUG=$(echo "${CLAUDE_PROJECT_DIR:-$PWD}" | sed 's|^/|-|; s|/|-|g')
```

**Index scan — NOT APPLICABLE HERE (adaptation).** The upstream step read every other project's
`MEMORY.md`; the session-remember pipeline is not installed in this container, so there is nothing to
scan and a session must never report having "written to memory". Durable learning goes to the repo:
a `KNOWN_ISSUES.md` entry, a register row, or `docs/plans/*.plan.md`. Skip this step and say so.
```bash
# (removed — no cross-project MEMORY.md index exists in this container)
```

For each proposed entry from Step 2, compare its description + key terms against the index lines of all other projects:

- **No match in any other project** → proceed normally, save as project memory in Step 4
- **Match found in ≥1 other project** → annotate with `[SEEN in N other projects: slug1, slug2]` and mark as **PROMOTION CANDIDATE**

Annotation format for Step 3 preview:
```
[2] type: feedback | file: feedback_<slug>.md
    name: <name>
    description: <one-line description>
    body preview: <first 3 lines>
    ⚡ PROMOTION CANDIDATE — also seen in: stack, prsnl-pdf [2 other projects]
```

Be conservative on matching — only flag when there is strong textual overlap in the description. When uncertain, do not annotate (saving as project memory is safe; false promotion flags are noise).

---

## Step 3: Present proposed memory entries — confirm before saving

For each non-trivial discovery from Step 2, draft the memory entry but **do not write it yet**.
Present each proposed entry as a numbered preview:

```
Proposed memory entries (N total):

[1] type: project | file: project_<slug>.md
    name: <name>
    description: <one-line description>
    body preview: <first 3 lines of content>

[2] type: feedback | file: feedback_<slug>.md
    ...
```

**Hard stop** — ask in PLAIN TEXT (per `/ask-human`; `AskUserQuestion` is forbidden here), numbered options with the recommended one first, then STOP:
- question: 'N discoveries ready to save. Which entries should be saved?'
- options (adjust based on whether any PROMOTION CANDIDATEs are present):
  - 'Save all entries (Recommended)' — saves project-memory entries; PROMOTION CANDIDATEs also saved with a `run /memory-promote` reminder appended to the report
  - 'Save all + flag promotion candidates for /memory-promote' — same as above but opens `/memory-promote` immediately after saving
  - 'Save specific entries — list numbers in notes (e.g. 1, 3)'
  - 'Skip — abort without saving'

If no PROMOTION CANDIDATEs exist, omit the second option.

Do **not** write any memory file until the user responds.

If the user replies 'skip' or there are no discoveries: report "No memories saved." and stop.

---

## Step 4: Save confirmed entries

For each confirmed entry (all, or the numbered subset the user approved):

**ADAPTATION — there is no memory store here, so route by DURABLE HOME instead of by memory-file type.**
The upstream trio (`project_*.md` / `feedback_*.md` / `user_*.md`) lives under
`~/.claude/projects/<slug>/memory/`, which does not exist in this container and would be wiped with it.
Only committed repo state survives, so:

- **A project quirk, hidden dependency or workaround** → `KNOWN_ISSUES.md` if it is a limitation a user
  could hit, otherwise a decision-register row in `docs/research/full-audit/raw/C-decisions.md`.
- **A rule about how to work here** (a gate that must run, a trap to avoid) → the relevant section of
  project `CLAUDE.md`, or a reviewer-agent def under `.claude/agents/` if it is an attack surface a
  future review should carry.
- **A ruling or design decision** → a register row, mirrored into MASTER-PLAN + SLICE-STATE in the same
  change (Invariant 19).
- **Anything genuinely session-scoped** → `var/claude/` (gitignored). It dies with the container, and
  that is the correct lifetime for it.

Say which home you used, so the developer reviews it as an ordinary diff. **Never report having
"written to memory"** — `scripts/claude-bootstrap/CLAUDE-global.md` § "Memory System Toggles — NOT
APPLICABLE HERE" states plainly that the pipeline is absent.

Write each discovery as a standalone memory entry — not a bullet in an existing file unless it naturally extends one. Keep entries focused: one fact, one "Why:", one "How to apply:".

There is no `MEMORY.md` index in this container. Instead, land the learning where it survives: a
`KNOWN_ISSUES.md` entry, a decision-register row, or the relevant `docs/plans/*.plan.md` — and note in
the output which of those you used, so the developer can review it as a normal diff.

---

## Step 5: Report

Print a summary:
```
Retrospective complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Session scope : [1-sentence summary]
Discoveries saved : N
  - [file] → [one-line description]
  ...
Nothing to save : [list lenses that returned no findings]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

If Step 2 found nothing for any lens: report "No non-obvious discoveries — session was routine." and stop.

---
name: expanding-context
description: Use at the start of Phase 1 Brainstorm for any task. Widens context before committing to an approach — ensures no blind spots. Silent by default; surfaces only surprises, material risks, or wrong-problem signals.
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

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /expanding-context — Use at the start of Phase 1 Brainstorm for any task. Widens context before committing to an approach — ensures no blind spots. Silent by default; surfaces only surprises, material risks, or wrong-problem signals.
>
> No flags — invoked automatically by Claude during the reasoning workflow.
> ```

---

# Expanding Context

You are about to commit to an approach. This skill ensures you see the full territory
before you do.

**What this skill does**: runs the `/expand` dimension framework internally. You do NOT
output the full expansion to the user — you use the findings to inform your Phase 1 and
Phase 2 thinking. Produce only a brief internal summary (3-5 bullets) then proceed.

**When to surface the full expansion to the user**: only if they explicitly asked for it
(e.g. "what am I missing?", "give me the full picture", "expand this"). Otherwise keep it
internal and continue with the enriched context.

---

## Internal expansion (run silently)

Quickly sweep these 6 groups — 1-2 observations each, focus on surprises and non-obvious
items only. Skip dimensions where nothing is notable.

**I — Identity**: Is the scope what it appears to be? Is the mental model obvious?

**II — Structure**: What depends on this? What does this depend on? Any hidden contracts?

**III — Behavior**: What are the non-obvious failure modes? What edge cases exist?

**IV — Quality**: Any known issues, dark observability, or test gaps that matter here?

**V — Context**: What constraints or assumptions are load-bearing for this decision?

**VI — Discovery**: Any gaps, risks, or contradictions worth surfacing before proceeding?

**Questions**: Generate 2-3 internal questions — especially Strategic ones. If any question
would materially change the approach, surface it to the user before continuing.

---

## Decision gate

After the internal sweep:

- **No surprises found**: proceed to Phase 2 with enriched context. No output needed.
- **1-2 notable findings**: mention them briefly inline ("One thing worth noting before we
  proceed: ...") then continue.
- **Material risk or wrong-problem signal**: STOP and surface it explicitly. Ask the user
  before continuing. This is more valuable than any implementation.

---

## Skip conditions

Do NOT invoke this skill when:
- Input is already broad ("review the whole codebase", "plan the next sprint")
- Task is a simple lookup or rename with no design decisions
- You already ran this skill in the current session for the same topic
- The user explicitly said "just do it" (Small task signal — respect it)

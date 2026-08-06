---
name: handoff
spotlight: true
description: Use at the end of a session to save current state so the next session can continue cleanly without losing context about what was done, what is pending, and any non-obvious gotchas.
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
> /handoff — Use at the end of a session to save current state so the next session can continue cleanly without losing context about what was done, what is pending, and any non-obvious gotchas.
>
> No flags — invoked without arguments.
> ```

---

Save session state for clean continuation next session.

Write a handoff note so the next session can continue cleanly. Use your knowledge of the current session — you were here. Write in first person ("I").

**Path:** Derive it from the current project directory:
`var/claude/handoff/latest.md`, IN THE REPO (gitignored via `/var`)
where `{slug}` = the project directory with `/` replaced by `-`.
Upstream wrote to `~/.claude/projects/<slug>/memory/sessions/handoff.md`. **Do not.** That path is
wiped when the container is reclaimed, so a handoff written there is gone exactly when it is needed —
and it is what delta 3 of this file's banner forbids. Worse, the example here hardcoded the slug
`-stack`, which is a SIBLING REPO's slug: the line was copied and never adapted, and it survived the
2026-08-06 round that claimed to compare skill content. Both rent-watch and twes-in had already
fixed it. phorj's own PreCompact hook (`scripts/claude-bootstrap/hooks/precompact-handoff.sh`)
already writes `var/claude/handoff/latest.md` plus a timestamped copy, so this skill now agrees
with the machinery sitting beside it instead of contradicting it.
Example: if working in `/home/user/myproject`, the slug is `-home-developer-myproject`.

Also append a timestamped copy at `var/claude/handoff/handoff-$(date +%Y-%m-%d-%H%M%S).md`, matching
the hook's own naming. Create `var/claude/handoff/` if it does not exist. Never `git add` either file.

Format:

```
# Handoff

## State
{What's done, what's not. Files modified, decisions made, branch state. 2-4 lines max.}

## Next
{What to pick up. Priority order. 1-3 items.}

## Context
{Non-obvious gotchas, blockers, env state from this session. Skip section entirely if nothing.}

## Memory Updates
{Any user/feedback/project memories worth creating or updating based on this session.
 Format: "- [type] description" (types: user, feedback, project, reference).
 Skip section entirely if nothing new to persist.}
```

Rules:
- Under 25 lines total
- Specific: file paths, branch names, command names, variable names
- Forward-looking — next session doesn't care about the journey, only the current state
- "Memory Updates" is advisory — the next session will see it and decide whether to act
- If nothing meaningful to hand off, write: "No active work."

After writing the file, append `<!-- manual -->` on its own line at the very end. This marker tells the stop hook that a human explicitly saved state — it will skip overwriting with an auto-generated handoff.

Say "Saved." when done — nothing else.

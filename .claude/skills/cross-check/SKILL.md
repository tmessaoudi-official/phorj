---
name: cross-check
spotlight: true
description: Deep standalone validation of a spec or doc — hunts contradictions, undefined terms, unstated assumptions, missing sections and ambiguities, then certifies the analysis with the DEC-268 reviewer ladder. Use it on a spec before building from it, or to detect spec-vs-implementation drift (Invariant 17).
user-invocable: true
args: "<spec-file> [--dry-run]"
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  phorj CONTAINER ADAPTATION (DEC-354, built 2026-07-27). Imported from the developer's machine
  bundle `claude-setup-global-20260722`; DEC-354 ruled 7 of its 48 skills IN, each ADAPTED.
  These deltas OVERRIDE the body below wherever they conflict:

  1. QUESTIONS ARE PLAIN TEXT. `AskUserQuestion` silently fails in this container (observed 4x on
     2026-07-26), so a gate that "asks" cannot fire. Every "invoke ask-human" below means: print the
     question, its options and the recommendation as ordinary prose, then STOP and wait for a reply.
  2. NO `advisor()` HERE. Independent certification = fresh-context read-only reviewer subagents
     (project CLAUDE.md, DEC-268 MAXIMAL ladder). Self-grading is the last resort and must be
     DISCLOSED as self-graded in the output.
  3. REPORTS GO TO `var/claude/…` in the repo — gitignored (`/var`), survives compaction inside the
     session, never committed. NOT `~/.claude/projects/…`: that is wiped when the container is
     reclaimed, so a report written there is lost (Invariant 19 — only committed repo state survives).
  4. PROJECT RULES WIN on any conflict: `/home/user/phorj/CLAUDE.md` — the invariants, the full
     correctness gate, the git-autonomy override, Invariant 19's canonical plan/decision homes.
═══════════════════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /cross-check — Deep standalone validation of a spec or doc: contradictions, undefined terms, unstated assumptions, missing sections, ambiguities. Certified by the DEC-268 reviewer ladder.
> ```
>
> Then output the complete flag table from the **"Flags"** section below. Then STOP.

---

# /cross-check — Spec Validation

Parse `$ARGUMENTS`:

## Flags

| Flag | Behavior |
|------|----------|
| `<spec-file>` | Path to spec/doc to validate (required) |
| `--dry-run` | Print findings to conversation only; no output file written |

If `<spec-file>` not provided: report error and stop.

---

## Deep doc validation

One mode only. **Mode A (spec-vs-Jira) was DELETED on import** (DEC-354 / J.2): there is no Jira and
no Jira MCP server in this environment, so the mode could never run — a documented mode that cannot
execute is worse than an absent one.

### Step 1 — Read spec fully

Read `<spec-file>` completely.

### Step 2 — Independent check

Per project **CLAUDE.md's DEC-268 ladder** (investigate → certify → repeat; TWO consecutive fully-clean rounds; cap 5, then ask in plain text — never silently proceed): investigate the three angles yourself, then certify with **fresh-context read-only reviewer subagents** that read the spec themselves (`advisor()` does not exist here).

- **Angle 1** (expanding-context): Are there implicit requirements not explicitly stated? Assumed context that a reader might not share?
- **Angle 2** (adversarial): What internal contradictions exist? What claim is made in one section that is contradicted in another?
- **Angle 3** (blast-radius): What is missing? What should be specified but isn't? What edge cases are unaddressed?

Give the reviewers the spec and the analysis so far; if any raises something new, resolve it and re-run the round (the clean counter resets). Converge on two consecutive fully-clean rounds.

### Step 3 — Output findings

Findings categorized as:
- **CONTRADICTION** — claim in section A directly contradicts claim in section B
- **UNKNOWN** — term or concept used without definition or reference
- **ASSUMPTION** — implicit prerequisite not stated
- **MISSING** — section that should exist but doesn't (error handling, rollback, security, etc.)
- **AMBIGUOUS** — statement that can be interpreted multiple ways

### Step 4 — Write output

- `--dry-run`: print to conversation only, stop.
- Otherwise: write to `<spec-file>.validation.md`

---

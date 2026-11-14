# ground prep (2026-11-14) Plan

> Ground preparation after the north star page: the four global-tool groups the developer ruled on 2026-11-13, then
> the expert opinion. **No development in this plan** - no `src/` change, no defect row, no feature row. Its purpose
> is that later development sessions start from a correct, organised Claude configuration and a measured picture of
> the project.
>
> Rulings this plan executes live in the Decisions Log of `docs/plans/2026-10-08-audit-remediation.plan.md`: the entry
> stamped 2026-11-13 19:26 that begins "Ground preparation (developer, via AskUserQuestion): ALL FOUR global-tool
> groups", and the two entries stamped 2026-11-14 11:06 that begin "Session scope" and "Ground preparation order".
> They are not copied here (Invariant 19). The goal every row is judged against is `.claude/rules/north-star.md`.

## Decisions Log
- [2026-11-14 11:19] AGREED: Plan approved (developer, via AskUserQuestion, 2026-11-14): ground-prep group 1 runs under ONE plan - a dedicated plan file; /rules-split as a dry run first, applied only on the developer's go, with a load probe before its commit; every skill pinned to project scope; the Claude-config batch closes audit-remediation row 6 in one commit; one push; the two multi-agent audits (/skill-audit, /audit) wait for a fresh session. Certification at the 3C gate: advisor() only (developer's choice at the tier question). Rejected: /rules-split alone under its own gate; the inline five only; proceeding fully autonomously.

## Formal Plan

**Group 1 - Structure** (rows 1-6). Every skill is pinned to PROJECT scope: run with their defaults,
`/memory-promote`, `/skill-audit` and `/audit` would propose edits to the developer's global `~/.claude`, which is
another repository with its own guards.

1. **Plan and cursors.** This file; the active-plan pointer; one SLICE-STATE cursor line and one MASTER-PLAN mirror
   line; the two places in SLICE-STATE that call the defect-queue grading pass "the next step" are corrected.
2. **`/rules-split`.** Dry run first: the consumer inventory (every file that greps, parses or cites `CLAUDE.md`,
   the five expertise headers that list what they do not repeat, the three reviewer agents), a prune check of each
   dated fact against the code, and the proposed areas - shown to the developer as area, rules file, globs, moved
   headings and line counts. Applied only on the developer's go. Then: line citations in LIVE files become quoted
   anchors (frozen research files keep theirs), the intake rule is added, and the load probe runs on a scratch copy
   BEFORE the commit (`claude -p --setting-sources project`, one control file that must not load, loads counted as
   `nested_memory` attachments in the probe's transcript, never by marker text).
3. **Row 6 batch** - one commit, as ruled on 2026-10-08 (the audit plan's entry that begins "E - this repo fixes all
   of its own Claude config in one commit"). `/repair` (scan, plan, the developer's confirmation); `/agent-def
   --audit` on the three reviewer agents, with the stale DEC-268 sentence replaced by what DEC-518 ratified, shown as
   a diff for the developer's OK first; a parse check of the Status block of every plan file; leftover files;
   `/memory-promote --source=project --dry-run`, each promotion shown as a diff. Closes audit-remediation row 6.
4. **Push and refresh.** One push through `/long-run`, then `/ci-watch`; then `/discover-expertise --write`, after
   the last commit, because the store's freshness is judged against HEAD.
5. **`/skill-audit --scope=project`** - a fresh session; it spawns agents, so the model question comes first.
6. **`/audit --scope=project`** - same.

**Groups 2-4** (rows 7-9), in the order recommended on 2026-11-13 (a recommendation, not a ruling): standing
guards; shipped-product checks; the hunters sweep on a frozen commit.

**Expert opinion** (row 10): the phorj-versus-PHP verdict with its flaws, across every domain, written LAST from the
findings of rows 8 and 9 and from the checkers `.claude/rules/north-star.md` names. A bar with no checker is reported
as owed, never as met.

**Rules that bind every row**

- Nothing a gate, a hook or a reviewer agent relies on moves behind `paths:`. A path-scoped rule loads only when a
  matching file is READ, and an invalid glob matches nothing without an error.
- `bash scripts/doc-guards.sh` runs before every docs commit; a red guard stops the commit.
- A proposal to add a `permissions.deny` entry or a blocking hook is refused (CLAUDE.md, "Claude config in this
  repo"); a `settings.json` edit is handed to the developer.
- Agent-definition edits are shown as a diff and wait for the developer's OK.
- Commits by pathspec, the developer's identity, no trailers; a row's evidence goes in a follow-up commit, never an
  amend.

**Rollback.** `git revert` of the row's commit (never an amend after a push). The pre-split `CLAUDE.md` is
snapshotted outside the repo before row 2 writes anything.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Plan file, active-plan pointer, SLICE-STATE cursor line, MASTER-PLAN mirror line | S | doing | - | docs/plans/** |
| 2 | Group 1: `/rules-split` - dry run, the developer's go, apply, quoted anchors, intake rule, load probe before the commit | M | todo | - | CLAUDE.md .claude/rules/** .claude/skills/** docs/** |
| 3 | Group 1: row 6 batch in one commit (closes audit-remediation row 6) - `/repair`, `/agent-def --audit` on the three reviewer agents (stale DEC-268 sentence, diff first), Status-block parse check, leftover files, `/memory-promote` at project scope | M | todo | - | .claude/** CLAUDE.md docs/plans/** |
| 4 | Group 1: one push, CI watch, then `/discover-expertise --write` (global store, nothing in the repo) | S | todo | - | - |
| 5 | Group 1: `/skill-audit --scope=project` (fresh session, model question first) | M | todo | - | .claude/skills/** |
| 6 | Group 1: `/audit --scope=project` (fresh session, model question first) | M | todo | - | .claude/** CLAUDE.md |
| 7 | Group 2: standing guards - `/new-hook` docs-drift and PHP forward-compat (warn-only, exit 0), `/usage-report` baseline | M | todo | - | .claude/hooks/** scripts/** |
| 8 | Group 3: shipped-product checks - `/phg-qa-sweep` on the release binary, `/cross-check` on UNIFIED-SPEC | L | todo | - | - |
| 9 | Group 4: hunters sweep on a frozen commit - `/inspect`, `/sleuth`, `/gaps`, `/forge` with phg-lenses, then `/aggregate-findings` | L | todo | - | - |
| 10 | Expert opinion: the phorj-versus-PHP verdict with its flaws across every domain, from rows 8 and 9 and the north-star checkers | M | todo | - | docs/** |
<!-- /progress-block -->
### Blocked
### Needs input
- Where the defect-queue grading pass (SLICE-STATE, section DEFECT QUEUE; owed under DEC-578) falls: after row 9
  inside this plan, or as its own task. Not ruled. Row 9's hunters may add entries, which argues for after.
- The order of groups 2-4 is a recommendation; the developer may reorder rows 7-9.
### Needs research
### Fragile
- Row 2 changes the one file every session and every reviewer agent reads. A moved heading can break a consumer
  without an error; the consumer inventory and the load probe are the guard, and both run before the commit.
### Known issues
- The session banner names `.claude/EXPERTISE-REFERENCE.md`; no such file exists in the repo (checked 2026-11-14).
  Row 3's `/repair` settles whether the banner or the repo is wrong.

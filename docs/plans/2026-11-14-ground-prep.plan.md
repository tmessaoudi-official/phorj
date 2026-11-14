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
- [2026-11-14 11:25] AGREED: /rules-split outcome (developer, via AskUserQuestion, after the dry run of 2026-11-14): SMALL SPLIT + PRUNE - two path-scoped rules files (.claude/rules/php-oracle.md, .claude/rules/gate-mechanics.md, about 31 lines moved out of 'Toolchain & quality gate'), an index line left for each, three drifted figures corrected (the oracle's patch number dropped, 9 test files to 12, the size-gate warns figure re-stamped), the intake rule added, the php-bridge pack header repointed. Every other section stays in CLAUDE.md: gates and never-do rules must be loaded before any file is read. Rejected: prune and intake rule only; also moving invariants 2-8 behind src paths (whether a reviewer subagent loads a path-scoped rule is unverified). Noted, no edit: var/phorj-app is absent on this machine.
- [2026-11-14 11:57] AGREED: /repair plan (developer, via AskUserQuestion, 2026-11-14): APPLY S1-S4, M1 and O1 - the three reviewer agents' opening paragraph reworded to what DEC-518 ratified (diff shown first); phg-lenses names the global framework for the anti-bandaid gate; nine Status rows made parseable; one Routing sentence about the parent /stack repo's path-scoped rules; src/lift/** added to php-oracle.md; .claude/RESUME.md removed after a backup copy outside the repo. Manual, outside this repo: the banner wording, /stack's rule globs. Moved to row 5: phg-qa-sweep's panel line. Rejected: keeping RESUME.md; leaving the agents untouched.
- [2026-11-14 13:33] AGREED: row 3b agent edits: Tier A only (7 stale-fact fixes in the three reviewer agents, diff shown in the question); Tier B trims not applied. The nine drifted agents/<file>.md:NN cites in the domain packs become quoted anchors in the same commit. Developer via AskUserQuestion.
- [2026-11-14 13:33] AGREED: row 3b memory promotions: P1 P2 P5 P6 into the tooling pack, P7 P8 into the engine-parity pack (P8's format-test line into the tooling pack); P6 = the trap and its check only; P3 is global and stays listed for the developer. Developer via AskUserQuestion.
- [2026-11-14 13:33] AGREED: P4: memory note speed-levers-authorized is amended - item 10 superseded by the 2026-09-27 mode ruling, items 1-9 stand, including never dynamic workflows nor team agents. Developer via AskUserQuestion.
- [2026-11-14 20:21] AGREED: row 3b follow-up: commit ee942fdf (`docs(claude): reviewer agents - seven stale facts corrected; pack cites into them are quoted anchors; six lessons promoted from memory`) carried three instruction clauses that were not in the approved diff (backend-parity 'anything else you find is a defect, not a disclosure'; safety-promises 'a diff that widens the rejected set needs that ruling, not an implementation'; completeness 'check the capabilities string in src/lsp/mod.rs before quoting either list'); found by the 6C review; the developer ruled: trim all three, in a follow-up commit. Developer via AskUserQuestion.

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
| 1 | Plan file, active-plan pointer, SLICE-STATE cursor line, MASTER-PLAN mirror line - commit `docs(ground prep): the plan for the four tool groups and the expert opinion; cursors say ground preparation runs first` | S | done | 97503aba | docs/plans/** |
| 2 | Group 1: `/rules-split` - dry run, the developer's go, apply, quoted anchors, intake rule, load probe before the commit - commit `docs(claude): split the oracle and hook detail out of CLAUDE.md into two path-scoped rules; intake rule; three drifted figures` | M | done | 48fe8127 | CLAUDE.md .claude/rules/** .claude/skills/** docs/** |
| 3 | Group 1: row 6 batch in one commit (closes audit-remediation row 6) - `/repair`, `/agent-def --audit` on the three reviewer agents (stale DEC-268 sentence, diff first), Status-block parse check, leftover files, `/memory-promote` at project scope - commit `docs(claude): the Claude-config sync - reviewer agents say what DEC-518 ratified; nine Status rows parse again`; `/agent-def --audit` and `/memory-promote` were NOT run as skills in this row (a skim of the agents' headings and a keyword check of the feedback memories only) - they are row 3b | M | done | 72437f60 | .claude/** CLAUDE.md docs/plans/** |
| 3b | Group 1: `/agent-def --audit` on each of the three reviewer agents (a full read: the completeness reviewer has sections named for Rule 6's four dimensions, executed tests and visual evidence, and whether they restate the global rules is what the audit decides) and `/memory-promote --source=project` over all 176 notes (fresh session; any agent edit as a diff first) - both run as skills; audit: completeness-reviewer FAILS Rule 5 on one reproduced table (kept by ruling, Tier B declined), the other two pass, seven stale facts fixed (Tier A); memory: 183 notes examined (29 feedback + 154 project; "176" was the top-level file count) by description and by a citation check, 9 read in full, six promoted into the tooling and engine-parity packs; the nine line cites into the agents became quoted anchors - commit `docs(claude): reviewer agents - seven stale facts corrected; pack cites into them are quoted anchors; six lessons promoted from memory` | M | done | ee942fdf | .claude/agents/** .claude/skills/** |
| 4 | Group 1: one push, CI watch, then `/discover-expertise --write` (global store, nothing in the repo) - pushed at `docs: the cursor says rows 1-3 of the ground preparation are done; one more stale oracle stamp; a pack cite repointed`; pre-push gate green (3752 passed, 2 skipped); CI and release green on that commit; the store rebuilt from it (its judging step, practices.md, was not run) | S | done | 06c16bc8 | - |
| 5 | Group 1: `/skill-audit --scope=project` (fresh session, model question first) | M | todo | - | .claude/skills/** |
| 6 | Group 1: `/audit --scope=project` (fresh session, model question first) | M | todo | - | .claude/** CLAUDE.md |
| 7 | Group 2: standing guards - `/new-hook` docs-drift and PHP forward-compat (warn-only, exit 0), `/usage-report` baseline, and the rules-file guard (frontmatter closes, every `paths:` glob matches a file, every quoted anchor resolves on one line - packs and rules files both) | M | todo | - | .claude/hooks/** scripts/** |
| 8 | Group 3: shipped-product checks - `/phg-qa-sweep` on the release binary, `/cross-check` on UNIFIED-SPEC | L | todo | - | - |
| 9 | Group 4: hunters sweep on a frozen commit - `/inspect`, `/sleuth`, `/gaps`, `/forge` with phg-lenses, then `/aggregate-findings` | L | todo | - | - |
| 10 | Expert opinion: the phorj-versus-PHP verdict with its flaws across every domain, from rows 8 and 9 and the north-star checkers | M | todo | - | docs/** |
<!-- /progress-block -->
### Blocked
### Needs input
- Where the defect-queue grading pass (SLICE-STATE, section DEFECT QUEUE; owed under DEC-578) falls: after row 9
  inside this plan, or as its own task. Not ruled. Row 9's hunters may add entries, which argues for after.
- The order of groups 2-4 is a recommendation; the developer may reorder rows 7-9.
- Memory promotions - row 3b applied P1, P2, P5, P6, P7 and P8 (ruled 2026-11-14) and amended the "speed levers"
  note (P4: item 10 superseded, items 1-9 stand). Still open for the developer: (P3) never batch a commit with the
  edits or the guard run it depends on - it is a universal rule, so its home is the global `/pre-commit` skill, which
  this repo never writes; (P1) the completion pair "GA and global" has NO in-repo definition - the promoted line
  names today's stand-ins (PHP-parity, raw floor and Vision from the gap matrix; the readiness plan as the GA proxy)
  and labels that mapping Speculative: define the pair, or confirm the stand-ins. The uncited project notes other than
  the nine read in full (69 notes: milestone state, build maps, session records) were kept on their description alone.
- Manual, outside this repo (from row 3's `/repair`): the session banner names `.claude/EXPERTISE-REFERENCE.md`
  even when only `expertise-core.md` exists (the developer's global banner hook); the parent `/stack` repo's rule
  globs match files under this folder; `wasm-pack` is in the allow list and not installed; four older plans carry
  no Status block (post-slice3-consolidation, claude-bundle-cross-repo-audit, json-adt-jit,
  product-driven-gap-programme) - archive or keep is the developer's call; the developer's global end-of-life
  snapshot is 42 days old (`bin/eol-refresh.sh` in the global config, reported by the expertise index).
- [x] W1. Run the rules load probe, `bash /tmp/probe-rules-load-20261114.sh` - denied to Claude by the permission
  engine (walkthrough rules-split) — DONE 2026-11-14: run by the developer in the session; seven headless calls in a
  scratch copy; a Read of `tests/x.rs` or `.github/workflows/ci.yml` loaded only `php-oracle.md`; a Read of
  `scripts/git-hooks/pre-commit`, `.config/nextest.toml` or `Cargo.toml` loaded only `gate-mechanics.md`; no read and
  a non-matching read loaded no scoped rule; the never-load control rule never loaded.
### Needs research
- Row 5 leads: `phg-ask-human`, `phg-lenses` and `phg-qa-sweep` declare their description as a folded block, and the
  expertise index prints it as `>`; `phg-qa-sweep` tells its panel to run "DEC-268 MAXIMAL". From row 3b: neither
  the tooling pack's description nor the routing row in `expertise-core.md` carries a cloud keyword, so the promoted
  key-pin line loads only when that pack is pulled in for another reason; two pack cites do not support their
  sentence (language-design "Reviewer lenses" cites the safety reviewer's ladder bullet; tooling "Reviewer lenses"
  cites its sections 5 and 6 for "`unsafe` outside `src/jit/`", which is its section 1) - targets kept, mismatch logged.
- Row 6 lead: project `CLAUDE.md` Invariant 1 counts TWO disclosed exceptions to the spine, and one of them
  (concurrency) is a hard-error exclusion; the safety reviewer lists four more exclusions of the same kind
  (`E-FOREIGN-RUNTIME`, `E-TRANSPILE-DB`, `-HTTPCLIENT`, `-MAIL`). Whether those count as exceptions to the spine is
  not stated in any file read on 2026-11-14. Not ruled here; it is why the "anything else is a defect" clause was
  trimmed from the parity reviewer.
- Row 6 lead, so the audit does not reopen it: `completeness-reviewer` reproduces the global Rule 6 table (the
  `/agent-def` audit's one Rule 5 finding). The developer chose Tier A only on 2026-11-14, so the trim (Tier B) is not applied; its stated risk
  was that whether a project subagent loads `~/.claude/CLAUDE.md` was not measured. Measure that before proposing
  the trim again.
- The agents' closing "TWO consecutive fully-clean rounds" and `phg-lenses`' `/converge` line were left as written
  on the reading that `CLAUDE.md` keeps that rule for when a panel does run. That reading is inferred from the
  "MAXIMAL tier" paragraph, not ruled.
- Whether a reviewer SUBAGENT loads a path-scoped rule is unverified (the probe covered the main session only).
- The 2026-11-13 local gate note says 3765 tests and the 2026-11-14 push log says 3752 run, 2 skipped. The CI gate
  job reports the same total on both commits (55 result lines, 3704 passed), so no test left between them; the
  local difference is a counting basis that was not traced.
### Fragile
- Row 2 changes the one file every session and every reviewer agent reads. A moved heading can break a consumer
  without an error; the consumer inventory and the load probe are the guard, and both run before the commit.
### Known issues
- RESOLVED 2026-11-14 — the session banner names `.claude/EXPERTISE-REFERENCE.md`; no such file exists in the repo.
  Row 3's `/repair`: the banner prints that name whenever `expertise-core.md` exists; the repo is right and needs
  no such file. The wording fix is in the developer's global hook (Needs input, manual).
- RESOLVED 2026-11-14 (row 3: `CLAUDE.md` "Routing" now says those rules do not govern phorj; anchoring the globs
  is a `/stack` change) — a path-scoped rules file of the parent `/stack` repo (`/stack/.claude/rules/claude-config.md`) loaded into a phorj
  session when phorj's `CLAUDE.md` was read (observed 2026-11-14). `claudeMdExcludes` keeps `/stack`'s `CLAUDE.md`
  files out; it does not cover that repo's rules. Row 3's `/repair` records whether this needs a fix or a sentence
  in "Routing".
- No guard checks the rules files yet (frontmatter on line 1, every glob live, the `CLAUDE.md` index matching each
  file's `paths:`, every quoted anchor on ONE line - one anchor broke on a line wrap on 2026-11-14 and was caught
  by hand). Every failure of a path-scoped rule is silent, so the check is owed: it belongs to row 7.
- RESOLVED 2026-11-14 (row 3 added the glob; 14 lifter files name `preg_`) — `.claude/rules/php-oracle.md` does not list `src/lift/**`, and the paused scout row lifts `preg_*` calls. The index
  line in `CLAUDE.md` keeps the kernel loaded; row 3 checks where `preg_` pins are written and widens the globs.

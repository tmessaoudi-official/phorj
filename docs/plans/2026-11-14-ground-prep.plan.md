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
- [2026-11-15 17:42] AGREED: Certification (developer, typed prompt, 2026-11-15, verbatim): "use only advisor for validation/certification from now on !" - from this point every 3C and 6C gate is one advisor() call, no tier question is asked and no reviewer panel is spawned. Reading of its reach (Claude, not ruled): it holds until the developer lifts it, including after the ground work ends. Unchanged, because changing them is a separate ruling: the certification-schedule file and the DEC-518 text in CLAUDE.md, which still name a milestone panel.
- [2026-11-15 17:43] AGREED: Rows 5 and 6, how they run (developer, via AskUserQuestion, 2026-11-15): HYBRID - /skill-audit --scope=project spawns its one project agent on opus (the skill table; an auditor, not a certifier); /audit --scope=project runs sections A-E INLINE in the main session, no subagents, every numbered item answered with its command output or a stated not-applicable; reports under var/claude/<skill>/ (phg-lenses convention). Rejected: six agents as the skills prescribe (0.55-0.82M tokens of start-up); everything inline (the skill audit would be judged by the author of the pack edits).
- [2026-11-15 18:18] AGREED: Row 5 fixes (developer, via AskUserQuestion, 2026-11-15, replacement texts shown in the question turn): ALL EIGHT P1 fixes applied as shown - phg-qa-sweep (LSP capability list points at the capabilities string; the certification paragraph defers to /certify and asks through AskUserQuestion at the cap; a serve journey, row 10), phg-lenses (lens K writes raw/K.md; one new dimension for the import discipline, the ladder and determinism), phg-ship-slice (perf verdict from scripts/microbench.sh; the Status step also moves SLICE-STATE). Rejected: corrections without the serve journey; none now.
- [2026-11-15 18:18] AGREED: phg-lenses closing rule (developer, via AskUserQuestion, 2026-11-15): FOLLOW THE GLOBAL RULE - the line "Non-blocking closes - no interrupts" (container era, no ruling behind it) is replaced: a review ends with its findings count and the which-to-fix choice goes through AskUserQuestion. Rejected: keeping it as a ruling; leaving it untouched.
- [2026-11-15 18:19] AGREED: P2 batch (developer, via AskUserQuestion, 2026-11-15): ALL, project CLAUDE.md included - the drifted line cites in the packs and expertise-core.md become quoted anchors; the stale reason about the PATH php is corrected in phg-qa-sweep and in CLAUDE.md (sentence shown in the question turn); the tooling pack description and its routing row gain the keyword cloud; phg-ask-human notes that DEC-359 is ruled; the memory index and three note descriptions are corrected. Rejected: all except CLAUDE.md; memory only.
- [2026-11-15 18:19] AGREED: Fifty unratified ASSUMED (review) entries (the /audit P1; developer, via AskUserQuestion, 2026-11-15): a NEW PLAN ROW before row 10 - sorted by theme, the overtaken ones marked, the rest put to the developer in batches. Rejected: after the ground work; leaving them.
- [2026-11-15 19:05] AGREED: Closing rulings for rows 5 and 6 (developer, via AskUserQuestion, 2026-11-15): (1) backend-parity-reviewer.md item 9 - APPLY AS SHOWN: the PATH php is "whatever the machine carries (8.5.4 with JIT off on 2026-11-15) and is never the oracle"; (2) the three wordings of phg-qa-sweep that were applied without being shown first (the loopback sentence in ground rule 3; "only when /certify runs a panel"; the PATH php phrase) - KEEP ALL THREE; (3) stop after this, row 7 in a fresh session. Rejected: dropping the version from the agent line; leaving it; trimming the three wordings.
- [2026-11-15 22:49] AGREED: Row 7 plan (developer, via AskUserQuestion, 2026-11-15): GO, the config guard BLOCKS at pre-push (and runs in pre-commit) - scripts/claude-config-guard.py with its suite, scripts/php-forward-probe.sh with its suite (lint-only, on every installed PHP build newer than the oracle, found by glob, never a pinned patch), write-time warnings, the usage baseline. Rejected: the guard warn-only at pre-push; revising the plan.
- [2026-11-15 22:49] AGREED: Row 7 extras (developer, via AskUserQuestion, asked twice at the developer's request, 2026-11-15): ALL FOUR - E1 the line-cite ratchet (counts per file may shrink, never grow); E2 the claims table (crate total, default-build count and skill count stated in always-loaded and root files must equal Cargo.toml and the skills folder); E3 a record-only defect-queue entry PHP86-DEPRECATED-NAMES (no fix, no test, no grade); E4 axis 13's checker cell of north-star.md names the new guard, worded as shown in the plan turn. Rejected: E1-E3 without the north-star cell; E3 only.
- [2026-11-15 22:49] AGREED: Row 7 hook delivery (developer, via AskUserQuestion, 2026-11-15): VIA lint-on-write.sh - the registered hook dispatches to the two new scripts, warn-only, exit 0, one JSON document; no settings.json change and nothing written to the global hooks folder, so the /new-hook tool the 2026-11-13 ruling names is not run (it writes ~/.claude/hooks). Rejected: two project hook files registered by the developer.
- [2026-11-15 22:49] AGREED: Row 7 usage baseline (developer, via AskUserQuestion, 2026-11-15): PROJECT FILE - var/claude/usage/baseline-2026-11-15.json, phorj only, gitignored; nothing saved under ~/.claude/usage, so a later comparison passes --baseline with that file. Rejected: the machine-wide global baseline; both.
- [2026-11-16 08:32] AGREED: Row 8, how it starts (developer, via AskUserQuestion, 2026-11-16): ROW 8 NOW, in the same session - node 26 from nvm put on PATH for the session's commands, the playground journey SKIPPED and reported as not checked because wasm-pack is not installed. Rejected: stopping for a fresh session; waiting until the developer installs wasm-pack.
- [2026-11-16 19:49] AGREED: Model split for the remaining ground-prep rows (developer, to save Fable tokens and rate limits): switch to Opus NOW; rows 8, 9 and 9b run on Opus with Sonnet subagents capped at 3-5, one fresh session per row, findings written to files; row 9b is one file of the unratified ASSUMED entries with a recommended verdict each, the developer marks only disagreements; row 10 (the phorj-vs-PHP verdict) alone runs on Fable in a fresh session that reads those files. This reverses the earlier order 'switch model only after row 10'.
- [2026-11-16 20:07] AGREED: Row 8 QA half (developer, via AskUserQuestion, 2026-11-16): REUSE AND CERTIFY the morning sweep (var/claude/phg-qa-sweep/2026-11-16-0843/report.md; binary unchanged since 2026-10-29, no newer src/ file) - reproduce N-4, N-5, N-6 on the binary, grep the register and MASTER-PLAN for every 'new' item, mark journey 9 PARTIAL, then certify. Rejected: full rerun; certify as-is.
- [2026-11-16 20:07] AGREED: Row 8 recording (developer, via AskUserQuestion, 2026-11-16): RECORD-ONLY defect-queue entries for N-1..N-8 in SLICE-STATE (ungraded, proposed grade in the text, citing the report; no fix, no test), as row 7 did for PHP86-DEPRECATED-NAMES. Rejected: report only; only N-5 and N-6.
- [2026-11-16 20:07] AGREED: Row 8 cross-check scope (developer, via AskUserQuestion, 2026-11-16): /cross-check Mode B on UNIFIED-SPEC PLUS a sixth category DRIFT (spec claims checked one way against src/ and the register, each with grep evidence); four Sonnet agents, one per quarter (1-606, 607-1408, 1409-2138, 2139-2941); every finding re-verified by the main session; report in var/claude/cross-check/2026-11-16/ (phg-lenses convention), the reverse direction listed as not verified. Rejected: internal only; two-way drift (overlaps row 9's /gaps).
- [2026-11-16 20:45] AGREED: Row 8 queue set (developer, via AskUserQuestion, 2026-11-16, after the 6C check found the committed set wider than the ruling): RATIFY the two extra entries (QA-N9 cookie-attribute injection, PKG-VERBS-HELP), SPLIT QA-N3 and QA-N8 into two rows, MOVE QA-N6 to table D (the LIFT-ECHO-INT contract covers bool literals only). Rejected: strictly as ruled; as committed.
- [2026-11-16 20:45] AGREED: Row 8 N-9 publication (developer, via AskUserQuestion, 2026-11-16): PUBLISH WITHOUT THE PAYLOAD - the public SLICE-STATE entry names the class (a ';' in Cookie value or cookiePath injects attributes on all four legs; PHP refuses or encodes it) with no literal exploit string; the full reproduction stays in var/claude/. Rejected: as written; keep out of the repo.
- [2026-11-16 20:45] AGREED: Row 8 N-9 guard (developer, via AskUserQuestion, 2026-11-16, Invariant 15): RECORD AS PENDING now - a PENDING (Invariant 15) row in the decision register beside DEC-568 plus the SLICE-STATE cursor line, no build; decided at the grading pass. Options recorded with it: reject like setrawcookie; encode like setcookie. Rejected: ruling either now.
- [2026-11-16 20:48] AGREED: Row 8 history (developer, via AskUserQuestion, 2026-11-16): REWRITE THE TWO NEVER-PUSHED LOCAL COMMITS that followed `d79d8e31` (the first queue commit and the first row-8 evidence commit; neither exists any more) by a soft reset to `d79d8e31` and two fresh commits, so the N-9 payload never reaches the public history; backup = the local reflog (no branch: master is the only branch). Rejected: push as-is plus a fix commit; not pushing yet.

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
| 3b | Group 1: `/agent-def --audit` on each of the three reviewer agents (a full read: the completeness reviewer has sections named for Rule 6's four dimensions, executed tests and visual evidence, and whether they restate the global rules is what the audit decides) and `/memory-promote --source=project` over all 176 notes (fresh session; any agent edit as a diff first) - both run as skills; audit: completeness-reviewer FAILS Rule 5 on one reproduced table (kept by ruling, Tier B declined), the other two pass, seven stale facts fixed (Tier A); memory: 183 notes examined (29 feedback + 154 project; "176" was the top-level file count) by description and by a citation check, 9 read in full, six promoted into the tooling and engine-parity packs; the nine line cites into the agents became quoted anchors - commit `docs(claude): reviewer agents - seven stale facts corrected; pack cites into them are quoted anchors; six lessons promoted from memory`; follow-up commit `docs(claude): reviewer agents - three clauses that were not in the approved diff are trimmed; the ruling and a row 6 lead logged` (three clauses that were not in the approved diff, trimmed by ruling); CI and release green on the follow-up | M | done | ee942fdf 3fdc0960 | .claude/agents/** .claude/skills/** |
| 4 | Group 1: one push, CI watch, then `/discover-expertise --write` (global store, nothing in the repo) - pushed at `docs: the cursor says rows 1-3 of the ground preparation are done; one more stale oracle stamp; a pack cite repointed`; pre-push gate green (3752 passed, 2 skipped); CI and release green on that commit; the store rebuilt from it (its judging step, practices.md, was not run) | S | done | 06c16bc8 | - |
| 5 | Group 1: `/skill-audit --scope=project` - run as the skill, its one project agent on opus (an auditor, not a certifier); report `var/claude/skill-audit/2026-11-15.md`: 0 P0, 9 P1 (phg-qa-sweep 4, phg-lenses 3, phg-ship-slice 2), 33 P2; every P1 re-checked against its cited line by the main session; all nine fixed by ruling, and 51 line cites in the packs and `expertise-core.md` turned into quoted anchors that each resolve on exactly one line - commit `docs(claude): project skills - nine audit fixes in three skills; fifty-one line cites become quoted anchors; the PATH php reason corrected`; a follow-up commit (`docs(claude): the tooling pack back under its size budget; two consistency lines in phg-qa-sweep`) shortens anchor phrases after the 6C check found the pack 219 bytes over, lists the `serve` journey in the pack, and makes two lines of `phg-qa-sweep` agree with the ruled changes | M | done | d6e1e22d | .claude/skills/** .claude/rules/** CLAUDE.md |
| 6 | Group 1: `/audit --scope=project` - launched as the skill; sections A-E answered INLINE in the main session, no subagents, by ruling (F-K are global-only); report `var/claude/audit/2026-11-15.md`, audit number one, no baseline: 0 P0, 1 P1 (fifty unratified ASSUMED entries, now row 9b), 3 P2, 7 P3; every numbered item A1-E6 carries its command output or a stated not-applicable; fixed from it: the stale reason about the PATH php in `CLAUDE.md` (same commit as row 5) and the memory index (outside the repo); a later commit (`docs(claude): the parity reviewer's line on the PATH php; the closing rulings of rows 5 and 6 logged`) corrects the same reason in the parity reviewer, on the developer's OK | M | done | d6e1e22d | .claude/** CLAUDE.md |
| 7 | Group 2: standing guards - `scripts/claude-config-guard.py` (rules files, quoted anchors, the line-cite ratchet, derived claims; blocks in pre-commit and pre-push) and `scripts/php-forward-probe.sh` (lint of the transpiled output on every installed PHP newer than the oracle), each with its suite; `lint-on-write.sh` relays both at write time and reports a missing `jq`; the usage baseline; the defect-queue entry PHP86-DEPRECATED-NAMES; axis 13's checker cell | M | done | 8bda1366 ef376f43 | scripts/** .claude/hooks/** .claude/rules/** docs/plans/SLICE-STATE.md CHANGELOG.md |
| 8 | Group 3: shipped-product checks - `/phg-qa-sweep` on the release binary, `/cross-check` on UNIFIED-SPEC - the QA half is the morning sweep, run as the skill (two `Skill` calls in that session's transcript; `var/claude/phg-qa-sweep/2026-11-16-0843/report.md`, 10 journeys run, playground skipped: no wasm-pack), reused and certified by ruling (binary unchanged since 2026-10-29): N-4, N-5, N-6 reproduced; journey 9 corrected to PARTIAL; the excluded examples are 18, not 55; N-8's help gap and part of N-3 were already in KNOWN_ISSUES, and N-1 goes beyond the stated residual of a FIXED entry; the cross-check ran as the skill after `/phg-lenses`, four Sonnet agents (one per quarter) plus a cross-quarter pass (topic merge and a status join by DEC number), report `var/claude/cross-check/2026-11-16/report.md`: 84 documentation findings (proposed P1 five: stale "build-pending" banners that outrank correct prose, Core.Database's transpile leg stated two ways, functions "not bare-importable", the naming default); every P1/P2 fact re-run by the main session (four cells after the first 6C check); a cookie-attribute injection on all four legs found and reproduced (QA-N9, published without its payload; DEC-581 PENDING under Invariant 15); the queue set ratified by the developer after 6C (two extra entries kept, N-3 and N-8 split, N-6 in table D) - commit `docs(slice-state, register): ground-prep row 8 findings in the defect queue as record-only entries; DEC-581 pending; the cursor moves to row 9` | L | done | cabd57d7 | docs/plans/SLICE-STATE.md docs/research/full-audit/raw/C-decisions.md |
| 9 | Group 4: hunters sweep on a frozen commit - `/inspect`, `/sleuth`, `/gaps`, `/forge` with phg-lenses, then `/aggregate-findings` | L | todo | - | - |
| 9b | Ratify or reject the fifty unratified ASSUMED entries (audit-remediation 43, scout-forcing-function 4, twes-in-forcing-function 2, dependency-refresh 1; ruled 2026-11-15 to run before row 10): sorted by theme, the ones later work overtook marked, the rest put to the developer in batches of up to four questions; each bullet rewritten as ratified or rejected | M | todo | - | docs/plans/** |
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
- From rows 5 and 6 (2026-11-15), each the developer's to rule or to do:
  - the certification schedule file and the DEC-518 text in `CLAUDE.md` still name a milestone panel, while the
    2026-11-15 ruling is `advisor()` only; `phg-lenses` still says `/converge` runs the "DEC-268 MAXIMAL" ladder and
    the three reviewer agents still close on "TWO consecutive fully-clean rounds" - a tier that is now off;
  - `CLAUDE.md` "Routing" lists twelve global skills before "load `/phg-lenses` first", the skill's description lists
    nine, and neither names `/audit`, `/skill-audit`, `/qa-sweep`, `/repair` or `/certify`;
  - `phg-lenses` says reports live under `var/claude/<skill>/`; row 3b's promotion report went to the global store
    (the skill's default) - whether memory promotion counts as a review skill is not stated;
  - two kept memory notes carry an expired `expires:` date (`resume-2026-09-09-push-pending`,
    `resume-2026-09-24-row-5g`);
  - manual: the GitHub MCP server does not connect (authorization header), `gh` does the work;
  - DONE 2026-11-15 (developer's OK on the shown diff): `backend-parity-reviewer.md` item 9 no longer calls the PATH
    `php` a dev build; it reads "whatever the machine carries (8.5.4 with JIT off on 2026-11-15) and is never the
    oracle". The diff shown to the developer misquoted the tail of the old line ("never gate against it"; the file
    says "so a green run against it proves little"), so only the clause the diff changed was replaced and the tail
    was left as it stood.
  - RATIFIED 2026-11-15 (keep all three): two lines of `phg-qa-sweep` and one wording had been applied without
    being shown verbatim first, as consequences of ruled fixes - ground rule 3 gained "The `serve` journey's loopback
    socket is local, not network"; the reviewers' row of its model table reads "only when `/certify` runs a panel";
    its line on the PATH `php` reads "whatever the machine carries and never the oracle".
- [x] W1. Run the rules load probe, `bash /tmp/probe-rules-load-20261114.sh` - denied to Claude by the permission
  engine (walkthrough rules-split) — DONE 2026-11-14: run by the developer in the session; seven headless calls in a
  scratch copy; a Read of `tests/x.rs` or `.github/workflows/ci.yml` loaded only `php-oracle.md`; a Read of
  `scripts/git-hooks/pre-commit`, `.config/nextest.toml` or `Cargo.toml` loaded only `gate-mechanics.md`; no read and
  a non-matching read loaded no scoped rule; the never-load control rule never loaded.
### Needs research
- Row 5 leads, closed 2026-11-15: the "DEC-268 MAXIMAL" paragraph of `phg-qa-sweep` and the missing cloud keyword
  are fixed. Left as they are: three descriptions declared as a folded block (they parse; only the developer's global
  expertise index prints `>`); two pack cites that do not support their whole sentence (language-design "Reviewer
  lenses" cites the safety reviewer's ladder bullet; tooling "Reviewer lenses" cites its sections 5 and 6 for
  "`unsafe` outside `src/jit/`", which is its section 1) - targets kept, mismatch logged.
- Row 7, closed 2026-11-15 (work commit 8bda1366, subject `feat(guards): a Claude-config guard and a PHP forward-compat probe, each with its suite; lint-on-write relays both`). What it left:
  the 123 line cites are now a ratchet, not converted - a shifted cite still resolves, so each one stays unproven
  until it becomes a quoted anchor; the 66 anchors RESOLVE on one line each, and whether each supports its sentence
  is still not checked by anything; the syntax-tree variant counts in `CLAUDE.md` (38, 15, 11, 8) are not in the
  claims table because no script derivation was found reliable; the probe is lint-only (compile-time deprecations),
  one PHP 8.6 release candidate is the only newer build on this machine and it is a debug build; one of the 22
  examples the probe skips is `examples/interop/withdecls/src/php.d.phg`, refused alone with a type error (a project
  declaration file - whether the project as a whole transpiles was not checked here); the usage baseline is
  `var/claude/usage/baseline-2026-11-15.json` (phorj, 7 days: 495 calls, re-ask 3 of 30), to be passed with
  `--baseline`; `jq` is still named in neither `.claude/cloud-setup.sh` nor the git hooks, the hook now SAYS when
  it is missing; the tooling pack is still 9 bytes under its budget, so nothing about row 7 was added to it.
  Follow-ups after the 6C check: ef376f43 (subject `docs(ground prep plan): row 7 done with evidence; the guard reports a non-UTF-8 file instead of reading around it; the probe's skips named`),
  then the commit whose subject begins "test(guards): the config guard's suite drops inherited GIT_ variables".
  Lead for the developer, same class and NOT fixed here: `scripts/test-validate-infra.sh` and
  `scripts/test-cloud-setup.sh` also run `git init`, `git add` or `git commit` inside a fixture without dropping
  inherited GIT_ variables. Measured 2026-11-16 (git 2.56.1, a scratch repository): a pre-push hook gets no
  `GIT_DIR` and no `GIT_INDEX_FILE` from the root or from a subdirectory, a pre-commit hook gets `GIT_INDEX_FILE`;
  a linked worktree was not measured. All suites run in pre-push only, so the exposure is a push started from a
  context that does export `GIT_DIR`.
- Row 9 lead: the new closing sentence of sleuth lens K in `phg-lenses` sits inside K's quoted prompt, yet two of
  its three clauses (run K as its own batch; add `raw/K.md` to the synthesis list) instruct the session that runs
  `/sleuth`, not K. Read it that way when row 9 runs; the wording is ruled text.
- Row 8 leads (from rows 5 and 6): `node`, `npm`, `npx` and `wasm-pack` are not on PATH in a session shell (`nvm`,
  `bun`, `deno`, `pnpm` and `yarn` sit under `/stack/tools`), so the editors and playground journeys need a PATH or a
  stated skip; `phg --help` lists none of `add`, `install`, `update`, `remove`, though each verb's own `--help`
  answers; `phg-qa-sweep` has a new `serve` journey (row 10 of its table); the release binary of 2026-10-29 is
  current (no commit touches `src/` after it).
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

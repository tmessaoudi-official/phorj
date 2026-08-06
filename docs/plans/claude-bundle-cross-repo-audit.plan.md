# Claude bundle — cross-repo audit and unification plan

> Audit of the Claude global/project bundle across all five `tmessaoudi-official` repos, oldest
> integration first, to find what phorj's copy is missing. Written 2026-08-06 from clones of the four
> other repos at their then-HEADs. **This file is the portable artefact**: the same table applies when
> unifying any of the other repos, with the "phorj" column swapped.

## Decisions Log

- [2026-08-06] AGREED: audit all five repos and port the divergences into phorj (developer request).
- [2026-08-06] AGREED (P0, applied): delete the commented-out credential-copy block from `install.sh`
  rather than re-comment it — a disabled exfiltration path in a SessionStart hook is one uncomment
  away from publishing oauth tokens from a PUBLIC repo.
- [2026-08-06] AGREED (P0, applied): implement the `<!-- manual -->` guard `/handoff` already
  documents, on BOTH `latest.md` write paths, with tests.
- [2026-08-06] AGREED (P1, applied): `log_obs` defaults to `var/claude/logs/` in-repo, not
  `~/.claude/logs/`, which dies with the container.
- [2026-08-06] AGREED (round 2, applied): `install.sh` copies UNCONDITIONALLY — the repo is always the
  truth. `cp -u` was nondeterministic in both directions and its header claim was false.
- [2026-08-06] RULED by the developer: **`permissions.deny` stays empty, permanently.** In a web session
  a denied command is an unrecoverable dead end — there is no terminal to run it in. Closes round-1
  open question 3, and makes every `PostToolUse` hook warn-only by construction.
- [2026-08-06] AGREED (round 2): the siblings' `disallowed-tools:` frontmatter is INERT — Claude Code
  does not read the key. Port REFUSED; an inert key that reads as enforcement retires the vigilance
  without replacing it.
- [2026-08-06] AGREED (round 2, applied): `/qa-sweep` is WRITTEN for phorj's own surfaces, not ported
  from pdfturbo's browser/PDF machinery. Closes round-1 open question 2.
- [2026-08-06] AGREED (round 2, applied): write-time `PostToolUse` lint hooks are worth it, warn-only,
  and the load-bearing part is the Invariant 13 size advisory. Closes round-1 open question 1.

## Chronology (integration order, oldest → newest)

| repo | `CLAUDE.md` added | `.claude/` added | `.claude/` last touched |
|---|---|---|---|
| **stack** | 2026-03-31 | 2026-04-17 | 2026-08-06 |
| **pdfturbo** | 2026-06-11 | 2026-06-11 | 2026-08-05 |
| **phorj** (this repo) | 2026-07-19 | — | **2026-07-29** ← a week stale |
| **twes-in** | 2026-07-29 | 2026-07-29 | 2026-08-05 |
| **rent-watch** | 2026-08-06 | 2026-08-06 | 2026-08-06 ← newest, the reference |

Every repo shares the SAME bootstrap wiring — `SessionStart → scripts/claude-bootstrap/install.sh` and
`PreCompact → hooks/precompact-handoff.sh` (twice: manual + auto matchers). **The integration mechanism
is already unified.** What has diverged is the bundle's CONTENT and the per-repo `.claude/` surface.

## What phorj was missing

### P0 — security: a dormant credential-exfiltration path (FIXED)

`install.sh` carried, commented out:

```bash
# cp -R /root/.claude /root/.claude.json /home/user/phorj/claude-bundle
# git -C /home/user/phorj/ add claude-bundle && commit && push --force-with-lease
```

`~/.claude.json` holds the **oauth account, userID and machineID**, and phorj is a **public** repo.
rent-watch's `install.sh` header names phorj as the upstream it removed this from. **[Verified it never
ran:** no `claude-bundle` directory and no `*.claude.json` in this repo's entire history.] Deleted, with
the reason recorded in the header so it cannot be reintroduced as a convenience.

### P0 — a documented promise that was inert (FIXED)

`.claude/skills/handoff/SKILL.md:59` promises: *"append `<!-- manual -->` … this marker tells the stop
hook that a human explicitly saved state — it will skip overwriting with an auto-generated handoff."*
`precompact-handoff.sh` contained **no mention of "manual"** — following the documented ritual silently
lost the note at the next compaction.

Fixed with one `LATEST_IS_MANUAL` variable honoured by BOTH writes. The second write matters: rent-watch's
first attempt guarded only the default path and left the opt-in LLM path clobbering unconditionally, with
the log claiming "kept" two lines before overwriting. 8 new assertions (4 + 4 converses, so a
never-refresh guard cannot pass); **sabotage-verified** — removing the guard fails exactly 2.

### P1 — observability wrote to a path that dies with the container (FIXED)

`log_obs` defaulted to `~/.claude/logs/hooks-errors.log`, wiped when the container is reclaimed: every
line logged in a real session went somewhere nobody could read. Now
`$CLAUDE_PROJECT_DIR/var/claude/logs/` (gitignored via `/var`), `$OBS_LOG` still honoured for tests.
`install.sh` now also pre-creates `var/claude/`.

### P1 — five global-framework rules pointed at machinery that does not exist here (FIXED)

rent-watch adapted its `CLAUDE-global.md` to its real container; phorj's was still upstream boilerplate.
**Rule 10 was the sharpest: it directly contradicted phorj's own project CLAUDE.md.** All five adapted
2026-08-06, each keeping the upstream text visible as "what it used to say" so the change is auditable:

| rule | phorj's global copy says | reality in phorj |
|---|---|---|
| **10 git** | "Never commit or push without explicit user request" | project CLAUDE.md § "Git autonomy" **authorises** add+commit+push (DEC-417) |
| **13 observability** | `~/.claude/logs/`; cites `session-remember` | dies with container; `session-remember` is not installed |
| **15 loop** | "invoke the `loop` skill — non-negotiable" | no `loop` skill exists; the host provides `/loop` |
| **17 plans** | location via a `~/.claude/projects/<slug>/plan-location` sentinel | settled `docs/plans/*.plan.md` (Invariant 19); no sentinel exists |
| Memory toggles | presented as live | the session-remember pipeline is absent |

### P1 — the DEC-268 reviewer PANEL is 1 of 3 agents

`CLAUDE.md` § "Certification ladder (DEC-268)" mandates a **3-lens fresh-context reviewer PANEL**
(correctness+regression / security+safety-promises / completeness+blast-radius). Every other repo ships
exactly that shape; phorj ships one agent, so the mandated panel is **structurally impossible**.

| repo | correctness lens | security/safety lens | completeness lens |
|---|---|---|---|
| rent-watch | `tenure-correctness-reviewer` | `source-resilience-reviewer` | `completeness-reviewer` |
| twes-in | `domain-correctness-reviewer` | `tenancy-security-reviewer` | `completeness-reviewer` |
| pdfturbo | `export-fidelity-reviewer` | `safety-promises-reviewer` | `completeness-reviewer` |
| **phorj** | `backend-parity-reviewer` ✓ | **MISSING** | **MISSING** |

This is the largest remaining gap and the reason a whole session's 3C/6C gates ran "self-graded,
disclosed" — attributed to `advisor()` being unavailable, when the deeper cause is that two of the three
lenses were never authored.

**RESOLVED 2026-08-06.** Developer approved the split; both agents authored (not copied — the other
repos' versions are domain-specific):

- `safety-promises-reviewer` — the `unsafe` island (`#![deny(unsafe_code)]` + the one scoped `allow` in
  `src/jit/`, CI-enforced), Invariant-14 LADDER exclusions and their disclosures
  (`E-CONCURRENCY-NO-PHP` / `E-FOREIGN-RUNTIME` / `E-TRANSPILE-{DB,HTTPCLIENT,MAIL}`), determinism and
  the network boundary, EV-7 no-crash, the narrow security surfaces (DEC-363 header CRLF/NUL, SQL
  prepared statements, argon2, RE2-not-backtracking, rustls, secrets), and the honesty promises
  (dependency count SSOT, NO-HIDDEN-LOSS, Invariant 11, the anti-bandaid gate).
- `completeness-reviewer` — tests EXECUTED not merely written, Rule 6's four dimensions with the blast
  radius grep re-run independently, Invariant 9 examples-ship-with-features (the example corpus IS the
  byte-identity coverage), Invariant 17's 100% RULE across transpile/lift/LSP/both editors, Invariant 19
  SSOT-quartet consistency, and the mechanical caps.

`CLAUDE.md` § "Certification ladder" now carries the lens→agent table, so a future session finds the
panel instead of falling through to the self-graded rung.

### P2 — smaller divergences

| item | phorj | others | note |
|---|---|---|---|
| `permissions.deny` | **0 entries** | rent-watch has 4 (`Read`/`Edit` on `./.env`, `./.env.*`) | phorj has no `.env`, so low value — but harmless and consistent |
| `PostToolUse` write-time lint hooks | **none** | rent-watch 3, stack 5, pdfturbo 2 | phorj's equivalents (`cargo fmt`, `clippy`) are in git hooks instead — arguably already covered, but not at write time |
| `SubagentStop` reminder | none | stack 1 | stack-specific |
| `qa-sweep` skill | absent | pdfturbo | phorj's CLAUDE.md already lists it as "queued, not yet imported" |
| `THINKING.md` maintenance rule | "run `wc -l ~/.claude/THINKING.md`" | "edit the REPO copy — `~/.claude` is generated" | a real trap; wording ported. (Both repos then cited `cp -u`; round 2 §R1 replaced it with `cp -f`, so the failure mode is now "silently overwritten" rather than "permanently newer" — the rule is unchanged, `~/.claude` is generated either way.) |

~~All 13 core skills are present and identical in name across phorj/rent-watch; phorj is not missing
any.~~ **WRONG — and wrong in an instructive way. Corrected in round 2, §R4:** that was a *name-level*
check written up as a content-level conclusion, which is exactly the single-direction comparison the
bidirectionality rule exists to prevent. Comparing bodies, phorj's copy was the shortest of all five
repos in every one of the 13 rows.

## Round 2 (2026-08-06) — all four siblings re-read at their NEW heads

The developer finished unifying every sibling and asked for another pass. Heads audited:
rent-watch `b7867a4`, twes-in `10aa265`, stack `47d3353`, pdfturbo `3f041a1`.
Full detail is **DEC-451** in the register; this is the index.

| § | item | outcome |
|---|---|---|
| R1 | `install.sh` — `cp -u` → unconditional `cp -f`, "the repo is always the truth" | PORTED + `test-install.sh` (18 assertions). rent-watch flagged this as *port-OUT item 0 for all four siblings* |
| R2 | `permissions.deny` (was open question 3) | **RULED: stays EMPTY, permanently.** Developer, 2026-08-06 — a deny in a web session is an unrecoverable dead end because there is no terminal to run the command in |
| R3 | `disallowed-tools:` frontmatter, declared 13/13 by the siblings | **PORTED, 14/14 — after this round first got it WRONG.** The refusal ("the key is INERT") was graded `[Verified]` against a **stale npm copy of the CLI**; the running binary does read it. The DEC-268 panel caught it the same day. See DEC-451 §3 — the postmortem is the round's most useful output |
| R4 | skill **content** (not names) compared across all five repos | phorj carried a 4-delta banner vs the siblings' 9, and **no banner at all** on 5 skills → one canonical 7-delta banner on all 13 |
| R5 | `/cross-check` `--drift` mode | PORTED, re-grounded on phorj's checkable claims. Also fixed: it wrote its report to a **tracked** path, contradicting its own delta 3 |
| R6 | `/qa-sweep` (was open question 2) | **WRITTEN, not ported** — pdfturbo's is browser/PDF-specific. 10 journeys over the surfaces `cargo test` never reaches. Includes the first live LSP capability audit |
| R7 | "do not invent a subject" + "verify a NEGATIVE with a control" | PORTED to **all three** lenses, re-grounded on phorj incidents (#67's mis-titled panic; two silently-no-op reverts) |
| R8 | write-time `PostToolUse` lint hooks (was open question 1) | **BUILT, warn-only** — `rustfmt`/`phg format` plus an Invariant 13 size advisory that fires at write time, when the cheap fix is still to split rather than to shave comments |

**No open questions remain from the round-1 P2 tranche.** All three were ruled or built above.

### Port-OUT items — things phorj found that the four siblings should take

1. ~~**`disallowed-tools:` is inert**~~ — **WITHDRAWN, it was phorj's error.** The siblings were right
   and stack's "partial mechanical backing" is accurate. What phorj offers instead is the postmortem
   (DEC-451 §3): the check read `/opt/node22/.../claude-code/cli.js` (v2.1.42, no skill loader at all)
   rather than the running `2.1.220`, so the answer was determined by the artefact chosen — a probe with
   no ability to fail. And a second reviewer "confirmed" it because its spawn prompt NAMED that path.
   **Give a reviewer the claim, never your evidence path** — a lens told where to look can only audit
   your reading, not your choice of what to read. That is the portable lesson.
2. **rent-watch's `test-install.sh` non-fatal-`mkdir` case is vacuous** (R1). It uses `chmod 500` on the
   project dir; the suite runs as uid 0, which ignores mode bits, so the assertion never exercised the
   failure. Use a path whose parent is a regular file (ENOTDIR, fails for every uid). Doing so here
   immediately exposed a live defect the mode version could not see: `set -e` plus an unguarded trailing
   `mkdir` took the entire SessionStart hook down with exit 1.
3. **A skill that writes its report next to the source file** (R5) may be writing into the tracked tree.
   Check each skill's Step-4 output path against `.gitignore`, not against intent.

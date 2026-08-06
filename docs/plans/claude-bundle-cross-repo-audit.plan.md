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

### P1 — five global-framework rules point at machinery that does not exist here

rent-watch adapted its `CLAUDE-global.md` to its real container; phorj's is still upstream boilerplate.
**Rule 10 is the sharpest: it directly contradicts phorj's own project CLAUDE.md.**

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
lenses were never authored. **Not yet done — needs the lens split ruled (see "Open" below).**

### P2 — smaller divergences

| item | phorj | others | note |
|---|---|---|---|
| `permissions.deny` | **0 entries** | rent-watch has 4 (`Read`/`Edit` on `./.env`, `./.env.*`) | phorj has no `.env`, so low value — but harmless and consistent |
| `PostToolUse` write-time lint hooks | **none** | rent-watch 3, stack 5, pdfturbo 2 | phorj's equivalents (`cargo fmt`, `clippy`) are in git hooks instead — arguably already covered, but not at write time |
| `SubagentStop` reminder | none | stack 1 | stack-specific |
| `qa-sweep` skill | absent | pdfturbo | phorj's CLAUDE.md already lists it as "queued, not yet imported" |
| `THINKING.md` maintenance rule | "run `wc -l ~/.claude/THINKING.md`" | "edit the REPO copy — `cp -u` makes a hand-edit permanently newer and diverges **silently and unrecoverably**" | a real trap; port the wording |

All 13 core skills are present and identical in name across phorj/rent-watch; phorj is not missing any.

## Open — needs a ruling

1. **The two missing panel agents.** What are phorj's other two lenses? The natural split, given the
   invariants: a **safety/promises** reviewer (byte-identity spine claims, LADDER disclosures, the
   `unsafe` island, fault-body parity) and a **completeness** reviewer (the Rule-6 four-dimension
   evidence gate, examples-ship-with-features, LSP+editors currency, SSOT quartet consistency). Both are
   authoring tasks, not copies — the other repos' versions are domain-specific.
2. **Write-time `PostToolUse` hooks for Rust** — worth it, or is the tiered git-hook gate enough?
3. Whether to import `qa-sweep`.

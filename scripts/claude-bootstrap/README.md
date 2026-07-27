# Claude container bootstrap

The phorj remote Claude containers are **ephemeral** — `~/.claude` starts empty every session, while
the project `CLAUDE.md` routes to "the global reasoning framework (`~/.claude/CLAUDE.md`)" and the
DEC-268 certification ladder references its 3C/6C phases. This directory restores that framework
per-session (Invariant 19: only committed state survives).

| File | What | Provenance |
|---|---|---|
| `CLAUDE-global.md` | The 8-phase workflow + core rules + mental models, with a phorj adaptation header. The body is the dev's bundle verbatim **except** the DEC-354 amendments listed in that header | Dev's machine bundle `claude-setup-global-20260722` |
| `THINKING.md` | Thinking-frameworks library (loaded on demand, not at start) | Same bundle, verbatim |
| `BLAST-RADIUS.md` | Pre-flight state checks for destructive/risky commands | Same bundle, verbatim |
| `install.sh` | Idempotent `cp -u` into `~/.claude/` (never clobbers a newer user copy) | New |
| `hooks/precompact-handoff.sh` | **PreCompact hook** — writes a deterministic handoff to `var/claude/handoff/` just before context compaction | Bundle hook, substantially adapted (DEC-354) |
| `hooks/log-helpers.sh` | `log_obs()` structured logging, never fatal (Rule 13) | Bundle, + `mkdir -p` for the log dir |
| `hooks/test-precompact-handoff.sh` | 14-assertion test suite for the hook — run it after any change | New |
| `apply-pending-settings.sh` | Applies a `settings.json.pending` that Claude is classifier-blocked from writing (see below) | New |

Runs automatically via the `SessionStart` hook in `.claude/settings.json`.

## Skills — repo-native, no install

The 12 skills live under `.claude/skills/` and Claude Code reads them in place.

- **Ruled IN by DEC-354** (7 of the bundle's 48, each **adapted** — see each file's adaptation header):
  `/converge` (the DEC-268 ladder, mechanised — its defaults ARE the project tier), `/sweep`
  (Phase 6, plus byte-identity / anti-bandaid / Op-triad / file-size dimensions), `/expanding-context`,
  `/sleuth` (plus mandatory lens **K**, backend divergence), `/inspect`, `/cross-check` (Jira mode
  deleted), `/aggregate-findings`.
- **Predating it:** `/ask-human`, `/gaps`, `/handoff`, `/pre-commit`, `/retrospective`.

### Questions are PLAIN TEXT — `AskUserQuestion` is forbidden here

Developer ruling, 2026-07-27 (recorded under DEC-354). `AskUserQuestion` **silently fails in this
container** — it returned "the user did not answer" four times on 2026-07-26 with the developer at the
keyboard. A question asked that way can vanish with no trace, so it is banned outright. Every question
is: context → a minimal concrete example → numbered options → the recommended option **first** with
its reason → a visible *"none of these / challenge the premise"* escape → **STOP**.
`.claude/skills/ask-human/SKILL.md` is that protocol, and `CLAUDE-global.md`'s question rules were
rewritten to match (upstream mandated the opposite).

## Deliberately NOT imported

Ruled OUT with reasons — the full 199-file audit is
`docs/research/2026-07-25-global-review/J-claude-bundle.md`:

- **session-remember** (20 files) — an LLM-per-session memory pipeline whose store lives under
  `~/.claude/projects/<slug>/memory/`, wiped on container reclaim. It would re-learn and re-forget
  every session, and a second memory beside `SLICE-STATE.md` + the decision register is precisely the
  divergent artifact Invariant 19 forbids.
- **All 57 `mcp/**` files** — corporate service configs incl. four `.env` files, plus
  desktop-automation drivers. **phorj is a public repo**; zero upside.
- **The 4 `ask-human`/gate Stop hooks** — they would double-gate against the container's own Stop
  hooks, and the guard they implement is for a tool that is now banned anyway.
- **`statusline.sh`, `session-start-banner.sh`** — render `~/.claude/run/` sentinels absent here.
- **The `deny` and `ask` permission tiers** — developer-ruled: in a remote container he has no
  terminal, so a `deny` blocks *him* too. Machine-level protections stay in his personal global
  settings, which this repo never touches.
- **The other 41 skills, 34 `bin/` files, 3 refs** — config-portability and memory-pipeline machinery.

## `settings.json` — the hand-over loop

Claude Code's classifier blocks Claude from writing `.claude/settings.json` (it is Claude's own
permission surface), and in a remote container the developer has no terminal. So a settings change
travels through the repo:

1. Claude writes `scripts/claude-bootstrap/settings.json.pending` and commits it.
2. The developer pulls, runs `bash scripts/claude-bootstrap/apply-pending-settings.sh`, reviews the
   diff, commits and pushes.
3. Claude pulls to re-sync. The script **deletes the pending file** on success, so the repo never
   carries two copies of the settings.

Current settings shape (DEC-354): `defaultMode: auto`, an **allow-list only** — no `deny`, no `ask` —
and two hooks: `SessionStart` → `install.sh`, `PreCompact` (both `auto` and `manual` matchers) →
`hooks/precompact-handoff.sh`.

## The PreCompact handoff

`hooks/precompact-handoff.sh` writes `var/claude/handoff/handoff-<stamp>.md` plus a `latest.md`
(gitignored, never committed) immediately before compaction: git state, the uncommitted paths, the
last 5 commits, the **last 8 user messages verbatim**, the last thing Claude said, and the
Invariant-19 pointers to resume from.

It is **deterministic — no LLM call**. The upstream hook shelled out to `claude -p` (Haiku) on every
compaction; here that spends the same weekly quota the developer is rationing and fails whenever the
API is unreachable. Opt in with `PHORJ_HANDOFF_LLM=1` (model via `PHORJ_HANDOFF_MODEL`) to append a
narrative on top of the deterministic note.

Contract: a PreCompact hook must never block compaction, so it **always exits 0** — every failure path
still logs a reason through `log_obs`. Verify with:

```bash
bash scripts/claude-bootstrap/hooks/test-precompact-handoff.sh   # 14 assertions
```

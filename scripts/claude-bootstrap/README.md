# Claude container bootstrap

The phorj remote Claude containers are **ephemeral** — `~/.claude` starts empty every session, while
the project `CLAUDE.md` routes to "the global reasoning framework (`~/.claude/CLAUDE.md`)" and the
DEC-268 certification ladder references its 3C/6C phases. This directory restores that framework
per-session (Invariant 19: only committed state survives).

| File | What | Provenance |
|---|---|---|
| `CLAUDE-global.md` | The 8-phase workflow + core rules + mental models, with a phorj adaptation header (project rules win on conflict; absent machine integrations are optional) | Dev's machine bundle `claude-setup-global-20260722`, verbatim body |
| `THINKING.md` | Thinking-frameworks library (loaded on demand, not at start) | Same bundle, verbatim |
| `BLAST-RADIUS.md` | Pre-flight state checks for destructive/risky commands | Same bundle, verbatim |
| `install.sh` | Idempotent `cp -u` into `~/.claude/` (never clobbers a newer user copy) | New |

Runs automatically via the `SessionStart` hook in `.claude/settings.json`. The companion skill subset
(`/ask-human`, `/handoff`, `/pre-commit`, `/gaps`, `/retrospective`) lives repo-native under
`.claude/skills/` and needs no install. Deliberately NOT imported (dev-ruled 2026-07-22): the
session-remember memory pipeline (the repo's SLICE-STATE + decision register replace it here), the
permission lists, and the other ~43 machine-specific skills.

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
| `install.sh` | Unconditional `cp -f` into `~/.claude/` — **the repo is always the truth** (developer ruling 2026-08-06). Idempotent: the same bytes land every session. A file that predates the hook is snapshotted once to `<name>.pre-bootstrap.bak` and never touched again. | New |
| `test-install.sh` | **Guard for the above — run it after any edit** (`bash scripts/claude-bootstrap/test-install.sh`). 18 assertions, no deps. Sabotage-verified: dropping the snapshot guard fails exactly 1, reverting to `cp -u` fails 2, dropping the final `mkdir`'s `\|\| true` fails 1. | New |
| `hooks/precompact-handoff.sh` | **PreCompact hook** — writes a deterministic handoff to `var/claude/handoff/` just before context compaction | Bundle hook, substantially adapted (DEC-354) |
| `hooks/log-helpers.sh` | `log_obs()` structured logging, never fatal (Rule 13) | Bundle, + `mkdir -p` for the log dir |
| `hooks/test-precompact-handoff.sh` | 34-assertion test suite for the hook — run it after any change | New |
| `apply-pending-settings.sh` | Applies a `settings.json.pending` when Claude cannot write `.claude/settings.json` directly (see below — as of 2026-08-06 it CAN, so this is a fallback, not the normal path) | New |

Two more live in `.claude/hooks/` rather than here, because they are project hooks rather than bundle
files:

| File | What |
|---|---|
| `.claude/hooks/lint-on-write.sh` | **`PostToolUse(Edit\|Write)` advisory, warn-only, always exit 0.** `rustfmt --check` on `.rs`, `phg format --check` on `.phg`, and an **Invariant 13 size advisory** (soft 300 / hard 500, and grandfathered-must-not-GROW). Blocking is forbidden — see `CLAUDE.md` § "the `deny` list stays EMPTY" |
| `.claude/hooks/test-lint-on-write.sh` | 18-assertion guard for it. Sabotage-verified; it pins "exit 0 even when a sub-tool errors", which a `set -e` sabotage slipped past until that assertion existed |

Two companion scripts live one level up in `scripts/`, written **instead of** importing bundle skills
whose machinery does not exist here (DEC-388):

| Script | What | Why native |
|---|---|---|
| `scripts/disk-reclaim.sh` | Frees rebuildable build artefacts. Dry run by default; `--tier=cache\|debug\|all`; `--yes` to apply. **Never touches `var/phorj-app`** (DEC-259). 19-assertion suite in `scripts/test-disk-reclaim.sh` | The bundle's `/cleanup` prunes Claude state; the real crisis here was `target/` at 22 GB on an 88%-full disk, which had already produced *spurious build reds* |
| `scripts/validate-infra.sh` | `bash -n` over every tracked shell file + the git hooks, YAML parse over workflows, JSON parse over tracked JSON. Emits the Rule-7 Coverage table. **Wired into `pre-push`.** 18-assertion suite in `scripts/test-validate-infra.sh` | The bundle's `/validate-infra` is 212 lines of compose/hadolint/yamllint — and this repo has 0 Dockerfiles, 0 compose files, and none of those three tools on PATH |

Runs automatically via the `SessionStart` hook in `.claude/settings.json`.

## Skills — repo-native, no install

The skills live under `.claude/skills/` and Claude Code reads them in place. **`ls .claude/skills/` is
the authoritative list — never restate a count in prose.** (This line said "The 13 skills" while a 14th
was being added in the same change, which is exactly the drift the rule is for.)

- **Ruled IN by DEC-354** (7 of the bundle's 48, each **adapted** — see each file's adaptation header):
  `/converge` (the DEC-268 ladder, mechanised — its defaults ARE the project tier), `/sweep`
  (Phase 6, plus byte-identity / anti-bandaid / Op-triad / file-size dimensions), `/expanding-context`,
  `/sleuth` (plus mandatory lens **K**, backend divergence), `/inspect`, `/cross-check` (Jira mode
  deleted), `/aggregate-findings`.
- **Added by DEC-388, reversing DEC-354's drop:** `/forge`. It was dropped as "infra-shaped"; it is
  architecture-shaped, and its **Chesterton's Fence** gate (challenge nothing that has a recorded WHY;
  drop any finding that cannot name a principle + alternative + both costs) is *precise* here rather
  than noisy, because phorj has the WHY corpus it looks for — 221 register rows, 18 frozen specs, a
  210-line INVARIANTS. Adapted: `--quick` is the default tier, four mandatory Invariant lenses.
- **Predating it:** `/ask-human`, `/gaps`, `/handoff`, `/pre-commit`, `/retrospective`.

### Agent definitions

**All three lenses of DEC-268's mandated panel now EXIST** (DEC-450, 2026-08-06). Until then only the
first did, so the mandated panel was *structurally impossible* and every 3C/6C gate fell through to the
self-graded rung — disclosed each time as "advisor() unavailable", which was true but not the whole
cause. Spawn all three in ONE message so they run concurrently on independent contexts.

| Lens | Agent |
|---|---|
| correctness + regression | `.claude/agents/backend-parity-reviewer.md` — the triple-spine attack surface: coverage-first, the `Op` triad, single-sourced kernels, reified operands, the CTy trap, scratch slots, sugar expansion, transpile-AND-lift currency, the PHP-8.5 floor |
| security + safety-promises | `.claude/agents/safety-promises-reviewer.md` — the `unsafe` island in `src/jit/`, Invariant-14 LADDER exclusions and their disclosures, determinism and the network boundary, EV-7 no-crash, the narrow security surfaces, and the honesty promises |
| completeness + blast-radius | `.claude/agents/completeness-reviewer.md` — tests EXECUTED not merely written, Rule 6's four dimensions with the blast-radius grep re-run independently, Invariant 9 examples-ship-with-features, Invariant 17's 100% RULE, Invariant 19 SSOT consistency |

All three carry **"do not invent a subject"** (the *host* of a claim must be real; an alleged absence
obviously is not) and **"verify a NEGATIVE with a control"** (a probe that cannot fail is worse than no
probe) — added 2026-08-06 from two documented false-green incidents in this repo.

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
- **The `deny` and `ask` permission tiers** — developer-ruled, and **re-ruled permanently on
  2026-08-06**: in a web/remote session he has no terminal, so a denied command is not a speed bump but
  an unrecoverable dead end that strands the session. Canonical statement: `CLAUDE.md` § "Claude config
  in this repo — the `deny` list stays EMPTY". Consequence: every `PostToolUse` hook here is warn-only,
  because one that blocks a write is a `deny` by another name. Machine-level protections stay in his
  personal global settings, which this repo never touches.
- **`/recent`** — **obsolete**, not merely unwanted: the PreCompact hook already emits git state,
  uncommitted paths and recent commits automatically.
- **`/skill-audit`** — its "10+ skills" precondition is now met (13), but it audits *skills*; the audit
  itself called that value circular.
- ~~**`/qa-sweep`**~~ — **BUILT 2026-08-06 (DEC-451 §6), and WRITTEN rather than imported.** pdfturbo's
  version drives a browser over a PDF editor with axe-core and CSP workarounds; none of that has an
  analogue here, so it was rebuilt around phorj's own surfaces: 10 journeys over the shipped binary, the
  LSP over real stdio JSON-RPC, the package-manager lifecycle, transpile↔lift round-trip, the editors,
  the playground, and the gate scripts. The browser half stays out (Playwright MCP is ruled OUT); the
  playground journey uses screenshots per the Completion Gate's visual-evidence clause.
- **`/mega-analysis` and a phorj-native `/full-review`** — deferred; `/aggregate-findings` already does
  the synthesis half, and fanning out 8 review skills is the most expensive thing available.
- **The other 39 skills, 34 `bin/` files, 3 refs** — config-portability and memory-pipeline machinery.

## `settings.json` — the hand-over loop

**Corrected 2026-08-06:** Claude *can* write `.claude/settings.json` in this container — the
`PostToolUse` hook of DEC-451 §8 was added directly, no hand-over needed. The claim below that the
classifier blocks it was true when written and is no longer; it is kept because the hand-over loop is
still the correct fallback the moment a write IS refused, and in a remote container the developer has
no terminal to run the command in. If a settings write is blocked, the change travels through the repo:

1. Claude writes `scripts/claude-bootstrap/settings.json.pending` and commits it.
2. The developer pulls, runs `bash scripts/claude-bootstrap/apply-pending-settings.sh`, reviews the
   diff, commits and pushes.
3. Claude pulls to re-sync. The script **deletes the pending file** on success, so the repo never
   carries two copies of the settings.

Current settings shape: `defaultMode: auto`, an **allow-list only** — no `deny`, no `ask`, and per the
2026-08-06 ruling there never will be — and three hooks: `SessionStart` → `install.sh`, `PreCompact`
(both `auto` and `manual` matchers) → `hooks/precompact-handoff.sh`, and `PostToolUse(Edit|Write)` →
`.claude/hooks/lint-on-write.sh`.

## The PreCompact handoff

`hooks/precompact-handoff.sh` writes `var/claude/handoff/handoff-<stamp>.md` plus a `latest.md`
(gitignored, never committed) immediately before compaction: git state, the uncommitted paths, the
last 5 commits, the **last 8 user messages verbatim**, the last thing Claude said, and the
Invariant-19 pointers to resume from.

It is **deterministic — no LLM call**. The upstream hook shelled out to `claude -p` (Haiku) on every
compaction; here that spends the same weekly quota the developer is rationing and fails whenever the
API is unreachable. Opt in with `PHORJ_HANDOFF_LLM=1` (model via `PHORJ_HANDOFF_MODEL`) to append a
narrative on top of the deterministic note. `PHORJ_HANDOFF_DIR` overrides the output directory
(default: `<cwd>/var/claude/handoff`).

Contract: a PreCompact hook must never block compaction, so it **always exits 0** — every failure path
still logs a reason through `log_obs`. Verify with:

```bash
bash scripts/claude-bootstrap/hooks/test-precompact-handoff.sh   # 34 assertions
bash scripts/claude-bootstrap/test-install.sh                    # 18 assertions
bash .claude/hooks/test-lint-on-write.sh                         # 18 assertions
```

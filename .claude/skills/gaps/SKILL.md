---
name: gaps
spotlight: true
description: Use when hunting for incomplete implementations, missing features, unfulfilled promises, stubs, TODO markers, partial feature flags, or undocumented capabilities across a project.
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

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /gaps — Use when hunting for incomplete implementations, missing features, unfulfilled promises, stubs, TODO markers, partial feature flags, or undocumented capabilities across a project.
>
> Flags:
>   --quick                        Run agents A, F, H only (~3 min, debt markers/test gaps/error handling)
>   --focus=<A|B|C|D|E|F|G|H|I|J> Run a single detection agent
>   --target=<path>                Analyze a specific directory
>   --scope=project|global|both is REMOVED here — see the adaptation header, delta 4.
>   --priority=high                Report Now items only — skip Soon/Later
> ```

---

# /gaps — Incompleteness & Missing Feature Detector

Hunt for incomplete implementations, missing features, unfulfilled promises, and pending work across the entire project. Produces a prioritized roadmap. **Never auto-applies — presents a ranked plan and waits for explicit direction.**

Differentiation from `/inspect`: `/inspect` finds *what's wrong with existing things*. `/gaps` finds *what's missing or unfinished* — features described but not implemented, code started but not completed, documentation that promises things the code doesn't deliver.

Use `--quick` (agents A, F, H only — debt markers, test gaps, error handling; ~3 min), `--focus=<A|B|C|D|E|F|G|H|I|J>` (single agent), `--target=<path>` (analyze a specific directory), `--priority=high` (Now items only — skip Soon/Later).

---

## Step 0: Setup

```bash
# There is no --scope here (adaptation delta 4): ONE pass over $TARGET. `~/.claude/` in this
# container is GENERATED from repo files by scripts/claude-bootstrap/install.sh, which overwrites it
# unconditionally every SessionStart — so auditing it audits a copy, and a finding against it is
# fixed in scripts/claude-bootstrap/ or not at all.
TARGET="${target_arg:-${CLAUDE_PROJECT_DIR:-$PWD}}"
if [[ -z "${target_arg:-}" ]]; then
fi
PROJECT_SLUG=$(echo "$TARGET" | sed 's|^/|-|; s|/|-|g')
GAPS_DIR="$HOME/.claude/projects/$PROJECT_SLUG/gaps"
mkdir -p "$GAPS_DIR"
TODAY=$(date +%Y-%m-%d-%H%M)
REPORT_PATH="${output_arg:-$GAPS_DIR/$TODAY.md}"
PRIOR_GAPS=$(ls "$GAPS_DIR"/*.md 2>/dev/null | sort -r | head -1 || true)
```

Announce: "Scanning gaps: `$TARGET` → saving to `$REPORT_PATH`"

If a prior `/gaps` run exists: note its date. Agents will flag items that have been pending since the prior run as [STALE], helping prioritize chronic incompleteness over fresh debt.

**No `--scope` handling** (adaptation delta 4): a single pass over `$TARGET`. If a caller passes
`--scope=global` or `--scope=both`, say plainly that the flag was removed for this repo and why, then
run the project pass — matching the wording `forge`, `inspect` and `sleuth` already use.

## Step 1: Detect Project Context

```bash
ls "$TARGET"/{package.json,Cargo.toml,pyproject.toml,go.mod,Makefile,docker-compose*.yaml,README.md} 2>/dev/null
[[ -f "$TARGET/CLAUDE.md" ]] && head -60 "$TARGET/CLAUDE.md"
git -C "$TARGET" log --oneline -10 2>/dev/null || true
find "$TARGET" -maxdepth 2 -name "*.md" -o -name "*.sh" -o -name "*.ts" -o -name "*.py" 2>/dev/null | head -20
```

Summarize: tech stack, approximate project age (from git log), primary language, team size (from commit authors). Pass as `PROJECT_CONTEXT` to each agent.

## Step 2: Spawn Gap-Detection Agents

Respect flags:
- `--quick`: spawn only agents A, F, H (debt markers, test gaps, error handling)
- `--priority=high`: instruct agents to report Now-priority items only
- `--focus=<X>`: spawn only that agent
- Default: spawn in two sequential batches — **never exceed 5 concurrent LLM agents** (5 is the proven rate-limit ceiling; >5 causes ~50% failures):
  - **Batch 1**: spawn agents A–E in one message; wait for all 5 to complete before continuing
  - **Batch 2**: spawn agents F–J in one message; wait for all 5 to complete

**Agent A: Explicit Debt Markers** — TODO, FIXME, HACK, XXX, WORKAROUND, BUG, KLUDGE comments; classified by age and actionability.

**Agent B: Stubs & Placeholder Detection** — empty function bodies, `raise NotImplementedError`, hardcoded placeholder returns, shell scripts with TODO bodies.

**Agent C: Partial Feature Implementations** — unhandled switch cases, parsed-but-unused CLI flags, stub API handlers, features with empty branches, state machines with missing transitions.

**Agent D: Undocumented Features (code exists, docs absent)** — commands not in CLAUDE.md, env vars not in .env.example, Makefile targets not in README, hook scripts not in docs.

**Agent E: Promised Features (docs mention, code missing)** — commands in CLAUDE.md with no file, env vars documented but never read, workflows referencing scripts that don't exist.

**Agent F: Missing Tests for Named Features** — named features with zero tests, error paths with no test, workflows with no integration test.

**Agent G: Config & Environment Gaps** — env vars used but not in .env.example, required config with no startup validation, placeholder values with no format hint.

**Agent H: Missing Error Handling Paths** — happy path without error path, silent switch/if fall-throughs, cleanup that runs on success but not failure.

**Agent I: Template & Placeholder Markers** — `<!-- ADAPT: -->` markers, unactivated `.sh.template` files, `{{VAR}}` placeholders, skeleton banners still present.

**Agent J: Integration & Dependency Stubs** — interfaces with no concrete implementation, abstract base classes never subclassed, plugin systems with no plugins registered, unused imports, Makefile targets calling non-existent scripts.

---

## Step 3: Synthesize Gaps Report

```markdown
# /gaps Report — <DATE>
Scanned: <DATE> | Project: <TARGET> | Stack: <PROJECT_CONTEXT>

## Executive Summary
[3-5 sentences: dominant type of incompleteness, most actionable gap, overall completeness feel]

## Priority Roadmap

### NOW — Act immediately (blocking or high-impact)
| # | Category | Gap | Location | Effort |

### SOON — Important but not blocking
| # | Category | Gap | Location | Effort |

### LATER — Nice to have
| # | Category | Gap | Location | Effort |

## Findings by Category
[A through J sections]

## Stale Gaps [CHRONIC] *(only if prior run exists)*
Items present in prior run that are still unfilled.

## Quick Wins (Effort=Quick, Priority=Now or Soon)
| # | Category | Gap | Next action |
```

## Step 4: Save Report

Write the synthesized report to `$REPORT_PATH`.

## Step 4b: Self-Reflection

Spawn ONE agent to reflect on this command's own definition using the just-saved report as evidence. Pass the actual `$REPORT_PATH` value. The agent produces its blind spots, prompt drift, and proposed changes sections, then reads `$REPORT_PATH` and writes the complete updated file (original content + its block) using the Write tool. Returns only: "Self-reflection appended." Parent announces: "Self-reflection complete — see `$REPORT_PATH`"

## Step 5: Present Roadmap — Hard Stop

Show: Executive summary, full NOW table, Quick Wins table, counts of SOON/LATER.

**Ask in PLAIN TEXT** (per `/ask-human` — `AskUserQuestion` is forbidden here) and STOP:

```
N gaps found (Now: X | Soon: Y | Later: Z). Nothing has been changed — every finding above is
a proposal.

1. Fix specific gaps (recommended) — name the IDs, e.g. `G1, G3`.
2. Show all Soon items.
3. Show one category in full — name it, e.g. `category B`.
4. Nothing — close the report.
```

*Never auto-fills anything. The user decides what to close.*

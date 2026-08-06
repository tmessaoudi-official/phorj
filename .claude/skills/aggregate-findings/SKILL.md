---
name: aggregate-findings
spotlight: true
description: Cross-stage synthesis of review reports — deduplicates findings that appear across /inspect, /sleuth, /gaps, /sweep and /inspect --vision runs. Produces one prioritized master list with cross-references instead of N separate reports. Use after running two or more of those skills.
user-invocable: true
args: "[--run=N] [--project=slug] [--top=N]"
side-effects: Writes a consolidated report to var/claude/reports/aggregate-<date>.md (gitignored; never committed)
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
> /aggregate-findings — Cross-stage synthesis of review reports — deduplicates findings across /inspect, /sleuth, /gaps, /sweep and vision runs into one prioritized master list.
> ```
>
> Then output the complete flag table from the **"Flags"** section below. Then STOP.

---

# /aggregate-findings

## When to use
Run after **two or more** of `/inspect`, `/sleuth`, `/gaps`, `/sweep`, `/inspect --vision` have produced reports, to synthesize them into one deduplicated, prioritized master list. (`/mega-analysis` was NOT imported — DEC-354 — so there is no umbrella run to key off: the stage set is simply whatever reports exist under `var/claude/`.)

## Flags
- `--top=N` — show only the top N unique findings (default: all)
- `--since=<date>` — only aggregate reports dated on/after this (default: the most recent report per skill)

## Step 0 — Locate reports

```bash
# Reports live in the repo under var/ (gitignored) — see the adaptation header.
REPO_ROOT="${CLAUDE_PROJECT_DIR:-$PWD}"
REPORT_ROOT="$REPO_ROOT/var/claude"
mkdir -p "$REPORT_ROOT/reports"
# Enumerate what actually exists — this list IS the stage set:
find "$REPORT_ROOT" -name '*.md' -not -path '*/reports/*' | sort
```

Enumerate every report found and state the count before reading — an unlisted report is a coverage gap.

## Step 1 — Read all stage reports (parallel, max 5 at a time)

Read every report enumerated in Step 0, in batches of ≤5 files (the project's concurrency ceiling for LLM-backed agents is 5). Typical stage set here: `/inspect`, `/inspect --vision`, `/sleuth`, `/gaps`, `/sweep`. There is no global-scope pass — those flags were removed on import.

Read each file and pass to Step 2.

## Step 2 — Spawn 3 synthesis agents (parallel)

Spawn exactly 3 agents with the full report content:

### Agent 1: Deduplication detector
Prompt: "You are given N stage reports from this project's review skills. Your job is to identify findings that appear in 2 or more stage reports — these are the highest-confidence issues. For each cross-stage finding, list: the finding name/ID, which stages mention it, what each stage says (noting any contradictions), and a deduplicated one-sentence summary. Output as a markdown table. Only report findings that appear in ≥2 stages."

### Agent 2: Priority ranker
Prompt: "You are given N stage reports from this project's review skills. Your job is to produce a single master priority list of ALL unique findings (not cross-stage-only), ranked by: (1) severity (P0/High before P1/Med), (2) fix cost (Quick before Long), (3) breadth of impact. Remove exact duplicates. Format: numbered list with severity badge, one-line description, estimated fix time, and which stage it came from. Cap at 50 entries."

### Agent 3: Quick wins extractor
Prompt: "You are given N stage reports from this project's review skills. Your job is to extract all 'quick win' findings: severity P1 or higher AND fix cost ≤30 min. These are the highest-value, lowest-effort items. Output as a table: finding, stage, exact file/line, exact fix, minutes. Include at most 20 rows; rank by impact."

## Step 3 — Synthesize into consolidated report

Combine all 3 agent outputs into:

```markdown
# Aggregate Findings — <date>
Generated: <timestamp> | Stages: <N, named> | Raw findings: ~<N> | Unique after dedup: ~<N>

## Top 10 Cross-Stage Findings (appear in ≥2 stages — highest confidence)
[Agent 1 table]

## Quick Wins (P1+ / ≤30 min fix)
[Agent 3 table]

## Master Priority List (all unique findings, ranked)
[Agent 2 list]
```

## Step 4 — Save and report

Save to `var/claude/reports/aggregate-<date>.md` (gitignored — never `git add` it).

Report to user:
- Total unique findings
- Cross-stage duplicates found and collapsed
- Top 5 quick wins
- Suggest: "Run `/aggregate-findings --top=10` to see just the highest priority items"
- Name any report that existed but could not be parsed — a silently dropped stage is a coverage lie

## Self-reflection
After saving, note any findings where the stages disagreed (e.g., one stage calls it P0, another calls it P2). Flag these as "conflicting severity" in the report.

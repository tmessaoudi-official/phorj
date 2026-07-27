---
name: inspect
spotlight: true
description: Use when performing a full project health inspection across security, dead code, deprecations, error handling, documentation, tests, configuration, code quality, performance, and tech debt. Use when you need a structured report with P0–P3 severity rankings. Add --vision for improvement proposals.
user-invocable: true
---

<!-- ═══════════════════════════════════════════════════════════════════════════════════
  phorj CONTAINER ADAPTATION (DEC-354, built 2026-07-27). Imported from the developer's machine
  bundle `claude-setup-global-20260722`; DEC-354 ruled 7 of its 48 skills IN, each ADAPTED.
  These deltas OVERRIDE the body below wherever they conflict:

  1. QUESTIONS ARE PLAIN TEXT. `AskUserQuestion` silently fails in this container (observed 4x on
     2026-07-26), so a gate that "asks" cannot fire. Every "invoke ask-human" below means: print the
     question, its options and the recommendation as ordinary prose, then STOP and wait for a reply.
  2. NO `advisor()` HERE. Independent certification = fresh-context read-only reviewer subagents
     (project CLAUDE.md, DEC-268 MAXIMAL ladder). Self-grading is the last resort and must be
     DISCLOSED as self-graded in the output.
  3. REPORTS GO TO `var/claude/…` in the repo — gitignored (`/var`), survives compaction inside the
     session, never committed. NOT `~/.claude/projects/…`: that is wiped when the container is
     reclaimed, so a report written there is lost (Invariant 19 — only committed repo state survives).
  4. PROJECT RULES WIN on any conflict: `/home/user/phorj/CLAUDE.md` — the invariants, the full
     correctness gate, the git-autonomy override, Invariant 19's canonical plan/decision homes.
═══════════════════════════════════════════════════════════════════════════════════════════════ -->

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /inspect — Perform a full project health inspection across security, dead code, deprecations, error handling, documentation, tests, configuration, code quality, performance, and tech debt. Produces a P0–P3 severity-ranked report.
> ```
>
> Then output the complete flag table from the **"Step 0: Setup"** section below. Then STOP.

| Flag | Value | Default | Description |
|------|-------|---------|-------------|
| `--quick` | — | off | Security + deprecations + error handling only (agents A, C, D) |
| `--focus=<X>` | A–J | — | Run a single lens agent only (e.g. `--focus=A`) |
| `--target=<path>` | path | `$CLAUDE_PROJECT_DIR` | Analyze a specific directory |
| `--output=<file>` | path | auto timestamped | Explicit report file path |
| `--vision` | — | off | After health agents complete, spawn 10 vision proposal agents (VA–VJ) and append proposals |

---

# /inspect — Project Health Inspector

Explore the project and surface issues, tech debt, and improvement opportunities. **Never auto-applies anything — this command only reads and reports.**

Use `--quick` (security + deprecations + error handling only), `--focus=<A|B|C|D|E|F|G|H|I|J>` (single lens), `--target=<path>` (analyze a specific directory), `--output=<file>` (explicit report path), `--vision` (after health agents complete, also spawn 10 vision proposal agents VA–VJ and append proposals to the same report). **`--scope=global|both` is REMOVED here** (adaptation): `~/.claude/` in this container is generated from repo files by `scripts/claude-bootstrap/install.sh`, so auditing it audits a copy.

---

## Step 0: Setup

```bash
# --target picks the directory to inspect; there is no --scope here (see adaptation above).
TARGET="${target_arg:-${CLAUDE_PROJECT_DIR:-$PWD}}"
# Reports live in the REPO under var/ (gitignored, survives compaction) — never in ~/.claude,
# which is wiped when the container is reclaimed. Never commit them.
REPO_ROOT="${CLAUDE_PROJECT_DIR:-$PWD}"
INSPECT_DIR="$REPO_ROOT/var/claude/inspections"
mkdir -p "$INSPECT_DIR"
TODAY=$(date +%Y-%m-%d-%H%M)
REPORT_PATH="${output_arg:-$INSPECT_DIR/$TODAY.md}"
PRIOR_REPORT=$(ls "$INSPECT_DIR"/*.md 2>/dev/null | sort -r | head -1 || true)
```

Announce: "Inspecting project: `$TARGET` → saving to `$REPORT_PATH`"
If `$PRIOR_REPORT` is non-empty, note its date for drift comparison.

**No `--scope` handling** (adaptation): a single pass over `$TARGET`. If a caller passes `--scope=global` or `--scope=both`, say plainly that the flag was removed for this repo and why, then run the project pass.

```bash
# Vision mode setup (only when --vision flag is set)
if [[ "${vision_flag:-}" == "true" ]]; then
  VISION_DIR="$HOME/.claude/projects/$PROJECT_SLUG/visions"
  mkdir -p "$VISION_DIR"
  PRIOR_VISION=$(ls "$VISION_DIR"/*.md 2>/dev/null | sort -r | head -1 || true)
fi
```

## Step 1: Detect Project Context

Before spawning agents, read the project to understand what you're analyzing:

```bash
# Detect tech stack signals
ls "$TARGET"/{package.json,Cargo.toml,pyproject.toml,go.mod,pom.xml,Gemfile,composer.json,Makefile,docker-compose*.yaml,*.sh} 2>/dev/null
[[ -f "$TARGET/CLAUDE.md" ]] && head -60 "$TARGET/CLAUDE.md"
[[ -f "$TARGET/.claude/settings.json" ]] && cat "$TARGET/.claude/settings.json"
```

Summarize the tech stack in one sentence (e.g., "Bash/Docker/Make project — shell scripts, Dockerfiles, YAML configs"). Pass this to each agent as `PROJECT_TYPE`.

## Step 2: Spawn Analysis Agents

Respect flags before spawning:
- `--quick`: spawn only A (Security), C (Deprecations), D (Error Handling)
- `--focus=<X>`: spawn only that agent
- `--vision`: run all 10 health agents as normal; Step 2b spawns vision agents afterward
- Default: spawn in two sequential batches — **never exceed 5 concurrent LLM agents** (5 is the proven rate-limit ceiling; >5 causes ~50% failures):
  - **Batch 1**: spawn agents A–E in one message; wait for all 5 to complete before continuing
  - **Batch 2**: spawn agents F–J in one message; wait for all 5 to complete

Replace `<TARGET>` with the actual target path. Replace `PROJECT_TYPE` with the detected stack. Replace `CURRENT_DATE` with today's date.

---

**Agent A: Security**

> Analyze `<TARGET>` for security issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) hardcoded secrets, tokens, passwords, API keys, or credentials anywhere in source files — check strings matching patterns like `password=`, `api_key=`, `secret=`, `token=`, `AUTH_`; (2) unsafe shell patterns: `eval`, `$()` with user input, unquoted variables in shell commands that could enable injection; (3) files with world-readable permissions containing sensitive data; (4) URLs using `http://` where `https://` should be enforced; (5) Dockerfiles running as root unnecessarily; (6) any pattern that exposes sensitive data in logs, error messages, or output; (7) missing input validation at system boundaries (user input, external APIs, file paths). For each finding: exact file, line number, issue description, severity (P0=exploit risk, P1=data exposure, P2=bad practice), recommended fix. Research only, no writes.

---

**Agent B: Dead Code & Unused Artifacts**

> Analyze `<TARGET>` for dead code. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) functions defined but never called — grep for function definitions, then grep for each name to confirm it has callers; (2) variables declared but never read — especially readonly arrays/vars that are set but never iterated or referenced; (3) imported modules or sourced files that contribute nothing used; (4) files in the project that are never referenced by any other file, Makefile target, or documented workflow; (5) commented-out code blocks (vs. intentional commenting); (6) configuration keys or env vars defined but never consumed; (7) test files that test non-existent or already-deleted code. For each finding: exact file, approximate line, why it appears dead (no callers/no readers found), confidence level. Note: a function may be called dynamically — flag these as "possibly dead" not "dead". Research only, no writes.

---

**Agent C: Deprecations & Staleness**

> Analyze `<TARGET>` for deprecated or outdated patterns. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) deprecated CLI flags, API calls, or library functions being used — check documentation/man pages where relevant; (2) package versions pinned to EOL or clearly outdated major versions (check package.json, pyproject.toml, Gemfile, go.mod, Cargo.toml, etc.); (3) deprecated shell constructs (backtick command substitution instead of `$()`, `[ ]` instead of `[[ ]]` in bash, `#!/bin/sh` when bash features are used); (4) Docker base images using deprecated tags (`:latest`, EOL distro versions, deprecated base images); (5) patterns superseded by better alternatives in the same codebase (e.g., old script does X manually while a new utility for X already exists); (6) CLAUDE.md or README sections referencing tools, commands, or files that no longer exist on disk. For each: file + line, what's deprecated, what to use instead. Research only, no writes.

---

**Agent D: Error Handling**

> Analyze `<TARGET>` for error handling gaps. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) shell scripts missing `set -e` or `set -euo pipefail` where failure should abort (check if errors silently continue); (2) commands whose exit codes are not checked — specifically: `cd`, `mkdir`, `cp`, `curl`, `docker`, `git` in scripts where failure would corrupt state; (3) `2>/dev/null` that silently discards errors in critical paths (vs. intentional suppression); (4) error messages that are too generic to diagnose ("Error occurred", "Failed") — no context, no file/line, no cause; (5) missing trap handlers for cleanup on unexpected exit; (6) functions that return meaningless exit codes (always return 0 even on failure); (7) try/catch or error handling code that swallows the exception without logging; (8) timeout-less network calls (`curl` without `--connect-timeout`, Docker pulls that can hang forever). For each: file + line, type of gap, consequence if triggered, fix suggestion. Research only, no writes.

---

**Agent E: Documentation & Comment Staleness**

> Analyze `<TARGET>` for documentation issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) function/script comments that claim parameters, behavior, or side effects that no longer match the actual code — look for `# Args:`, `# Usage:`, `# Returns:` comments and verify them; (2) README or CLAUDE.md sections that reference files, commands, or paths that don't exist; (3) TODO/FIXME/HACK/XXX markers with no ticket number, no date, and no owner — unactionable tech debt; (4) example commands in docs that are broken (wrong paths, deprecated flags, missing required args); (5) public-facing functions or commands with no documentation at all; (6) inline comments that describe WHAT the code does (redundant) vs WHY (missing). Count undocumented public interfaces. For each: file + line, issue type, severity, fix cost. Research only, no writes.

---

**Agent F: Test Coverage Gaps**

> Analyze `<TARGET>` for test coverage issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) locate all test files (*.test.sh, *.spec.ts, *_test.go, test_*.py, etc.) and test directories; (2) for each major source module, check whether a corresponding test file exists; (3) identify business-critical code paths (parsing, validation, data transformation, error handling) that have no tests; (4) find test files that reference functions, files, or commands that no longer exist; (5) find tests that always pass trivially (no assertions, only `true` as the test body); (6) find test infrastructure that would fail to run (missing test runner, broken harness, missing fixtures). If no tests exist at all: flag this prominently. Produce: coverage map (module → has tests?), critical untested paths, broken test infrastructure. Research only, no writes.

---

**Agent G: Configuration & Drift**

> Analyze `<TARGET>` for configuration issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) env vars referenced in code that are not documented in README, .env.example, or CLAUDE.md — undocumented requirements; (2) settings/config values that CLAUDE.md or README says should be X but the actual file shows Y; (3) .env.example or .env files that are missing entries that appear in the code; (4) hard-coded values that should be configurable (ports, paths, timeouts, limits appearing as magic numbers); (5) config files with duplicate keys or conflicting settings; (6) permission settings (e.g., settings.json) that reference files/scripts not present on disk; (7) COMPOSE_FILE or similar aggregation files with entries pointing to non-existent paths. For each: file + line, drift description, impact. Research only, no writes.

---

**Agent H: Code Quality & Consistency**

> Analyze `<TARGET>` for code quality issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) functions longer than 60 lines that should be decomposed; (2) naming inconsistency — same concept named differently in different files (e.g., `get_user` vs `fetch_user` vs `load_user` for the same operation); (3) copy-pasted blocks (3+ consecutive identical or near-identical lines appearing in 2+ places — should be extracted); (4) functions doing multiple unrelated things (violates single responsibility — detectable by AND in the function name or multiple distinct logic phases); (5) circular dependencies or unusual coupling (A calls B calls A, or a "utility" module importing from a "domain" module); (6) magic numbers/strings without named constants; (7) inconsistent error handling patterns across similar functions in the same module. For each: file + line, issue type, fix suggestion. Research only, no writes.

---

**Agent I: Performance Anti-Patterns**

> Analyze `<TARGET>` for obvious performance issues. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) loops that re-read a file or re-run a command on every iteration when the result could be cached before the loop; (2) sequential network calls that could be parallelized (multiple `curl` or `docker pull` calls in sequence); (3) parsing or processing the same file twice in the same script when one pass would do; (4) missing caching for expensive operations that are called repeatedly with the same inputs; (5) large file reads into memory when streaming would work; (6) Docker layers ordered incorrectly (frequently changing layers early, causing unnecessary cache invalidation); (7) `find` or `grep -r` commands in loops; (8) sync operations blocking on a slow external call when async is possible. For each: file + line, bottleneck type, estimated impact, fix suggestion. Research only, no writes.

---

**Agent J: Tech Debt Markers**

> Analyze `<TARGET>` for explicit tech debt. PROJECT_TYPE: PROJECT_TYPE. CURRENT_DATE. Find: (1) every TODO, FIXME, HACK, XXX, WORKAROUND, NOTE:, KLUDGE comment — list file + line + full comment text; (2) commented-out code blocks (more than 3 lines) that are not explanatory — these are usually code someone was afraid to delete; (3) version-gated blocks ("# TODO: remove when X reaches version Y") where that version has now been surpassed; (4) workaround comments that reference external bugs or issues (e.g., "# workaround for docker bug #1234") — check if the upstream issue is resolved; (5) `sleep` calls used as synchronization workarounds instead of proper polling. Produce a ranked list: items where the condition has been met (can act now) vs items still pending. Research only, no writes.

---

## Step 2b: Spawn Vision Agents VA–VJ (only when `--vision` is set)

*Skip this entire step if `--vision` was NOT passed. Proceed directly to Step 3.*

After all Step 2 inspection agents complete, spawn these 10 vision agents in **two sequential batches of 5** (same cap as health agents — never exceed 5 concurrent LLM agents per Rule 4). They focus on improvement proposals, not current issues. Pass `PROJECT_TYPE` and `CURRENT_DATE` (already detected in Step 1).

- **Vision Batch 1**: spawn VA–VE in one message; wait for all 5 to complete
- **Vision Batch 2**: spawn VF–VJ in one message; wait for all 5 to complete

---

**Vision Agent VA: Architecture & Design Patterns**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements, not diagnose current issues. Propose architectural improvements: (1) evaluate the current dominant design pattern — is it the right fit for this project's size and goals? (2) identify missing layers (data access / business logic separation, abstractions over tool calls); (3) find tightly coupled modules where a simple adapter would reduce coupling; (4) would event-driven patterns improve anything? (5) is there a shared abstraction that would unify currently-duplicated patterns? (6) would a plugin architecture add value? For each proposal: title, current state, proposed state, concrete 3-5 step implementation, expected benefit, effort [Quick ≤1h / Medium 1-4h / Long >4h]. Research only, no writes.

---

**Vision Agent VB: Tooling & Automation**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose tooling and automation improvements: (1) repetitive manual steps that could be automated; (2) new slash commands that would save significant time; (3) pre-commit hooks or validations that should run automatically; (4) CI/CD pipeline improvements; (5) developer aliases or shortcuts; (6) a "one-command" setup script if missing; (7) tools with better alternatives; (8) opportunistic automation for operations done > once a week. For each: title, what it replaces or adds, implementation sketch, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VC: Observability & Monitoring**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose observability improvements: (1) structured logging if not present; (2) progress indicators for long-running operations; (3) explicit success/failure markers for operations that could fail silently; (4) a health-check mechanism if missing; (5) a session/run report generated automatically; (6) metrics worth tracking (build times, test durations, latencies); (7) centralized log location and rotation strategy. For each: title, current gap, proposed addition, implementation sketch, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VD: Testing Strategy**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose testing improvements: (1) what test types exist vs. what's missing for this project's risk profile? (2) a target test coverage strategy — which modules MUST have tests? (3) integration tests to add for common failure modes; (4) property-based testing or fuzzing for input-parsing functions; (5) a test naming and organization convention if missing; (6) test fixtures or data factories to reduce duplication; (7) a fast smoke-test suite (< 30s) for quick feedback loops. For each: title, current state, proposed state, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VE: Developer Experience**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose developer experience improvements: (1) reduce "clone → first successful run" time; (2) commonly-needed commands with no shortcut; (3) is `.env.example` complete? (4) a "getting started in 5 minutes" README section if missing; (5) error messages that require reading source to understand; (6) a `make help` or `--help` for discoverability; (7) a Makefile or justfile for multi-step workflows; (8) automated local tooling installation. For each: title, current friction, proposed improvement, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VF: Naming & Consistency**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose naming and consistency improvements: (1) naming conventions in use — check for inconsistency (snake_case vs camelCase vs kebab-case); (2) ambiguous or generic names (handle, process, data, info, result); (3) files whose name doesn't match content; (4) a consistent naming convention if undocumented; (5) abbreviations used inconsistently (cfg vs config vs configuration); (6) prefix/suffix patterns not applied consistently. For each: current name, proposed name, files affected, effort [Quick]. Research only, no writes.

---

**Vision Agent VG: Security Hardening**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose proactive security improvements (not current vulnerabilities — those are for health agents). Propose: (1) trust boundary validation where inputs from external sources lack validation; (2) secrets management improvements (env var → secrets manager, .env.secret files); (3) SBOM for dependency tracking; (4) least-privilege Docker container permissions; (5) automated secret scanning in pre-commit hooks; (6) a security review checklist specific to this project's risk surface. For each: title, current state, proposed improvement, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VH: Performance & Scalability**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements (not diagnose current anti-patterns — those are for health agents). Propose: (1) slowest operations that could be made faster with caching, parallelization, or algorithmic improvement; (2) operations that scale poorly with input size; (3) build/test/compile time improvements; (4) network calls that could be batched or made async; (5) a persistent cache for repeated expensive computations; (6) startup costs that could be deferred. For each: title, current characteristic, proposed improvement, expected speedup, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VI: Knowledge & Documentation**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose knowledge and documentation improvements: (1) "tribal knowledge" items that should be written down (complex workarounds, non-obvious decisions, surprising behavior); (2) missing operational runbooks (how to debug X, reset Y, upgrade Z); (3) Architecture Decision Records (ADRs) for 3-5 past decisions; (4) an onboarding guide for a new developer; (5) a CHANGELOG workflow; (6) "why does this exist?" questions the code doesn't answer; (7) CLAUDE.md or README sections needing expansion. For each: title, knowledge gap, proposed addition, effort [Quick/Medium/Long]. Research only, no writes.

---

**Vision Agent VJ: Infrastructure & Reliability**

> Explore `<TARGET>`. PROJECT_TYPE. CURRENT_DATE. You are a vision agent — propose improvements. Propose infrastructure and reliability improvements: (1) single points of failure — how to degrade gracefully; (2) operations that should be idempotent but aren't; (3) retry logic with exponential backoff for network calls; (4) cleanup/teardown steps missing on error; (5) a "self-healing" mechanism for common failure modes; (6) documented rollback procedures for destructive operations; (7) health checks for long-running processes; (8) proper wait/poll loops replacing sleep-based synchronization. For each: title, current fragility, proposed improvement, effort [Quick/Medium/Long]. Research only, no writes.

---

## Step 3: Synthesize Findings

After all agents complete, synthesize into a structured report:

```markdown
# /inspect Report — <DATE>
Generated: <DATE> | Project: <TARGET> | Stack: <PROJECT_TYPE>
Prior inspection: <PRIOR_DATE or "none">

## Summary
| Category | P0 | P1 | P2 | P3 |
|----------|----|----|----|----|
| A: Security | | | | |
| B: Dead Code | | | | |
| C: Deprecations | | | | |
| D: Error Handling | | | | |
| E: Documentation | | | | |
| F: Tests | | | | |
| G: Configuration | | | | |
| H: Code Quality | | | | |
| I: Performance | | | | |
| J: Tech Debt | | | | |
| **TOTAL** | | | | |

## Findings (P0 and P1 only — reply "details" for P2/P3)

### [P0|P1] — [Title] — Category X
**File**: `path/to/file:line`
**Issue**: [precise description]
**Impact**: [consequence if not fixed]
**Fix**: [concrete fix, 1-3 steps]
**Effort**: [Low <15min | Medium 15-60min | High >1h]
---
[repeat for each P0/P1 finding]

## Drift vs Prior Inspection
### ✓ Resolved since <PRIOR_DATE>
### ⚠ New since <PRIOR_DATE>
### [CHRONIC] Present in 2+ inspections

## Quick Wins (P2/P3, Effort=Low)
[List of easy fixes the user can knock out quickly]

## Top 5 Actions
1. [Title] — [why] | [how] (Est: X)
...

## Vision Proposals (only present when --vision was set)

### Summary Table
| Category | Quick | Medium | Long | Total |
|----------|-------|--------|------|-------|
| VA: Architecture | | | | |
| VB: Tooling | | | | |
| VC: Observability | | | | |
| VD: Testing | | | | |
| VE: DX | | | | |
| VF: Naming | | | | |
| VG: Security | | | | |
| VH: Performance | | | | |
| VI: Knowledge | | | | |
| VJ: Infrastructure | | | | |
| **TOTAL** | | | | |

### Quick Wins — Do These First
[All Quick proposals across VA–VJ, sorted by impact]

### Top 5 Highest-Impact Proposals
1. [Category] **[Title]** — [1 sentence why] (Effort: X)
...

### Roadmap
**This week** (Quick ≤1h): [list]
**This sprint** (Medium 1–4h): [list]
**This quarter** (Long >4h): [list]
```

**Severity guide:**
- **P0**: Exploit risk, data loss, or silent corruption — fix before next commit
- **P1**: Breaks correctness, exposes data, or actively misleads — fix this sprint
- **P2**: Reduces quality, creates future bugs, or wastes time — fix when near
- **P3**: Stylistic, minor, or optional — fix opportunistically

**Chronic rule**: Any finding present in 2+ prior inspections gets bumped to P0/P1 regardless of original severity.

## Step 4: Save Report

Write the synthesized report to `$REPORT_PATH`.
Also write a JSON summary to `$INSPECT_DIR/latest.json`:
```json
{"date": "<DATE>", "target": "<TARGET>", "p0": N, "p1": N, "p2": N, "p3": N, "proposals": N, "report": "<path>"}
```
(`proposals` = total vision proposals count; 0 if `--vision` not set)

Announce: "Report saved to `$REPORT_PATH`"

## Step 4b: Self-Reflection (about this command, not the project)

Spawn ONE agent to reflect on this command's own definition using the just-saved report as evidence:

> Read `.claude/skills/inspect/SKILL.md` in full (repo-native — this skill is committed, not installed). You are auditing this command — not the project it just analyzed. Then read the report at the path provided to you. Based on what was actually found this run, produce ONLY this block:
>
> ---
> ### Command Self-Reflection — `/inspect`
> *Proposals only. Nothing auto-applies — user reviews and accepts or rejects each.*
>
> **Blind Spots** *(findings this run that no current agent prompt, A–J, was designed to catch)*
> For each: finding summary → which agent's stated scope it fell under → why the prompt missed it.
> Write "None detected" if all findings were anticipated by existing prompts.
>
> **Prompt Drift** *(agents that overreached or underdelivered vs. their stated scope)*
> For each: agent letter → what drifted → proposed one-sentence fix.
> Write "None detected" if all agents stayed on scope.
>
> **Missing Coverage** *(project health dimensions with no agent at all in A–J)*
> For each: missing dimension → why it matters → 1-sentence agent prompt sketch.
> Write "None" if all key health dimensions are covered.
>
> **Proposed Changes** *(max 3 — P1=high value / P2=quality improvement / P3=nice-to-have)*
> | # | Priority | Change | Rationale |
> |---|----------|--------|-----------|
> Write "No changes proposed this run." if nothing to add.
> ---
>
> After drafting your block above, append it to the report: read `$REPORT_PATH`, then write the complete updated file (original content + your block) using the Write tool. Return only: "Self-reflection appended — [N] proposals."

Pass the actual `$REPORT_PATH` value to the agent. The agent writes directly to the report — no inline display. Announce: "Self-reflection complete — see `$REPORT_PATH`"

## Step 5: Present Findings

Show in conversation:
- Summary table (all categories)
- All P0 and P1 findings in full
- Quick wins list
- Top 5 Actions
- If `--vision` was set: Vision Quick Wins table + Top 5 Vision Proposals
- **Print as plain text and STOP until answered** (adaptation rule 1): question 'What would you like to see next?', options: 'Show all P2/P3 findings (Recommended)' / 'Show vision proposals for a category — specify in notes' / 'Nothing — close the report'. (Only include the vision option if `--vision` was set.)

*Nothing auto-applied. All findings are proposals.*

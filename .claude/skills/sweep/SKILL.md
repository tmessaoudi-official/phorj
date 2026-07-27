---
name: sweep
spotlight: true
description: Use when running a Phase 6 second sweep on uncommitted changes before committing, or reviewing code written outside the standard agent workflow.
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

## phorj dimensions — MANDATORY additions to this skill's review set

Run these **in addition to** the dimensions below, on every sweep of this repo:

- **Byte-identity spine (Invariant 1).** Does the change keep `phg run` ≡ `phg run --tree-walker` ≡
  the transpiled PHP under a real `php`? A change to one backend, one value kernel, or the transpiler
  that has no differential case shaped like the new behaviour is a **P0** — the differential harness's
  coverage IS `examples/**/*.phg`, so an unexampled feature has ZERO byte-identity coverage.
- **Anti-bandaid gate (project CLAUDE.md Phase 6).** For every `||` fallback, `2>/dev/null`,
  `|| true`, error trap, retry loop, timeout bump or default-value assignment introduced: state the
  exact failure mode, the *physical* evidence that confirmed it (log, measurement, trace, test
  output), and whether the root cause is fixed. No evidence ⇒ **P0**, replace it with a root-cause fix.
- **Exhaustive-match triad (Invariant 3).** A new `Op` variant must extend `vm::exec_op`,
  `BytecodeProgram::validate` and `compiler::stack_effect` in the SAME change; all three are
  wildcard-free — a reintroduced `_` arm is a P0.
- **File-size caps (Invariant 13).** Soft 300 / hard 500 lines per source file; a feature that pushes
  a file past the soft cap should have STARTED by splitting it.

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then STOP — do not execute any other steps.
>
> ```
> /sweep — Run a Phase 6 second sweep on uncommitted changes before committing, or review code written outside the standard agent workflow.
>
> No flags — invoked without arguments.
> ```

---

Run a Phase 6 Second Sweep on current uncommitted changes. **Never auto-applies anything — this command only reads and reports.** Use before committing or to review code written outside the standard agent workflow.

## Steps

1. **Assess the diff**:
   - `git diff --stat` — change footprint (files changed, lines added/removed)
   - `git diff` — full diff
   - `git diff --cached --stat` + `git diff --cached` — staged changes too

2. **Review each changed file** using the Phase 6 checklist:

   **All files**:
   - **Bug hunt**: logic errors, off-by-one, null/nil/undefined deref, unchecked error returns, unhandled edge cases
   - **Security**: credentials/secrets in code, injection risks (SQL, shell, template), missing input validation at system boundaries
   - **Contracts**: changed function signatures, changed CLI flags, changed API response shapes, changed config keys — flag every one as a potential breaking change
   - **Tests**: new behavior without a test? Modified behavior without updated tests?
   - **Docs**: changed public interface without updated documentation?

   **Shell scripts** (`.sh`):
   - Missing `set -euo pipefail` or equivalent
   - Unquoted variable expansions (`$VAR` instead of `"$VAR"`)
   - Missing error handling after commands that can fail silently
   - `rm -rf` on an unvalidated or unquoted path

   **Config / infra files** (`.yaml`, `.yml`, `Dockerfile`, `.env`):
   - Secrets or credentials committed directly
   - `ARG` without matching `ENV` if runtime access needed
   - Trailing `;` in list vars that would be silently swallowed

3. **Classify each finding** by severity:
   - **CRITICAL**: security hole, data loss risk, broken API contract, shell injection, unhandled error that will crash in production
   - **WARNING**: missing test, logic edge case, performance regression, missing error handling, unquoted variable
   - **NOTE**: style, naming, non-blocking improvement

4. **Output a structured findings table**:

```
## Sweep Results

| # | Severity | File:Line | Finding | Fix |
|---|----------|-----------|---------|-----|
| 1 | CRITICAL  | bin/deploy.sh:42      | Unquoted $DIR in rm -rf     | Quote: rm -rf "$DIR" |
| 2 | WARNING   | src/parser.sh:118     | Missing exit-code check     | Check return value of curl |
| 3 | NOTE      | src/checker/calls/ufcs.rs:41 | Unused binding        | Remove or document |

**Verdict**: PASS (safe to commit) or BLOCKED (N critical findings must be fixed first)
```

5. **Save the report**: Write findings to a timestamped file so they survive the session:

```bash
PROJECT_SLUG=$(echo "${CLAUDE_PROJECT_DIR:-$PWD}" | sed 's|^/|-|; s|/|-|g')
SWEEP_DIR="$HOME/.claude/projects/$PROJECT_SLUG/sweeps"
mkdir -p "$SWEEP_DIR"
SWEEP_PATH="$SWEEP_DIR/$(date +%Y-%m-%d-%H%M%S).md"
```

Write the full findings table (including verdict) to `$SWEEP_PATH`. Announce: "Sweep report saved to `$SWEEP_PATH`"

## Notes

- A single CRITICAL finding means verdict is BLOCKED
- Multiple WARNINGs with no CRITICAL = PASS with notes (your discretion)
- Apply **Kernighan's Law**: if the diff is hard to understand, that itself is a WARNING (complexity)
- Apply **Chesterton's Fence**: before flagging a removal as wrong, understand why the code existed (`git blame`, commit message)
- Apply **Hyrum's Law**: any changed public interface (CLI flag, function signature, config key, command output format) is a potential contract break — flag it

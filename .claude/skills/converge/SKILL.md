---
name: converge
spotlight: true
description: Run the project's DEC-268 MAXIMAL certification ladder, or a deeper tunable convergence sweep, over an audit/migration/gate. Defaults ARE the phorj ladder — 3 adversarial evidence-based lenses, TWO consecutive fully-clean rounds, cap 5 rounds, certified by fresh-context reviewer subagents. Override with --cycles/--converge/--angles/--certify. Asks for approval in PLAIN TEXT before starting and reports progress every cycle. --auto runs autonomously to convergence or a hard cap.
user-invocable: true
args: "[--cycles=N] [--converge=K] [--scope=dec268|3C|6C|custom] [--angles='angle1;angle2;angle3'] [--certify=reviewer|self] [--auto] [--auto-cap=N]"
side-effects: None — read-only analysis loop; findings incorporated into conversation context only.
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
> /converge — Run the project's DEC-268 MAXIMAL certification ladder (3 adversarial evidence-based lenses, TWO consecutive clean rounds, cap 5, fresh-context reviewer subagents), or a deeper tunable convergence sweep. Every parameter is overridable. Asks for approval in PLAIN TEXT and reports progress every cycle. --auto runs to convergence or a hard cap.
> ```
>
> Then output the complete flag table from the **"Flags"** section below. Then STOP.

---

# /converge — Convergence Loop

Runs a structured multi-angle convergence loop with explicit user approval of parameters before any cycle executes. Reports progress after every cycle. In autonomous mode, runs silently to convergence or cap — inline output is always shown, only `ask-human` pauses are suppressed.

**Relationship to the project's Phase 3C/6C gates.** Project `CLAUDE.md` (DEC-268) mandates a 3-lens reviewer panel with two consecutive clean rounds at **every** 3C and 6C gate, all task sizes — and today that ladder is hand-rolled from memory each time. Running `/converge` with its defaults **IS** that gate, executed mechanically instead of remembered. Reach for the flags when you want more than the mandated tier: a wider lens set, a higher clean-round threshold, or an enumerated custom scope for a large audit or migration.

## Flags

- `--cycles=N` — maximum total cycles before escalating (default: **5** — DEC-268's cap)
- `--converge=K` — consecutive fully-clean cycles required to declare convergence (default: **2** — DEC-268's *two consecutive fully-clean rounds*; any finding resets the counter)
- `--scope=dec268|3C|6C|custom` — which lens set to use (default: **`dec268`**). The `3C`/`6C` names describe the angle *content* (expanding-context / adversarial / blast-radius) and are kept for continuity; `dec268` is the project-mandated panel — running it here IS the 3C/6C gate, performed rather than remembered.
  - `dec268` (**default — the project's ratified ladder**): the 3-lens reviewer PANEL, each lens adversarial and **evidence-based** (the reviewer reads the actual diff/tests/specs itself, never the author's narrative): (1) **correctness + regression**, (2) **security + safety-promises**, (3) **completeness + blast-radius**. This is the tier project `CLAUDE.md` mandates at every 3C/6C gate, all task sizes.
  - `3C`: pre-implementation-style angles (expanding-context, adversarial, blast-radius)
  - `6C`: pre-completion-style angles (expanding-context on result, failure modes, callers/docs)
  - `custom`: angles provided via `--angles`
- `--angles='A;B;C'` — semicolon-separated angle descriptions when `--scope=custom`; for custom scope, at least one angle **must** be prefixed with `enumerate:` (e.g. `enumerate:list all image dirs`). See Angle Requirements below.
- `--certify=reviewer|self` — how a cycle's findings get judged (default: **`reviewer`**)
  - `reviewer` (**default**): each lens is run by a **fresh-context read-only reviewer subagent** that reads the artefacts itself. `advisor()` does not exist in this environment, so this IS the top of DEC-268's availability chain here. Convergence still requires `--converge=K` (2) consecutive fully-clean rounds — independence removes the self-grading blind spot, it does not remove the project's two-round requirement.
  - `self`: self-graded CLEAN/RESET/STUCK comparison against the previous cycle. Last resort — a restricted subagent context with no ability to spawn reviewers. **Using it obliges you to state in the output that certification was self-graded and why** (project CLAUDE.md's disclosure rule).
- `--auto` — start in autonomous mode immediately after Step 0 approval; no mid-loop ask-human calls
- `--auto-cap=N` — hard safety ceiling for autonomous mode (default: **30**, max: **30**); overrides `--cycles` when autonomous and N > auto-cap; prevents runaway token burn

---

## Angle Requirements

These rules apply to every angle in every cycle, regardless of scope.

### Evidence gate (all scopes)

Every angle result **must** include at least one of:
- A command and its actual output (grep, find, ls, read, wc — something that ran and produced text)
- An explicit enumerated list of items checked with a total count
- A file path + line number citation pointing to the specific location of the finding

**Pure prose reasoning fails the evidence gate.** "I believe X is covered" or "X looks correct" without a supporting command or citation is not a valid angle result. If an angle produces only prose, it must be re-run with concrete evidence before the cycle result is recorded.

### Enumeration angle (custom scope — mandatory)

When `--scope=custom`, at least one angle must be designated `enumerate:`. This angle:

1. **Runs an explicit enumeration command** (`ls`, `find`, `grep` on an index file, or equivalent) to list every member of the set being audited
2. **States the total count** — "N members found: [list]"
3. **Cross-checks coverage** — after all other angles complete, compares members visited this cycle against the total enumerated. Any member not visited by any angle is a scope gap.
4. **Scope gaps are findings** — an unvisited member triggers a RESET with the finding "scope gap: <member> not covered"

The enumeration angle cannot be satisfied by memory or assumption. It must show the command that produced the member list.

**Example** — for an audit that must cover every shipped example (the differential harness's whole coverage surface):
```
enumerate: run `ls examples/*/` to get the full example-dir list (N dirs found),
           then cross-check which dirs were grep'd in other angles this cycle
```

---

## Step 0 — Approval gate (MANDATORY — never skip)

Parse flags. Missing parameters take the DEC-268 defaults (`--scope=dec268`, `--cycles=5`, `--converge=2`, `--certify=reviewer`). If `--auto` was passed, note that autonomous mode will activate after approval. Then **print the following as plain text and STOP until answered** (adaptation rule 1 — `AskUserQuestion` does not work here):

```
Question: "About to run a convergence loop. Parameters:"
  • Scope:          <3C | 6C | custom angles>
  • Certify:        <self | reviewer>
  • Max cycles:     N  (total attempts before escalating)
  • Converge after: K  consecutive fully-clean cycles
  • Autonomous cap: <auto-cap value> cycles max if autonomous mode is active
  • Angles:
      1. <angle 1 description>
      2. <angle 2 description>
      3. <angle 3 description>

Options:
  1. "Proceed with these parameters (Recommended)"
  2. "Proceed autonomously — run silently to convergence or safety cap; no mid-loop interrupts"
  3. "Change parameters"   → follow up in plain text for N, K, and Certify
  4. "Skip — do not run the loop"
```

If user selects "Proceed autonomously" OR `--auto` flag was passed: set `autonomous = true`. Proceed to Step 1.

If user selects "Change parameters": ask two follow-up questions (max cycles, convergence threshold), then re-display the updated config and confirm once more before proceeding.

If user selects "Skip": exit immediately, report "Convergence loop skipped by user."

---

## Step 1 — Initialize state

```
TOTAL_CYCLES  = N                          # from approved config (default 5 — DEC-268 cap)
CONVERGE_REQ  = K                          # from approved config (default 2 — DEC-268 clean rounds)
CERTIFY       = reviewer | self            # from approved config (default reviewer)
AUTO_CAP      = min(auto-cap, 30)          # hard safety ceiling for autonomous mode
autonomous    = <true if --auto or chosen> # autonomous mode flag
counter       = 0                          # consecutive clean cycles so far (self mode only)
cycle_num     = 0                          # total cycles run
prev_findings = []                         # findings from the immediately preceding cycle
```

---

## Step 2 — Run one cycle

Increment `cycle_num`.

**Autonomous safety cap check**: If `autonomous == true` AND `cycle_num > AUTO_CAP` → go to Step 5 (autonomous safety cap).

Run all angles against the current context. For each angle:
1. Execute the angle (grep, read, enumerate, or reason with evidence)
2. **Apply evidence gate**: confirm the result includes a command + output, enumerated list, or file citation. If not, re-run the angle with concrete evidence before proceeding.
3. List findings as bullet points. A finding is anything unresolved — a risk, gap, side-effect, inconsistency, or scope gap.

**If `--scope=custom` and an `enumerate:` angle is present:**
After all other angles complete, run the cross-check: compare the enumerated member list against members visited in this cycle. Any unvisited member → add as a scope gap finding before recording the cycle result.

**After running all angles, emit a progress line:**

```
[converge] Cycle cycle_num/TOTAL_CYCLES | counter/CONVERGE_REQ clean | <status>
```

Where `<status>` is one of:

- `CLEAN (counter/CONVERGE_REQ)` — no findings at all this cycle
- `RESET (counter → 0) — new: <one-line finding>` — something appeared that was not in prev_findings
- `STUCK — persistent: <one-line finding>` — findings identical to prev_findings, nothing new

*This progress line is always emitted, even in autonomous mode. Autonomous mode suppresses ask-human pauses, not output.*

---

## Step 3 — Evaluate and act

**If `CERTIFY == reviewer`** (default): the reviewer subagents' verdicts ARE the evaluation — do not self-compare. Spawn one read-only reviewer per lens, each given the artefacts (diff, files, tests, spec) and told to **read them itself** and to try to REFUTE the work:
- Every lens returns zero findings → **Case A (CLEAN)**, `counter += 1`. **Do NOT jump straight to converged** — DEC-268 requires TWO consecutive fully-clean rounds, so a single clean round is `counter = 1`.
- Any lens raises something not in `prev_findings` → **Case B (RESET)**, `counter = 0`.
- A lens repeats a point after a resolution attempt → **Case C (STUCK)**.
`prev_findings` is still tracked, and is what tells the next round's reviewers what changed.

**If `CERTIFY == self`**: evaluate using the original self-graded comparison:

**Case A — CLEAN:**
- `counter += 1`
- `prev_findings = []`
- If `counter == CONVERGE_REQ` → go to Step 4 (converged)
- Else → go to Step 2 (next cycle)

**Case B — RESET (new finding appeared):**
- `counter = 0`
- `prev_findings = current_findings`
- Incorporate the new finding into context/plan

- **If `autonomous == true`**: emit one line and continue without pausing:
  ```
  [converge] ↺ RESET cycle_num — autonomous: <finding summary>. Incorporating and continuing.
  ```
  Go to Step 2.

- **If `autonomous == false`**: **print as plain text and STOP until answered**:
  ```
  Question: "New finding detected in cycle cycle_num. Counter reset to 0.
             Finding: <description>
             Continue the loop or escalate now?"
  Options:
    1. "Continue — incorporate and retry (Recommended)"
    2. "Continue autonomously — run rest of loop silently (no more ask-human calls)"
    3. "Escalate — surface to user and stop"
  ```
  - If "Continue": go to Step 2
  - If "Continue autonomously": set `autonomous = true`, go to Step 2
  - If "Escalate": go to Step 5 (cap escalation)

**Case C — STUCK (same findings, nothing new):**
- `counter` unchanged (neither increments nor resets)
- `prev_findings` unchanged
- Attempt deeper resolution of the persistent finding
- Emit: `[converge] STUCK on cycle cycle_num — attempting deeper resolution`
- Go to Step 2
- *(No ask-human call for STUCK — deeper resolution is attempted automatically in both modes)*

**Case D — Cycle cap reached (`cycle_num == TOTAL_CYCLES` and `counter < CONVERGE_REQ`):**
- Go to Step 5 (cap escalation)

---

## Step 4 — Converged

Emit:
```
[converge] ✓ CONVERGED — cycle_num cycles total, counter/CONVERGE_REQ consecutive clean cycles.
```

Report a one-line summary of what was verified across all clean cycles. Exit.

---

## Step 5 — Cap escalation (could not converge)

**Determine cap type:**
- If reached via autonomous safety cap (`cycle_num > AUTO_CAP`): emit `[converge] ✗ AUTONOMOUS SAFETY CAP — {AUTO_CAP} cycles reached.`
- Otherwise: emit `[converge] ✗ CAP REACHED — cycle_num/TOTAL_CYCLES cycles, counter/CONVERGE_REQ clean.`

In both cases:
- List all remaining findings accumulated so far
- Exit autonomous mode: `autonomous = false`

**Print as plain text and STOP until answered** — this is the one guaranteed question in autonomous mode, and per project CLAUDE.md the 5-round cap NEVER silently proceeds:
```
Question: "Could not converge in cycle_num cycles (counter/CONVERGE_REQ clean).
           <If autonomous safety cap: 'Autonomous safety cap of AUTO_CAP cycles reached.'>
           Remaining findings:
             • <finding 1>
             • <finding 2>
           How do you want to proceed?"
Options:
  1. "Rerun — N more cycles (Recommended)"          → restart Step 1 with same K, new N
  2. "Rerun autonomously — N more cycles"           → restart Step 1 with autonomous = true
  3. "Decompose — split task and converge each part"
  4. "Escalate manually — I will review and decide"
```

Wait for direction. This is the only guaranteed ask-human call in autonomous mode.

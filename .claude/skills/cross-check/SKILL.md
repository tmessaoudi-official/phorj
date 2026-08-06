---
name: cross-check
spotlight: true
description: Deep standalone validation of a spec or doc — hunts contradictions, undefined terms, unstated assumptions, missing sections and ambiguities, then certifies the analysis with the DEC-268 reviewer ladder. Use it on a spec before building from it, or to detect spec-vs-implementation drift (Invariant 17).
user-invocable: true
args: "<spec-file> [--drift] [--dry-run]"
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
> /cross-check — Deep standalone validation of a spec or doc: contradictions, undefined terms, unstated assumptions, missing sections, ambiguities. Certified by the DEC-268 reviewer ladder.
>                With --drift, also verifies every mechanically checkable claim against the actual tree.
> ```
>
> Then output the complete flag table from the **"Flags"** section below. Then STOP.

---

# /cross-check — Spec Validation

Parse `$ARGUMENTS`:

## Flags

| Flag | Behavior |
|------|----------|
| `<spec-file>` | Path to spec/doc to validate (required) |
| `--drift` | Also verify every mechanically checkable claim against the actual tree (Mode B) |
| `--dry-run` | Print findings to conversation only; no output file written |

If `<spec-file>` not provided: report error and stop.

Natural targets in this repo: `CLAUDE.md`, `docs/INVARIANTS.md`, `docs/ARCHITECTURE.md`,
`FEATURES.md`, `KNOWN_ISSUES.md`, `README.md`, `examples/README.md`, any `docs/specs/*.md`, any
`docs/plans/*.plan.md`, and the SSOT quartet (`docs/plans/MASTER-PLAN.md`,
`docs/specs/UNIFIED-SPEC.md`, `docs/plans/SLICE-STATE.md`,
`docs/research/full-audit/raw/C-decisions.md`).

---

## Deep doc validation

**The Jira-comparison mode was DELETED on import** (DEC-354 / J.2): there is no Jira and no Jira MCP
server in this environment, so it could never run — a documented mode that cannot execute is worse
than an absent one. What remains is Mode A (internal consistency, the default) and Mode B (`--drift`,
doc vs the actual tree), ported from the sibling repos 2026-08-06.

### Step 1 — Read spec fully

Read `<spec-file>` completely.

### Step 2 — Independent check

Per project **CLAUDE.md's DEC-268 ladder** (investigate → certify → repeat; TWO consecutive fully-clean rounds; cap 5, then ask in plain text — never silently proceed): investigate the three angles yourself, then certify with **fresh-context read-only reviewer subagents** that read the spec themselves (`advisor()` does not exist here).

- **Angle 1** (expanding-context): Are there implicit requirements not explicitly stated? Assumed context that a reader might not share?
- **Angle 2** (adversarial): What internal contradictions exist? What claim is made in one section that is contradicted in another?
- **Angle 3** (blast-radius): What is missing? What should be specified but isn't? What edge cases are unaddressed?

Give the reviewers the spec and the analysis so far; if any raises something new, resolve it and re-run the round (the clean counter resets). Converge on two consecutive fully-clean rounds.

### Step 3 — Output findings

Findings categorized as:
- **CONTRADICTION** — claim in section A directly contradicts claim in section B
- **UNKNOWN** — term or concept used without definition or reference
- **ASSUMPTION** — implicit prerequisite not stated
- **MISSING** — section that should exist but doesn't (error handling, rollback, security, etc.)
- **AMBIGUOUS** — statement that can be interpreted multiple ways
- **STALE** — a claim that was true once and is contradicted by the current tree (`--drift` only)

---

## Mode B — `--drift`: doc vs reality

phorj's docs make an unusual number of **mechanically checkable** claims — counts, invariant
guarantees, CLI surfaces, percentage ledgers — and a stale one is worse than a missing one because it
is trusted. For every such claim in the doc, verify it and record the command you ran as the evidence.

**Counts and percentages drift fastest and are the highest-yield thing to check.** The canonical
precedent: `CLAUDE.md`'s own dependency claim said "four exceptions" when `Cargo.toml` carried 14 —
an understatement by ~3×, in the file every session reads first.

| Claim shape | How to verify |
|---|---|
| "N vetted external crates" / a dependency count | `Cargo.toml` **is the SSOT**, with `docs/specs/UNIFIED-SPEC.md` § "External dependency policy". Re-derive the number; never carry one forward from prose. An *understated* count is the recorded failure mode. |
| "N tests" / the suite is green | `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features` — the summary line is the count. `--all-features` is mandatory (the gate misses `http-client`/`mail`/`database-*` without it). |
| "the `Op` triad is wildcard-free" (Invariant 3) | The naive `grep -n '_ =>' src/vm/exec.rs src/chunk/validate.rs src/compiler/emit.rs` **fires falsely — do not use it**: it returns 4 hits today, all legitimate (inner matches on receiver/closure kind, plus one in a comment), and none an `Op` catch-all. You must read each hit and decide whether it is an arm of the match **over `Op`** — only those count. Also check for a *named* catch-all (`other => other`), which reads as deliberate and greps as handled. |
| "no file exceeds the caps" (Invariant 13) | `bash scripts/size-gate.sh`. A grandfathered file in `scripts/size-baseline.txt` must not have GROWN. |
| A `docs/` cross-reference or heading name | `grep -n '<heading>' <file>` — a `see § "X"` pointing at a section that was renamed or deleted is STALE, and a pointer to a *deleted file* is the worst case. |
| The SSOT quartet agrees (Invariant 19) | read all four. A DEC row with no MASTER-PLAN entry, a MASTER-PLAN item absent from SLICE-STATE, or a percentage ledger that no longer matches `docs/research/full-audit/raw/M-gap-matrix.md` §4 is divergence. SLICE-STATE has been stale by a full wave before, with four BUILT features recorded as "build queued". |
| "a second copy of this fact exists elsewhere" | grep the claim's distinctive phrase repo-wide. Invariant 19 says exactly one canonical home; every other mention must be a pointer. A stale duplicate is worse than no duplicate. |
| A CLI verb or flag exists | run it: `./target/release/phg --help`, `./target/release/phg <verb> --help`. A documented verb absent from the parser is STALE — and note `phg vendor` is **retired and errors** (DEC-282), and there is **no `runvm`** (the VM is `run`'s default engine). |
| "feature X runs" | write the smallest `.phg` that uses it and run all three legs: `phg run`, `phg run --tree-walker`, `phg transpile` piped to the pinned php from `scripts/toolchain.env`. A feature that runs on one leg only is a live Invariant 1 finding, not a doc bug. |
| "feature X transpiles / lifts" (Invariant 17) | both directions, same change. `phg transpile` it, then `phg lift` the result. A feature that runs but does not travel is not done. |
| "the LSP surfaces X" (Invariant 17's 100% RULE) | `grep -n '<capability>Provider' src/lsp/**` and check both editors (`editors/vscode/`, the LSP4IJ path) plus the TextMate grammars. Known standing gap, do not re-report as new: there is **no `signatureHelpProvider`** at all. |
| "every feature has an example" (Invariant 9) | `ls examples/**/*.phg` against `examples/README.md`. The glob in `tests/differential.rs` means **the example corpus IS the byte-identity coverage** — a feature with no example has zero parity coverage. |
| A perf claim / a bench verdict | Invariant 11: no claim above [Inferred] without a measured before/after from `phg benchmark`. Under DEC-365 NO-HIDDEN-LOSS an unmeasurable or failing bench is an **OWED** verdict, never "passed" — and a touched `bench/baseline.json` or `bench/micro-baseline.json` is a finding (note the glob `bench/*-baseline.json` misses the first one) until the recovery is shown to be real. |
| A php-version claim | `scripts/toolchain.env` is the single editable knob. The transpile floor is **PHP 8.5**; the bare `php` on PATH is 8.6-dev and too permissive — a doc gating against bare `php` is STALE. |
| "N skills / N agents / N hooks exist" | `ls .claude/skills/`, `ls .claude/agents/`, `ls scripts/claude-bootstrap/hooks/`. **Never restate the count in prose** — that is the drift this row exists to catch. |
| A doc-guard or infra claim | `bash scripts/doc-guards.sh`, `bash scripts/validate-infra.sh`, `bash scripts/microbench-gate.sh`. |
| A tool is available | `command -v <tool>`. Present here: `cargo`, `cargo-nextest`, `rustc`, `git`, `jq`, `python3`, `php`. |

Report each as **STALE** with: the claim, the command, its actual output, and the corrected value.
Do **not** silently fix the doc — report first. Docs are the project's memory, and a correction the
developer has not seen is indistinguishable from a new error.

**Verify a negative with a control.** "No stale claims found" is only as good as the probe: if a
`grep` returns nothing, first grep for something you know IS there, so you know the probe can fire.

---

## Step 4 — Write output

- `--dry-run`: print to conversation only, stop.
- Otherwise: write to `var/claude/reports/crosscheck-<basename>-<date>.md` (gitignored via `/var`).
  Do **not** write `<spec-file>.validation.md` beside the source — this skill said to do exactly that
  until 2026-08-06, and that path is **tracked** here, so following it dropped an untracked report
  into the working tree one `git add -A` from history. Reports are session state, not deliverables
  (delta 3 of this file's own banner).

State in the output whether certification was by reviewer subagents or **self-graded** (and if
self-graded, why no reviewer was available). Also state which claims you could **not** check and why —
a doc validated with unverifiable claims silently marked OK is the failure mode this skill exists to
catch.

---

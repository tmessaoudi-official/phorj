---
name: phg-lenses
description: >
  MANDATORY companion to every global review skill run in phorj. Load this BEFORE running
  /sweep, /sleuth, /inspect, /gaps, /forge, /cross-check, /converge, /pre-commit or
  /aggregate-findings here — it carries the phorj review dimensions (the invariants a review
  must check), sleuth lens K (backend divergence), and the repo conventions those global skills
  do not know about. Extracted 2026-08-18 from the deleted repo-local copies of those skills
  (global-is-reference ruling: a repo may not duplicate a global skill; what was repo-specific
  in them lives here instead).
---

# /phg-lenses — phorj review dimensions & conventions

This skill adds no procedure of its own. It is the **domain payload** for the global review
skills: run the global skill for its machinery, with everything below folded into its scope.

## Repo conventions (apply to every review skill)

- **Reports live in the repo**: `var/claude/<skill>/` (gitignored). Never `~/.claude/projects/…`.
- **Non-blocking closes — no interrupts.** End with the findings and a plainly-stated offer
  (`N findings (P0:a P1:b P2:c) — say which to fix`), never a blocking question.
- **`/converge` runs the DEC-268 MAXIMAL ladder** — the three lenses are the repo agents:
  `backend-parity-reviewer`, `safety-promises-reviewer`, `completeness-reviewer`
  (`.claude/agents/`). Two consecutive fully-clean rounds, cap 5.
- **Project scope only.** `~/.claude/` is the developer's own persistent install, out of this
  repo's audit scope — audit it from its own sessions, not from here.
- **The SSOT quartet governs every claim about roadmap/spec/slice/decision** (Invariant 19):
  `docs/plans/MASTER-PLAN.md` · `docs/specs/UNIFIED-SPEC.md` · `docs/plans/SLICE-STATE.md` ·
  `docs/research/full-audit/raw/C-decisions.md`. A finding that contradicts one of them cites it.

## Review dimensions — MANDATORY additions to any sweep/review of this repo

Run these **in addition to** the global skill's own dimensions, on every review:

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
- **CTy-operand trap (Invariant 7 — MUST-CHECK).** Did the change un-reject an expression form, or add
  one whose result can be an arithmetic operand? Then the compiler's `CTy` resolver must type it, and a
  differential case shaped `expr + 1` must exist. Otherwise the VM rejects what the interpreter accepts.
- **Mid-expression scratch slots (Invariant 8 — MUST-CHECK).** Any op that stashes a receiver (the
  `??` / `?.` / `!`-unwrap family) must use `self.height - 1`, never `locals.len() - 1`. A new such
  construct needs a differential case with TWO of them in one expression.
- **Always-current surfaces (Invariant 17).** transpile AND lift updated in the SAME change; `phg check`
  ≡ LSP diagnostics (DEC-252); and THE 100% RULE — the LSP surfaces it everywhere it could appear
  (completion, hover, go-to-definition, find-usages, document symbols, diagnostics with the right tags,
  signature help) AND both editors updated in the same change, grammars included. "The compiler knows it
  but the editor doesn't" is an incomplete feature.
- **SSOT quartet (Invariant 19).** `docs/plans/MASTER-PLAN.md` · `docs/specs/UNIFIED-SPEC.md` ·
  `docs/plans/SLICE-STATE.md` · `docs/research/full-audit/raw/C-decisions.md`, mutually consistent in
  THIS change. A slice started or finished without SLICE-STATE moving is a finding; so is a second copy
  of a fact the quartet owns.
- **File-size caps (Invariant 13).** Soft 300 / hard 500 lines per source file; a feature that pushes
  a file past the soft cap should have STARTED by splitting it.

## Sleuth lens K — MANDATORY additional agent for /sleuth

Beyond the global skill's agents A–J, always run **agent K** on this repo, and report its findings
as category **K** alongside A–J:

> **K — Backend divergence.** phorj has a triple spine: the tree-walking interpreter (the reference
> oracle), the bytecode VM, and the Phorj→PHP transpiler, and Invariant 1 requires identical stdout
> AND identical failure behaviour across all three. Hunt for places where they can disagree:
> a value kernel re-inlined in a backend instead of used from `src/value/` (Invariant 4); a fault
> string typed twice; a VM path that compiles via plain `compile` instead of
> `check_and_expand_reified` + `compile_with` (Invariant 6); an expression form the checker accepts
> but the compiler's `CTy` resolver cannot type (Invariant 7); a mid-expression scratch slot using
> `locals.len() - 1` instead of `self.height - 1` (Invariant 8); compile-time sugar that reaches a
> backend un-expanded (Invariant 5). For each: file + line, which two legs diverge, the smallest
> program that would show it, and whether an `examples/**/*.phg` case covers it (if not, that is the
> finding). Research only, no writes.

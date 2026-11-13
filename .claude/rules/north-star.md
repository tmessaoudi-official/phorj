# NORTH STAR - phorj   (loaded every session; every decision is judged against this page)
Ruled by the developer 2026-11-13 (DEC-578, DEC-579, DEC-580). Philosophy: `VISION.md` (imported by CLAUDE.md).
This page holds bars and rules only. Current values live in the checkers it names, never here.

## Goal
Make Phorj a superset of PHP's capabilities - not of its syntax or its footguns (MASTER-PLAN finishing wave,
locked ruling C) - that is measurably better than PHP on every axis below, reaching literal 100 % of the vision
with no park left (DEC-493), built defects-first (DEC-578), with staged stable releases on the way (DEC-579).
- In scope: the language, its three legs and the JIT; transpile and lift as migration bridges; the tooling;
  stdlib parity; the beyond-PHP programme (`docs/plans/MASTER-PLAN.md`).
- Out of scope: a PHP runtime or drop-in (VISION, "What Phorj is not"); any feature that depends on PHP at run
  time; calling the goal met with gaps documented instead of closed (rejected in DEC-493).
- Done when: every axis has a checker and the checker is green; every gap-matrix row is covered or excluded by a
  ruling; `KNOWN_ISSUES.md` holds no park.

## Rules for every session
1. **Defects first, severity-gated (DEC-578).** No new feature row starts while a P0 or P1 defect is open. A
   feature row adds language, stdlib, lifter, or LSP and editor surface.
   - A defect is behaviour that is wrong: the legs disagree outside the exceptions Invariants 1 and 14 name; or
     all legs agree on behaviour a ruling or the spec says is wrong; or the no-crash contract
     (`docs/INVARIANTS.md`) is breached; or it is a security defect.
   - P0/P1 = wrong output, a crash or hang, or a security exposure: these block. P2/P3 = no difference in a
     program's output or exit behaviour and no security exposure: queued, may run alongside features. The ruled
     pair: `DECIMAL-ORDER` blocks, `TRACE-FRAME-NAMES` does not.
   - The ONE list is the defect queue in `docs/plans/SLICE-STATE.md`; every entry carries its grade and the
     reason for it.
   - A defect whose fix needs a ruling does not block: record the question (Invariant 15), raise it at the closing
     check-in, take the next defect.
   - This is the ordering rule. It supersedes the orders stated in `docs/plans/MASTER-PLAN.md` (the "Active
     programme" and "Next" rows, §0 THE QUEUE, §0.06) and the "scout is the forcing function" headline of
     SLICE-STATE; their rows stay queued.
2. **Axes in conflict are surfaced** - asked in spec mode, recorded PENDING in autonomous mode. Never trade one
   axis for another silently (Invariant 14; MASTER-PLAN finishing-wave rule 3).
3. **A bar with no checker is OWED, not met**, and is never reported as passed (DEC-580). "Armed" means a checker
   exists. It does not mean the bar is met. The checker column below is updated in the same change that adds a
   checker.
4. **Stable releases (DEC-579).** A stable pre-1.0 release may be cut when the gate is green and no P0/P1 is
   open; 1.0.0 only at the goal. Cutting or moving a `v*` tag is the developer's explicit instruction in every
   mode, never a session's own step.

## The fifteen axes (list closed, DEC-580)
| # | Axis | Bar | Checker today |
|---|------|-----|---------------|
| 1 | Speed | every benched feature beats release PHP + JIT (ratio above 1.0) or is flagged in flight (DEC-269, Invariant 18); in the scout campaign a loss that cannot be flipped stops the lane and escalates (DEC-507); no flag survives to the goal (DEC-493) | armed: `scripts/microbench-gate.sh` (pre-push, skips without docker), `scripts/perf-gate.sh` (CI). Losses carried: `_owed` in `bench/micro-baseline.json` |
| 2 | Correctness | no open defect of rule 1's first two kinds: the legs agree, and on the right behaviour | armed for the shipped corpus: `tests/differential.rs`, `tests/conformance.rs`. Owed: the defect queue as a counted check |
| 3 | Robustness and security | no input aborts a `phg` process; no memory growth in a `phg serve` soak; every PHP footgun class Phorj removes has a rejecting test; no reachable advisory | armed: CI job `unsafe-island`. Owed: fuzzing (needs a dependency ruling), soak test, footgun suite, advisory check |
| 4 | Learnability and tooling | every stable construct and native has a doc entry with a runnable example; a tutorial and a PHP migration guide exist; the LSP serves every construct (Invariant 17) | partly armed: `scripts/surface-ratchet.sh` (a floor that only rises). Owed: docs coverage check, tutorial, migration guide |
| 5 | Code cleanliness | no grandfathered over-cap file; the target architecture built (DEC-562); no unrecorded catch-all arm (Invariants 3, 13) | armed: `scripts/size-gate.sh`, the catch-all ratchet (CD-32). Owed: the architecture work |
| 6 | Memory | peak memory at or below PHP on every benchmark, or flagged like a speed loss | owed |
| 7 | Startup and latency | cold start of `phg run` and time of `phg check` on the example corpus within a budget | owed (budget not ruled) |
| 8 | Migration | every file of the reference PHP corpora lifts and checks, or carries a named refusal with a decision row | armed for fixtures: `tests/lift_roundtrip.rs`. Owed: the corpus census as a check |
| 9 | Parity completeness | every gap-matrix row covered or excluded by ruling, recomputed by script | owed (recomputed by hand today) |
| 10 | Diagnostics | every checker and parser error has a code and a `phg explain` entry | armed for coded errors: `src/cli/tests/explain_ratchet.rs`. Owed: coding the uncoded ones (prerequisite of DEC-565) |
| 11 | Stability | every construct `STABILITY.md` lists as stable has a conformance fixture | armed as regression: `tests/conformance.rs`. Owed: the coverage map |
| 12 | Deployment | release binary under a size ceiling; the four platform builds green; build reproducible from the lockfile | armed: CI job `cross-build`. Owed: size ceiling (not ruled), reproducibility check |
| 13 | Supply chain and docs truth | crates within the dependency policy; no reachable advisory; no stale fact in an always-loaded file | armed: `tests/dependency_claims.rs`, `scripts/doc-guards.sh`. Owed: advisory check, drift check for always-loaded files |
| 14 | Web throughput | requests per second and tail latency of `phg serve` at or above php-fpm on the same routes | owed |
| 15 | Real application | at least one real application runs on Phorj with its own CI (Invariant 18's `var/phorj-app`; MASTER-PLAN W6-1) | owed |

## Open - not ruled, do not decide alone
- Release numbering: the manifest reports `1.0.0-nightly.0`; a `v0.x` tag would sort below it.
- Whether DEC-416 (a retired form is changed outright, no deprecation window) still holds after the first stable
  release; `SEMVER.md` promises a `W-DEPRECATED` grace release.
- The grades of the defect queue: an entry's grade is a proposal until the developer ratifies it.
- Whether a perf row is a feature row under rule 1. The ruling paused one (readiness row 19), so until this is
  ruled a perf row is not started ahead of a P0/P1 defect either.
- The budgets and ceilings of axes 7, 12 and 14.

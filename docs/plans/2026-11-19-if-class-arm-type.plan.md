# IF-CLASS-ARM-TYPE: an `if` expression of two classes types as `Other` on the VM (2026-11-19) Plan

> Defect-queue entry IF-CLASS-ARM-TYPE (P0, ratified 2026-11-18; SLICE-STATE § DEFECT QUEUE), the developer's pick on
> 2026-11-19 (INIT-CHAIN plan, close-out). Reproduced on the release binary: `var y = if (c) { new Kid(1) } else { new
> Par(2) }; var w = y with { x = 9 };` with `c` false prints `with kid 9 true` on `phg run` and `with par 9 false` on the
> tree-walker and PHP. Root cause, read in the code: `ctype`'s `Expr::If` arm (`src/compiler/cty.rs`) typed an
> un-narrowed `if` as its then-branch alone, and `Kid` (extended by nobody) is exact, so `with` built a `Kid`. Second
> form found by probe: a class arm beside an erased `Option` payload binding (typed `Other`) took the class in both
> `if` and `match` (`om kid 4` / `oi kid 5` on the VM, `par` elsewhere), because `cty_compatible` lets an unknown vote
> for nothing. Closed by probe: `?` inside a branch (checker: Result-mode `?` only as a whole initializer) and `??`
> with a `Par` default for `Kid?` (checker rejects it). Built in autonomous mode (away window, plan gate ASSUMED).

## Decisions Log
- [2026-11-19 14:09] ASSUMED (review): plan gate (autonomous, away window): fix IF-CLASS-ARM-TYPE with one join shared by match and if, widened so a class arm beside an unknown (Other) arm gives Other in match too (an R9-K1 residual found by probe: om kid 4 on the VM). No ruling needed: the VM is brought to the oracle (Invariant 2). Alternatives: if-only fix (leaves the match residual); treat Other as incompatible with everything (loses scalar specialization, e.g. 1 beside f()).
- [2026-11-19 15:03] ASSUMED (review): close-out: keep the plan file and write the ledger, as the developer ruled for the INIT-CHAIN plan the same day ("Keep + ledger"); pushed through `fbbe45a0` (`test: pin a field of a mixed-class value beside an ambiguous member`); next defect GENERIC-PAYLOAD-OPERAND (INIT-READ-BEFORE-SET needs an E-code name, an Invariant-15 question, PENDING in autonomous mode). Alternatives: delete the plan (loses the sabotage evidence).

## Formal Plan

1. **Failing test first** (`tests/differential.rs`, `if_expression_of_two_classes_types_as_other`, pinned output on
   three legs): two classes in both orders, the true condition, a field operand, a throwing branch, a numeric `if`,
   two `if`s in one expression, an erased generic call branch, and a class arm beside an erased `Option` payload in a
   `match` and in an `if`. Red on the VM `with` line only, for the stated reason.
2. **Fix**: one join, `match_bindings::join_arm_ctys`, shared by `ctype_match` (R9-K1) and the un-narrowed `if`
   (both non-throwing branches vote): incompatible arms, or a class arm beside an unknown, give `Other`; an unknown
   beside a scalar still votes for nothing; the first arm's error is kept. `cty.rs` stays under the 500 hard cap.
3. **Sabotage**: then-branch-only typing reddens `with` and `gen`; dropping the class-beside-unknown clause makes the
   VM print `om kid 4 oi kid 5` (release-binary probe).
4. **Example + SSOT**: a line in `examples/guide/runtime-class-dispatch.phg` (`animal Kit2` on three legs) and its
   README row; SLICE-STATE queue row out; CHANGELOG. LSP and editors: no new syntax or symbol. Transpile and lift:
   unchanged (the VM is brought to the oracle).
5. **Gate + perf**: the full all-features gate, release binary; perf: the change only widens `Other` on mixed-class
   arms (no hot path specialized on a class `with`); `phg benchmark` OWED unless the box is quiet.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing three-leg differential test | S | done | red on the VM `with` line only (`with kid 9 true` vs `with par 9 false`), tree-walker and PHP exact | tests/differential.rs |
| 2 | Shared join for match and if | S | done | differential binary 294/294 green with `PHORJ_REQUIRE_PHP=1` | src/compiler/match_bindings.rs src/compiler/cty.rs |
| 3 | Sabotage both clauses | S | done | then-only typing: `with kid 9 true`, `gen kid 6`; no class-beside-unknown: VM `om kid 4 oi kid 5`; both restored byte-exact | - |
| 4 | Example, README, SLICE-STATE, CHANGELOG | S | done | example `animal Kit2` on three legs, `phg format --check` clean | examples/guide/runtime-class-dispatch.phg examples/README.md docs/plans/SLICE-STATE.md CHANGELOG.md |
| 5 | Full gate, release binary, perf | S | done | all-features gate 2026-11-19 14:16: nextest 3772 passed 2 skipped, clippy all-features and no-default-features 0 warnings, fmt, size gate (grandfathered=54 fails=0), release build; perf OWED (load 24.5) | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
- `src/compiler/cty.rs` is at 494 lines: any growth past 500 breaches the hard cap (Invariant 13).
### Known issues
- Perf OWED (DEC-365): no `phg benchmark` before/after (1-min load 24.5 during the gate); the change only widens `Other` for mixed-class arms, so a specialized class path is lost only where it was wrong.

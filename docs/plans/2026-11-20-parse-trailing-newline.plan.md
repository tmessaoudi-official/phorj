# R9-AE-4 PARSE-TRAILING-NEWLINE: a number with a trailing `\n` is not a number on the PHP leg either (2026-11-20) Plan

> Defect-queue entry R9-AE-4 PARSE-TRAILING-NEWLINE (P1 ratified 2026-11-17; SLICE-STATE § DEFECT QUEUE), quick
> win #5 (`var/claude/aggregate-findings/2026-11-16/report.md`). Reproduced with the release binary at `8c6cf04d` and
> `scripts/dk php -n`: the native legs print `null` for `String.parseFloat("1.5\n")` (strict and permissive) and
> `Decimal.of("1.5\n")`, the transpiled PHP prints `1.5` for both `parseFloat` forms and dies in `bccomp()`
> (`ValueError: … not well-formed`) on `Decimal.of`. Root cause [Verified: read the emitted helpers]: the helper
> regexes in `src/transpile/runtime_php.rs` end in `$` without the `D` modifier, and PCRE's `$` also matches before a
> final newline.

## Decisions Log

## Formal Plan

1. **Failing test first** (`tests/differential.rs`, `agree_out_php`, the three legs and pinned output):
   `parseInt`, both `parseFloat` forms and `Decimal.of` on `"…\n"`, plus the plain values as a control.
2. **Fix**: `D` on the four anchored helper regexes (`__phorj_parse_int`, both `__phorj_parse_float` grammars,
   `__phorj_dec_of`). The `Validation` natives' regexes are the separate row VALIDATION-regex-trailing-newline.
3. **Sabotage, SSOT, gate**: one mutant per regex.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing test | S | done | `a_number_with_a_trailing_newline_is_not_a_number_on_any_leg` red on the PHP leg alone (php exited non-zero: the `bccomp` fatal); the native legs printed the pinned output | tests/differential.rs |
| 2 | `D` on four helper regexes | S | done | 307 targeted tests green | src/transpile/runtime_php.rs |
| 3 | Sabotage, SSOT, gate | S | done | resumed 2026-11-23 (the 11-20 run was killed; `target/` was later wiped with its script and patch): one mutant per regex, each restored by content (`cmp` RESTORE-OK): `parse_int` without `D` stays green (equivalent mutant, now verified), the permissive `parseFloat` grammar reddens the `lax` line, the strict one the `float` line, `dec_of` reddens on the `bccomp` fatal; full all-features gate green on the PHP 8.5.13 image: 3801 passed, 2 skipped, both clippy runs, fmt, release build, size gate | CHANGELOG.md docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- `__phorj_parse_int`'s `D` is not provable by output: its digit re-check already rejects `"12\n"` (an equivalent
  mutant: removing the `D` leaves the regression green, run 2026-11-23), so it is defence in depth.
- The `Validation` natives' regexes have the same `$` shape; they are the separate queue entry
  VALIDATION-regex-trailing-newline and are not touched here.

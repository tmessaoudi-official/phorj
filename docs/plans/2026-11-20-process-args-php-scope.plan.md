# PROCESS-ARGS-PHP-SCOPE: `Process.arguments()` reads the real arguments on the PHP leg inside a function (2026-11-20) Plan

> Defect-queue entry PROCESS-ARGS-PHP-SCOPE (P1 proposed 2026-11-20, ratification owed; SLICE-STATE § DEFECT QUEUE),
> found while closing R9-G-2. Reproduced with the release binary at `a606823b` and `scripts/dk php -n`:
> `examples/process/main-args.phg -- alpha beta` prints `matches Process.arguments() = true` natively and `false` on
> the transpiled PHP; a probe calling `Process.arguments()` from a helper function prints nothing there and a lambda
> sees 0 arguments. Root cause [Verified: read the emitted PHP]: the native's PHP twin (`src/native/process.rs`)
> emits `array_slice($argv ?? [], 1)` at the call site, and a PHP function does not see the global `$argv`.

## Decisions Log

## Formal Plan

1. **Failing test first** (`tests/cli_argv.rs`, skip-loud under `PHORJ_REQUIRE_PHP=1`): an inline probe reading
   `Process.arguments()` from a helper function and a lambda; native and `php -n` stdout and exit code pinned.
2. **Fix**: one constant, `native::PHP_PROGRAM_ARGS = array_slice($GLOBALS['argv'] ?? [], 1)`, used by the twin and
   by the entry bootstrap (`src/transpile/program_emit.rs`), so `main(List<string>)` and `Process.arguments()` read
   the same variable. Measured before choosing: the hand-edited transpile prints the native output on PHP 8.5 and 8.6
   (`scripts/dk --php 8.6`) with an empty stderr.
3. **Sabotage, SSOT, gate**: the constant reverted to `$argv` must redden the test (not skip); README mapping line,
   CHANGELOG, queue row out; full gate.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing test | S | done | `process_arguments_inside_a_function_match_on_the_php_leg` red: PHP printed `matches Process.arguments() = false` | tests/cli_argv.rs |
| 2 | One shared expression | S | done | 114 targeted tests green (`cli_argv`, `process`, the transpile unit tests) | src/native/process.rs src/native/mod.rs src/transpile/program_emit.rs |
| 3 | Sabotage, SSOT, gate | S | done | the constant reverted to `$argv` reddens the test under `PHORJ_REQUIRE_PHP=1` (`lambda 0`, not skipped), restored green; full gate (nextest all-features 3800 passed 2 skipped, clippy x2, fmt, size-gate 54/0, release) | examples/process/README.md CHANGELOG.md docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- The PHP leg still disagrees with the native legs on a NON-UTF-8 argument or variable (KNOWN_ISSUES
  NONUTF8-PROCESS-INPUT, DEC-590 PENDING); this row fixes valid input only.

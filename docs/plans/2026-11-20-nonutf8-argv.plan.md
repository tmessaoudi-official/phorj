# R9-G-1 NONUTF8-ARGV: a non-UTF-8 argument is refused, never a panic (2026-11-20) Plan

> Defect-queue entry R9-G-1 NONUTF8-ARGV (P1 ratified 2026-11-17; SLICE-STATE § DEFECT QUEUE), quick win #2
> (`var/claude/aggregate-findings/2026-11-16/report.md`). Reproduced on the release binary at `43d93e8e`:
> `phg check examples/hello.phg $'\xff'` panics in `std::env::args()` (exit 101), and so does a built binary given
> such an argument. Root cause [Verified: read the code]: both argument reads in `src/main.rs` (the built-artifact
> path and the CLI dispatcher) call `std::env::args()`, which panics on a non-Unicode argument.

## Decisions Log
- [2026-11-20 12:41] ASSUMED (review): a non-UTF-8 argument after `argv[0]` is refused with one stderr line and exit 1 (what `phg check /nonexistent.phg` returns; T-6's no-crash contract), never mapped lossily: a lossy file name names a different file, and a lossy `Process.args()` value would be a new language behaviour (the developer's call, to be asked together with R9-G-2's environment variables). `argv[0]` is only a label and is read lossily. Basis: advisor() 3C round 1.

## Formal Plan

1. **Failing tests first**: `tests/cli_argv.rs` (unix; trailing argument, file name, program argument after `--`
   refused with exit 1, empty stdout, no `panicked`; control: `é` and an emoji reach `Process.args()` with a
   non-UTF-8 `argv[0]`); `tests/build.rs` `built_binary_refuses_a_non_utf8_argument`.
2. **Fix**: `src/cli/argv.rs` (`utf8_argv`, pure and unit-tested; `args_or_exit`, `program_args_or_exit`); both
   `src/main.rs` call sites swap in one line each (the file is at 498 of 500).
3. **Sabotage, probe, SSOT, gate**: a mutant per call site and one for the `argv[0]` rule; release binary probed;
   CHANGELOG, queue row out; full gate.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing tests | S | done | 3 tests red with exit 101 (the panic) for the stated reason | tests/cli_argv.rs tests/build.rs |
| 2 | Refuse instead of panic | S | done | 8 targeted tests green; `src/main.rs` still 498 lines | src/cli/argv.rs src/cli/mod.rs src/main.rs |
| 3 | Sabotage, probe, SSOT, gate | S | done | three compiling mutants red on their own tests (CLI site: both `cli_argv` tests; artifact site: `built_binary_refuses_a_non_utf8_argument`; strict `argv[0]`: the unit test and the control), restored 8/8; release binary probed: exit 1 with the one-line error for a trailing argument, a file name and a program argument; full gate (nextest all-features 3797 passed 2 skipped, clippy x2, fmt, size-gate 54/0, release) | CHANGELOG.md docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
### Needs input
- Whether `Process.args()` and `Environment.all()` should take a non-UTF-8 value lossily (U+FFFD) instead of
  refusing or dropping it: one question for R9-G-1 and R9-G-2, recorded before R9-G-2 starts.
### Needs research
### Fragile
- `src/main.rs` is at 498 lines (hard cap 500).
### Known issues

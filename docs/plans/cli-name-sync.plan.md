# CLI Name Sync Plan

> Reconcile `phg` CLI command names with internal file/module/handler names so one word is used
> per concept, end-to-end. CLI command surface is the canonical anchor (unchanged); code renamed
> to match. SSOT roadmap stays `MASTER-PLAN.md`; this is the execution log for the sync.

## Decisions Log
- [2026-07-06] AGREED (developer): JIT dep-policy amendment ruled — full amendment, **in-tree
  `src/jit/`** layout (recorded in `perf-wave.plan.md`; not built yet). Cross-ref only.
- [2026-07-06] AGREED (developer): CLI command names stay **canonical and unchanged** (long forms
  kept: `format`/`benchmark`/`disassemble`/`tokenize`); ALL code renamed to match them (Option 2 —
  developer rejected shortening the CLI).
- [2026-07-06] AGREED (developer): rename the **stage/engine modules too** —
  `src/lexer/` → `src/tokenizer/` (matches `phg tokenize`, fits the `parser`/`checker`/`compiler`
  `-er` stage pattern) and `src/fmt/` → `src/format/` (matches `phg format` AND removes the
  `crate::fmt` vs `std::fmt` overload — 2 files use `std::fmt`). `token.rs` KEPT (data type, shared
  by tokenizer + parser).
- [2026-07-06] AGREED (developer): KEEP role-suffix files `src/cli/test_runner.rs` and
  `src/cli/debug_repl.rs` — the command word already matches (`test`/`debug`); `_runner`/`_repl`
  describe the file's role, not a mismatch.
- [2026-07-06] AGREED (developer): `runvm` cleanup = **FULL SWEEP as a SEPARATE task/commit**, NOT
  folded into this rename. Handoff's "5 files" was wrong — it's **~150 occurrences / ~80 files** in 3
  flavors: (a) ~6 literal `phg runvm` command refs (genuinely stale), (b) ~8 test/helper fn names
  (`runvm_*`/`coop_runvm`), (c) ~130 `run ≡ runvm` parity-spine comments. NUANCE: post-CLI-reshape
  `phg run` = the VM, so `run ≡ runvm` is stale in BOTH terms → the correct rewrite is
  `interpreter ≡ VM ≡ PHP` (or `tree-walker ≡ VM`), a *semantic* rewrite requiring per-site care +
  its own `PHORJ_REQUIRE_PHP` gate. Deferred to its own task; this rename commit touches NO `runvm`.

## Formal Plan

### Renames
| From | To | Kind |
|---|---|---|
| `src/lexer/` | `src/tokenizer/` | module dir (22 refs / 18 files) |
| `src/fmt/` | `src/format/` | module dir (11 refs / 4 files; avoid `std::fmt`) |
| `src/cli/bench.rs` | `src/cli/benchmark.rs` | command file |
| `src/cli/fmt_cmd.rs` | `src/cli/format_cmd.rs` | command file |
| `cmd_lex` | `cmd_tokenize` | handler fn |
| `cmd_fmt` / `fmt_source` | `cmd_format` / `format_source` | handler fns |
| `cmd_disasm` | `cmd_disassemble` | handler fn |
| `cmd_bench*` (4 variants) | `cmd_benchmark*` | handler fns |

### Steps
1. `git mv` the two module dirs + two command files.
2. Fix `mod`/`use`/path refs: `lexer`→`tokenizer` (unambiguous), `crate::fmt`/local `fmt::`→`format`
   (NEVER touch `std::fmt`/`core::fmt`), `mod bench`→`mod benchmark`, `mod fmt_cmd`→`mod format_cmd`.
3. Rename handler fns + update the `main.rs` dispatch call sites (CLI command strings UNCHANGED).
4. `cargo build` — the compiler is the guardrail; fix every missed ref.
5. Docs: `docs/ARCHITECTURE.md` module map (lexer→tokenizer entries).
6. (`runvm` full sweep = SEPARATE task — see Decisions Log; NOT in this commit.)

### Acceptance (full gate)
- `cargo build --release`, `cargo clippy --all-targets`, `cargo fmt --check` clean (warnings deny).
- `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo test --workspace` green — proves NO
  behavior changed (rename-only; the byte-identity spine is untouched).
- CLI surface unchanged: `phg format|benchmark|disassemble|tokenize` still dispatch (existing tests).

### Rollback
Pure rename; `git checkout -- .` / `git reset --hard HEAD` before commit restores prior state.
No behavioral risk — the differential + oracle gate is the safety net.

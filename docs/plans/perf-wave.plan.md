# Perf Wave Plan — make phorj measurably faster than PHP, per-feature

> Working plan for the G-8 perf mandate. SSOT roadmap stays `MASTER-PLAN.md` (G-8, W6-4, UA-0.10);
> this file is the execution log + decisions for the perf wave. Full diagnosis: memory
> `perf-benchmarking-truth`.

## Decisions Log
- [2026-07-05] AGREED: The **endgame is a JIT/AOT backend** — truly beating PHP+JIT on hot numeric
  loops requires native codegen. Push the bytecode VM as far as it goes first (closes most of the gap);
  open the §15 JIT/AOT fork when a feature provably cannot beat release-php+JIT after VM optimization.
  "Faster on everything" is literal and committed.
- [2026-07-05] AGREED: **Substrate-first, rising-tide sequencing.** The 6–28× gap is uniform → shared
  VM overhead (dispatch loop, per-op `Op` clone, allocation, value repr). Fix the substrate first (one
  fix lifts every feature), re-measure the whole matrix, then chase per-feature stragglers.
- [2026-07-05] AGREED: **Autonomous marathon.** Build harness, profile, land substrate fixes, sweep
  features, commit each green+measured slice; stop only to surface genuine forks (per-feature ceiling
  decisions, any §15 JIT call).
- [2026-07-05] AGREED: **Profiler = Docker + callgrind** on the existing release binary (perf blocked:
  `perf_event_paranoid=4`, no CAP_PERFMON, host sudo denied; valgrind absent on host). Deterministic,
  no rebuild, no host perms.
- [2026-07-05] AGREED: **JIT/AOT is the path (Option 1)** — VM micro-opt curve flattened (fix#1 −10%,
  safe wins −0%, frame-caching ≤5%); no bytecode-VM tuning under `forbid(unsafe)` closes the 26× gap.
  Beating PHP needs native codegen. **Harness (Option 4) co-runs** as the JIT measurement backbone AND
  the playground perf-number source. `forbid(unsafe)` question folds INTO the JIT design (JIT needs
  unsafe/Cranelift). Frame-caching (Option 3) DROPPED.
- [2026-07-05] AGREED: **PHP execution model = bytecode VM (= `phg runvm`) + optional JIT.** PHP is
  NEVER a tree-walker. So the honest races are `runvm` vs `php-no-JIT` (VM vs VM) and phorj-JIT vs
  `php+JIT` (native vs native). `phg run` (tree-walk) races nothing in PHP — it's the oracle only.
- [2026-07-05] AGREED: **CLI reshape.** `phg run` and bare `phg <file>` → the **VM** (then JIT).
  `phg run --tree-walker` → the interpreter. **`phg runvm` REMOVED entirely** (docs/scripts swept same
  change; the distributed binary already dispatches via `cmd_runvm`, so the runtime default is
  unchanged — only the CLI surface). Tests still run both backends + compare (unchanged).
- [2026-07-05] AGREED: **Keep the tree-walker as the correctness oracle** (independent 2nd
  implementation; validates the whole compile pipeline; total coverage incl. concurrency; the
  executable spec). Not user-facing. Its value rises with the JIT. Bounded maintenance via
  single-sourced kernels. PHP is a bonus 3rd oracle, cannot replace it.
- [2026-07-05] AGREED: **Playground perf display = precomputed NATIVE numbers, 4 engines**
  (tree-walk / VM / PHP+JIT / transpiled-PHP-under-real-php), time + peak memory, per-example + a
  global summary. NO live in-browser timing (php-wasm has no JIT → misleading). Harness computes them;
  frontend-only display (no WASM rebuild; `wasm-pack` absent locally).
- [2026-07-05] AGREED: **Explore Option 2 (VM ceiling)** — research how close a hard-tuned VM
  (possibly relaxing `forbid(unsafe)` for validated-bytecode indexing) gets to PHP-no-JIT; if the VM
  can beat PHP-no-JIT, JIT is only needed to beat PHP+JIT (sharpens the roadmap).
- [2026-07-05] AGREED: **Perf premise** — the CLI rename is a UX win (fast engine by default, kills the
  7s tree-walk trap); it does NOT beat PHP. Only the JIT/AOT backend beats PHP.

## Measured baseline (2026-07-05) — the honest truth
Pure execution, self-timed (phg `Runtime.monotonicNanos`, php `hrtime`), best-of-5, startup excluded.
phg runvm (release) vs **real release PHP 8.5.7 NTS via `docker run php:8.5-cli`** (all 3 local php
builds are ZTS DEBUG, JIT off — no honest baseline on-box):

| Heavy workload | phg runvm | PHP+JIT | PHP no-JIT | phg vs PHP+JIT | peak mem phg/php |
|---|---|---|---|---|---|
| fib(30) CPU recursion | ~270 ms | ~9.6 ms | ~29 ms | **~28× slower** | 12 / 2 MB |
| heap 2M object allocs | ~775 ms | ~79 ms | ~133 ms | **~10× slower** | 12 / 2 MB |
| str 200k concat | ~1200 ms | ~1–2 ms | ~2 ms | O(n²) footgun† | 12 / 2 MB |

† `s = s + "x"` allocates each iteration (immutable string) → O(n²) vs PHP `.=` O(n). Separate class
from VM dispatch — track as an idiom/algorithm issue, not a dispatch bug.

## Formal Plan (stages)
**Stage 0 — Instrument (parallel).**
- 0a. callgrind the fib hot path in Docker → deterministic top-cost attribution (100% root cause).
- 0b. Per-feature microbench harness: corpus of isolated micro-programs (arith int/float/decimal,
  string concat/interp/`%`-format, list index/map get-put/set, method call, closure call, match,
  enum construct, try/catch, loop forms, …), each self-timed in-process (warmup + median/best-of-N),
  `runvm` vs Docker release-php+JIT, ns/op, baseline-tracked, regression-gated (extend
  `scripts/perf-gate.sh`). This IS the W6-4/UA-0.10 work.

**Stage 1 — Substrate fixes (rising tide), each measured before/after on the full matrix + memory:**
candidate levers from the code read (confirm/re-rank with callgrind first):
- dispatch loop `code[ip].clone()` of 73-variant non-`Copy` `Op` (~38M×/fib30) → avoid the clone
  (index by ref / make hot ops cheap / restructure the borrow);
- repeated per-op indirection + bounds checks (frame/code re-derivation each iteration);
- call/frame setup cost; allocation strategy for `Value`/`Instance`; value representation.

**Stage 2 — Re-measure whole matrix**, rank remaining per-feature stragglers by gap.

**Stage 3 — Per-feature sweep:** each straggler → optimize to beat release-php+JIT, or surface a
§14-ladder-style ceiling decision (accept tolerance / JIT that feature / transpile-only).

**Stage 4 — Gate:** per-feature regression gate green; every feature's DoD includes its microbench
beating release-php.

## Progress
- **Stage 0a DONE** — callgrind (Docker, fib28, 1.53B Ir) root-caused the gap: exec_op 35% + run_main
  26% (= 61% dispatch machinery), `Op::clone` 8%, stack traffic (push/pop_int) ~15%, Value clone/drop
  ~5%. 100%-confidence root cause: non-threaded match dispatch + per-op work.
- **Fix #1 DONE (substrate)** — eliminated the per-op `Op::clone`: `exec_op` now takes `&Op` (match
  `*op`; `program` is `&'a` so extract it in both dispatch loops — `mod.rs` main + `closure.rs`
  run_until — to split the borrow; only `Fault`/`IsInstance` arms need `ref`). Measured (interleaved
  A/B, best-of-8, identical load): **fib −10.5%, heap −6.6%**; callgrind confirms `Op::clone` gone,
  instruction count **1,534M → 1,339M (−12.8%)**. Full gate green (build+clippy+fmt+`PHORJ_REQUIRE_PHP=1
  cargo test --workspace`). Modest ~8% as scoped — the 61% dispatch machinery is the next target.

- **Stage 1 diminishing-returns signal (2026-07-05)** — line-level callgrind (debug-info release,
  source-mounted) on the fix#1 binary: biggest *addressable* cost is bounds-checked indexing
  (`slice/index.rs` 6.84% run_main + 3.11% exec_op ≈ 10%), but `forbid(unsafe_code)` blocks
  `get_unchecked`. Tried the two zero-risk wins (pre-reserve stack/frames; guard `do_return`'s
  `handlers.retain`): **measured ~0%** (fib +0.4%, heap −0.3% — the `raw_vec` grow cost was
  warm-up-only, amortized away in steady-state heavy workloads; handler-guard saves nothing with no
  handlers). **Reverted** (Invariant 11 — no perf commit without a measured gain). Cumulative tally:
  fix#1 −10%, safe wins −0%. Frame-context caching predicted only ~3-5% (bounds checks on
  `ip`/`code[ip]` remain) with two-loop spine risk + a gate blind spot (concurrency is quarantined
  from the oracle, yet the coop driver runs these loops). **Curve is flattening → JIT/AOT pivot fork
  surfaced to developer** (the ratified endgame; no bytecode-VM micro-opt under `forbid(unsafe)`
  closes the 26× gap — that needs native codegen).

## Acceptance
- Harness runs the full feature corpus, `runvm` vs release-php+JIT, ns/op, regression-gated.
- Every substrate fix has a measured before/after on the heavy matrix (time + memory).
- No feature ships perf-"done" while its microbench loses to release-php+JIT (or it's a recorded
  ceiling decision).

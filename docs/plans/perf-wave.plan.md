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
- [2026-07-05] AGREED: **`phg benchmark` headline = VM vs release-php+JIT** (tree-walk perf is
  meaningless — it's the oracle). FOLDED INTO the harness step (step 2): benchmark-vs-php + migrating
  `perf-gate.sh` off the tree÷VM machine-independent anchor onto a php baseline are the same effort as
  the per-feature harness. Keep the tree-walk leg reachable as `--vs-oracle` until the harness lands so
  CI keeps its anchor meanwhile.

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

## Step 4 RULED (developer, 2026-07-05) — Cranelift JIT, native proven 3× faster than php
- **SPIKE RESULT (thesis PROVEN):** hand-written native fib(30), `rustc -O`: **`Rc`-boxed-`Value`
  (naive transpile, NO unboxing) = 3.21 ms vs php+JIT 9.6 ms = ~3× FASTER**; native-`i64` (unboxed) =
  ~0 ms (rustc const-folded — the ceiling). **Native codegen beats php+JIT even with phorj's boxed
  Value repr.** Unboxing is a bonus, not a requirement. (`docs/research/jit-aot-design-exploration.md`)
- **RULED: Cranelift JIT** (fast EVERYWHERE — `phg run`/`serve`/`build` all beat php+JIT via one
  runtime-JIT backend). NOT a production-only AOT (that would leave interactive `phg run` on the VM).
  **Requires amending the dependency policy** to admit a codegen crate (currently *explicitly excluded*
  — performance domain) — feature-gated, non-wasm, corosensei-shaped `unsafe` confinement. The formal
  amendment (UNIFIED-SPEC §External-dep-policy table entry + CHANGELOG + wasm feature-gate check) is the
  first gate of the Cranelift build. Reject LLVM. Reject C (transpile→rustc) as the shipped answer
  (production-only).
- **NEAR-TERM WIN (ruled): `phg serve` → VM.** serve currently runs requests via `call_named` (the
  tree-walk INTERPRETER, ~150× slower than php+JIT) — switch to the VM (~25× faster, byte-identical).
  ALSO add `phg serve --tree-walker <file>` (mirrors `phg run --tree-walker`): serve defaults to the
  VM, `--tree-walker` selects the interpreter oracle.
- **Staged Cranelift plan** (post-amendment): emit Cranelift IR for arithmetic/control-flow core →
  Value runtime (boxed first — already beats php) → wire JIT into `phg run`/`serve` (hot-fn compile) →
  AOT-all for `phg build` → unboxing pass for the statically-typed hot paths (the bonus).

## Deferred until the perf goal is met (developer, 2026-07-05)
**Nothing else is tackled until phorj is measurably faster than PHP.** THEN pursue all three
concurrency directions (researched 2026-07-05; the CLI reshape is orthogonal to all of them):
1. **Real shared-memory parallelism leveraging immutability** — phorj's immutable/value semantics =
   no data races = safe cross-thread sharing, a capability PHP structurally lacks. Needs `Rc`→`Arc`
   on the shared `Value` heap (a JIT/AOT value-repr decision — `Value` currently can't cross an OS
   thread, `KNOWN_ISSUES.md:249`) + a Ladder §14 call to drop `run≡runvm` interleaving-identity for
   parallel code (tree-walker becomes a *sequential* semantics reference). The "beats PHP beyond
   speed" story.
2. **Strengthen the cooperative green threads** — finish the deferred `spawn` forms (method / closure /
   overloaded currently run synchronous-degenerate, not truly concurrent); deterministic, stays
   byte-identical, no §14 change.
3. **Evaluate `async`/`await`** — currently none; leaning REJECT (function coloring fights phorj's
   surprise-free philosophy; `spawn`/`Task`/`Channel` structured concurrency is better) — but research
   the comparison when we get there.

## Step 1 (CLI reshape) — execution log
- Code DONE: `phg run`/bare → VM; `phg run --tree-walker` → interpreter; `runvm` command removed
  (main.rs dispatch + help + usage). Tests fixed (cli.rs, build.rs → `run`; the dump-locals test uses
  `--tree-walker` since the rich locals dump is an interpreter-only feature). Docs/examples sweep
  (`phg runvm` → `phg run`, parity prose de-named, README command table) via subagent.
- FOLLOW-UP COMMIT (approved 2026-07-05): **coherent internal rename** — the reshape made the backend
  fn names lie (`cmd_run`=tree-walk while `phg run`=VM). Rename the PAIR: VM → `cmd_run`/`run_program`;
  tree-walker → `cmd_treewalk`/`treewalk_program`. ~30-file mechanical, zero behavior change, its own
  commit after the reshape lands green. (Can't just drop "runvm" — `cmd_run` is taken by the
  tree-walker; must rename both.)

## Step 2 (per-feature harness) — execution log
- MVP DONE: `scripts/microbench.sh` + `bench/micro/<name>.{phg,php}` pairs. phorj VM (`phg run`) vs
  **real release PHP 8.5.7+JIT via `docker run php:8.5-cli`**, best-of-K, self-timed (warmup call +
  timed call), checksum defeats DCE AND gates output-identity. Idiomatic PHP is **hand-authored** (NOT
  transpiled — transpiled carries `__phorj_*` helper weight → false wins; advisor catch). Table +
  `--json`. Starter corpus: intadd, methodcall, objalloc.
- FIRST HONEST PER-FEATURE NUMBERS (VM vs php+JIT, ns/op, best-of-3): intadd ~180 vs 1 (**~154×**);
  methodcall ~280 vs 6 (**~45×**); objalloc ~435 vs 50 (**~9×**). Pattern: pure-dispatch ops show the
  full ~150× gap (what the JIT must erase — corroborates callgrind's 61% dispatch); the more real
  work/op (allocation), the smaller the gap. Confirms empirically: **no bytecode-VM tuning closes
  this — it's the JIT case.**
- FOLLOW-UPS: expand corpus (float/decimal arith, string concat/interp/%-format, list-index, map
  get/put, set, closure-call, match, enum, try/catch — weight toward alloc/builtin-heavy); reshape
  `phg benchmark` headline to VM-vs-php; migrate `perf-gate.sh` off the tree÷VM anchor to a php
  baseline. ⚠ canary: every php micro must report a plausible NONZERO ns/op (0 = JIT ate it).

## Step 3 (VM ceiling test) — DONE, the interpreter ceiling is PROVEN
Ran the `forbid(unsafe)` spike on branch `spike/unsafe-dispatch` (relaxed the crate lint; added
`get_unchecked` on the validated-bytecode hot path — dispatch loops `functions[func]`/`code[ip]`/
`frames[fr]`, plus `Const` const-pool and `GetLocal` stack indexing). Byte-identical to base (0 diffs
on real examples → the number is real, not fast-because-broken). MEASURED (A/B vs base, best-of-8):
**intadd −6.5%, methodcall −3.2%.** Removing EVERY validated bounds check — the single biggest
remaining VM lever — buys **~3–6%**. **Spike REVERTED** (not worth breaking `#![forbid(unsafe_code)]`
for ~5%; that invariant also deliberately reserves computed-goto/JIT dispatch).
**CONCLUSION (airtight):** stack all VM levers — fix#1 −10%, frame-caching ~5%, bounds-checks ~5% —
and the ceiling is **~20% total**, taking intadd from 154× → ~120× slower. The 61% dispatch tax is
structural to interpretation. **No bytecode-VM tuning closes the 9–154× gap. Only native codegen
(JIT/AOT) does.** The perf hunt's "why" is now empirically closed: phorj isn't faster than PHP because
it interprets and PHP+JIT compiles to native — and the VM has been proven near its floor.
**→ Next: Step 4, the JIT/AOT design (the only remaining path).**

## Acceptance
- Harness runs the full feature corpus, `runvm` vs release-php+JIT, ns/op, regression-gated.
- Every substrate fix has a measured before/after on the heavy matrix (time + memory).
- No feature ships perf-"done" while its microbench loses to release-php+JIT (or it's a recorded
  ceiling decision).

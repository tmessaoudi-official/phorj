# Perf Wave Plan — make phorj measurably faster than PHP, per-feature

> Working plan for the G-8 perf mandate. SSOT roadmap stays `MASTER-PLAN.md` (G-8, W6-4, UA-0.10);
> this file is the execution log + decisions for the perf wave. Full diagnosis: memory
> `perf-benchmarking-truth`.

## Decisions Log
- [2026-07-06] AGREED (developer, interactive): **JIT slice 1(b) design LOCKED.** (1) **Native→native
  calls** (Cranelift cross-`FuncId` relocations, incl. self-recursion resolved at
  `finalize_definitions`) — NOT a runtime-call bridge (a bridge taxes every call and fib is
  call-dominated → would lose; the bridge would be throwaway). So **recursive `fib` JITs in 1(b)**.
  (2) **Eager compile-all-eligible** into one program-lifetime `JITModule` (the matched pair for native
  calls: a native call needs the callee compiled+finalized in the same module) — **no user `--lazy`
  CLI flag** (compilation policy is internal, not a user knob; steady-state speed is trigger-identical;
  the real best-perf policy is **hot-count triggering deferred to JIT-3**, matching php+JIT; a dev-only
  env seam can A/B later if needed). (3) **Module lifetime** = program lifetime, `free_memory()` once at
  end — ruled by cranelift source (no `Drop` on `JITModule`; drop leaks the mmap; verified
  `src/backend.rs`). (4) **Operand representation = a memory operand stack in the JIT context** (spill
  operands to a Rust-side `Vec<Value>`; Cranelift emits native control-flow + direct calls to `value.rs`
  kernel helpers) — sidesteps stack-VM→SSA phi/block-param complexity and any short-circuit/ternary
  stack-at-boundary hazard, keeps byte-identity by construction; SSA-register operands + unboxing are
  JIT-5. Removes the ~61% match-dispatch/fetch tax; helper-call + memory-traffic overhead remains →
  **measure fib honestly, do not assume the spike's 3×** (advisor: opaque kernel `call`s don't inline;
  a short measurement is the signal for whether unboxing must come sooner). Build 1(b) as green
  sub-commits: (b1) codegen over the memory stack + comparisons/`Neg`/`SetLocal`/branches/loops
  (unit-tested, unwired) → (b2) native calls + recursion (unit-tested) → (b3) eligibility predicate +
  `phg run` wiring (VM fallback) + JIT-hitting differential examples (loop + recursive fib) + honest
  fib measurement. (b3) is spine-sensitive → fresh advisor byte-identity review before commit.
- [2026-07-06] AGREED (developer, interactive): **JIT marathon execution order LOCKED = Option A —
  ruled staged, breadth-first (boxed Value runtime first, unboxing LAST).** Sequence: (JIT-1) arith/
  control-flow IR emit + `cranelift-jit` dep + `forbid→deny` + `#![allow]` island, wired into `phg run`
  → (JIT-2) boxed `Value` runtime → (JIT-3) hot-fn compile wired into `phg run` + `serve` → (JIT-4)
  AOT-all for `phg build` → (JIT-5) unboxing pass for statically-typed hot paths → (Stage 2) re-measure
  the 12-feature matrix → (Stage 3) per-feature sweep (each straggler beats php+JIT or a §14 ladder
  ceiling call — surfaced, not autonomous) → (Stage 4) mandate gate GREEN (G-8 MET). Rationale
  (developer-endorsed): the spike proved boxed codegen already ~3× > php+JIT, so breadth wins G-8 on the
  widest surface fastest and unboxing self-prunes into Stage-3 stragglers; the gap is uniform (61%
  dispatch tax) so one native-codegen lever lifts all; coverage-gated ordering rejected (microbench
  ratios are load-noisy — that's why the mandate gate blocks only on identity + WIN→LOSS flips).
  Autonomous marathon: each slice a green+measured commit, ratchet re-`--emit`'d per win, stop at §14
  ladder forks (Stage 3) + surface the first `unsafe`-island landing; **never push** (developer pushes).
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

- [2026-07-06] RULED (developer, 2026-07-06): **JIT dependency-policy amendment.**
  Realized while surfacing that this is NOT a table-row add: (a) it introduces phorj's **FIRST
  first-party `unsafe`** — all four current exceptions confine unsafe to *third-party* crates, but a
  JIT's call site (`finalize → transmute(buf→fn ptr) → call`) is unsafe **in phorj's own code**,
  colliding with `#![forbid(unsafe_code)]` (src/lib.rs:3, src/main.rs:4); (b) it **amends dependency-
  policy clause 1**, which currently *excludes* performance/codegen crates (UNIFIED-SPEC:827) and says
  anything outside the listed domains "requires revisiting this policy itself." Fork surfaced to the
  developer: (1) **VM-ceiling first** — small auditable first-party unsafe (bytecode-index bounds
  elision in the hot loop), NO Cranelift, NO policy amendment; measure vs PHP-no-JIT (~9× headroom just
  to match) before the big commitment [recommended — lowest-regret, decouples the reversible small step
  from the irreversible one, matches the prior "explore VM ceiling" agreement]; (2) **full amendment
  now, separate `phorj-jit` crate** — core `phorj`/`phg` keep `#![forbid]` literally intact; cost =
  exposing `Op`/`Value`/chunk internals across a `pub` boundary; (3) **full amendment now, in-tree
  `src/jit/`** — root `forbid`→`deny` + one `#[allow]` island; simpler, tighter coupling, but pierces
  the crate-root forbid invariant.
  RULING (developer, 2026-07-06): full amendment now (VM-ceiling-first DECLINED); layout = option (3)
  **in-tree `src/jit/`**. Rationale: the JIT is a 4th backend coupled to `Op`/`Value`/chunk (inv
  #3/#4/#6), all in the single `phorj` lib crate; dispatch (`src/cli/mod.rs`) + bench/disasm/playground
  compile-paths are lib code, so a separate crate forces those internals `pub` + creates a
  `phorj -> phorj-jit -> phorj` cycle (cleanest fix = a vtable in the perf hot path, self-defeating).
  Mechanism: crate-root `#![forbid(unsafe_code)]` -> `#![deny(unsafe_code)]` + ONE `#[allow(unsafe_code)]`
  island in `src/jit/`, enforced by a CI gate that fails the build if `unsafe` appears outside
  `src/jit/`; admit dependency-policy **domain #7 - native codegen via `cranelift-jit`**, feature-gated
  `jit` (non-wasm; playground stays VM). Ratified amendment files (UNIFIED-SPEC §dep-policy clause 1 +
  admitted-deps table, CHANGELOG, ci.yml gate) to be written WHEN the JIT work starts - not now.

- [2026-07-06] AGREED (developer) — **A1 measurement-harness reshape, scoped after discovery.**
  DISCOVERY: `scripts/microbench.sh` ALREADY is the honest per-feature harness (phorj VM vs
  release-php+JIT via `docker run php:8.5-cli`, ns/op, checksum output-identity gate, WIN=VM faster).
  Corpus = 11 pairs in `bench/micro/`. So A1's hard part exists. **Perf-gate anchor RULING (reframes
  the earlier "migrate off tree÷VM" ruling, which predated the microbench discovery):** KEEP
  `perf-gate.sh`'s tree÷VM `vm_speedup` as the **machine-independent VM-regression backstop**
  (relabelled: VM-health, NOT a php claim — `perf-gate.sh` header + `bench/baseline.json` `_comment`
  DONE 2026-07-06) + ADD microbench WIN-count as a SEPARATE G-8 mandate gate. Rationale: perf-gate
  runs on a noisy shared `ubuntu-latest` runner (ci.yml:68) where tree÷VM's machine-independence is
  load-bearing; microbench needs docker; the two metrics measure different things — keep both.
  **Remaining A1 (not yet done, needs docker + a cold release rebuild — `target/` was cleaned):**
  (a) `bench/micro/trycatch.{phg,php}` micro (needs NATIVE phorj try/throw/catch that runs on the VM —
  `examples/interop/exceptions.phg` is PHP-only/E-FOREIGN-RUNTIME, find/author a native throwable);
  (b) `phg benchmark` headline → VM-vs-php primary, tree÷VM behind `--vs-oracle` (MUST preserve the
  `vm_speedup` JSON field — `perf-gate.sh:43` reads it), keep local-`php` `--vs-php` as indicative;
  (c) wire the microbench WIN-count mandate gate (a `microbench.sh --gate` mode + baseline, then a CI
  job on the docker-capable lane, or pre-push/local to keep CI docker-free — sub-decision open).

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
- **JIT gate-1 (dep-policy amendment + scaffold) DONE (2026-07-06)** — the ruled FIRST gate of the
  Cranelift build shipped: UNIFIED-SPEC §dep-policy admits **domain #7 (native codegen)** with the
  clause-1 "performance-excluded" carve-out + an admitted-deps table row (`cranelift`, *not yet in
  tree*); CHANGELOG entry; a CI `unsafe-island` job (fails if `allow(unsafe_code)` appears outside
  `src/jit/` — arms for the `forbid`→`deny` downgrade); and an empty `src/jit/mod.rs` scaffold (crate
  still `#![forbid(unsafe_code)]`, unsafe-free, compiles clean). NEXT (fresh session — the heavy
  marathon): add the `cranelift-jit` crate + `forbid`→`deny` + the `#![allow]` island + first Cranelift
  IR emit for arithmetic/control-flow, wired into `phg run`.
- **JIT-1 leak fix DONE (2026-07-06, `c780540`)** — `JITModule` has NO `Drop` (verified cranelift-jit
  0.133 `src/backend.rs`); `compile_and_run` now calls `unsafe free_memory()` after the entry returns
  instead of leaking the code mmap on `drop`. Gate green (`-p phorj --features jit` = 1795).
- **1(b) build-notes (VM seams captured 2026-07-06 — mirror EXACTLY for byte-identity):** the memory-
  operand-stack design's helpers must reproduce these VM `exec.rs` arms/kernels: `Neg` int → `value::
  int_neg` (checked; `i64::MIN` → "integer overflow"), Float → `-x`; `Not` Bool → `!b` (else "cannot
  apply ! to {type}"); `Eq`/`Ne` → `Value::eq_val` (value.rs:489, pub); `Lt/Gt/Le/Ge` → `vm::compare`
  (src/vm/mod.rs:467 — `Result<bool,String>`; maps `value::compare_ord`; NOT pub → either `pub(crate)`
  it or replicate its exact op→ordering→bool + None-handling); `GetLocal(slot)`/`SetLocal(slot)` index
  `stack[slot_base+slot]` (VM grows the stack; there is NO static slot-count field on `Function` —
  chunk.rs:476 — so the eligibility scan sizes the JIT frame's locals region as `1 + max(slot)` over
  GetLocal/SetLocal); `JumpIfFalse` pops, `Bool(false)`→jump / `Bool(true)`→fall-through / else "expected
  bool, found {type}"; `Jump(t)` sets ip=t. `Call(idx)`/`Return`: mirror `exec.rs:431`/`do_return`
  (shared value stack + `slot_base`; native→native = Cranelift call to the callee's declared `FuncId`,
  args pre-pushed on the shared stack). Fault propagation across native frames: each `Call` site checks
  the callee's returned status (like the arith null-check) and branches to the fault-exit.
- **JIT-1 codegen slice (a) DONE (2026-07-06)** — the boxed-via-kernels substrate shipped, gate-green,
  unpushed. `cranelift`/`cranelift-jit`/`cranelift-module` 0.133 behind the non-default `jit` feature
  (non-wasm; verified building on the 1.96.0 pin). **Unsafe island landed:** `forbid`→`deny` on both
  crate roots + the single `#![allow(unsafe_code)]` in `src/jit/mod.rs`. `src/jit::compile_and_run`
  lowers a **default-deny int-arith leaf subset** (`Const`(int)/`GetLocal`/`AddI`/`SubI`/`MulI`/`DivI`/
  `RemI`/`Return`, straight-line) to native code via Cranelift, run through `finalize→transmute→call`;
  arithmetic dispatches the single-sourced `value.rs` kernels, so overflow/div-zero faults are
  byte-identical to the VM by construction (Invariant 4). Anything else → `JitError::Unsupported` (the
  seed of the eligibility predicate). 4 tests (`--features jit`): value ≡ VM oracle for int arithmetic;
  overflow + divide-by-zero surface the exact kernel strings; a non-int function is default-denied. NEW
  CI `jit` job builds/lints/tests `-p phorj --features jit` — the `--workspace` gate never compiles the
  feature, so without it src/jit/ would rot unverified (a structural false-green; advisor-caught).
  **⚠ The full gate is now `--workspace` (PHP oracle) PLUS `-p phorj --features jit` — a green that
  skipped the feature did NOT exercise the JIT.** NOT wired into `phg run`: commit (b) does the wiring
  behind the eligibility predicate + control-flow (branches/loops for fib) + a differential example
  that provably hits the JIT (avoids the run≡runvm false-green). **No perf claimed** — unwired and
  unmeasured; the spike's ~3×-over-php+JIT is a hypothesis for the wired path, measured under `phg run`
  in (b) (Invariant 11). Marathon order = Option A (Decisions Log 2026-07-06).
- **A1 trycatch micro DONE (2026-07-06)** — `bench/micro/trycatch.{phg,php}` added (native
  `class Odd implements Error` + `throws`/`try`/`catch`; output-identical checksum `8999994`).
  Corpus now **12**. Honest matrix (docker `php:8.5-cli` release+JIT, this host): **ALL 12 LOSE** —
  trycatch VM 356 vs php+JIT 167 = **0.47×** (closest-to-win); others 0.01–0.11×. Confirms G-8 is
  missed across the board (the JIT is the lever). ⇒ the **mandate gate must be a RATCHET** (baseline
  current per-feature ratios in `bench/micro-baseline.json`, fail on regression / flip WIN→LOSS), NOT
  a "require WIN" gate — else it red-fails on day one.
- **A1 mandate gate DONE (2026-07-06)** — `scripts/microbench-gate.sh` (+ `--emit` + a
  `MICROBENCH_GATE_JSON`/`MICROBENCH_BASELINE` docker-free test seam) gates against
  `bench/micro-baseline.json` (12 features). ⚠ DESIGN CORRECTED BY EMPIRICAL EVIDENCE: the first cut
  ratcheted on absolute VM ns/op (ceiling = baseline*1.7) — it FALSE-FAILED under machine load
  (stringconcat/trycatch swung 3–4× at load avg ~7, NO code change). Absolute native-VM-vs-docker-php
  ns/ratio is too noisy to BLOCK on. So the gate now BLOCKS only on the two LOAD-INSENSITIVE signals:
  (1) output-identity break (VM≠php checksum — bench micros aren't in the differential, so this is
  their only parity check), (2) WIN→LOSS flip (a feature whose baseline ratio ≥1 now <1 — the real
  G-8 ratchet: keep beating php once you beat it). Ratio deltas are REPORTED, not blocked. VM-perf
  regression stays covered ROBUSTLY by `perf-gate.sh` (same-process tree÷VM, load-immune) — the two
  gates are complementary. All 12 currently LOSE → the gate today enforces identity + arms the flip
  ratchet for when the JIT lands wins. Self-skips (exit 0) on absent docker/release-binary. Wired into
  pre-push after the oracle. Verified: 3 seam logic-tests (no-flip→pass, flip→fail, identity→fail) +
  real baseline → PASS. RATCHET: re-`--emit` after a JIT win. ONLY remaining A1 bit: the cosmetic
  `phg benchmark` headline reshape (move tree÷VM behind `--vs-oracle`) — low priority.
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
  tree-walk INTERPRETER) — switch to the VM (faster + byte-identical; measured ~2.3× lower serve
  latency — see the execution log below; the pre-build guess was "~25×", the fib figure, wrong for a
  native-call-heavy handler).
  ALSO add `phg serve --tree-walker <file>` (mirrors `phg run --tree-walker`): serve defaults to the
  VM, `--tree-walker` selects the interpreter oracle.
- **Staged Cranelift plan** (post-amendment): emit Cranelift IR for arithmetic/control-flow core →
  Value runtime (boxed first — already beats php) → wire JIT into `phg run`/`serve` (hot-fn compile) →
  AOT-all for `phg build` → unboxing pass for the statically-typed hot paths (the bonus).

## Step "serve → VM" (near-term win) — execution log (2026-07-05, autonomous)
Chosen as the bounded autonomous slice after the developer push (Cranelift is a multi-session marathon
gated on the dep-policy amendment; serve→VM is ruled, self-contained, ships a real relative win, and
builds the VM `run_entry` — call-by-name + return-value capture — the JIT will need anyway).
- **Verified facts** (before design): the one interpreter call-site is `serve.rs:111`
  `call_named(prog,"respond",[bytes])`. Free functions are compiled FIRST in `functions`, bare-named
  (no package mangling) → `respond` is findable by name. `Op::Return` already stashes the entry frame's
  return `Value` into `exit_value` when `frames.len()==1` — so a VM entry needs only: push args → push
  entry Frame → run loop → read `exit_value`+`out`. `Program` and `Ty` are both `Send+Sync` (no `Rc`)
  but `BytecodeProgram` holds `Rc` (class layouts) → NOT `Send` → cannot be shared across worker
  threads; each worker must compile its own from the shared `Arc<Program>`.
- **Design (2 commits):**
  1. VM `run_entry(entry, args) -> (Value, String)` + extract the shared dispatch loop into
     `run_to_completion(&mut self)`; `run_main` becomes a thin wrapper (byte-identical). `run_entry` is
     NON-cooperative — mirrors `call_named` (which runs `run_call` directly), so run≡runvm holds on the
     serve path; do NOT copy `cmd_run`'s `uses_concurrency` coop branch. Verified by full differential
     (proves `run_main` unchanged) + a unit test asserting `run_entry` ≡ `call_named` for a sample fn.
  2. serve cutover. serve.rs stays compiler-free: it takes a `HandlerFactory` (a `Send+Sync`
     `Fn() -> Box<dyn FnMut(&[u8]) -> Result<(Value,String), Diagnostic>>`) the CLI supplies; each
     worker (and the single-thread path) calls it once to build its own non-`Send` handler that OWNS
     its per-thread compiled `BytecodeProgram` (VM) or an `Arc<Program>` clone (interp). The factory,
     built in `cli::serve_program`, captures `Arc<Program>`(checked+expanded)+`Arc<reified>` and does
     `compile_with` inside (per worker) → no `Rc` crosses a thread, no compiler import in serve.rs.
     `serve --tree-walker` selects the interp factory. Entry resolution: single free `respond` by name
     (arity-guarded); an overloaded `respond` is unsupported on the VM path (errors clearly — use
     `--tree-walker`) — degenerate config, documented, no silent divergence.
- **Validation** (serve is OUTSIDE the differential — the gate won't catch a VM≠interp break): new
  dual-backend tests in `tests/serve.rs` drive a fixed request set through BOTH engines asserting
  byte-equal response bytes (normal path + production 500; the dev error page is explicitly outside the
  byte-identity value contract — not gated). Plus measure per-request latency both backends (Inv-11 /
  G-8) and report before/after — framed honestly: ~150×→~25× slower than php+JIT (a real relative win,
  NOT perf-mandate completion; the mandate needs the JIT).
- **SHIPPED — measured (release binary, keep-alive socket, representative parse+route+build `respond`,
  best-of per-request over 3590 samples):** VM (default) **17.1 µs median/request** (best 15.2) vs
  tree-walker **39.6 µs median** (best 33.3) = **~2.3× faster end-to-end**. The ratio understates the
  handler-compute gain — the fixed loopback socket round-trip is inside both numbers. Two commits:
  `caabfc4` (VM `run_entry`) + the serve cutover (this one). Gotchas hit + resolved: (1) the VM
  compiler requires an entry, but serve/web programs legitimately have no `main` (interp `call_named`
  never needs one) → new `ast::synth_empty_main()` injected in `vm_factory` (inert; never invoked). (2)
  `MAX_REQUESTS_PER_CONN=100` closes a keep-alive socket after 100 requests (a benchmark-client gotcha,
  not a serve bug). Still ~25× slower than php+JIT — the mandate is unmet until the JIT; serve→VM is
  the right infra + a real relative win.

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

## Step 2 corpus expansion (2026-07-05, autonomous — developer chose this over the JIT amendment)
Expanding `bench/micro/` beyond the 3 starter pairs (intadd/methodcall/objalloc) toward the plan's
list — weighted alloc/builtin-heavy. Each pair's `.php` mirror MUST produce a byte-identical checksum
(the harness output-identity gate). Constraints: keep every accumulator well under 2^63 (Phorj int is
CHECKED — overflow FAULTS; PHP wraps to float — so an overflow is both a fault AND a checksum break);
fold any float work into an INT checksum (truncate) to dodge float-format divergence; int→string in
interpolation is identical across both legs (safe). Validated per-pair by running `phg run x.phg` vs
local `php-8.5.8 x.php` and diffing the checksum field (Docker only needed for the perf ratio).

**SHIPPED — corpus now 11 micros** (added 8: `floatarith`, `listindex`, `mapget`, `match`, `interp`,
`stringconcat`, `closurecall`, `enum` — alongside `intadd`/`methodcall`/`objalloc`). Every pair's
checksum is byte-identical VM≡php (harness output-identity gate — all 11 pass, no mismatch). First full
table (VM ns/op vs release-php+JIT via Docker, best-of-3, noisy host) — **every feature LOSES**, the
honest G-8 picture: closurecall ~0.37× (closest), objalloc/enum/interp ~0.1–0.16×, and the cheapest
ops (intadd, mapget, methodcall, listindex, floatarith, match, stringconcat) ~0.01–0.07× (php+JIT
near-free on those — corroborates callgrind's 61%-dispatch tax). This is the per-feature baseline the
JIT must erase; it IS the JIT's measurement backbone. **Canary caught (6C):** `stringconcat` +
`listindex` first shipped with loop-invariant/precomputable operands → php+JIT hoisted them to 1 ns/op
(measuring NOTHING; the checksum gate can't detect this — the plan's "php micro must report nonzero
ns/op" canary does). Fixed to index-varying / data-dependent operands (`15124eb`); php+JIT now reports
plausible 16/6 ns/op. `enum`'s php mirror is the leanest tag-`match` (PHP has no payload enums → the
hardest baseline). REMAINING follow-ups (separate, more invasive — deferred): `trycatch` micro;
reshape `phg benchmark` headline to VM-vs-php; migrate `perf-gate.sh` off the tree÷VM anchor.

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

# Perf Wave Plan — make phorj measurably faster than PHP, per-feature

> Working plan for the G-8 perf mandate. SSOT roadmap stays `MASTER-PLAN.md` (G-8, W6-4, UA-0.10);
> this file is the execution log + decisions for the perf wave. Full diagnosis: memory
> `perf-benchmarking-truth`.

## Decisions Log
- [2026-07-09] 🌙 **OVERNIGHT AUTONOMY DIRECTIVE (developer, going offline until morning).** Standing
  orders, override the "stop on fork" rule: **(1) NO STOP until the developer returns** — work
  continuously, rely on auto-compaction, keep everything durable (commit each green slice, keep this
  plan + MEMORY current every slice so nothing is lost). **(2) NEVER ask** — design forks / §15
  adjudication questions are PARKED here as `PENDING-DECISION` (minimal failing program + option
  previews per §15) and I move to the next buildable item; do NOT block. **(3) Scope:** finish the perf
  sequence (floats → §15 jit-default flip → strings), THEN take the CLEAR (ruled, unblocked) MASTER-PLAN
  sections + MORE SUGAR. **(4) HARD BAR:** every feature must be BETTER than PHP, or at least EQUAL —
  never worse. Security + typing + error-detection + every non-PHP feature are non-negotiable (§14
  ladder: surface+PENDING, never silent downgrade; no perf win at their expense). **(5) Perf claims**
  only vs a FRESH docker php:8.5-cli+JIT baseline, gate WIN/LOSS not magnitude ([[perf-benchmarking-truth]]).
  Advisor (the reviewer tool, not the developer) stays IN the loop for spine-sensitive slices; a 5-round
  advisor cap → park the finding as PENDING and continue (don't ask).
- [2026-07-09] 🏁 **MARATHON START (developer: "very big perf wave, finish all of it") — full autonomous
  run of the queued sequence ovf-spec → floats → §15 jit-default flip → strings; AUTO-COMMIT each green
  slice, NO push (developer pushes). Stop only on a genuine §14/§15 fork or a 5-round advisor cap.**
- [2026-07-09] ✅ **ovf-spec CODEGEN SHIPPED (`2b77b9b`, gate-green, unpushed).** Speculative wrapping
  int arith + sticky-flag Variable + back-edge guard + code-5 VM-redo, exactly as the advisor-3C design.
  45 jit tests (5 new end-to-end `ovf_spec_*` + 8 re-pointed funnel tests) + full workspace (1556 lib +
  differential + conformance-minus-decimal + 12 + 27) + clippy(both) + fmt + release, green. INVARIANTS
  #13 records the coupling MUST-CHECK. NEXT: honest re-measure intadd vs FRESH docker php+JIT (advisor
  predicts it may NOT flip — back-edge guard adds ~1 branch/iter to tight single-accum loops; that is
  the RANGE/no-overflow-analysis trigger, NOT a reason to weaken the guard). ⚠ **PRE-EXISTING RED (NOT
  ovf-spec, reproduced on clean HEAD via stash):** the decimal conformance PHP-oracle test fails —
  `bcmul()` undefined because php-8.5.8 loads bcmath as a SHARED ext, and the harness runs php `-n -d
  extension=bcmath` WITHOUT an `extension_dir`, so the `.so` never loads. See PENDING-DECISION below.
- [2026-07-09] 🅿️ **PENDING-DECISION: bcmath conformance-oracle gap (pre-existing, blocks the FULL
  `PHORJ_REQUIRE_PHP=1` oracle on decimal).** `tests/conformance.rs::php_n_args` runs php with `-n`
  (no ini) and, when bcmath is not a BUILT-IN (static) extension, adds `-d extension=bcmath` — but this
  phpbrew php-8.5.8 build has bcmath as a shared `.so`, which needs `-d extension_dir=<dir>` to load
  under `-n`. Repro: `PHORJ_REQUIRE_PHP=1 cargo test -p phorj --test conformance` → `Call to undefined
  function bcmul()`. Options for the developer: (A) harness adds `-d extension_dir=$(<oracle-php> -i |
  grep extension_dir)` when falling back — RECOMMENDED, keeps `-n` determinism, one-line-ish fix; (B)
  rebuild php-8.5.8 with `--enable-bcmath` static (toolchain change, heavier); (C) quarantine the
  decimal transpile-conformance case under a "no-bcmath" skip with a loud SKIP (loses oracle coverage
  for decimal). I did NOT self-rule (it touches the PHP-oracle spine); for the overnight run I treat
  this ONE failure as a known HEAD baseline and gate later slices on "no NEW oracle failures vs it".
- [2026-07-09] 🔬 **ovf-spec ADVISOR-3C REFINEMENT (fresh context, pre-codegen) — Concern A confirmed
  BLOCKING; back-edge sticky guard added to the minimal slice.** The advisor killed the "speculative
  wrapping non-termination is only pathological/astronomical" rationalization with a trivial eligible
  counterexample: `function spin() -> int { mutable int i = 1; while (i != 0) { i = i * 3; } return i; }`
  — VM (checked) faults overflow in ~40 iters; native wrapping `3^k mod 2^64` is always odd, never 0 →
  the `i != 0` back-edge never falls false → **infinite hang** (never reaches Return, never checks sticky,
  never redoes). A byte-identity spine violation ("identical failure behaviour"), not a slowdown. ROOT:
  the unboxed subset admits a loop whose exit test reads a speculatively-wrapped value (SetLocal @1387 +
  back-edge Jump @1439 + Ne @1357, all widen-1). **FIX (mandatory, not optional hardening): sticky-check
  at EVERY back-edge on EVERY compiled fn** — at `Jump(t)`/`JumpIfFalse(t)` with `t <= ip`, emit
  `fault_if(sticky_nonzero, 5)` before the branch. Bounds native to ≤1 partial iteration past the first
  overflow → redo on VM → true fault in correct order. **PERF honesty (carry into measurement):**
  recursion (fib-shaped, depth-bounded, no back-edge) stays fully branchless → clean win; a tight
  single-accumulator loop (intadd) gets ~1 branch/iter back → **ovf-spec may NOT flip intadd** — that is
  the range/no-overflow-analysis trigger (plan line ~48), NOT a reason to weaken the guard. **Plumbing
  (advisor-confirmed sound):** code 5 → `run_unboxed` returns `JitRun::Fault(REDO marker)`; the b3b hook
  (`exec.rs:473`) already redoes on ANY `Fault` → VM renders the true fault+line. `run_unboxed`'s ONLY
  production caller is `exec.rs:464` (the marker string never reaches a user — asserted in a comment).
  `compile_and_run` is BOXED, never sees code 5 → its named tests stay green untouched; the boxed guards
  at tests.rs:673-750 lock the ORACLE but don't exercise the rewrite → **coverage gap closed by NEW
  end-to-end tests** (`cmd_run` vs `cmd_treewalk`, `Err==Err`, modelled on
  `jit_stack_overflow_threshold_matches_the_oracle`) incl. the hang counterexample (asserts eligibility
  so it can't false-green via silent VM skip). Correct design bits (don't second-guess): Neg-MIN
  branchless (`ineg` doesn't hardware-trap, unlike `sdiv`) via `is_min`→sticky OR; Div/Rem KEEP both
  branches (zero + MIN/-1) redirected to exit(5); sticky = Cranelift Variable seeded 0 in entry (required
  for the loop-header phi); sticky-select at every Return arm. `3C round 1 → advisor: clean`.
- [2026-07-08] 🔬 **ovf-spec GROUNDING + DESIGN REFINEMENT (fresh code-read of `src/jit/mod.rs`
  `build_body_unboxed`, lines ~1181–1451) — BEFORE the sketch is implemented, advisor-3C pending.**
  Confirmed the current unboxed path faults IMMEDIATELY at each op via `fault_if(cond,code) → fault_exit`
  → returns `(0,code)` (1 ovf / 2 div-zero / 3 mod-zero / 4 stack-ovf), in exact execution order — THAT
  is what makes it VM-byte-identical (first fault wins, same order as the VM's per-op checked arith).
  **BYTE-IDENTITY BUG in the original sketch (found during grounding):** the sketch defers overflow to a
  sticky flag but keeps div-zero/mod-zero/stack-ovf as IMMEDIATE direct codes 2/3/4. If a (now-deferred)
  overflow PRECEDES a div-by-zero in execution order, the VM faults at the overflow FIRST, but the sketch
  would return the div-zero code → WRONG fault string. Fault ORDERING is parity-affecting.
  **REFINED DESIGN (supersedes the sketch's "div-zero returns 2/3 directly"):** make EVERY fault exit with
  **code 5 = redo-on-VM**. Overflow (AddI/SubI/MulI: wrapping `iadd/isub/imul` + OR the `*_overflow` carry
  into a sticky-flag Variable, no branch; Neg-MIN + Div/Rem-MIN÷-1: OR into sticky BUT still branch since
  MIN÷-1 hardware-traps → those branch to `exit(5)`) → at `Return`, sticky≠0 ⇒ `(0,5)` else `(value,0)`.
  Div/rem ZERO + Op::Call stack-overflow → still branch (mandatory: hardware trap / unbounded recursion)
  but to `exit(5)`, NOT their own code. Op::Call callee `ccode≠0` ⇒ propagate as `exit(5)`. Net: the
  unboxed path returns ONLY `(value,0)` or `(_,5)`; codes 1/2/3/4 vanish from it; the **VM redo is the
  single source of fault truth** — reproduces the true first fault in correct order (sound: eligible ⇒
  side-effect-free ⇒ deterministic re-run; also handles transient/cancelling overflow — wrapped success
  with sticky set still redoes → VM faults at the real overflow op). `JitRun` gains `RedoOnVm`; the b3b
  `Op::Call` hook (vm/exec.rs) maps code 5 → run the callee on the VM (reuses the existing VM-fallback
  path). TDD proof obligations: overflow-mid-loop → same fault+line as VM; div-zero-AFTER-overflow →
  OVERFLOW fault (ordering!); pure div-zero (no prior ovf) → div-zero fault; MIN÷-1; neg-MIN; non-overflow
  loop → wrapping==checked value. **STILL spine-sensitive; advisor byte-identity review is the real gate.**
  **STATUS 2026-07-08:** design certified (advisor-3C clean); the two ORDERING/transient guards landed
  green (`4867b2d`, `src/jit/tests.rs`). CODEGEN deferred to FRESH context. **IMPL CHECKLIST (advisor-3C):**
  (1) `RedoOnVm` resolved INTERNALLY at the two entries (`compile_and_run` + the b3b `Op::Call` hook both
  re-run the callee on the VM) so the PUBLIC `JitRun` stays `Value`/`Fault` — existing entry tests
  (`jit_overflow_faults…`, `jit_division_by_zero…`) must stay green. If `JitRun` gains a variant anyway,
  `grep 'JitRun::'` every match (tests/benchmark/disassemble/playground) — no `_` arm (Op-variant coupling).
  (2) Seed the sticky Variable to 0 on the entry block, all paths (like the filler-0 locals seed) — an
  unseeded read = verifier fail / spurious redo. (3) `sadd_overflow`'s result[0] IS the wrapped value —
  push it, OR result[1] into sticky; DELETE the `fault_if`, do NOT add a separate `iadd`. Keep the diff
  tiny. (4) Verify the redo re-runs the callee from the ORIGINAL args (the hook must not have consumed/
  mutated the operand-stack args before deciding to redo). (5) **COUPLING INVARIANT (write into
  INVARIANTS.md):** every faulting op in the unboxed subset MUST set sticky or exit(5); a future subset
  widening to a new faulting op (shift, checked `as`, pow) that forgets this = a SILENT byte-identity P0
  (wrapped success masks a VM fault) — same class as the Op-variant / CTy-operand MUST-CHECKs.
  **PERF (the whole point):** the sticky OR is a loop-carried dependency (phi at the loop header, serial
  chain). After green, re-measure `intadd` vs a FRESH docker `php:8.5-cli`+JIT baseline (do NOT reuse a
  stale one — that trap already bit once this session); gate WIN/LOSS not magnitude. If intadd still
  LOSES, the sticky chain is the prime suspect → next lever = accumulate-at-loop-exit or range/no-overflow
  analysis, NOT more widening.
- [2026-07-08] ✅ **AGREED (developer) — commit-gate speed: root-caused to opt-level-0 + 2 monster
  sweeps; NOT test-less-often. FINAL: deps-opt2 + workspace-opt1 + nextest + speed-tier + `--features jit`.**
  Measured pain: per-commit `cargo test` = **126s SERIAL** (8 cores). Diagnosis (Rule 14 applied to test
  perf — the initial "nextest → ~30s" estimate was WRONG and retracted):
  • nextest alone = 126s→100s (1.26×) — Amdahl-capped by ONE 100s test.
  • **The whole suite MINUS 2 tests = 8.0s.** The cost is 2 workspace-compute-bound monsters:
    `format::every_repo_phg_formats_idempotently_and_safely` (formatter dogfood over every repo `.phg`,
    ~100–180s, variable) and `runtime::shipped_manual_example_runs_on_both_backends` (one impure
    `fib(30)` example on both backends, ~35–69s; `differential.rs` already SKIPS it — impure).
  • argon2 (24.8s) + registry (27.4s) were **opt-level-0 artifacts** — Cargo.toml had NO `[profile]`,
    so every dep + workspace crate built unoptimized. Fixed by `[profile.dev.package."*"] opt-level=2`
    (deps: near-free, rarely recompiled) + `[profile.dev] opt-level=1` (workspace: cheap tier, speeds
    interpreter/formatter dispatch; fast tier 27.5s→8s). opt-level is behaviour-invariant; release
    profile untouched (shipped binary + correctness gate unchanged). Reversible in one line if the JIT
    compile loop feels sluggish (developer chose opt1 over reverting — the 8s is measured, the 395s
    rebuild is sunk, opt1 is milder than the opt3 already shipped).
  • **Gate gap fixed:** `jit` is NOT a default feature, so the old hook (`cargo nextest run`) never
    tested the JIT. Per-commit now runs `--features jit` → ovf-spec's TDD is gated per-commit.
  DESIGN: per-commit = `fmt --check` + `nextest --features jit` fast tier (exclude the 2 monsters);
  pre-push = full `nextest --features jit` (incl. the 2 monsters) + clippy (moved here — lint batches
  cleanly, was only 0.13s warm) + PHP oracle (8.5) + microbench-gate. Net **126s → ~9s/commit (~14×)**,
  full coverage retained at the pre-push boundary the developer already hits every ~10-20 commits.
  Rejected the "run pre-commit every 10-20 commits / write-but-don't-run tests" proposal: bisection cost
  is linear in the deferral window, correctness regressions don't bulk-fix (they interact), solo-direct-
  to-master makes these hooks the ONLY gate, and unrun TDD tests can be tautological (Rule 7).
- [2026-07-08] ✅ **RULED (developer, int-overflow fork) — NEXT BUILD SLICE = "ovf-spec": speculative
  unchecked int arithmetic + sticky-overflow-flag + VM-redo-on-overflow.** Resolves why intadd LOSES
  (per-op `*_overflow`+branch) WITHOUT sacrificing phorj's integer-overflow detection (the feature PHP
  lacks — PHP silently promotes to float). Mirrors PHP's own JIT deopt, adapted to phorj's fault
  semantics, and fits the existing side-effect-free / VM-fallback model.
  **DESIGN SKETCH (for the fresh-context build — advisor-review before commit):**
  - **Codegen (`build_body_unboxed`):** replace `AddI/SubI/MulI` per-op `*_overflow`+`fault_if` with
    WRAPPING `iadd`/`isub`/`imul` PLUS OR-ing each op's overflow bit into a sticky-flag Variable (no
    per-op branch). `Neg` MIN and `Div/Rem` MIN/-1 → fold into the sticky flag too. **KEEP the div/rem
    ZERO check as a real per-op branch** (hardware traps on divide-by-zero — cannot speculate it; rare,
    so the branch is cheap).
  - **Exit:** at every `Return`, if the sticky flag ≠ 0 → return a NEW code (e.g. 5 = "speculation
    overflowed, redo on VM") instead of `(value,0)`; else `(value,0)` as today.
  - **`run_unboxed` + `Op::Call` hook (b3b):** code 5 → a new `JitRun::RedoOnVm` (distinct from
    `Fault`); the hook re-runs the callee on the VM, which does per-op CHECKED arithmetic and produces
    the EXACT byte-identical fault (phorj faults per-op, so redo is always correct even for a
    transient/cancelling overflow). Sound because JIT-eligible ⇒ side-effect-free (re-run is safe — the
    same invariant b3b already relies on).
  - **Byte-identity proof obligation (TDD):** a loop that overflows mid-iteration → RedoOnVm → SAME
    fault+line as the pure VM; a non-overflowing loop → wrapping==checked value; MIN/-1 div & rem;
    neg-MIN; div-by-zero still faults DIRECTLY (not via redo).
  - **Then re-measure intadd** — target LOSS→WIN (per-op branches gone, feature intact).
  ⚠ **SPINE-SENSITIVE → FRESH CONTEXT** (fault-semantics + Op::Call ABI change; advisor byte-identity
  review is the real gate). AFTER ovf-spec: floats (f64, no fault-check tension) → §15 jit-default flip
  → strings/collections.
- [2026-07-08] AGREED (developer, §15 + next-direction, post-widen-1 re-measure):
  **(A) jit-on-by-default in stock `phg` = Option 3 — on by default + a `--no-jit` runtime escape**
  (fail-closed to VM, byte-identical; adds Cranelift + the unsafe-island to the DEFAULT non-wasm build;
  wasm/playground stay VM). Rationale: identical hot path to plain on-by-default, plus a free field
  escape + A/B lever, and it makes the fast path the default so every future subset-widening auto-ships
  to users. **(B) Execution order = gate-fix → §15 flip → floats → strings:** (1) fix `microbench.sh`
  resolution (the gate currently LIES — intadd reads 1.00× LOSS, is 4.3× WIN; honest gate is a
  prerequisite for trusting every later verdict); (2) ship the jit default (A); (3) float-loop unboxed
  subset (`Kind::Float` + native `fadd`/`fsub`/… , f64 in the SSA ABI — a scoped mirror of the int
  path, flips `floatarith`); (4) strings/collections (the big lever — webish/stringconcat/mapget — needs
  HEAP/boxed values in the unboxed path → large fresh-context design + likely §14 ladder fork).
  **(C) ⛔ STANDING CONSTRAINT (developer, emphatic): the perf hunt must NOT sacrifice any phorj
  stronghold** — strong static typing, real compile/interpret-time error detection, or ANY phorj feature
  that PHP lacks. If a perf slice would compromise one, STOP and ask (do not self-rule) — same gate as
  §14/§15. (The JIT already honors this: it runs AFTER the checker, and eligibility is a runtime
  fast-path that fails closed to the fully-checked VM — zero type/error-detection surface change.)
  Floats + strings are spine-sensitive → each gets a FRESH context (advisor byte-identity review).
- [2026-07-08] 🔧 **CORRECTION — widen-1 does NOT flip intadd to a WIN (false-baseline error retracted).**
  An earlier entry here claimed "intadd ~4.3× WIN"; that was WRONG — it compared phorj-jit (~6.6M ns)
  against an anomalously SLOW php baseline (28.28M ns) from one loaded manual `docker run`. The
  `perf-benchmarking-truth` trap exactly: never trust a single php baseline; ratios swing 3-4× at load.
  **HONEST re-measure (after the microbench.sh total-ns fix, jit binary vs docker php:8.5-cli+JIT):**
  intadd php+JIT **5.24M ns** vs phorj-jit **9.57M ns** = **0.55× LOSS** (best-of-3); confirmed best-of-10
  on a loaded box (php 13.18M < phorj 19.12M, same direction). **intadd JITs correctly and is
  byte-identical** (was ~0.01× on the pure VM → the JIT is ~30-50× faster than the VM, delivery proven
  via `hits>0`), **but still LOSES to php+JIT ~0.6×.** ROOT CAUSE (hypothesis, [Inferred]): phorj emits a
  per-op overflow check (`sadd/ssub/smul_overflow` + branch to fault_exit) on EVERY `AddI/SubI/MulI`;
  php's tracing JIT specializes and elides them. So the real next perf lever is **overflow-check
  elision** (range/provably-non-overflowing analysis), NOT more subset-widening. Matrix now (honest):
  **1 WIN (fibrec ~1.55×) / 13 LOSS.** widen-1's VALUE stands: it correctly widened the unboxed subset
  to loops (byte-identical, tested, a prerequisite for any int-loop perf) — the perf mandate for intadd
  is simply not yet met. ✅ microbench.sh FIXED (total-ns; the fix revealed this truth — the floored
  `1.00×` was hiding a LOSS, not a win). ⛔ HARD MANDATE: intadd LOSS = a P0-perf item (overflow-check
  elision is the fix). **RE-OPENS the next-direction order (surface to developer):** int-loop overflow-check elision
  is TANGLED with Invariant 4 (fault parity) — the per-op `*_overflow` checks reproduce `value.rs`'s
  checked-int faults byte-identically, so they can't just be dropped; a real int-loop win needs
  range/no-overflow analysis (hard) or a cheaper parity-preserving check idiom. **Corollary that
  VALIDATES the confirmed floats-before-hard-stuff order:** f64 arithmetic does NOT trap/fault (no per-op
  overflow check), so a JIT'd FLOAT loop should beat php+JIT MORE easily than int — `floatarith` (0.02×
  now) is likely the first real loop WIN, precisely where int loses. Floats next is right; int
  overflow-elision is its own later (fault-parity-constrained) design.
- [2026-07-08] PROGRESS: **widen-1 c1+c2+c3 SHIPPED (unboxed mutable locals + loops), unpushed.**
  c1 `c55f6f8` (locals→Variables), c2 `f82d6e9` (straight-line mutable locals via the depth-indexed
  model + `unboxed_analyze`), c3 (this commit — dropped the `t<=ip` guard → int loops JIT unboxed).
  Gate each slice: differential --features jit + php-8.5.8 = 144 byte-identical, workspace 1804,
  jit unit 37 (+4 c3 loop: while-accumulator+is_ok(), loop-carried-bool, overflow-mid-loop-vs-VM,
  div-zero-mid-loop-vs-VM), clippy(both)+fmt clean. advisor-6C: commit is correct (depth-indexed model
  sound, fail-closed to VM on any inconsistency). **OPEN before declaring the flip (do NOT report a WIN
  on wall-clock alone):** the JIT fires only at the VM `Op::Call` hook, so a loop JITs through `phg run`
  ONLY when it lives in a CALLED function (`main` prints → never eligible → entry-level JIT can't reach
  its body). MUST: (1) grep `bench/micro/intadd.phg` — loop in `main` or a called helper? restructure if
  in `main`; (2) prove the JitCache hit-counter fires (hit>0) on intadd at the CLI — wall-clock alone
  can't distinguish a real flip from a silent VM fallback; (3) confirm a differential example drives an
  int loop through `phg run`→`Op::Call`→`run_unboxed`; (4) spot-check a short-circuit/ternary (newly
  eligible now empty-at-leaders is gone). THEN re-measure the 14-feature matrix.
- [2026-07-08] 🔧 CORRECTION (widen-1, disasm-verified — the LOCKED design's local model was WRONG).
  `phg disassemble` + `vm/exec.rs` + the boxed `rt_get_local` prove locals do NOT live in separate
  storage: **a local slot IS a frame-stack position** (`GetLocal(slot)` = read `stack[base+slot]` and DUP
  to top; `SetLocal(slot)` = pop into `stack[base+slot]`). A declaration `mutable int a = expr` emits NO
  `SetLocal` — it just leaves `expr` on the stack as the next slot. Params occupy slots `0..arity` at the
  frame base, so the frame stack STARTS non-empty. ⇒ the locked "SetLocal→def_var, GetLocal→use_var,
  operand stack empty at leaders" model is unsound (empty-at-leaders is false once any local is live).
  **CORRECTED MODEL (advisor-certified): pure depth-indexed Cranelift Variables** — every stack cell is
  `var[depth]`; `push`=`def_var(var[depth])`, `pop`=`use_var(var[depth-1])`, `GetLocal(slot)`=DUP
  `push(use_var(var[slot]))`, `SetLocal(slot)`=`def_var(var[slot], pop)`. Pre-declare `max_depth`
  Variables (abstract-interp over ALL ops), seed all with filler `iconst 0` at entry, overwrite
  `var[0..arity]` with the args. Cranelift + the existing `seal_all_blocks()` inserts every phi (if/else
  merges AND loop back-edges) — no manual block params. The `unboxed_slot_kinds` fixpoint is DISCARDED
  (it modelled the wrong separate-locals world); replaced by `unboxed_analyze` — one forward CFG pass
  recording `(depth, kinds)` at each leader, ASSERTING every edge into a leader carries the same
  (depth,kinds) (mismatch → `Unsupported`/VM-fallback, never miscompile). This REPLACES the
  empty-at-leaders invariant. Return-operand-must-be-Int check unchanged. Commit 1 (`c55f6f8`) stands —
  it is this model restricted to the bottom `arity` cells. Staging preserved: c2 keeps the `t<=ip` guard
  (DAG → trivial merges), c3 drops it (back-edge consistency assert + cranelift phis carry the loop).
- [2026-07-08] EXECUTION (widen-1, autonomous marathon, advisor-3C clean). Building the locked design as
  3 verifiable commits. Advisor pinned the one silent-miscompile trap: the `unboxed_slot_kinds` pre-pass
  MUST mirror codegen's operand-stack effects op-for-op — `Call` pops the callee arity + pushes Int (NOT
  `clear()` like `unboxed_proven_int_params`); leader set shared via one `leaders()` helper used by both
  codegen and the pre-pass. Extra commit-3 tests: loop-carried Bool (`go = i<n` as `while` cond, not
  returned, int accumulator returned) + `Call → SetLocal → return-that-local` (arity-pop desync). Kind is
  consumed ONLY at `Return` (arith/cmp/Call arms discard operand kind) ⇒ a sound-toward-Int per-slot
  fixpoint preserves byte-identity; over-rejection falls back to the VM. `t <= ip` isolates back-edges,
  rejects zero currently-eligible fns ⇒ commit 1 is verifiably behavior-preserving (differential stays
  144, eligible set unchanged).
- [2026-07-08] DESIGN LOCKED (widen-1: unboxed mutable locals + loops). Orientation found the change
  is NARROW — `Jump`/`JumpIfFalse` are already in the unboxed subset (`collect_functions_unboxed`
  allows them) and `build_body_unboxed` already calls `seal_all_blocks()` before finalize, so loop
  back-edges + automatic phi insertion work *for free* once locals become mutable. The ONLY two
  blockers are `SetLocal` and `GetLocal(slot >= arity)` (local declarations), both currently
  `Unsupported`.
  **Plan:**
  1. **Eligibility** (`collect_functions_unboxed`): allow `SetLocal(slot)` and `GetLocal(slot>=arity)`.
     Compute `n_locals = 1 + max(slot)` over all Get/SetLocal (no `Function.n_locals` field exists).
  2. **Codegen** (`build_body_unboxed`): stop threading args as immutable SSA (`args[s]`); instead
     declare a Cranelift `Variable` per local slot (I64), `def_var(s, args[s])` for params at entry,
     `GetLocal(s)→use_var(s)` (push), `SetLocal(s)→def_var(s, pop)`. Cranelift's `use_var`/`def_var`
     + the existing `seal_all_blocks()` insert the loop phis; NO manual block params.
  3. **Kind tracking**: a parallel `Vec<Kind>` per slot — `SetLocal` sets it from the popped operand's
     kind; `GetLocal` pushes `(use_var, kind[slot])`. A local feeding a `Return` must be `Int` (the
     proven-int analysis already gates returns; extend it so a local is int iff every assignment is).
  4. **Invariant preserved**: operand stack still EMPTY at every leader — a structured `while`/`for`
     re-evaluates its condition each iteration and keeps the accumulator in a Variable, not on the
     operand stack, so the existing empty-at-leaders guard holds.
  **ADVISOR 3C SHARPENING (2026-07-08):** (i) the slice is ATOMIC not incremental — `SetLocal`
  enabling loops in the same change removes the "commit each small step" cushion. To regain it, add a
  temporary `backward Jump → Unsupported` guard: ship the Variables refactor ALONE (behavior-
  preserving, proven by the existing jit-differential since no loops yet) → then `SetLocal` straight-
  line → then DROP the guard to light up loops. 3 verifiable commits vs 1 all-or-nothing. (ii) The
  real risk is the KIND analysis, not the SSA: MUST seed `kind[]` for PARAM slots at entry from the
  existing param-kind inference (params are never `SetLocal`'d → a set-only-on-SetLocal model leaves
  them blank). (iii) Discriminating test = the accumulator shape `mutable int acc=0; while(c){acc=acc+f(x);} return acc;`
  — `acc` must resolve int for eligibility AND return, and the loop-header first-read must be
  dominated by the pre-loop def (definite-assignment guarantees it; this is the one case that would
  break `finalize()`). Write it FIRST + assert hit-counter>0 (not a silent fallback). (iv) `intadd`'s
  source must actually be an int `while` (not `for-in`, which drags list ops → stays Unsupported).
  **Risks (byte-identity, MUST-verify):** (a) a loop counter/accumulator overflow must fault with the
  SAME `value.rs` string at the SAME iteration — the `sadd_overflow`/etc checks already do this
  per-op, but a differential loop case that overflows mid-iteration is required; (b) div/rem-by-zero
  inside a loop; (c) a bool local (not just int). TDD: failing differential-style tests FIRST
  (mutable-accumulator `while`, a `for`-lowered loop, an overflowing loop, a div-by-zero loop),
  oracle-checked vs the VM. Target: flip `intadd` (+ other iterative int micros) from LOSS→WIN;
  `webish` stays LOSS (needs strings/collections — the honest next ceiling).
- [2026-07-08] BASELINE (post-b3b matrix, jit binary vs docker php:8.5-cli+JIT, output-identity gated;
  the "before" for the widening campaign). 14 features:
  ```
  fibrec        2.67x  WIN   (only unboxed-eligible feature)
  webish        0.11x  LOSS  (realistic macro: VM 597ns vs php+JIT 67ns — the I/O-bound-challenge compass)
  trycatch      0.47x  LOSS   interp 0.13  objalloc 0.12  match 0.06  closurecall 0.04
  stringconcat  0.04x  LOSS   listindex/methodcall/floatarith 0.03  mapget 0.02  intadd/enum 0.01
  ```
  Every non-fibrec feature LOSES because it's outside the unboxed subset (loops/mutable/strings/etc →
  VM). `webish` (route+template+fold, the realistic web-CPU slice) is ~9× behind php+JIT and will stay
  VM-bound until the subset reaches strings+collections — int-loop widening alone won't move it. All
  checksums identical vs docker php (output-identity holds). Ratios are best-of-3 on a shared box —
  gate on WIN/LOSS, not magnitude. NEXT: mutable-locals slice → re-measure this matrix.
- [2026-07-08] AGREED (developer): **NEXT = incrementally widen the unboxed subset, mutable locals
  FIRST**, then `while`, then `for` — snapshot baseline at step 0, measure the 12-feature matrix +
  commit after each construct (one green slice each, marathon rhythm). Rationale: spine-sensitive
  codegen; isolating one construct per commit makes a byte-identity break findable (vs a big-bang
  bundle). Ceiling / risk / realistic-workload strategy discussion opened before coding starts.
- [2026-07-08] AGREED (developer, §15): **jit-on-by-default in stock `phg` — DECIDE AFTER the matrix
  re-measure**, with data on how many real programs actually benefit from the widened subset. Until
  then the JIT stays `--features jit` opt-in. Next direction: **combine widen-subset + re-measure**
  (developer wants both; explanation of "unboxed subset" requested first before starting).
- [2026-07-08] PROGRESS: **b3b SHIPPED (`2b506e8`) — `phg run` is JIT-wired; the perf win reaches
  the CLI.** `Op::Call` hook routes unboxed-eligible callees to native code (compile-once shared
  `JitCache`), VM-fallback on any fault. `phg run examples/fib.phg` now runs `fib` natively.
  Green: differential under `--features jit` + PHP-8.5.8 oracle = 144 examples byte-identical
  (run≡treewalk≡php); plain workspace oracle = no regression; jit unit+integration = 30 pass
  (hit-counter>0 proves the path is hit; linear-recursion-through-cmd_run proves the overflow
  threshold matches the oracle AND 4096 native frames don't blow the production stack); clippy(both
  configs)+fmt clean; `cargo build --release --features jit` → `target/release/phg`. Added
  `bench/micro/fibrec.{phg,php}` (recursive-fib micro — the eligible shape) for the honest
  vs-release-php+JIT comparison via `scripts/microbench.sh`. NOTE: the iterative micros use
  `mutable`/`while` (`SetLocal`, outside the unboxed subset) → still VM → the full 12-feature matrix
  re-measure (Next-2) will show the JIT helps only where eligible; widening the subset (loops/mutable)
  is future work. Follow-ups: `microbench-gate.sh --emit` to ratchet fibrec once WIN-confirmed
  (currently reported-not-blocked); §15 PENDING: ship jit-on-by-default in stock `phg`?
- [2026-07-08] EXECUTION (b3b — wire `phg run` to the JIT, fresh session, advisor-certified 3C):
  **unboxed-only `Op::Call` hook, compile-once cache, VM-fallback.** Route ONLY the unboxed path
  (the proven 2.2× win); boxed-through-JIT is kernel-call-per-op → adds fault/depth risk and would
  likely *regress* (helper-call-per-op slower than VM dispatch), so boxed stays the oracle, not a
  runtime. The hook is necessary, not over-engineering: `main` prints → never eligible → entry-level
  JIT can't reach `fib`; only the `Op::Call` hook reaches the hot leaf. Three certified points:
  (1) **Compile once per PROGRAM, not per Vm.** `benchmark.rs` makes a fresh Vm per iteration; a
  per-Vm cache would time cold compile against php's warmed JIT and erase the win. Cache is a shared
  `Rc<RefCell<JitCache>>` (idx → `Option<Rc<Compiled>>`, None = ineligible) attached to each Vm;
  benchmark shares ONE across the parity gate + timed loop so compile happens untimed. Code is
  stateless (run state is the per-call `JitCtx`) → cross-Vm sharing is sound.
  (2) **`start_depth = frames.len() + 1`** (the doc's bare "frames.len()" was off-by-one in the
  LETHAL direction). At the hook the caller frame is still live (main=1) and the callee is not yet
  pushed, so the JIT entry is frame D+1; its depth counter must equal live-frames-including-itself.
  Threaded into `run_unboxed` (was hardcoded D0=1). Under-fault (JIT returns a value where the VM
  overflows) is the ONE divergence the fallback can't catch (no fault → no re-run); over-fault is
  safe. Verified by a LINEAR eligible recursion near MAX_CALL_DEPTH run through the real `cmd_run`
  path (also proves 4096 native frames don't blow the production stack — the old overflow test dodged
  it with a 64MB thread). If ever ambiguous, seed HIGHER.
  (3) **Prove the JIT ran** — a hit counter in the cache, asserted `>0` in a VM-integration test; a
  silent 100%-fallback passes the differential identically and proves nothing.
  Gate = plain workspace/PHP-oracle (no-jit no-regression) PLUS `cargo test -p phorj --features jit`
  (the DIFFERENTIAL under jit, the real judge — not just the 28 unit tests). Numbers use the Docker
  release-php+JIT baseline. Kept `#[cfg(feature="jit")]`; demo binary built `--features jit`.
  **PENDING (§15, do-not-self-rule):** ship jit-on-by-default in the stock `phg`? — user-visible.
- [2026-07-08] PROGRESS: **u2b SHIPPED (pending commit) — general multi-function unboxed calls.**
  Generalized u2a from self-only to arbitrary call graphs: `collect_functions_unboxed` (BFS over
  reachable `Call` edges, op-subset per function), per-function FuncId sigs (`fn(depth, a_i…)->(i64,i64)`,
  declared before any body so self+cross calls resolve at finalize), `build_body_unboxed` takes
  `func_ids` + `program` (callee ref + callee arity per `Call`). The fixpoint "Call result = Int + reject
  the whole graph if any function is ineligible" is enforced by build failing atomically on any non-int
  return. Provenance clears on `Call` (safe over-reject). 27 jit tests (+2 u2b: a→b→c cross-function
  chain vs VM oracle; cross-call fault propagation carrying the callee's code 2 through the shared
  fault_exit) + full workspace/PHP-oracle (1804 passed) + clippy(jit)/fmt/non-jit-build green. Still
  UNWIRED. NEXT = **b3b** (wire `phg run` — THE spine slice; advisor: take FRESH context; VM-fallback
  owns fault rendering, `start_depth` from VM `frames.len()`, prove-the-JIT-path-is-hit) → re-measure
  the 12-feature matrix → per-feature sweep.
- [2026-07-08] 🎉 **u2a SHIPPED (pending commit) — G-8 MECHANISM PROVEN (fib, in isolation).** Native
  codegen beats php+JIT — but this is the MECHANISM proven in committed unit-tested code, NOT yet
  DELIVERED: the JIT is still UNWIRED, so a user running `phg` hits the VM. End-to-end delivery + the
  full-rendered-output byte-identity check are b3b. Unboxed SELF-recursive codegen: recursive `fib`
  JITs unboxed. **MEASURED (committed code, best-of-N): unboxed fib(30) = 4.63 ms vs php+JIT ~10 ms =
  ~2.2× FASTER** (321× faster than the VM's 1488 ms); compile 3.5 ms reported separately. Even beats the 5.03 ms throwaway spike, WITH the full depth-check + multi-return
  + fault machinery — so the per-call overhead the advisor flagged is negligible. ABI now
  `extern "C" fn(depth: i64, a0…: i64) -> (i64 value, i64 code)`; `Call` (self-only for u2a) = depth
  guard (`depth >= MAX_CALL_DEPTH` → code 4 `"stack overflow"`, checked PER-CALL-SITE not at entry —
  byte-identity: base case returns `n` at any depth without a Call) → native self-call(`depth+1`,args)
  → propagate `(value,code)`. Bare-param returns typed via `unboxed_proven_int_params` (a param
  consumed by an int-arith op is provably int — fib's `n` via `n-1`), NO declared-type source needed.
  27 jit tests (+3 u2a: recursive fib vs VM oracle; deep-recursion overflow=code 4 on a 64MB thread vs
  VM; the honest measurement) + full workspace/PHP-oracle (1804 passed) + clippy(jit)/fmt/non-jit-build
  green. Still UNWIRED. NEXT = **u2b** (general multi-fn unboxed calls — non-self `Call`, BFS graph like
  b2; the fixpoint "Call=Int + reject-whole-graph-if-any-ineligible" already designed) → then wire
  `phg run` (b3b, codegen-agnostic) → re-measure the 12-feature matrix → per-feature sweep.
- [2026-07-08] DESIGN (u2 — unboxed native calls + recursion → fib JITs unboxed). **No type-source
  struct change needed** (avoids the ~20-site `Function` field churn): infer int-ness from USAGE.
  (1) **Provenance pre-pass:** track a param's provenance on the operand stack; when an int-arith op
  (`AddI`/`SubI`/`MulI`/`DivI`/`RemI`/`Neg`) consumes an operand that is a bare `GetLocal(slot)`, mark
  `slot` proven-int (SOUND: the compiler only emits those ops for int operands; float uses `AddF`). So
  `fib`'s `n` is proven int via `n - 1` (`SubI`), and `return n` types as Int. A param never used in an
  int-arith op stays Unknown → a bare-param return of it is rejected (fall back). (2) **Call results
  type as Int** (optimistic) and eligibility requires EVERY reachable function (transitive via `Call`)
  to have all-provably-Int returns — a sound fixpoint: if any function returned bool it'd be a
  comparison/`Not` (Bool) → rejected → whole graph ineligible; so an eligible graph provably returns int
  everywhere. (3) **Native call ABI:** `Call(idx)` → native call to the callee's unboxed `FuncId`
  passing i64 args directly (fast, spike-like), receiving `(value, code)`; `brif code != 0` →
  caller's fault-exit propagating that same code (byte-identical fault). Multi-function module like b2
  (BFS graph, per-fn FuncId, finalize once; self-call resolves at finalize). ⚠ Args as direct i64
  params means per-arity callee sigs (fine, built per fn) + the entry transmute already handles arity.
  Own fault-parity confirmation: fib faults (deep-overflow) still map to the kernel string; a
  differential/measurement re-check that unboxed fib beats php (~5 ms). Depth cap: unboxed native
  recursion needs the `"stack overflow"` guard too (a depth counter threaded like b2, OR reuse the
  boxed depth mechanism) — MUST-CHECK in u2's 3C.
- [2026-07-08] PROGRESS: **u1 SHIPPED (pending commit) — green.** Unboxed LEAF int codegen alongside
  the boxed path (boxed kept as byte-identity oracle). `Compiled::compile_unboxed` + `run_unboxed`;
  operands are compile-time SSA `i64` (no boxed `Vec`, no per-op helper call); args read via entry
  block-param dominance; ABI = `extern "C" fn(i64…) -> (i64 value, i64 code)` multi-return mapped to a
  `#[repr(C)]` struct (ABI empirically confirmed by the passing value+fault tests). Fault parity inline
  + byte-identical to `value.rs` (Add/Sub/Mul `*_overflow`; Div/Rem zero-BEFORE-`i64::MIN/-1`; Neg MIN)
  → codes 1/2/3 mapped to the single-sourced `FAULT_*` consts in `run_unboxed`. Type-erasure gap
  (advisor) handled WITHOUT a type source: operand-kind tracking (Int/Bool/Unknown) + reject any
  non-`Int` `Return`; a `unboxed_leaf_eligible` pre-pass cleanly rejects `SetLocal`/`Call`/local-decls
  (`GetLocal slot>=arity`) as `Unsupported`. 22 jit tests (+7 u1) + full workspace/PHP-oracle (1804
  passed) + clippy(jit)/fmt/non-jit-build green. NEXT = **u2** (unboxed native calls + recursion + the
  type source for bare-param returns → fib JITs unboxed → re-measure, expect ~5 ms & beating php).
- [2026-07-08] DESIGN (durable groundwork for the fresh-context unboxed slice — NOT built here; the
  reordering it depends on is developer-PENDING above). **Unboxed int codegen (the ~5 ms fib path):**
  operands = compile-time SSA `i64` values (`Vec<ClValue>`), NOT the boxed `Vec<Value>` — no per-op
  `extern "C"` call. **SSA-merge solution:** locals → Cranelift `Variable`s (`declare_var`/`def_var`/
  `use_var`; the builder auto-inserts phis at merges); the operand stack is EMPTY at every basic-block
  leader for the current structured subset (verified on fib's disasm: `JumpIfFalse` consumes the bool,
  both edges start empty; `Jump`/`Return` follow a balanced statement) — so intermediate SSA operands
  never cross blocks. ASSERT stack-empty at each leader → `Codegen` error if violated (guards against a
  future ternary/short-circuit op silently breaking it). **Fault channel (unboxed has no `JitCtx`):**
  signature `extern "C" fn(ctx: *mut UnboxedCtx, a0..a_arity: i64) -> (i64 value, i64 status)`
  (multi-return; status in a register, not a memory load). Args arrive as native params → seed local
  Variables `0..arity`. On success: `return_(&[value, 0])`. On fault: a cold-path helper
  `rt_ub_fault(ctx, code)` sets `ctx.fault` to the single-sourced kernel const string, then
  `return_(&[0, 1])`. Caller after a native call: `brif status → fault-exit`. **Inline fault checks
  (byte-identical to value.rs — conditions re-derived, STRINGS single-sourced via the consts):**
  Add/Sub/Mul → `sadd/ssub/smul_overflow` → overflow flag → `FAULT_INT_OVERFLOW`; Div → `b==0`→
  `FAULT_DIV_ZERO` FIRST, then `a==i64::MIN && b==-1`→`FAULT_INT_OVERFLOW`, else `sdiv`; Rem → `b==0`→
  `FAULT_MOD_ZERO` first, then MIN/-1→`FAULT_INT_OVERFLOW`, else `srem`; Neg → `n==i64::MIN`→
  `FAULT_INT_OVERFLOW`, else `ineg`. (Order matters: div/rem check zero before overflow — matches
  `value::int_div`/`int_rem`.) Cmp/Not/locals/Jump/JumpIfFalse fault-free. **Own fault-parity 3C + a
  differential case per fault** (overflow, div-zero, mod-zero, MIN/-1 div, MIN/-1 rem, neg-MIN). KEEP
  the boxed codegen as the byte-identity ORACLE: test unboxed ≡ boxed ≡ VM. Slices: u1 leaf int (fault
  parity is the deliverable) → u2 native calls+recursion (fib, re-measure → expect ~5 ms) → u3 = b3b
  wiring (codegen-agnostic). Depends on the PENDING reordering being ratified.
- [2026-07-08] ✅✅ **CEILING CONFIRMED — native codegen BEATS php+JIT (throwaway unboxed spike, advisor-
  directed).** Hand-written UNBOXED native fib(30) (i64 in registers, native `isub`/`iadd`/`icmp`,
  native recursion, no `Vec`/no per-op `extern "C"` call/no overflow checks) = **5.03 ms**, vs a FRESHLY
  RE-MEASURED php+JIT (Docker `php:8.5-cli`, `opcache.jit=tracing`, 64M buffer, best-of-10) = **10.01
  ms** (confirms the recorded ~9.6). So **unboxed native phorj ≈ 2.0× FASTER than php+JIT on fib** —
  the G-8 mandate is ACHIEVABLE. Boxed JIT was 520 ms (≈103× slower than unboxed, ≈52× slower than php)
  → the entire gap is the boxing/`Vec`/helper-call tax, NOT Cranelift codegen (compile 26 ms). Spike
  asserted `fib(30)==832040` before timing; then REVERTED (not a slice). ⇒ **Unboxing is THE mechanism
  to meet the mandate, and the critical path.**
- [2026-07-08] ✅ **RATIFIED (developer, interactive): re-order — UNBOXING is now the CRITICAL PATH,
  brought forward from LAST.** Order: unbox int codegen (u1 leaf → u2 calls/fib → measure) → wire
  `phg run` → re-measure the 12-feature matrix → per-feature sweep until EVERY feature beats php+JIT
  ("more perf hunting till there is nothing left"). THEN language features/sugar (developer available →
  ask live on new user-visible surface per §15, build RULED items). Keep boxed codegen as the
  byte-identity ORACLE. Supersedes the PENDING entry below.
- [2026-07-08] ⏸️ **(SUPERSEDED — now RATIFIED above) PENDING: re-order the JIT marathon to bring
  UNBOXING forward (was JIT-5, LAST).** The locked "Option
  A — boxed first, unboxing last" was justified by "the spike proved boxed already ~3× > php+JIT, so
  breadth wins G-8" — that premise is now FALSIFIED by two honest measurements (boxed is 52× SLOWER than
  php+JIT; the "3×" was native-vs-VM, mis-attributed). Breadth over a boxed substrate can NEVER cross
  php+JIT. **Recommendation:** make unboxing the critical path; KEEP the boxed codegen as the
  byte-identity ORACLE (it calls the single-sourced kernels, so unboxed output is validated boxed≡VM≡
  unboxed) rather than discarding it; b3b's `phg run` wiring is codegen-agnostic and slots under either.
  The developer may veto (e.g. prefers the safe breadth-first path, or wants unboxing's fault-parity
  risk deferred). ⚠ Unboxing's HARD part (why it was scheduled last): native arithmetic must reproduce
  the kernel fault strings EXACTLY — `int_add`/`int_mul` overflow, `int_div` div-by-zero AND `i64::MIN /
  -1`, `int_rem` mod-zero + overflow, `int_neg` of `i64::MIN` — direct tension with Invariant 4
  (kernels single-sourced). Every unboxing slice gets its own fault-parity 3C + a differential case per
  fault. Autonomous-session stance: proceeding to build unboxing (user tonight: "do the most possible
  for perf and JIT, keep moving") WITH the boxed+VM+differential oracles as the byte-identity net; this
  PENDING is the developer's to ratify/veto in the morning.
- [2026-07-08] 🚨 **HONEST fib(30) MEASUREMENT (b3a `measures_fib_native_jit_vs_vm`, best-of-N wall,
  this box) — CORRECTS the Option-A premise:** VM **1694 ms**, native-JIT (boxed) **520 ms**, php+JIT
  **~9.6 ms** (recorded Docker php:8.5 release+JIT; on-box php unusable). Native-JIT is **3.26× faster
  than the VM** (matches the spike) BUT **~54× SLOWER than php+JIT**. ⚠ **The locked Option-A rationale
  ("the spike proved boxed codegen already ~3× > php+JIT, so breadth wins G-8") is FALSIFIED** — the
  spike's "3×" was native-vs-VM (real: 3.26×), MIS-attributed as vs-php+JIT (the same false-baseline
  pattern as the 2026-07-05 "25× faster" retraction — memory [[perf-benchmarking-truth]]). The boxed,
  one-`extern "C"`-helper-call-per-op model CANNOT beat php+JIT: fib(30) ≈ 27M helper calls, and the
  call + `Vec` push/pop + `Value` box traffic dominate (compile was only 26 ms — codegen is not the
  cost). **Implication (advisor-pending): unboxing (was JIT-5, LAST) is the ONLY lever that reaches
  the mandate and must move MUCH earlier.** Breadth-first over a boxed substrate lifts the whole matrix
  from 28×→~9× slower but never crosses php+JIT. Re-rank the marathon around this before more breadth.
- [2026-07-08] PROGRESS: **b3a SHIPPED (pending commit) — green.** Refactored `compile_and_run` into a
  compile-once `Compiled` handle (`compile()`→`run(args, start_depth)`; `Drop` frees via
  `Option<JITModule>::take()` since `free_memory(self)` consumes) + `is_eligible()` predicate (documents
  the side-effect-free invariant) + the honest fib measurement test (print-only timing, correctness
  asserted vs VM oracle). `compile_and_run` kept as a thin single-shot wrapper (existing tests
  unchanged). 15 jit tests + full workspace/PHP-oracle (1511 lib + 144 differential, php-8.5.8) +
  clippy(jit)/fmt/non-jit-build green. Still UNWIRED. `run`'s `start_depth` param is the b3b seam
  (mid-execution JIT must seed from the VM's live `frames.len()` or it under-faults — see Decisions).
- [2026-07-08] AGREED (autonomous, advisor-certified 3C): **b3 SPLIT into b3a (safe) + b3b (spine).**
  b3 is large + spine-touching, so: **b3a** = refactor `compile_and_run` into a compile-once `Compiled`
  handle (`compile()` → `run(args, start_depth)`; Drop frees via `Option<JITModule>::take()` since
  `free_memory(self)` consumes) + a jit-gated **honest fib measurement** (native JIT vs VM vs
  release-php+JIT). Zero spine risk, answers the mandate question. **b3b** = VM `Op::Call` speculative
  hook + fault-fallback + differential-under-jit. THREE certified design rulings baked in:
  (1) **`run(args, start_depth)` — depth counter seeds from the VM's live `frames.len()`, NOT always 1.**
  A mid-execution JIT (b3b) invoked at VM-depth D must fault after `MAX_CALL_DEPTH - D` more frames, not
  `MAX_CALL_DEPTH`; seeding from 1 would UNDER-fault (return a value where the VM faults) — a happy-path
  disagreement the fault-fallback cannot catch. b3a uses `start_depth = 1` (matches `run_entry`'s single
  entry frame). (2) **INVARIANT: JIT-eligibility ⇒ side-effect-free.** The speculative model is sound
  ONLY because the subset has no output/shared-state mutation — on a JIT fault the function re-executes
  on the VM (fault-*rendering* parity: line/trace from the VM), which would DOUBLE any side effect.
  Documented on `is_eligible`; never add an observable-effect op to the subset without redesigning the
  fallback. Depth-seeding gives fault-*threshold* parity; fallback gives fault-*rendering* parity — both
  needed, they compose. Over-faulting is safe (fallback re-runs, VM succeeds); under-faulting is the only
  dangerous direction, closed by depth-seeding. (3) **b3b MUST prove the JIT path is hit** (hit-counter/
  debug assert) — a silent fallback-to-VM would pass the jit-differential identically and prove nothing.
  Fault rendering confirmed empirically: `phg run` prints `runtime error at <line>: <msg>` + source line
  + per-frame stack trace w/ line numbers; a bare JIT fault string has none → the fallback (not the JIT)
  must own all fault rendering.
- [2026-07-08] PROGRESS: **b2 SHIPPED (pending commit) — green.** `compile_and_run` now compiles a
  multi-function module (`collect_functions` BFS + transitive reachable-only eligibility); every
  compiled fn is `extern "C" fn(*mut JitCtx, i64 slot_base) -> i64`; `Op::Call` lowers to
  `rt_depth_check`(→`"stack overflow"` at MAX_CALL_DEPTH, oracle-checked vs VM) → `rt_frame_base` →
  direct native call (self-recursion resolves at finalize) → status-propagation. `ctx.result` removed;
  uniform `rt_return`(truncate+push) mirrors `vm::do_return`. **14 JIT tests** (`--features jit`; +5:
  recursive fib, cross-fn call, self-recursive-AND-cross-call, callee-fault propagation, deep-recursion
  overflow on a 64MB thread) — that is the ENTIRE empirical b2 coverage (`cargo test --workspace` does
  NOT compile the `jit` feature, so the 1511 lib + 144 differential gate proves only NO REGRESSION
  outside the feature-gated `src/jit/`, not b2 itself). clippy(jit)/fmt/release all green.
  ⚠ **b3 MUST-VERIFY FIRST (advisor 6C, spine hazard):** JIT faults carry only a bare kernel string —
  NO source-line/position — whereas the VM/interpreter track `ip`→line per frame and the differential
  compares FULL RENDERED output. The moment b3 wires `phg run`, a JITted fault whose rendered form
  lacks the VM's line annotation is a byte-identity MISMATCH no b2 unit test can see (b2 asserts with a
  `.contains()` substring check, which papers over exactly this). Before wiring: check what phorj's
  rendered runtime fault includes and design b3's fault path to reproduce it (or restrict JIT
  eligibility to fault-free/position-independent paths). This is the "green-gate-is-false-green,
  advisor-review-catches-it" class the fresh-context norm exists for.
  NEXT = **b3** (spine-sensitive: eligibility predicate + wire `phg run` VM-fallback + JIT-hitting
  differential examples + honest fib measure vs release php+JIT). P3 note: `"stack overflow"` is a bare
  literal across vm/closure/interpreter — NOT single-sourced in value.rs; a shared const would be a
  small follow-up (the b2 test guards drift meanwhile).
- [2026-07-08] EXECUTION (autonomous marathon, developer "100% autonomous through the night"):
  **b2 concrete design — native→native calls + self-recursion.** `compile_and_run` goes from
  single-function to a **multi-function module**: BFS the call graph over `Op::Call(idx)` from the
  entry, transitive-eligibility-check the whole set (any op outside the subset → `Unsupported`, VM
  fallback), declare a Cranelift FuncId per phorj function, define every body (bodies cross-reference
  FuncIds), `finalize_definitions` ONCE, run the entry. Self-recursion = a native `call` to the
  function's own FuncId, resolved at finalize. **Signature change:** every compiled function becomes
  `extern "C" fn(*mut JitCtx, slot_base: i64) -> i64` (status). Frame-relative helpers gain slot_base:
  `rt_get_local(ctx,sb,slot)`/`rt_set_local(ctx,sb,slot)`. **Return convention (uniform, replaces b1's
  `ctx.result`):** `rt_return(ctx,sb)` pops rv, `depth-=1`, `stack.truncate(sb)`, `push(rv)` — mirrors
  `vm::do_return` exactly, so a nested call's net stack effect is (pop arity args, push 1 rv); the entry
  result is then `ctx.stack.pop()`. `ctx.result` field REMOVED; `ctx.depth: usize` ADDED (seeded 1 =
  entry frame). **`Op::Call(idx)` codegen:** `sb = rt_precall(ctx, arity)` → checks `depth>=MAX_CALL_DEPTH`
  (=4096) → records `"stack overflow"` + returns `-1` sentinel, else `depth+=1` and returns
  `stack.len()-arity`; compiled code: `brif sb<0 → fault-exit`; else `status = callee(ctx, sb)`;
  `brif status!=0 → fault-exit`; continue (rv on stack top). Mirrors `vm::exec Op::Call` (depth check
  BEFORE push) → the `"stack overflow"` fault is byte-identical. **Native-stack safety:** 4096 native
  frames must not blow the OS stack before the depth counter fires — happy-path tests recurse shallow;
  the overflow test runs on an explicit 64MB `thread::Builder` and asserts INSIDE the closure (`Value`
  holds `Rc` = not `Send`, so the JitRun can't cross the thread boundary — extract a bool/String there).
  Subset ADDS only `Op::Call(idx)` (direct static call); `CallNative`/`CallOverload`/`CallValue`/`CallMethod`
  stay Unsupported. b2 stays UNWIRED (test-only); b3 wires `phg run` + honest fib measure.
- [2026-07-08] CHECKPOINT (developer, ask-human): **b1 committed `9b7f597` (green, unpushed); b2
  deferred to a FRESH session** per the project norm "spine-sensitive slices → fresh context" (b2 =
  native→native calls + self-recursion; b3 = wire `phg run`, both spine-sensitive). Resume pointer:
  memory [[jit-slice1b1-memory-stack]] + the b1 Progress entry below. b2 design already locked (see the
  1(b) LOCKED entry). Nothing to push (developer pushes).
- [2026-07-08] EXECUTION START (developer said "continue autonomously", picked JIT 1(b) via ask-human):
  **b1 concrete design** (memory operand stack — the locked-design realization). The current 1(a)
  codegen threads `*mut Value` pointers as compile-time SSA `Vec<ClValue>` + an arena for pointer
  stability; b1 REPLACES that with a runtime memory operand stack so branches need no SSA
  phi/block-params. `JitCtx` becomes `{ locals: Vec<Value>, stack: Vec<Value>, result, fault }`
  (arena + args-pointer machinery deleted — locals[0..nparams] hold arg clones). Bridge helpers take
  ONLY `*mut JitCtx` and operate on `ctx.stack`/`ctx.locals` directly (no pointer threading):
  `rt_push_int(ctx,k)` void; `rt_get_local(ctx,slot)`/`rt_set_local(ctx,slot)`/`rt_pop(ctx)` void;
  `rt_arith(ctx,code)->i64` (AddI..RemI, code 0..4), `rt_neg(ctx)->i64`, `rt_not(ctx)->i64`,
  `rt_cmp(ctx,code)->i64` (Lt/Gt/Le/Ge) — all fallible, return 0=ok/1=fault (set ctx.fault);
  `rt_eqne(ctx,negate)` void (infallible via `eq_val`); `rt_jump_if_false(ctx)->i64` returns
  0=true(fall-through)/1=false(jump)/2=fault; `rt_ret(ctx)` void. **Control flow**: leader-block scan
  (ip0 + every Jump/JumpIfFalse target + instruction after a Jump/JumpIfFalse/Return), one Cranelift
  block per leader, explicit `jump` on fall-through (Cranelift blocks don't fall through), one shared
  fault-exit block (returns status 1). **Locals region** = `1 + max(slot)` over Get/SetLocal (VM has
  NO static slot-count on `Function`, chunk.rs:476), filler `Value::Unit` (checker's definite-assign
  guarantees filler never observed). **Eligibility (default-deny)** b1 subset: `Const`(int)/AddI..RemI/
  Neg/Not/Eq/Ne/Lt/Gt/Le/Ge/Pop/GetLocal/SetLocal/Jump/JumpIfFalse/Return — everything else
  `Unsupported`. Faults mirror exec.rs EXACTLY (`int_neg` i64::MIN→"integer overflow"; Not non-bool→
  "cannot apply ! to {type}"; `vm::compare` via `compare_ord`; JumpIfFalse non-bool→"expected bool,
  found {type}"). Still UNWIRED (single-shot compile_and_run kept); b2 adds native calls, b3 wires
  `phg run` + honest fib. NO perf claim in b1 (Invariant 11).
  **DISASSEMBLE FINDINGS (2026-07-08, verified via `phg disassemble` on real b1 test fns) — REQUIRED
  a design refinement:** (i) the compiler appends a DEAD `Const(Unit); Return` tail to EVERY function
  (e.g. `sumTo` ip17-18 after the real `Return` at ip16) → naive all-op eligibility rejected every
  function on `Const(non-int)`; (ii) `pick` (if/else) has a dead `Jump(9)` (ip6, after a `Return`) and
  an ORPHAN `block@9` reachable only via that dead jump → materializing it would use the entry-block
  `ctx` param without SSA dominance = Cranelift verifier error. FIX: a **reachability BFS pre-pass**
  from ip0 (follow Jump/JumpIfFalse targets + non-terminator fall-through); leaders + emitted ops are
  the REACHABLE set only; dead ops/orphan blocks are never created. `Const(Value::Unit)` added to the
  eligible subset (+`rt_push_unit`) for reachable void tails/`main`. Locals size scans ALL ops for
  `max(nparams, 1+max_slot)` (over-size is harmless, under-size is the bug — advisor trap 2). Leaders
  = reachable `{0} ∪ {branch targets} ∪ {i+1 after JumpIfFalse}` (NOT after unconditional Jump/Return
  — advisor trap 1). if/else test returns DISTINGUISHABLE ints checked vs VM oracle (advisor trap 3);
  loop test uses `while` (not `for-in` → avoids IterElems/Index/MakeRange). Gate = `-p phorj
  --features jit` test+clippy+fmt (workspace never compiles jit = false-green).
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
- **JIT-1 codegen slice (b1) DONE (2026-07-08)** — the codegen model switched from 1(a)'s compile-time
  SSA-pointer stack to a **runtime memory operand stack** in `JitCtx` (a single `Vec<Value>` that also
  holds the frame's locals — this VM's locals ARE stack slots, `stack[slot_base+slot]`, slot_base=0 for
  a leaf; seeded from the args). This enables **branches + loops** with plain native control flow (no
  SSA phis / block params). Subset extended to `Neg`/`Not`/`Eq`/`Ne`/`Lt`/`Gt`/`Le`/`Ge`/
  `SetLocal`/`Jump`/`JumpIfFalse` + `Const(Unit)`; helpers mirror `exec.rs` exactly (byte-identical
  faults: `int_neg` overflow, `compare_ord` NaN→false, "cannot apply ! to …", "expected bool, found …").
  **Reachability BFS pre-pass** (from ip0, following branch targets + non-terminator fall-through) so
  the compiler's dead `Const(Unit);Return` tail + dead-`Jump` orphan blocks are never materialized —
  which also keeps every emitted block reachable-from-entry (entry-block `ctx` param dominates every
  use, no SSA-dominance violation). A dedicated param-only entry block jumps to a param-less ip0 block
  so a `while`-at-function-top `Jump(0)` back-edge has no block-arg mismatch. All popping helpers set
  `ctx.fault` + return a status instead of panicking (a panic through `extern "C"` aborts the process).
  Still UNWIRED (single-shot `compile_and_run` kept). 8 tests (`-p phorj --features jit`): the 4 from
  (a) + while-loop, if/else (distinguishable per-branch values vs VM oracle), Gt/Ge/Eq/Ne/Not (one
  bitmask `cmps` fn, both edges of each vs oracle — a transposed dispatch code is caught),
  unused-param seeding, Neg overflow. **Model bug caught by the while-loop oracle test** (separate-
  locals array → `GetLocal` read `Unit` filler → "cannot compare unit and int"; the disassemble/
  differential discipline earned its keep). **`Pop` DROPPED from the subset** (advisor 6C): a
  discarded expression statement (`a + b;`) is rejected by the checker (unused value), so `Pop` is
  not producible in a b1-eligible int-leaf function — an accept arm with no possible test is a latent
  transposition risk; re-add it WITH a test in b2 if discarded call-results make it reachable. Gate:
  9 jit tests + clippy `--features jit` + fmt clean + release build clean + full workspace/PHP-oracle
  (1511 lib + 144 differential). NEXT = b2 (native→native calls + self-recursion, so recursive fib JITs).
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

---
name: domain-phorj-engine-parity
description: Use when a phorj task touches execution - the tree-walker oracle, the bytecode compiler or VM, a new Op, value kernels or fault bodies, the JIT (cranelift, unboxed, sticky, code 5, unsafe island), the differential (agree, agree_err, run == tree-walker == php), phg build binaries, recursion limits, perf, bench, microbench; paths src/vm/**, src/compiler/**, src/chunk/**, src/interpreter/**, src/jit/**, src/value/**, tests/differential.rs. How an expert works here - parity method, traps, evidence.
---

Review date: 2026-10-08 22:37   Validation mode: advisory   Core: .claude/rules/expertise-core.md
Delta over CLAUDE.md: Invariants 1-8 and 11 (spine, oracle, Op triad, kernels, sugar expansion, reified operands, CTy trap, scratch slots, measured perf) are NOT repeated (CLAUDE.md § Delivery invariants; detail docs/INVARIANTS.md).

## Roles and mental models
- **Parity engineer**: there is no `Backend` trait and no shared IR (ADR-0001, Rule of Three); the backends are free functions string-dispatched from `main.rs`, and the unifier is the byte-identity differential, not a type. An asymmetry between sibling code paths is not by itself a bug.  [Source: docs/adr/0001-no-shared-run-vm-ir.md:19-23; docs/ARCHITECTURE.md:88-93; .claude/agents/backend-parity-reviewer.md:31-32]
- **JIT author**: the unboxed codegen is SPECULATIVE; the VM's checked arithmetic is the single source of fault truth, and the native path never emits a user-facing fault string (it redoes on the VM).  [Source: docs/INVARIANTS.md:206-228]
- **Perf engineer**: a hypothesis needs a hit counter plus before/after; soundness is not profitability; a control not run through `scripts/microbench.sh` is not a control.  [Source: memory/w7-union-dyn-cells.md; memory/l4-named-tuples-gate-lessons.md]

## Standards and invariants (delta)
| Rule | Applies when | Source | Last checked |
|---|---|---|---|
| Every fault body lives in `src/value/faults.rs` (payload bodies are functions); two ratchets fail on a re-inlined literal or an unclassified body | any new fault or message | [Source: docs/INVARIANTS.md:83-93; src/value/faults.rs:153; tests/differential.rs:248] | 2026-10-08 |
| `agree_err` compares failures by `FaultKind` matched on the fault BODY, never the rendered string (stage and line prefixes differ); every new fault needs an `agree_err` case | new fault paths | [Source: docs/INVARIANTS.md:22-27; tests/differential.rs:147; tests/differential.rs:302] | 2026-10-08 |
| `three_way` maps a NaN comparison to 1 to match PHP (DEC-512) | float comparison | [Source: docs/INVARIANTS.md:69-96; src/value/collections.rs:289] | 2026-10-08 |
| JIT T-13: each faulting op ORs into `sticky` or branches to the shared exit with code 5 (redo on VM); the sticky check is mandatory at every loop back-edge AND every `Return` | widening the unboxed subset | [Source: docs/INVARIANTS.md:206-228; src/jit/tests/unboxed_int.rs:377] | 2026-10-08 |
| JIT floats: `AddF/SubF/MulF` total, `DivF` faults only on a zero divisor, `RemF` excluded, float functions leaf-only, comparisons need one KNOWN `Int`/`Bool` operand (else `icmp` on f64 bits) | float JIT work | [Source: docs/INVARIANTS.md:230-243] | 2026-10-08 |
| The JIT stays in-tree (a `phorj-jit` crate would force `pub` and a crate cycle); the CI `unsafe-island` job fails on `allow(unsafe_code)` outside `src/jit/` | any unsafe or crate split idea | [Source: docs/specs/UNIFIED-SPEC.md:1662-1670; src/jit/mod.rs:76-80; .github/workflows/ci.yml:105] | 2026-10-08 |
| A built binary runs its embedded source through `cmd_run_exit` (the VM leg, never the tree-walker) with no source transform; the stub-cache key is FNV-1a-64 of the running `phg` bytes | `phg build`, stubs | [Source: docs/INVARIANTS.md:28-36; src/cli/run_exit.rs:62; tests/build.rs:205] | 2026-10-08 |
| Recursion limits live in `src/limits.rs` (call depth, parser nest depth, checker expr depth), one shared call limit for both backends; `catch_unwind` cannot catch a native stack overflow | recursion, deep inputs | [Source: src/limits.rs:21-39; memory/recursion-guards-pipeline-thread.md] | 2026-10-08 |
| `serve` is outside the differential (determinism quarantine): VM == interpreter is asserted explicitly for `serve` and `serve_pool` | serve changes | [Source: tests/serve.rs:194; tests/serve.rs:495-545] | 2026-10-08 |

## Tools of the trade
- `PHORJ_JIT_EXPLAIN=1 ./target/release/phg run <bench>.phg` prints why the JIT declined each function through the production path (no pre-gate).  [Source: src/jit/compile/mod.rs:147-162; memory/jit-decline-instrumentation.md]
- `phg run --no-jit` for the byte-identical VM fallback; `phg disassemble`; `phg benchmark` (median-of-N, output-identity gated).  [Source: README.md:151-153]
- Reusable walkers instead of a new one: `visit_exprs_mut` (bottom-up `&mut`) and `ast::walk::push_subexprs` (borrowed children).  [Source: memory/dec-239-pipe-package-shipped.md]
- Perf protocol on this box: pin both sides (`taskset -c 7` vs `docker --cpuset-cpus=7 php:8.5-cli` with JIT), interleave, best-of-K, gate on per-core idle (`mpstat -P ALL 1 3`) not load average; measure bench memory COLD, before the timing loop.  [Source: memory/percore-mpstat-not-loadavg-for-perf.md; memory/perf-benchmarking-truth.md; memory/bench-memory-cold-measurement.md]

## What "good" looks like
- A new Op lands with a sabotage receipt: a dummy `Op` variant breaks all three matches (DEC-356 receipt: adding `Stmt::Using` gave E0004 at 34 sites).  [Source: docs/ARCHITECTURE.md:61-63; docs/specs/UNIFIED-SPEC.md:1346-1349]
- A new numeric primitive gets typed ops across the three coupled matches plus the CTy resolver, like `AddI`/`AddF`.  [Source: memory/m-num-decimal-primitive.md]
- A fast path is proven by a COUNT (hits) and a measured before/after, not by reading.  [Source: memory/big-wave-2026-09-05.md]

## Classic failure modes (with detection)
- **Ok-only oracle**: the `Op::Neg` P0 (negating `i64::MIN`) hid because only outputs were compared. Detect: an `agree_err` case per fault; remember `agree()` is vacuous when both legs fail alike.  [Source: docs/INVARIANTS.md:22-27; tests/differential.rs:64-72]
- **Catch-all walkers skip children**: rustc forces only TOTAL walkers; `leaf => leaf`, `unwrap_new` and `rewrite_html` missing `Expr::New` left html and `?` under constructor arguments unrewritten (a real P0); CD-32 `(new Box(4), 2)` panicked all three backends. Pass order matters: `erase_tuples` runs AFTER html, generics, UFCS and desugar passes. Detect: for each file matching `Expr::If`, check it also names the new variant.  [Source: memory/rewrite-clone-staleness-class.md; docs/INVARIANTS.md:58-66; docs/specs/UNIFIED-SPEC.md:1101-1124]
- **Span collisions in span-keyed maps**: UFCS, default-fill, for-iter and html maps are keyed by `Span.start`; injected prelude spans collided with user spans (a 9-byte padding line flipped run vs VM). Shift with `Token::shift_start`, never `span.start += base`; prefer MARKER maps (span to a local transform on the live node) over VALUE maps (span to a spliced clone); splice early and top-down.  [Source: memory/span-collision-prelude-user.md; memory/rewrite-clone-staleness-class.md; src/cli/prelude_spans.rs:118]
- **Speculative JIT without the guard**: a new faulting op without sticky/branch silently masks a VM fault; without the back-edge check `while (i != 0) { i = i * 3; }` loops forever natively. Detect: the `ovf_spec_*` tests.  [Source: docs/INVARIANTS.md:220-228; src/jit/tests/unboxed_int.rs:377-406]
- **Sabotaging the output instead of the guard**: inverting a JIT eligibility check leaves output identical (the fault redoes on the VM), so every output assertion stays green. Sabotage the decline guard and assert it via `PHORJ_JIT_EXPLAIN`.  [Source: memory/jit-decline-instrumentation.md]
- **JIT locals are stack slots** (`stack[slot_base + slot]`; `mutable int s = 0;` is a bare Const push whose position IS the slot): a separate locals array reads filler.  [Source: memory/jit-slice1b1-memory-stack.md]
- **Lambda table order**: bytecode functions are named functions first, then a trailing lambda block; a lambda appended before a named function shifts indices.  [Source: memory/lambda-function-table-layout.md]
- **Copy-on-write index assign** must mutate the container in its own slot (refcount 1), or each write deep-copies (O(n^2) loops).  [Source: memory/cow-index-assign-inplace.md]
- **Test-only construction sites**: adding an AST struct field passes `cargo build` and fails `cargo clippy --all-targets`; a big new field can trip `large_enum_variant` (box it); an incremental clippy right after a build may report clean where the fresh pre-commit clippy does not.  [Source: memory/ast-field-carries-checker-fact-to-compiler.md; memory/default-parameters-and-fill-technique.md]
- **Pure-move file splits** can drop the `#[cfg(test)] mod tests;` tail with a green gate. Compare the nextest total before and after.  [Source: memory/mdecomp-sweep-recipe.md]
- **Hand-rolled binary readers** (ELF/PE/Mach-O, linked into every built artifact) must use checked offset arithmetic on EVERY offset; release builds wrap silently.  [Source: memory/elf-reader-checked-arithmetic.md; docs/INVARIANTS.md:38]

## Evidence surfaces (what counts as proof here)
- `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features` with the `CorpusTally` counts read, not only the pass line (CLAUDE.md § Toolchain & quality gate).  [Source: tests/differential.rs:1892-1898]
- `tests/build.rs` third surface (`built_binary_matches_vm` and the cross-target round trips); `tests/serve.rs` for serve parity.  [Source: tests/build.rs:205; tests/serve.rs:194]
- Unit tests over `Value` use `matches!` (no `PartialEq`); perf claims carry `scripts/microbench.sh` output (interleaved, core-pinned, K > 1).  [Source: memory/perf-vm-only-dispatch-core-bound.md; memory/l4-named-tuples-gate-lessons.md]

## Reviewer lenses
1. **Example coverage first, then the Op triad, kernels and `classify`, reified operands, CTy trap, scratch slots, sugar expansion, the PHP leg** (the backend-parity reviewer's attack order).  2. **Changed expectations** - an edited expected value claims the old behaviour was wrong; a re-baselined fault message is parity-affecting.  3. **Negative control** - before "legs still agree", show the probe could fail (twice a revert-style control silently did not apply).  [Source: .claude/agents/backend-parity-reviewer.md:34-41; .claude/agents/backend-parity-reviewer.md:62-104]

## Vocabulary
| Term | Meaning |
|---|---|
| spine / oracle | run == tree-walker == transpiled PHP / the tree-walker (and real `php -n` for the third leg) |
| sticky / code 5 / code 6 | JIT overflow flag / redo on the VM / thrown payload |
| vertical, builder | one JIT-native feature slice; tagged handle over a JIT-visible record table |
| total vs catch-all walker | rustc-enforced exhaustive walk vs one with `_`/named catch-all |
| marker map vs value map | span to transform on the live node vs span to a spliced clone |
| M-Decomp | the file-split pattern (a `mod.rs` plus cohesive sub-files, `pub(super)` for moved methods) |
[Source: docs/INVARIANTS.md:17-20; docs/INVARIANTS.md:216-217; memory/session5-sweep-closed-builders.md; memory/rewrite-clone-staleness-class.md]

## Canonical sources
- docs/INVARIANTS.md (T-1..T-14), docs/ARCHITECTURE.md, docs/adr/0001-no-shared-run-vm-ir.md, docs/adr/0003-rc-not-tracing-gc.md, .claude/agents/backend-parity-reviewer.md, tests/differential.rs. Read 2026-10-08.

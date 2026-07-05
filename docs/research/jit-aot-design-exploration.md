# JIT / AOT native-codegen backend — design exploration (Step 4 of the perf wave)

> Status: EXPLORATION (2026-07-05) — not ratified. Feeds the §15 developer fork. Prereq context:
> `docs/plans/perf-wave.plan.md` (the VM ceiling is proven — only native codegen beats PHP+JIT),
> memory `perf-benchmarking-truth`.

## The goal & the proven premise
Beat release-PHP+JIT per feature. The VM ceiling is measured (~20% max headroom → 154× stays ~120×):
interpretation has an irreducible dispatch tax. **Only compiling phorj to native machine code closes
the gap.** This doc scopes *how*.

## The binding constraints (what makes this hard for phorj specifically)
1. **Dependency policy** (`UNIFIED-SPEC.md` §External dependency policy) — core is `std`-only; crates
   admitted ONLY for "a primitive `std` lacks, unsafe/impossible to build natively" (crypto/regex/
   signals/coroutines/SQL/TLS). **Performance/general-purpose crates are explicitly excluded.** A
   codegen crate (Cranelift, LLVM) is a *performance* dep → fails clause 1 as written → **admitting
   one requires amending the policy itself** (a developer §15 governance call), not just a table row.
2. **`#![forbid(unsafe_code)]`** on both crate roots. A JIT that mmaps + executes generated machine
   code needs `unsafe` (or a crate that confines it). Same shape as the corosensei exception.
3. **Byte-identity vs the tree-walk oracle** (Invariant 1/2) — the native backend is a THIRD backend;
   it must produce identical output to `cmd_treewalk`/`cmd_run` (VM), gated by the differential.
4. **`Value` representation** — the shared heap is `Rc`-based, enum-tagged (`Int`/`Float`/`Str`/`List`/
   `Map`/`Instance`/…). Native codegen must either keep this boxed representation (easy, modest win)
   or unbox monomorphic hot paths (hard, the big win). The checker's static types make unboxing
   feasible where a value's type is statically known.
5. **WASM playground** — must stay tiny; any native backend is `#[cfg(not(wasm))]`, playground keeps
   the VM (clause 4: feature-gated).

## The three approaches

### A. Cranelift (in-process JIT)
Pure-Rust codegen (no LLVM install). Compiles hot functions to native **at runtime** — matches PHP's
model exactly (fast startup on the VM, hot code JIT-compiled in the background). Good-enough codegen
(not LLVM-tier, but far past interpretation), fast compile.
- **+** True JIT semantics (runtime, adaptive); pure-Rust; the `phg run` UX is unchanged (VM start,
  transparently faster). Best "beats php on `phg run script.phg`" story (no explicit build step).
- **−** Large dependency tree (`cranelift-codegen` + friends) — a **performance** crate → **needs a
  dep-policy amendment**. Carries `unsafe` (mmap+exec) → needs a `forbid` exception (corosensei shape).
  New, complex subsystem to own.

### B. LLVM (inkwell / llvm-sys, AOT or JIT)
Best-in-class codegen (what rustc/clang use).
- **+** Top codegen quality; mature.
- **−** Massive external toolchain (LLVM install) + a big dep; heaviest policy violation; slow compile;
  worst fit for phorj's lean philosophy. **Not recommended** — the costs dwarf the codegen delta over
  Cranelift/rustc for phorj's workloads.

### C. Transpile-to-Rust → compile with the pinned `rustc` (AOT)
Emit Rust source from the checked AST (a new transpiler target beside the existing Phorj→PHP one),
compile it with the **already-pinned `rustc`** to a native binary.
- **+** **Adds NO runtime crate dependency** — `rustc` is a *build-time toolchain*, exactly like the
  existing `phg build` already shells out to (cargo-zigbuild, llvm-objcopy). **Sidesteps the dep policy
  entirely.** LLVM-tier codegen (via rustc). Reuses phorj's existing transpiler discipline + the
  existing `phg build` cross-compile machinery. No new `unsafe` in phorj (generated Rust is safe;
  rustc handles codegen). Strongest policy + philosophy alignment.
- **−** **AOT only** — an explicit `phg build` step; compile latency is rustc-seconds, so it can't
  power interactive `phg run` (the VM stays for that). "phorj vs php on a raw script" needs
  `phg build x.phg && ./x` vs php's runtime JIT — a different UX (but production runs the prebuilt
  native binary, which beats php+JIT outright). Mapping every phorj construct to safe Rust is real
  work (closures, COW `Value`, faults, the `Rc` heap).

## The key architectural insight (independent of A/B/C)
phorj already has **two execution surfaces**: `phg run` (interactive, VM) and **`phg build`**
(standalone binary — today it *embeds source + runs it on the VM at startup*). The native backend's
natural slot is **`phg build` → compile to real native code** (not embed-run-on-VM), while **`phg run`
keeps the VM** (fast startup for dev/scripts). This gives:
- dev/scripts: VM (startup-fast, php-competitive-ish, already shipped),
- production: native AOT (crushes php+JIT — native, no interpreter, no JIT warmup).

Cranelift (A) additionally could JIT *within* `phg run` for a runtime-adaptive story matching php.

## Scored comparison (against phorj's constraints)
| Criterion | A. Cranelift JIT | B. LLVM | C. transpile→rustc AOT |
|---|---|---|---|
| Dep policy fit | ✗ needs amendment | ✗✗ worst | ✓ no new dep |
| `forbid(unsafe)` fit | ✗ needs exception | ✗ needs exception | ✓ generated Rust is safe |
| Codegen quality | good | best | best (rustc=LLVM) |
| `phg run` (interactive) win | ✓ runtime JIT | ~ | ✗ (VM stays) |
| `phg build` (production) win | ✓ | ✓ | ✓ |
| Reuses existing infra | new subsystem | new subsystem | ✓ transpiler + phg build |
| Byte-identity risk | 3rd backend | 3rd backend | 3rd backend (all equal) |
| Effort | high | highest | high (but familiar shape) |

## THE deciding fork (must be ruled first — everything hinges on it)
**Does "faster than PHP" mean the *interactive* `phg run script.phg` path, or the *production* prebuilt
binary?** These have opposite answers:

- **Interactive `phg run`** (maps to how people invoke `php script.php`) → **only a runtime JIT
  (A/Cranelift) delivers it.** Approach C leaves `phg run` on the VM — still 9–154× slower — and forces
  `phg build x.phg && ./x` (rustc-seconds latency + rustc on the user's machine) to get speed, while
  PHP just runs and JITs transparently. If the mandate is the typed command (everything this session
  says it is: we benchmarked `phg run`-equivalents vs `php`; we made `phg run`=VM *so users get speed*;
  the framing is "why aren't we faster than PHP"), then **C is at best a stepping stone and Cranelift is
  on the critical path** — the dep-policy amendment is required, not "revisit later".
- **Production binary** → **C (transpile→rustc AOT) is right and sufficient**, no dep-policy amendment,
  no new `unsafe`, reuses the transpiler + `phg build` machinery, LLVM-tier codegen.

This is the developer's product call — not decided here. But the likely read (interactive) points at A.
Reject B (LLVM) regardless — costs dwarf its codegen edge over rustc.

## Open developer forks (§15 — must be ruled, not decided autonomously)
1. **[PRIMARY] Interactive vs production mandate** (above) — decides A vs C, and whether the dep-policy
   amendment is on the critical path.
2. **If interactive (→ A/Cranelift): amend the dependency policy** to admit a performance/codegen
   domain (currently *explicitly excluded*) — a first-of-its-kind exception (feature-gated, non-wasm,
   corosensei-shaped `unsafe` confinement). This is the governance gate for the whole JIT.
3. **Execution shape:** VM for `phg run` + native for `phg build` (C), or VM+JIT unified in `phg run`
   (A)? (Falls out of #1.)

## Staged build plan (once the fork is ruled)
1. **Spike (approach-AGNOSTIC — validates native codegen for BOTH A and C; do this next regardless of
   how the fork rules):** hand-transpile `fib` to native, compile with rustc, measure vs php+JIT — but
   **bracket the representation**: measure BOTH a native-`i64` version (proves the *ceiling*) AND an
   `Rc`-boxed-enum-`Value` version (proves what the *real* transpiler achieves before the unboxing
   pass). The gap between them is the prize step 5 must capture. ⚠ If boxed-`Value` fib does NOT beat
   php+JIT, that is a critical early finding (unboxing becomes mandatory, not optional), not a footnote.
2. Rust-emitter for the arithmetic/control-flow core (int/float, if/while/for, calls) → differential
   against the VM on the numeric example subset.
3. `Value` runtime in generated code (the `Rc` heap, COW, faults) → widen to collections/objects.
4. Wire into `phg build` (replace embed-run-on-VM with compile-to-native); byte-identity gated.
5. Unboxing pass for statically-typed hot paths (the codegen win that closes the last gap).

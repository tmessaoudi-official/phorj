# Architecture / File-Decomposition Plan (Invariant 13 M-Decomp campaign)

> Ratified direction (dev, 2026-07-23): shrink the big files into a better architecture + folder
> structure. **79 files exceed the 500 hard cap** (all grandfathered → shrink-only). This is the
> prioritized roadmap. Each split is a BEHAVIOR-PRESERVING move (cohesion split, `pub(super)`,
> glob re-export) verified by the full gate (differential byte-identity + JIT tests) — never a
> logic change. Split by COHESION, never by line count.

## Why now — decomp unblocks perf
Every new JIT vertical (the perf campaign) touches `analyze.rs`, `emit_unboxed/mod.rs`, and
`verticals.rs` — all grandfathered giants. The last two verticals (`listcontains`, and the pending
`mapkeys`…) each fight the size ratchet. Splitting these THREE first gives every remaining perf
vertical a home with headroom, so perf work stops paying the ratchet tax.

## The M-Decomp method (established, proven on `runtime_tables.rs` + `list_contains.rs`)
1. Identify a cohesive cluster (a type + its algebra, a predicate family, one vertical family).
2. Move it to `foo/<cluster>.rs` (or a sibling `foo_<cluster>.rs`), `pub(super)` the moved items.
3. Parent keeps a `mod <cluster>;` + `use <cluster>::*;` (or fully-qualified calls).
4. The parent file SHRINKS below its baseline; the new file is well under cap.
5. Gate: fmt + size-gate + clippy(both legs) + full workspace under the PHP oracle. Byte-identity
   is the safety net — a bad move fails the differential.

## PRIORITY 1 — JIT subsystem (unblocks the perf campaign)

### `src/jit/analyze.rs` (2869) → `src/jit/analyze/`
- `analyze/kind.rs` — `Kind`, `Own`, their impls + the kind-algebra (`borrowed_copy`,
  `field_read_kind`, `join_kind`, `join_unknown_bottom`, `abi_param_kinds`, `is_*_kind`). ~330 lines.
- `analyze/natives.rs` — the `unboxed_native_is_*` predicate family + `unboxed_native_bridge2`
  shape table. ~150 lines (this is where a new vertical's predicate goes — headroom for the campaign).
- `analyze/pass.rs` — the `unboxed_analyze` fixpoint + its big per-`Op` match. The bulk (~2000).
- `analyze/mod.rs` — re-exports (`pub(super) use`), keeps the module seam.

### `src/jit/emit_unboxed/mod.rs` (1988) → keep `mod.rs` as the driver, extract families
- The giant per-`Op` emit match stays in `mod.rs` (it's the dispatcher — a cohesive exhaustive
  match), but the ARM BODIES that are >a couple lines move to family files that already exist
  (`verticals.rs`, `list_contains.rs`, `concat.rs`, `objects.rs`, `scalar.rs`, `enums.rs`), so the
  match arms become one-line `family::arm_x(...)?` calls. Target: `mod.rs` < 500 over time.
- New `emit_unboxed/verticals/` DIRECTORY (convert the single `verticals.rs` 1264 → a folder):
  `verticals/mod.rs` (re-export) + `verticals/set.rs`, `verticals/map.rs`, `verticals/list.rs`,
  `verticals/index.rs`, `verticals/hof.rs`. `list_contains.rs` folds in as `verticals/list.rs`.
  **This is the home for every remaining perf vertical** (mapkeys/mapvalues → `verticals/map.rs`,
  the HOF folds → `verticals/hof.rs`).

### `src/jit/handles.rs` (2280) → `src/jit/handles/`
Split by handle family (the runtime `rt_u_*` helpers): `handles/list.rs`, `handles/map.rs`,
`handles/set.rs`, `handles/str.rs`, `handles/mod.rs`. Each groups a helper family.

### `src/jit/tests/verticals.rs` (2423) → `src/jit/tests/verticals/`
Per-family test files mirroring the emit split (`tests/verticals/set.rs`, `map.rs`, …).
`listcontains.rs` already models this.

## PRIORITY 2 — the other worst offenders (not perf-blocking; steady cleanup)
- `src/checker/desugar_db.rs` (3144) → `desugar_db/` by phase (parse / bind / lower / emit).
- `src/cli/explain.rs` (1998) → `cli/explain/` by topic group (one file per command family).
- `src/transpile/runtime_php.rs` (1366) → `transpile/runtime_php/` by helper family (already
  started: reflect helpers moved to `runtime_tables.rs`). Continue: json/, text/, log/, math/.
- `src/cli/preludes.rs` (1196) → `cli/preludes/` one file per injected module's source string.
- `src/vm/exec.rs` (1053) → `vm/exec/` by op-family (arith / control / collection / call).
- `src/loader/mod.rs` (1029) → `loader/` split (already has `resolve.rs`; add `attribution.rs`,
  `passes.rs`).
- Remaining ~70 files in the 500–1000 band: split opportunistically AS each is next edited
  (split-as-you-go — Invariant 13's default), lowest-risk.

## Sequencing
1. `emit_unboxed/verticals/` folder + `analyze/natives.rs` FIRST — smallest, highest leverage,
   directly unblocks the perf verticals. (Do before/with the next perf loss.)
2. `analyze/{kind,pass}.rs`, `handles/` — larger, do as dedicated slices.
3. Priority-2 files: split-as-you-go.

Each split = its own commit, gate-green, byte-identity-verified. No behavior change ever.

## Status
PLAN ratified 2026-07-23. Not yet executed (this doc is the "think about" deliverable). The perf
campaign continues in parallel; new verticals land in `emit_unboxed/verticals/` per this plan.

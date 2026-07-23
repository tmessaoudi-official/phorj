# VM+JIT vs PHP 8.5.8+JIT — full micro scorecard (2026-07-23) — ⚠ HARD FLAG

> **WIN-OR-FLAG mandate (dev, 2026-07-23): "everything must beat php with no compromise; if you
> can't reach it, hard flag."** This report is that flag. Under the baseline described below, **18 of
> 48 micros LOSE** to php+JIT — several by 3–16×. They are surfaced here, not silently accepted.

## UPDATE 2026-07-23 — 1 of 18 CLOSED: `listcontains` 0.06x -> 1.97x WIN
Added a **`List.contains` JIT unboxed vertical** (inline linear scan of the flat int block,
byte-identical to the interpreter's `list_contains`; non-flat list -> code-5 VM redo), mirroring the
existing `Set.contains` vertical. Measured: VM 892ms -> 26ms (~34x faster), flipping the worst loss
(0.06x) to a 1.97x WIN. Byte-identity proven (JIT == VM == tree-walker checksums; differential
172/172; new `src/jit/tests/listcontains.rs`). No regression (setcontains 1.02x, listindex 1.51x,
listmap 8.59x unchanged). **17 losses remain** — same pattern (Map/Set HAMT extraction, HOF folds,
string-scan, JSON), each its own vertical / representation slice.

## UPDATE 2026-07-23 (2) — 2 of 18 CLOSED: `sumby` 0.34x -> ~17x WIN
Extended the existing `List.map`/`count` **hofpipe vertical** to `List.sumBy` (the same inline native
loop — one direct call per element, no VM re-entry — with a CHECKED `sadd_overflow` accumulator; an
overflow carry -> code-5 VM redo reproduces `list_sum_by`'s exact `"integer overflow in List.sumBy"`
fault byte-for-byte). This directly disproves the stale 2026-07-20 note that "re-entrant HOF folds
cannot be won by verticals" — the win comes precisely from eliminating the per-element re-entrant
dispatch. Measured (phg-JIT vs php-8.5.8+JIT, opcache.jit=tracing confirmed on): **14.9M ns vs 254M
ns = ~17x WIN**, checksums identical (`20000000`). Byte-identity proven (JIT == VM == tree-walker;
differential 172/172; new `src/jit/tests/sumby.rs` — delivery `hits>0` + capture/negative/empty edges
+ the overflow redo). Enabler: `arm_list_hof` M-Decomp-extracted `verticals.rs` -> `verticals_hof.rs`
(Inv 13) to make room for the fold accumulator modes. **16 losses remain.** NEXT: `listreduce` (0.30x)
— unboxed-clean (result = seed type `U`; seed operand + 2-arg `(acc,elem)` callback). ⚠ **`maxby`/`minby`
(0.19–0.20x) are BLOCKED (HARD FLAG, dev to rule):** they return `T?` and the unboxed `Kind` enum has NO
nullable variant, so the element result can't stay unboxed — needs a representation lever (add an `Int?`
arena kind / restrict to non-empty-`??` peephole / accept the flag). Then the `mapkeys`/`mapvalues`/
`mapmerge` HAMT-extraction + string-scan + JSON clusters, each its own slice.

## Methodology (and its one caveat)

- `scripts/microbench.sh`, K=5, interleaved + core-pinned (`taskset`), output-identity gated
  (checksums matched on every row shown — both legs did equivalent work).
- phorj: `target/release/phg run <micro>.phg` — **VM + JIT** (default; JIT confirmed engaging, see
  evidence below).
- **CAVEAT — baseline php is a FROM-SOURCE build, not the docker image.** The org proxy blocks docker
  image pulls in this container, so the canonical `docker run php:8.5-cli` path was unavailable. I
  built **PHP 8.5.8 from source** (`--enable-cli --enable-bcmath --enable-mbstring --with-openssl
  --enable-opcache`), run with `-dopcache.enable_cli=1 -dopcache.jit_buffer_size=128M
  -dopcache.jit=tracing` (`jit.enabled==true` verified). This is a legitimate real php+JIT, but it is
  NOT byte-identical to the official image (CFLAGS/build differences possible). **This contradicts the
  MASTER-PLAN's recorded "jsonround/dbwork = wins" — that discrepancy MUST be reconciled on the dev's
  box against the official docker baseline before treating these as confirmed regressions.**
- To reproduce here: `MICROBENCH_PHP_BIN=<from-source 8.5.8 cli> bash scripts/microbench.sh`
  (the new local-php mode; docker stays the default when `MICROBENCH_PHP_BIN` is unset).

## Scorecard (ratio = php_ns / vm_ns; >1 = phorj WIN)

### WINs (27) — phorj beats php+JIT
scalar/arith/control-flow/OOP/closures dominate: trycatch 34.22×, listmap 8.57×, match 8.05×,
objalloc 8.03×, mathsign 6.83×, mathmin 6.81×, mathmax 6.67×, mathabs 6.50×, floatarith 6.00×,
hofpipe 5.93×, closurecall 4.12×, enum 4.02×, webish 2.93×, interp 2.64×, maphas 2.54×,
methodcall/fibrec 2.43×, strbuild 2.28×, stringconcat 2.06×, intadd 2.02×, mapget 1.54×,
listindex 1.51×, setintersection 1.37×, forin 1.36×, mapinsert 1.27×, setcontains 1.04×,
listappend 1.02×.

### LOSSES (18) — ⚠ php+JIT faster — HARD FLAG
| feature | ratio | class |
|---|---|---|
| listcontains | 0.06× | boxed-`Value` linear scan native (php `in_array` = fast C) |
| mapkeys | 0.09× | Map(HAMT) key extraction vs php array_keys |
| mapvalues | 0.09× | Map(HAMT) value extraction |
| mapmerge | 0.12× | Map(HAMT) merge |
| stringcontains | 0.16× | string scan |
| maxby | 0.19× | HOF fold |
| minby | 0.20× | HOF fold |
| listfilter | 0.22× | HOF filter |
| mapfilter | 0.23× | HOF filter over Map |
| isurl | 0.23× | string/regex scan |
| isemail | 0.24× | string/regex scan |
| mapmap | 0.29× | HOF map over Map |
| listreduce | 0.30× | HOF fold |
| jsonround | 0.32× | JSON encode/decode |
| sumby | 0.34× | HOF fold |
| setdifference | 0.45× | Set(HAMT) op |
| setunion | 0.66× | Set(HAMT) op |
| floatloop | 0.82× | JIT'd float loop, constant-factor |
| floatmul | 0.99× | JIT'd float loop, constant-factor (near-tie) |
| dbwork | 0.82× | (sqlite) |
| deepjson | 0.90× | JSON |

## Root cause (evidence, `phg run` vs `phg run --no-jit`, self-timed ns)
- **floatmul**: JIT 5.17M ns vs no-JIT 490.7M ns → **JIT ~95×**. Fully JIT'd; the 1% gap is a
  constant-factor in float codegen. **Marginal, plausibly closeable** with a codegen tweak (needs a
  proven before/after).
- **listcontains**: JIT 874.5M ns vs no-JIT 1141.7M ns → **JIT only ~1.3×**. The JIT compiles the
  loop but each iteration is a `List.contains` **native call over boxed `Value`** (~36 ns/element vs
  php's ~2 ns in C). The native is the floor; the JIT cannot inline it.
- **listmap** (a WIN, for contrast): JIT 28.9M vs no-JIT 932M → JIT ~32×; wins 8.57× because php's
  *closure* dispatch is slow — phorj wins HOFs-with-closures but loses raw C-builtin-shaped ops.

**Diagnosis:** the losses cluster on (1) per-element **native calls over boxed immutable `Value`
collections** (contains/keys/values/merge), (2) **HOF folds** where the result is scalar but the VM's
per-element closure/native dispatch dominates, (3) **string/regex scanning**, (4) **JSON**. php wins
these because they map to hand-optimized C builtins over flat mutable arrays.

## What this needs (NOT a night fix — architectural, dev to prioritize)
- Unboxed / specialized collection representations (flat primitive vectors) with JIT-visible fast
  paths for scan/keys/values/fold — so the JIT can eliminate the per-element native boundary.
- Native fast-paths for `contains`/`keys`/`values`/`merge` that avoid `Value` boxing on primitive
  element types.
- String-scan (`contains`/regex) and JSON encode/decode hot-path review.
- The two float near-ties (floatmul/floatloop) are separable, smaller codegen wins.

## Status
HARD-FLAGGED, unresolved. No speculative fix applied (Rule 14 — no fix without a proven before/after).
Reconcile against the official docker php:8.5-cli baseline on the dev box; if confirmed, this is a
WIN-OR-FLAG breach requiring the collection-representation work above.

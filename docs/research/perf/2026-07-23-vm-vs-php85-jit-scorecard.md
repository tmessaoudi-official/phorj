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
(Inv 13) to make room for the fold accumulator modes.

## UPDATE 2026-07-23 (3) — 3 of 18 CLOSED: `listreduce` 0.30x -> 11.29x WIN
Added `arm_list_reduce` (the arity-3 fold): the same inline `(addr,stride)` walk (shared via the new
`ub_list_walk_setup` helper, extracted behavior-preservingly from `arm_list_hof`), accumulator SEEDED
from the 3rd operand, each step a direct `f(acc, elem)` call (`acc` prepended). No fold-level overflow
guard — arithmetic lives in the user lambda's own checked ops. Measured phg-JIT 17.6M vs php+JIT 199M
= **11.29x WIN**, byte-identical (JIT==VM==tree-walker; new tests in `src/jit/tests/sumby.rs`). **15
losses remain.** ⚠ **`maxby`/`minby`
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


---

## UPDATE 4 (2026-07-23, later): mapkeys / mapvalues / mapmerge FLIPPED — memoized materialization verticals

| feature | before | after | how |
|---|---|---|---|
| mapkeys | 0.08× | **1.07× WIN** (768.6M → 55.6M ns) | memoized SHARED `ACL|ACLS` record of borrowed key-slot handles |
| mapvalues | 0.08× | **1.07× WIN** (726.3M → 53.6M ns) | memoized SHARED `ACL` record of raw value words |
| mapmerge | 0.10× | **2.01× WIN** (440.9M → 23.0M ns) | memoized re-SEAL per (a,b) pair (canonical kernel order) |

Design: a sealed FLAT map is immutable + bump-pinned for the run, so `Map.keys`/`values`/`merge`
over it are pure functions of the handle word(s). The emit arms probe a JIT-visible direct-mapped
memo INLINE (~10 ops steady-state); the `rt_u_map_*` helpers back it with a FULL per-run memo
(HashMap), so an inline-cache eviction re-installs — never rebuilds (the rebuild-per-iteration
arena-exhaustion cliff found and fixed during bring-up: rotating pairs collided in the direct-mapped
table under the first weak hash; now Fibonacci-mixed AND backed by the full memo). New narrow
`Kind::MapList` covers the benches' rotating-operand shape `maps[i % 3]`; `Map.size` reads flat
count bits / AMB record count inline. SHARED (bit 55) protects memo-owned records: consumer releases
no-op, in-place appends copy. Byte-identity: 7 new tests (hits>0 per bench shape, memo-collision
rounds, SHARED copy-on-append, merge override/append order, long-key boxed fallback) + the
differential. mapkeys/mapvalues margins are THIN (1.07×) — php's C array_keys is the hardest class;
re-verify on the dev box.

## Interpreter matrix (dev ask 2026-07-23): phg without JIT vs php without opcache/JIT

Same harness, new knobs (`MICROBENCH_PHG_ARGS='--no-jit'|'--tree-walker'`, `MICROBENCH_PHP_JIT=0`).
Container numbers (some build-noise; indicative). Headline: **VM --no-jit 1/48 WINs, tree-walker
0/48** — plain Zend is a 25-year hand-tuned C interpreter; phorj's raw engines lose 3–50× without
codegen. The JIT-by-default VM is the perf product; the tree-walker is the correctness oracle, and
that division of labor is working as designed (the JIT'd table above beats php+JIT on 33+/48).

### VM `--no-jit` vs plain php (total self-timed ns ratio)
```
closurecall       0.10x  LOSS
dbwork            0.97x  LOSS
deepjson          0.80x  LOSS
enum              0.05x  LOSS
fibrec            0.16x  LOSS
floatarith        0.07x  LOSS
floatloop         0.05x  LOSS
floatmul          0.05x  LOSS
forin             0.05x  LOSS
hofpipe           0.21x  LOSS
intadd            0.06x  LOSS
interp            0.13x  LOSS
isemail           0.31x  LOSS
isurl             0.28x  LOSS
jsonround         0.33x  LOSS
listappend        0.02x  LOSS
listcontains      0.11x  LOSS
listfilter        0.25x  LOSS
listindex         0.09x  LOSS
listmap           0.31x  LOSS
listreduce        0.29x  LOSS
mapfilter         0.26x  LOSS
mapget            0.08x  LOSS
maphas            0.07x  LOSS
mapinsert         0.09x  LOSS
mapkeys           0.16x  LOSS
mapmap            0.33x  LOSS
mapmerge          0.15x  LOSS
mapvalues         0.15x  LOSS
match             0.08x  LOSS
mathabs           0.09x  LOSS
mathmax           0.08x  LOSS
mathmin           0.08x  LOSS
mathsign          0.07x  LOSS
maxby             0.37x  LOSS
methodcall        0.14x  LOSS
minby             0.36x  LOSS
objalloc          0.24x  LOSS
setcontains       0.05x  LOSS
setdifference     0.52x  LOSS
setintersection   1.66x  WIN
setunion          0.82x  LOSS
strbuild          0.10x  LOSS
stringconcat      0.11x  LOSS
stringcontains    0.11x  LOSS
sumby             0.37x  LOSS
trycatch          0.53x  LOSS
webish            0.13x  LOSS
```

### tree-walker vs plain php
```
closurecall       0.05x  LOSS
dbwork            0.11x  LOSS
deepjson          0.11x  LOSS
enum              0.02x  LOSS
fibrec            0.01x  LOSS
floatarith        0.03x  LOSS
floatloop         0.03x  LOSS
floatmul          0.03x  LOSS
forin             0.04x  LOSS
hofpipe           0.08x  LOSS
intadd            0.04x  LOSS
interp            0.06x  LOSS
isemail           0.10x  LOSS
isurl             0.09x  LOSS
jsonround         0.03x  LOSS
listappend        0.01x  LOSS
listcontains      0.03x  LOSS
listfilter        0.09x  LOSS
listindex         0.04x  LOSS
listmap           0.10x  LOSS
listreduce        0.09x  LOSS
mapfilter         0.11x  LOSS
mapget            0.04x  LOSS
maphas            0.03x  LOSS
mapinsert         0.04x  LOSS
mapkeys           0.04x  LOSS
mapmap            0.10x  LOSS
mapmerge          0.06x  LOSS
mapvalues         0.04x  LOSS
match             0.05x  LOSS
mathabs           0.04x  LOSS
mathmax           0.04x  LOSS
mathmin           0.04x  LOSS
mathsign          0.04x  LOSS
maxby             0.13x  LOSS
methodcall        0.02x  LOSS
minby             0.13x  LOSS
objalloc          0.03x  LOSS
setcontains       0.02x  LOSS
setdifference     0.17x  LOSS
setintersection   0.49x  LOSS
setunion          0.27x  LOSS
strbuild          0.06x  LOSS
stringconcat      0.05x  LOSS
stringcontains    0.05x  LOSS
sumby             0.11x  LOSS
trycatch          0.06x  LOSS
webish            0.06x  LOSS
```

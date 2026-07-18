# Profiling example — `workload.phg`

A focused workload for measuring the two M2 backends, plus a guide to **how** Phorj collects the
time and memory numbers. (Phorj the language has no clock or syscall surface — `println` is the only
builtin — so a `.phg` program *cannot* profile itself. All measurement lives in the Rust `bench`
tool; this page documents that mechanism.)

## What the workload exercises

| Section | Cost center |
|---|---|
| `fib(18)` | **CPU** — exponential call volume, shallow recursion. Where the bytecode VM's per-call overhead advantage over the tree-walker shows up. |
| `allocateChain(1000)` | **Heap + stack** — 1000 `Cell` instances simultaneously live (each recursion frame keeps its `c` alive across the call) on the `Rc`-shared object heap, 1000-deep recursion. |
| `for (Cell c in […])` | **Object access** — a method call + field read per list element. |

It runs byte-identically on both backends (gated by `tests/differential.rs`, which
globs `examples/**/*.phg`).

## Running it

```sh
phg benchmark  examples/benchmark/workload.phg            # per-phase wall-clock + memory
phg benchmark --vs-php examples/benchmark/workload.phg    # + a 3-way comparison against transpiled PHP
phg disassemble examples/benchmark/workload.phg           # the bytecode the VM executes
phg run --tree-walker examples/benchmark/workload.phg    # tree-walking interpreter oracle
phg run    examples/benchmark/workload.phg                # bytecode VM
```

`benchmark` runs the whole program **101×** (median of 101, one untimed warmup), so it takes several
seconds — that's the sampling cost, not the program's runtime.

## `--vs-php` — who's the winner?

`phg benchmark --vs-php <file>` adds a head-to-head against the **PHP backend**: it transpiles the
program to PHP, runs it once to **gate output identity** (a transpile divergence aborts the
comparison — it's a bug report, not a timing result), then median-times `php <file>` the same way.
The report gains a `vs PHP` section naming the faster of the Phorj VM and PHP. Requires `php` on
PATH; absent it, the section is a graceful skip note.

A representative run on this workload (`fib(18)` + 1000-instance allocation):

```
  tree-walk run 89.078 ms
  vm run        11.910 ms
verdict: vm run is 7.48× faster than tree-walk run

vs PHP — PHP 8.6.0-dev (cli)
  php run       38.519 ms  (spawns a process per sample)
  winner: Phorj (vm) — 3.23× faster than PHP (38.519 ms → 11.910 ms)
```

**Read it honestly:** this is the Rust bytecode VM vs the PHP interpreter on the *same algorithm* —
informative, but apples-to-oranges (different runtimes). The PHP timing includes process spawn and
depends on whether opcache/JIT is enabled in your `php.ini` (the figure above is a debug PHP build, so
a tuned PHP would close the gap). The number to trust most is the in-process interpreter-vs-VM verdict.

## How execution time is collected

`bench` times four phases with the standard-library monotonic clock (`std::time::Instant`), each as
the **median of 101 samples** after one untimed warmup (warmup pays one-time allocation/cache costs
outside the measured window; the median rejects scheduler-jitter outliers a mean would absorb):

- `parse+check` — front-end (lex → parse → type-check)
- `compile` — AST → bytecode (one-time, VM only)
- `tree-walk run` — the interpreter executing the program
- `vm run` — the bytecode VM executing the program

Before any timing, an **output-identity gate** runs both backends once and aborts the benchmark if
their stdout differs — comparing the speed of two backends that *disagree* would be meaningless
(this is the differential harness's parity contract, enforced at run time).

## How memory is collected

Memory sampling (`src/mem.rs`) is **Linux-only and std-only** — no crates, no `unsafe`, just reading
and writing files under `/proc/self`:

- **current RSS** (`VmRSS`) and **peak RSS** (`VmHWM`) are parsed out of `/proc/self/status`.
- The peak is a kernel-tracked high-water mark. Writing `5` to `/proc/self/clear_refs` (Linux ≥ 4.0)
  **rewinds** `VmHWM` down to the current `VmRSS`, so a single execution's *growth* can be isolated.

`bench` reports:

- **`cold run +N RSS`** — the peak-RSS growth of **one tree-walk execution from a cold heap**,
  measured *before* the timing loops run. This is the honest per-execution figure: it must be taken
  cold because once glibc has mapped pages for the heap it almost never returns them to the OS, so a
  post-warmup or sequential per-backend measurement reads ~0 KiB and would mislead. (True
  per-backend attribution would need a fresh process per backend — out of scope for this tool.)
- **`process peak`** (`VmHWM`) and **`resident now`** (`VmRSS`) — the bench process's lifetime
  high-water mark and current resident set.

On any non-Linux host (including the cross-built Windows/macOS binaries), `/proc` is absent: every
sampling function returns `None` and `bench` prints `memory: unavailable on this platform` instead
of failing.

## Beating PHP: the JIT (`phg run`, feature `jit`)

The perf mandate (G-8) is *phorj must be faster than PHP, per feature*. The lever is the Cranelift
JIT (`src/jit/`): built with `--features jit`, `phg run` compiles unboxed-eligible functions —
self/cross-recursive `int` functions with no mutable locals (no `SetLocal`) whose every return is a
provably-`int` operand — to native code, routing the VM's `Op::Call` to it (with a VM-fallback on any
fault so error rendering is unchanged). `examples/fib.phg`'s `fib` is the canonical eligible shape.

The honest comparison is **`scripts/microbench.sh`**: the phorj VM (your jit binary) vs a *real*
release **`php:8.5-cli` with opcache+JIT in Docker** (the on-box php builds are ZTS-debug, JIT off —
not a valid baseline). Each micro self-times and prints a checksum that gates output-identity before
any timing is trusted.

```console
$ cargo build --release --features jit
$ PHG_BIN=target/release/phg bash scripts/microbench.sh
feature              VM ns/op      php+JIT     ratio  verdict
fibrec               14259872     33920305     2.38x  WIN      # recursive fib(32), native
intadd                    239            1     0.00x  LOSS     # iterative — still on the VM
...
```

**`fibrec` (recursive `int` fib) is a WIN vs release php+JIT** — ~2.4× best-case on a shared box;
the robust claim is the WIN itself (per-feature WIN/LOSS is the gated signal — absolute ratios swing
with machine load, so `microbench-gate.sh` ratchets WIN→LOSS flips, not magnitude). The other micros
still LOSE because they are *iterative* (`mutable`/`while` = `SetLocal`, outside the current unboxed
subset) and so still run on the VM — widening the subset to loops is future work.

> Use `microbench.sh` / `phg run` for the php comparison, **not** `phg benchmark`: a self-timing
> micro like `fibrec` emits a different nanosecond count each run, which trips `phg benchmark`'s
> output-identity gate. `phg benchmark` compares phorj's own backends on a *deterministic* program
> (e.g. `phg benchmark examples/fib.phg`); it also honours the JIT under a jit binary.

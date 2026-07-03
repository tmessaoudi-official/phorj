# A6 — Performance & Efficiency Audit (runtime slowness)

Auditor: parallel agent A6 (batch 2), 2026-07-03. HEAD `0691228`, clean tree.
Dimension: what makes the language runtime slow — VM/interpreter hot paths, allocations,
algorithmic complexity, test-gate latency, build time.

Machine context: 8 physical cores; release binary `target/release/phg` rebuilt this session.
PHP oracle used: `/stack/tools/phpbrew/php/php-8.5.7/bin/php` (note: it is a **ZTS DEBUG build
with Xdebug active** — see F9).

---

## Executive summary

- The VM is 17–23× faster than the tree-walking interpreter on call/alloc-heavy workloads —
  as intended [Verified: measured, F7].
- **Inversion found**: on a string-concat loop the VM is 1.53× *slower* than the interpreter
  [Verified: measured, F1]. Root cause is the non-Rc `Value::Str(String)` + per-`GetLocal`
  deep clone (F1/F2).
- The "cmd_run global lock" in project memory is **not a Mutex** — no lock exists. The
  serializer is `on_deep_stack` spawning a fresh 256 MiB-stack thread per `cmd_run`/`cmd_runvm`
  call; measured: it costs ~97 µs/call and cuts 8-way parallel scaling from 7.2× to 4.8×
  [Verified: standalone microbenchmark, F5].
- Stdlib `List.append` is O(n) per call by documented design → O(n²) loops; measured 66 ms
  interpreter / 26 ms VM for a 3000-append program (F3).
- The two slow gate tests identified: `fmt::every_repo_phg_formats_idempotently_and_safely`
  (**200.5 s**) and `runtime::shipped_manual_example_runs_on_both_backends` (**154.7 s**) —
  single monolithic corpus-sweep test functions that nextest cannot parallelize; the whole
  216.9 s full-oracle run is critical-pathed on the first (F6, F10)
  [Verified: full-oracle nextest run this session, 1672/1672 green].
- `--vs-php` benchmark numbers on this machine are invalid as comparisons: the PHP is a
  DEBUG+Xdebug build and the timing includes process spawn (F9).

---

## F1 — `Value::Str(String)` is the only non-shared Value variant; string workload inverts run/runvm

**Files**: `src/value.rs:124` (`Str(String)`), `src/vm/exec.rs:149-153` (`Op::GetLocal` clones).

Every other compound variant is `Rc`-shared and O(1) to clone (`Bytes(Rc<Vec<u8>>)`,
`List(Rc<Vec<Value>>)`, `Map`, `Set`, `Instance`, `Enum`, `Closure` — `src/value.rs:126-149`,
each with explicit "cloning is a refcount bump" doc comments). `Str(String)` deep-copies its
entire buffer on every `Value::clone()`.

Consequences in hot paths:
- `Op::GetLocal` (`src/vm/exec.rs:151`): `self.stack[idx].clone()` — every read of a string
  local copies the whole string. A loop growing an accumulator string does O(len) copy per
  *read*, on top of the O(len) concat itself.
- `Value::as_display()` (`src/value.rs`, `Str(s) => Some(s.clone())`): string interpolation
  (`Op::Concat`, `src/vm/exec.rs:168-180`) allocates a fresh `String` per part even when the
  part already is a string — a `Cow<'_, str>`/borrow would avoid it.

**Measured evidence** (`phg benchmark`, median of 101, this session):
string-concat loop (4000 iterations of `acc = acc + "x{i}-";`, final length 22,890 bytes):

```
tree-walk run 3.157 ms
vm run        4.822 ms   ← VM 1.53× SLOWER than the interpreter
```

The VM should never lose to the tree-walker; on string-heavy code it does. The interpreter
suffers the same non-Rc Str cost but touches the value fewer times per iteration.
[Verified: `phg benchmark` output captured this session; file scratchpad/bench-results.txt]

**Fix direction** (P2): make `Str` shared (`Rc<str>` or `Rc<String>` — with a COW
`make_mut`-style path for the mutating natives), or at minimum stop cloning on `GetLocal` for
the concat operand path. Any change is parity-affecting surface — full differential gate + a
before/after `phg benchmark` required (Invariant 11).

Severity: **P2** — correctness unaffected, but this is the single biggest user-visible runtime
inefficiency found, and it inverts the documented "VM is the fast backend" contract on a whole
workload class.

## F2 — Per-dispatch clones on the VM call/construct paths

All read-only observations [Verified: read the code at the cited lines]; each is a String or
descriptor allocation per executed op:

| Site | What is cloned | Frequency |
|---|---|---|
| `src/vm/exec.rs:666` (`Op::CallMethod`) | `self.program.names[name_idx].clone()` — method name `String` | every dynamic method call |
| `src/vm/exec.rs:671` (`Op::CallMethod`) | `inst.class.clone()` — class name `String` | every dynamic method call |
| `src/vm/exec.rs:499` (`Op::MakeEnum`) | `EnumDesc` clone = 2 `String`s | every enum-value construction |
| `src/vm/exec.rs:508` (`Op::MatchTag`) | `variant.clone()` `String` | every match-arm tag test |
| `src/vm/exec.rs:536` (`Op::MakeInstance`) | `ClassDesc` clone = class `String` + `Vec<String>` field names | every `new` |
| `src/vm/mod.rs:220` | `code[ip].clone()` per dispatched op | cheap for index-only ops; allocates only for `Panic(String)`/`Assert(String)`/`IsInstance(String)` variants (rare) — acceptable |

The `(class, mname)` `String`-pair `HashMap` key for method dispatch (exec.rs:690) means two
heap allocations + string hashing per call even on the non-overloaded path. `Op::GetField` by
contrast already has an inline cache that avoids the name clone on monomorphic sites
(exec.rs:561-567) — the same technique would apply to `CallMethod`. The workload.phg method-call
benchmark still shows the VM 17× ahead of the interpreter, so this is polish, not a fire.

Severity: **P3** (interned name indices / `Rc<str>` names / call-site inline cache would remove
all of these; only worth it with a measured before/after per Invariant 11).

## F3 — `List.append` in a loop is O(n²) — documented, but the doc lives only in Rust source

**File**: `src/native/list.rs:232-241`.

```rust
let mut out = (**xs).clone();   // full Vec copy per append
out.push(v.clone());
```

The doc comment says: "Lists are immutable (COW), so this returns a fresh list; for building in
a hot loop prefer `List.fill` + index-set (O(1) per write since M-DOGFOOD W8) or `List.map`."
So the trap is known and a fast path exists (`Op::SetIndexLocal` in-place COW,
`src/vm/exec.rs:245-260`, uses `Rc::make_mut` at refcount 1). Two problems remain:

1. **No refcount-1 fast path in `append` itself.** When the arg's `Rc` strong count is 1 the
   copy is still made (the natives receive `&[Value]`, so the callee cannot take ownership —
   a signature change or `Rc::strong_count == 1` + unsafe-free `Rc::try_unwrap` on an owned arg
   path would be needed). Same shape in `list_concat` (list.rs:218-228).
2. **The guidance is invisible to users** — it exists as a Rust doc comment, not in
   FEATURES.md / examples. A user writing the natural `xs = xs.append(x)` loop gets quadratic
   behavior with no warning.

**Measured**: 3000-append + filter/map/sum program: interpreter 66.6 ms, VM 25.9 ms — for a
program that would be sub-millisecond with amortized growth
[Verified: `phg benchmark` this session]. String `+` in a loop is equally O(n²) (F1) — also
matches PHP semantics, but PHP's engine COWs and Phorj currently pays the copy every time.

Severity: **P2** for the missing user-facing guidance; **P3** for the refcount-1 fast path.

## F4 — Interpreter variable model: `Vec<HashMap<String, Value>>` + per-block HashMap alloc

**File**: `src/interpreter/mod.rs:109-133` (`CallScopes`), `src/interpreter/stmt.rs:349,364`
(a fresh `push_scope()` → `HashMap::new()` **per loop iteration**), `declare` allocates a
`String` key per binding (`mod.rs:129`).

Every variable read hashes the name and walks scopes rev-linearly; every loop iteration
allocates and drops a HashMap. This is the classic tree-walker cost profile. Given Invariant 2
(the interpreter is the reference oracle, prioritized for clarity over speed) this is
**by-design** — noted for completeness, not flagged for change. The VM exists precisely to
avoid it (slot-indexed locals), and does (F7).

Severity: **P3 / by-design** [Verified: read the code].

## F5 — The "cmd_run global lock": no lock exists; the serializer is per-call 256 MiB thread spawn

**Files**: `src/cli/mod.rs:300-308` (`on_deep_stack`), used by `cmd_run`/`cmd_runvm`/
`cmd_check`/`run_program`/bench/debug-REPL (cli/mod.rs:1114,1134,1152,1165,1180,1196;
cli/bench.rs:239; cli/debug_repl.rs:114).

Search result: **no `Mutex`/`RwLock`/global state guards `cmd_run`** [Verified: grepped all of
`src/` and `tests/` for Mutex/RwLock/OnceLock/LazyLock; the only globals are `native/time.rs:27`
FROZEN RwLock, `native/random.rs:35` RANDOM_STATE RwLock, `native/process.rs:26` PROCESS_ARGS
RwLock (all read-mostly), the native registry OnceLock (`native/mod.rs:371`, read-only after
init), and *test-local* mutexes (`CLOCK_LOCK`, `RNG_LOCK`, `ARGS_LOCK`) that only serialize
their own suites. `src/lock.rs` is the dependency lockfile, unrelated.]

What DOES serialize parallel `cmd_run` callers: every call runs
`std::thread::Builder::new().stack_size(256 * 1024 * 1024).spawn_scoped(...)` — a fresh 256 MiB
stack mmap/munmap per call, which contends on the kernel's per-process mmap lock.

**Measured** (standalone rustc -O microbenchmark, this box, 300 iterations of ~0.34 ms CPU work):

```
base (no spawn)            102.6 ms
serial, spawn per call     131.8 ms   → ~97 µs overhead per call
8-way parallel WITH spawn  speedup 4.81×
8-way parallel NO spawn    speedup 7.22×
```

[Verified: microbenchmark source + output in session scratchpad (spawnbench.rs).]

So the per-call spawn (a) adds ~0.1 ms latency to every `phg run` pipeline invocation
(irrelevant for CLI, real for in-process test harnesses that call `cmd_run` thousands of
times), and (b) demonstrably degrades multi-threaded scaling — 4.8× instead of 7.2× on 8
cores in the clean-room test. The memory note "~60% serialized / fmt corpus only 1.65×
from 8-way" is worse than my clean-room 4.8×; the remainder is plausibly allocator
contention from lex/parse/check allocation churn inside each call — [Inferred: consistent
with the measured spawn degradation plus the allocation-heavy pipeline; not directly profiled].

**Fix direction** (P2, test-latency not user-latency): reuse a single long-lived big-stack
worker (channel + one `std::thread` with 256 MiB stack, or a small pool), or only re-spawn
when recursion depth could actually matter. Purely internal; no parity surface.

## F6 — Test-gate latency: two monolithic corpus tests ARE the critical path

Full-oracle run this session (`PHORJ_PHP=…php-8.5.7 PHORJ_REQUIRE_PHP=1 cargo nextest run
--workspace`, 16 threads): **1672 tests, all green, total wall 216.9 s** — and the wall clock
is critical-pathed on two single `#[test]` functions [Verified: nextest output captured, F10]:

| Test | Time | What it does |
|---|---|---|
| `phorj::fmt every_repo_phg_formats_idempotently_and_safely` | **200.5 s** | formats every repo `.phg` + meaning-preservation: `cmd_run(src) == cmd_run(fmt(src))` per file (`src/fmt/tests.rs:26-27`) |
| `phorj::runtime shipped_manual_example_runs_on_both_backends` | **154.7 s** | runs the shipped manual example corpus on both backends |
| `phorj::registry download_client_verify_cache_and_reject` | 56.3 s | registry flow |
| `phorj native::crypto verify_a_committed_php_argon2id_hash` | 39.2 s | argon2id KDF — legitimately expensive by design |
| `phorj::conformance conformance_single_file_golden` | 28.0 s | golden corpus sweep |
| `phorj::differential all_examples_transpile_and_match_php` | 26.8 s | php-oracle leg, forks `php` per example |
| `phorj::build distributed_download_embed_run_matches_runvm` | 20.0 s | build/embed flow |
| `phorj::differential all_examples_match_between_backends` | 18.9 s | run ≡ runvm glob |

These are the "120 s and 60 s suites" from the audit lead — at full-oracle they are in fact
200 s and 155 s. The structural problem: each is ONE test function sweeping a whole corpus
in-process, so nextest's 16-way test-level parallelism cannot touch it — everything else
(1670 tests) finishes long before, and the run degenerates to waiting on these two. Their
internal loops call `cmd_run` per corpus item, paying F5's 256 MiB-spawn tax and its degraded
scaling. Why they're slow: **legitimate work volume** (hundreds of full pipeline executions)
× **zero test-level parallelism** × **F5 throttling any internal parallelism**.

**Fix directions** (P2 for gate latency): (a) reuse a long-lived deep-stack worker (F5) —
benefits every corpus loop at once; (b) shard the monolithic sweeps into a few
`#[test]`-per-directory functions so nextest schedules them across threads; (c) both.
Structural reads [Verified: read the test sources + nextest.toml]:

- `.config/nextest.toml` documents the box-measured profile: `cargo test` 228 s → nextest
  16 threads 147 s; much of the suite is I/O-bound — the `serve` tests **wait on 5 s socket
  keep-alive/idle timeouts** and each PHP-oracle differential test **spawns a `php`
  subprocess**.
- `tests/differential.rs` globs `examples/**/*.phg` (121 files) and runs run ≡ runvm ≡ php per
  file — the php leg forks a process per example; this is the long pole and is *legitimate
  work* (the byte-identity spine), not an algorithmic accident.
- The split gate already ships: pre-commit `PHORJ_SKIP_PHP=1` (~118 s), pre-push full oracle.

Severity: **P2** — the split gate mitigates the developer loop, but the full-oracle (pre-push)
gate's 217 s is ~92% one test's wall time; sharding + F5 worker reuse would roughly halve it.

## F7 — VM vs interpreter vs PHP: measured medians (phg benchmark, median of 101)

[Verified: full output captured this session → scratchpad bench-results.txt]

| Workload | parse+check | tree-walk | VM | verdict |
|---|---|---|---|---|
| `examples/fib.phg` (recursion, calls) | 137 µs | 4.661 ms | 204.5 µs | VM 22.79× faster ✓ |
| `examples/bench/workload.phg` (calls + object alloc) | 101 µs | 37.26 ms | 2.154 ms | VM 17.30× faster ✓ |
| string-concat loop (4000×, scratch program) | 25 µs | 3.157 ms | 4.822 ms | **VM 1.53× SLOWER** ✗ (F1) |
| append loop 3000× + filter/map/sum (scratch program) | 132 µs | 66.57 ms | 25.93 ms | VM only 2.57× faster (F3 dominates both) |

Memory: process peak ~8.2–8.4 MiB for the small programs; workload.phg peak 20.9 MiB with
+12.9 MiB cold-run RSS growth (matches its by-design live-chain allocation).

Conclusion: the VM's win is real where call dispatch dominates and disappears (or inverts)
where `Value::Str` copying or O(n²) list building dominates. Those two are the runtime's
actual hot-path weaknesses.

## F8 — Compile-time performance

- `cargo build --release` after this session's small doc-tree change: **50.29 s**, essentially
  all of it the single `phorj` crate compile [Verified: build log this session].
- The workspace is one 84,227-line main crate + `playground` (`Cargo.toml [workspace]`)
  [Verified: `wc -l` over `src/**/*.rs`, Cargo.toml read]. One big leaf crate = zero
  crate-level build parallelism for the tail; every `src/` edit recompiles all 84 k lines in
  release mode.
- No monomorphization bombs or giant generated matches observed: the three exhaustive Op
  matches are hand-written and modest (`vm/exec.rs` 811 lines total); no proc-macros; deps are
  4 small vetted crates. The 50 s is plain volume + LTO-ish release codegen [Inferred: from
  crate structure and file sizes; `--timings` not run].
- `mold` linker adoption is already planned (memory: pending `sudo apt-get install mold`).
- Split into `phorj-core`/`phorj-cli`(/`phorj-lsp`) crates would parallelize and cache better,
  but is a Large refactor with M-Decomp-style churn — flag only [Speculative].

Severity: **P3**.

## F9 — Benchmark harness: `--vs-php` numbers on this box are not meaningful comparisons

[Verified: benchmark output this session]

- The PHP used is `PHP 8.5.7 (cli) … (ZTS DEBUG)` **with Xdebug active** — workload.phg's
  vs-php leg *aborted* with "Xdebug has detected a possible infinite loop … stack depth of
  '512' frames" (Xdebug's `xdebug.max_nesting_level`), so the 3-way comparison silently loses
  its PHP leg on any recursion deeper than 512.
- Where it does run, the printed "Phorj 232× faster than PHP" (fib) folds in per-sample
  process spawn AND a debug-build+Xdebug interpreter — the banner's own caveat ("includes
  process spawn and depends on opcache/JIT") under-states this.
- Risk: these numbers look citable and are not. Suggested tooling hardening (P3): have
  `phg benchmark --vs-php` run php with `-n` (ignore ini → no Xdebug) or at least detect+warn
  when Xdebug/`DEBUG` build is present, mirroring the differential harness's tier-1 `php -n`
  oracle discipline.

Severity: **P2** for the silent Xdebug abort (a >512-deep recursion makes `--vs-php`
"skipping" routine), **P3** for the comparison-quality caveat.

## F10 — Full-oracle nextest run: captured timing tail (this session)

Command: `PHORJ_PHP=/stack/tools/phpbrew/php/php-8.5.7/bin/php PHORJ_REQUIRE_PHP=1
cargo nextest run --workspace --no-fail-fast` (16 threads per `.config/nextest.toml`).

```
PASS [  19.462s] phorj native::crypto::tests::hash_then_verify_roundtrip
PASS [   5.372s] phorj::lift_roundtrip lift_roundtrip_preserves_behavior
PASS [  19.961s] phorj::build distributed_download_embed_run_matches_runvm
PASS [  18.874s] phorj::differential all_examples_match_between_backends
PASS [  10.563s] phorj::runtime runtime_bench_shape_runs_and_backends_agree
PASS [  26.843s] phorj::differential all_examples_transpile_and_match_php
PASS [  27.985s] phorj::conformance conformance_single_file_golden
PASS [  39.161s] phorj native::crypto::tests::verify_a_committed_php_argon2id_hash
PASS [  56.257s] phorj::registry download_client_verify_cache_and_reject
PASS [ 154.698s] phorj::runtime shipped_manual_example_runs_on_both_backends
PASS [ 200.528s] phorj::fmt every_repo_phg_formats_idempotently_and_safely
────────────
Summary [ 216.881s] 1672 tests run: 1672 passed (6 slow), 1 skipped
```

Gate green — 1672/1672 passed. The 216.9 s total vs 200.5 s slowest test shows the run is
critical-pathed on `fmt::every_repo_phg_formats_idempotently_and_safely` (see F6).

---

## Explicit answers to the tasked questions

1. **cmd_run global lock**: no Mutex exists (F5) — the serializing mechanism is the per-call
   256 MiB `on_deep_stack` thread spawn, measured at ~97 µs/call and 4.8× (vs 7.2×) 8-way
   scaling. It protects nothing shared — it exists solely to give the recursive tree-walker a
   deep stack — so a reusable worker thread is a safe fix.
2. **run vs runvm vs php**: F7 table. VM wins 17–23× on call-heavy code as intended; VM
   LOSES 1.53× on string-concat; php comparisons on this box are polluted (F9).
3. **O(n²) traps**: `List.append`/`List.concat` per-call full copy (F3, measured 26–67 ms
   for 3 k appends); string `+`/interpolation accumulate O(n²) via non-Rc Str + GetLocal
   clone (F1, measured inversion). Mitigations exist (`List.fill` + index-set `SetIndexLocal`
   O(1) path) but are undocumented for users.
4. **Compile time**: 50.3 s release rebuild of the single 84 k-line crate; no structural
   bloat found beyond the monolithic-crate shape (F8).
5. **Unnecessary allocations**: F1 (`as_display` clone per interpolation part,
   `GetLocal` deep Str clone), F2 (per-call/construct String clones in `CallMethod`/
   `MakeEnum`/`MakeInstance`/`MatchTag`, String-pair HashMap dispatch keys), F4 (interpreter
   per-iteration HashMap scopes + String keys — by-design oracle cost). Lexer/parser: token
   model carries `String` per ident/literal (`src/token.rs:21,43,51,67,68`) — standard, and
   parse+check is 25–137 µs on real programs (F7), so **not** a bottleneck worth borrowing
   complexity for [Verified: measured parse+check times].

## Priority recap

| # | Finding | Severity | Evidence |
|---|---|---|---|
| F1 | Non-Rc `Value::Str` + GetLocal clone → VM slower than interpreter on strings | P2 | Measured |
| F3 | O(n²) `List.append` loops; fast path exists but user-invisible | P2 (docs) / P3 (fast path) | Measured |
| F9 | `--vs-php` silently loses its PHP leg under Xdebug; debug-PHP comparisons | P2/P3 | Measured |
| F5 | Per-call 256 MiB thread spawn serializes parallel harnesses | P2 (test latency) | Measured (microbench) |
| F6 | Full-oracle gate critical-pathed on 2 monolithic corpus tests (200 s + 155 s) | P2 | Measured (nextest) |
| F2 | Per-dispatch String clones on call/construct ops | P3 | Code read |
| F8 | Monolithic 84 k-line crate → 50 s release rebuilds | P3 | Measured build |
| F4 | Interpreter scope model — by-design oracle cost | P3/by-design | Code read |

Per Invariant 11, none of the fix directions above should land without a `phg benchmark`
before/after; F1 and F3 fixes are parity-affecting and need the full differential gate.

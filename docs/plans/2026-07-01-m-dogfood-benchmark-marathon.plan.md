# M-DOGFOOD — Dogfood Benchmark Marathon Plan

> Autonomous full-day marathon. Validate the language against a real program (port the PHP
> `benchforge` suite to Phorj), close every gap that blocks it, and ship both automated
> (`phg bench --vs-php`) and **manual** in-language benchmarking. Plan-location: `repo`.
> Started 2026-07-01. Autonomous mode: `_AUTONOMOUS_3C=1` (session sentinel set).

## Decisions Log

- [2026-07-01] AGREED: **Direction = dogfood.** Port PHP `benchforge` (`/stack/projects/phorj-app/files/benchforge`) to Phorj as the forcing function; each wall becomes a prioritized fix. Pause A2 (generators) until the base is validated.
- [2026-07-01] AGREED: **Type-system proposals REJECTED** — (1) no force-import of primitives (`int`/`float`); they are keyword-class builtins, importing them breaks familiarity-first ([[philosophy-of-phorj]]). (2) No `Integer`/`Float`/`Decimal` object wrappers — Java-autoboxing anti-pattern; every need is already met by UFCS (`n.abs()`), `int?` (nullability), Value-native generics (`List<int>`), and the `decimal` primitive. Instead: document the **keyword-vs-import 3-way rule**.
- [2026-07-01] AGREED: **Keyword-vs-import 3-way rule** — built-in types (`int float string bool bytes decimal void never`, `List Map Set`, `T?`, function types, ranges) are keywords, NEVER imported; user/library types use `import type Pkg.Path.Name`; stdlib functions use `import Core.X`. Write into `docs/INVARIANTS.md`.
- [2026-07-01] AGREED: **Manual benchmarking reconciled with the determinism spine** — non-deterministic measurement is legal but **quarantined from the byte-identity example set** (like `Core.Time`/`Core.Process`/serve socket, all `pure:false`). Add `Core.Runtime.memoryBytes()`/`peakMemoryBytes()` (reuse `src/mem.rs`) + a `Stopwatch` over `Core.Time`. Programs using them live outside `tests/differential.rs` with dedicated tests + a README walkthrough (not glob-gated). `phg bench --vs-php` stays as the automated path.
- [2026-07-01] NOTE: Pre-existing uncommitted `0.5.0→0.5.1-alpha.1` version bump (Cargo.toml/lock, `src/bundle/manifest.rs`, CHANGELOG heading) is left untouched — stage only marathon files per commit (explicit `git add`, never `-a`).
- [2026-07-01] OUT OF SCOPE: the locked Phorj naming-overhaul breaking codemod (task #25); A2 generators (resume after).

## Design forks to batch (surface via AskUserQuestion — genuine taste decisions)

- **DF-1 return-type syntax — RESOLVED (2026-07-01):** parser accepts BOTH `function f(): void` and
  `function f() -> void`. **Decision: `:` is canonical** (PHP/TS familiarity — the developer flagged
  `-> type` as looking old, confirming the lean); `->` stays accepted (non-breaking). A `phg fmt`
  normalization to `:` + an optional `W-` deprecation on `->` are deferred (breaking-ish — do when the
  developer wants the codemod). No code change this marathon.

## Verified gap inventory (Phase 0)

| Gap | Status | Evidence |
|---|---|---|
| `List<T> xs = []` empty init | **Real bug** | live-ran → `cannot infer element type of empty list literal` (`src/checker/expr.rs:898`, expected-type threading YAGNI'd) |
| `throws A, B, C` comma form | **Absent** (only `A \| B` union works) | `src/parser/items.rs:244` |
| Line-wrapping `extends`/`implements`/`throws`/params | Unverified | verify-then-fix |
| `phg bench --vs-php` | **Exists** | `src/cli/bench.rs:26` |
| `phg bench` peak-RSS memory | **Exists** | `src/mem.rs`, CLAUDE.md |
| `Core.Time.nowMilliseconds()` stopwatch primitive | **Exists** (`pure:false`) | `src/native/time.rs` |
| In-language memory API | **Missing** | no `Core.Runtime` |
| Old lambda syntax in examples | **Not real** — `function(x) -> T {}` is current | grep `examples/` clean, only Rust `fn(` in `differential.rs` |
| `Core.File.read` ("can't read a file") | Unverified | `src/native/file.rs` exists — verify/fix |
| Conformance old syntax (219 `.phg`) | Unverified | audit |

## Waves (each: TDD → `run≡runvm≡real PHP 8.5` → clippy/fmt → commit green)

- **W0 Unblockers** — empty-collection init (`List<T> xs = []`, expected-type at decl/return/arg site); `throws A, B, C` comma sugar; line-wrapping audit+fix; keyword-vs-import rule → `docs/INVARIANTS.md`.
- **W1 Manual benchmarking tier** — `Core.Runtime.memoryBytes()`/`peakMemoryBytes()` (`pure:false`, reuse `src/mem.rs`); `Stopwatch`; quarantine plumbing + `examples/bench/manual/` README.
- **W2 File IO + JSON hardening** — verify/fix `Core.File.read`; `Core.Json` round-trip on real nested data.
- **W3 OOP + error-model dogfood** — port benchforge's interfaces, `abstract AbstractBenchmark`, `BenchmarkStatus` enum, custom exceptions + `throws`/`try`/`catch`.
- **W4 Benchmark port (simplest-first)** — Fibonacci → PrimeSieve → Sorting → Aggregation → StringProcessing → Search → Matrix → ObjectGraph; each → `phg bench --vs-php` number → fix surfaced gap.
- **W5 Stdlib breadth gap-fill** — close whatever Text/List/Map ops the port reveals missing.
- **W6 Large-data memory stress** — validate large `List`/`Map`; measure memory scaling.
- **W7 Capstone demo** — full runnable phorj `benchforge` in `/stack/projects/phorj-app`; side-by-side PHP-vs-phorj perf/memory report.
- **W8 Perf investigation** — profile + fix (or evidence-document) any benchmark where phorj is unexpectedly slow.
- **W9 CLI report formatting** — `phg bench --vs-php --json` / clean suite output.
- **W10 IDE extensions gap-fill** — VSCode + PhpStorm: new keywords/natives/diagnostics/hover/completion.
- **W11 Playground/WASM refresh** — surface new features in the browser playground.
- **W12 Conformance + docs refresh** — audit 219 `conformance/*.phg`; refresh guides, `examples/README.md` matrix, `KNOWN_ISSUES.md`.
- **Wrap** — full gate (`PHORJ_PHP=/stack/tools/phpbrew/php/php-8.5.7/bin/php PHORJ_REQUIRE_PHP=1 cargo test --workspace`, clippy, fmt) → release build → CHANGELOG/memory/plan → handoff (comparison table). No auto-push.

## Progress

- [x] **W0** (empty-list init `2350428`; comma-throws + line-wrap `debe230`; keyword-vs-import rule → INVARIANTS §12)
- [x] **W1** (Core.Runtime memory+monotonic natives, Stopwatch, quarantine)  - [x] **W2** (File.read verified working; nested-quote interpolation `adbc343`; Core.Json verified)  - [ ] W3  - [ ] W4  - [ ] W5  - [ ] W6
- [x] **W3** (OOP + **error-model** spine COMPLETE — interface/abstract Template-Method/enum/checked
  exception `throws`/`try`/`catch` all validated in the demo, run≡runvm; a failing benchmark exercises
  the caught→Failed path. The other 6 benchmarks are **intentionally not ported** — they need in-place
  cross-call mutation Phorj's value semantics forbid; documented as the dogfood finding in
  `KNOWN_ISSUES.md` (`c8e5337`) + `/stack/projects/phorj-app/FINDINGS.md` with the decision surface.)
- [x] **W4** (list upcast `671612a`; 2-benchmark demo)
- [x] **W5** (`Core.List.append` `5b23515`)
- [~] W6 (sieve(100000) already exercises a 100k-element list + memory report — demonstrated in the demo)
- [x] **W7** (working demo `benchforge.phg` + `BENCHFORGE.md`)
- [x] **W8** (O(n²)→O(1) index-assign `b8a2877`, the headline)
- [x] **W9** (`bench --json` / `--vs-php --json` `43e8e3b`)
- [x] **W10** (grammar + LSP keyword sync `2d9d78a`)
- [x] **W11** (playground compiles on wasm32 against all changes; default sample current-syntax; CI rebuilds+deploys on push — no source change needed)
- [x] **W12** (old-syntax audit CLEAN across all `.phg` incl conformance; `W-UNKNOWN-IMPORT` DEFERRED — needs a single-sourced known-module set incl injected-prelude-only modules `Core.Http`/`Core.Secret`, else false-positives on valid code; DF-1 return-type syntax = open user taste decision)

### W3/W4/W7/W8 status (2026-07-01)
- **W8 perf — DONE, the headline win** (`b8a2877`): `xs[i]=v` was O(n)-per-write (COW deep-copied the
  whole container every write; both backends held a spurious 2nd `Rc`). Fixed via `Frame::lookup_mut`
  (interp) + new `Op::SetIndexLocal` (VM). sieve(20000): interp 2.73s→16ms (170×), VM 2.55s→8ms (305×).
  COW preserved; 1300 lib + 126 differential (incl PHP oracle) green.
- **W4/W7 — working demo delivered** (in `/stack/projects/phorj-app/`, NOT committed — that's the `/stack`
  repo, separate autonomy scope): `benchforge.phg` (2 of 8 benchmarks — Fibonacci + PrimeSieve) with the
  full OOP spine, self-timing via `Core.Runtime`; `BENCHFORGE.md` has the Phorj-vs-PHP-8.5 table. Sieve:
  Phorj VM ~3.3× slower than optimized PHP 8.5 (was un-runnable before the W8 fix).
- **W3 — partial**: interface + abstract Template-Method + enum + class + `Map<string,string>` metrics
  all validated on a real program. Remaining: the error-model path (`try`/`catch` in `run()`), and
  porting the other 6 benchmarks (Sorting/Aggregation/StringProcessing/Search/Matrix/ObjectGraph).
- **W4 also fixed a checker gap** (`in the W8 commit's precursor`): heterogeneous list of interface
  implementers now upcasts to the annotation's element type (expected-type-directed list checking,
  generalizing W0's empty-list fix). Committed with W0-adjacent checker work.

### New gaps surfaced during the marathon (feed later waves)
- **`Core.List` has no append/push/add** — only query ops (map/filter/reduce/concat…). Imperative `$arr[] = x` has no direct equivalent; idiom is `List.map(range, fn)` / `List.concat`. → W5 (decide: add mutable append or keep functional).
- **`import <unknown module>` type-checks clean** (e.g. `import Core.Types` on a non-existent module is a silent no-op) — candidate `W-UNKNOWN-IMPORT` lint. → W12.
- **DF-1 return-type syntax** (`:` vs `->`, both accepted) — batch as a design question.

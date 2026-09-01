# C — Three stdlib gaps the developer raised: iterable Input / filesystem locking / no-op clone

> **Scope.** Evidence-based investigation only. **No design ruling is made here** (Invariant 15 —
> ADJUDICATION RULE). Each topic ends with findings + options + ONE recommendation + why; the
> developer rules.
> **Method.** Every claim below is graded and cites `path:LINE`. Probes were run against the
> committed release binary `/home/user/phorj/target/release/phg` (`phg 1.0.0-nightly.0`,
> built 2026-07-25 21:03) and the transpile-floor oracle
> `/stack/tools/phpbrew/php/php-8.5.8/bin/php` (`PHP 8.5.8`, per `scripts/toolchain.env:38`).
> No cargo builds were run (disk constraint), so no *new* Rust code was compiled inside the
> phorj workspace; standalone `rustc` probes were used for the std-API checks.

---

## HEADLINE ANSWERS (one line each)

| Topic | Answer |
|---|---|
| (1) Iterable Input | **The lazy-iteration protocol ALREADY EXISTS and already streams — for stdin only.** `Core.IteratorModule` (`Iterator<T>`, DEC-257) + `Input.lines()` stream an 88 MB input in **23.7 MB peak RSS** and are byte-identical on all three legs today. The gap is purely that **no FILE can be streamed** — `Core.FileSystemModule` and `Core.File` are whole-slurp only. |
| (2) FS locking | **No locking exists anywhere** (zero `flock`/lock code in `src/`). But the presumed blocker is FALSE: **`std::fs::File::{lock, lock_shared, try_lock, try_lock_shared, unlock}` are STABLE on the pinned toolchain 1.97.1** — verified by compiling and running them. **No new dependency is needed**, and Rust-std locks and PHP `flock()` were verified to interoperate bidirectionally. |
| (3) No-op clone | **It already works.** `p with { }` parses, checks, runs identically on VM + tree-walker, and transpiles to PHP `clone($p)`. It is **undocumented and unexampled**, has **no recorded design ruling**, and the **lifter refuses it**. |

---

# TOPIC 1 — Iterable / streaming input ("handle any size of files")

## 1.1 Current state (evidence)

### The iteration protocol EXISTS — `Iterator<T>`, DEC-257

`src/cli/preludes.rs:359-364` — the whole protocol, injected as `Core.IteratorModule`:

```
interface Iterator<T> {
    function hasNext(): bool;
    function next(): T;
}
```

[Verified: read `src/cli/preludes.rs:356-364`.]

`for … in it` over any DIRECT `Iterator<T>` implementor is lowered **pre-backend** to a
hasNext/next while-pull, so all three legs run the identical loop
(`src/checker/rewrite_foreach.rs:1-14` doc comment; the rewrite itself at
`src/checker/rewrite_foreach.rs:250-294`). This is Invariant 5 discipline — backends never see a
foreach-over-Iterator. [Verified: read the file.]

**Exactly what `for … in` accepts today** (`src/checker/stmt/flow.rs:436-499`):

| Iterable | Element | Cite |
|---|---|---|
| `List<T>`, `Set<T>` | `T` | `flow.rs:438` |
| `string` | 1-char `string` | `flow.rs:440` |
| `Map<K,V>` | requires the two-binding form `for (K k, V v in m)` | `flow.rs:411-423`, `442-449` |
| any class DIRECTLY `implements Iterator<E>`, or an `Iterator<E>`-typed value | `E` | `flow.rs:457-479`, resolver `flow.rs:524-541` |
| anything else | `E`-coded error: "`for`-`in` requires a List, Set, string, Map, or an `Iterator<T>` implementor" | `flow.rs:492-497` |

[Verified: read `src/checker/stmt/flow.rs:390-541`.]

**No generators / no `yield`.** There is no `Yield` token and no `yield` keyword in the phorj
grammar: `src/token.rs` has no `Yield` variant (grep over `src/token.rs`, `src/tokenizer/*.rs`
returned only doc-comment prose hits). `yield` appears in the tree only as (a) a PHP reserved word
phorj must refuse as a symbol name (`src/checker/common.rs:387`) and (b) a PHP construct the
LIFTER refuses as Tier-2/Tier-3 (`src/lift/parser/exprs.rs:303-304`). [Verified: greps above.]
Register status: generators/`yield` is a **deferred, spine-sensitive, fresh-context-only** item, not
a rejected one — `docs/plans/MASTER-PLAN.md:1649-1650` (*"W4-2 · Generators/`yield` + iterator
protocol (XL, DESIGN)"*), `docs/plans/SLICE-STATE.md:1396-1398` (*"generators/`yield` = ABSENT as a
language surface … standing rule = FRESH context only"*). [Verified: doc sweep, quoted lines.]

### `Core.Input` — the FULL surface (5 functions), and it ALREADY streams

Native layer `src/native/input.rs:155-215` — four natives, all `pure: false`:

| Native | Signature | Semantics | Cite |
|---|---|---|---|
| `readAll` | `(): string` | drains stdin, **lossy UTF-8** (invalid → U+FFFD) | `input.rs:157-170`, `112-118` |
| `readAllBytes` | `(): bytes` | drains stdin, exact bytes | `input.rs:171-184`, `120-122` |
| `readLine` | `(): string?` | ONE line via `std::io::stdin().read_line`, strips exactly one `\n`/`\r\n`, `null` at EOF | `input.rs:185-200`, `124-145`, `90-98` |
| `isInteractive` | `(): bool` | `std::io::stdin().is_terminal()` | `input.rs:201-213`, `147-153` |

Prelude layer `src/cli/preludes.rs:62-100` — the `Input` class plus the streaming iterator:

```
class Input {
  static function readAll(): string { … }
  static function readAllBytes(): bytes { … }
  static function readLine(): string? { … }
  static function isInteractive(): bool { … }
  static function lines(): InputLines { return new InputLines(); }
}
class InputLines implements Iterator<string> { … }   // one-line lookahead in hasNext, hand-over in next
```

[Verified: read `src/cli/preludes.rs:62-100`.]

`readLine` is **truly incremental** — it calls `std::io::stdin().read_line` per pull
(`src/native/input.rs:132`), never a `read_to_end`. So `Input.lines()` is a real stream, not a
slurp-then-split. [Verified: read the body.]

**Two process-global controls** (relevant to any file-stream design that reuses this shape):
a test override seam with a read cursor (`input.rs:30-46`, `60-86`) and a
`phg serve` disable flag that makes reads behave as an exhausted pipe, never an error
(`input.rs:38-57`, `101-103`, `125-127`). [Verified: read.]

### The FILESYSTEM side is whole-slurp ONLY

`Core.FileSystemModule` (`src/native/fs.rs:113-268`) registers **18 natives** —
`readText, readBytes, writeText, writeBytes, appendText, copy, move, delete, size, exists, isFile,
isDir, createDir, removeDir, removeDirAll, listDir, walk, tempDir`. **Not one takes an offset, a
length, a line count, or returns a handle.** [Verified: read the full `fs_natives()` vec.]
The prelude surface mirrors it 1:1 (`src/cli/preludes.rs:165-224`).

The read bodies are unconditional whole-file slurps:
`std::fs::read_to_string` / `std::fs::read` behind `read_text_inner`/`read_bytes_inner`
(`src/native/fs_bodies.rs`, invoked via the `fs_native!` macro at `src/native/fs.rs:26-27`).
[Verified: read `src/native/fs_bodies.rs:86-119` for the write side and the macro at `fs.rs:19-25`.]

The older `Core.File` (`src/native/file.rs:100-218`) is 8 functions —
`read, exists, write, append, delete, rename, copy, size` — also whole-slurp
(`std::fs::read_to_string` at `file.rs:9`). [Verified: read.]

### `limits.rs` has NO I/O or memory limit

`src/limits.rs` declares exactly five constants: `MAX_CALL_DEPTH` (4096, `limits.rs:21`),
`MAX_NEST_DEPTH` (512, `limits.rs:30`), `MAX_EXPR_DEPTH` (10_000, `limits.rs:39`),
`INT_BITS` (`limits.rs:45`), `FLOAT_BITS` (`limits.rs:50`), all locked by
`limit_values_are_stable` (`limits.rs:56-64`). **There is no file-size cap, no read cap, no heap
cap.** A `FileSystem.readText` of an arbitrarily large file is bounded only by the OS.
[Verified: read the entire 65-line file.]

Corroborating negative from the doc sweep: **nothing in the repo records whole-file reads as a
memory risk** — zero hits for `large file` / `whole file` / `memory risk` / `OOM` / `slurp` across
MASTER-PLAN, SLICE-STATE, C-decisions, M-gap-matrix, E-phorj-surface, F-cross-language,
KNOWN_ISSUES, FEATURES, UNIFIED-SPEC. [Verified: delegated doc sweep, negative result explicitly
confirmed.]

### Register status of file streaming: DEFERRED (and once outright REJECTED)

- `docs/plans/SLICE-STATE.md:1309` — *"**non-stream FS breadth** into Core.Fs
  (glob/stat/perms/mtime/tempFile/scandir — **DEFER file-handle streams**)"*.
- `docs/research/roadmap-completeness/raw/A.md:56` — historic charter row:
  `| A-streams | Stream wrappers / resources (fopen, filters) | omit | weak | reject | — (M6 IO instead) | L |`
- `docs/archive/specs/2026-06-21-php-parity-and-beyond.md:253-254` — same reject, plus
  `| L-lazy-seq | … Lazy iterators / Seq<T> generator protocol | new | weak | **reject** |`.
  **NOTE: the `L-lazy-seq` reject is now stale** — DEC-257 shipped exactly that protocol
  (`docs/plans/MASTER-PLAN.md:483-487` "DEC-257 COMPLETE").
- `docs/research/full-audit/raw/M-gap-matrix.md:218` — *"GU: **the entire fopen stream-handle
  family (16 rows)**"*; `:229` FN-STREAM 15 rows, 0 covered; `:921-923` names FN-FS stream handles
  as *"the largest pool"* of genuine remaining gaps.
- `docs/plans/MASTER-PLAN.md:1630-1633` (W3-6) — *"fs-OOP question resolved (**statics until
  streams demand handles**)"*.

[Verified: delegated doc sweep with verbatim quotes; every line cited above was quoted back.]

**Also on record: the DB "stream" is not a true cursor.** `docs/research/full-audit/raw/C-decisions.md:979-986`
(DEC-228): *"both drivers **materialize the result set at `stream()`** (rusqlite/postgres iterators
borrow their statement — self-referential lifetime, unavailable under `#![deny(unsafe_code)]`)"* —
mirrored in `docs/specs/UNIFIED-SPEC.md:1261-1264`. This is a **directly relevant precedent**: the
same self-referential-lifetime problem does NOT apply to a file handle (a `File` + `BufReader` owns
its buffer; nothing borrows a statement), so a file stream can be a *real* incremental cursor where
the DB one could not. [Verified: quotes; the lifetime reasoning is [Inferred: `std::io::BufReader<File>`
is `'static`-ownable, unlike `rusqlite::Rows<'stmt>`.]]

## 1.2 Probe transcripts

### P1-A — the streaming protocol works and is byte-identical on all three legs *today*

```
$ ./target/release/phg run examples/cli/stdin-filter.phg < in.txt   # 4 lines
exit=0 → "scanned 4 line(s)" (+ filtered output)
$ ./target/release/phg run --tree-walker examples/cli/stdin-filter.phg < in.txt   → exit=0
$ ./target/release/phg transpile examples/cli/stdin-filter.phg > c.php             → exit=0
$ /stack/tools/phpbrew/php/php-8.5.8/bin/php c.php < in.txt                        → exit=0
$ diff a.out b.out → VM==TW
$ diff a.out c.out → VM==PHP (byte-identical)
```

[Verified: ran the three legs + both diffs; both diffs empty.]
**This is the load-bearing fact for the whole topic**: `Iterator<T>` + a per-pull PHP `fgets`
mapping is *already proven* byte-identical in production. The PHP leg is
`(defined('STDIN') ? (($__phorj_l = fgets(STDIN)) === false ? null : preg_replace("/\r?\n$/", '', $__phorj_l)) : null)`
(`src/native/input.rs:196-199`).

### P1-B — memory: streaming vs slurp, 88.9 MB / 2,000,000-line input

Peak RSS measured by polling `/proc/<pid>/status` `VmHWM` (script:
`scratchpad/probe-std/peak.sh`; `/usr/bin/time` is absent in this container).

| Program | Path | Result | Peak RSS |
|---|---|---|---|
| `for (string l in Input.lines())` (VM) | streaming | `lines=2000000` | **23,712 KB** |
| `Input.readAll()` (VM) | slurp via stdin | `bytes=88888890` | **200,080 KB** |
| `FileSystem.readText("big.txt")` (VM) | slurp from file | `bytes=88888890` | **97,344 KB** |

[Verified: ran all three; outputs and peaks as shown.]
Reading: streaming is **~4× smaller than the file itself** and **~8.4× smaller than the cheapest
slurp**; `readAll` costs **2.25× the input** (the lossy-UTF-8 `String::from_utf8_lossy(...).into_owned()`
at `src/native/input.rs:113-117` allocates a second copy). `FileSystem.readText` costs ~1.10×.

### P1-C — the workaround that exists, and its exact limits

`phg run prog.phg < big.file` streams a named file today (P1-B row 1 used exactly this).
**Limits [Verified: from the code, not guessed]:**
1. **Exactly ONE file per process** — stdin is a single cursor (`src/native/input.rs:31-34`,
   one `pos`); there is no way to name a second file.
2. **You cannot both stream a file and read stdin** — same reason.
3. **Unusable under `phg serve`** — stdin is disabled before workers run
   (`src/native/input.rs:48-53`, `101-103`), reads return empty/`null`.
4. **The whole program becomes spine-quarantined** — `Core.Input` natives are `pure: false`
   (`src/native/input.rs:162,177,190,206`), which excludes the example from the byte-identity
   differential (`src/native/input.rs:5-9` doc comment; validated instead by `tests/stdin.rs`).
   Note this quarantine already applies to `Core.FileSystemModule` too (`src/native/fs.rs:62`
   `pure: false`), so a `FileSystem.lines()` adds **no new** quarantine.

### P1-D — nothing in the current API can read a file incrementally

Enumerated the complete native surface of both filesystem modules (`src/native/fs.rs:113-268` = 18
fns; `src/native/file.rs:100-218` = 8 fns). No offset, no length, no chunk, no handle, no
`readLines`. [Verified: full read of both registries.] A user therefore **cannot** write a
`FileLines implements Iterator<string>` class today, even though the protocol is right there —
there is no primitive to pull from.

## 1.3 Cross-language scan (Invariant 16)

| Language | Streaming-lines idiom | Shape | Byte-identical PHP mapping? |
|---|---|---|---|
| **Rust** | `BufReader::new(File).lines()` → `Iterator<io::Result<String>>` | external iterator over an owned buffered handle | n/a (it IS the implementation side) |
| **PHP** | `$h=fopen($p,'r'); while(($l=fgets($h))!==false){…}` ; OO `SplFileObject` (itself `Iterator`); `function lines($p){ … yield $l; }` generator | handle + per-call pull; or a real generator | **YES — this is the target.** `fgets` verified present (`function_exists('fgets')` → `true`), `SplFileObject` present, `Generator` present [Verified: ran on php-8.5.8] |
| **Python** | `for line in open(p):` — the file object *is* the iterator | implicit external iterator | maps to `fgets` loop |
| **Go** | `bufio.NewScanner(f)`; `for sc.Scan() { sc.Text() }` | **`Scan() bool` + `Text() T` — this is EXACTLY phorj's `hasNext()`/`next()`** | maps to `fgets` loop |
| **Java** | `Files.lines(path)` → `Stream<String>` (lazy, `AutoCloseable`) | lazy stream + explicit close | maps, but the `Stream` combinator surface does not |
| **C#** | `File.ReadLines(p)` → `IEnumerable<string>` (lazy); vs `ReadAllLines` (eager) | `MoveNext()`/`Current` — same 2-method pull as phorj | maps to `fgets` loop |
| **Kotlin/Swift** | `File.useLines { }` / `AsyncLineSequence` | scoped-closure lazy sequence (auto-close!) | scoped form maps cleanly |
| **Node** | `readline.createInterface` / `fs.createReadStream` async iterator | callback/async | does NOT map (async colours the API) |

**Convergent conclusion:** every eager/lazy pair in the industry names them distinctly
(`ReadAllLines` vs `ReadLines`, `read_to_string` vs `lines`, `file()` vs `fgets`), and the two
languages whose protocol phorj *already matches* (Go's `Scan()/Text()`, C#'s `MoveNext()/Current`)
both expose file streaming through exactly that protocol. [Verified: the shapes; [Speculative] on
"industry consensus" as a normative argument.]

**LADDER RULE (Invariant 14) verdict for topic 1: CASE 1 — faithful idiomatic PHP exists.**
A `FileSystem.lines(path): Iterator<string>` maps to the single most idiomatic PHP file-reading
loop there is (`fopen`+`fgets`+`fclose`), and the **identical shape is already proven
byte-identical** by P1-A. **No quarantine beyond the FS module's existing `pure:false` exclusion,
no `E-TRANSPILE-*` needed.** Also lift-relevant (Invariant 17): a PHP `while(($l=fgets($h))!==false)`
loop becomes liftable — today `fgets`-style loops have no phorj target at all.
[Inferred: from P1-A byte-identity + the `fgets` availability check; the *new* natives' PHP legs are
not yet written, so this is Inferred not Verified.]

## 1.4 Gaps (severity + grade)

- **C1 · P1 · No file can be streamed at all.** `Core.FileSystemModule` (18 natives) and
  `Core.File` (8 natives) are whole-slurp only; no offset/length/handle/line primitive exists, so a
  user cannot even build the iterator themselves. Peak RSS scales ~1.1× the file for
  `FileSystem.readText` and 2.25× for `Input.readAll`, with **no limit in `limits.rs`** to stop it.
  [Verified: `src/native/fs.rs:113-268`, `src/native/file.rs:100-218`, `src/limits.rs` full read,
  P1-B measurements.]

- **C2 · P1 · The asymmetry is the actual bug-shaped part.** `Core.Input` HAS `lines(): Iterator<string>`
  (`src/cli/preludes.rs:71,77`); `Core.FileSystemModule` has no counterpart. The protocol, the
  lowering, the transpile shape, and the one-line-lookahead idiom all already exist and are proven
  — only a `path`-taking source is missing. [Verified: the two preludes side by side,
  `preludes.rs:62-100` vs `:165-224`.]

- **C3 · P2 · The stdin workaround is single-slot and serve-hostile.** `< file` streams exactly one
  file, cannot coexist with real stdin, and is disabled under `phg serve`.
  [Verified: `src/native/input.rs:31-34, 48-57, 101-103`.]

- **C4 · P2 · No transpiling precedent for an opaque native handle.** The two existing opaque handle
  types are `DatabaseHandle` (`src/checker/resolve.rs:296-297`, `src/cli/preludes.rs:785`) and
  `MailHandle` (`src/checker/resolve.rs:299`), and **both belong to `E-TRANSPILE-*`-quarantined
  modules** (`src/cli/pipeline.rs:600,615`). The transpiler's `emit_type` has **no special case**
  for either: an unknown `Type::Named` falls to `type_pos_ref` → `php_type_ref` and emits the name
  as a PHP *class* type-hint (`src/transpile/types.rs:47,71`, `src/transpile/names.rs:64-72`).
  A PHP `fopen()` returns a **`resource`**, not a class instance — so a naive `FileHandle` field
  would emit an unsatisfiable hint. A handle-based design **must** add a `emit_type` mapping
  (`"FileHandle" => "mixed"`). [Verified: greps for `DatabaseHandle|MailHandle|FileHandle` in
  `src/transpile/` returned zero relevant hits; read `types.rs:45-72` and `names.rs:64-72`.]

- **C5 · P3 · Iterator protocol holes (pre-existing, would be inherited by any streaming design).**
  (a) A class implementing `Iterator` **only through a parent** is NOT foreach-able — documented
  deferral with a targeted hint (`src/checker/stmt/flow.rs:480-491`, `:520-523`).
  (b) An **interface-typed** `Iterator<E>` value reports **empty throws** (`flow.rs:526`, comment at
  `:519-520`: *"interface-method throws are an existing documented deferral"*) — so passing a
  throwing stream as `Iterator<string> it` would not discharge its faults at the loop site the way a
  concrete type does. A file stream **will** throw (`FileSystemError`), which makes (b) go from
  theoretical to load-bearing. [Verified: read `flow.rs:457-541`.]

- **C6 · P3 · `Input.readAll` costs 2.25× the input** because of the lossy-UTF-8 double allocation
  (`String::from_utf8_lossy(&bytes).into_owned()`, `src/native/input.rs:113-117` and `140-143`) —
  a `Cow::Borrowed` fast path would avoid the second copy for valid UTF-8.
  [Verified: P1-B 200,080 KB for an 88,888,890-byte input + read of the code.]

## 1.5 Options & recommendation — topic 1

All four options assume the **existing** `Iterator<T>` protocol (no language change, no generators).

| | Option | Shape | New machinery | Transpile | Verdict |
|---|---|---|---|---|---|
| **O1** | **`FileSystem.lines(path): FileLines`**, backed by a **new opaque `FileHandle`** (the `DatabaseHandle` recipe) + natives `openRead/readLine/close` | `for (string l in FileSystem.lines(p)) { … }` | new `Value` variant (or reuse), `resolve.rs` reserved-opaque row, prelude class, **`emit_type` → `mixed`** (C4), close/`Drop` discipline | `fopen`/`fgets`/`fclose` — the P1-A shape | **truest streaming**; largest blast radius |
| **O2** | **`FileSystem.lines(path)` backed by an OFFSET-CHUNK native** — `readChunkFrom(path, byteOffset, maxBytes): (string, int)`; the prelude iterator keeps the offset and re-opens+seeks per chunk. **No handle at all.** | identical user-facing syntax to O1 | **zero** new `Value`/type/transpile machinery — one ordinary fallible native + a pure-Phorj prelude class | `fopen`+`fseek`+`fread`+`fclose` per chunk, or `file_get_contents($p, false, null, $off, $len)` | **cheapest**; O(chunks) re-opens; no leak risk (nothing stays open) |
| **O3** | Scoped closure — `FileSystem.withLines(path, (string) => void)` (Kotlin `useLines` / Java try-with-resources) | callback, not `for` | needs higher-order prelude fn (fine — Phorj-level) | maps | auto-closes, but **breaks `break`/`return`** out of the loop and abandons the protocol phorj already has |
| **O4** | Generators / `yield` | `function lines(p) { … yield l; }` | **XL** — deepest VM control-flow change | must prove byte-identity vs PHP generators | already ruled **fresh-context-only, spine-sensitive** (`SLICE-STATE.md:1396-1398`); wrong tool for this ask |

### RECOMMENDATION: **O2 first, with O1 as the declared upgrade path** — surfaced for the developer's ruling, not decided.

**Why O2 first:**
1. **It ships the developer's actual ask with ZERO new spine machinery.** The user-visible syntax is
   identical to O1 (`for (string l in FileSystem.lines(p))`), the protocol already exists
   (`preludes.rs:359-364`), the lowering already exists (`rewrite_foreach.rs`), and the byte-identity
   of exactly this shape is already **Verified** (P1-A). O2 adds *one* ordinary fallible native to
   `src/native/fs.rs` plus a prelude class copied almost verbatim from `InputLines`
   (`preludes.rs:77-99`) — no `Value` variant, no reserved opaque type, no `emit_type` special case,
   no close/leak discipline, no `Drop` semantics to reason about. That sidesteps C4 entirely.
2. **The memory guarantee is the same** — a chunked reader holds one chunk, not the file. O(1) memory
   in the file size, exactly the 23.7 MB-for-88 MB profile of P1-B.
3. **It is honest about cost and surfaces the trade.** Re-open+`fseek` per chunk costs one syscall
   triple per chunk (not per line); with a 64 KB chunk an 88 MB file is ~1,360 re-opens. That is a
   **measurable** perf delta vs O1 and must be benched per Invariant 11/18 (`phg benchmark` + the
   PHP baseline) before any perf claim. If the bench flags it, O1 is the recorded upgrade — and
   because the *user-facing surface is identical*, O1 later is a **non-breaking internal swap**,
   exactly the "surface contract stable, drivers upgrade underneath" pattern the register already
   blessed for DB streams (`C-decisions.md:979-986`).
4. **It is not a semantic downgrade** (Invariant 14 case 3 is not triggered): O2 delivers real
   incremental delivery with real O(1) memory. It is an *implementation* trade-off, not a semantic one.

**What a v1 needs either way** (the checklist the developer should see before ruling):

| Dimension | Requirement |
|---|---|
| **New type(s)** | O2: **none** at the `Value`/`Ty` level — one prelude class `FileLines implements Iterator<string>`. O1: a reserved opaque `FileHandle` (`resolve.rs:289-300` recipe) + a `Value` variant + `emit_type → mixed`. |
| **Protocol** | Already exists. Copy `InputLines` (`preludes.rs:77-99`): one-line lookahead in `hasNext()`, hand-over in `next()`, `panic("iterator exhausted")` past the end (the DEC-257 misuse contract, `preludes.rs:74-76`). |
| **`for` integration** | Free — `flow.rs:457-479` recognises any DIRECT `implements Iterator<E>`; `rewrite_foreach.rs` lowers it pre-backend. **Must declare `implements Iterator<string>` on the class itself**, not via a parent (C5a). |
| **Errors** | `throws FileSystemError` on `hasNext`/`next`, matching the module taxonomy (`preludes.rs:141-161`). The loop-site auto-propagation is already ruled and implemented (`flow.rs:460-477`). **C5b is a live risk** for `Iterator<string>`-typed parameters. |
| **Transpile shape** | `fopen`/`fgets`/`fclose` (O1) or `file_get_contents($p,false,null,$off,$len)` (O2). Both are ordinary idiomatic PHP; `fgets` verified present on php-8.5.8. **Wrap in the existing `__phorj_fs_*` helper pattern** (`src/transpile/fs_php.rs:47-67`) returning the `[ok, payload]` pair the `FileSystemResult` call-site wrap expects (`src/native/fs.rs:49-51`, DEC-313). |
| **Lift shape** (Invariant 17 — mandatory same-change) | PHP `while(($l=fgets($h))!==false){…}` → `for (string l in FileSystem.lines($p))`. Today the lifter has no target for this at all. |
| **Memory guarantee** | O(chunk) / O(longest line), independent of file size. Must be *measured* (the P1-B `peak.sh` recipe) — Invariant 11 forbids an unmeasured perf/memory claim. |
| **`limits.rs`** | Consider a **new** documented constant — e.g. a max single-line length so a pathological 1-line 10 GB file faults cleanly instead of OOMing. `limits.rs` currently has **no** I/O limit (`src/limits.rs`, full read) and `limit_values_are_stable` (`:56-64`) must be extended in the same commit if one is added. **This is itself a developer decision** — adding a cap changes observable failure behaviour, which is parity-affecting (Invariant 1: identical *failure behaviour*, not just stdout). |
| **Naming** | Must not collide with the eager reads. Industry precedent is a distinct name (`ReadLines` vs `ReadAllLines`); `FileSystem.lines()` mirrors `Input.lines()` exactly and is the consistent choice. |
| **Examples** (Invariant 9) | A runnable `examples/fs/*.phg` + an `examples/README.md` entry, in the same change. |

---

# TOPIC 2 — Filesystem locking ("lock a file, access it when available")

## 2.1 Current state (evidence)

### There is NO locking anywhere in phorj

A repo-wide grep for `flock|LOCK_EX|LOCK_SH|LOCK_NB|try_lock|advisory|withLock|lockFile|file_lock`
across `src/`, `docs/`, `examples/` (excluding `RwLock`/`Mutex`/`.lock()` noise) returns **zero
filesystem-locking code**. Every `Lock*` hit is the package-manager lockfile
(`src/pm/lockfile.rs`, `src/pm/vendor.rs`, `src/pm/ops.rs`) — an unrelated `phorj.lock` manifest.
[Verified: the grep, reviewed hit-by-hit.]

### And no atomicity either — both legs write unlocked, consistently

| Operation | Rust leg | PHP leg |
|---|---|---|
| `writeText` | `std::fs::write(p, contents)` — truncate+write, no lock (`src/native/fs_bodies.rs:91`) | `@file_put_contents($p,$c,0)` — **no `LOCK_EX`** (`src/transpile/fs_php.rs:54`) |
| `writeBytes` | `std::fs::write` (`fs_bodies.rs:101`) | same helper (`src/native/fs.rs:144`) |
| `appendText` | `OpenOptions::new().create(true).append(true)` + `write_all` (`fs_bodies.rs:112-116`) | `@file_put_contents($p,$c,FILE_APPEND)` — **no `LOCK_EX`** (`fs_php.rs:54`) |
| `move` | `std::fs::rename` (`fs_bodies.rs:130`) | `@rename` (`fs_php.rs:66`) |

[Verified: read both sides.] There is **no** temp-file+atomic-rename write path in the FS module
(the only atomic-rename on record is build-cache-internal, `docs/specs/UNIFIED-SPEC.md:1604`).
The two legs are *consistently* unlocked, so byte-identity holds for a single process — but there is
no concurrency story at all.

### Register status: NO disposition whatsoever

`flock` has **no DEC row, no gap-matrix disposition, no queue entry, no reject**. The only three
substantive repo-wide hits are inventory-level:
- `docs/research/full-audit/raw/D-php-surface.md:514` — `- FN-FS-016 flock (LOCK_SH/EX/UN/NB)`
  (side-A PHP inventory only)
- `docs/research/full-audit/raw/D-php-surface.md:517` — `- FN-FS-019 file_put_contents (FILE_APPEND, LOCK_EX)`
- `docs/research/php-gap-round2.md:84-86` — swept into an unruled bulk bucket
  (*"realpath/chmod/**flock**/tempnam … all matrix rows (GU/P/GP)"*)

Zero hits in MASTER-PLAN, SLICE-STATE, C-decisions, M-gap-matrix, E-phorj-surface,
F-cross-language, KNOWN_ISSUES, FEATURES, UNIFIED-SPEC. [Verified: delegated doc sweep, negative
explicitly confirmed per file.] **So this is a genuinely un-adjudicated question — the developer
has never ruled on it.**

### THE PRESUMED BLOCKER IS FALSE — std HAS portable advisory locking on the pinned toolchain

The task brief anticipated *"Rust `std::fs` (no portable flock in std!)"* and a resulting
dependency-policy question. **That is no longer true.** `std::fs::File` file locking was stabilised
upstream and **is present on the pinned toolchain**.

`rust-toolchain.toml` pins `channel = "1.97.1"`; `rustc --version` → `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
[Verified: read + ran.]

**Probe P2-A** — compiled and ran a standalone program against that exact toolchain
(`scratchpad/probe-std/lockprobe.rs`, `rustc --edition 2021 -O`):

```
lock() OK (exclusive, blocking)
lock_shared() OK
try_lock() OK -> acquired
try_lock_shared() OK
ALL std-only file locking APIs present on rustc 1.97.1
```

[**Verified**: compiled clean (no `feature` gate, no nightly) and ran; output as shown.]
So `File::lock`, `File::lock_shared`, `File::try_lock`, `File::try_lock_shared`, `File::unlock`
are all available in **stable std**.

**Consequence for the dependency policy: there is NO dependency question.** The policy
(`docs/specs/UNIFIED-SPEC.md:879-885`) admits a crate only when *"**No `std`-only path is both
secure and Phorj-native**"* (clause 3, `:914-915`) — and here the std-only path **exists**.
Clause 1 would also have excluded a `fs2`/`nix`/`libc` admission outright: *"Convenience,
performance, general-purpose … crates do **not** qualify"* (`:903-905`), and
*"Anything outside the admitted domains requires **revisiting this policy itself**, not just adding
a row"* (`:920-921`). And phorj's own `unsafe` is barred outside the JIT island
(`FEATURES.md:128-129`: *"`#![deny(unsafe_code)]` on both crate roots; the JIT's audited `unsafe`
(confined to `src/jit/`) is the sole exception"*). **All three exits are closed and none is needed.**
[Verified: policy text quoted from the doc sweep + P2-A.]

> **Side note (P3, tangential):** `CLAUDE.md:8-9` says the core is std-only with *"four vetted,
> feature-gated exceptions — `argon2`, `regex`, `ctrlc`, `corosensei`"*. `Cargo.toml` actually
> declares **eleven** admitted domains including `unicode-segmentation`, `rustls`, `webpki-roots`,
> `rusqlite`, `postgres`, `mysql`, `lettre`, `cranelift-*` (`Cargo.toml:127-180`), and
> `UNIFIED-SPEC.md:871-877` itself warns *"'zero external dependencies' claims in older docs are
> **false and must not be repeated**"*. The CLAUDE.md line is stale.
> [Verified: read `Cargo.toml:127-180` + the spec quote.]

### Windows IS a shipped target — so portability is a real constraint, not a hypothetical

`docs/specs/UNIFIED-SPEC.md:1420-1422` — *"Linux (gnu+musl, x86_64+aarch64), **Windows (x86_64)**,
macOS"*; `:1510-1513` — *"**SHIPPED (v0.4.0)** — Linux … + **Windows (x86_64-gnu) cross builds**"*;
`FEATURES.md:113` — *"Standalone executable (Linux cross + Windows) | 🔨"*.
[Verified: doc sweep quotes.]

`std::fs::File::lock` is portable by construction (std implements it over `flock(2)` on Unix and
`LockFileEx` on Windows) — **which is the decisive advantage over a `libc::flock` FFI or a
Unix-only crate.** [Inferred: std would not have stabilised a non-portable `fs` API; the probe
confirms the Unix behaviour but this host is Linux.]
**⚠ [Unverified: no Windows host available in this container]** — std's documentation carries a
platform-behaviour caveat that Windows locks are **mandatory** (kernel-enforced) rather than
advisory, and PHP's `flock()` on Windows also differs from Unix. **A cross-platform semantics
statement must be verified on a Windows runner before it is documented as a guarantee.**

## 2.2 Probe transcripts

### P2-B — cross-process semantics: blocking wait, try-lock, and advisory-not-mandatory

`scratchpad/probe-std/lockprobe2.rs` — one holder taking `File::lock()` for 1500 ms, three contenders:

```
holder:    exclusive lock acquired
contender: try_lock  -> Err("WouldBlock")            ← try-lock reports contention, does not block
contender: wrote WITHOUT lock -> ADVISORY only        ← a non-cooperating writer is NOT blocked
holder:    releasing
contender: blocking lock() waited 1192ms             ← blocking wait works, acquires on release
```

[**Verified**: ran; output verbatim.] So: `try_lock()` → `Err(TryLockError::WouldBlock)` on
contention; `lock()` blocks until available (the developer's literal ask — *"access a file when it
is available"*); the lock is **advisory** (a writer that does not lock is unaffected).

### P2-C — PHP 8.5 `flock` semantics AND bidirectional interop with Rust std locks

```
--- Rust holds exclusive; PHP tries LOCK_EX|LOCK_NB ---
array(2) { ["acquired"]=> bool(false)  ["would_block"]=> int(1) }
--- no holder; PHP tries LOCK_EX|LOCK_NB ---
array(2) { ["acquired"]=> bool(true)   ["would_block"]=> int(0) }
--- PHP holds exclusive; Rust try_lock ---
php holder: locked
contender: try_lock -> Err("WouldBlock")
php holder: released
```

[**Verified**: ran `flock.php` / `hold.php` against `/stack/tools/phpbrew/php/php-8.5.8/bin/php`
interleaved with `lockprobe2`; output verbatim.]

Also verified on php-8.5.8: `function_exists('flock')` → `true`;
`LOCK_SH=1, LOCK_EX=2, LOCK_UN=3, LOCK_NB=4`; the third `&$would_block` out-param works.

**This is the strongest result in the whole report for topic 2**: the Rust leg and the PHP leg use
the *same OS advisory lock* and see each other **bidirectionally**. A phorj lock and its transpiled
PHP twin are not merely "equivalent shapes" — they are literally the same lock.

## 2.3 Cross-language scan (Invariant 16)

| Language | API | Blocking | Try | Shared/Excl | Scoped/RAII | Portable |
|---|---|---|---|---|---|---|
| **PHP** | `flock($h, LOCK_EX[\|LOCK_NB], &$wouldBlock)` + `LOCK_UN` | yes (default) | yes (`LOCK_NB`) | yes | **no** — manual, leaks on early return | yes-ish (Windows differs) |
| **Rust std** ≥1.89 | `File::{lock, lock_shared, try_lock, try_lock_shared, unlock}` | yes | yes (`Err(WouldBlock)`) | yes | no explicit guard type (released on `File` drop) | **yes** (flock/LockFileEx) |
| **Go** | `syscall.Flock(fd, LOCK_EX)` | yes | `LOCK_NB` | yes | no | no (Unix syscall; Windows needs a separate path) |
| **Java** | `FileChannel.lock()` / `tryLock()` → `FileLock` (`AutoCloseable`) | yes | yes | yes | **yes** (try-with-resources) | yes |
| **Python** | `fcntl.flock` (Unix) / `msvcrt.locking` (Windows) — two different APIs | yes | `LOCK_NB` | yes | no | **no** (caller branches per-OS) |
| **C#** | `FileStream` share modes; `FileStream.Lock(off,len)` (byte-range) | — | — | byte-range | `using` | yes |
| **Kotlin/Swift** | delegate to Java `FileLock` / POSIX | — | — | — | yes | — |

**Convergent design lessons:**
1. Every language that has a **scoped** form (Java `try-with-resources`, C# `using`) makes it the
   recommended one, precisely because manual lock/unlock leaks on early return / on throw.
2. **PHP is the leak-prone one** (manual `LOCK_UN`) — so a scoped phorj API is *strictly better than
   PHP* while still transpiling to PHP's own primitive. That is squarely the Invariant 16
   "byte-identity is a tool, not the priority ordering" posture.
3. Whole-file advisory is the universal common denominator; byte-range (`fcntl`/C# `Lock(off,len)`)
   is not portably expressible on Rust std today.

**LADDER RULE (Invariant 14) verdict for topic 2: CASE 1 — faithful idiomatic PHP exists, and P2-C
proves it is the SAME lock.** `flock()` is the direct, idiomatic PHP twin. **No `E-TRANSPILE-LOCK`
is warranted, no quarantine beyond the FS module's existing `pure:false` exclusion.**
One honest caveat to surface: a *scoped* `withLock(path, closure)` is strictly *safer* than PHP's
manual `flock`/`LOCK_UN`, so the PHP leg would need a `try { … } finally { flock(LOCK_UN); fclose(); }`
wrapper (or a `__phorj_fs_with_lock` helper) to preserve the guarantee. Per Invariant 16 that
helper-emission trade **must be surfaced and ruled by the developer, never self-decided** — it is
surfaced here.

## 2.4 Gaps (severity + grade)

- **C7 · P1 · No locking primitive at all, and no atomic-write path.** Two phorj processes (or a
  phorj process and any other program) writing the same file interleave freely; `appendText`'s
  `O_APPEND` gives atomicity only for small single writes, and `writeText`'s truncate+write has a
  visible torn window. [Verified: `src/native/fs_bodies.rs:86-119`, `src/transpile/fs_php.rs:47-57`,
  plus the zero-hit lock grep.]

- **C8 · P1 · The `phg serve` + `Core.SessionModule` combination makes this concretely load-bearing,
  not theoretical.** `phg serve` runs multiple workers (`src/native/input.rs:14-16`: *"`phg serve`
  disables stdin before workers run"*), so any file-backed state written from a handler is a
  multi-writer race today. [Inferred: from the worker model in the stdin-disable rationale; the
  concrete session-store backing was not traced in this pass — **[Unverified]** whether the current
  session store is file-backed.]

- **C9 · P2 · The question has never been adjudicated.** Not a DEC, not a gap-matrix disposition,
  not queued, not rejected — only two side-A inventory rows
  (`docs/research/full-audit/raw/D-php-surface.md:514,517`) and one unruled bulk bucket
  (`docs/research/php-gap-round2.md:84-86`). Per Invariant 15 this is squarely the developer's
  call. [Verified: doc sweep negative.]

- **C10 · P2 · Windows semantics are UNVERIFIED and must not be documented as a guarantee.**
  Windows is a shipped target (`UNIFIED-SPEC.md:1420-1422,1510-1513`) but std's Windows locks are
  reportedly **mandatory** rather than advisory, and PHP's `flock()` on Windows differs too — which
  would mean the *documented semantics* differ per platform even though the *API* is portable.
  **[Unverified: no Windows host in this container.]** Also note `KNOWN_ISSUES.md:1742`:
  *"aarch64 / Windows artifacts aren't executed in CI here"* — so CI would not catch a divergence
  either. [Verified: the doc quotes; the semantics claim itself is Unverified.]

- **C11 · P3 · `emit_type` gap applies here too if a handle type is introduced.** Same as C4 — a
  `FileLock`/`FileHandle` opaque type has no PHP type-hint mapping today
  (`src/transpile/types.rs:47,71`). A **scoped-closure** API avoids this entirely (no handle ever
  reaches user code).

## 2.5 Options & recommendation — topic 2

Four semantics forks the developer must rule (per the brief), with what the evidence says about each:

| Fork | Options | Evidence |
|---|---|---|
| **Wait discipline** | blocking-wait / try-lock / timeout | Blocking + try are both **Verified** available in std (P2-A/P2-B) and in PHP (P2-C). **Timeout is NOT natively available** on either side — it would have to be a spin-with-sleep loop, which is a bandaid shape and would make timing observable (parity-hostile). |
| **Advisory vs mandatory** | — | **Not a choice on Unix**: std and PHP both give advisory (P2-B: an unlocked writer succeeded). On Windows it may be mandatory (C10). |
| **Granularity** | whole-file / byte-range | **Whole-file only** is portably expressible — Rust std exposes no byte-range lock. Byte-range would need `libc`/`fcntl` → dependency + `unsafe` → **blocked by policy** (`UNIFIED-SPEC.md:903-905, 914-915, 920-921`). |
| **Lifetime** | scoped closure (`withLock(path, fn)`) / manual `lock()`+`unlock()` | Every language with a scoped form recommends it; PHP's manual form is the leak-prone one. Scoped also avoids C11 entirely. |

| | Option | Surface | Trade |
|---|---|---|---|
| **O5** | **Scoped closure, both disciplines** — `FileSystem.withLock<T>(path, () => T): T throws FileSystemError` (blocking) + `FileSystem.tryWithLock<T>(path, () => T): T? throws FileSystemError` (`null` when contended) | no handle ever visible; release guaranteed by construction | needs a `try/finally` PHP wrapper (Invariant 16 trade — surfaced) |
| **O6** | Manual handle — `FileLock l = FileSystem.lock(path); … l.unlock();` | 1:1 with PHP `flock`/`LOCK_UN` | leak-prone (C11 + PHP's own known footgun); needs an opaque type + `emit_type` mapping |
| **O7** | Locking folded invisibly into `writeText`/`appendText` (always `LOCK_EX`, like PHP's `file_put_contents(..., LOCK_EX)`) | zero new API | **changes existing behaviour** on both legs; a silent semantic change to a shipped surface — and does nothing for read-modify-write, which is the actual use case |
| **O8** | Atomic-replace write instead of locking — `writeAtomic(path, contents)` = temp + `rename` | no lock at all; solves "never see a torn file" | does NOT solve "wait until available"; solves a *different* problem (both are probably wanted) |

### RECOMMENDATION: **O5 (scoped closure, blocking + try variants), whole-file, advisory — with O8 offered as a companion, not a substitute.** Surfaced for the developer's ruling.

**Why:**
1. **It answers the literal ask.** *"lock … access a file when it is available"* is `File::lock()`'s
   blocking wait — **Verified** working across processes in P2-B (waited 1192 ms, then acquired).
2. **Zero dependency question, zero `unsafe`, zero policy amendment.** P2-A proves the std API is on
   the pinned toolchain. This is the single most important finding of topic 2: the expected blocker
   does not exist.
3. **The PHP leg is not merely equivalent, it is the SAME LOCK** (P2-C, bidirectional). That makes
   this LADDER case 1 with unusually strong evidence — no quarantine, no `E-TRANSPILE-*`.
4. **Scoped beats manual on the one axis phorj cares about**: a lock that cannot leak. PHP's manual
   `LOCK_UN` is a documented footgun; every language with a scoped form makes it the default. This
   is a "better than PHP" departure of exactly the shape the register already blessed for
   `Core.Deque`/`Core.PriorityQueue` (`src/cli/preludes.rs:243-251, 290-298`: PHP `Spl*` throws on
   empty; phorj returns `T?` — *"safer, more OOP, and impossible to forget to guard"*).
5. **Whole-file + advisory is not a preference, it is the portable ceiling.** Byte-range needs
   `fcntl` → a dependency and `unsafe` → both blocked. Say so explicitly rather than implying a
   choice exists.
6. **Reject timeout for v1** — no native support on either leg; a spin-sleep is a bandaid and makes
   wall-clock observable (determinism-hostile, Invariant 10 adjacent). `tryWithLock` + a
   user-written retry loop covers the need honestly.

**Must be surfaced in the ruling (do not let these be decided silently):**
- The **`try/finally` PHP-helper emission** needed to make the scoped guarantee survive transpilation
  (Invariant 16: the trade is the developer's).
- The **Windows semantics divergence** (C10) — if it is real, it needs a disclosure paragraph
  wherever the lock's semantics are documented, and it must be **verified on a Windows runner**
  before shipping, not assumed.
- Whether **O8 (`writeAtomic`)** ships alongside — the developer's phrasing is about *waiting*, but
  "never read a half-written file" is the sibling problem and the cheaper of the two.

---

# TOPIC 3 — Cloning with no modification

## 3.1 Current state (evidence)

### The syntax is `obj with { field = expr, … }` — postfix, and it ALREADY accepts an EMPTY brace list

AST node (`src/ast/exprs.rs:233-241`):

```
/// `obj with { field = expr, … }` — a functional update (M-mut.4a, Fork 2 = B): a fresh instance
/// copying `object`'s fields with the named ones overridden, **bypassing the constructor**.
/// `object` must be a concrete class; `fields` names a subset of its (promoted) fields. Lowers to
/// the existing `Op::MakeInstance` (no new `Op`); transpiles to PHP `clone($obj, ['f' => …])`.
CloneWith { object: Box<Expr>, fields: Vec<(String, Expr)>, span: Span },
```

Parser (`src/parser/exprs/climb.rs:563-584`) — the field loop is
`while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof)`, so **zero fields falls out
naturally**; nothing rejects it. [Verified: read both.]

The empty form is **explicitly pinned by a test** — `src/parser/tests/stmts.rs:221-224`:

```
    // empty override list parses.
    match expr("p with { }") {
        Expr::CloneWith { fields, .. } => assert!(fields.is_empty()),
```

[Verified: read.] Checker: `check_clone_with` (`src/checker/assign.rs:338-396`) validates the
receiver is a concrete class (`E-WITH-NONCLASS`, `:351-359`) then iterates `fields` — with zero
fields the loop body never runs, so **no diagnostic is possible**. It returns `obj_ty` unchanged
(`:395`). [Verified: read the whole function.]

**There is NO `clone` keyword and no `.clone()` method.** `clone` exists in the tree only as a
PHP-reserved word phorj refuses as a *symbol* name (`KNOWN_ISSUES.md:645-651`) and as a PHP
construct the LIFTER refuses (`src/lift/parser/exprs.rs:303-304`). [Verified: greps + probes P3-B/C.]

### Semantics as implemented: SHALLOW, constructor bypassed, source untouched

`examples/guide/clone-with.phg:7-9` states the contract: *"produces a FRESH instance copying `obj`
with the named fields replaced. The constructor is NOT re-run, and the source instance is never
modified."* [Verified: read.]

Depth follows from the value model, which is documented and not `with`-specific:
`docs/MILESTONES.md:205-207` — *"`List`/`Map`/`Set`/`Bytes` are **copy-on-write value types** …
`Instance` is a **shared-mutable handle** (PHP/Java semantics)"*; `src/value/types.rs` (the
`Instance` doc, ~`:225-232`) — *"a field write through one binding (`o.f = e`) is visible through
every other binding — PHP/Java object semantics"*. So a cloned instance's *instance-typed* fields
are shared. **Confirmed by probe P3-D.** [Verified: doc + probe.]

### Register status: no DEC, no spec text, no ruling on the empty form

- `obj with { … }` has **no DEC number** — it is milestone slice **M-mut.4a**
  (`docs/MILESTONES.md:210`, `CHANGELOG.md:3168` *"functional update (fresh instance via
  `Op::MakeInstance`)"*, `KNOWN_ISSUES.md:1284`).
- `docs/specs/UNIFIED-SPEC.md` has **ZERO hits** for `with {`, `CloneWith`, `functional update`, or
  `record update` — **the spec never states the `with` grammar at all.**
- The **empty** `with { }` no-op has **no recorded design ruling anywhere** — the only artifact
  pinning it is the parser test above. Treat it as *allowed-by-construction and test-locked*, **not
  adjudicated**.
- `__clone` sits at `N/A` (`docs/research/full-audit/raw/M-gap-matrix.md:166`: *"`with { }`
  clone-update covers the use without a hook"*) with a stale contrary `adopt` in
  `docs/research/roadmap-completeness/raw/A.md:35`.
- Phorj shipped this **before** PHP: `M-gap-matrix.md:92` — *"Phorj shipped 8.5's clone-with first"*;
  `:332` — *"shipped before PHP 8.5's `clone($o, [...])`"*.

[Verified: delegated doc sweep, verbatim quotes per line.]

## 3.2 Probe transcripts

### P3-A — `p with { }` WORKS end-to-end on all three legs

`scratchpad/probe-std/clone-empty.phg`:

```
class Point { constructor(public int x, public int y) {} }
Point p = new Point(1, 2);
Point q = p with { };
Output.printLine("q = ({q.x}, {q.y})");
Output.printLine("same object? {p == q}");
```

```
--- VM run ---            q = (1, 2) / same object? true / exit=0
--- tree-walker ---       q = (1, 2) / same object? true / exit=0
--- transpile ---         $q = clone($p);
```

[**Verified**: ran all three; outputs verbatim.] **A no-op clone IS expressible today.**
Note the transpiler emits bare **`clone($p)`** for the empty case — not `clone($p, [])` — i.e. it
already special-cases the empty field list. `p == q` is `true` because `==` is structural, not
identity.

### P3-B — bare `clone p` is a PARSE ERROR

```
parse error at 9:21: expected ';' after variable declaration, found Ident("p")
    Point q = clone p;
                    ^
```

[**Verified**: ran.] There is no `clone` prefix keyword.

### P3-C — `p.clone()` is a TYPE ERROR

```
type error at 9:22: type `Point` has no method `clone`
```

[**Verified**: ran.] There is no UFCS/method `clone` either.

### P3-D — depth: SHALLOW, and both backends agree

```
class Box   { constructor(public mutable int n) {} }
class Outer { constructor(public mutable Box inner, public mutable int k) {} }
Outer c = o with { };
c.k = 99;             → o.k=5  c.k=99          ← scalar field is independent (fresh instance)
c.inner.n = 42;       → o.inner.n=42  c.inner.n=42   ← nested INSTANCE is SHARED
```

VM and `--tree-walker` produced byte-identical output. [**Verified**: ran both.]
So `with { }` is a **shallow** clone with PHP `clone` semantics exactly.

### P3-E — PHP 8.5 has native `clone with`; PHP 8.4 does NOT

On `/stack/tools/phpbrew/php/php-8.5.8/bin/php` (`PHP 8.5.8`):

```
clone($p, ["x"=>9])  → int(9), int(2)   ← native clone-with WORKS
clone($p)            → int(1)           ← works
clone($p, [])        → int(1)           ← EMPTY array works
```

On the bare `php` on PATH (`PHP 8.4.19`):

```
PHP Parse error:  syntax error, unexpected token "," in Command line code on line 1
```

[**Verified**: ran both.] **This materially confirms the transpile floor matters**: the whole
`with { … }` → `clone($o, [...])` mapping requires **PHP ≥ 8.5**, which is exactly the declared floor
(`CLAUDE.md:49-50`). The *empty* case is even safer — it emits bare `clone($p)`, valid since PHP 5.
[Verified: P3-A output + this probe.]

### P3-F — the formatter keeps the empty form, but prints a double space

```
$ phg format clone-empty.phg   → "1 file(s) formatted, 0 error(s)"
line 14:    Point q = p with {  };     ← TWO spaces between the braces
$ format again → IDEMPOTENT (diff empty)
$ run after format → q = (1, 2) / same object? true
```

[**Verified**: ran; `diff` empty, program still correct.] Cosmetic only — idempotency (and hence
`every_repo_phg_formats_idempotently_and_safely`) is intact. Cause: the `CloneWith` printer at
`src/format/printer/exprs.rs:315-327` joins an empty field list inside a `{ … }` that already
carries its own padding.

### P3-G — the LIFTER refuses it (Invariant 17 gap)

`src/lift/printer/exprs.rs:217-224` — `Expr::CloneWith { .. }` is in the refusal arm:

```
"printer: bytes/lambda/clone-with/inject/html/tagged-template are outside the lift subset"
```

and `src/lift/parser/exprs.rs:303-304` refuses PHP `clone` on the way in as *"Tier-2/Tier-3"*.
[Verified: read both.] So **a PHP `clone $x` cannot be lifted to phorj at all**, even though phorj
has the exact target construct.

## 3.3 Cross-language scan (Invariant 16)

| Language | No-change copy | Shape | Hook |
|---|---|---|---|
| **PHP** | `clone $o` (statement-keyword); `clone($o, [])` (8.5) | shallow | `__clone()` post-hook; 8.3 allows readonly re-init inside it |
| **Rust** | `x.clone()` (`Clone` trait); `#[derive(Clone)]` | user-defined depth (usually deep) | trait impl |
| **C#** | `with { }` on records — **empty `with { }` is legal and is the idiomatic no-op copy**; `MemberwiseClone()` | shallow | `protected Copy(record)` ctor |
| **Kotlin** | `data class`'s generated `copy()` — **`copy()` with no args is THE canonical no-op copy** | shallow | none |
| **Swift** | value semantics — assignment copies; `struct` needs nothing | COW | `init(copying:)` convention |
| **Java** | records: no `with`; `Cloneable`/`clone()` widely regarded as broken | shallow | `clone()` override |
| **JS/TS** | `{...o}` / `structuredClone(o)` | shallow / deep | — |

**Convergent conclusion — phorj is already in the majority camp.** The two languages whose syntax
phorj's `with { }` is directly modelled on (**C# records `with { }`**, **Kotlin `copy()`**) both make
the **argument-less / empty form the canonical no-op copy**. `p with { }` is therefore not an
accident to be tidied away — it is the idiomatic spelling in the very family this syntax came from.
[Verified: the language shapes; [Speculative] on "therefore phorj should keep it" as a normative claim.]

**LADDER RULE (Invariant 14) verdict for topic 3: CASE 1, already satisfied.** `p with { }` already
transpiles to `clone($p)` (P3-A) — the most faithful idiomatic PHP possible. Nothing to ladder.
The only Invariant-17 debt is the **lift** direction (P3-G).

## 3.4 Gaps (severity + grade)

- **C12 · P2 · DISCOVERABILITY, not capability — this is the real answer to the developer's question.**
  `p with { }` works on all three legs (P3-A) but is: absent from `examples/guide/clone-with.phg`
  (which only shows 1-field, multi-field, and self-referential overrides — `:21,27,34`), absent from
  `FEATURES.md` (`:69` mentions *"functional `obj with { … }`"* with no no-op note), and **absent
  from `UNIFIED-SPEC.md` entirely** (zero hits for `with {`). A developer asking *"do we have a way
  now??"* about a feature that already ships is itself the evidence that the docs failed.
  [Verified: probe P3-A + doc-sweep negatives.]

- **C13 · P2 · Invariant 17 violation: `with { }` transpiles but does NOT lift.**
  `src/lift/printer/exprs.rs:217-224` refuses `CloneWith`; `src/lift/parser/exprs.rs:303-304`
  refuses PHP `clone`. Invariant 17 requires *"transpile AND lift updated in the same change"* —
  this is pre-existing debt, and it is the one place PHP→phorj migration loses a construct phorj
  actually has. [Verified: read both files.]

- **C14 · P3 · No recorded design ruling for the empty form.** Only `src/parser/tests/stmts.rs:221-224`
  pins it. If the developer *wants* it (and the C#/Kotlin scan says it is idiomatic), it should be a
  recorded decision + an example, not a parser-test accident. If the developer does *not* want it,
  the current state is a silent surface with no diagnostic.
  [Verified: doc-sweep negative + the test.]

- **C15 · P3 · Shallow-vs-deep is nowhere stated in `with`'s own documentation.**
  `examples/guide/clone-with.phg` says *"FRESH instance"* and *"the source instance is never
  modified"* — both true of the *cloned object* but **silent about nested instance fields**, which
  P3-D shows are shared. A reader could reasonably expect the guarantee to be transitive. The value
  model does document it (`docs/MILESTONES.md:205-207`) — but not at the point of use.
  [Verified: read the example + probe P3-D.]

- **C16 · P3 · Formatter emits `with {  }` (double space)** for the empty case
  (`src/format/printer/exprs.rs:315-327`). Idempotent and harmless, but it is the *canonical
  spelling* if the empty form gets blessed. [Verified: P3-F.]

- **C17 · P3 · No `__clone`-equivalent hook, and the register contradicts itself.**
  `M-gap-matrix.md:166` disposes it `N/A` (*"`with { }` clone-update covers the use without a
  hook"*) while `roadmap-completeness/raw/A.md:35` still lists it `adopt`. Only matters if
  post-copy fixup is ever wanted. [Verified: doc sweep.]

## 3.5 Options & recommendation — topic 3

| | Option | Surface | Cost | Trade |
|---|---|---|---|---|
| **O9** | **Bless `p with { }` as the canonical no-op clone** — document it, example it, record the decision, fix the double space | **zero code change to semantics**; docs + example + register row + a 1-line formatter fix | ~nil | matches C#/Kotlin idiom exactly; the transpile is already `clone($p)` (P3-A) |
| **O10** | Add a bare `clone x` prefix keyword | `Point q = clone p;` | new token, new AST node or a parser sugar → `CloneWith{fields:[]}`, formatter, LSP, lift; **`clone` is PHP-reserved as a symbol name** (`KNOWN_ISSUES.md:650`) so name-collision guards need review | most PHP-familiar; **two spellings for one operation** — the exact "dual API forever" failure mode DEC-257 explicitly rejected (`C-decisions.md:1493-1499`) |
| **O11** | Add `x.clone()` UFCS/method | `Point q = p.clone();` | needs a universal method on every class, or a `Core.Clone` trait + conformance; interacts with user classes that define their own `clone` | Rust-familiar, but collides with user-defined `clone` methods and needs a whole trait story |
| **O12** | `Core.Clone.of(x)` native | `Point q = Clone.of(p);` | a new module for one function | least ergonomic; a native returning a fresh `Instance` duplicates what `Op::MakeInstance` already does |

### RECOMMENDATION: **O9 — bless and document the existing `p with { }`; add NOTHING to the language.** Surfaced for the developer's ruling.

**Why:**
1. **The capability already exists and is already correct on all three legs** (P3-A: VM ≡ tree-walker
   ≡ `clone($p)`; P3-F: format-idempotent; P3-D: consistent shallow semantics on both backends).
   The developer's question — *"do we have a way now??"* — has answer **yes**. Adding syntax to
   deliver a shipped capability is pure surface inflation.
2. **It is the idiomatic spelling in the exact language family this syntax came from.** C# records
   (`with { }`) and Kotlin data classes (`copy()`) both make the empty/argument-less form the
   canonical no-op copy. Phorj is not being clever here; it is being conventional.
3. **O10/O11 create two spellings for one operation** — the "dual API forever on the flagship type"
   failure mode the register explicitly rejected when reshaping the DB streams
   (`C-decisions.md:1493-1499`). Every alternative costs token/AST/formatter/LSP/lift work to buy
   nothing semantically new.
4. **The genuine debt is elsewhere and should be fixed instead**: **C13** (lift refuses `CloneWith`
   — a live Invariant 17 violation), **C12/C15** (docs silent on both the no-op form and the shallow
   depth), **C14** (no recorded ruling), **C16** (double space). That is a small, high-value docs +
   lift slice, not a language change.

**Concretely, O9 = (surfaced as a proposal, not started):**
- `examples/guide/clone-with.phg`: add the no-op case **and** a nested-instance case making the
  shallow boundary visible (closes C12 + C15 in the file a user actually reads).
- `FEATURES.md:69` / `docs/specs/UNIFIED-SPEC.md`: state the `with` grammar at all — the spec
  currently never does (closes C12's worst part).
- Decision register `docs/research/full-audit/raw/C-decisions.md`: record the ruling on the empty
  form (closes C14; per Invariant 19 the register is the canonical home).
- `src/format/printer/exprs.rs:315-327`: emit `p with { }` not `p with {  }` (closes C16).
- **Lift** (`src/lift/parser/exprs.rs:303`, `src/lift/printer/exprs.rs:217-224`): map PHP
  `clone $x` → `x with { }` and un-refuse `CloneWith` on the printer side (closes C13, satisfies
  Invariant 17). ⚠ This is the only part with real risk — PHP `clone` can invoke `__clone`, which
  phorj has no equivalent for (C17), so the lift must **refuse loudly** when the source class
  declares `__clone` rather than silently dropping the hook (Invariant 14 case 3 — silent semantic
  downgrade is FORBIDDEN). **That refusal boundary is itself a developer decision.**

---

# CROSS-TOPIC FLAGS

## LADDER-RULE (Invariant 14) implications

| Topic | Ladder case | Basis |
|---|---|---|
| (1) File streaming | **CASE 1 — faithful idiomatic PHP exists** | PHP `fopen`/`fgets` verified present on php-8.5.8; the *identical* `Iterator`+`fgets` shape is **already** byte-identical on all three legs (probe P1-A). No `E-TRANSPILE-*`, no new quarantine (the FS module is already `pure:false`-excluded). |
| (2) FS locking | **CASE 1 — and unusually strong: it is the SAME OS LOCK** | Probe P2-C: Rust-std `File::lock` and PHP `flock()` block each other bidirectionally. No `E-TRANSPILE-LOCK` warranted. |
| (3) No-op clone | **CASE 1, already satisfied** | `p with { }` → `clone($p)` today (P3-A). Nothing to ladder. |

**No topic requires ladder case 2 (native-only) and none is at risk of case 3 (silent downgrade) as
recommended.** Two case-3 tripwires to watch: (a) the O2 chunked reader must be honest that it is
chunk-based, not degrade the streaming contract; (b) the lift of PHP `clone` must **refuse** when
`__clone` is present rather than drop it.

## Invariant 16 trades that MUST be surfaced and ruled (never self-decided)

1. **Topic 2 — a `__phorj_fs_with_lock` PHP helper** (`try { … } finally { flock(LOCK_UN); fclose(); }`)
   to make a scoped lock's release guarantee survive transpilation. Byte-identity is preserved *by
   emitting a helper*, which is an acceptable tool — but the trade is the developer's.
2. **Topic 1 — the O2-vs-O1 perf/complexity trade** (chunk re-opens vs an opaque handle) and, if O1
   is chosen, the `emit_type("FileHandle") => "mixed"` special case (C4/C11) — the first transpiling
   opaque handle in the codebase.
3. **Topic 1 — whether to add an I/O limit to `limits.rs`.** Adding one changes observable *failure*
   behaviour, which Invariant 1 makes parity-affecting on all three legs.

## Dependency-policy implications

**NONE for any of the three topics.** This is the single most consequential finding:

- **Topic 2's expected blocker does not exist.** `std::fs::File::{lock, lock_shared, try_lock,
  try_lock_shared, unlock}` compile and run on the pinned `rustc 1.97.1` (probe P2-A) — verified,
  not recalled. So policy clause 3 (*"No `std`-only path is both secure and Phorj-native"*,
  `UNIFIED-SPEC.md:914-915`) is **not** met, and no crate may be admitted for locking even if one
  were wanted. Clause 1 (`:903-905`) would independently disqualify a general-purpose OS-integration
  crate, and `#![deny(unsafe_code)]` (`FEATURES.md:128-129`) bars a hand-rolled `libc::flock`.
  **All three routes closed, none needed.**
- **Topic 1** is `std::fs` + `std::io` only under every option.
- **Topic 3** needs no code at all under the recommendation.

**Stale-doc flag (P3):** `CLAUDE.md:8-9` claims *"four vetted, feature-gated exceptions — `argon2`,
`regex`, `ctrlc`, `corosensei`"*. `Cargo.toml:127-180` declares **eleven** domains
(+ `unicode-segmentation`, `rustls`, `webpki-roots`, `rusqlite`, `postgres`, `mysql`, `lettre`,
`cranelift-*`), and `UNIFIED-SPEC.md:871-877` explicitly warns that stale dependency claims
*"must not be repeated"*. [Verified: read both.]

## Invariant 17 (always-current surfaces) implications

- **Topic 1**: a `FileSystem.lines()` needs its **lift** shape in the same change
  (PHP `while(($l=fgets($h))!==false)` → the phorj `for`-loop). Today no such target exists.
- **Topic 2**: a lock API needs its **lift** shape (PHP `flock($h, LOCK_EX)` → the scoped form) —
  which is awkward, since PHP's manual lock/unlock does not map onto a closure without recognising
  the *pair*. **A recognised limitation to surface**, not a silent omission.
- **Topic 3**: **C13 is a live, pre-existing Invariant 17 violation** — `with { }` transpiles but
  does not lift.

## Pre-existing findings surfaced en route (not in scope, recorded)

- **C5b** (`src/checker/stmt/flow.rs:519-520, 526`): an interface-typed `Iterator<E>` value reports
  **empty throws**, so a throwing iterator passed as `Iterator<string> it` does not discharge its
  faults at the loop site the way a concrete type does. Currently benign (no stdlib `Iterator` in a
  parameter position throws), but a `FileSystem.lines()` **will** throw `FileSystemError` — this
  becomes load-bearing the moment topic 1 ships. [Verified: read `flow.rs:457-541`.]
- **C6**: `Input.readAll`/`readLine` double-allocate via
  `String::from_utf8_lossy(...).into_owned()` (`src/native/input.rs:113-117, 140-143`) — 2.25× the
  input measured in P1-B. A `Cow::Borrowed` fast path would avoid it. **Requires a `phg benchmark`
  before/after** per Invariant 11.

---

## Probe artifacts

All under `/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-std/`:

| File | Purpose |
|---|---|
| `clone-empty.phg` | P3-A — `p with { }` on 3 legs |
| `clone-bare.phg`, `clone-method.phg` | P3-B / P3-C — rejected alternative spellings |
| `clone-distinct.phg` | P3-D — shallow-vs-deep, both backends |
| `stdin-lines.phg`, `stdin-slurp.phg`, `fs-slurp.phg` | P1-B — memory measurements |
| `big.txt` (88,888,890 bytes, 2,000,000 lines) | P1-B input |
| `peak.sh` | peak-RSS harness (`/proc/<pid>/status` `VmHWM`; `/usr/bin/time` absent here) |
| `lockprobe.rs`, `lockprobe2.rs` (+ binaries) | P2-A / P2-B — std locking API + cross-process semantics |
| `flock.php`, `hold.php` | P2-C — PHP `flock` + bidirectional interop |
| `in.txt`, `a.out`, `b.out`, `c.php`, `c.out` | P1-A — 3-leg byte-identity of the streaming precedent |

**Repo was not modified.** No commits, no staged changes; the only writes were this file and the
scratchpad probes.

# A — `package` declaration enforcement + a file-level attribute escape hatch

Investigator: read-only review agent. Binary probed: `/home/user/phorj/target/release/phg`.
Probe corpus: `/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-pkg/`.
No repo file modified.

**Bottom line up front.** The developer's claim is *half* right, and the half that is right has a
single, precisely-locatable cause. `Core.`-reservation, PascalCase, and must-declare-a-package ARE
enforced everywhere. But `folder = package` (`E-PKG-PATH`), the loose-file-must-be-`Main` rule, and
the whole public-surface file family (`E-FILE-*`) are enforced **only for files the entry's import
graph actually reaches**. A single-file program with no user imports takes a *fast path* that returns
before the validators ever run — so `package Foo.Bar;` in a loose file is silently accepted, and
adding one `import` to the very same file makes it a hard error. This is not rot; it is the
observable consequence of the DEC-282 (2026-07-17) rewrite, whose own register entry discloses two
of the three retirements but not the fast-path interaction.

---

## Spec says

### The naming SSOT — `docs/specs/UNIFIED-SPEC.md`

**§"Naming overhaul"** is thinner on packages than the topic needs. The only package-shaped rules
there are naming-only:

- `docs/specs/UNIFIED-SPEC.md:23` — *"PascalCase packages/types, camelCase functions."*
  [Verified: read the line]
- `:288` — *"**Packages are nouns** (`Validation`, not `Validate`)."* [Verified]
- `:310` — the `Core.Console`→`Core.Output` rename table. [Verified]

There is **no** statement in §"Naming overhaul" about `package Main` for loose files, about
package↔folder mapping, or about PSR-4. Those live in two *other* spec sections:

**§"Import roots and PSR-4 mapping"** (`:481-527`) — explicitly **NOT SHIPPED**:

> `docs/specs/UNIFIED-SPEC.md:483-486`: *"**Status: DESIGNED, NOT IMPLEMENTED — and it PRE-DATES the
> unified import model: it MUST be re-based/re-adjudicated before build (audit finding B4-5; tracked
> as MASTER-PLAN W2-7) or it becomes "import redesign #5".**"* [Verified: read verbatim]

and its own model block is marked superseded:

> `:502-505`: *"**DEC-282 (2026-07-17)** shipped the real model: NO manifest at all. App root =
> walk-up to the nearest `src/`-bearing directory; three ordered search roots (entry dir → `src/` →
> `vendor/`); import-driven lazy loading; folder = package. The `[packages]` map below was never
> built — `phorj.toml` is retired entirely."* [Verified]

with the closing line that matters most here:

> `:524`: *"`package Main;` stays a reserved root at the project source root."* [Verified]

**§"Public-surface file-naming rule"** (`:529-566`) — marked **SHIPPED**, and this is where the
`E-FILE-*` codes are specified:

> `:531`: *"**Status: SHIPPED (approved 2026-06-28, hard errors; `E-FILE-*` live in the loader +
> `phg explain`).**"* [Verified]

| code | when (spec `:551-555`) |
|---|---|
| `E-FILE-NAME` | type module's stem ≠ its public type's name (incl. casing) |
| `E-FILE-MULTI-PUBLIC` | non-`main` file declares ≥2 public types |
| `E-FILE-MIXED-PUBLIC` | non-`main` file declares a public type AND a public free function |

and, load-bearing for gap A3 below:

> `:560-564`: *"`folder=path` (`E-PKG-PATH`) governs *packages*; this rule governs *the public surface
> within a package* — orthogonal axes. … Enforced in the **loader**, **project mode only**, in the
> same per-file pass as `E-PKG-PATH`."* [Verified — emphasis on "project mode only" is mine]

> *"Deferred: a per-project opt-out; applying the rule inside `package Main`; auto-rename tooling."*
> `:564-566` [Verified] — note: **a per-project opt-out was already contemplated and deferred**, which
> is directly relevant to the developer's escape-hatch question.

### The decision register — `docs/research/full-audit/raw/C-decisions.md`

The rules the developer remembers are real and all ruled. In chronological order:

- **DEC-025** (`:48`): *"User code **mandatorily packaged**, `package` never inferred — even
  `-e`/stdin one-liners write `package Main;`; reserved `package Main` = runnable entry (Go model)"*
  — status ✅. [Verified]
- **DEC-029** (`:53`): *"Directory=package, strict folder=path (`E-PKG-PATH`), enforcement
  **path-aware in the loader, never in `check()`**; flat AST merge"* — status ✅, marked ASKED.
  [Verified] — this is the ruling that put `E-PKG-PATH` in the loader, and it is *why* the fast path
  can bypass it: the checker deliberately does not own this rule.
- **DEC-035** (`:58`): *"**Casing is a HARD ERROR for all**: package/folder segments PascalCase
  (`E-PKG-CASE`) … **no `W-CASE` lint fallback**"* — status ✅. [Verified] — note explicitly: the
  developer already **rejected** a warn-only fallback for casing.
- **DEC-020** is cited in code as the `Core.` reservation ruling (`src/loader/fs.rs:91`).
  MASTER-PLAN `:2390` records: *"| Core-package reservation | loader-side enforcement ADOPTED
  (shipped, W0-4) |"*. [Verified]
- Retirement row `:304`: *"| `package main` lowercase (M5 S1) | `package Main` PascalCase reshape |
  06-23, developer |"*. [Verified]

**DEC-282** (2026-07-17) is the decisive entry. Its layout-laws clause (`:2208-2212`):

> *"(c) **Layout laws**: folder=package (E-PKG-PATH, relative to the root — src/Model/Article.phg ⇒
> `package Model;`) + file=type (E-FILE-NAME — Article.phg must contain type Article; other members
> may accompany). Function-only files: FILENAME free, folder law still binds (**"even functions must
> have a package — not in the wind"**). `package Main` = entry-only, location/name-exempt,
> UNIMPORTABLE."* [Verified]

and its BUILT clause (`:2275-2283`) discloses the retirements:

> *"RETIRED: manifest.rs, lock.rs, vendor.rs, `phg vendor` … , **loose Main-only rule (file loads;
> stdin/-e keep it)**; 11 example tomls dropped … **Eager-validation semantics change: files no
> import reaches are INERT** (the old whole-tree Core-hijack/lowercase-package rejections became
> unreachable-by-construction — tests flipped to assert inertness)."* [Verified]

**This is the answer to "we had rules!".** The loose-`Main` rule was not lost — it was
**deliberately retired for files** (kept only for stdin/`-e`) by DEC-282, and whole-tree eager
validation was **deliberately** replaced by import-reachability. Both are recorded. What is *not*
recorded anywhere is the third consequence: that the no-user-imports fast path also drops
`E-PKG-PATH` and `E-FILE-*` for the entry file itself (gaps A2/A3).

DEC-282 addendum (i) (`:2250-2257`) also fixes the root semantics:

> *"`src/` IS the root marker — walk UP from the entry file to the nearest directory containing
> `src/` (or `vendor/`) … Package names resolve UNDER `src/` (stripped — `src/Model/Article.phg` ⇒
> `package Model;`). **Entries live ANYWHERE under the app root** (bin/, xyz/, public/, root)."*
> [Verified]

### `docs/archive/specs/2026-07-24-wildcard-imports.md`

No package↔folder ruling. It reuses the loader's index (`:22-23`, `:107-108`) and adds
`E-IMPORT-UNKNOWN` / `E-IMPORT-AMBIGUOUS`. Relevant only as confirmation that
`index_packages`/`peek_package` are the shared package-surface machinery. [Verified: grepped
`package` in that file, read all 20 hits]

### `docs/plans/MASTER-PLAN.md`

- `:1559`: *"**W2-7** Import-roots PSR-4 `[packages]` map — **⚠ B4-5 gate: re-base on the unified-import
  model (S0–S2 + UA-L2) and re-adjudicate BEFORE build.**"* [Verified] — PSR-4-style *namespace
  aliasing* (decoupling package name from folder) is **queued, unbuilt, and gated on
  re-adjudication**. So the developer's "must be folder psr4 compliant" is, today, only the plain
  `folder = package` law; there is no aliasing layer yet.
- `:2390`: Core-package reservation shipped (W0-4). [Verified]

### Spec-vs-reality drift found while reading

`UNIFIED-SPEC.md:562` says the public-surface rule is *"Enforced in the loader, **project mode
only**"*. **Project mode no longer exists** — DEC-282 retired `manifest.rs`/`lock.rs`/`vendor.rs`
and routes every file through one unified loader (`src/loader/entry.rs:21-27`). The spec sentence was
never updated, so it now reads as if there were still a mode boundary that grants loose files an
exemption. Whether that exemption is *intended* is exactly the question the developer is raising.
[Verified: grepped `phorj.toml`/`manifest` in `src/` — `src/pm/` only; no `loader/manifest.rs`]

---

## Code enforces (table)

### The end-to-end trace

1. **Tokenizer** — `src/tokenizer/mod.rs:156`: byte-0 `#!` line is skipped wholesale (DEC-282
   shebang support). No `package` handling. [Verified: read `lex_inner`]
2. **Parser** — `src/parser/items/decls/items.rs:150-169` `parse_program`: reads an *optional*
   `package …;` first (`:152-156`), then items. `:172-180` `parse_package` parses a dotted path with
   **zero** validation (no casing, no reservation, no length check). `:115-119`: a `package` keyword
   found at item position → parse error *"'package' must be the first declaration, before any import
   or definition"*. [Verified: read the file]
3. **AST** — `src/ast/decls/mod.rs:97`: `Program.package: Vec<String>`, empty when absent, *"the
   checker rejects that as `E-NO-PACKAGE`"*. [Verified]
4. **Loader** — `src/loader/entry.rs` is the dispatcher; `src/loader/fs.rs` holds all four
   package/file validators; `src/loader/assemble.rs:48-50` is the **only** place three of them are
   called; `src/loader/discovery.rs:24-51` computes the roots.
5. **Checker** — `src/checker/program/walk.rs:101-132` owns `E-NO-PACKAGE`,
   `E-RESERVED-PACKAGE`, `E-PKG-CASE` — applied to `program.package`, i.e. **the merged unit's
   package**, which `src/loader/assemble.rs:151,159-163` sets to *the entry file's* package. So in a
   multi-file build the checker sees only the entry's package line; every other file's package line
   is checked by the loader (`validate_package_decl`) or not at all.

### Every validator that exists, and where it is called

| validator | file:line | callers | codes |
|---|---|---|---|
| `enforce_loose_main` | `src/loader/fs.rs:23-32` | `src/loader/entry.rs:191` (`load_loose_src` — **stdin/`-e` only**) | *(none — uncoded message)* |
| `validate_folder_path` | `src/loader/fs.rs:37-84` | `src/loader/assemble.rs:48` **only** | `E-PKG-PATH` |
| `validate_package_decl` | `src/loader/fs.rs:93-110` | `src/loader/assemble.rs:49` **only** | `E-RESERVED-PACKAGE`, `E-PKG-CASE` |
| `validate_public_surface` | `src/loader/fs.rs:117-172` | `src/loader/assemble.rs:50` **only** | `E-FILE-MULTI-PUBLIC`, `E-FILE-MIXED-PUBLIC`, `E-FILE-NAME` |
| `validate_decl_file` | `src/loader/fs.rs:178-201` | `src/loader/assemble.rs:135` | `E-DECL-PACKAGE`, `E-DECL-NONFOREIGN` |
| checker package gates | `src/checker/program/walk.rs:101-132` | every `check()` — on the **merged/entry** package only | `E-NO-PACKAGE`, `E-RESERVED-PACKAGE`, `E-PKG-CASE` |
| `E-VENDOR-MAIN` | `src/loader/assemble.rs:51-57` | inline in assemble | `E-VENDOR-MAIN` |

[Verified: `grep -rn "validate_folder_path\|validate_package_decl\|validate_public_surface\|enforce_loose_main\|validate_decl_file" --include=*.rs src/` returned exactly the callers above — 4 call sites total, all inside `src/loader/{assemble,entry}.rs`]

### THE FAST PATH — the root cause

`src/loader/entry.rs:48-66`:

```rust
fn load_unified_src(entry: &Path, entry_src: String) -> Result<Unit, String> {
    let entry_prog = parse_at(entry, &entry_src)?;
    check_unused_imports(&entry_prog, &entry_src, entry)?;
    let roots = discover_roots(entry);

    // Fast path: no user imports AND no ambient `*.d.phg` declaration files under the roots →
    // a self-contained script; skip all disk scanning. …
    let mut queue: Vec<Vec<String>> = user_imports(&entry_prog, entry)?;
    if queue.is_empty() && collect_unified_decls(&roots)?.is_empty() {
        return Ok(Unit {
            program: entry_prog,           // <-- returned RAW
            diag_src: entry_src,
            stats: None,
            …
        });
    }
    …
    assemble(entry, sources, &decl_files, Some((entry, &entry_src)))   // :164
}
```

The fast-path `return` at `:58-65` happens **before** `assemble` at `:164`, so
`validate_folder_path`, `validate_package_decl` and `validate_public_surface` never run.
`Core.*` imports do **not** count as user imports, so a normal
`import Core.Output;`-only program takes the fast path. [Verified: probe (c) vs probe (j) below —
byte-identical package/entry shape, differing only by one user `import`, opposite outcomes]

Consequence: **package-law enforcement is a function of the import graph, not of the file.** The
comment calls the fast path a pure performance optimization ("skip all disk scanning"); it is in fact
also a semantics gate.

### The entry-root bug

`src/loader/entry.rs:91-92`:

```rust
let mut sources: Vec<Source> =
    vec![Source::first_party(entry.to_path_buf(), &roots.entry_local)];
```

The entry file's folder=path root is **always** `roots.entry_local` (= `entry.parent()`), never
`src_root` — whereas imported files get the root they were *found* under
(`src/loader/entry.rs:154-158`). Since `entry_local` is by definition the entry's own directory,
`relative_under(entry, entry_local)` is always a bare filename, so `expected` is always empty
(`src/loader/fs.rs:54-63`) and any non-`Main` entry hits the `:64-72` branch:

> *"package `X` cannot sit directly in the source root — a dotted package needs a matching
> subdirectory"*

This means a **correct** file per DEC-282 addendum (i) — `src/App/Cmd/Runner.phg` declaring
`package App.Cmd;`, entry-role, folder matches — is rejected, with a message describing a different
problem. [Verified: probe (i)]

### Enforcement matrix — what IS and IS NOT enforced today

Columns: **L-fast** = loose/single file, no user imports (the common case);
**L-slow** = a file that has ≥1 user import; **Imported** = a non-entry file reached by the import
graph; **Inert** = a `.phg` on disk that no import reaches; **stdin/-e**.

| rule | code | L-fast | L-slow | Imported | Inert | stdin/`-e` |
|---|---|---|---|---|---|---|
| must declare a package | `E-NO-PACKAGE` | ✅ | ✅ | ✅ | ❌ | ✅ |
| `Core.` root reserved | `E-RESERVED-PACKAGE` | ✅ *(checker)* | ✅ | ✅ *(loader)* | ❌ | ✅ |
| segments PascalCase | `E-PKG-CASE` | ✅ *(checker)* | ✅ | ✅ *(loader)* | ❌ | ✅ |
| loose file ⇒ `package Main` | *(uncoded)* | **❌** | ⚠ accidental¹ | n/a | ❌ | ✅ *(uncoded)* |
| `folder = package` | `E-PKG-PATH` | **❌** | ⚠ wrong-root¹ | ✅ | ❌ | n/a |
| one public type / stem match | `E-FILE-NAME` | **❌** | ✅ | ✅ | ❌ | n/a |
| ≤1 public type | `E-FILE-MULTI-PUBLIC` | **❌** | ✅ | ✅ | ❌ | n/a |
| no type+function mix | `E-FILE-MIXED-PUBLIC` | **❌** | ✅ | ✅ | ❌ | n/a |
| vendored ⇏ `Main` | `E-VENDOR-MAIN` | n/a | n/a | ✅ | ❌ | n/a |
| PSR-4 namespace *aliasing* | `E-PKG-ROOT-*` | ❌ | ❌ | ❌ | ❌ | ❌ (unbuilt, W2-7) |

¹ For an entry file the check runs against the wrong root, so it always reports "cannot sit directly
in the source root" rather than the real rule — see the entry-root bug above.

[All rows Verified by probe transcripts below, except `E-VENDOR-MAIN` (Verified by reading
`src/loader/assemble.rs:51-57`) and the PSR-4 row (Verified by `MASTER-PLAN.md:1559` + grep: no
`E-PKG-ROOT` string anywhere in `src/`)]

### What the test suite pins (and does not)

- `src/loader/tests/loose.rs:20-23` `loose_non_main_is_rejected` — calls **`load_loose_src`**, i.e.
  the stdin/`-e` path only. There is **no test** asserting anything about a loose *file* with a
  non-`Main` package. [Verified: read the whole 38-line file]
- `src/loader/tests/project_structure.rs:59-90` pins `E-PKG-PATH` for a mismatched **imported**
  file and for a non-`Main` file directly in the source root — both go through `assemble`.
  [Verified: read `:1-90`]
- `src/loader/tests/public_surface.rs:19` pins `E-FILE-NAME` — again via a project layout.
  [Verified: grepped]

So the fast-path hole is not merely unenforced, it is **untested in either direction** — nothing
pins the current permissive behaviour either, which lowers the risk of closing it.

### The stale success message

`src/loader/unit.rs:30-41`:

```rust
"OK — whole project type-checks clean: {} file{}, {} package{}, {} definition{} \
 validated (every file + vendored deps)\n"
```

The literal `(every file + vendored deps)` is a **false promise** after DEC-282 made loading
import-driven and lazy. Probe (h) shows it printing `2 files … validated (every file + vendored
deps)` for a tree containing **3** `.phg` files, one of which carries three simultaneous violations.
[Verified: read `unit.rs`; probe (h)]

### Blast radius of any fix (single chokepoint — good news)

`grep -rn "loader::load\|load_loose_src\|load_with_buffer" --include=*.rs src/ | grep -v '^src/loader/'`
[Verified]:

| surface | call site | path taken |
|---|---|---|
| `phg run` / `check` / `transpile` / `tokenize` / … | `src/main.rs:524` | `loader::load` → `load_unified_src` |
| `phg run -` / `-e` | `src/main.rs:525-526` | `load_loose_src` |
| `phg build --php` | `src/main.rs:207` | `loader::load` |
| `src/main.rs:308,441` (other subcommands) | | `loader::load` |
| `phg test` | `src/cli/test_runner.rs:77` | `loader::load` |
| **LSP** | `src/lsp/mod.rs:488` | `load_with_buffer` → `load_unified_src` |
| DAP | `src/dap.rs:387` | `load_loose_src` |

Every file-based surface funnels through `load_unified_src`. A fix there lands `check` ≡ LSP ≡ `run`
≡ `transpile` ≡ `build` ≡ `test` **simultaneously**, satisfying Invariant 17 / DEC-252 by
construction. Conversely: the LSP has *exactly* the same hole today, so `phg check` ≡ LSP currently
holds (both equally permissive) — no DEC-252 violation exists right now, and a fix must keep them
paired.

### Migration cost of tightening ≈ zero

Scanned every non-`target`/`.git`/`var` `.phg` in the repo (shebang + comment-skipping package peek,
mirroring `src/loader/discovery.rs:58-77`), comparing the declared package to the trailing path
components:

```
non-Main package files: 31; folder-matching: 30; NOT matching folder: 1
   examples/package-manager/greet-src/greet.phg -> Acme.Greet
```

[Verified: ran the python scan; output pasted verbatim]

That single outlier is a *fixture*: `examples/package-manager/README.md:14` documents it as
*"greet-src/greet.phg  # the dependency's SOURCE (what a publisher writes)"*, copied into
`vendor/Acme/Greet/` by `phg add`. `greet-src/` is not one of the three search roots, so it is inert
by design. [Verified: read the README line + `find examples/package-manager`]

---

## Probe transcripts

All run with `/home/user/phorj/target/release/phg`. Every probe body is the same minimal
`#[Entry(kind: EntryKind.Cli)] function main(): int` program; only the `package` line and location
vary.

### (a) loose file, no `package` at all — **ENFORCED**

```
$ phg check a_nopkg.phg
type error at 1:1: every file must declare a package (e.g. `package Main;`) as its first line
import Core.Output;
^
  [E-NO-PACKAGE]
  hint: add `package Main; import Core.Runtime.Entry; import Core.Runtime.EntryKind;` at the top of the file
exit=1
```
`phg run` identical, exit=1.

### (b) loose file, `package Main;` — **ACCEPTED (correct)**

```
$ phg check b_main.phg
OK (type-checks clean)
exit=0
$ phg run b_main.phg
b
exit=0
```

### (c) loose file, `package Foo.Bar;` — **ACCEPTED — THE GAP**

```
$ cat c_foobar.phg
package Foo.Bar;

import Core.Output;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;

#[Entry(kind: EntryKind.Cli)]
function main(): int {
    Output.printLine("c");
    return 0;
}

$ phg check c_foobar.phg
OK (type-checks clean)
exit=0
$ phg run c_foobar.phg
c
exit=0
```

No error. No warning. This is the developer's primary complaint, reproduced exactly.

### (c′) same file, transpiled — the package is **semantically discarded**

```
$ phg transpile c_foobar.phg
<?php
function main(): int {
    echo "c", "\n";
    return 0;
}
exit(main());
```

**No `namespace Foo\Bar` is emitted at all.** Compare the assemble path, probe (d′) below, which
emits real namespaces. On the fast path `package Foo.Bar;` is inert decoration.

### (d) project file whose package matches its folder — **ACCEPTED (correct)**

Layout: `d_match/src/main.phg` (`package Main;`, `import Acme.Geometry.helper;`) +
`d_match/src/Acme/Geometry/Lib.phg` (`package Acme.Geometry;`).

```
$ cd d_match && phg run src/main.phg
from Acme.Geometry
exit=0
```

### (d′) same project, transpiled — namespaces ARE emitted

```
$ cd d_match && phg transpile src/main.phg
<?php
namespace Acme\Geometry {
    use function \Main\main;
    function helper(): string {
        return "from Acme.Geometry";
    }
}
namespace Main {
    function main(): int {
        echo \Acme\Geometry\helper(), "\n";
        return 0;
    }
}
namespace {
    exit(\Main\main());
}
```

### (e) project file whose package MISMATCHES its folder — **ENFORCED**

`e_mismatch/src/Wrong/Place/Lib.phg` declares `package Acme.Geometry;`; main imports
`Acme.Geometry.helper`.

```
$ cd e_mismatch && phg run src/main.phg
…/e_mismatch/src/Wrong/Place/Lib.phg: package `Acme.Geometry` does not match its location — directory `Wrong/Place` implies `package Wrong.Place;` (folder = path) [E-PKG-PATH]
exit=1
```

Note the resolution order: `index_packages` (`src/loader/discovery.rs:82-99`) indexes by the
**declared** package, so the import resolves first and `validate_folder_path` catches it after.

### (f) project file with lowercase package — **ENFORCED**

`f_projlower/src/acme/geo/Lib.phg` declares `package acme.geo;`.

```
$ cd f_projlower && phg run src/main.phg
…/f_projlower/src/acme/geo/Lib.phg: package segment `acme` must be PascalCase [E-PKG-CASE]
exit=1
```

### (f′) loose file, lowercase `package foo;` — **ENFORCED (by the checker)**

```
$ phg check f_lower.phg
type error at 1:1: package segment `foo` must be PascalCase
package foo;
^
  [E-PKG-CASE]
  hint: did you mean `package Foo`?
exit=1
```

### (g) loose file, `package Core.Evil;` — **ENFORCED (by the checker)**

```
$ phg check g_core.phg
type error at 1:1: `Core` is a reserved package root (the standard library)
package Core.Evil;
^
  [E-RESERVED-PACKAGE]
  hint: use a different root, e.g. `package App;`
exit=1
```

So `Core.` reservation and PascalCase **are** enforced even on the fast path — because they live in
the *checker* (`src/checker/program/walk.rs:110-132`) and run against the entry's own
`program.package`. Only the *loader-owned* rules are bypassed.

### (h) un-imported file with THREE violations — **INERT, and the summary lies**

`h_inert/` = a working `d_match` copy plus `src/Totally/Wrong/Junk.phg` containing
`package Core.Hijack.lowercase;` (Core hijack **+** lowercase segment **+** folder mismatch), which
nothing imports.

```
$ cd h_inert && phg run src/main.phg
from Acme.Geometry
exit=0

$ cd h_inert && phg check src/main.phg
OK — whole project type-checks clean: 2 files, 2 packages, 2 definitions validated (every file + vendored deps)
exit=0
```

Three files on disk; the message claims *"every file"*. (Inertness itself is DEC-282-disclosed and
arguably fine; the **message** is not.)

Checking that same file **directly** does catch the checker-owned pair (but still not the folder law):

```
$ cd h_inert && phg check src/Totally/Wrong/Junk.phg
type error at 1:1: `Core` is a reserved package root (the standard library)  [E-RESERVED-PACKAGE]
type error at 1:1: package segment `lowercase` must be PascalCase            [E-PKG-CASE]
exit=1
```

### (h′) can a `Core.` hijack be reached by import? — **NO, structurally**

`m_coreimport/src/Evil/J.phg` declares `package Core.Evil;`; main writes `import Core.Evil.junk;`.

```
$ cd m_coreimport && phg run src/main.phg
type error at 9:45: unknown function `junk`
exit=1
```

`Core.*` is resolved at step 0 without touching disk, so a user file claiming a `Core.` package can
never be imported. The hijack is unreachable-by-construction — matching DEC-282's disclosure.

### (i) non-`Main` entry inside a project, folder CORRECT — **WRONGLY REJECTED**

`i_pkgentry/src/App/Cmd/Runner.phg` declares `package App.Cmd;` and lives in `src/App/Cmd/` — a
correct layout per DEC-282 addendum (i).

```
$ cd i_pkgentry && phg run src/App/Cmd/Runner.phg
…/i_pkgentry/src/App/Cmd/Runner.phg: package `App.Cmd` cannot sit directly in the source root — a dotted package needs a matching subdirectory (expected under `App/Cmd/`) [E-PKG-PATH]
exit=1
```

The message is self-contradicting (`App/Cmd/` *is* the directory it is in). Cause: the entry-root bug
at `src/loader/entry.rs:91-92`. Net effect: "an entry must be `package Main`" **is** enforced — but
by accident, with an incorrect diagnostic, and only when the entry is on the slow path.

### (j) the fast/slow proof — same shape, one extra import, opposite verdict

`j_forced/Runner.phg` = probe (c) verbatim **plus** `import Helpers.helper;`, with
`j_forced/Helpers/H.phg` declaring `package Helpers;`.

```
$ cd j_forced && phg run Runner.phg
…/j_forced/Runner.phg: package `Foo.Bar` cannot sit directly in the source root — a dotted package needs a matching subdirectory (expected under `Foo/Bar/`) [E-PKG-PATH]
exit=1
```

**Probe (c) exit 0, probe (j) exit 1, differing by one `import` line.** This is the decisive evidence
that enforcement is import-graph-dependent.

### (k) public-surface rules on a loose file — **NOT ENFORCED**

`k_surface/whatever.phg`: `package Foo.Bar;` + `public class Alpha` + `public class Beta` — two
public types (should be `E-FILE-MULTI-PUBLIC`) in a file whose stem matches neither (should be
`E-FILE-NAME`), and no entry to grant the exemption.

```
$ phg check whatever.phg
OK (type-checks clean)
exit=0

$ phg transpile whatever.phg
<?php
final class Alpha {
    function __construct(public int $a) {}
}
final class Beta {
    function __construct(public int $b) {}
}
```

The spec calls these SHIPPED hard errors (`UNIFIED-SPEC.md:531`). They are — on the slow path only.

### (l) stdin / `-e` — **ENFORCED, but with no error code**

```
$ cat c_foobar.phg | phg run -
package `Foo.Bar` cannot run from stdin/-e; only `package Main` runs there (save it as a file — packages resolve against the entry file's directory)
exit=1

$ phg run -e "$(cat c_foobar.phg)"
package `Foo.Bar` cannot run from stdin/-e; only `package Main` runs there (save it as a file — packages resolve against the entry file's directory)
exit=1

$ cat b_main.phg | phg run -
b
exit=0
```

No `[E-…]` bracket — the only rule in this whole area with no code, so `phg explain` cannot cover it
and no tooling can key on it. [Verified: `src/loader/fs.rs:27-31` builds the string with no
`with_code`; the surrounding validators all carry codes]

---

## Attribute-escape-hatch grammar reality

### How `#[Entry]` unreserved `main` (the precedent the developer is reasoning from)

**DEC-337** (`C-decisions.md:3289-3320`) [Verified: read verbatim]:

> *"**Problem.** `#[Entry(kind: Cli)]` (DEC-331) read `Cli`/`Web` as a BARE magic identifier —
> string-matched in `parse_entry_kind`, never imported, never resolved. This violated the "nothing in
> the wind" invariant … Flagged by the developer."*
>
> *"**Ruling.** The kind is an injected enum `Core.Runtime.EntryKind` { Cli, Web, Desktop, Mobile,
> Worker, Embedded }, reached QUALIFIED — `#[Entry(kind: EntryKind.Cli)]`. … (1) **separate import**
> `import Core.Runtime.EntryKind;`; (2) **reserved kinds are real variants**"*
>
> *"**Compile-time only (Inv 5).** `EntryKind` is a pure marker bare_type under `Core.Runtime` (empty
> prelude source) — never a runtime enum; the attribute arg is erased before any backend, so the PHP
> leg never sees it."*

The mechanism worth transplanting: **an attribute + a real import to gate it + full erasure before
any backend.** `#[Entry]` made `main` an ordinary name by moving entry-ness from *the name* to *an
explicit, import-gated, compile-time-only marker*. Two accepted spellings: short `EntryKind.Cli`
(member-import-gated) and self-gating fully-qualified `Core.Runtime.EntryKind.Cli`. Enforcement is in
`check_entry_points` (`src/checker/program/walk.rs:196`).

### Does the parser support ANY file-level / inner attribute today? — **NO**

`parse_attributes` (`src/parser/items/decls/functions.rs:63-101`) is called from exactly two places
[Verified: grepped]:

- `src/parser/items/decls/items.rs:14` — start of `parse_item` (top-level items)
- `src/parser/items/types/members.rs:13` — start of `parse_member` (class members)

`parse_program` (`src/parser/items/decls/items.rs:150-169`) reads `package` **first** and never calls
`parse_attributes`. There is **no file-level, inner, or module-level attribute grammar of any kind**.

Allowed targets are further narrowed:
- `src/parser/items/decls/items.rs:71-85` — top level: `function` or `class` **only**, else
  `E-ATTR-TARGET` (*"attributes (`#[…]`) are only allowed on a top-level `function` or `class`"*).
  Enum / interface / trait / import / type alias are all rejected *"until their target slices land"*.
- `src/parser/items/types/members.rs:15-24` — members: methods only.
- `src/parser/items/decls/items.rs:28-37` — never on a foreign `declare`.

### Probed: all four candidate spellings fail today

```
$ head -2 inner_bang.phg
#![Package(Main)]
                        <- blank line
$ phg check inner_bang.phg
type error at 3:1: every file must declare a package (e.g. `package Main;`) as its first line
import Core.Output;
^
  [E-NO-PACKAGE]
```

**⚠ The `#![…]` line was SILENTLY SWALLOWED.** `src/tokenizer/mod.rs:156`:

```rust
if lx.pos == 0 && src.starts_with("#!") {
    while let Some(b) = lx.peek() { if b == b'\n' { break; } lx.bump(); }
}
```

DEC-282's byte-0 shebang skip eats **any** first line starting with `#!`, including `#![…]`. So a
Rust-style inner attribute is not merely unimplemented — it currently **collides with a shipped
feature and vanishes without a diagnostic**. Any `#![…]` design must first narrow the shebang rule
(e.g. `#!` **not** followed by `[`; real shebangs always continue with `/` or whitespace, so the
disambiguation is safe and one-token cheap). [Verified: probe + read `lex_inner`]

```
$ phg check attr_before_pkg.phg          # #[File(loose)] then package Foo.Bar;
parse error at 1:1: attributes (`#[…]`) are only allowed on a top-level `function` or `class`
#[File(loose)]
^
  [E-ATTR-TARGET]
  hint: place the `#[…]` attribute directly above a top-level `function` or `class`
```

```
$ phg check attr_on_pkg.phg              # package Foo.Bar #[loose];
parse error at 1:17: expected ';' after package, found HashBracket
package Foo.Bar #[loose];
                ^
```

```
$ phg check attr_before_pkg2.phg         # #[Loose] then package Foo.Bar;
parse error at 1:1: attributes (`#[…]`) are only allowed on a top-level `function` or `class`
  [E-ATTR-TARGET]
```

Note the failure *shape* of the prefix-before-`package` forms: the attribute parses fine, then
`parse_program` has already recorded `package = []` (the `#[` token is not `Package`), and
`parse_item` rejects at the target check. So enabling it is a **local** change — teach
`parse_program` to `parse_attributes()` before the `package` peek and thread the result onto
`Program` — not a grammar redesign. `package` staying "the first declaration"
(`items.rs:115-119`) is unaffected: an attribute is a *modifier on* the declaration, not a
declaration.

### `phorj.json` is NOT read by the compiler

[Verified: `grep -rn "phorj.json" --include=*.rs src/`] — every hit is in `src/pm/`
(`src/pm/mod.rs:32 MANIFEST_FILE`, `manifest.rs`, `ops.rs`, `json.rs`) plus unrelated
`__phorj_json_*` transpiler helpers. The loader/checker **never** open it. A `phorj.json`-driven
opt-out would therefore create a brand-new compiler↔manifest coupling that directly contradicts
DEC-282's central ruling (*"NO manifest at all"*, `C-decisions.md:502`) and DEC-282 addendum (i)
(*"No marker file, no config"*, `:2253`) — and would re-open the standing
*"developer explicitly dislikes the phorj.toml idea"* re-adjudication (`:2284-2292`).

---

## Cross-language scan (Invariant 16 / META-7)

| language | package/namespace ↔ directory coupling | enforcement | consequence |
|---|---|---|---|
| **Go** | Import path ≈ directory; **package *name* need not equal the directory name** — all files in one dir must share one package clause, but the name is convention only | compiler enforces *one package per directory*; name↔dir is lint/convention (`revive`, `staticcheck`) | Tools must read the file to learn a package's name; `gopls` has open bugs about exactly this. [Verified: golang-nuts + go issue #70755 — *"Package name is the same as the last element of its import path. It is **not a rule** but a best practice"*; *"the package name is only related to them by convention"*] |
| **Java** | package ↔ directory **hard-coupled** by the spec's default file-system mapping | `javac` hard error for a public type in the wrong directory / misnamed file | Zero ambiguity, high ceremony; deep packages ⇒ deep empty dir chains. [Inferred: universally-known `javac` behaviour; not probed this session] |
| **Kotlin** | **DECOUPLED by design** — the docs state a file "may start with a package header" and say **nothing** about directory matching | none in the compiler; IDE offers a *"package directive does not match file location"* inspection (a hint, fixable both ways) | Deliberate relaxation of Java's rule, explicitly to reduce ceremony; convention survives via tooling. [Verified: fetched kotlinlang.org/docs/packages.html — no directory requirement stated anywhere] |
| **C#** | **DECOUPLED**; `namespace` is free of folders | **analyzer** `IDE0130` *"Namespace does not match folder structure"* — configurable severity, `dotnet_diagnostic.IDE0130.severity = none`, `#pragma warning disable IDE0130` | The canonical "warn, don't fail, and let me turn it off" model. Note the real-world tax: multiple Roslyn bugs (#55014, #55550, #73261, #74758, #75169) about false positives and "cannot be disabled". [Verified: MS Learn IDE0130 + the linked roslyn issues] |
| **Rust** | Module tree ↔ files coupled **by default**, with a **first-class escape hatch**: `#[path = "…"] mod foo;` overrides the file a module loads from | compiler error only when the default path is absent *and* no `#[path]` given | The precedent closest to the developer's instinct: strict default + an explicit, per-item, compile-time attribute opt-out. [Inferred: standard documented `#[path]` semantics; not probed this session] |
| **Swift** | **No packages-as-directories at all** — one module per build target; files are flat within a target, `import` names targets | n/a | Sidesteps the question entirely; loses per-directory namespacing. [Inferred] |
| **TypeScript** | Module identity **is** the file path (`import './a/b'`); no separate namespace declaration in modern usage | resolver error if the path is wrong | The path *is* the name — nothing to keep in sync, but no logical/physical decoupling either. [Inferred] |
| **PHP** | PSR-4 maps a namespace **prefix** to a base directory via `composer.json`; the language itself enforces **nothing** | autoloader fails to find the class at runtime; `composer dump-autoload -o` warns | Enforcement is a *tooling* concern; the language permits any namespace in any file. Phorj's transpile target. [Inferred: PSR-4 + composer behaviour] |

**The consequential reading for phorj.** Two independent axes are conflated in the developer's
framing:

1. *Is the package name derivable from the location?* Java/Go-ish/phorj: yes. Kotlin/C#/PHP: no.
2. *If it isn't, is that an error, a warning, or nothing?* Java: error. C#: configurable warning.
   Kotlin: IDE hint. PHP: nothing.

Kotlin and C# both started from a strict-ish position and **deliberately relaxed to a
warning-or-hint**, and both report the same reason: the strict rule taxes small/experimental files
and generated code far more than it protects. Rust took the third road: **strict by default, with an
explicit compile-time attribute escape hatch per item** (`#[path]`) — which is structurally the
closest analogue to what the developer is proposing, and it has held up well in practice.

Also worth flagging as a correction: **Go does not enforce what the developer's "Go packages" mental
model implies.** Go enforces *one package clause per directory*, not *package name == directory
name*. Phorj's `folder = package` law is therefore **stricter than Go's**, and closer to Java's.
[Verified: the Go sources above]

---

## Gaps

### A1 — A loose file may declare any PascalCase non-`Main` package, silently — **SEVERITY: HIGH**
`package Foo.Bar;` in a single file with no user imports type-checks clean, runs, and transpiles.
No error, no warning. This is the developer's primary complaint, reproduced exactly.
**Evidence:** probe (c) — `phg check c_foobar.phg` → `OK (type-checks clean)`, exit 0.
[Verified: probe transcript]
*Provenance:* DEC-282 BUILT explicitly retired this — *"RETIRED: … loose Main-only rule (file loads;
stdin/-e keep it)"* (`C-decisions.md:2277`). So it is a **ruled** retirement whose consequence the
developer now wants revisited — not a regression. [Verified]

### A2 — Package-law enforcement is import-graph-dependent (the fast path) — **SEVERITY: HIGH**
`src/loader/entry.rs:53-66` returns the raw entry program before `assemble` (`:164`), so the three
loader validators at `src/loader/assemble.rs:48-50` never run for a no-user-imports entry. Adding a
single `import` flips a file from accepted to hard error.
**Evidence:** probe (c) exit 0 vs probe (j) exit 1 — identical package + entry shape, one extra
`import` line. [Verified: both transcripts]
This is the **root cause** of A1, A3 and A4, and the fast path's own comment describes it only as a
performance optimization ("skip all disk scanning") — the semantic side effect is undocumented
anywhere in the repo. [Verified: read the comment + grepped the register for a matching disclosure;
DEC-282 discloses inertness and the loose-Main retirement, not this]

### A3 — The public-surface file rules are bypassed on the same path — **SEVERITY: HIGH**
`E-FILE-NAME`, `E-FILE-MULTI-PUBLIC`, `E-FILE-MIXED-PUBLIC` are specced as SHIPPED hard errors
(`UNIFIED-SPEC.md:531`) but never fire for a fast-path file.
**Evidence:** probe (k) — two public classes in `whatever.phg`, no entry ⇒ should be two distinct
errors; got `OK (type-checks clean)`. [Verified]
Aggravating doc drift: the spec says *"Enforced in the loader, **project mode only**"*
(`UNIFIED-SPEC.md:562`), and project mode was retired by DEC-282 — so the spec now describes a mode
boundary that no longer exists. [Verified]

### A4 — `package` is semantically inert on the fast path (no PHP namespace emitted) — **SEVERITY: MEDIUM**
The same source transpiles to namespaced PHP on the slow path and to **global-namespace** PHP on the
fast path, because mangling happens inside `assemble` only.
**Evidence:** probe (c′) — `package Foo.Bar;` → `<?php function main(): int {…}` with no `namespace`;
probe (d′) — `namespace Acme\Geometry { … }`. [Verified: both transcripts]
Not a byte-identity spine break (all three legs agree *within* each path, and a self-contained file
has no external referents), but it means the accepted-in-A1 package declaration is pure decoration —
which strengthens the case that accepting it is the bug.

### A5 — `phg check`'s success line claims "every file" while validating only reached files — **SEVERITY: MEDIUM**
`src/loader/unit.rs:32-33` hardcodes `validated (every file + vendored deps)`. Post-DEC-282 loading
is lazy, so this is a false promise in user-facing output.
**Evidence:** probe (h) — 3 `.phg` on disk (one with a `Core.` hijack **+** a lowercase segment **+**
a folder mismatch), output `OK — whole project type-checks clean: 2 files, 2 packages, 2 definitions
validated (every file + vendored deps)`. [Verified]
Inertness itself is DEC-282-disclosed and defensible; the *message* contradicts it.

### A6 — A correct non-`Main` entry is rejected with the wrong diagnostic — **SEVERITY: MEDIUM**
`src/loader/entry.rs:91-92` always uses `roots.entry_local` as the entry's folder=path root, never
`src_root`. Since `entry_local == entry.parent()`, `expected` is always empty
(`src/loader/fs.rs:54-63`) and every non-`Main` entry hits the *"cannot sit directly in the source
root"* branch — even when its folder is exactly right.
**Evidence:** probe (i) — `src/App/Cmd/Runner.phg` declaring `package App.Cmd;` →
*"cannot sit directly in the source root — … (expected under `App/Cmd/`)"* while sitting in
`App/Cmd/`. [Verified]
Net effect: *"an entry must be `package Main`"* is enforced **by accident** with a self-contradicting
message. This is load-bearing for any fix to A1/A2: **closing the fast path without fixing A6 first
would start rejecting correct non-`Main` entries with this same wrong message.** Also note DEC-282's
*"`package Main` = entry-only, location/name-exempt"* (`:2211`) and
`src/cli/explain/imports_casts.rs:106-108` (*"`Main` is the ENTRY package: location-free, name-free,
and unimportable"*) are ambiguous about whether an entry **must** be `Main` — that is itself an
unruled question. [Verified: read both]

### A7 — The loose-`Main` rule is the only rule here with no error code — **SEVERITY: LOW**
`src/loader/fs.rs:27-31` returns a bare string; probe (l) confirms no `[E-…]` bracket. `phg explain`
cannot cover it and no tooling can key on it, unlike every sibling rule.
[Verified: probe (l) + read the function]

### A8 — No file-level attribute grammar exists (answers the developer's question) — **INFORMATIONAL**
`parse_attributes` is reachable only from `parse_item` and `parse_member`; `parse_program` reads
`package` first and never calls it; top-level attributes are restricted to `function`/`class` by
`E-ATTR-TARGET`.
**Evidence:** `src/parser/items/decls/{items.rs:14,71-85,150-169}`, `functions.rs:63-101`,
`types/members.rs:13`; probes `attr_before_pkg`, `attr_before_pkg2`, `attr_on_pkg`. [Verified]

### A9 — `#![…]` at byte 0 is silently eaten by the DEC-282 shebang skip — **SEVERITY: HIGH (as a design constraint)**
`src/tokenizer/mod.rs:156` skips **any** byte-0 line starting with `#!`. A Rust-style inner attribute
therefore disappears with no diagnostic.
**Evidence:** probe `inner_bang.phg` — `#![Package(Main)]` on line 1 produced `E-NO-PACKAGE` at
**3:1**, i.e. the line was consumed as a shebang. [Verified]
This does not block a `#![…]` design, but it makes one strictly more expensive: the shebang rule must
first be narrowed (e.g. `#!` not followed by `[`). Any option list that omits this is misleading.

### A10 — Spec/register drift in this area — **SEVERITY: LOW (docs)**
(a) `UNIFIED-SPEC.md:562` *"project mode only"* — that mode no longer exists (DEC-282).
(b) `UNIFIED-SPEC.md §"Naming overhaul"` is cited by CLAUDE.md Inv 12 as the naming SSOT for
`package`, but contains no package↔folder rule at all — those live in two other sections and the
register. A reader following the SSOT pointer finds nothing.
(c) `UNIFIED-SPEC.md:564` already records *"Deferred: a per-project opt-out"* for the public-surface
rule — the escape hatch the developer is now asking about was contemplated and shelved, and no
register row supersedes that deferral. [Verified: read all three]

### A11 — Tightening is nearly free; the current permissiveness is untested — **SEVERITY: INFORMATIONAL (de-risks any fix)**
Repo scan: 31 non-`Main`-package `.phg` files, **30 already folder-matching**, 1 outlier
(`examples/package-manager/greet-src/greet.phg`, a documented publisher-source fixture that is inert
by design). And `src/loader/tests/loose.rs` only tests `load_loose_src` (stdin/`-e`) — **no test pins
the permissive file behaviour in either direction**.
[Verified: python scan output pasted above; read `loose.rs` in full]

---

## Options & recommendation per gap

> **Invariant 15 (ADJUDICATION RULE) applies to everything below.** All of it is user-visible
> language/design surface. These are options plus one recommendation each, recorded as **PENDING** —
> nothing here is ruled. Where a change is purely a bug fix with no user-visible design content, that
> is stated explicitly.

### For A2 (and therefore A1, A3, A4) — the fast-path bypass

- **Option 1 — Validate the entry before the fast-path return (keep the perf win).** Run
  `validate_package_decl` + `validate_folder_path` + `validate_public_surface` on `entry_prog` at
  `src/loader/entry.rs:56` (before the `:58` return), against the *correct* root (see A6). The disk
  scan the fast path exists to avoid is not needed by any of the three validators — they take
  `(prog, file, root)` only — so the optimization survives intact.
  *Failure mode:* changes what compiles. Mitigated by A11 (1 repo file affected, and it is inert).
  *Reach:* one chokepoint fixes `run`/`check`/`transpile`/`build`/`test`/LSP together (Inv 17).
- **Option 2 — Warn instead of fail on the loose case** (`W-PKG-LOOSE` / `W-PKG-PATH`), keeping hard
  errors for imported files. The Kotlin/C# road.
  *Failure mode:* DEC-035 already **rejected** warn-only for the sibling casing rule (*"no `W-CASE`
  lint fallback"*, `C-decisions.md:58`) — adopting warnings here would split the doctrine, and
  Roslyn's IDE0130 bug trail is a live example of the ongoing cost. Also warnings on stdout/stderr
  interact with the byte-identity spine and would need a disclosure.
- **Option 3 — Hard-fail only the narrow loose case** (loose entry + non-`Main` package ⇒ error),
  leaving `E-FILE-*` bypassed on the fast path. Smaller blast radius, but leaves A3/A4 open and
  keeps the same file/import-graph inconsistency in a smaller form.
- **Option 4 — Ratify the status quo:** record that a fast-path file is exempt by design, and fix
  only A5's message. Cheapest; but it makes `E-FILE-*`'s "SHIPPED hard errors" status false and
  leaves A4's inert-`package` oddity in place.

**Recommendation: Option 1**, and A6 fixed in the same change (below). *Why:* it is the only option
that makes the rule a property of the *file* rather than of the *import graph*, which is what makes
the current behaviour surprising; it restores three already-ruled rules (DEC-029, DEC-282 layout
laws, the public-surface spec) without inventing any new policy; the migration cost is one inert
fixture (A11); it needs no new error codes or messages; and it lands every surface at once through a
single chokepoint. It does **not** by itself decide the loose-`Main` question — see the next block.

### For A1 specifically — what should a loose non-`Main` file do?

Note the two sub-questions are separable and Option 1 above forces the first one:

1. **Should `package Foo.Bar;` be legal in a file that is not under a matching folder?** Under Option
   1 the answer becomes "no" automatically (folder=path binds), i.e. DEC-282's loose-Main retirement
   is effectively reversed for *files*. If the developer wants the retirement kept, Option 1 must be
   scoped to `E-FILE-*` + `E-PKG-CASE` only.
2. **Must an *entry* be `package Main`?** Currently enforced by accident (A6). DEC-282 says
   *"`package Main` = entry-only"* — ambiguous between "only entries may be `Main`" and "entries must
   be `Main`". This needs an explicit ruling either way, and if the answer is "must", it deserves its
   own code (e.g. `E-ENTRY-PACKAGE`) rather than a misdirected `E-PKG-PATH`.

**Recommendation:** rule (1) as *hard error, folder=path binds for every file* (consistent with
DEC-029/DEC-035's no-warnings doctrine and with the "nothing in the wind" invariant DEC-282 itself
invokes), and rule (2) explicitly as *an entry must be `package Main` unless it also satisfies
folder=path*, with a dedicated code. Both are PENDING developer rulings.

### For A6 — the entry-root bug

Two distinct defects: (a) the wrong root is passed; (b) the message misdescribes the situation.
- **Option 1 — Pass the right root:** compute the entry's root as `src_root` when the entry is under
  it, else `entry_local`. This is a **pure bug fix**, no design content: it makes probe (i) pass,
  which DEC-282 addendum (i) says it should (*"Entries live ANYWHERE under the app root"*,
  *"Package names resolve UNDER `src/`"*).
- **Option 2 — Exempt entry-role files from folder=path entirely** (any `#[Entry]` file is
  location-free, extending DEC-282's `Main`-exemption to all entries). Design content: changes what
  `package Main = entry-only` means.
- **Option 3 — Make it an explicit rule:** `E-ENTRY-PACKAGE` — "an entry must declare `package
  Main`" — replacing the misdirected `E-PKG-PATH`.

**Recommendation: Option 1 (the root fix) unconditionally — it is a bug, not a policy — plus Option 3
for whichever policy A1(2) rules.** Option 1 must land *before or with* any A2 fix, or closing the
fast path will start emitting the wrong message for correct layouts.

### For A5 — the "every file" claim

- **Option 1 — Tell the truth:** `… validated (N of M .phg files reached by the import graph;
  un-imported files are not compiled)`. Requires counting on-disk `.phg` (the index already walks
  them — `index_packages`, `src/loader/discovery.rs:82-99`).
- **Option 2 — Drop the parenthetical** entirely.
- **Option 3 — Add a `--strict`/`phg check --all` mode** that eagerly validates every `.phg` under
  the roots, and keep the current message only for that mode.

**Recommendation: Option 1 now** (a one-line honesty fix to a false promise; no design content), with
Option 3 recorded as a separate QUEUED idea — an eager sweep is genuinely useful for CI but is a new
user-visible surface and needs its own ruling.

### For A7 — the uncoded loose-`Main` error

Assign a code (`E-LOOSE-PACKAGE` or reuse `E-PKG-PATH`) and add a `phg explain` entry. **Pure
consistency fix**, no design content — every sibling rule in this area carries a code, and
`src/cli/tests/explain_coverage.rs` is the existing pattern that would pin it.

### For A8/A9 — the file-level attribute escape hatch

The developer's framing — *"since we made the Entry to unreserve the main free function … maybe we
should do the same thing for the package! to work free of structure! we need an attribute for the
file! that the file starts with??"* — has a **hidden sub-question that must be answered first**,
because the options differ by it:

> **What does the marker exempt?** (a) *this file's `package` is exempt from folder=path* (keep the
> dotted name, drop the location law) — or (b) *this file is structure-free entirely* (also exempt
> from `E-FILE-*`, and/or treated as an entry regardless of location)? These are different features
> with different blast radii and should not be bundled by default.

Syntax options, each with the A9 constraint made explicit:

| # | syntax | grammar cost | trade-offs |
|---|---|---|---|
| **1** | `#[Loose] package Foo.Bar;` — prefix attribute on the `package` declaration | Teach `parse_program` to `parse_attributes()` before the `package` peek; thread onto `Program`; add a `package` target to the `E-ATTR-TARGET` allow-list. **No tokenizer change.** | Matches the existing `#[Entry]` prefix-attribute shape exactly; reads as a modifier on the thing it modifies; scoped to the one declaration it affects, so it cannot be mistaken for a whole-file pragma. Import-gateable exactly like DEC-337's `EntryKind` (`import Core.Runtime.Package;`). Cost: `package` becomes attributable, a new target class. Formatter + LSP + lifter must round-trip it (Inv 17). |
| **2** | `#![Package(Main)]` / `#![Loose]` — Rust-style inner attribute at byte 0 | **Requires narrowing the shebang rule first** (`src/tokenizer/mod.rs:156`) or the line vanishes silently (A9, verified). Plus a new inner-attribute grammar in `parse_program`. | Most familiar to Rust readers; naturally file-scoped. But it introduces a second attribute sigil (`#!` vs `#[`) into a language that has exactly one today, and it puts a load-bearing marker in the one position that currently means "shebang" — a `chmod +x bin/console` file could not carry both on line 1. Highest cost, highest collision risk. |
| **3** | `package Foo.Bar #[Loose];` — postfix attribute | Small parser change at `parse_package` (`items.rs:172-180`). | Contradicts prefix-attribute convention everywhere else in the language; probe shows it currently dies at `expected ';' after package`. Not recommended on consistency grounds alone. |
| **4** | A modifier keyword — `loose package Foo.Bar;` / `package Foo.Bar as free;` | New keyword or contextual keyword. | Terse and self-documenting, but adds reserved-word surface for a rare feature; the language's own precedent (DEC-337) chose an *attribute* over magic identifiers precisely to avoid "nothing in the wind" — a bare `loose` keyword re-introduces exactly that shape. |
| **5** | `phorj.json` opt-out (`"looseFiles": [...]` / `"strictLayout": false`) | Loader would have to start reading `phorj.json`. | **Directly contradicts DEC-282** (*"NO manifest at all"* `:502`; *"No marker file, no config"* `:2253`) and would re-open the standing package-manager re-adjudication (`:2284-2292`). Also invisible at the file you are reading, which is the opposite of what the developer asked for (*"an attribute for the file"*). Strongly not recommended. |
| **6** | No hatch — strict only | Zero. | Simplest and most uniform; DEC-282's search-root model already gives loose scripts a home (`package Main` is location-free). The hatch's real value is for generated code, scratch files, and vendored oddities — none of which exist in the repo today (A11). |

**Recommendation: Option 1** (`#[Loose] package Foo.Bar;`, or a better-named attribute), with Option
6 as the serious alternative to weigh against it. *Why Option 1 over Option 2:* the A9 shebang
collision is verified and real, so `#![…]` costs a tokenizer change to a shipped DEC-282 feature and
permanently contests line 1 with `#!/usr/bin/env phg`; Option 1 needs no tokenizer change at all.
*Why Option 1 over 3/4/5:* it is the only spelling that reuses the language's existing, already-ruled
mechanism verbatim — prefix `#[…]` attribute, import-gated per DEC-337, erased before every backend
per Invariant 5 — so it inherits the formatter/LSP/lifter/transpile currency story instead of
inventing a new one. *Why Option 6 deserves a real hearing:* nothing in the repo needs the hatch
today (A11), and shipping an exemption before shipping the enforcement risks the enforcement never
biting.

**Sequencing note (not a ruling, but a consequence worth surfacing):** the escape hatch is only
meaningful *after* enforcement exists. Building the hatch first would be a no-op; building
enforcement first is independently valuable and immediately testable. Recommend the enforcement fix
(A2 Option 1 + A6 Option 1 + A5 + A7) as one slice, and the hatch as a separate slice gated on the
A1/A8 rulings.

### Grading the developer's claim (item 5 of the brief)

**"None of these are enforced now" — PARTIALLY ACCURATE.** Precisely:

| the developer's rule | status |
|---|---|
| "if a file is not in a structured src folder, it must be `package Main`" | **NOT enforced for files** (probe c). Enforced for stdin/`-e` (probe l). Deliberately retired by DEC-282 (`C-decisions.md:2277`). |
| "`package X.Y` must be folder-compliant" | **Enforced for imported files** (probe e) — **NOT** for a fast-path entry (probe c/j), **NOT** for un-imported files (probe h), and **misfires** for non-`Main` entries (probe i). |
| "must be PascalCase" | **ENFORCED everywhere reachable** — checker for the entry (probe f′), loader for imported files (probe f). Only un-imported files escape (probe h). |
| "`Core.` reserved" | **ENFORCED everywhere reachable** (probes g, h-direct) and additionally unreachable-by-import (probe h′). |
| "PSR-4 compliant" (namespace↔folder *aliasing*) | **Never built** — DESIGNED-NOT-IMPLEMENTED, gated on re-adjudication (`UNIFIED-SPEC.md:483`, `MASTER-PLAN.md:1559`). Today only the plain `folder = package` law exists. |

So: two of the five rules are fully enforced, one is enforced with three holes and a misfiring
diagnostic, one was deliberately retired by a recorded ruling, and one was never built. The *feeling*
that enforcement vanished is explained by the fact that the single most common shape a developer
tests by hand — a lone file with only `Core.*` imports — is exactly the shape that takes the
unvalidated fast path.

---

## Sources (cross-language scan)

- [Kotlin — Packages and imports](https://kotlinlang.org/docs/packages.html)
- [IDE0130: Namespace does not match folder structure — Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/style-rules/ide0130)
- [roslyn #75169 — IDE0130 cannot be disabled in .editorconfig](https://github.com/dotnet/roslyn/issues/75169)
- [roslyn #55550 / #55014 / #73261 / #74758 — IDE0130 false positives](https://github.com/dotnet/roslyn/issues/55550)
- [golang-nuts — "Is the package name must same with name folder name?"](https://groups.google.com/g/golang-nuts/c/oawcWAhO4Ow)
- [go #70755 — gopls quickfix when package name does not match directory name](https://github.com/golang/go/issues/70755)

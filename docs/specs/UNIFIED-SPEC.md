# Phorj — Unified Design Specification

> **One document, eighteen frozen designs.** Consolidated 2026-07-03 (unification-audit Stage D,
> HEAD `0691228`) from every spec under `docs/specs/2026-*.md`, per the developer's ruling to fold
> all of them into a single navigable SSOT. The original files now live in
> [`archive/`](archive/README.md) (2026-07-04) — each section's bare "Source: …md" citation names a
> file under `archive/`; **this document is the reference from now on** — external pointers target
> section anchors here.
>
> **Reading conventions**
> - Every section opens with a **Status** line: `SHIPPED` (implemented, gated green) ·
>   `ADOPTED` (a governing policy in force) · `DESIGNED — not implemented` (ruled, pending build) ·
>   `PARTIALLY SUPERSEDED` / `SUPERSEDED by X` (kept for rationale; the named section/spec is current) ·
>   `HISTORICAL` (a frozen record; the living SSOT has moved elsewhere).
> - Staleness found by the 2026-07-03 unification audit (`docs/research/2026-07-03-unification-audit/`)
>   is **resolved inline, not preserved** — where an original spec asserted something now false
>   (e.g. "zero external dependencies"), this document states current reality and notes what changed.
> - Code samples inside `HISTORICAL` blocks may use retired syntax (`->` returns, `fn` lambdas,
>   pre-overhaul names). They are labeled. Canonical current syntax: `function` (never `fn`),
>   `: T` return annotations, `(A) => B` function types, mandatory `new`, `Core.` stdlib root,
>   PascalCase packages/types, camelCase functions.
> - Rationale and rejected alternatives are deliberately preserved — this is a *design* record,
>   not a rulebook. For the pure delivery rules see `CLAUDE.md` and `docs/INVARIANTS.md`; for the
>   roadmap see `docs/plans/MASTER-PLAN.md` (the roadmap SSOT).

## Table of contents

- **Part I — Foundations**
  - [Founding language design](#founding-language-design) *(2026-06-15)*
  - [Ecosystem strategy](#ecosystem-strategy) *(2026-06-15)*
- **Part II — Language surface, naming, imports**
  - [Naming overhaul](#naming-overhaul) *(2026-06-30 — SHIPPED)*
  - [Nothing in the wind](#nothing-in-the-wind) *(2026-07-01 — principle in force; closure = W2-6)*
  - [Unified import and injected-type discipline](#unified-import-and-injected-type-discipline)
    *(2026-07-03 — the CURRENT import model, SHIPPED S0–S2)*
  - [Import roots and PSR-4 mapping](#import-roots-and-psr-4-mapping) *(2026-07-01 — needs re-base)*
  - [Public-surface file-naming rule](#public-surface-file-naming-rule) *(2026-06-28 — SHIPPED)*
- **Part III — Type system & semantics**
  - [Comprehensive statics](#comprehensive-statics) *(2026-06-28 — A+B SHIPPED, LSB deferred)*
  - [Secret type](#secret-type) *(2026-06-28 — SHIPPED)*
  - [Nested-value index-assignment](#nested-value-index-assignment) *(2026-07-01 — SHIPPED)*
- **Part IV — Standard library & policy**
  - [Standard library charter](#standard-library-charter) *(2026-06-29 — ADOPTED, governing)*
  - [Typed auto-escaping HTML](#typed-auto-escaping-html) *(2026-06-19 — SHIPPED)*
  - [External dependency policy](#external-dependency-policy) *(2026-06-27, amended 2026-07-03)*
  - [PHP extension tiers](#php-extension-tiers) *(2026-06-19 — rule in force)*
  - [PHP parity and beyond gap audit](#php-parity-and-beyond-gap-audit) *(2026-06-21 — HISTORICAL)*
- **Part V — Build & distribution (M2.5)**
  - [phg build master design](#phg-build-master-design) *(2026-06-16)*
  - [Phase 2 cross-OS builds](#phase-2-cross-os-builds) *(2026-06-16 — SHIPPED v0.4.0)*
  - [Phase 3a stub registry](#phase-3a-stub-registry) *(2026-06-17 — SHIPPED; 3b deferred)*
- [Appendix A — source-file map and supersession chains](#appendix-a--source-file-map-and-supersession-chains)

---

# Part I — Foundations

## Founding language design

**Status: HISTORICAL — the frozen v0.1 origin (2026-06-15).** The vision, philosophy, and most core
decisions stand; several surface details were deliberately superseded by later ratified designs
(noted inline). Source: `2026-06-15-phorj-language-design.md`.

### Vision & intent

Phorj is a **new general-purpose programming language inspired by PHP**, built as a **learning
journey that produces a real, runnable socle** while **fixing specific, well-known PHP limitations**.
Explicit non-goal: "dethrone Java and Rust" — the honest target is to **borrow the best ideas from
Java and Rust to fix PHP's worst weaknesses** and prove it with a working compiler. Two intents:
(1) deeply understand language/compiler design by building one end-to-end; (2) give the language a
concrete reason to exist by fixing PHP pains.

### Design philosophy

- **Familiar + explicit wins.** Across every syntax decision the owner chose the PHP/Java-familiar,
  explicit option (`function`, semicolons, type-first, always-typed).
- **Managed now, systems later.** GC first (fast path to a runnable language); ownership/no-GC is a
  deliberate **v2** research branch.
- **Sound over convenient.** No type juggling, no truthiness, no implicit coercion.

The later, sharper formulation (which governs all subsequent work): *Phorj is to PHP what TypeScript
is to JavaScript — a pragmatic, legible, provably-correct upgrade. Familiarity-first IS the adoption
strategy. Phorj removes surprises, never capability.*

### Frozen v0.1 surface (with supersession notes)

| Concern | v0.1 decision | Current state |
|---|---|---|
| Variable sigil | none (no `$`) | stands |
| Member + static access | `.` for everything; `::` dropped | stands |
| Function keyword | `function` | stands (lambda `fn` later retired too — [naming overhaul](#naming-overhaul)) |
| Terminator | semicolons required | stands |
| Modules | `import a.b.c` dotted paths | stands; model refined by [unified import](#unified-import-and-injected-type-discipline) |
| Concat | interpolation `"Hello {name}"` | stands |
| Pipe | `\|>` (from PHP 8.5) | stands |
| Local declaration | type-first `int n = 5;` | stands |
| Mutability | mutable by default, `const`/`final` | **superseded**: Phorj is immutable-by-default with `mutable` (GA-direction ruling) |
| Return annotation | `-> float` in samples | **superseded**: canonical `: T`; `->` retired (W2-4, parser-reject pending) |
| Construction | `Greeter g = Greeter("Tak")` | **superseded**: `new` is mandatory |
| Concurrency | "model TBD" | **resolved**: uncolored `spawn`+channels on cooperative green threads (see [Ecosystem strategy](#ecosystem-strategy) E-8) |

Type system (all stand): sound static typing, no juggling/coercion; true monomorphized generics;
null safety `T?` + unwrap-before-use; ADT `enum` with payloads + compiler-verified exhaustive
`match`; `==` value equality / `is` identity; strict `bool` only (no truthiness); `int` = 64-bit
signed (+`decimal` for money; sized `i8..u64` deferred to v2); UTF-8 `string`. Collections split the
PHP `array` wart: `List<T>` / `Map<K,V>` / `Set<T>` (+ tuples, still pending). OOP: single
inheritance + traits (the safe multiple-inheritance answer, no diamond problem), method overloading
by arity+exact type, `constructor(...)` with promotion + visibility, asymmetric visibility,
statically-typed property accessors, value types, `this` (no `$this`). Errors: exceptions-familiar
surface — later refined into the ratified three-tier model (`throws E` checked + `Result<T,E>` +
unchecked faults; see [gap audit §2.1](#php-parity-and-beyond-gap-audit)). Removed PHP footguns (no
debate): `@` suppression, `$$x`, loose `switch` fallthrough, verbose `use(...)` capture,
function-scoping.

### Execution architecture (as founded)

```
M1: tree-walking interpreter  ← THE SOCLE   (lexer → parser → checker → evaluator)
M2: bytecode + stack VM                     (single self-contained binary via bundling)
v2: native/systems research                 (AOT or JIT; ownership/no-GC; sized ints)
```

The "Go model" server thesis: Phorj compiles to **one binary that IS the web server** — no FPM
per-request model, no resident app server; `scp` one binary and run it.

### v0.1 decisions log (kept verbatim — the founding record)

| # | Decision | Choice | Rationale |
|---|---|---|---|
| 1 | Project intent | Learning journey + real socle, scoped to fix PHP pains | Best ROI for a solo dev; gives the language a reason to exist |
| 2 | Memory model | Managed GC first; ownership/no-GC = v2 | Fastest path to runnable; defers the hardest part |
| 3 | PHP lineage | Clean break + one-way migration tool | Syntax changes make a strict superset impossible |
| 4–6 | Sigil / access / concat | No `$`; `.` for all; interpolation | One operator; `.` is member access so concat = interpolation |
| 7–9 | Keyword / terminator / modules | `function`; `;` required; `import a.b.c` | Familiarity + explicitness |
| 10–12 | Locals | Type-first, no implicit vars, no inference | "Every var typed" rule |
| 13 | Collections | Split `List`/`Map`/`Set`/tuples | Fixes PHP `array` wart |
| 14–15 | Constructor / overloading | `constructor(...)` + promotion; arity+exact type | Decoupled from class name; ad-hoc polymorphism PHP lacks |
| 16–18 | Errors / equality / truthiness | Exceptions-familiar; `==` value · `is` identity; strict bool | No juggling; kills a bug class |
| 19–20 | Ints / decimal | `int` + sized (v2); native `decimal` | Money math without bcmath |
| 21–22 | "MI" / power feats | Traits; value types + accessors + operator overloading¹ | MI power without the diamond problem |
| 23–25 | Exec model | Tree-walker first → bytecode VM; ambitious POC scope | Crafting-Interpreters path; max learning |
| 26–28 | Impl language / name / location | Rust; Phorj (`.phg`); `/stack/projects/phorj` | AST fit, learning synergy, native-v2 alignment |

¹ Operator overloading was later **rejected** by the gap audit (no deterministic PHP target — hidden
`__add` action-at-a-distance); derived `equals`/`compareTo` cover the pragmatic slice.

Prior art studied: **Hack** (the closest older sibling), **Crafting Interpreters** (the M1→M2 path),
**Rust** (impl language + traits/ADT reference), **Go** (server model + concurrency inspiration).

## Ecosystem strategy

**Status: HISTORICAL strategy record (2026-06-15) — the strategic frame stands; the milestone table
was superseded by later roadmaps and today's SSOT is `docs/plans/MASTER-PLAN.md`.** Source:
`2026-06-15-ecosystem-roadmap-design.md`.

### The strategic reframe — two backends, one asset

- **Phorj → PHP transpiler**: runs anywhere PHP runs; the PHP-ecosystem bridge.
- **Native VM**: the standalone "Go model" single-binary server.

Same source, two targets. **Bootstrap lever:** while native infra matured, real Phorj apps could
test/deploy via the PHP backend — the native track was never on the critical path. A later standing
ruling sharpened this: **the PHP transpile/lift legs are migration + test bridges ONLY, never a
runtime Phorj depends on** — every feature must run natively on the Rust backends.

### PHP interop — kept vs rejected (the founding verdicts, all still in force)

| Idea | Verdict | Why |
|---|---|---|
| Transpile-to-PHP backend | ✅ first-class | The ecosystem bridge |
| Native Rust connectors for the VM | ✅ build | Clean, no PHP-engine coupling |
| PHP→Phorj migration tool (typed subset, batch/offline) | ✅ (shipped as `phg lift`) | One-way, best-effort, human-reviewed |
| "Rebuild PHP→Phorj on the spot" (live transpile) | ❌ reject | Sound static typing vs dynamic PHP is undecidable |
| PHP C-extensions via FFI / embed the PHP engine | ❌ reject | Drags the whole engine in; shatters the clean break |
| Dynamic `.so` plugins | ❌ park (v2+) | Breaks single-binary; Rust has no stable ABI |

### The extension-system crux

Unlike PHP's loose registration, every native module in statically-typed Phorj registers **both** a
type signature (for the checker) and an implementation (interpreter + VM) plus an optional
PHP-emission mapping. This dual+ registration is the foundation the stdlib rides on — realized as
the single-sourced `NativeFn` registry (signature + `eval` + `php` in one entry; see the
[stdlib charter](#standard-library-charter) §5).

### Founding ecosystem decisions (E-1…E-8)

| # | Decision | Choice | Current note |
|---|---|---|---|
| E-1 | Backends | Two, behind a clean `Backend` trait | Trait **still not present** at HEAD (per Rule of Three — three pipelines remain free functions; `docs/ARCHITECTURE.md`). The old "grep `trait ` = 0" verification is stale: three *other* traits now exist (`Transport`, `DebugFrontend`, `Suspend`) — but no Backend trait, which was the substantive claim |
| E-2 | PHP ecosystem | Bridge via transpile; native connectors for the VM; batch migration | In force |
| E-3 | Packages | Git-based/decentralized first behind `PackageSource`; registry-capable later | Shipped as `phg vendor` (M5) |
| E-4 | Sequencing | Extension API + stdlib → modules → packages → connectors | Followed |
| E-5 | Architecture | Pluggable traits *where earned* | Tempered in practice: only 4 traits in ~75K LOC, each earning its keep (audit-attested no premature abstraction) |
| E-6 | Testing | One Phorj test surface; PHPUnit-bridge then native | `phg test` shipped (M-Test) |
| E-7 | First connector | HTTP server + Postgres | HTTP server shipped (M6 `phg serve`); DB = W3-1 SQL DBAL (designed, dep amendment approved) |
| E-8 | Concurrency | **Uncolored Go-style `spawn`+channels**, pluggable scheduler, **no async/await coloring** (irreversible — deliberately avoided) | Shipped as cooperative single-threaded green threads (`corosensei`); PHP leg excluded under the LADDER rule via **`E-CONCURRENCY-NO-PHP`** (note: some docs cite a nonexistent `E-TRANSPILE-CONCURRENCY` — the real code is `E-CONCURRENCY-NO-PHP`) |

The founding milestone/ROI table (M2→M8) is retired — numbering drifted (two competing M7/M8
meanings across docs) and the live plan is `docs/plans/MASTER-PLAN.md` waves 0–6. The founding CLI
names it used (`phorj fmt`, `phorj test`, `bench`) predate the [naming overhaul](#naming-overhaul):
canonical verbs are `format`, `benchmark`, `disassemble`, `tokenize`.

---

# Part II — Language surface, naming, imports

## Naming overhaul

**Status: SHIPPED — all 7 stages landed green + byte-identical (2026-06-30).** This remains the
**naming SSOT** for code Claude writes and for W2-9 (re-verification of remainders). Source:
`2026-06-30-naming-overhaul-design.md`.

### Policy (locked)

1. **No abbreviations / shortcuts** in user-facing names — spell out (`recv`→`receive`,
   `args`→`arguments`).
2. **EXCEPT universal mathematical notation** — `sqrt` `abs` `pow` `sin` `cos` `tan` `exp` `log`
   `log10` `gcd` `lcm` `pi` `e` (those ARE the clear names).
3. **EXCEPT type-referencing names** — `toInt` / `parseFloat` / `nextInt` / `asBool` mirror the kept
   primitive type names, so they are consistent, not shortcuts.
4. **EXCEPT universal acronyms** — `Json` `Html` `Url` `Csv` `Regex` `Http`; hash `md5`/`sha256`/`crc32`.
5. **Packages are nouns** (`Validation`, not `Validate`).
6. **Familiarity-first** where it doesn't conflict (kept `Channel`/`Task`/`spawn`/`join`/`Some`/`None`).

### The change list (all confirmed via ask-human, all landed)

**Types.** `Empty` → lowercase keyword **`empty`** (the holdable unit type; collision-proof because
user classes are PascalCase; coexists with `void`). New rule: **`void` may NOT appear in a union**
(uninhabited — `E-VOID-IN-UNION`); **`empty` MAY** (inhabited). Result variants **`Ok`/`Err` →
`Success`/`Failure`** (no abbreviation, symmetric; `Error` is reserved as the exception root so it
can't be reused). Kept: `int float bool string bytes decimal void never List Map Set Optional Error
Channel Task`; `Some`/`None`.

**Keywords.** Lambda **`fn` → `function`** (the `fn` token retired; named functions already used
`function`).

**Concurrency.** `recv` → `receive`. Kept `spawn send join Channel Task` — deliberately `Task` not
`Thread` (cooperative green tasks, not OS threads) and `Channel` not `Observable` (CSP queue, not
reactive streams).

**CLI subcommands.** `fmt`→`format` · `bench`→`benchmark` · `disasm`→`disassemble` · `lex`→`tokenize`.
(The old names are **dead** — docs teaching them are wrong, per the 2026-07-03 audit B3-3.)

**Packages.** `Core.Console`→**`Core.Output`** (output-only; future stdin = `Core.Input`) ·
`Core.Validate`→`Core.Validation` · `Core.Convert`→`Core.Conversion` · `Core.Reflect`→
`Core.Reflection` · `Core.Crypto`→`Core.Cryptography` · `Core.Text`→**`Core.String`** · NEW
**`Core.Environment`** (absorbed `Process.get/all` as `Environment.get/all` — a dedicated flat
module, NOT a `Process.environment.*` object path, rejected D-L9). Kept: `Math File Bytes Html List
Map Set Json Time Http Regex Path Process Random Encoding Hash Url Csv Decimal Test`.

**Native functions** (per module): Output `println`→`printLine`; String `upper`/`lower`→
`upperCase`/`lowerCase`; Html `el`→`element`, `voidEl`→`voidElement`, `attr`→`attribute`,
`boolAttr`→`booleanAttribute`; Decimal `div`→`divide`; Math `ipow`→`integerPower`,
`intdiv`→`integerDivide`, `negInfinity`→`negativeInfinity`, `isNan`→`isNaN`; Path
`basename`→`baseName`, `dirname`→`directoryName`, `stem`→`fileStem`; Process `args`→`arguments`;
Map `getOr`→`getOrDefault`; Random `next`→`nextInt` (+ added `nextFloat`); Time
`nowMillis`→`nowMilliseconds` (whole `millis` family); Url `urlEncode`→`encodeForm`,
`rawUrlEncode`→`encodeUriComponent`, `urlDecode`→`decodeForm`, `rawUrlDecode`→`decodeUriComponent`.

**Kept (challenged but correct):** math notation; type-referencing names; universal acronyms;
`Some`/`None`; `of` factories; Html `raw`/`render`/`text`; hash digest names.

### Why the codemod was safe

The PHP transpile target of each native was **unchanged** — only the Phorj-surface name changed —
so transpiled output stayed byte-identical. Staged by category (natives → packages → Output/
Environment → CLI → `fn` keyword → `empty` + `Success`/`Failure` → living-docs sweep), full gate per
commit, always verifying the `phg` binary itself (the A1 loader-path lesson). Distributable
coordinates (manifest `module`, vendor dirs) stay lowercase. Key commits: `4eec4f3` (fn→function),
`21bb2c2` (receive), `e8bfcc8` (milliseconds), `6ac717a` (`empty` + `E-VOID-IN-UNION`), `5c17351`
(`Success`/`Failure`). Follow-up: W2-9 re-verifies the full matrix against the tree.

## Nothing in the wind

**Status: PRINCIPLE IN FORCE; fault-intrinsic imports SHIPPED (DEC-196 Q3, 2026-07-05).**
Design-locked 2026-07-01. The import-mechanics half was **superseded by the
[unified import model](#unified-import-and-injected-type-discipline)** (2026-07-03), which also
*reversed* one decision (bare function imports). The intrinsics half **shipped 2026-07-05** as the
two-mode `Core.Assert`/`Core.Abort` model (decision 1 below) — the original single-`import Core;`
qualified-only proposal was superseded by developer ruling. Source:
`2026-07-01-no-wind-namespace-and-language-surface-design.md`.

### The governing principle (developer's definition — authoritative, still in force)

**"In the wind" = a name (function/value/type) usable WITHOUT an explicit `import`.** The rule:
*nothing is usable without an explicit import*, with the single closed exception of **the language
grammar itself** (keywords + built-in type words), which cannot be imported because it is syntax.

Corollary (as originally stated): a name imported to a bare call site is NOT in the wind — the sin
is the *absence* of an import, not a bare call site. **⚠ Later refinement (2026-07-03, supersedes
the corollary for functions):** bare *type* imports are exactly that shape (`import Core.Http.Router`
→ bare `Router`), but **functions were ruled NOT bare-importable at all** — a bare `trim(x)` is
maximally in-the-wind even when imported; functions stay module-qualified or UFCS. See
[unified import §3](#unified-import-and-injected-type-discipline).

### Decisions and their fate

1. **Fault intrinsics behind explicit imports (two-mode) — ✅ SHIPPED (DEC-196 Q3, 2026-07-05).**
   The four intrinsics live in two reserved language-core modules — **`Core.Assert`** = { `assert` }
   and **`Core.Abort`** = { `panic`, `todo`, `unreachable` } — and follow the SAME two-mode discipline
   as types/variants (the model the developer ruled after this section's original *single-`import Core;`,
   qualified-only* proposal was surfaced as conflicting with DEC-196 Q3):
   - **whole-module import → QUALIFIED call:** `import Core.Assert;` ⇒ `Assert.assert(cond[, "msg"])`;
     `import Core.Abort;` ⇒ `Abort.panic("msg")` / `Abort.todo()` / `Abort.unreachable()`.
   - **member import → BARE call:** `import Core.Abort.panic;` ⇒ `panic("msg")` (grouped:
     `import Core.Abort.{ panic, todo };`, DEC-186 syntax).
   Any intrinsic call not covered by the matching import = **`E-UNIMPORTED`**. This honors *nothing in
   the wind* (a bare intrinsic requires an explicit member import naming it; the module import gives the
   attributed qualified form), preserving the special semantics (`never`-typing; compile-time literal
   message for `--dump-on-fault`; guaranteed-not-stripped; lowers to PHP `throw`) and staying disjoint
   from `Core.Test.assert`. Implemented as a raw-program pass (`resolve_intrinsic_imports`) that
   validates coverage and normalizes the qualified form to the bare intrinsic every backend already
   lowers — backends unchanged, byte-identity preserved. `is_intrinsic_name` still reserves the four
   names against user-function shadowing. The earlier "single `import Core;`, qualified-only" text is
   superseded by this (broader) two-mode model.
2. **Deep imports + dual call form** — `import Core.A.B.C` to any depth; no wildcards. The
   *type*-leaf case shipped via the unified import model (member-imports). The *function*-leaf case
   (`import Core.List.doThis` → bare `doThis(...)`) was **REVERSED** — functions are not
   bare-importable. Deep-import ambiguity/shadowing questions folded into W2-6.
3. **Import aliasing** — `import a.b as c;` existed (M5 S2c); extension to stdlib + deep paths is
   part of W2-6.
4. **De-reserve built-in type names that belong to importable modules** (developer-selected):
   **`Attr` → `Core.Html`** (no literal-syntax justification; `Html` itself STAYS built-in — it
   backs the `html"…"` typed literal, like `bytes`↔`b"…"`); **`Error` → `Core.Error`**;
   **`Channel`/`Task` → `Core.Async`** — the developer explicitly rejected `Core.Concurrent` as a
   misnomer: Phorj green threads are cooperative + single-threaded (`Value` is `Rc`, not `Send`), a
   `Task` is never parallel, and `Core.Async` names what it actually is. Primitives and
   `List`/`Map`/`Set` KEEP reserved status (literal syntax justifies them).
   **NOT YET IMPLEMENTED** — W2-6 (whose inventory must also account for the 9 reserved numeric
   words `double`, `i8`–`u64`).
5. **Real parallelism — ON HOLD.** The `Rc` memory model is a *commitment* that selects the
   concurrency model: shared-memory threading is off the table unless the 2.4× object fast-path is
   abandoned. Models brainstormed for the eventual M-Parallel plan: async-I/O reactor (1 core,
   Node-style), **actor/message-passing (best structural fit — per-heap threads + owned-value
   channels, no data races by construction, Erlang precedent)**, data-parallel `List.map` (rides
   immutability, shippable soonest), shared-memory `Send`/`Sync` (worst fit, kills the `Rc` win).

Same-session context decisions (recorded here because the spec was their SSOT): Q1 — NO
string-instantiate/string-call dynamic dispatch (un-typeable/un-erasable); ADD method-references-
as-values + a typed-registry guide. Q2 — `Core.File` stateless namespace ops shipped (`a23ca00`).
Q3 — full HTTP client direction: admit `rustls` under the crypto clause (realized in the 2026-07-03
[dependency-policy amendment](#external-dependency-policy)); reuse M6 `Request`/`Response`; socket
quarantined behind a `Transport` trait; milestone W3-2 (design draft exists).

## Unified import and injected-type discipline

**Status: ADOPTED 2026-07-03 (developer, interactive adjudication) — the CURRENT import model.
SHIPPED: S0 `11a6c71`, S1 `cd29f3c`, S2 `0cedcb8`+`202ec2b`+`20ecfe0`+`bc523c1` (feature-complete;
`type_only` vestige removed).** Supersedes the split `import`/`import type` surface and the
import-mechanics parts of [Nothing in the wind](#nothing-in-the-wind). Source:
`2026-07-03-unified-import-and-injected-type-discipline.md`.

### Motivation

The developer found `#[Route(...)]`, `Router`, `Request`, `Response` (from `import Core.Http`)
usable **bare** — no qualifier, no member import — violating "nothing in the wind", which was
already enforced on injected **enum variants** (`Json.Object`, `E-INJECTED-VARIANT-BARE`).
Inspection found six injection preludes and two pre-existing import kinds; the fix unifies the
import surface and extends the discipline to all injected Core types.

### The model (locked)

**1. One `import` keyword — `import type` is REMOVED (no back-compat).** The resolver classifies
each `import PATH [as ALIAS];` by resolving `PATH`:
- resolves to a **module/package** ⇒ bind a **call-qualifier** (last segment or alias):
  `import Core.Http` → `Http.foo()`;
- resolves to a **type** (class/enum/interface/trait) ⇒ bind the **bare type name**:
  `import Core.Http.Router` → `Router`; `import Acme.Geometry.Rect` → `Rect`;
- neither ⇒ `E-IMPORT-UNKNOWN`.

The former `import type PATH` is deleted from the grammar (**it no longer parses** — any doc
teaching it is wrong); the four `E-TYPE-IMPORT-*` codes re-homed as `E-IMPORT-BUILTIN` /
`E-IMPORT-UNKNOWN` / `E-IMPORT-CONFLICT` / `E-IMPORT-SHADOW`.

**2. Injected Core types get import discipline.** The six preludes (`src/cli/mod.rs`
`inject_*_prelude`), classified by module-leaf vs member-name:

| Module | Injected | Leaf | Discipline |
|---|---|---|---|
| `Core.Json` | `Json` enum | `Json` | leaf==type ⇒ compliant as-is; variants stay `Json.Object` |
| `Core.Regex` | `Regex` class | `Regex` | compliant as-is |
| `Core.Secret` | `Secret<T>` class | `Secret` | compliant as-is |
| `Core.Decimal` | `RoundingMode` enum | `Decimal` | member ⇒ `Decimal.RoundingMode` (or member-import) |
| `Core.Http` | `Request`,`Response`,`Route`,`Router` (+ `#[Route]`) | `Http` | members ⇒ `Http.X` / `#[Http.Route]` |
| `Core.Time` | `Duration`,`Date`,`Instant` | `Time` | members ⇒ `Time.X` |

Rules for the multi-type modules: **default = qualified by leaf** (`Http.Router`, `Time.Duration`,
`Decimal.RoundingMode`, `#[Http.Route(...)]`); **bare only via member-import**
(`import Core.Http.Router;` → bare `Router`); violations = **`E-INJECTED-TYPE-BARE`** (mirror of
`E-INJECTED-VARIANT-BARE`) with a fix-it. The preludes' own internal references are exempt (they are
the declaring block). The qualifier is **Phorj-surface only** — the transpiler erases it; PHP stays
bare (`new Router()`). This required new **qualified type resolution** `Qualifier.Type` in type
position (S1) — parser preserves the dotted `Type::Named{name:"Http.Router"}` (so `phg format`
prints the qualified form and the migration is fmt-idempotent); a dedicated collapse pass
(`src/checker/collapse_injected.rs`, modeled on `expand_aliases`) rewrites it to the bare name
after `desugar_auto_router` and before `check_resolutions`, so the checker and every backend see
bare `Router`. The injected-type registry is single-sourced in
`checker::enforce_injected::module_of`.

**3. Functions are NOT bare-importable; no associated functions.** Functions/natives stay
**module-qualified** (`String.trim(s)`) or **UFCS** (`s.trim()`, method-first per DEC-087) — always
traceable. A bare imported free call (`trim(s)`) is exactly the in-the-wind problem and is rejected
*by omission* (no function-import form exists). **No associated functions**: `MyClass.stringify(x)`
does NOT resolve a free `stringify(MyClass x)` — a class is not a free-function namespace; only
modules are. Use `x.stringify()` (UFCS) or a `static function`. `Module.fn(x)` works only because
`Module` is a module.

### Acceptance (all met at HEAD)

`import type` no longer parses (repo grep = 0 in code); bare injected member types without a
member-import → `E-INJECTED-TYPE-BARE`; `Http.Router` + `#[Http.Route]` + qualified
`new Http.Router()` resolve; member-import enables bare use; single-type modules unchanged; full
PHP-oracle gate green at each slice; migrated examples byte-identical. Minor deferred:
`instanceof`/`as` with qualified names (0 usages in the corpus).

## Import roots and PSR-4 mapping

**Status: DESIGNED, NOT IMPLEMENTED — and it PRE-DATES the unified import model: it MUST be
re-based/re-adjudicated before build (audit finding B4-5; tracked as MASTER-PLAN W2-7) or it becomes
"import redesign #5".** Breaking change to the M5 import/package model; needs a migration codemod.
Source: `2026-07-01-import-roots-psr4-design.md`.

### The two orthogonal axes (the core clarification — still valid)

- **Namespace** — the logical package name written in code and emitted as the PHP namespace
  (`App.Data` → `namespace App\Data`).
- **Root / origin** — which *directory* the files physically live in (`src/`, `bin/`, `vendor/`).

Origin is conveyed **by the namespace root + a `vendor:` marker**, not by a per-import prefix on
everything. By eye: `Core.` = stdlib · `vendor:` = dependency · everything else = first-party.

### The designed model (subject to re-adjudication)

Optional `[packages]` map in `phorj.toml`:

```toml
[packages]
App        = "src"        # App.*  ⇒ files directly under src/ (App.Data = src/Data.phg)
Console    = "bin"        # additional root
Migrations = "migrations"
```

Resolution: (1) **no entry → default convention**: source root `src/`, folder = namespace path —
zero-config projects keep working (**LOCKED** by the developer 2026-07-01: the "mandatory, no
default" alternative was rejected as too heavy for small projects); (2) an entry aliases that
directory (decoupling namespace from folder — the PSR-4 move); (3) extra entries add roots;
(4) **`vendor:` imports** (`import vendor:Acme.Strutil;`) resolve from the vendored tree, required
for deps (they're outside your control and could collide with first-party roots — the prefix both
disambiguates and signals "external" by eye). **Emitted PHP namespace is always the namespace path,
never the folder** — folder mapping is a loader concern; PHP output is folder-independent (PSR-4).
New codes: `E-PKG-ROOT-*`, `E-UNKNOWN-ROOT` (with did-you-mean), `E-VENDOR-MISSING`. `vendor:` is
the only prefix this slice; `package Main;` stays a reserved root at the project source root.

**Re-base requirement:** the design's loader/checker surfaces reference the pre-S0 import
classification (`import type`-era maps). The semantics above are namespace-root/loader-level and
largely orthogonal to the unified model, but the implementation plan and the `vendor:` parser
surface must be re-derived against the shipped S0–S2 loader before any build.

## Public-surface file-naming rule

**Status: SHIPPED (approved 2026-06-28, hard errors; `E-FILE-*` live in the loader + `phg explain`).**
Source: `2026-06-28-public-surface-file-rule-design.md`.

### Goal & rule

Make a file's name tell you its public surface **without** importing PSR-4's micro-file tax or
contradicting Phorj's Go-shaped, function-heavy, folder=path package model — "Go packages,
PSR-4-ish public-type files". A non-`main` file is exactly one of two kinds, decided by what it
exports:

- **Type module** — exactly **one** public named type; the file stem must equal that type name
  **byte-exactly, casing included** (`class Circle` ⇒ `Circle.phg`).
- **Function module** — **zero** public types, any number of public free functions; lowercase/topic
  stem.

Both kinds may contain any number of `private`/`internal` helper types and functions (single-file-
scoped, invisible across files — the ergonomic allowance). A file may NOT mix a public type with a
public free function, nor declare two public types. **A file declaring the entry point `main` is
fully exempt** (covers every single-file guide example, loose `phg run`, `-e`/stdin).

| code | when |
|---|---|
| `E-FILE-NAME` | type module's stem ≠ its public type's name (incl. casing) |
| `E-FILE-MULTI-PUBLIC` | non-`main` file declares ≥2 public types |
| `E-FILE-MIXED-PUBLIC` | non-`main` file declares a public type AND a public free function |

### Why non-contradictory, and where it lives

`folder=path` (`E-PKG-PATH`) governs *packages*; this rule governs *the public surface within a
package* — orthogonal axes. The two things that made PSR-4 impossible for Phorj (free functions,
helper types) are explicitly carved out. Enforced in the **loader**, project mode only, in the same
per-file pass as `E-PKG-PATH`; front-end only — the byte-identity spine is untouched (a renamed file
produces identical output). Deferred: a per-project opt-out; applying the rule inside `package
Main`; auto-rename tooling.

---

# Part III — Type system & semantics

## Comprehensive statics

**Status: Areas A + B SHIPPED (inherited + overloaded statics; tests in
`src/checker/tests/static_methods.rs`); Area C (late static binding) DEFERRED as a documented
non-feature.** Research delivered 2026-06-28; the header's "awaiting scope fork" is stale — the
recommended scope was built (audit B3-13). Source: `2026-06-28-statics-research-design.md`.

### Baseline (B0, pre-research)

`ClassName.method(args)` called a `static` method, gated on a per-class **own-only**
`static_methods` set; a static call lowers to a single direct call. Three deferrals were researched:

### Area A — inherited static methods (`Child.parentStatic()`) — BUILT

PHP-faithful: statics are inherited. Design: flatten `static_methods` across ancestors exactly as
`methods` already flatten (reuse `class_supertypes`); resolution + lowering target the **declaring**
class's static function (walk `cls`→ancestors for the first owner — most-derived wins on override).
No runtime concept needed, no new `Op`/`Value`. Low cost, closes the most common gap.

### Area B — overloaded static methods — BUILT

The checker already had full overload *resolution* (`check_overload_call`) for instance/free calls;
the design routes multi-signature static calls through the same machinery, then lowers to the
**resolved** overload's function. The byte-identity risk B0 flagged ("silently calling one
overload") is exactly what compile-time resolution removes: the chosen overload is fixed at check
time, identical on all backends. (Shipped; mixing static and instance overloads of one name is
rejected.)

### Area C — late static binding (`static::`, `new static()`) — DEFERRED, documented non-feature

The real fork. LSB is a genuine PHP idiom (ORMs, active-record factories) — familiarity argues for
it — but: it is subtle/surprising (`self::` vs `static::` is a classic PHP footgun); it introduces a
**runtime "called class" concept** threaded through static dispatch — the first static feature that
isn't pure compile-time resolution, against "legible, no new runtime machinery unless necessary";
and `new static()`'s type is an F-bounded-polymorphism shape Phorj doesn't have. **Ruling: defer +
reject cleanly** — the factory-returns-subclass pattern is achievable by overriding the static in
each subclass (explicit > magic). Revisit as its own milestone only if a concrete need appears.

## Secret type

**Status: SHIPPED (design-locked 2026-06-28, developer-resolved Fork B; `inject_secret_prelude` +
checker tests live).** Source: `2026-06-28-secret-type-design.md`.

### The resolved fork — opaque & non-printable (loud)

Not a runtime-`***`-rendering wrapper (that would need a new `Value` variant + a *silent* `***`).
Instead: a `Secret<T>` value simply **isn't a string and has no display**, so any attempt to
print/interpolate it is a clean **compile error** — the strongest, loudest guarantee, falling out of
the type system for free. Decided after an implementation discovery reopened the earlier
"displays as `***`" wording: Phorj's display path renders only primitives, so a class-typed `Secret`
is already unprintable — no `***` machinery is needed or wanted. Loud > silent; zero new
`Op`/`Value`/`Ty`.

### Model — an injected generic class

```phorj
class Secret<T> {
  constructor(private T value) {}
  function expose(): T { return this.value; }
}
```

Injected when a program imports `Core.Secret` (a user-declared `Secret` wins, like every prelude).
`new Secret(apiKey)` infers `Secret<string>` via the generic-class unifier. The field is `private`
⇒ `.expose()` is the only read path. Non-printable by construction: `Output.printLine(s)` / `"{s}"`
is a type error. Under the [unified import discipline](#unified-import-and-injected-type-discipline)
`Secret` is a single-type module (leaf==type) — compliant as-is.

**`W-SECRET` lint (secondary nudge):** warns when `<recv>.expose()` appears as a **direct argument
to a known sink** (`Output.printLine`/`print`, `Core.File.write`) with a `Secret<_>` receiver —
"exposing a Secret directly into a sink". Documented scope limit: the lint is *syntactic* on the
direct argument — laundering through a local is not flagged; full taint analysis is out of scope
(by-construction `Secret` dominates taint tracking, per the gap audit's rejection of
K-taint-tracking). The type-level non-printability is the real guarantee.

**Transpile (peer target):** a PHP `final class Secret` whose promoted constructor parameter is
annotated `#[\SensitiveParameter]` (value redacted in PHP stack traces — the K-secrets-type intent);
`T` erases to `mixed` via ordinary generic erasure; `final` because a secret wrapper must not be
subclassable; keyed to the injected class only.

## Nested-value index-assignment

**Status: SHIPPED (`Op::SetPath` live in `vm/exec` + `chunk::validate` + `compiler::stack_effect`).**
M-DOGFOOD follow-on, surfaced by porting `benchforge` (the Matrix benchmark). Source:
`2026-07-01-nested-value-index-assign-design.md`.

### Problem

A value-type element set previously required the container to be a **simple local**: `this.f[i]=e`,
`grid[i][j]=e`, `m[k1][k2]=e` were `E-ASSIGN-TARGET` compile errors, blocking in-place matrix/2-D
algorithms and field-held-collection mutation. (Field *paths* `a.b.c=e` and `map[k].field=e` already
worked — handle semantics.)

### Model — a place is a base + a chain of steps, made mutable root-to-leaf

An assignment target is a **place expression**: a base binding, then steps (`.field` | `[index]`),
ending in a settable step. Two invariants make this sound under Phorj's memory model:

- **Instances are shared-mutable handles** — `.field` navigation mutates the shared instance in
  place. No copy.
- **Lists/Maps are value-type (COW)** — nested value containers are made unique with `Rc::make_mut`
  **at each level, root-first**: after `make_mut` on the outer container *in its slot*, the inner
  `Rc` is uniquely held, so the inner `make_mut` is in-place too. COW preserved — a genuinely shared
  level still copies, correctly. The root must be mutated in its slot so the outer `make_mut` sees
  refcount 1, else the whole chain copies.

Supported: all forms of `base (.field | [index])* <final settable step>` at arbitrary depth (no
artificial cap beyond the recursion guard). Checker: a generalized **place walker** (types each
step, requires a `mutable` root binding, `E-ASSIGN-PATH-TYPE` for mid-path mismatches;
`E-ASSIGN-TARGET` narrowed to genuinely-illegal bases — call results, literals). Interpreter:
recursive lvalue eval descending with `make_mut`/`borrow_mut`, setting via the shared
`value::list_set`/`map_set` kernels; eval order = all index expressions left-to-right, then RHS
(matching the VM). VM: one new op **`Op::SetPath(PlaceDesc)`** (root = local slot or eval-base-off-
stack marker + ordered `Field`/`Index` steps), navigating in place, never putting a `&mut` on the
value stack; extends the three coupled matches per the Op-coupling invariant. Compound-assign on a
deep path (`grid[i][j] += 1`) rides the same walker (read-path + set-path desugar).

---

# Part IV — Standard library & policy

## Standard library charter

**Status: ADOPTED 2026-06-29 — the governing policy for every `Core.*` module.** Descriptive of the
conventions already practised and prescriptive for everything added next. When a new native
disagrees with the charter, change the native — or amend the charter in the same change with a
rationale. Source: `2026-06-29-m4-stdlib-charter.md`. (The charter's module list used pre-overhaul
names; canonical names are post-[naming-overhaul](#naming-overhaul): `Output`, `String`,
`Conversion`, `Validation`, `Cryptography`, `Reflection`, etc. The tree now has 26 native module
files.)

Five axes govern every stdlib addition:

### 1. Naming

Modules are `Core.<Pascal>` — reserved `Core` root + jargon-free, domain-obvious PascalCase leaf
(`Output` not `Io`, `File` not `Fs`). Functions are `camelCase`; predicates start
`is`/`has`/`starts`/`ends`/`contains` and return `bool` — never `0`/`1` or `int?`. A name must not
collide with a PHP-reserved symbol after erasure. No abbreviations that aren't already idiomatic;
match the sibling module — consistency beats individual preference. (See the
[naming overhaul](#naming-overhaul) for the binding no-abbreviation policy.)

### 2. Argument order — subject-first

Every native takes its **subject first**, then operands, then options; the **closure/callback goes
last** (longest, most-likely-multiline argument):

```
String.split(s, sep)          List.map(xs, f)             Map.getOrDefault(m, key, default)
String.replace(s, from, to)   List.reduce(xs, init, f)    Decimal.divide(a, b, scale, mode)
```

This is the order UFCS method sugar (`s.split(sep)`) desugars to, and it reads left-to-right. Phorj
has no named arguments; order the most-likely-omitted argument last.

### 3. Optional vs fault — the recoverability rule

The single most important stdlib decision:

- **Return `T?` when absence is an ordinary, expected outcome** the caller routinely handles:
  `List.first(xs) -> T?`, `Map.get(m,k) -> V?`, `String.parseInt(s) -> int?`, `Json.parse(s)`,
  `File.read(p)`. The default for any parse/lookup/IO that can fail on normal input; composes with
  `??`/`?.`/if-let/`match`.
- **Fault when the precondition is a programmer error**: index OOB, `m[k]` on a required key,
  division by zero, overflow, a negative scale. A fault aborts with a stack trace — a *bug*, not a
  condition.
- **Two surfaces for the same data are allowed and encouraged** when both modes are legitimate:
  `m[k]` (faults — "I know it's there") AND `Map.get` (`null` — "it might not be").
- **`throws E` is the third tier** — a recoverable error carrying *information* (not just absence),
  enforced up the call chain. Never where `T?` suffices.
- A fault message is a **compile-time string literal, byte-identical across both backends**
  (compared by `FaultKind` in the differential harness); the transpiled PHP throws a matching body.

### 4. Determinism tiers — what may enter `differential.rs`

The byte-identical `interpreter ≡ VM ≡ real PHP` spine is sacred.

- **Tier 1 — pure & deterministic**: byte-identity-gated; MUST ship a runnable guide example.
- **Tier 2 — deterministic but representation-sensitive** (float printing: irrationals, `NaN`/`inf`,
  `1e20`): the `interpreter ≡ VM` spine is always identical (both Rust); only PHP's native formatter can
  differ. Never printed raw in an example — exercise through a predicate or formatter; documented in
  `KNOWN_ISSUES.md`.
- **Tier 3 — impure/non-deterministic** (clock, external-state FS, network, randomness, env,
  process): **quarantined** — excluded from `differential.rs` (`uses_impure_native`), validated by
  dedicated tests with seeded/injected/fixture inputs. Network was forbidden until M6 — the
  determinism, not the dependency, is the gate.

### 5. Native (Rust) vs injected `.phg` prelude

- **Native** (`src/native/<module>.rs`) when the operation needs Rust primitives or must be a single
  typed op. A native single-sources checker signature + `eval` (`Pure`/`HigherOrder`/`Reflective`) +
  `php` mapping in one `NativeFn`.
- **Injected `.phg` prelude** (`cli::inject_*_prelude`, gated on the import) when best expressed in
  Phorj itself — a type with methods (`Json`, `RoundingMode`, `Time`'s `Instant`/`Duration`) — riding
  the existing backends with no new plumbing, itself byte-identity-gated. (Injected types now carry
  the [import discipline](#unified-import-and-injected-type-discipline). The audit flagged the
  injected-prelude *mechanism* as a watch item (B2-2): the per-type special-case rules exist only
  because stdlib types inject as AST preludes instead of resolving through the loader — a unification
  decision is recommended before the W3/W4 waves.)
- **Higher-order natives** run closures via the backend-supplied `ClosureInvoker` (re-entrant VM
  `run_until`) — results AND faults byte-identical by construction. No new `Op`.
- **The PHP mapping uses only `php -n`-available core** (PCRE, not mbstring — see
  [PHP extension tiers](#php-extension-tiers)); documented exception: `decimal` (BCMath, loaded
  explicitly).
- **Erasure-safety**: a native's `Ty::Param` is registry-only; the compiler types native calls by
  shape and the transpiler emits via the `php` closure, so no type variable reaches a backend.

### 6. Every native ships complete (developer rule)

A Tier-1/2 native lands in the **same change** as: a runnable `examples/guide/<topic>.phg` line
(auto-gated by the differential glob), an `examples/README.md` coverage-matrix entry, unit tests,
and any Tier-2 `KNOWN_ISSUES` note. Tier-3 ships its dedicated non-differential test instead.

**Quick decision tree for a new stdlib function:** name it camelCase (predicate → `is…` → `bool`);
subject first, closure last; fails on normal input → `T?`, programmer bug → fault, information to
enforce → `throws E`; pure → Tier 1 + example, representation-sensitive → Tier 2 never printed raw,
impure → Tier 3 quarantine; Rust primitives → native, Phorj-expressible → prelude, takes a closure →
higher-order; ship example + README + tests same change.

## Typed auto-escaping HTML

**Status: SHIPPED IN FULL — Waves 1 (escape kernel), 2 (builders), 3 (`html"…"` sugar) + the named
per-tag helper set. Names below are post-overhaul canonical (`element`/`voidElement`/`attribute`/
`booleanAttribute`; the original spec used `el`/`voidEl`/`attr`/`boolAttr`, renamed 2026-06-30).**
Source: `2026-06-19-core-html-design.md`. Trigger: *"in a `.phg` file, if I want to write HTML, how
do I do it, like in PHP?"* — locked answer: all three layers together.

### Problem & thesis

PHP's headline feature — a `.php` file IS an HTML template — is also its most infamous footgun:
`echo "<h1>$name</h1>"` with untrusted `$name` is stored XSS, and escaping is opt-in, so the
*unsafe* path is the *short* path. Phorj's contract (TypeScript:JavaScript) fixes the footgun at the
type level: the answer to "how do I write HTML" is **a distinct type `Html` that you cannot produce
from untrusted text except through an escaping boundary**. The unsafe path stops compiling.

### The kernel — `Html` as an erased newtype

`Html` is a distinct checker type (`Ty::Html`) that **erases to PHP `string`** — structurally like
`bytes`. No new AST variant, no new `Value`, **zero new `Op`**, zero VM/interpreter divergence: the
safety lives entirely in the type, erased before the backends run. One rule the checker enforces:

> **`string` is not assignable to `Html`, and `Html` is not assignable to `string`.** The only
> bridges are the named natives.

| Native | Signature | Meaning | PHP emission (tier-1) |
|---|---|---|---|
| `Html.text` | `(string) => Html` | Lift untrusted text in, **auto-escaped** — the safe boundary | `htmlspecialchars($a, ENT_QUOTES, 'UTF-8')` |
| `Html.raw` | `(string) => Html` | **Audited trust opt-out** — greppable | identity |
| `Html.render` | `(Html) => string` | Exit boundary | identity |
| `Html.concat` | `(List<Html>) => Html` | Join fragments | `implode('', $a)` |

### The escaping table — THE byte-identity invariant

`Html.text`'s Rust `eval` and its PHP emission **must produce byte-identical output** — the single
highest-risk point of the feature. Pinned exactly: PHP side always emits
`htmlspecialchars($s, ENT_QUOTES, 'UTF-8')` (flags pinned — PHP's defaults have changed across
versions; pinning is version-stable and `php -n`-safe). Rust side replicates that exact
five-character table, **`&` first** (else its own insertions double-escape):
`&`→`&amp;` · `<`→`&lt;` · `>`→`&gt;` · `"`→`&quot;` · `'`→`&#039;`. Phorj strings are valid UTF-8,
so `htmlspecialchars`' invalid-byte handling never triggers. A unit test pins the Rust table against
`php -n` over an adversarial fixture.

### Builders

`Html.element(tag, attrs, children)` + `Html.voidElement(tag, attrs)` cover all of HTML;
`Html.attribute(name, value)` / `Html.booleanAttribute(name)` produce `Attr` — a **second erased
newtype**, so attribute values are also auto-escaped and a raw string cannot be smuggled into
attribute position. Tags/names are author-supplied literals (trusted); only values and children
carry untrusted data, and both have boundaries. A curated named per-tag set (`div p span a h1…` +
void `br img input hr`) ships as native registry entries (one-line macro to extend) — NOT a
`.phg`-stdlib bootstrap, staying consistent with the rest of `Core.*`.

### Sugar — `html"…"` (the "like PHP" layer)

A prefixed string literal, lexed like `b"…"`, **desugared in the parser** into kernel calls — after
desugaring the AST contains only `Html.raw`/`Html.text`/`Html.concat`, so all backends and the
byte-identity gate see ordinary native calls. Literal chunks → `Html.raw` (author markup is trusted
by definition); each hole `{e}` resolves **by type** in the checker: `Html` → embedded; `string` →
`Html.text(e)` (escaped — the safe default); `int`/`float`/`bool` → escaped via to-string; anything
else → compile error `E-HTML-HOLE`. **The crucial safety point: the default hole behavior is
escape** — injecting trusted markup requires visibly writing `{Html.raw(x)}`. Unsafe is long, safe
is short — the inverse of PHP. Multi-line came free (ordinary `"…"` already accepts raw newlines —
verified in the lexer, which retired the separate multi-line-strings backlog item).

### Challenged alternatives (all rejected)

| Alternative | Why rejected |
|---|---|
| `Html` = plain `string` | No compile-time safety — collapses to PHP's footgun |
| New `Value::Html` runtime variant | Pointless runtime cost + a new divergence surface; the property is static — erase it like `bytes` |
| Sugar-only | Can't compose programmatically (build `List<Html>` in a loop, factor helpers) — templating-in-strings is PHP's dead-end |
| Kernel-only | Verbose for real pages; the sugar is the "like PHP" payoff |
| Builders as `.phg` stdlib | No stdlib-in-Phorj bootstrap exists; native entries erase cleanly |

**Open (documented, not silent):** v1 covers text + attribute-value contexts (both under
`ENT_QUOTES`); URL context (`javascript:` URLs), CSS, and `<script>` bodies need context-specific
escaping — a later wave (`Html.url_attr`/typed URLs; gap-audit row K-html-context-escape). A
`W-HTML-RAW` audit lint is deferred.

## External dependency policy

**Status: ADOPTED 2026-06-27; AMENDED 2026-07-03 (SQL driver + TLS domains approved).** This policy
is why "zero external dependencies" claims in older docs are **false and must not be repeated**:
Phorj's *core stays `std`-only*, but four vetted, feature-gated crates ship **by default**, and two
more domains are approved. Source: `2026-06-27-dependency-policy.md`.

### The rule

Phorj's core (lexer, parser, checker, interpreter, VM, transpiler, loader, bundler) **remains
`std`-only**. An external crate is admitted **only** when ALL hold:

1. **The domain is a primitive `std` lacks where the responsible implementation is a vetted crate,
   not hand-rolled code.** The admitted sub-domains (each with the same shape — *dangerous or
   impossible to implement safely from phorj's own code*):
   - **Crypto** — "never roll your own"; `std` ships none.
   - **Untrusted-input parsers where a safe engine cannot be built in `std`** — specifically
     **regex**: a hand-rolled matcher over attacker-controlled patterns is a ReDoS + correctness
     hazard; a vetted linear-time finite-automaton engine is strictly safer.
   - **OS-signal handling** (2026-06-29) — `std` exposes no signal API; the only native path is
     hand-rolled `unsafe` `sigaction`, piercing `#![forbid(unsafe_code)]`. Scoped to signal
     handling, NOT general OS integration/async runtimes/I-O frameworks.
   - **Stackful coroutines** (2026-06-29) — green-thread suspension mid-evaluation, deep in the
     interpreter/VM stack; `std` has no primitive and the alternative is `unsafe` stack switching.
     A low-level primitive, NOT an async runtime (tokio et al. remain disallowed).
   - **Embedded SQL engine + SQL drivers** (2026-07-03 amendment) — see below.
   - **TLS** (2026-07-03 amendment) — see below.

   Convenience, performance, general-purpose, or *format-parsing* crates (JSON, TOML, YAML, HTTP
   parsing) do **not** qualify — those are done in `std`.
2. **The crate is independently audited / widely vetted** with an active maintenance record. An
   unaudited crypto implementation is *more* dangerous than the dependency — never admitted.
3. **No `std`-only path is both secure and Phorj-native.** Delegating to the PHP transpile target is
   NOT a substitute — the bridges exist only to migrate and to test; Phorj's own runtime must
   implement every feature natively.
4. **Feature-gated** so the WASM playground stays tiny + browser-safe.

If a candidate fails any clause, the feature is deferred — it does not justify a dependency.
Anything outside the admitted domains requires revisiting this policy itself, not just adding a row.

### Admitted dependencies (default features `crypto`,`regex`,`signals`,`green`)

| Crate | Domain | Used by | Gate | Key justification |
|---|---|---|---|---|
| `argon2` (RustCrypto) 0.5.x | Argon2id password hashing | `Core.Cryptography` | `crypto` | OWASP #1 KDF; audited; emits standard PHC strings → interoperates with PHP `password_verify` |
| `regex` (BurntSushi) 1.x | ReDoS-safe regex | `Core.Regex` | `regex` | RE2-style finite automaton, guaranteed linear-time, exhaustively fuzzed; its restricted feature set (no backref/lookaround) is exactly the regular subset PHP `preg_*` matches identically, so the byte-identity spine holds; unsupported patterns rejected at `Regex.compile` |
| `ctrlc` 3.x | OS signals (SIGINT/SIGTERM) | `phg serve` graceful shutdown | `signals` | Confines the unavoidable `unsafe`; serve is outside the byte-identity spine (quarantined), so this never touches `interpreter ≡ VM ≡ PHP` |
| `corosensei` 0.3.x | Stackful coroutines | `spawn`/channels (green threads) | `green` (non-wasm) | Miri-tested, by the hashbrown/parking_lot author; wasm32 has no native stack to switch (verified) — on wasm the interpreter delegates to the VM's frame-swap; green threads are quarantined from the PHP oracle |

Transitive: argon2 → `password-hash`, `base64ct`, `rand_core`/`getrandom`; regex →
`regex-automata`, `regex-syntax`, `aho-corasick` — same audit umbrellas. Full list:
`THIRD-PARTY-NOTICES.md`.

**Scope note (audit B3-8):** the `phorj-playground` workspace member additionally uses
`wasm-bindgen` + `serde_json` for the browser boundary — build-target tooling for the playground
artifact, outside the core policy's four-plus-two domains; recorded here so the dependency surface
is stated completely.

### 2026-07-03 amendment (developer-ruled)

**Admitted domains #5 and #6 — SQLite (`rusqlite`) and TLS (`rustls`)** — the corosensei/ctrlc
shape: native-only, feature-gated, quarantined from the byte-identity spine. Gating both W3-1
(DB access) and W3-2 (HTTP client); **the crates enter the tree with those builds** (not yet present
at HEAD). Companion rulings: the DB layer is a multi-driver **SQL DBAL** (PDO/Doctrine-DBAL analog):
SQLite (P1) + Postgres (`postgres` sync) + MySQL/MariaDB (`mysql` sync) — ALL sync, no tokio;
**Oracle deferred** (closed Instant Client → clause 2 fails); **MongoDB is a separate LADDER item**
(non-SQL, no PDO analog → native-only `E-TRANSPILE-MONGO`; async-driver problem) requiring its own
future design. Both W3-1/W3-2 ship a pure zero-dep P0 first (`Core.Sql` Tier-A value; `Core.Url`).
Design drafts: `docs/research/wave3-4-drafts/`.

### Process to admit the next one

(1) a table entry above with clause-by-clause justification; (2) a `CHANGELOG.md` note;
(3) feature-gating verified against the playground build.

## PHP extension tiers

**Status: the core rule is IN FORCE (since `0bb620b`); the tier-3 declaration/guard mechanism is
DESIGNED, NOT IMPLEMENTED (lands with the first tier-3 module).** Source:
`2026-06-19-extension-policy-design.md`.

### Problem

Phorj's transpile contract — every feature maps to **idiomatic PHP that runs anywhere** — was
silently violated: `Core.Bytes.toString` emitted `mb_check_encoding` (mbstring), which is *usually
present but not guaranteed*. The correctness oracle runs **`php -n`** (no ini ⇒ shared-module
extensions absent), and minimal real-world PHP (Alpine, hardened containers) ships without mbstring.
The example passed locally (statically-compiled mbstring survives `-n`) and fataled on CI — a
statically-linked local extension **masks** the portability gap entirely. The deeper issue: no
policy existed for which PHP functions emitted code may use.

### The tiers

| Tier | Examples | Availability | Phorj stance |
|---|---|---|---|
| **1 — always-compiled** | `Core`/`standard` (`strlen`, `substr`, `str_*`, `intdiv`, `explode`…), PCRE (`preg_*`), `json_*` | Every PHP; survives `php -n` | **Allowed in core stdlib** |
| **2 — default-but-removable** | mbstring, ctype, tokenizer, fileinfo | Usually present; absent under `php -n` / minimal builds | **Forbidden in core stdlib** — pick a tier-1 equivalent |
| **3 — genuinely optional** | gd, curl, intl, pdo_* | Installed deliberately | **Allowed only in an extension-bound module that declares + guards it** |

Tier-2 is the trap — "works on my machine" is precisely its failure mode. The rule collapses it
away: core targets tier-1; anything beyond is tier-3 and must be explicit. In force concretely:
UTF-8 validity → `preg_match('//u', $s) === 1`, **not** `mb_check_encoding`; string length/slice →
`strlen`/`substr` (byte semantics). The known tension — codepoint-true Unicode string semantics want
mbstring — is the W4-4 design question (its case-folding divergence from the `php -n` oracle is a
LADDER-quarantine candidate).

### The tier-3 mechanism (proposed; for `Core.Image`/intl-class modules later)

Three coordinated pieces make a genuine extension need honest: (1) **declare** in `phorj.toml`
`[require]` using Composer's own vocabulary (`ext-gd = "*"`); (2) **preflight guard** in emitted
PHP — `if (!extension_loaded('gd')) { fwrite(STDERR, …); exit(1); }` — a clean diagnosable exit,
never an undefined-function fatal mid-run; (3) **transpile-time gate** — a `// requires: ext-gd`
header + `--php-target=baseline|full` where `baseline` (default/CI) rejects tier-3 use at transpile
time. Also proposed: a denylist transpile-scan regression test (transpile every example, assert no
`mb_*`/`ctype_*`/`gd_*`/`curl_*` in output) — the static analogue of the value oracle. Non-goals: a
Cargo-feature matrix for Phorj's own build (YAGNI); vendoring PHP extensions; touching `interpreter ≡ VM`.

## PHP parity and beyond gap audit

**Status: HISTORICAL (2026-06-21/22) — the definitive 20-track gap audit that seeded the roadmap.
Superseded as a plan by `docs/plans/MASTER-PLAN.md` (which executed its verdicts — see the P-plan
ledger, MASTER-PLAN §12); the decision register `docs/research/full-audit/raw/C-decisions.md` is the
canonical rulings record. Preserved here: the philosophy lens, the verdict vocabulary, the ratified
error-model decision, the rejection catalogue with reasons, and the cross-track themes — the parts
with enduring design value. The full ~800-row triage table remains in the source file (the largest
of the 18; its per-row statuses are heavily stale — many "adopt" rows have since SHIPPED: match-
position, decimal, LSP, formatter, lift, serve, set algebra, traits-construct, statics, secrets…).
Its closing "GA ~72% · Global ~58%" figures are obsolete; the live model is
`docs/research/full-audit/raw/M-gap-matrix.md` §4.** Source: `2026-06-21-php-parity-and-beyond.md`.

### The philosophy lens (authoritative, quoted by later work)

Every candidate is judged by: *a pragmatic, legible, provably-correct upgrade of PHP; the
relationship TypeScript has to JavaScript.* Familiarity-first IS the adoption strategy. Phorj
removes **surprises**, never **capability**. Every feature must map to idiomatic PHP (PHP-absent
features are compile-time-only, erased before the backends, preserving the byte-identity spine).
The filter is **"what is the most PHP-familiar, legible, pragmatic form of this?"** — not "what is
the most powerful?". PL-theory maximalism that doesn't earn its surprise budget is rejected.

**Verdict vocabulary** (reused by later audits): `kind`: `port` (a PHP feature we lack) / `new`
(beyond-PHP) / `map` (maps to a shipped feature or a doc/emission refinement) / `omit` (PHP
capability deliberately reshaped). `rec`: adopt / defer / reject. `fit`: strong / ok / weak.

### The ratified error model (DECIDED 2026-06-22, developer, locked)

**Three tiers, one enforced-failure principle:**
1. **`throws E`** — enforced, *typed* exception declaration (the fix to PHP's unchecked `@throws`
   docblock), checker-enforced at the call site, `?`-propagable, **specific error type required**
   (no bare `throws Exception` swallow), transpiling to idiomatic PHP exceptions. The PHP-familiar
   *default* surface.
2. **`Result<T, E>`** — error-as-value (functional, `match`/`?`), transpiling to a PHP value; for
   data-flow / `?`-chain code.
3. **Unchecked faults/panics** — programmer bugs / invariant violations (index-OOB, force-unwrap-
   null) that *crash* with a stack trace, never declared up the call chain — **the explicit fix to
   Java's "everything is checked" mistake**.

Both checked tiers are typed + checker-enforced + `?`-composable; `throws` erases before the
backends (front-end-only ⇒ byte-identity-safe, no new `Op`). `try/catch` handles the `throws`
surface and the imported-PHP interop bridge.

### The rejection catalogue (with reasons — the enduring negative space)

**Dynamic-PHP footguns (defeat static checking — the exact surprise Phorj removes):**
`__get`/`__set`/`__call` magic; `compact`/`extract`/`$$x`; function-`static` + `global`;
`isset`/`empty` truthiness predicates; `&$x` references (contradicts the value/handle split);
`(int)` cast operators (named conversion functions instead); C-style `switch` (fall-through —
`match` covers it).

**No deterministic PHP target / breaks the spine:** operator overloading (hidden `__add`
action-at-a-distance; derived `equals`/`compareTo` cover the pragmatic slice); guaranteed TCO (a
recursive program succeeding under TCO fails under transpiled PHP); async/await (colored functions
contradict uncolored `spawn`); algebraic effects; reactive signals; `__destruct` (`Rc`/`Drop` has no
deterministic finalization).

**Cannot honor the `php -n` oracle:** ICU collation/transliteration (no tier-1 approximation);
other ICU-locale features defer to the tier-3 policy rather than reject.

**PL-theory maximalism (overruns the surprise budget for a PHP audience):** solver-backed refinement
types (newtypes cover the slice); units-of-measure; typestate; GADTs/declared variance (erased
generics are invariant by design); open proc-macros (the *closed* derive channel is the answer);
lazy sequences/fibers (fight the eager-array transpile target); a Rust-style borrow checker
(narrowed v2 goal: a cycle collector if needed); structural "shapes" (clash with nominal identity);
reflection-based mocking (interface fakes are legible); taint tracking (strictly dominated by
by-construction `Secret`/`Html`/parameterized SQL); OTel-style tracing machinery.

**Reverses a deliberate decision / over-scoped for a single dev pre-1.0:** hosted package registry
(M5 chose git+vendor+offline); FaaS adapters; LTS backports; live PHP FFI; importing dynamic PHP
(`eval`/`$$x` — un-importable into a closed no-`eval` language); gradual/optional typing
(`allowJs`-style `mixed` holes punch through the static spine — decl-files + import is the Phorj
answer); versioned/i18n/video docs.

### Cross-track themes (the programmes, not rows — still the best strategic summary)

1. **The error model is the keystone fork** — resolved (above); Result-first with try/catch as the
   PHP-interop bridge.
2. **Generics aren't done until enums are generic** — `Result`/`Option`/typed containers all ride
   `erase_generics` (since shipped).
3. **Narrowing completeness is "provably-correct upgrade" made concrete** — flow narrowing + union
   exhaustiveness + equality refinement + sealed hierarchies as one programme, paired with
   return-totality (the audit's #1 soundness leak, since fixed).
4. **The stdlib must become a *product*, not an accretion** — a written charter precedes the breadth
   push (delivered: the [stdlib charter](#standard-library-charter)); the "Hack HSL was the killer
   feature" lesson.
5. **Determinism quarantine is the universal mechanism for impure batteries** — random, clock, env,
   network, process all break the spine the same way; one seam (exclusion from `differential.rs` +
   seedable/injectable inputs) unblocks all of them AND makes user tests deterministic.
6. **Lexer-only ergonomics are free wins** — numeric separators, base literals, exponents, `\u{…}`:
   front-end-only, byte-identical by construction, pure familiarity.
7. **Tooling-as-adoption-lever must be sequenced** — fmt first (it de-risks the AST printer LSP
   needs), then scaffold/completions, then LSP, then editor clients, then doc-gen; the test runner
   is the biggest ecosystem table-stake (all since shipped except doc-gen).
8. **Governance/stability is cheap docs, GA-blocking, and a genuine PHP upgrade** — semver +
   breaking-change definition + a *frozen conformance corpus* (Phorj can state BC *provably* via the
   byte-identity spine — PHP can't) + stable diagnostic codes + an honest differentiation statement
   (don't claim speed). Editions: policy at GA, build post-1.0.
9. **Incremental adoption is the whole thesis** — the TypeScript-beat-Hack lesson: decl-files,
   codemod, migration report, mixed projects, and the deploy direction (front-controller, PHAR,
   `--php-target` floor) must be first-class tested workflows.
10. **Clusters of deferred corners are one mechanism each** — union follow-ups, mutation corners,
    transpile hazards: bundle by shared fix, don't track ~12 independent rows.

---

# Part V — Build & distribution (M2.5)

> **Dependency-claim correction (applies to all three sections below):** these specs predate the
> [dependency policy](#external-dependency-policy) and describe the artifact as "std-only /
> zero-dependency". The accurate current framing: the **hand-rolled object-format readers, container,
> CRC-32, SHA-256 stay std-only by policy** (no `object`/`goblin`/`sha2` in code that runs inside the
> artifact), while the *crate as a whole* ships the four vetted feature-gated deps. The
> tooling-exemption principle (§ boundary below) is unchanged and remains the governing test.

## phg build master design

**Status: ADOPTED architecture — Phases 1–3a SHIPPED (see the two sections below); Phase 3b
(signing + macOS stubs) DEFERRED.** Source: `2026-06-16-m2.5-phorj-build-design.md`.

### Goal

`phg build foo.phg` produces a self-contained executable that runs `foo.phg` on the VM with no
Phorj install on the user's machine — Linux (gnu+musl, x86_64+aarch64), Windows (x86_64), macOS
(x86_64+aarch64). Non-goals at design time: argv/exit-code surface (later shipped as language
features), multi-file bundling pre-M5, replacing the transpiler.

### The unifying decision — payload is a named **section**, never an appended overlay

A raw overlay (bytes after EOF + footer) works on ELF/PE but is a **dead end on Mach-O**: arm64
macOS mandates a code signature at exec, the signature must be the last content in the file, and
anything appended after it invalidates it. Therefore: **the payload is always a named section in the
host object format** — `.phorj` on ELF and PE/COFF, `__PHORJ,__source` on Mach-O — added with
`llvm-objcopy --add-section`. A section lives *inside* the signed region, so one mechanism is
Authenticode- and `codesign`-compatible everywhere. The invariant is **section-first, sign-last**.
One uniform model: *locate a named blob inside my own image → validate → run it*; only retrieval
(object-format parsing) differs per OS, and that reader is hand-rolled std.

### The payload container (forward-compatible)

A **versioned, CRC-guarded container**, not raw source — so "embed source" can become "embed
bytecode", gain argv/exit metadata, or add compression without a format break. Little-endian:

```
 off  size  field
   0     8  magic             = "PHORJ\0\0"
   8     2  container_version = 1
  10     2  header_len        = 32     (may grow; old readers skip unknown tail)
  12     1  payload_kind      0=source_utf8  1=bytecode(future)
  13     1  compression       0=none  1=deflate(future)  2=zstd(future)
  14     1  encryption        0=none
  15     1  flags             bit0=has_argv_spec  bit1=has_exitcode
  16     8  payload_len (u64)
  24     4  payload_crc32
  28     4  header_crc32      (of bytes [0..28))
  32   var  payload
```

Reader contract: locate section → absent ⇒ behave as the normal CLI (fall-through) → check magic →
`header_crc32` (guards against trusting a garbage `payload_len`) → bounds-check → `payload_crc32` →
refuse newer `container_version` → dispatch on `payload_kind`. Any failure **falls through to the
normal CLI; never panics**.

**v1 payload = the `.phg` source text.** The binary already contains the full
lex→parse→check→compile→VM pipeline, so it re-runs the source at startup (~17µs — negligible). This
avoids a fragile hand-rolled bytecode serializer that would track every `Op`/`Value`/desc change —
a *fourth* coupled match surface beyond the three in `docs/INVARIANTS.md`. Source→bytecode is later
a `payload_kind` flip, not a format change. Documented limitation: the source is recoverable from
the artifact (acceptable v1; bytecode raises the bar later).

### Orchestration — the stub-registry model

`phg build` does not invoke a compiler at build time in the end state: CI builds one **runtime
stub** per target per release (a phorj with no embedded section), publishes them with a hash
manifest **baked into the phg binary**; `phg build` validates the program first (fail with
diagnostics, emit nothing), fetches the stub from cache-or-registry, embeds the container, `chmod
+x`. The **host-target stub is the running binary itself**, so building for your own OS is offline
from first run. Optional `--sign` shells out to platform signers with user credentials (default =
runnable unsigned; ad-hoc on macOS so it launches). **Signing without a Mac:** `rcodesign` performs
macOS sign + notarize + staple *from a Linux runner*. Because the embedded payload changes the file
hash, **CI cannot pre-sign final PE/Mach-O artifacts** — CI signs stubs; final-artifact signing is
the opt-in `--sign` step.

**Cache key includes the phg binary's own hash**, not just the triple — otherwise a stale stub
embeds your source into an **old VM**, silently breaking the parity spine (`docs/INVARIANTS.md` #1)
at the distribution layer. This is decision B-6, load-bearing across all three phases.

### The std-only ethos boundary

*The produced binary and the runtime code stay std-only; everything that builds, embeds, signs, or
ships an artifact is host build-tooling and exempt.* Inside the line: the VM, the hand-rolled
object-format readers, container + CRC-32 (+ later SHA-256). Outside: `zig`/`cargo-zigbuild`,
`llvm-objcopy`, `rcodesign`, `osslsigncode`, CI, `curl`. **Test: does it end up linked into the
artifact or its runtime?** No → exempt. Watched leakage risk: pulling `object`/`goblin` in to *read*
the section (that code runs inside the artifact) — forbidden; the reader is hand-written std.

### Master decisions log

| # | Decision | Choice | Rationale |
|---|---|---|---|
| B-1 | Embedding | Named section, not overlay | Only sections are signing-compatible on Mach-O; uniform across formats |
| B-2 | Payload (v1) | Source text | Full pipeline already in the binary; avoids a 4th coupled match surface |
| B-3 | Container | Versioned + dual-CRC + reserved fields | Forward-compat: format evolution = value flips, not breaks |
| B-4 | Section tooling | `llvm-objcopy` (rustup `llvm-tools-preview`) | One tool, all three formats, from Linux |
| B-5 | Orchestration | Stub registry (CI builds/signs once per release) | Instant builds, offline-once-cached, signing-ready (bun/deno model) |
| B-6 | Cache key | Includes the phorj **binary hash** | A stale stub breaks the parity spine at distribution |
| B-7 | macOS signing | `rcodesign` from Linux | No Mac in the pipeline; Notary API is HTTPS |
| B-8 | Final signing | Opt-in `--sign`; default runnable unsigned | Payload changes the hash ⇒ CI can't pre-sign finals |
| B-9 | std-only boundary | Artifact/runtime hand-rolled; build tooling exempt | "Does it link into the artifact?" |
| B-10 | Phasing | ELF-only P1 → cross P2 → CI+signing P3 | Each phase a strict subset of one architecture; no rework |
| B-11 | argv/exit | Not in v1 (documented) | No language surface existed yet (since shipped) |

## Phase 2 cross-OS builds

**Status: SHIPPED (v0.4.0) — Linux (x86_64-musl, aarch64-gnu/musl) + Windows (x86_64-gnu) cross
builds; Mach-O/fat readers shipped fixture-tested, macOS stub production deferred to 3b.** Source:
`2026-06-16-m2.5-phase2-cross-os-design.md`.

### Module structure — `bundle.rs` → `bundle/`

Phase 2 roughly tripled the module; a single 700+-line file mixing three binary formats, CRC logic,
and subprocess orchestration does too much. Split into focused, independently-testable units:
`mod.rs` (public surface) · `container.rs` (magic/CRC/encode/decode, moved verbatim) · `section.rs`
(`find_section` — the magic-sniffing dispatcher) · `elf.rs` / `pe.rs` / `macho.rs` (per-format
minimal section lookup) · `cross.rs` (orchestration + cache; named to avoid confusion with Cargo
build scripts).

`find_section` sniffs leading magic (`7F ELF` / `MZ` / `CF FA ED FE` Mach-O-64-LE / `CA FE BA BE`
fat-BE) and delegates; unknown magic → `None` → normal CLI, never panics. All readers do **minimal
section lookup, not full parsing**, with `checked_add`/`checked_mul` on every offset — adversarial
input returns `None`, never overflow-panics (EV-7). The per-format field-offset walkthroughs (PE:
`e_lfanew`→COFF header→40-byte section headers matching `.phorj`; Mach-O: load-command iteration to
`LC_SEGMENT_64`/`__PHORJ,__source`; fat: BE header, recurse per slice) live in the source file and
in `bundle/*.rs` itself — each offset pinned by a fixture during TDD. `embedded_source()` dispatches
via `find_section` on the *running image's own* format — which is why all reader arms shipped even
though the macOS stub is deferred.

### Cross-build orchestration & the stub cache

No CI in Phase 2 ⇒ **build the stub locally on demand, then cache** — the natural precursor to
Phase 3's download-and-cache (same cache, same embedding). Driver: **`cargo-zigbuild`** + zig
(pinned 0.16.0) — it owns the rustc→zig linker plumbing, glibc floor pinning, and windows-gnu/musl
link config; preferred over a bespoke `zig cc` wrapper. Stub builds run with
`RUSTFLAGS=--cap-lints=warn` **on the subprocess only**, so the tracked `[lints]
warnings = "deny"` can't fail a cross build on a target-specific lint without editing the manifest.
Cross targets require a **source checkout** in Phase 2 (a distributed binary errors precisely,
pointing at Phase 3); the host build reuses the running binary and needs no source. Missing rustup
target → a precise `rustup target add <T>` error, not a cryptic cargo failure.

Cache layout: `${XDG_CACHE_HOME:-~/.cache}/phorj/stubs/<key>/<triple>/phg[.exe]`, `<key>` =
**FNV-1a-64 of the running phg binary's own bytes** — any source/VM change ⇒ different bytes ⇒
cache miss ⇒ rebuild (B-6). FNV, not SHA-256, deliberately: a cache key is not a security boundary;
a collision's blast radius is local + recoverable (P2-3). (Contrast Phase 3a, where download
integrity IS a security boundary and gets a real hash.)

### Phase-2 decisions log (kept — several are trap-documenting)

| # | Decision | Choice / lesson |
|---|---|---|
| P2-1 | Scope | Linux+Windows real stubs now; macOS reader-ready/stub-deferred (needs `rcodesign` + a macOS SDK) |
| P2-2 | Structure | `bundle/` split per format behind one `find_section` |
| P2-3 | Cache key | FNV-1a-64 of host phorj bytes (identity, not security) |
| P2-4 | Provisioning | Local on-demand `cargo-zigbuild` + cache |
| P2-5 | Driver | `cargo-zigbuild` + pinned zig |
| P2-6 | Lints | `--cap-lints=warn` on the subprocess only |
| P2-7 | Outputs | host/single → `-o` or `./<stem>[.exe]`; `--all` → `dist/<stem>-<target>[.exe]` |
| P2-8 | Robustness | Checked arithmetic everywhere; endianness-explicit readers (LE Mach-O bodies vs BE fat headers is the trap) |
| P2-9 | Sourceless cross | Precise error pointing at Phase 3 |
| P2-10 | `--all` host naming | `dist/<stem>-<host-triple>` (resolved from `rustc -vV`), never a literal `-host` |

**Resolved risk worth remembering (F5):** `llvm-objcopy --set-section-flags <n>=noload,readonly` is
**required on PE**, not cosmetic — without it, `--add-section` writes a section header with zero raw
data and the embedded program is silently lost. An earlier attempt to *skip* the flags on PE (on an
unverified theory) was itself the bug; the flags apply unconditionally on ELF+PE, verified by the
tier-2 Windows round-trip against a real `cargo-zigbuild` PE — fixture tests could not catch this
(they don't invoke `llvm-objcopy`). **Standing caveat:** the Mach-O/fat readers have only
synthetic-fixture validation — fixture leakage (a self-consistent-but-wrong offset shared by fixture
builder and reader) would pass unit tests yet fail on a real Mach-O; the deferred macOS-stub session
MUST re-verify offsets via a real tier-2 dump round-trip before shipping Mac binaries.

Test strategy (3 tiers, all still the pattern for this area): (1) offline synthetic-fixture reader
tests incl. adversarial truncation/overflow/wrong-endianness; (2) toolchain-gated real-binary
round-trips (`cargo-zigbuild` → embed → `--dump-section` → decode == source), graceful skip when
tooling is absent; (3) native execution where the host can run the artifact (musl on this box ⇒
full byte-identity vs the VM across the gnu→musl libc boundary).

## Phase 3a stub registry

**Status: SHIPPED (2026-06-28) — `bundle/sha256.rs`, `bundle/manifest.rs`, `build.rs`, the
`cross.rs` 3-way branch, CI as `.github/workflows/stub-registry.yml` (the spec's `release.yml` name
was taken by the human-archive workflow). Phase 3b (signing + macOS stub) remains DEFERRED.**
Source: `2026-06-17-m2.5-phase3a-stub-registry-design.md`.

### Goal & the seam

A **distributed** phg binary (no source checkout) can cross-build by **downloading a prebuilt stub**
from a CI registry, verifying it against a **baked sha256 manifest**, caching it, and embedding —
closing Phase 2's P2-9 limitation. `build_stub`'s miss path becomes a 3-way branch: cache hit →
return; `Cargo.toml` present → build locally (Phase 2, unchanged); else → `download_stub`.
Everything downstream is unchanged — a downloaded stub is interchangeable with a locally-built one.
The host build never downloads.

### Download-and-cache client

manifest lookup (miss → precise "no prebuilt stub… needs a source checkout" error) → resolve base →
fetch to a **temp file in the same directory** (same-fs rename) → **verify sha256 on the temp file**
→ only then atomic-rename into the cache. A corrupt/tampered/partial download never poisons the
cache; the cache stays keyed on the phorj-hash path, so a rebuilt phorj re-downloads (B-6). All
failure modes are precise, embed nothing, exit 1 — the sha256-mismatch message ("refusing to embed")
is the parity-spine refusal.

**Transport:** std has no TLS, so HTTPS shells out to **`curl`** (`-fSL --proto =https,http`) —
host tooling, exempt exactly like zig/objcopy; `PHORJ_CURL` override mirrors `PHORJ_OBJCOPY`.
`file://`/local paths use `fs::copy` — the **hermetic test seam** (fixture-dir registry, no network,
no curl). Registry base defaults to `{CARGO_PKG_REPOSITORY}/releases/download/v{version}/`
(single-sourced via the `repository` field); `PHORJ_STUB_REGISTRY` overrides. Asset name
`phg-stub-<triple>[.exe]` is a constant shared by client and CI (a rename touches both in one
commit).

### Integrity — why a real hash here (unlike the cache key)

The artifact is an **executable**, and a wrong stub silently embeds your source into a mismatched
VM — breaking the parity spine at the distribution layer. Integrity is a real security boundary
(unlike the FNV cache key, which is identity-only): **`bundle/sha256.rs`** is a hand-rolled
FIPS 180-4 SHA-256 (~70 lines std, known-vector tested) — same hand-rolled-std ethos as the CRC-32,
honoring the reader-side std-only line. The manifest is tolerant line-based text
(`<triple> <sha256-hex>` + optional `version` sanity line); verified downloads must match exactly.

### Manifest baking — the circularity break (P3-3, the clever bit)

A stub IS a phg binary; if the manifest were compiled into *every* binary, a stub's bytes would
depend on the manifest whose entries are the hashes of those bytes — an unsolvable fixpoint.
Resolution: **`build.rs` + `PHORJ_BAKE_STUB_MANIFEST`** — set (CI's primary build only) → bake the
file; unset (every other build, including all cross stubs) → bake an **empty** manifest. So cross
stubs are manifest-independent ⇒ stable hashes ⇒ no fixpoint; dev builds get an empty manifest but
have `Cargo.toml` anyway (build locally — correct by construction); only the `x86_64-linux-gnu`
primary carries the manifest (P3-7 — other hosts get the clear source-checkout error). Runtime
`PHORJ_STUB_MANIFEST` overrides the baked one — the test seam. *Rejected alternatives:* a committed
`include_str!` file CI rewrites (parks release data in git; manual circularity discipline);
post-build manifest-section injection (lives in a strippable section, not `.rodata`).

### CI (2-pass, no secrets)

Pass 1: matrix-build the 4 cross stubs (`cargo-zigbuild`, env unset). Pass 2: hash them with host
`sha256sum`, write the manifest, build the primary with the env set, publish stubs + primary as
release assets with `GITHUB_TOKEN`. The client verifies with its own hand-rolled SHA-256 — a
built-in **cross-implementation check** (if `sha256sum` ≢ the hand-rolled hash, verification fails).
Unsigned stubs need no certs; that is exactly why 3a/3b split (P3-1): signing without certs would be
unverifiable scaffolding — YAGNI on cert-gated code.

### Phase-3a decisions log

| # | Decision | Choice |
|---|---|---|
| P3-1 | Split | 3a registry only (complete, verifiable); signing + macOS stub → 3b when certs/SDK exist |
| P3-2 | Integrity | Baked sha256 manifest + hand-rolled std SHA-256 (a real security boundary) |
| P3-3 | Baking | `build.rs` + env; empty when unset (automatic circularity break) |
| P3-4 | Dev vs distributed | `Cargo.toml` presence (reuses P2-9) |
| P3-5 | Transport | `curl` (exempt host tool) + `fs::copy` for local (hermetic tests) |
| P3-6 | Registry base | `CARGO_PKG_REPOSITORY` + version; env override |
| P3-7 | Manifest reach | Baked only into the x86_64-linux primary in v1 |
| P3-8 | CI | GitHub Actions, 2-pass, no secrets |

---

## Appendix A — source-file map and supersession chains

| Original file (in `archive/`) | Section here | Status |
|---|---|---|
| `2026-06-15-phorj-language-design.md` | [Founding language design](#founding-language-design) | HISTORICAL (vision stands; surface details superseded inline) |
| `2026-06-15-ecosystem-roadmap-design.md` | [Ecosystem strategy](#ecosystem-strategy) | HISTORICAL (strategy stands; milestone table → MASTER-PLAN) |
| `2026-06-16-m2.5-phorj-build-design.md` | [phg build master design](#phg-build-master-design) | ADOPTED; P1–3a shipped, 3b deferred |
| `2026-06-16-m2.5-phase2-cross-os-design.md` | [Phase 2 cross-OS builds](#phase-2-cross-os-builds) | SHIPPED (v0.4.0) |
| `2026-06-17-m2.5-phase3a-stub-registry-design.md` | [Phase 3a stub registry](#phase-3a-stub-registry) | SHIPPED (2026-06-28) |
| `2026-06-19-core-html-design.md` | [Typed auto-escaping HTML](#typed-auto-escaping-html) | SHIPPED (all waves; names post-overhaul) |
| `2026-06-19-extension-policy-design.md` | [PHP extension tiers](#php-extension-tiers) | Rule in force; tier-3 mechanism designed-not-built |
| `2026-06-21-php-parity-and-beyond.md` | [PHP parity and beyond gap audit](#php-parity-and-beyond-gap-audit) | HISTORICAL; verdicts executed via MASTER-PLAN §12; row statuses stale in the original |
| `2026-06-27-dependency-policy.md` | [External dependency policy](#external-dependency-policy) | ADOPTED + 2026-07-03 amendment (rusqlite/rustls domains) |
| `2026-06-28-public-surface-file-rule-design.md` | [Public-surface file-naming rule](#public-surface-file-naming-rule) | SHIPPED |
| `2026-06-28-secret-type-design.md` | [Secret type](#secret-type) | SHIPPED |
| `2026-06-28-statics-research-design.md` | [Comprehensive statics](#comprehensive-statics) | A+B SHIPPED; LSB deferred non-feature (original header stale) |
| `2026-06-29-m4-stdlib-charter.md` | [Standard library charter](#standard-library-charter) | ADOPTED, governing |
| `2026-06-30-naming-overhaul-design.md` | [Naming overhaul](#naming-overhaul) | SHIPPED (naming SSOT; W2-9 re-verifies remainders) |
| `2026-07-01-import-roots-psr4-design.md` | [Import roots and PSR-4 mapping](#import-roots-and-psr-4-mapping) | DESIGNED; **pre-dates unified import — re-base before build (W2-7)** |
| `2026-07-01-nested-value-index-assign-design.md` | [Nested-value index-assignment](#nested-value-index-assignment) | SHIPPED |
| `2026-07-01-no-wind-namespace-and-language-surface-design.md` | [Nothing in the wind](#nothing-in-the-wind) | Principle in force; import mechanics superseded by 2026-07-03 model (function-import decision REVERSED); intrinsics/de-reservation pending (W2-6) |
| `2026-07-03-unified-import-and-injected-type-discipline.md` | [Unified import and injected-type discipline](#unified-import-and-injected-type-discipline) | ADOPTED — the CURRENT import model; S0–S2 SHIPPED |

**Supersession chains (explicit):**
- Import surface: M5 `import`/`import type` → *no-wind* deep-import design (2026-07-01) →
  **unified import + injected-type discipline (2026-07-03, CURRENT)**. `import type` no longer
  parses; functions are NOT bare-importable (reversing the no-wind corollary for functions).
- Roadmap: ecosystem-roadmap milestones (2026-06-15) → php-parity rollup (2026-06-21) →
  **`docs/plans/MASTER-PLAN.md` (CURRENT SSOT)**.
- Dependency posture: "zero external deps" (pre-2026-06-27, now false everywhere it survives) →
  **dependency policy (2026-06-27) + amendment (2026-07-03)**.
- Naming: founding names → **naming overhaul (2026-06-30, SSOT)**; `->` return/fn-type syntax →
  `: T` / `(A) => B` (W2-4 parser-reject pending; the planned rejection code `E-RETIRED-SYNTAX`
  does not exist yet).

# In-language vs Externalize Audit (2026-07-13)

> Developer directive (META-6): "critical thinking for each feature — does it live in the language or
> should it be externalized." The language is RICH (does everything PHP does, better/faster/safer/
> secure) + zero-cost safe sugar, but NOT bloated — library/packaging concerns are externalized.
> Lens per item: **KEEP-CORE** (beats-PHP capability, needs a native, or a zero-cost primitive) ·
> **SUGAR** (zero-cost safe sugar — compile-time/erased) · **EXTERNALIZE** (library / separate tool).
> Certification self-graded (advisor inactive, META-5). Raw per-item detail: `scratchpad/audit-*.md`
> (stdlib / appdomain / features / tooling). This doc consolidates the four sweeps + the open rulings.

## KEEP-CORE / SUGAR (principled — stays in the language)
- **Stdlib primitives:** Output, String, List, Map, Math, Option, Result, Conversion, Bytes, Decimal,
  Hash, Random, Regex — native/perf-critical or foundational error/safety vocabulary.
- **App primitives (needs a Rust native / PHP-core capability):** Cryptography, File, Path, Process,
  Environment, Reflection (the L1 primitive), Runtime, Url, Secret, Db (enhanced-PDO — DEC-208), Csv, Ini.
- **Language capabilities:** erased generics + `T:Interface` bounds (DEC-211), user attributes + L1
  reflection, overflow-checked-default + `#[UncheckedOverflow]`, `as` checked cast, enums/ADT + match
  (+ DEC-209), null-safety (`??`/`?.`/`!`/`?`), traits + multiple inheritance, sealed hierarchies,
  concurrency, the tagged-template primitive (DEC-212).
- **Zero-cost sugar:** mandatory `new`, mandatory `this.`, `using`/Closable (DEC-203), string interp,
  `with`, `|>`, type aliases, expr-`if`, ctor promotion, `var`, DEC-209 `default`.
- **Tooling that IS the language/compiler:** transpile (byte-identity spine), disassemble, test,
  benchmark, format, explain (mechanism), build (mechanism), the loader module-system.

## EXTERNALIZE candidates (ranked — the "should be library / separate tool" list)
1. **Package management** (`phg vendor` + `phorj.toml` + manifest/lock, ~1255 LOC) — **DEC-216 PENDING**
   (separate tool). Cleanest, off the run/check path.
2. **Http** (Router/middleware/groups, `.phg` framework in preludes) → userland; **keep** the
   `Request.parse(bytes)` / `Response.serialize()` primitive + the `phg serve` respond hook.
3. **DI** (`desugar_di/`, 1292 LOC compiler pass) → **DEC-215** (L1 attribute primitive + DI as L2 library).
4. **desugar_router** (`desugar_router.rs`, 489 LOC — `#[Route]`/`autoRouter` compiler pass) → **NEW
   finding: a peer to DI**, same DEC-215 L1/L2 treatment (a web-framework pass baked into the compiler).
5. **serve** (959 LOC) → separate (application HTTP; precondition: expose socket natives to userland).
6. **lift** (PHP→Phorj, 5005 LOC) → separate migration tool (biggest binary win; near-zero core coupling —
   owns its own lexer/parser/AST/printer).
7. **lsp** (1629 LOC) → separate (needs a checker-as-library API first).
8. **Time** (~130-line pure-`.phg` calendar prelude) → library; **keep** the clock native seam.
9. **Validation** (isInt/isAlpha/… string predicates) → library (no primitive/perf/safety case).
10. **html domain** → **DEC-212** (library on the tagged-template primitive). Already ruled.
11. **Dotenv, Event, Cli, Log, Uuid, Sessions, Serde, Template** → userland; keep thin primitives
    (File/Environment/Secret; Crypto+cookie for Sessions; L1-reflection derive for Serde; tagged-template
    for Template; Core.Random for Uuid).
12. **debug/DAP** (727 LOC) → separate (debugger norm; low ROI, VM-coupled).
- Borderline (weaker signal): Encoding (base64/hex over Bytes), Set, Csv/Ini (thin).

## OPEN ADJUDICATIONS (developer's call — to be ruled with ladders + previews)
- **DEC-217 (Test framework in/out):** genuine tie — PHPUnit is PHP *userland* (externalize) vs
  Rust/Go ship a *built-in* test runner (keep). Recommend surfacing with both precedents.
- **DEC-218 (externalize delivery destination):** userland (DEC-208 style, no curated lib) vs first-party
  bundled lib (html/DEC-212 style). **Interacts with DEC-216** — if package management is removed, a
  "userland" web spine has no distribution path, so DEC-216 and DEC-218 must be ruled together.
- **DEC-219 (overloading dispatch):** resolve statically where argument types are known (zero-cost) vs
  the current runtime multiple-dispatch (per-call cost). A META-6 zero-cost-sugar tension.
- **Ranges (perf note, not a ruling):** `a..b` materializes a `List<int>` — zero-cost only on the JIT
  (range accumulator); VM/interpreter/transpile allocate (PHP `range()` parity). Keep the accumulator
  firing on every range-for; add a doc note.

## Sequencing note
These are DESIGN candidates, not scheduled work. Ruling order suggestion: DEC-216 + DEC-218 together
(packaging + delivery destination — they gate every other externalization's distribution), then the
DEC-215 framework-pass family (DI + desugar_router via the L1 primitive), then the per-module moves
(Http → primitive+userland, Time/Validation → library), then DEC-217 (Test) and DEC-219 (overloading).
Every move is a tracked, tested, register-recorded slice — nothing externalized silently.

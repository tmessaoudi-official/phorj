# twes-in as phorj's second forcing function — a framework app

> **Developer directive, 2026-09-27:** *"explore /stack/projects/twes-in/api and use it like scout to
> prepare for what phorj should be able to do!"* — with the standing priority: *"offer the maximum of
> features that are visible and usable without compromising any security/performance/architecture/best
> practices."*
>
> **twes-in is READ-ONLY.** Nothing in this campaign writes to `/stack/projects/twes-in`. Every lift and
> census runs on a copy in a scratch directory. It is the second yardstick beside scout
> (`2026-09-07-scout-forcing-function.plan.md`), and a different kind: scout has zero composer runtime
> dependencies, twes-in is a **Symfony 8.1 + API Platform 4 + Doctrine ORM 3** invoicing API — 1 037 source
> files / 68 296 lines in `src/` (2.3× scout's source), 294 test files, 181 composer packages / 11 427 PHP
> files in `vendor/`. Where scout measures the LANGUAGE, twes-in measures how phorj meets a FRAMEWORK app.
>
> This plan is the record of truth (Invariant 19). `SLICE-STATE.md` is the live cursor; the rulings are
> DEC-551…553 in the register.

---

## Decisions Log

- [2026-09-27 17:12] AGREED: **ORDER — impact-first** across both forcing functions: (1) this plan's two
  P0s (T1, T2 — the lifter emits drafts that do not parse); (2) scout row 4l, PCRE (`Core.Regex` for users,
  ≈59 of scout's 147 census errors); (3) T3, DEC-439 vendor stubs (twes-in becomes measurable and "lift your
  Symfony app, keep `vendor/` in PHP" becomes usable); (4) scout's small L3 rows batched (4p2, 4p3 + 4h2,
  4r, 4m, 4o2, 4f2). REJECTED: finish scout first; twes-in first; performance first.
- [2026-09-27 17:12] AGREED: **DEC-551 — STUBS BRIDGE, LIFTS REPLACE.** phorj never hand-writes its own
  Symfony / API Platform / Doctrine: vendor packages are LIFTED from their own source and PUBLISHED as phorj
  registry packages (copyright + LICENSE kept; all measured MIT), leaf-first, each replacing its stub;
  DEC-439's transpile-only stubs are the bridge until then. Confirmed after a measured challenge (lift rates
  below; 138 framework files use reflection / `eval` / magic methods / `new $class`).
- [2026-09-27 17:12] AGREED: **DEC-552 — attributes on members AND parameters** (properties, methods,
  class constants, parameters), every surface in the same change (Invariant 17).
- [2026-09-27 17:12] AGREED: **DEC-553 — a safe top type** (no operation until narrowed; lifts from PHP `mixed`,
  transpiles back to `mixed`). Answered with the spelling `unknown` in a question that did not cite DEC-335.
- [2026-09-27 17:18] AGREED: **DEC-553 reconciled — build DEC-335's `Any` + `Object` as specified**
  (`docs/specs/2026-07-23-any-object-top-types.md`, ruled 2026-07-23, BUILD-READY). DEC-335 already defined the
  same semantics; the spelling `unknown` is withdrawn and nothing is superseded. Lesson: search the specs index
  (`MASTER-PLAN.md` §0.07) BEFORE asking a design question.
- [2026-09-27 17:12] ASSUMED (review): **T2 refuses a HYPHENATED docblock atom by name** (`numeric-string`,
  `array-key`, `non-empty-string`, `class-string`, …) instead of emitting it verbatim — because the lifter's contract is
  never to emit a draft that does not parse, a PHP class name can never contain `-`, and a refusal needs no ruling.
  Correction at build (2026-09-27): `non-empty-list<T>` / `non-empty-array<…>` ALREADY lifted as their base generic and
  keep doing so, and `int<0, max>` was already refused as a Tier-2 generic. MAPPING the remaining pseudo-types
  (`numeric-string` → `string`, …) is a docblock-type decision like DEC-543/544 and goes to the developer in a later
  ask. Alternatives: map now; keep emitting them.
- [2026-09-27 17:12] ASSUMED (review): **T5 lifts `sprintf('%d', $x)` to interpolation ONLY when `$x`'s
  DECLARED type is `int`** (a parameter, a `@var` local, an int literal) — because `%d` equals `%s` exactly for
  an int, and DEC-166/DEC-515 already make declarations the lifter's only source of types. Anything else keeps
  its refusal. Alternatives: keep refusing every `%d`; map `%d` to an `(int)` cast (changes floats and numeric
  strings — rejected as a silent semantic change).

## Census baseline (T0, 2026-09-27, phorj `7463358b`)

Lift of `twes-in/api/src` (a copy), `phg lift php -o out`:

- **484 / 1 037 files lift**; 553 are refused, each with its reason in `LIFT-REPORT.md`.
- **317 vendor symbols** referenced. That count INCLUDES app classes whose own file was refused (`App\Tenancy\Domain\Company`, 134 references), so every refusal cascades into "vendor".
- `phg check` on one file per package: **10 / 114 packages clean**. **91 of the 104 failures stop at the loader** on an unresolved vendor import (`Symfony.Component.Uid` alone: 63 packages). So twes-in's type-error count is NOT comparable to scout's 147 and will not be until T3 lands — the loader masks everything behind the first import.

Refusal classes (files), ranked:

| Files | Class | Row |
|---:|---|---|
| 126 | bare `array` with no docblock shape — many are API Platform's `array $uriVariables = [], array $context = []` | T12 (after T7) |
| 94 + 33 | `mixed` in a signature / in a docblock — 91 are `ProcessorInterface::process(mixed $data, …)` | T7 (DEC-553 → DEC-335 `Any`) |
| 73 | `self::X` inside a CLASS-level attribute (`#[ApiResource(… normalizationContext: ['groups' => [self::READ]])]`) | T4 |
| 64 + 32 | an attribute on a class member / on a parameter | T6 (DEC-552) |
| 27 | `sprintf` `%d` (23), `%04d` (3), `%07d` (1) | T5 |
| 16 | first-class callable of a method or closure (`$o->m(...)`) | T11 |
| 14 + 9 | a union inside a docblock generic / a literal-type member (`'asc'\|'desc'`) | T11 |
| 10 | `yield` (generators) | T10 |
| 8 + 4 | variadic parameter / argument unpacking outside a list literal | T11 |
| 8 | the `iterable` type | T11 |
| 3 + 3 | elvis `?:` / assignment as a sub-expression | T11 |
| ≤2 each | 15 smaller classes (keyed destructure, `(array)` cast, by-ref, enum constants, …) | T11 |

Check failures on the 13 packages that pass the loader: 13 `unknown type` (vendor), **9 throwable names — ruled by DEC-546, expected and loud**, 3 `unknown function`, and **two drafts that do not parse** (T1, T2).

Vendor lift rates (DEC-551's measured challenge, `src/` minus tests): `psr/clock` 1/1 · `symfony/uid` 10/30 ·
`symfony/serializer` 28/123 · `symfony/http-kernel` 46/178 · `doctrine/orm` 119/471 · `api-platform/core` 126/862.

## Status

<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| T0 | Census baseline (above): 484/1037 lift, 10/114 packages check, classes ranked | S | done | - | docs/plans/2026-09-27-twes-in-forcing-function.plan.md |
| T1 | P0 — BUILT 2026-09-27 (`decls/ternary_stmt.rs`: a statement-position ternary is rewritten to PHP's own `if` statement before lifting; 3 lifter tests red first, sabotage `if false` reds both positives; round-trip `ternary_as_statement` under the PHP oracle; census: all 489 twes drafts and all 12 scout drafts parse). A PHP ternary used as a STATEMENT (`$c ? $a->m() : $b->m();`, `Licensing/Application/ManagePayments.php`) lifts to `if (c) { a.m() } else { b.m() };`, which phorj parses as an `if` STATEMENT whose arms lack `;` — a draft that does not parse. Lift a statement-position ternary to an `if` statement (the value is discarded, so it is exact). Third instance of the class `KNOWN_ISSUES.md` records as LIFT-ECHO-TERNARY and LIFT-TERNARY-IN-CONCAT (a lifted ternary in a position where an `if`-expression does not parse); those two stay open unless the fix covers them | S | done | - | src/lift/lifter/* |
| T2 | P0 — BUILT 2026-09-27 (`parser/doc_types.rs`: a hyphenated docblock atom is refused by name; test red first, sabotage reds it; twes 484 → 483 lifted, `LotPicking.php` now refused with the reason). PHPStan pseudo-types in a docblock (`list<(StockLot, numeric-string)>`, `Module/Inventory/Domain/LotPicking.php`) are emitted VERBATIM into the draft, which does not parse. Refuse each by name (ASSUMED above); the mapping is a later ask. Measured in `src/`: `numeric-string` 27, `array-key` 10, `non-empty-list` 5, `non-empty-string` 3, `int<0, max>` 1 | S | done | - | src/lift/* |
| T3 | DEC-439 `--vendor=stub` (DEC-551's bridge): `declare class` / `declare function` foreign stubs generated from `vendor/`'s own type hints; a signature that cannot be stubbed falls back to the report. Transpile-only, disclosed (Invariant 14). Then re-census | L | todo | - | src/lift/* |
| T4 | `self::X` / `static::X` inside a class's own class-level attribute → the class name (PHP resolves them to the class being declared). Behind T3: the attribute class itself is vendor | S | todo | - | src/lift/* |
| T5 | `sprintf` `%d` → interpolation when the argument's declared type is `int` (ASSUMED above); `%0Nd` padding after it | S | todo | - | src/lift/lifter/sprintf.rs |
| T6 | DEC-552 — attributes on properties, methods, class constants and parameters: parser, checker target validation, formatter, transpile, lift, LSP, both editors, example | L | todo | - | src/* editors/* examples/* |
| T7 | DEC-553 → build DEC-335 as specified: `Any` + `Object` top types (member-less, narrowing by `instanceof` smart-cast / `match`), transpile `Any` → `mixed` / `Object` → `object`, lift `mixed` → `Any` / `object` → `Object` / `new \stdClass()` → `new Object()` / `is_object` → `instanceof Object`, JIT boxed path, LSP, editors, example; DEC-335's two PENDING build points (`E-INSTANCEOF-ANY`, `Any` in a union) are asked at pickup | L | todo | - | src/* editors/* examples/* |
| T8 | Re-census after T1–T7: lift rate, packages clean, error classes | S | todo | - | - |
| T9 | DEC-551 — lift-and-publish vendor packages leaf-first as phorj registry packages (LICENSE kept): `psr/clock`, `symfony/uid`, `psr/log`, the HttpKernel exceptions, then upward; each replaces its stub | L | todo | - | - |
| T10 | Generators (`yield`, 10 files) — ask: DEC-257's iterator design is the context | M | todo | - | - |
| T11 | The long tail above (first-class method callables, docblock unions, variadics, `iterable`, elvis, …) — each re-ranked at T8 | M | todo | - | src/lift/* |
| T12 | Bare `array` (126 files) — re-measured after T7, since API Platform's `array $context` becomes a `Map<string, Any>` question | M | todo | - | - |
<!-- /progress-block -->

### Blocked
- T4 is behind T3 (the attribute's class is vendor). T12 is behind T7.

### Needs input
- The pseudo-type MAPPING (T2's ASSUMED), generators (T10).

### Known issues
- twes-in's error count is loader-masked until T3 (see the baseline).

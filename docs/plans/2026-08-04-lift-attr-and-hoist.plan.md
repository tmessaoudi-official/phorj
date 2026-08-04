# LIFT-ATTR + DEC-397 lifter hoist — plan

**Status:** Phase 4 approved (developer ruled *"option 1 for both"*, 2026-08-04). Two slices, two commits.
**Tracks:** task #48 (hoist) and task #46 (attributes). Closes the two gaps recorded by the DEC-417
editor slice (`SLICE-STATE.md` §"TWO GAPS RECORDED"), the second of which (#47) shipped in `b219856`.

## Decisions Log

- [2026-08-04] AGREED: **Q2 / #48 hoist shape = literal-hoist.** A variable whose FIRST assignment sits
  in a nested block is declared at function-body top using that assignment's value **iff the value is a
  literal**; the in-block assignment then becomes a plain assignment. A non-literal first assignment is
  NOT hoisted — hoisting it would move side effects out of their branch (Invariant 14 forbids the silent
  downgrade) — and gets a loud `// CANNOT LIFT:` note naming the variable instead.
- [2026-08-04] AGREED: the nullable fallback originally proposed is **withdrawn as unexpressible** —
  `mutable var b = null` is `E-INFER-NULL` and `mutable int? b = null` needs a type the lifter cannot
  infer from untyped PHP locals. [Verified: both forms run through `phg check`.]
- [2026-08-04] AGREED: **Q1 / #46 policy = structural lift, forward-compatible.** Lift attribute
  CLASSES and USE SITES rather than discarding or commenting out unknown attributes. `\` → `.` for
  namespaced names. Nothing is dropped silently; when DI-v2 L1 (`subjectsWith<Attr>()`) lands, already
  lifted attributes light up with no re-lift.
- [2026-08-04] NOTED (research, corrects my own earlier framing): PHP userland attributes are NOT
  unmappable. Phorj has the same concept (DEC-194 `#[Attribute]` classes) and type-checks attribute
  arguments against the attribute class's constructor at COMPILE time, which PHP only does when the
  attribute is reflected. The real gap is the *consumer*: attributes are erased before every backend
  and there is no runtime attribute reflection (DEC-092 rejected it). `UNIFIED-SPEC` §"DI v2 / L1–L2"
  already rules the replacement — compile-time attribute reflection + reverse discovery, resolution
  kind "BOTH, compile-time-FIRST", byte-identity-safe because discovery feeds codegen before backends.
  Status DESIGN, scheduled Ω-4/Ω-7. **So "lift a Symfony app" = lift the app onto phorj's native L2
  consumers, not lift the framework.**

## Slice A — DEC-397 the lifter hoist (#48)

**Root cause** [Verified]: `src/lift/lifter/decls/statements.rs:180` emits `Stmt::VarDecl` at the site of
the first assignment, and `declared: &mut HashSet<String>` is threaded through nested blocks — so a
variable first assigned inside an `if` is *declared* inside that `if`. PHP has function scope; phorj has
block scope. Reproducer and its two errors:

```php
function f(): int { if (true) { $b = 5; } $b = 7; return $b; }
```
→ `mutable var b = 5;` inside the block, then `b = 7;` outside → `E-ASSIGN-UNKNOWN` + `E-UNKNOWN-IDENT`.

**Mechanism (deliberately minimal).** Pre-scan the function body; for each qualifying variable emit its
declaration at the top of the lifted body AND seed `declared` with its name before lifting the body.
Seeding is what makes every in-block assignment lift as a plain assignment for free — and it is also
what keeps the output clear of `E-SHADOW-LOCAL`, which DEC-397 explicitly requires ("the lifter must not
emit programs `E-SHADOW-LOCAL` rejects").

**Qualifying set:** first assignment is inside a nested block, the variable is **referenced outside that
block**, AND the assigned value is a literal.
- "Nested block" = any of `if`/`elseif`/`else`/`while`/`for`/`foreach`/`try`/`catch`/`finally`/`switch`.
- First assignment already at body top level → unchanged (current behaviour is correct there).
- Non-literal value → not hoisted, `// CANNOT LIFT:` note naming the variable.

### CORRECTIONS from the baseline fixture run (2026-08-04, before any build)

Eleven PHP fixtures were lifted and checked against the release binary *before* writing code. Three of
this plan's own assumptions did not survive, and are corrected here rather than discovered mid-build:

1. **The "referenced outside that block" condition is REQUIRED, not an optimization** — my first draft
   hoisted every nested-first-assignment. Fixture `h2` (`while ($n > 0) { $acc = 1; }`, `$acc` never read
   outside) **already lifts and checks clean today**; hoisting it would add a spurious declaration to
   working output. The condition above is the fix. [Verified: `h2-loop-zero-iterations.php` → `OK
   (type-checks clean)`]
2. **R1's closure-boundary hard stop guards NOTHING today** — the lifter refuses closures outright:
   `lift parse error: closures and arrow functions are Tier-2`. Writing scope-crossing logic now would be
   dead code defending an unreachable state. Recorded as a REQUIREMENT ON TIER-2 instead: when closures
   become liftable, the pre-scan must stop at the closure boundary. [Verified:
   `h4-closure-boundary.php`]
3. **The `foreach`-bind hoist case is also unreachable** — `array` parameters are Tier-2 (`lift: an
   'array' type needs List/Map/Set inference`). [Verified: `h5-foreach-bind.php`]

### CORRECTION to Slice B from the same run

4. **Emit the CANONICAL DOTTED form for recognized built-ins, not the bare leaf.** A bare `#[Route(…)]`
   in lifted output would be `E-INJECTED-TYPE-BARE` unless the lifter also synthesized
   `import Core.Http.Route;`. The fully-qualified `#[Core.Http.Route(…)]` is **self-gating** — no import
   bookkeeping at all — and it is the same `\` → `.` shape the developer ruled. This removes a whole
   import-synthesis sub-problem. [Verified by RUN, not by reading: `#[Core.Http.Route("GET","/users")]`
   with no import clears the import gate entirely (its only complaint is `E-ROUTE-HANDLER` about the
   test function's signature), whereas the bare `#[Route(…)]` with no import is `E-INJECTED-TYPE-BARE`
   with the hint *"member-import it … or write it qualified"*.]
5. A PHP `#[\Attribute]`-marked class **already round-trips** structurally
   (`class Column { public function __construct(public string $name) {} }` → `open class Column {
   constructor(public mutable string name) {} }`); slice B only needs to re-attach the marker.
   [Verified: `a5-attribute-class.php`]

**Acceptance:** the reproducer above lifts to code that `phg check`s clean and runs identically on
`run` / `run --tree-walker` / php-8.5.8.

## Slice B — LIFT-ATTR structural attribute lift (#46)

**Root cause** [Verified]: `src/lift/lexer.rs:165` — `if c == '#'` skips to end of line, so `#[...]` is
swallowed whole with no diagnostic.

1. **Lexer:** `#[` → a new `PTok::AttrOpen`. A bare `#` stays a line comment (PHP allows both).
2. **Lift AST:** `PhpAttribute { name, args, line }`; `attrs: Vec<PhpAttribute>` on `PhpFunction` and
   `PhpClass`. `PhpExpr::NamedArg { name, value }` — required, because `#[Route(path: '/x')]` is the
   dominant real-world form and dropping named args would gut the feature.
3. **Parser:** `parse_item` is the single dispatch point, so attribute groups parse there and attach to
   the item that follows. `#[A, B(1)]` (PHP's grouped form) splits into two attributes.
4. **Lifter:** `\`-qualified names → dotted (`ORM\Column` → `ORM.Column`, leading `\` stripped); PHP's
   own `#[\Attribute]` marker → phorj `#[Attribute]`, so a custom attribute CLASS round-trips.
5. **Printer:** emit each attribute on its own line above the declaration.
6. **Honesty:** an attribute that is neither a phorj built-in nor an attribute class present in the
   lifted output gets a `// CANNOT LIFT:`-style note saying its class must be ported — because phorj
   hard-errors `E-UNKNOWN-ATTRIBUTE` on an undeclared attribute name, so the draft will not check
   until the user supplies it. Stated, never silent.

**Open sub-decisions deferred to the developer (not self-ruled):** the exact spelling of a namespaced
attribute (`ORM.Column` vs `OrmColumn` — Invariant 12 casing interacts), and the PHP *engine* attribute
tier (`#[\Override]`, `#[\SensitiveParameter]`, `#[\AllowDynamicProperties]`,
`#[\ReturnTypeWillChange]`) which are not userland classes and need Invariant-14 ladder treatment.
Slice B ships the general machinery; those two are recorded as follow-ups.

## Risks

- **R1 — hoisting past a closure** would change scope semantics. Mitigation: hard stop + test.
- **R2 — double evaluation.** Only literals are hoisted, and a literal has no side effects, so the
  redundant init is unobservable. Non-literals are refused rather than moved.
- **R3 — `E-SHADOW-LOCAL` regression.** Seeding `declared` (rather than emitting a second `VarDecl`) is
  what prevents it; a lifted-then-checked test pins it.
- **R4 — Invariant 13.** `statements.rs` is 260 lines and `lexer.rs` 449 (hard cap 500). The pre-scan
  goes in a new sibling file, not into either.

## Rollback

Both slices are additive to the lifter only — no backend, no value kernel, no `Op`. Revert the commit;
`phg lift` returns to its prior output. No migration, no on-disk format.

---

# 3C PANEL RESULT (2026-08-04) — PLAN REFUTED, 24 findings (8 × P0)

Two fresh-context adversarial reviewer lenses (DEC-268 MAXIMAL) read the code rather than this plan's
narrative. **Neither slice is buildable as specified.** Findings preserved here because regenerating them
costs ~10 min of compute and the container is reclaimed. `[V]` = the reviewer ran it; I independently
re-verified the two starred ones.

## Slice A (hoist) — the qualifying rule is wrong in five ways

| # | P | Finding | Evidence |
|---|---|---|---|
| A1 | P0 | **Seeding `declared` does NOT suppress a `foreach` binding** — `Stmt::For` is emitted unconditionally, never reads or writes `declared`. Hoisted name + for-in bind = one of DEC-339's ten shadowing forms. `if (true) { $v = 1; } foreach ([10,20] as $v)` → `E-SHADOW-LOCAL` | `lifter/decls/statements.rs:107-114`; `checker/plumbing.rs:214-239` [V] |
| A2 | P0 | **Same for `catch` bindings** — `name` is pushed into a *clone* of `declared`, so seeding the outer set has zero effect | `statements.rs:137-145` [V] |
| A3 | P0 | **The "nested block" list was written from PHP-the-language, not from `PhpStmt`.** `Block(Vec<PhpStmt>)` is MISSING (a bare `{ … }` is a real phorj scope), and `switch` **can never reach the lifter** (it is in `UNSUPPORTED_KW`). Real set: `If{then,elifs,els}`, `While`, `For`, `Foreach`, `Block`, `Try{body,catches,finally}` | `lift/ast.rs:163-217`; `lift/parser/mod.rs:24`; `statements.rs:158-160` [V] |
| A4 | P0 | **R2 "the redundant init is unobservable" is FALSE.** `if (false) { $b = "five"; } echo $b;` → PHP prints nothing (warning, exit 0); hoisted phorj prints `five` on both legs. `tests/lift_roundtrip.rs:101-126` asserts lifted stdout == original PHP stdout, and `run_php` only fails on non-zero exit — so this is a LIVE failing test, not a theoretical one | `tests/lift_roundtrip.rs:78-126` [V] |
| A5 | P0 | **The largest `E-SHADOW-LOCAL` emitter is one line away and absent from the plan**: `declared` is constructed INSIDE the per-item loop, so every top-level statement gets a fresh set. The two-line program `$x = 1; $x = 2;` lifts to two `mutable var x` → `E-SHADOW-LOCAL`. The plan cites "must not emit programs `E-SHADOW-LOCAL` rejects" as the hoist's second reason to exist; that claim is false for the simplest possible input | `lifter/decls/mod.rs:65-68` [V] |
| A6 | P1 | `declared` is populated in **reverse source order** for `if/elseif/else` (`els` lifted first, `elifs` walked `.rev()`, `then` last) — so today's `VarDecl` lands in the LAST branch, and "the FIRST assignment" is ambiguous | `statements.rs:47-63` [V] |
| A7 | P1 | **"Literal" is undefined and the two existing predicates disagree on the dangerous variant.** `lit_type` = Int\|Float\|Str\|Bool; `literal_pattern` also admits `Null` → `mutable var b = null` is `E-INFER-NULL`, the exact form this plan already withdrew. Also: `PhpExpr::Array` is NOT side-effect-free (`[1, foo()]`), and `PhpExpr::Interp` holes READ variables | `mappings.rs:129-137`; `exprs.rs:233-239`; `ast.rs:241,248,239` [V] |
| A8 | P1 | **`for` init/step is the same DEC-397 bug and is not a block** — `For.init` is `Option<PhpExpr>`, so a block-based pre-scan never visits it. Two `for ($i = 0; …)` loops in one function reproduce the identical error pair this plan quotes as its symptom | `statements.rs:80-87,232-238`; `ast.rs:183-188` [V] |
| A9 | P1 | **if/else assigning different literal types becomes a HARD type error under hoisting** (`E-ASSIGN-TYPE`), where today only the outer read fails. This is PHP's dominant branch-assign idiom | [V] |
| A10 | P1 | **FIVE body-lift sites seed `declared`; the plan names none.** Wiring the hoist into `lift_function` only leaves every METHOD body still broken, and no differential coverage would catch it | `declarations.rs:12-27,138-148,169-186`; `magic.rs:20-34`; `decls/mod.rs:66` [V] |
| A11 | P2 | R1's closure hard-stop guards nothing — the lift AST has no closure variant at all; closures are parser-rejected. An unreachable guard that reads as verified safety is worse than none | `parser/exprs.rs:302`; `parser/mod.rs:35-36` [V] |

## Slice B (LIFT-ATTR) — the token-variant design regresses working behaviour

| # | P | Finding | Evidence |
|---|---|---|---|
| B1 | P0 | ★ **A new `PTok::AttrOpen` turns today's silent drop into a HARD LIFT FAILURE** for attributes on methods, properties, enums, enum cases and promoted ctor params. `parse_item` is the single dispatch for TOP-LEVEL items only; `parse_member`, `parse_params`, `parse_enum` never route through it and would hit the new token. Real-world PHPUnit `#[Test]`, Symfony `#[Autowire]` on promoted params and `#[Override]` lift **successfully today** and would stop | `lexer.rs:165`; `parser/items.rs:110,215,329,374` — ★ re-verified: `#[Attr] class Box { #[Ignore] private int $n; #[Override] public function get() }` lifts exit=0 today |
| B2 | P0 | ★ **Named args — the plan's stated must-have — produce a draft phorj itself rejects.** `check_user_attribute_use` calls `check_arg` POSITIONALLY with no named-arg normalization, so `Expr::NamedArg` reaches `check_expr` → `E-NAMED-ARG-MISPLACED`. Named args work only for built-ins read structurally (`#[Entry(kind:)]`) | `checker/program/attributes.rs:45-46`; `checker/expr/core.rs:156-166` — ★ re-verified: `#[Column(name: "x")]` → `E-NAMED-ARG-MISPLACED` |
| B3 | P0 | **Built-in name collision, and the plan's honesty check stays SILENT for it.** A Symfony `#[Route('/x')]` binds to phorj's built-in `Core.Http.Route` → `E-ROUTE-ARGS`, not `E-UNKNOWN-ATTRIBUTE`. The plan gates its note on "neither a built-in nor a locally declared class" — `Route` IS a built-in, so no note. Same for `#[\Deprecated]`, which phorj **deliberately** does not map to PHP's runtime-firing one (byte-identity) — and which the plan's engine-tier list omits | `checker/program/attributes.rs:139-141,238-245` [V] |
| B4 | P1 | **`printer::class` never reads `c.attrs`** — a class attribute would populate the AST and vanish at print time. Invisible to any test asserting on the lifted AST instead of printed text | `lift/printer/items.rs:60-73` (function, handles attrs) vs `:106-130` (class, does not) [V] |
| B5 | P1 | **Bare `#[Attribute]` without an import is `E-INJECTED-TYPE-BARE`** — the lifter already synthesizes imports for `#[Entry]`/`EntryKind`/`Core.ErrorModule` for exactly this reason; the plan says nothing | `checker/enforce_injected.rs:52-56,166-173`; `lifter/decls/mod.rs:104-155` [V] |
| B6 | P1 | **A new `PTok` variant is compile-time SILENT** — every `PTok` match in the lifter carries a `_` arm and `at()`/`eat()` use `mem::discriminant`, so `rustc` cannot enumerate the sites needing updates. **The lexer's own doc already chose a SIDE CHANNEL over a new token variant** for precisely this failure mode ("one missed site rejects valid PHP"); the plan reverses that documented decision without engaging it | `parser/items.rs:29,57,68,153`; `parser/exprs.rs:417,432`; `parser/stmts.rs:44,239`; `lexer.rs:113-118` [V] |
| B7 | P1 | **`\` → `.` contradicts the single-source namespace rule.** `strip_root_ns` is documented as "THE one place that rule lives" and says inner separators are LEFT ALONE because flattening "would invent a name". Worse: `#[ORM.Column]` type-checks clean because attribute matching is by LEAF — so `ORM.Column` and `Doctrine.Column` both silently bind to `class Column` | `lifter/mappings.rs:156`; `checker/program/attributes.rs:17` [V] |
| B8 | P1 | **Enums, properties and params cannot carry attributes in phorj at all** — only `FunctionDecl` and `ClassDecl` have `attrs`. So "nothing is dropped silently" is self-contradictory as written: those forms either hard-error (B1) or drop silently | `ast/decls/functions.rs:14`; `ast/decls/classes.rs:66`; `parser/items/decls/items.rs:71-73` [V] |
| B9 | P1 | **Invariant 13: R4 checked two files and missed the ones that actually fail.** `lift/printer/items.rs` is at **500 / 500 — the first added line FAILS the gate**, and B4 requires edits there. `lift/parser/exprs.rs` 669 with **1 line** of headroom, and `parse_args` (where named args would parse) lives there. `parser_tests.rs` 556 with 3 | `scripts/size-gate.sh:47-58` [V] |
| B10 | P2 | **Lifted output IS checked today** — `tests/lift_roundtrip.rs` runs lift through treewalk + run + transpile and asserts stdout equals the original PHP's. So B2/B3/B5 each break that harness the moment a case is added | `tests/lift_roundtrip.rs:101-126` [V] |
| B11 | P2 | Missing: `examples/` deliverable (Invariant 9), closing `KNOWN_ISSUES.md:185`, SLICE-STATE + **register rows for both AGREED rulings** (Invariant 19), and an explicit LSP/editors line (Invariant 17). Stale pointers to fix in passing: `KNOWN_ISSUES.md:187` and `SLICE-STATE.md:384` both cite `lexer.rs:144`; the real site is `:165`. `examples/lift/README.md` shows `-> string` where the printer emits `: string` | [V] |

## Two PRE-EXISTING bugs the panel surfaced (independent of both slices)

- **A5** — `$x = 1; $x = 2;` at top level lifts to non-compiling phorj. Two-line reproducer, cheap fix.
- **B11** — `examples/lift/README.md` documents a return-type arrow the printer never emits.

## Consequences for the design

- **Slice B switches to a SIDE CHANNEL** keyed by next-token index (the PHPDoc precedent at
  `lexer.rs:110-121`), NOT a new `PTok` variant. This is the codebase's own documented choice for this
  exact failure mode, and it dissolves B1 and B6 outright: no parser site has to learn to skip anything,
  so nothing that lifts today stops lifting.
- **Slice A's qualifying rule needs a dominating-assignment condition**, because A4 shows a bare literal
  hoist genuinely diverges from PHP when the block does not execute. Candidate: hoist only when an
  UNCONDITIONAL body-top-level assignment to the same name precedes every read outside the first-assignment
  block — provably safe (that assignment fixes the value before any read), covers the reported DEC-397
  shape, and incidentally dissolves A9 because the hoisted init no longer comes from a branch.

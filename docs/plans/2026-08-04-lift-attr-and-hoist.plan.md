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

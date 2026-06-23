# Pattern cluster Plan (M-RT follow-up — guards + destructuring + flow-narrowing)

> The post-M-RT language-ergonomics slice: `match`/`if-let` **guards**, **payload destructuring**, and
> **flow-narrowing** — the defining TS/Rust pattern capability a PHP-from-TS migrant expects. Front-end
> only (no new `Op`, no `Value` change targeted). Byte-identical `run ≡ runvm ≡ real PHP 8.4`.
> **Design-first**, then slice-by-slice build.

## Decisions Log
- [2026-06-23 ~10:00] AGREED: post-M-Decomp milestone selection. The full GA top-10 spine items 1–4
  (totality, generic enums/Result, error model, OO slices incl. overloading/extends/traits) are all
  CLOSED; error model + M6 web + M-Decomp verified shipped. Developer chose **all of the remaining open
  spine items (#5–#10)**, and accepted the recommended **risk-adjusted order**:
  **#5 pattern cluster → #7/#9 stdlib breadth+charter → #8 DX trio → #6 M-NUM decimal**, with **#10
  GA-governance docs interleaved** as low-effort filler.
  Rationale: front-load the front-end-only / additive wins (which also validate the fresh
  decomposition cheaply) and defer the single value-kernel-touching, externally-constrained milestone
  (decimal) to last — unless money becomes an explicit near-term business need, which would override.
- [2026-06-23 ~10:00] AGREED: open **#5 with a design pass first** (brainstorm → spec + plan →
  developer approval → slice-by-slice build), not autonomous build-through. #5 is a Large language slice
  touching the parser + checker + all three backends' pattern surfaces.
- [2026-06-23 ~10:15] AGREED: **scope = "Everything" (maximal envelope)** across all three axes:
  (1) **guards** — match-arm + if-let; (2) **payload destructuring** — un-reject nested type-patterns in
  variant payloads (`Wrapper(Circle c)`) **plus** new **class/named-field destructuring** `Point { x, y }`
  (new `Pattern::Struct`); (3) **flow-narrowing** — negative/else narrowing, early-return narrowing,
  post-exhaustive-match narrowing, **plus** equality/null/literal refinement (`== null`, literal `==`).
  Front-end-only target (no new `Op`, no `Value` change); byte-identical `run ≡ runvm ≡ real PHP 8.4`.
  Grounded gap inventory verified against `ast/mod.rs`/`parser/patterns.rs`/`checker/stmt.rs`/`KNOWN_ISSUES.md`.
- [2026-06-23 ~10:25] AGREED: **guard keyword = `when`, as a *contextual* keyword** (special only in
  guard position — after a pattern before `=>`, and in if-let before `)`; like `as` for import aliasing,
  reserves nothing globally). Chosen over `if` after challenge: kills the body-`if`-expr collision
  (`Circle c when … => if (…) {…}` reads cleanly), strong guard-specific precedent (C#/F#/Elixir/Erlang),
  zero reservation cost via contextual treatment. Guarded arms do NOT count toward exhaustiveness — an
  unguarded fallback for that shape is still required (new checker rule).
- [2026-06-23 ~10:30] AGREED: **class/struct destructuring = full nesting + rename** (`Pattern::Struct`):
  shorthand `Point { x, y }`, rename `Point { x: px }`, and nested field patterns
  `Line { from: Point { x, y }, to }`. Chosen over shorthand-only after challenge: uniform with the
  already-committed nested payload patterns (`Wrapper(Circle c)`) — anything less is an arbitrary
  asymmetry/surprise; and struct patterns are single-type tests, so nesting adds **no new exhaustiveness
  surface** (only binding + nested-`instanceof` lowering, which earns its own build sub-slice).

> [2026-06-23] AGREED: **execution = Inline, task-by-task** (executing-plans), with a checkpoint after
> each sub-slice (S5.1, S5.2, S5.3) for review. Chosen over subagent-driven: the 12 tasks are tightly
> sequential, contend on the same core files (`checker/matches.rs` + the three backends' match arms), and
> S5.3-T6 edits `CLAUDE.md` (subagent-blocked) — so the parallelism that would justify subagents is absent.
> Per-task commits, gate-green each time.

> [2026-06-23] AGREED (execution, S5.1): **S5.1-T1 match-arm guards SHIPPED** (`c7e7f13`,
> contextual `when`, no new `Op`, byte-identical, 827 tests). **S5.1-T2 if-let/while-let guards
> DEFERRED** to a follow-up — cost-discovery: it needs an invasive `Stmt::If.guard` field (~18
> construction/consumer sites incl. the fragile `rewrite_*`/loader AST-rebuild passes) or a
> synthetic-local desugar, disproportionate to its marginal value now that match-arm guards (the
> headline) are done. Recorded in KNOWN_ISSUES; can land later as its own small slice.

> [2026-06-23] AGREED (primitives sweep + re-sequence): a primitives-coverage sweep (bidirectional,
> inline) found Tier-1 byte-identical gaps the big feature-audit under-weighted. Developer asked for the
> recommended order; locked sequence (interleaved before the cluster's S5.2): **(P1) number literals
> `0x`/`0b`/`0o`/`1_000`/`1e9` → (P2) bitwise `& | ^ ~ << >>` + `**` → (P3) `Console.print` +
> `Text`/`Math`/`List` breadth (incl. `parseInt`) → (S5.2) struct/nested destructuring → (S5.3)
> flow-narrowing engine, with the deferred if-let `when` guards FOLDED IN here** (both live in the
> `Stmt::If` arm — doing them together makes the ~18-site change cheap). Rationale: bank the cheap,
> high-certainty, dependency-free primitives first (they answer the developer's "are we covered?"
> concern); the cluster loses nothing by waiting. Bitwise design notes: `&`/`|` are expr-context vs
> type-context (intersection/union); `<<` is a new token; `>>` is two adjacent `Gt` in expr position
> (NEVER a lexer token — protects nested generics `List<List<int>>`). Gap map to be appended to the SSOT.

> [2026-06-23] PROGRESS: S5.1 match-arm guards SHIPPED (`c7e7f13`/`63e2e6c`); if-let guards DEFERRED
> (fold into S5.3). Primitives **P1 number literals SHIPPED** (`70e15df`), **P2 bitwise SHIPPED**
> (`0e529ad`), **P3.1 Console.print SHIPPED** (`6f2bf72`). All green, byte-identical run≡runvm≡PHP-8.4,
> 833 tests.
> [2026-06-23] SCOPE REFINEMENT (P3): the "stdlib breadth" splits cleanly. **Ship now (P3.2, byte-safe):**
> `Text.startsWith`/`endsWith`/`repeat`, `Math.round`(→int), `List.length` — all total, non-optional,
> map 1:1 to PHP (`str_starts_with`/`str_ends_with`/`str_repeat`/`(int)round`/`count`). **DEFER to M4
> (stdlib milestone):** `Text.parseInt`(→`int?` needs a byte-identical PHP validation helper),
> `Text.indexOf`/`substring` (optional/bounds), `List.sort`/`contains`/`first`/`last`/`slice` (need
> generic ordering/equality + optional returns) — these want the optional-return + generic-ordering
> design M4 will establish, not ad-hoc natives. **NEXT: P3.2 → S5.2 → S5.3(+if-let guards).**
> [2026-06-23] PROGRESS: **P3.2 byte-safe stdlib subset SHIPPED** — `Text.startsWith`/`endsWith`/`repeat`
> (→ `str_starts_with`/`str_ends_with`/`str_repeat`), `Math.round`→int (→ `(int)round`, half-away-from-zero
> matching PHP's default mode), `List.length` generic (→ `count`). All five are additive registry entries
> (no plumbing change — the four-backend native path was already generic); `Text.repeat` faults cleanly on
> a negative count (EV-7). Examples `text.phg`/`math.phg`/`collections.phg` extended + README coverage rows;
> bool results rendered via expression-`if` (PHP echoes a bool as `1`/`""`, not `true`/`false`). 835 tests
> green, byte-identical run≡runvm≡real PHP 8.4. **NEXT: S5.2 struct/nested destructuring → S5.3 (+if-let).**
> [2026-06-23] PROGRESS: **S5.2 struct/nested destructuring COMPLETE** (3 commits). **T1** (`5d20a8f`):
> `Pattern::Struct { type_name, fields: Vec<FieldPat> }` — shorthand/rename/full-nesting; reuses
> `Op::IsInstance` + field reads (no new `Op`); compiler `path` generalized to `PathSeg { Enum | Field }`;
> transpiler `classify_pattern` made recursive (`tests: Vec<String>`). Codes E-STRUCT-PAT-TYPE /
> E-STRUCT-FIELD-UNKNOWN / E-PATTERN-DUP-BIND (+ `phg explain`). **T2** (`0fff128`): nested type patterns
> in variant payloads (`W(Circle c)`) un-rejected; soundness via `is_irrefutable` (a refutable payload
> doesn't discharge variant coverage — also closed the pre-existing `Some(0)`-alone gap; `match_arm_key`
> no longer false-dups refined payloads). **T3** (this commit): KNOWN_ISSUES struct deferrals (uninit
> fields, conservative payload totality, generic-class struct patterns). All byte-identical
> run≡runvm≡real PHP 8.4; `examples/guide/pattern-matching.phg` covers struct + payload nesting. 841
> tests green. **NEXT: S5.3 flow-narrowing engine (+ folded-in if-let `when` guards).**

## Formal Plan

> **For agentic workers:** implement task-by-task. Each task is independently testable and ends with a
> green gate + a commit. TDD: write the failing test first, watch it fail, implement, watch it pass.

**Goal:** ship the pattern cluster (guards + destructuring + flow-narrowing) per
`docs/specs/2026-06-23-pattern-cluster-design.md`, byte-identical `run ≡ runvm ≡ real PHP 8.4`.

**Architecture:** front-end-only. Guards add a `MatchArm.guard` slot lowering to existing branch ops;
destructuring adds `Pattern::Struct` lowering to `IsInstance` + field reads; narrowing is a checker-only
engine. No new `Op`, no `Value` change, no global keyword (`when` is contextual).

### Global Constraints (every task)
- **Gate (run before each commit):** `export PATH=/stack/tools/cargo/bin:$PATH` then
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && \
  PHORGE_PHP=/stack/tools/phpbrew/php/php-8.4.22/bin/php PHORGE_REQUIRE_PHP=1 cargo test`.
  The pre-commit hook runs this; a commit that fails it must not land. PHP floor is **8.4** (the local
  php-master is too permissive — [[php-transpile-floor-84]]).
- **No new `Op`, no `Value` change.** If a task seems to need one, STOP — the design is front-end-only.
- **Byte-identity:** every shipped `.phg` runs identically on `run`/`runvm` and round-trips through real
  PHP (auto-gated by the `tests/differential.rs` example glob).
- **Examples ship with features** ([[examples-ship-with-features]]): the guide example lands in the same
  milestone.
- **Test homes** (post-decomposition): checker tests → `src/checker/tests/matching.rs` (by feature);
  parser tests → `src/parser/tests/patterns.rs` (by construct); cross-backend → `tests/differential.rs`.
- New diagnostics self-document via `phg explain <CODE>` (add to the explain table).

### Touch-site map (verified)
| Concern | File:fn |
|---|---|
| Pattern/MatchArm defs | `src/ast/mod.rs` (Pattern @58, MatchArm @97, Expr::Match @207) |
| pattern parse | `src/parser/patterns.rs::parse_pattern` |
| match parse (builds MatchArm) | `src/parser/exprs.rs::parse_match` @457 |
| if-let / while-let parse | `src/parser/stmts.rs` (`try_var_decl_header`, if/while) |
| checker match | `src/checker/matches.rs::check_match` @6, `check_pattern` @~150, `match_arm_key` |
| checker narrowing (today, inline) | `src/checker/stmt.rs` `Stmt::If` arm @111 |
| interpreter | `src/interpreter/expr.rs::Expr::Match` @176, `interpreter/mod.rs::match_pattern` @562 |
| compiler | `src/compiler/expr.rs::Expr::Match` @144, `compiler/matches.rs::emit_pattern_test` @50 |
| transpile | `src/transpile/matches.rs::emit_match` (already an if/elseif/else ladder) |

---

## Sub-slice S5.1 — Guards (match + if-let)

### Task S5.1-T1 — match-arm guards end-to-end
Adding `MatchArm.guard` forces every match consumer to compile (Rust exhaustiveness) — so this is one
atomic, all-backend task, like the `Op`-trio coupling rule.

**Files:** Modify `src/ast/mod.rs` (MatchArm), `src/parser/exprs.rs` (parse_match), `src/checker/matches.rs`
(check_match), `src/interpreter/expr.rs` (Match eval), `src/compiler/matches.rs` (emit), `src/transpile/matches.rs`
(emit_match). Test: `src/parser/tests/patterns.rs`, `src/checker/tests/matching.rs`, `tests/differential.rs`.

- [ ] **Step 1 — failing parser test** in `parser/tests/patterns.rs`: assert
  `match s { Circle c when c.r > 0.0 => 1, Circle c => 0, _ => -1 }` parses, and `arms[0].guard.is_some()`,
  `arms[1].guard.is_none()`. Run `cargo test -p … patterns` → FAIL (field/parse missing).
- [ ] **Step 2 — AST:** add `pub guard: Option<Expr>` to `MatchArm` (after `pattern`).
- [ ] **Step 3 — parser:** in `parse_match`, after `parse_pattern()` and before the `=>` expect, add a
  contextual-`when` check: `if let TokenKind::Ident(k) = self.peek().clone() { if k == "when" { self.advance(); guard = Some(self.parse_expr()?); } }`
  then `MatchArm { pattern, guard, body, span }`. (`when` stays a normal ident elsewhere — only consumed here.)
- [ ] **Step 4 — checker:** in `check_match`, after binding the arm's pattern, type the guard (when
  `Some`) as `Ty::Bool` in the arm's narrowed scope (`E-GUARD-TYPE` if not bool). Exhaustiveness:
  a guarded arm must NOT mark its shape covered — gate the existing "mark covered" branches
  (Wildcard/Binding @55, Variant @59, Type @62, Null @76, `match_arm_key` @43) on `arm.guard.is_none()`.
  If a shape is reachable only via guarded arms with no unguarded fallback → existing exhaustiveness
  failure fires; add `E-MATCH-GUARD-EXHAUST` as the hinted code.
- [ ] **Step 5 — interpreter** (`interpreter/expr.rs` Match loop): after `match_pattern` succeeds and
  bindings are installed, if `arm.guard` is `Some(g)`, eval `g`; on `false` continue to the next arm.
- [ ] **Step 6 — compiler** (`compiler/matches.rs`): after `emit_pattern_test` + binds, if guard `Some`,
  compile the guard expr and emit `JumpIfFalse → next-arm label` (reuse existing jump emission; the binds
  are live locals). No new `Op`.
- [ ] **Step 7 — transpile** (`transpile/matches.rs::emit_match`): for a guarded arm, fold binds +
  guard into the `elseif` condition: leading pattern test, then one `(($bind = <access>) || true)`
  conjunct per bind, then `&& (<guard>)`. Body block unchanged.
- [ ] **Step 8 — checker test** `matching.rs`: `match s { Circle c when c.r>0.0 => … , _ => … }` with
  only guarded `Circle` arms and no unguarded `Circle`/`_` fallback → `E-MATCH-GUARD-EXHAUST`; the
  with-fallback version type-checks. Non-bool guard → `E-GUARD-TYPE`.
- [ ] **Step 9 — differential** `differential.rs`: (a) guard fall-through to next arm; (b) two
  same-shape arms with different guards, first-match-wins; (c) **guard arithmetic on a bound payload**
  (`Code n when n + 1 > 500 => …`) — the CTy operand case. Assert `run ≡ runvm` and (via the harness)
  real PHP.
- [ ] **Step 10 — gate + commit:** `feat(lang): match-arm guards (contextual when) (patterns S5.1-T1)`.

### Task S5.1-T2 — if-let / while-let guards
**Files:** Modify `src/parser/stmts.rs` (if/while-let parse), `src/checker/stmt.rs` (if-let check),
interpreter/compiler/transpile if-let lowering sites. Test: `parser/tests/stmts.rs`,
`checker/tests/matching.rs`, `differential.rs`.

- [ ] **Step 1 — failing parser test:** `if (var u = lookup(id) when u.active) { … }` parses with a guard.
- [ ] **Step 2 — parser:** in the if-let / while-let path (after the binding initializer, before `)`),
  accept a contextual `when <expr>` and store it on the lowered node (the existing if-let desugaring gains
  an optional guard conjunct).
- [ ] **Step 3 — checker:** the guard is typed `Ty::Bool` in the scope where the bound (narrowed,
  non-null) variable is visible.
- [ ] **Step 4 — backends:** lower as "binding succeeded AND guard true" — interpreter: eval guard after
  the successful bind; compiler: `JumpIfFalse` after the bind test + guard; transpile: the if-let already
  emits a null-check condition; append `&& (<guard>)`.
- [ ] **Step 5 — tests:** differential — if-let with a guard that passes vs. fails (falls to else/skips
  loop); checker — non-bool guard rejected.
- [ ] **Step 6 — gate + commit:** `feat(lang): if-let / while-let guards (patterns S5.1-T2)`.

### Task S5.1-T3 — guards example + docs
**Files:** Create `examples/guide/pattern-matching.phg` (guards section only for now); Modify
`examples/README.md`, `KNOWN_ISSUES.md`, the `phg explain` table.

- [ ] **Step 1:** write `pattern-matching.phg` exercising match guards + an if-let guard, producing
  deterministic `Ok` output (exact-representable values only — [[examples-ship-with-features]]).
- [ ] **Step 2:** add the `examples/README.md` index + coverage row.
- [ ] **Step 3:** add `phg explain` entries for `E-MATCH-GUARD-EXHAUST` and `E-GUARD-TYPE`.
- [ ] **Step 4 — gate** (the example glob now byte-identity-gates it + real PHP) **+ commit:**
  `docs(patterns): guards guide example + explain codes (S5.1-T3)`.

---

## Sub-slice S5.2 — Struct / nested destructuring

### Task S5.2-T1 — `Pattern::Struct` (class field destructuring, shorthand + rename + nesting)
**Files:** Modify `src/ast/mod.rs` (new Pattern variant + `StructFieldPat`/`FieldTarget`),
`src/parser/patterns.rs` (parse), `src/checker/matches.rs` (check_pattern + match_arm_key),
`src/interpreter/mod.rs` (match_pattern), `src/compiler/matches.rs` (emit_pattern_test),
`src/transpile/matches.rs` (emit_match). Test: `parser/tests/patterns.rs`, `checker/tests/matching.rs`,
`differential.rs`.

- [ ] **Step 1 — failing parser test:** `Point { x, y }`, `Point { x: px }`, `Line { from: Point { x, y }, to }`
  parse to `Pattern::Struct` with the right `FieldTarget`s.
- [ ] **Step 2 — AST:** add `Pattern::Struct { type_name: String, fields: Vec<StructFieldPat>, span: Span }`;
  `pub struct StructFieldPat { pub field: String, pub target: FieldTarget }`;
  `pub enum FieldTarget { Bind(String), Sub(Pattern) }`. (Adding a Pattern variant forces every
  `match … Pattern` to gain an arm — checker, interpreter, compiler, transpile, `ast::free_vars` if it
  walks patterns. Compile errors enumerate the sites; handle each.)
- [ ] **Step 3 — parser** (`parse_pattern`): in the `TokenKind::Ident(name)` branch, before the
  `LParen` check, add: if the name is PascalCase and the next token is `LBrace`, parse a brace field list
  — each entry is `field` then optional `: <ident|pattern>` (ident ⇒ `Bind(that)`; pattern ⇒ `Sub`;
  bare ⇒ `Bind(field)`); recurse `parse_pattern` for a `Sub`.
- [ ] **Step 4 — checker** (`check_pattern` Struct arm): resolve `type_name` to a class
  (`E-STRUCT-PAT-TYPE` else); each `field` must exist on the class (`E-STRUCT-FIELD-UNKNOWN`); a `Bind`
  declares a local typed from the field's declared type **and registers its `CTy`** (the operand trap,
  [[cty-tracks-operand-types]]); a `Sub` recurses against the field type. Duplicate bind names in one
  pattern → `E-PATTERN-DUP-BIND`. `match_arm_key`: a struct pattern keys like a type pattern (single
  type test; doesn't change exhaustiveness obligations).
- [ ] **Step 5 — interpreter** (`match_pattern` Struct arm): `value` must be an `Instance` of
  `type_name`; for each field read the instance field, then `Bind` → install local, `Sub` → recurse
  `match_pattern` (fail the arm if a sub-pattern fails).
- [ ] **Step 6 — compiler** (`emit_pattern_test` Struct arm): emit `Op::IsInstance(type_name)` +
  `JumpIfFalse`; for each field emit the field-read onto a path slot then `Bind` (register local) or
  recurse the sub-pattern test. Reuse the existing `path`/`skips` machinery.
- [ ] **Step 7 — transpile** (`emit_match` Struct arm): condition `($subj instanceof Point)`, binds
  `$x = $subj->x;` in the body (or as `(($x=$subj->x)||true)` conjuncts when the arm is also guarded);
  nested → recurse with `$subj->from` as the new subject and a conjoined `instanceof`.
- [ ] **Step 8 — checker tests:** the three new codes (`E-STRUCT-PAT-TYPE`, `E-STRUCT-FIELD-UNKNOWN`,
  `E-PATTERN-DUP-BIND`); a valid shorthand/rename/nested pattern type-checks.
- [ ] **Step 9 — differential:** shorthand `Point { x, y }`, rename `Point { x: px }`, nested
  `Line { from: Point { x, y }, to }`, and a **CTy operand** case (`Point { x, y } => x + y`). Assert
  `run ≡ runvm` + real PHP.
- [ ] **Step 10 — gate + commit:** `feat(lang): class/struct destructuring patterns (patterns S5.2-T1)`.

### Task S5.2-T2 — nested type-patterns in variant payloads (`Wrapper(Circle c)`)
**Files:** Modify `src/checker/matches.rs` (lift the top-level-only restriction), verify the 3 backends'
Variant arms recurse type patterns. Test: `differential.rs`, `checker/tests/matching.rs`.

- [ ] **Step 1 — failing differential test:** `match w { Wrapper(Circle c) => c.r, Wrapper(Square s) => s.side, _ => 0.0 }`
  — currently `E-MATCH-TYPE` (top-level only). It should type-check and run identically.
- [ ] **Step 2 — checker:** remove the "type patterns are top-level-only" rejection (the `E-MATCH-TYPE`
  guard around the Variant-field loop @~210) so a `Pattern::Type` element inside `Variant.fields` is
  checked recursively (type-test + bind narrowed). Keep exhaustiveness over the payload's union/enum
  honest (a `Wrapper(Circle c)` alone doesn't cover `Wrapper(Square …)`).
- [ ] **Step 3 — backends:** confirm `match_pattern` / `emit_pattern_test` / `emit_match` already recurse
  into `Variant.fields` (they iterate the field patterns); a `Pattern::Type` element now flows through
  the same `IsInstance` path. Add arms only where a site special-cased "top-level type pattern."
- [ ] **Step 4 — tests:** the differential above + a CTy operand case (`Wrapper(Circle c) => c.r + 1.0`);
  a non-exhaustive payload union → exhaustiveness error.
- [ ] **Step 5 — gate + commit:** `feat(lang): nested type-patterns in variant payloads (patterns S5.2-T2)`.

### Task S5.2-T3 — destructuring example + docs
**Files:** Modify `examples/guide/pattern-matching.phg` (+ struct/nested section), `examples/README.md`,
`KNOWN_ISSUES.md` (remove the "type pattern nested in a variant payload" deferral row), `phg explain` table.

- [ ] **Step 1:** extend the guide example with shorthand/rename/nested destructuring producing
  deterministic output.
- [ ] **Step 2:** add `phg explain` entries for the three S5.2 codes; remove the now-fixed KNOWN_ISSUES row.
- [ ] **Step 3 — gate + commit:** `docs(patterns): destructuring guide + explain codes (S5.2-T3)`.

---

## Sub-slice S5.3 — Flow-narrowing engine (checker-only)

### Task S5.3-T1 — extract `narrow_from_condition` (behavior-preserving refactor)
**Files:** Modify `src/checker/stmt.rs` (the inline `Stmt::If` narrowing @111) + a new helper (same
module or `checker/expr.rs`). Test: existing narrowing tests in `checker/tests/*` must stay green.

- [ ] **Step 1:** write the helper `narrow_from_condition(&self, cond: &Expr, polarity: bool) -> Vec<(String, Ty)>`,
  initially recognizing exactly today's sources at `polarity = true`: `x instanceof T` → `x:T`; if-let
  binding → non-null inner. (No behavior change yet — the else/false path returns empty.)
- [ ] **Step 2:** rewrite the `Stmt::If` then-block narrowing to call the helper with `polarity = true`
  and install the returned shadows (preserving the M-mut.1 mutability-inheritance rule).
- [ ] **Step 3 — gate** (existing S1/S2 narrowing tests prove no regression) **+ commit:**
  `refactor(checker): extract narrow_from_condition (patterns S5.3-T1)`.

### Task S5.3-T2 — else / negative narrowing
- [ ] **Step 1 — failing checker test** `matching.rs`: `if (s instanceof Circle) {} else { /* s : remaining union */ }`
  — the else-branch reads a remaining-member method/field and type-checks; and `if (x != null) {} else { /* x is null */ }`.
- [ ] **Step 2:** implement the `polarity = false` forms in `narrow_from_condition`: `instanceof T` →
  `Ty::union_of(members ∖ T)` (re-normalized; no-op when not a union); `== null`/`!= null` swap; `!cond`
  flips polarity; `a && b` conjoins (true side only). Apply the false-set to the **else-block** scope in
  `Stmt::If`.
- [ ] **Step 3 — differential:** a runtime path that exercises the narrowed else value on both backends.
- [ ] **Step 4 — gate + commit:** `feat(checker): else/negative flow-narrowing (patterns S5.3-T2)`.

### Task S5.3-T3 — early-return narrowing
- [ ] **Step 1 — failing checker test:** `if (!(s instanceof Circle)) { return …; } /* s : Circle here */`
  type-checks against `Circle`'s surface for the rest of the block.
- [ ] **Step 2:** in `Stmt::If` (no/empty else, then-block diverges per `block_terminates` from the
  totality cluster), apply the `polarity = false` narrowings to the **statements after the `if`** in the
  enclosing block. (Thread the narrowed shadows into the remaining-statement check.)
- [ ] **Step 3 — differential** runtime path + **gate + commit:**
  `feat(checker): early-return flow-narrowing (patterns S5.3-T3)`.

### Task S5.3-T4 — post-exhaustive-match narrowing
- [ ] **Step 1 — failing checker test:** `match opt { null => return d, _ => {} } /* opt : non-null */`
  (and a class-union variant) narrows the scrutinee for the rest of the block.
- [ ] **Step 2:** after a `match` *statement* whose arms all diverge (`block_terminates`) except one,
  narrow the scrutinee variable to the surviving arm's pattern type for the remainder of the block.
- [ ] **Step 3 — differential** runtime path + **gate + commit:**
  `feat(checker): post-exhaustive-match narrowing (patterns S5.3-T4)`.

### Task S5.3-T5 — equality / literal refinement
- [ ] **Step 1 — failing checker test:** on a primitive-union `x: int | string`, inside
  `if (x == "ok") { /* x : string */ }` the string branch type-checks string ops.
- [ ] **Step 2:** add the `x == <literal>` source to `narrow_from_condition` (true-branch only): narrow a
  primitive-union scrutinee to the literal's member type. (No false-branch narrowing — a single literal
  doesn't exclude a whole member.)
- [ ] **Step 3 — differential** + **gate + commit:** `feat(checker): equality/literal refinement (patterns S5.3-T5)`.

### Task S5.3-T6 — narrowing example, docs, milestone close
**Files:** Modify `examples/guide/pattern-matching.phg` (flow-narrowing section), `examples/README.md`,
`KNOWN_ISSUES.md` (remove "negative/flow narrowing" + "no flow-typing beyond structural termination"
rows; add the `||`-disjunction + common-member-on-raw-union deferrals), `docs/MILESTONES.md`,
`CHANGELOG.md`, `CLAUDE.md` (mark the slice), `phg explain` if any code added.

- [ ] **Step 1:** finalize the guide example with a flow-narrowing idiom (else + early-return) producing
  deterministic output; run `cargo build --release` and confirm `target/release/phg`
  ([[build-binary-after-each-feature]]).
- [ ] **Step 2:** update KNOWN_ISSUES (remove fixed rows, add deferrals), CHANGELOG, MILESTONES, the
  CLAUDE.md M-RT-follow-up note.
- [ ] **Step 3 — full gate + commit:** `docs(patterns): flow-narrowing guide + close pattern cluster (S5.3-T6)`.

### Self-review (writing-plans)
- **Spec coverage:** guards (S5.1) · struct+nested+payload destructuring (S5.2) · all four narrowing
  forms (S5.3-T2..T5) · diagnostics (per-task) · example+docs (T3/T6) — every spec §3–§7 item has a task.
- **No new `Op`/`Value`:** asserted in Global Constraints; each backend task reuses existing ops.
- **Type consistency:** `narrow_from_condition(&Expr, bool) -> Vec<(String, Ty)>`, `Pattern::Struct{type_name,fields,span}`,
  `FieldTarget::{Bind,Sub}`, `MatchArm.guard: Option<Expr>` — used consistently across tasks.
- **Deferrals** (KNOWN_ISSUES at T6): `||`-disjunction narrowing; common-member access on a raw union;
  or-patterns; range/slice patterns.

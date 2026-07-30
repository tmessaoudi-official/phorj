# Mechanical exhaustiveness for `Expr` / `Stmt` / `Pattern` (DEC-356, RULED 2026-07-26)

> **Status:** RULED by the developer 2026-07-26, **not yet built**. Canonical home for the rule
> (Invariant 19). Decision *identity + status* = the DEC-356 row in
> `docs/research/full-audit/raw/C-decisions.md`. Original analysis:
> `docs/research/2026-07-25-completeness-register.md` §6.4.

## Why this is the structural item, not one more bug

Every P0/P1 ruled in this agenda so far shipped because **a match arm silently passed something
through**. This decision addresses the class rather than the instances.

Measured state — *[Verified 2026-07-26 by direct grep/read]*:

- **`Expr` 37 variants** (`src/ast/exprs.rs`), **`Stmt` 15** (`src/ast/stmts.rs`), **`Pattern` 11**
  (`src/ast/types_core.rs`).
- **Exactly 17 named catch-alls** (`other => other`, `leaf => leaf`) across **10** checker files:
  `resolve_variant_imports.rs` ×3 · `desugar_db.rs` ×2 · `desugar_router.rs` ×2 · `rewrite_html.rs` ×2 ·
  `rewrite_ufcs.rs` ×2 · `desugar_di/walker.rs` ×2 · `rewrite_generics.rs` · `rewrite_invoke_tostring.rs` ·
  `overloads.rs` · `desugar_di/mod.rs`.
- **`src/checker/desugar_db.rs:67-69` literally declares the invariant it then breaks:** *"INVARIANT —
  keep the rewriter TOTAL … A new expression-bearing AST node → add its arm here"* — and the same file
  closes with `other => other` at `:2644` and `leaf => leaf` at `:2967`.
- **`src/ast/walk.rs:748`** has `_ => {}` in `collect_pattern_bindings`, immediately beneath a comment
  recording that missing a pattern form *"would drop struct-bound names from `free_vars`, miscompiling a
  lambda that captures one (the guard-recursion lesson)"*. The comment documents the bug having already
  fired; the catch-all that lets it fire again is on the next line.

**A named catch-all is worse than `_`**: it compiles cleanly, reads as deliberate, and greps as a
*handled* case.

## THE RULING

> **Fix all 18 known catch-all sites AND land the gate that keeps them fixed — as ONE slice.**
> `B` (a single shared total visitor) is a **separately-ruled follow-up**, not part of this slice.

The register presented D / C / B as ranked alternatives. **They are not alternatives:**

- **D alone decays** — nothing prevents catch-all #19.
- **C alone ships a gate over 18 known-broken sites** — the gate would pass while the bugs remain.
- **B becomes safe only after D** — once every site has explicit arms, the compiler *enumerates* the
  blast radius that a shared visitor must preserve. Attempting B first means refactoring 13 rewriters
  with their own semantics against the byte-identity spine with no such enumeration.

### D — the fix

Replace the 17 checker catch-alls plus `walk.rs:748` with explicit arms.

**`walk.rs:748` specifically: explicit *named no-op* arms, NOT `unreachable!()`.** The pattern forms
that bind nothing (wildcard, literal, …) are perfectly *reachable* — they simply contribute no
bindings. `unreachable!()` would be factually wrong and would put a panic on a checker path. Listing
them by name gives the same compiler enforcement with no new failure mode.

### C — the gate

A never-constructed probe variant (e.g. `Expr::__ExhaustivenessProbe` behind `#[cfg(test)]`) whose
addition must break the build in every rewriter that should care. Note the honest limitation: a match
that still carries a catch-all keeps compiling, so C enforces *"a new variant is considered"* only in
matches D has already made total — which is exactly why the two ship together.

## Invariant 3 is extended in the same change

Invariant 3 already mandates this discipline one layer down: a new `Op` variant must extend three
exhaustive matches, *"all wildcard-free (verified 2026-07-25) — never reintroduce a `_` arm."* This
ruling applies the identical rule to `Expr`/`Stmt`/`Pattern`, so **Invariant 3's wording in
`CLAUDE.md` + `docs/INVARIANTS.md` is widened to cover them** — written down rather than remembered.
The precedent matters: the project has already proven it can hold this line for `Op`.

## RE-MEASURED 2026-07-30 — the inventory above has DECAYED, and `walk.rs` is BUILT

*[Verified 2026-07-30 by classifying every hit by the enum it matches on.]* The ruling's own prediction
("**D alone decays** — nothing prevents catch-all #19") came true **before D shipped**:

| | 2026-07-26 (spec) | 2026-07-30 (measured) |
|---|---|---|
| named catch-alls in `src/checker/` | 17 | **26** |

By enum: **8 `Expr`** · **2 `Stmt`** · **1 `Pattern`** · 10 `Item` · 4 `Ty` · 1 unclassified. The five new
ones arrived in `desugar_db.rs` (+2), `rewrite_ufcs.rs` (+1), `desugar_di/walker.rs` (+1) and
`rewrite_generics.rs` (+1); the four `Ty` ones (`calls/methods.rs`, `calls/args.rs`, `calls/member.rs`,
`common.rs`) the original count never listed at all.

**BUILT so far:** `src/ast/walk.rs`'s `collect_pattern_bindings` — the site the ruling named explicitly —
now carries **named no-op arms** (`Wildcard | Int | Float | Decimal | Str | Bool | Null |
Type { binding: None, .. } => {}`), not `unreachable!()`, exactly as ruled. `walk.rs` was 812 lines
(62% over the hard cap), so its inline test module was split to `walk_tests.rs` rather than squeezing
comments to hold a grandfathered ceiling.

**The 26 rewriter sites are NOT yet done, and the reason is measured, not assumed.** The pass order in
`src/cli/pipeline.rs` runs `erase_tuples` *after* `resolve_html` / `erase_generics` / `rewrite_ufcs` /
`desugar_db` / `desugar_di` / `desugar_router` / `resolve_variant_imports`, so `Expr::Tuple` IS live at
all seven — their catch-alls really do swallow it. Static analysis says every one of the seven also
misses `NamedArg`, and most miss `NewColl` / `Pipe` / `Inject` / `TaggedTemplate`:

| walker | handles / 37 | expression-bearing forms it misses |
|---|---|---|
| `desugar_db.rs:2966` | 24 | Tuple, NamedArg, Inject, Pipe |
| `desugar_di/walker.rs:607` | 23 | Tuple, NamedArg, NewColl, TaggedTemplate, Pipe |
| `resolve_variant_imports.rs:430` | 23 | Tuple, NamedArg, NewColl, TaggedTemplate, Inject, Pipe |
| `rewrite_ufcs.rs:291` | 23 | Tuple, NamedArg, NewColl, TaggedTemplate, Inject, Pipe |
| `rewrite_html.rs:211` | 21 | Tuple, NamedArg, NewColl, InstanceOf, Cast, Inject, Pipe |
| `rewrite_generics.rs:310` | 21 | Tuple, NamedArg, NewColl, New, TaggedTemplate, Inject, Pipe |
| `desugar_router.rs:406` | 20 | Tuple, NamedArg, NewColl, ParentCall, OverloadSelect, TaggedTemplate, Inject, Pipe |

**But a static miss is not automatically a live bug, and the first probe proved it.** A generic call
inside a tuple (`var (a, b) = (pick<int>(1, 2), 3);`) works on both backends despite `erase_generics`
statically missing `Tuple` — so each cell needs its own reproduction before a fix is written (Rule 14).
That per-cell probing, plus a differential case for each real find, is what makes the remaining work a
slice of its own rather than a mechanical sweep. **Technique for the fix, when it happens:** an
`e @ (Expr::A(..) | Expr::B(..) | …) => e` or-pattern arm gives full compiler enforcement (a new variant
not in the list is a non-exhaustive-match error) at ~10 lines per site instead of 37 — which is what keeps
D compatible with Invariant 13's caps.

## Definition of done

1. All 17 checker catch-alls replaced with explicit arms; `walk.rs:748` replaced with named no-op arms.
2. The probe-variant gate in place, with a test proving it fails when a variant is added to a match
   that should handle it.
3. Invariant 3 widened in `CLAUDE.md` and `docs/INVARIANTS.md` to name `Expr`/`Stmt`/`Pattern`.
4. Full ALL-FEATURES gate green — this touches 10 checker files, so the byte-identity spine is the
   regression surface that matters.
5. `B` (shared total visitor) recorded as a QUEUED follow-up decision, not silently dropped.

# SPEC (RULED — BUILD-READY, NOT YET BUILT) — Wildcard & group imports (`import X.Y.*` / `{A,B}`)

> Status: **RULED 2026-07-24 (dev, AskUserQuestion) — BUILD-READY, NOT BUILT.** Captured from an
> interactive design session per Invariant 15 (adjudication is the developer's) + Invariant 19
> (records live in the repo). All sub-decisions ruled (see RULED sections below). Its own slice
> (Q-A in the import/visibility cluster), NOT part of DEC-331 s3; built BEFORE the visibility-model
> slice (Q-B). Mirrored as QUEUED in MASTER-PLAN + SLICE-STATE.

## Problem

phorj today has only single-member imports: `import X.Y.Z;` (brings bare `Z`), value-leaf
(`import Core.Output.printLine;`), and alias (`import A.B as C;`). There is no way to bring several
members at once. Same-package cross-file symbols are ALREADY implicitly visible (no import) —
[Verified: `src/loader/mod.rs:67` "Same file → always legal. Same package, different file → legal
unless `private`"] — so wildcard/group imports are a CROSS-package convenience only.

## Cross-language survey (META-7, Inv 16)

Go: no wildcard (package-qualified `p.Name`). Rust: `use p::*` glob (Clippy warns) + idiomatic
`use p::{A,B}`. Java: `import x.*` allowed, style guides forbid. TS: `import * as ns` = namespace
object (`ns.Name`) + named `{A,B}`. C#: `using X.Y;` flat whole-namespace. Kotlin: `import x.*`
flat. **Consensus:** the group form `{A,B}` is the safe idiom; flat `*` is convenient but
collision-prone; namespace-binding (`* as ns`) is collision-free but changes call syntax.

## Unifying principle (resolves most grey areas with ZERO new semantics)

> `import X.Y.*` ≡ writing an explicit `import X.Y.Z;` for **every member Z of X.Y you'd be allowed
> to import individually** — same visibility rules, same collision rules. It is **compile-time sugar**
> expanded in `cli::check_and_expand` BEFORE any backend (Inv 5), so interp/VM/transpiled-PHP never
> see `*` (PHP `use` stays per-symbol; byte-identity preserved — Inv 17). Determinism (Inv 10): the
> expanded set is sorted.

## RULED so far (dev, AskUserQuestion, 2026-07-24)

- **D1 — Forms: BOTH.** `import X.Y.*;` (all public+internal immediate members) AND
  `import X.Y.{A, B};` (explicit multi-member). Shared resolver + shared compile-time expansion.
- **D2 — Collisions: EAGER error on ANY overlap.** If two wildcard imports bring the same name, the
  program does NOT compile (`E-IMPORT-AMBIGUOUS`), whether or not the name is used. The escape hatch
  is an explicit `import X.Y.Z;` (disambiguates by naming the winner) — and the `except` clause (D5).
  (Stricter than the lazy/use-site alternative; dev chose strict.)
- **D3 — What `*` binds: all PUBLIC + INTERNAL immediate members, shallow.** Never `private`
  (file-scoped). NOT sub-packages ("except embedded packages"). `protected` is N/A — it is a
  class-member modifier, not a top-level `Visibility` (`Visibility` = Private<Internal<Public,
  [Verified: `src/ast/exprs.rs:397`]). `internal` is only bound where an explicit cross-package
  import could already reach it (existing loader:832 rule). *(dev: "option 1 for now" — treat as
  ruled unless revisited.)*
- **D4 — Scope: any submodule, NOT bare `Core.*`.** Allowed on project packages, vendored packages,
  and stdlib SUBMODULES (`import Core.Http.*;`). Bare-root `import Core.*;` is `E-WILDCARD-STDLIB-ROOT`
  (would flood the file with the entire stdlib).
- **D-process — walk the remaining grey areas ONE-BY-ONE** (dev chose depth over bundling).
- **D3 CONFIRMED — `*` binds public + internal** (round 2). `internal` included; `private` never;
  `protected` N/A (class-member axis).
- **D5 RULED — exclusion clause `import X.Y.* except { A, B };`** (keyword `except`, not `hiding`).
  Removes names from the wildcard set before expansion; the ergonomic escape hatch for the strict
  D2 eager-collision rule (except the clash, then re-import it aliased). **sub-open RULED: excepting a
  name NOT in the resolved wildcard set = HARD ERROR `E-EXCEPT-UNKNOWN`** (with did-you-mean hint;
  aligns with the empty=hard-error + E-IMPORT-UNKNOWN 'names must resolve' stance).
- **(a) RULED — aliasing: group + re-import only.** `import X.{A as B, C};` ok; `import X.* as Y;` =
  `E-WILDCARD-ALIAS` (flat wildcard has no single name). Namespace-object `* as ns` explicitly NOT
  this slice.
- **(b) RULED — empty/no-op wildcard = HARD ERROR** `E-WILDCARD-EMPTY` (dev overrode the warn
  proposal). A `*` (or `except` set) that binds zero new names fails to compile.
- **G6 RULED — `E-IMPORT-UNKNOWN` lands in THIS slice** (dev, 2026-07-24; cross-linked from the
  visibility-model spec). Today `import Acme.Nothing;` is silently accepted while unused
  ([Verified: H-enforcement audit §2.3 / M2] — unknown import error-at-site is missing). This slice
  adds it for single, group `{}`, AND wildcard `*` forms (Go model — beats PHP's silent `use`):
  an import naming a non-existent package/member is a compile error at the import line, whether or
  not it is used. Applies in loose, project, and vendored modes.

## RULED — micro-opens closed (dev, AskUserQuestion, 2026-07-24). Spec is now BUILD-READY.

- **(c) Unused import RULED — ADD `W-UNUSED-IMPORT` in THIS slice, scoped to wildcard/group forms**
  (dev overrode the defer recommendation). NOTE [Verified: H-audit M3]: phorj has NO unused-* lints
  today, so this is the FIRST. **Coherence note (dev-aware):** a future `W-UNUSED-*` family (M3)
  should ABSORB this and extend the same warning to single `import X.Y;` — this slice ships the
  wildcard-scoped subset; the family slice unifies coverage so we don't end with 'warns unused `*`
  but not unused single import' permanently.
- **(d) Determinism RULED (confirmed)** — the `*`/`except` expansion is SORTED before use (Inv 10).
- **(e) `phg format` RULED — SORT members alphabetically** inside `{ … }` AND `except { … }`
  (canonical, deterministic, zero re-run churn; matches the sorted `*` expansion).
- **(f) Transpile/lift RULED (confirmed)** — transpile EXPANDS `*`/`{}`/`except` to per-symbol,
  alphabetically-sorted PHP `use` (Inv 5 compile-time expansion; byte-identity safe — PHP has no
  wildcard `use`). Lift NEVER emits `*`/`{}`/`except` (PHP source has no such form).
- **Grammar RULED** — `{ }` and `except { }` use a DEDICATED import-list parser (not the
  attribute/named-arg machinery). Exact `*` token: lexer/parser detail for the build.
- **Same-package local shadowing** — existing `local > imported` (loader:829) applies unchanged
  (local wins); add a differential + checker case to pin it. (Mechanical, no ruling needed.)
- **Error/warning-code catalog (all NEW; wire `phg explain` entries):** `E-IMPORT-AMBIGUOUS`
  (D2 eager collision), `E-WILDCARD-STDLIB-ROOT` (bare `Core.*`), `E-WILDCARD-ALIAS` (`* as Y`),
  `E-WILDCARD-EMPTY` (binds nothing new), `E-EXCEPT-UNKNOWN` (excepts an absent name),
  `E-IMPORT-UNKNOWN` (import names a non-existent package/member), `W-UNUSED-IMPORT` (wildcard/group).

## Backends / invariants checklist (for the eventual build)

- Inv 5: expand before backends (compile-time sugar).  Inv 10: sorted expansion.  Inv 17: transpile
  AND lift updated same change; `phg check` ≡ LSP.  Inv 9: shipped example + README entry.
  Inv 13: new parser/checker code split-as-you-go.  Differential: examples exercising `*`, `{}`,
  `except`, and a collision→explicit-fix case.

# SPEC (RULED — BUILD-READY, NOT YET BUILT) — Wildcard & group imports (`import X.Y.*` / `{A,B}`)

> Status: **RULED 2026-07-24 (dev, AskUserQuestion) — BUILD-READY, NOT BUILT.** Captured from an
> interactive design session per Invariant 15 (adjudication is the developer's) + Invariant 19
> (records live in the repo). All sub-decisions ruled (see RULED sections below). Its own slice
> (Q-A in the import/visibility cluster), NOT part of DEC-331 s3; built BEFORE the visibility-model
> slice (Q-B). Mirrored as QUEUED in MASTER-PLAN + SLICE-STATE.

## Problem

phorj today has single-member imports (`import X.Y.Z;`, value-leaf, alias `import A.B as C;`) AND —
**[Verified 2026-07-25 against `src/parser/items/decls.rs:222` `parse_import_group`]** — the GROUP
form `import P.{ a, b as c };` (DEC-186) already exists, expanded to one `Item::Import` per member at
PARSE time, with per-member aliasing and an empty-`{}` guard (`E-IMPORT-GROUP-EMPTY`). Same-package
cross-file symbols are ALREADY implicitly visible (no import) [Verified: `src/loader/mod.rs:67`].

> **BUILD RE-SCOPE (2026-07-25, autonomous — corrects this spec's original premise):** the `{A,B}`
> group form is DONE (DEC-186). The genuinely-missing Q-A surface is: **(1) wildcard `import X.Y.*;`**,
> **(2) `except { … }`**, and **(3) the diagnostics** `E-IMPORT-AMBIGUOUS` / `E-WILDCARD-STDLIB-ROOT`
> / `E-WILDCARD-ALIAS` / `E-WILDCARD-EMPTY` / `E-EXCEPT-UNKNOWN` / `E-IMPORT-UNKNOWN` /
> `W-UNUSED-IMPORT`. **Pipeline placement:** group expansion is purely syntactic (parser); wildcard
> `*` needs the target package's member set + visibility, which the LOADER already indexes
> (`index_packages`/`peek_package`, `vis_violation` mod.rs:69) — so `*`/`except` expand in the loader
> (or a loader-fed `check_and_expand` step), producing plain per-symbol `Item::Import`s before any
> backend (Inv 5). This is an implementation-placement call (not a user-visible design fork); the
> ruled semantics (D1–D5, catalog) are unchanged. `E-WILDCARD-EMPTY` = wildcard binds zero NEW names
> (distinct from the pre-existing `E-IMPORT-GROUP-EMPTY` = literal empty `{}`).

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

## BUILD PLAN — TDD steps (autonomous, 2026-07-25; execute in order, gate+commit each)

Ground truth: groups `{A,B}` DONE (parser `parse_import_group`, DEC-186). Loader has package→member
surface (`index_packages` mod.rs:82; `known_type`/`known_function`/`prov_fns` + `vis_violation`
mod.rs:69). `*` is `TokenKind::Star` (already tokenized for `*`). Expansion belongs in the LOADER
(has members+visibility), producing per-symbol `Item::Import` before backends (Inv 5).

1. **Parser — wildcard + except** (`parser/items/decls.rs::parse_import`): after `.`, accept `Star`
   → a wildcard import; then optional contextual `except { a, b }` (reuse the group-list reader).
   `import X.* as Y;` → `E-WILDCARD-ALIAS`. Represent as a new `Item::Import` shape (add
   `wildcard: bool` + `except: Vec<String>`, or a sibling `Item::ImportWildcard{prefix,except,span}`).
   Tests: parse `X.Y.*;`, `X.Y.* except {A};`, `X.* as Y` → E-WILDCARD-ALIAS.
2. **Loader — expansion + diagnostics**: when assembling, expand each wildcard to the SORTED set of
   public+internal immediate members of the target package (types+functions; NOT sub-packages, NOT
   private); subtract `except`; emit per-member `Item::Import`. Diagnostics: bare `Core.*` →
   `E-WILDCARD-STDLIB-ROOT`; zero-new-binding → `E-WILDCARD-EMPTY`; `except` name absent →
   `E-EXCEPT-UNKNOWN` (did-you-mean); two wildcards → same leaf → `E-IMPORT-AMBIGUOUS`.
   Tests (loader/tests.rs + project fixtures): each code fires; a clean `*` binds the expected set.
3. **E-IMPORT-UNKNOWN** (G6): any import (single/group/`*`) naming a non-existent package/member →
   error at the import line, used or not, in loose+project+vendored. (Widen existing resolve check.)
4. **W-UNUSED-IMPORT** (wildcard/group scope): warn when a wildcard/group-bound name is never used.
   (First unused-lint; scoped per the coherence note.)
5. **`phg explain`**: register all 7 codes (E-IMPORT-AMBIGUOUS/WILDCARD-STDLIB-ROOT/WILDCARD-ALIAS/
   WILDCARD-EMPTY/EXCEPT-UNKNOWN/IMPORT-UNKNOWN, W-UNUSED-IMPORT).
6. **Transpile/lift**: auto (expansion → plain imports before backends); assert transpiled PHP shows
   sorted per-symbol `use`; lift never emits `*`/`except`. **format**: sort `{}`/`except {}` members.
7. **Example + README** (Inv 9): `examples/guide/wildcard-imports.phg` exercising `*`, `except`, and a
   collision→explicit-fix; `examples/README.md` entry. Differential covers it (VM≡tree-walker≡PHP).
8. **Gate + DEC-268 panel** (this feature IS substantial → full 3-lens fresh-context reviewer PANEL,
   two clean rounds) → commit+push.

## STEP 2 DETAIL — loader expansion (grounded 2026-07-25; step 1 shipped f8c5224)

**Hook point:** `src/loader/mod.rs` — AFTER Pass-1 builds the index (~line 613), BEFORE Pass-2 rewrite.
Pass-1 yields: `defined: HashMap<(pkg,name)→mangled>` (functions), `types: HashMap<(pkg,name)→mangled>`,
`prov_fns`/`prov_types: HashMap<(pkg,name)→DefInfo{vis}>`. Wildcard `Item::Import`s live in
`parsed: Vec<(PathBuf, Program)>` items. Expand IN PLACE (replace each wildcard import with its
per-member `Item::Import { path: prefix+[name], wildcard:false }` list) before Pass-2 rewrites items.

**Two enumeration sources:**
- **User/vendored package** (prefix P present in the index): members = `{ name : (P,name) ∈ defined∪types
  AND vis(prov) ∈ {Public, Internal} }`. (Private is file-scoped, excluded.) Requires the wildcard to
  trigger loading of package P (same as a normal `import P.Member` — the import-graph walk mod.rs:270).
- **Native `Core.*` submodule** (e.g. `Core.Http`): members via the native registry — promote
  `src/lsp/catalog.rs::module_members(qualifier)->Vec<String>` (currently `pub(super)`) to a
  `pub(crate)` native enumerator (or add `native::module_members`), reuse it here.

**Algorithm per wildcard import `{prefix, except}`:** (1) if `prefix == ["Core"]` → `E-WILDCARD-STDLIB-ROOT`.
(2) resolve member set from the right source; (3) every `except` name must be IN the set else
`E-EXCEPT-UNKNOWN` (did-you-mean); (4) subtract `except`; (5) SORT; (6) if the resulting NEW-binding set
is empty → `E-WILDCARD-EMPTY`; (7) if any produced leaf collides with another wildcard's leaf (or an
already-bound name) → `E-IMPORT-AMBIGUOUS`; (8) emit per-member imports.

**All-paths guarantee:** the loader covers loose + project (the real cross-package programs). Add a
**checker guard**: any `Item::Import { wildcard: true, .. }` that survives to the checker → internal
error (defensive; expansion must have removed them). The raw `check_and_expand` snippet path is
package-agnostic and can't reference cross-package wildcards.

**Codes + explain:** register E-WILDCARD-STDLIB-ROOT / E-WILDCARD-EMPTY / E-EXCEPT-UNKNOWN /
E-IMPORT-AMBIGUOUS in `src/cli/explain.rs` (the `every_emitted_diagnostic_code_has_an_explanation`
test enforces this). TDD in `src/loader/tests.rs` + `tests/project.rs` fixtures.

## ⬚ PENDING (dev to rule — surfaced during the step-2 build, 2026-07-25)

- ⬚ **P-Q-A-1 — Core-submodule wildcards (`import Core.Http.*`) DEFERRED.** [Verified during build]
  the loader's native/prelude pre-pass intercepts `Core.*` imports BEFORE the wildcard-expansion hook,
  so a Core wildcard never reaches it (a naive attempt silently binds nothing — a false positive).
  Rather than ship silent-wrong behavior, `Core.*` wildcards (bare AND submodule) are **parser-rejected**
  for now (`E-WILDCARD-STDLIB-ROOT`: bare = "floods stdlib"; submodule = "not yet supported — import
  explicitly"). Proper Core-submodule support needs native-registry expansion wired through the prelude
  pass — a follow-up slice. **User/vendored-package wildcards work fully.** (D4 allowed Core.Sub.*; this
  narrows it pending the follow-up.)
- ⬚ **P-Q-A-2 — `*` binds PUBLIC-only cross-package (not "public+internal" as D3's shorthand said).**
  [Verified: `loader::vis_violation` mod.rs:69 — a cross-package `internal` member is `E-VIS-INTERNAL`,
  i.e. NOT individually importable]. Implemented per the spec's own **unifying principle** ("every
  member you'd be allowed to import individually" = `vis_violation`-legal): cross-package `*` binds
  public only; same-package would bind public+internal but is already implicitly visible. D3's literal
  "public+internal" wording conflicts with the actual rule for cross-package internal — flagging for
  dev confirmation (the principled behavior is the safe/consistent one).

- ⬚ **P-Q-A-3 — `W-UNUSED-IMPORT` (step 4) DEFERRED to the W-UNUSED-* lint family.** Wildcard/group
  imports are currently EXEMPT from the hard `E-UNUSED-IMPORT` (safe: `check_unused_imports` skips
  `wildcard:true`; expanded members are created after that check). A soft `W-UNUSED-IMPORT` needs
  per-expanded-member usage analysis and is the FIRST of the `W-UNUSED-*` family (audit M3 / C-unused-import)
  — better designed as one coherent lint slice than a wildcard-only one-off. Deferred; recorded here.

## BUILD STATUS (autonomous, 2026-07-25)
Steps 0-1 (parser) ✅ f8c5224 · step 2 (loader expansion + 4 diagnostics) ✅ 6bf9c3b · step 3
(E-IMPORT-UNKNOWN) ✅ 30bc060. Core-submodule wildcard DEFERRED (P-Q-A-1). W-UNUSED-IMPORT DEFERRED
(P-Q-A-3). NEXT: step 5 example (Inv 9) → step 6 format sort → step 7 FEATURES.md → step 8 DEC-268 panel.

## Backends / invariants checklist (for the eventual build)

- Inv 5: expand before backends (compile-time sugar).  Inv 10: sorted expansion.  Inv 17: transpile
  AND lift updated same change; `phg check` ≡ LSP.  Inv 9: shipped example + README entry.
  Inv 13: new parser/checker code split-as-you-go.  Differential: examples exercising `*`, `{}`,
  `except`, and a collision→explicit-fix case.

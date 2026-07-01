# Post-M-DOGFOOD workstreams — fresh autonomous session plan

> Run FULLY AUTONOMOUS (30/8 convergence, commit green work, stop only on genuine forks / risky ops /
> blocked commits). Execute the five workstreams IN ORDER. Each: TDD → byte-identical
> `run≡runvm≡real PHP 8.5` → `cargo fmt --check` (check EXIT CODE, never pipe) + `clippy --workspace
> --all-targets -D warnings` → commit. Gate: `PHORJ_PHP=/stack/tools/phpbrew/php/php-8.5.7/bin/php
> PHORJ_REQUIRE_PHP=1 cargo test --workspace`. `export PATH=/stack/tools/cargo/bin:$PATH`. Enum construct
> = `new V()`; text stdlib = `Core.String`. Autonomous decisions locked with the developer 2026-07-01.

## Decisions Log
- [2026-07-01] AGREED: order = **Audit → 1b → benchmarks → Spec 2 → Clarity**; fully autonomous 30/8.
- [2026-07-01] AGREED: Spec 2 import model = PSR-4 `[packages]` map (optional, default `src/` folder=path),
  first-party bare, `vendor:` prefix for deps, `Core.` stdlib. Specs approved + committed (`8fc85f2`).
- [2026-07-01] AGREED: clarity workstream LAST; **enable blanket `clippy::pedantic` and fix ALL** (developer
  overrode the "selective lints only" rec — wants maximal strictness; expect large churn).

## Workstream 1 — Enforcement audit (soundness) — FIRST
Enumerate EVERY language rule and test that invalid code is rejected. Spot-checks confirmed private
method/ctor/field access ARE enforced — the hole the developer hit is elsewhere, so be SYSTEMATIC.
Approach: for each category, write a should-error test; any program that type-checks when it must not is a
finding → add the check (front-end, clean `E-`/`W-` diagnostic) + test. Categories: visibility
(public/private/protected/internal across all six access sites), mutability (immutable reassign, `mutable`
param already rejected, const), types (assignability, generics invariance, unions/intersections),
null-safety (`T?` non-null guarantee, `!`/`?.`/`??`), exhaustiveness (`match` over enum/union/optional,
guards), totality (return-on-all-paths, `never`), packages (`E-PKG-*`, folder=path), overloading/inheritance
(LSP, final-by-default, abstract), traits, interfaces (unimpl/sig), throws (discharge), reserved names,
casing. Deliverable: a findings table + fixes + a `conformance/` or checker-test suite of should-error cases.
Done: every enumerated rule has a passing should-error test; no known hole.

## Workstream 2 — Slice 1b: field-base index-assign (`this.f[i]=e` / `obj.f[i]=e`)
Extends the shipped nested-value index-assign (`84622c2`, `value::set_nested` + `Op::SetPathLocal`) to a
FIELD base. Currently `E-ASSIGN-TARGET` ("a field base lands in the next slice"). Approach: the place root
can be a field of a shared-mutable instance — checker `place_root` accepts a `Member` base rooted at a
local/`this`; interpreter navigates instance field (`borrow_mut`) then calls `set_nested`; VM extends the
path descriptor with a field-base (root off the stack / `this` slot + a leading field step) OR a sibling
`Op::SetFieldPath`. Byte-identical; guide example; unblocks a Sorter-with-field pattern.

## Workstream 3 — Port remaining benchforge benchmarks (demo, `/stack/projects/phorj-app/`)
Port Search (binary search on a sorted `List` + Map hash-lookup), StringProcessing (`Core.String` ops),
ObjectGraph (shared-mutable instances). Sorting STAYS blocked (in-place recursive quicksort needs by-ref
params — the one genuine blocker; use `List.sort` or skip). Each runs `run≡runvm`. `/stack` repo — do NOT
auto-commit there (separate autonomy scope); leave as demo deliverables + update BENCHFORGE.md/FINDINGS.md.

## Workstream 4 — Spec 2: import-roots (PSR-4) — LARGE BREAKING + codemod
Full spec: `docs/specs/2026-07-01-import-roots-psr4-design.md` (approved). Build order: manifest `[packages]`
parse (`E-PKG-ROOT-*`) → parser `vendor:` prefix → loader root→dir resolution (default src/ folder=path;
`App="src"` alias; extra roots; `vendor:` → vendor/) → checker (`E-UNKNOWN-ROOT`, import map) → transpiler
(namespace = root path, folder-independent) → migration codemod (`tools/import_roots_migrate.py`, dry-run
first) → migrate all examples/projects/fixtures. Output-preserving for single-package first-party programs.

## Workstream 5 — Codebase clarity — LAST
The developer found the compiler hard to understand. (1) Rewrite `docs/ARCHITECTURE.md` as a NARRATED
walkthrough — follow one program lex→parse→check→{interpret|compile→VM|transpile}, naming the key types and
where each lives. (2) Fill missing module `//!` docs (one job per `mod`). (3) **Enable `clippy::pedantic`
workspace-wide and fix ALL** resulting warnings (developer-chosen; large churn — commit in logical batches).
Keep the gate green throughout.

## Progress
- [ ] W1 audit  - [ ] W2 field-base (1b)  - [ ] W3 benchmarks  - [ ] W4 import-roots  - [ ] W5 clarity

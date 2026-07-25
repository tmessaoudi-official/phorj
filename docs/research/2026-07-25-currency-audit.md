# Surface-Currency Audit — 2026-07-25

Invariant 17 (transpile AND lift updated same change as every language/stdlib feature; `phg check`
≡ LSP diagnostics). Verifies that the features shipped tonight are reflected across ALL surfaces —
lifter (PHP→Phorj), transpiler (Phorj→PHP), formatter, and LSP — not just interpreter/VM/checker.

Method: built `target/release/phg`, ran `transpile` / `lift` / `check` / `format` on representative
`.phg`/`.php` inputs, and grepped `src/lift/`, `src/transpile/`, `src/format/`, `src/lsp/`.

**RESULT: No GAPs. Every surface is UP-TO-DATE or N-A with a sound, stated reason.**

Legend: UP-TO-DATE / GAP / N-A.

---

## Feature 1 — Q-A wildcard/group imports (`import X.*`, `import X.* except {A}`, `import X.{A,B}`)

Design anchor: wildcards/groups are pure compile-time sugar — the loader expands them to per-member
`Item::Import` (sorted, public-members-only cross-package) **before any backend**
(`src/loader/import_hygiene.rs::expand_wildcard_imports`, called at `src/loader/mod.rs:504`). By Inv 5,
every backend therefore sees plain per-symbol imports.

| Surface | Status | Evidence |
|---|---|---|
| TRANSPILE | UP-TO-DATE | `phg transpile examples/project/wildcard-imports/src/main.phg` emits the fully-expanded program (cross-package refs fully-qualified, e.g. `new \Acme\Geometry\Rect(3, 4)`); no `*`/`{}` reaches PHP. Group form `import Acme.Geometry.{Rect, Shape};` transpiles identically. `phg run` ≡ `phg run --tree-walker` byte-identical (`area: 12 shape: true`). |
| LIFT | N-A | The Tier-1 lifter never consumes PHP `use`/namespaces — `use`/`namespace` are in `UNSUPPORTED_KW` (`src/lift/parser/mod.rs:22-40`) and the `\` namespace separator is a lex error. `phg lift` on `use Foo\{A, B};` → `lift lex error: unexpected character \`\`\`` (exit 1). The lifter only ever *synthesizes* per-symbol imports (`import Core.Output;`, `import Core.Runtime.Entry;` — `src/lift/lifter/decls.rs`) and prints them one-per-line (`src/lift/printer/items.rs:45-49`). It never produces or ingests wildcard/group forms. Correct by the never-guess contract; no action. |
| FORMATTER | UP-TO-DATE (wildcard) | `phg format -` on the wildcard example preserves `import Acme.Geometry.* except { Paint };` verbatim (`except` sorted, deterministic — `src/format/printer/items.rs:80-84`). Group `{A,B}` is desugared at PARSE time (the AST `Item::Import` has `wildcard`/`except` but no group node — `src/ast/decls.rs:455-470`), so the formatter re-emits it as per-line imports. Pre-existing sugar-desugaring (group imports predate tonight, DEC-186); the tonight-shipped construct (wildcard) round-trips. See Observation 1. |
| LSP | N-A / UP-TO-DATE | Import completion (`import X.`) offers concrete Core modules + project packages (`src/lsp/completion/mod.rs:87-98`), NOT a `.*` shorthand — completing to concrete names is correct; offering `.*` would be an anti-feature. Wildcard diagnostics (`E-WILDCARD-*`, `E-IMPORT-AMBIGUOUS`, `E-EXCEPT-UNKNOWN`) run through the SAME unified loader `phg check` uses (`diagnostics_for_uri` → `load_with_buffer`, `src/lsp/mod.rs:471-493`), so check ≡ LSP. |

---

## Feature 2 — Q-B visibility: member `internal`

`internal` has no PHP analog → erases to PHP `public` (`vis()`, `src/transpile/mod.rs:730-734`;
`is_promoted` routes through `Modifier::is_member_visibility`, `mod.rs:716-717`).

| Surface | Status | Evidence |
|---|---|---|
| TRANSPILE | UP-TO-DATE | On a class exercising every form, `phg transpile` produced: `internal mutable int field` → `public int $field`; `internal const int KONST` → `public const int KONST`; `internal static int counter` → `public static int $counter`; `constructor(internal int promoted)` → `__construct(public int $promoted)` (promoted, field kept); `internal function method()` → bare `function method()` (public-by-default in PHP). `internal` never leaks. `phg run` ≡ `--tree-walker` byte-identical (`12`). |
| LIFT | N-A | PHP has no `internal`; the lifter emits phorj default visibility (no keyword) for public PHP members and never emits `internal`. `phg lift` on plain PHP confirms only default-visibility output. Correct. |
| FORMATTER | UP-TO-DATE | `phg format -` round-trips `internal` on class, field, const, static, promoted param, and method (all preserved). `internal` is also in `KEYWORDS` (`src/lsp/keywords.rs:37`, shared list). Only reformat delta on the hand-written fixture was a canonical blank line after `package Main;` — `internal` untouched. |
| LSP | UP-TO-DATE | `internal` is a completable keyword (`src/lsp/keywords.rs:37`). Member-visibility diagnostics use the exact `phg check` pipeline (`diagnostics_for_uri` → `load_with_buffer` → `front_end_diagnostics`, `src/lsp/mod.rs:471-493`), so check ≡ LSP is architecturally guaranteed. Verified: `phg check` on a `Main`-package call of an `internal` method fires `E-METHOD-VISIBILITY` with the subtree hint "accessible only inside `Acme\Lib\Widget`'s package and its sub-packages". |

---

## Feature 3 — Top-level `internal` = package subtree + package hierarchy (DV-1/2)

Loader/checker-only concept, PHP-erased.

| Surface | Status | Evidence |
|---|---|---|
| TRANSPILE | N-A | `internal class Hidden` → `final class Hidden` (the `internal` subtree modifier erased; no leak). PHP namespaces are emitted from package paths regardless of the subtree rule; no transpile impact. Confirmed no `internal` in transpiled output. |
| LIFT | N-A | The lifter always emits a single `package Main;` and never produces package hierarchies or top-level `internal`. Correct. |
| LSP | UP-TO-DATE | The DV-1/2 subtree visibility rule is enforced by the checker; the LSP shares the exact loader + front-end pipeline (DEC-252/DEC-282, `src/lsp/mod.rs:464-493`), so the new subtree rule is reflected identically in editor diagnostics. Verified via the `E-METHOD-VISIBILITY` reproduction above whose hint states the sub-package rule. |

---

## Observations (not gaps)

1. **Formatter desugars group `import X.{A,B}` into per-line imports.** The group form is parsed
   directly into separate `Item::Import` nodes (no group node in the AST — `src/ast/decls.rs:455-470`),
   so the formatter cannot reconstruct the `{...}` syntax and emits one line per member. This is a
   PRE-EXISTING behavior (group imports predate tonight — DEC-186), and the construct actually shipped
   tonight (the `*` wildcard, stored as an AST flag) DOES round-trip. Flagged only for awareness; if
   preserving group syntax through the formatter is ever desired, the AST would need a group-import
   representation. No action required for surface currency.

## Commands run (reproducibility)

```
export PATH=/stack/tools/cargo/bin:$PATH && cargo build --release && source scripts/toolchain.env
phg transpile examples/project/wildcard-imports/src/main.phg
phg transpile examples/project/member-internal/src/main.phg
phg check   examples/project/{wildcard-imports,member-internal}/src/main.phg
phg format - < <internal-all-forms fixture>          # round-trip check
phg lift    <use Foo\{A,B}; fixture>                 # → lex error (Tier-1, expected)
phg lift    <plain PHP fixture>                      # → per-symbol synthesized imports
phg run / phg run --tree-walker <fixtures>           # byte-identity spot checks
```

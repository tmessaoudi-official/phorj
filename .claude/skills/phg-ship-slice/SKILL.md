---
name: phg-ship-slice
description: Use before marking a phorj slice/row done or committing a language/stdlib feature. Walks the per-slice definition of done — example + README row, phg format idempotency, run ≡ --tree-walker ≡ PHP, transpile AND lift, LSP + both editors + grammar, perf WIN-OR-FLAG vs PHP, fresh release binary, sabotage, status row — each tied to its CLAUDE.md invariant.
user-invocable: true
args: "[<row or feature name>]"
---

## --help

> If ARGUMENTS contains `--help`: output the text below verbatim, then immediately STOP — do not execute any other steps. (`--help` takes precedence over all other flags.)
>
> ```
> /phg-ship-slice — the per-slice definition of done, runnable, before a row is marked done.
>
> Usage: /phg-ship-slice [<row or feature name>]
> ```

---

The RULES are the delivery invariants in `CLAUDE.md` (§ "Delivery invariants") and stay there — they
must be in context while a slice is being DESIGNED. This skill is the checklist you run when the slice
is being CLOSED, because the invariants were each missed at close time at least once (the editors row
for four syntax slices, formatter idempotency twice in one session, the release binary, the example).
Each step names its invariant by number and title; if CLAUDE.md and this skill ever disagree,
CLAUDE.md wins and this skill is stale — fix it.

Load `/phg-lenses` first if a review round follows; this skill does not replace it.

## Checklist (report each as ✓ with evidence, or N/A with the reason)

1. **Example ships** — invariant 9 "Examples ship with features": a runnable `.phg` under `examples/`
   (auto-gated by `tests/differential.rs`, which globs `examples/**/*.phg`) AND a row in the
   `examples/README.md` Index table. CLI/tooling features: a walkthrough README + a small companion
   `.phg`. A fault cannot be a runnable example — document it in a README.
2. **Formatter idempotency** — `phg format --check <new .phg files>` clean; the repo-wide test is
   `tests/format.rs` `every_repo_phg_formats_idempotently_and_safely`, which fails on any `.phg` that
   `phg format` would change.
3. **Byte-identity spine** — invariant 1: `phg run` ≡ `phg run --tree-walker` ≡ the transpiled PHP
   under a real `php` — same stdout AND same failure behaviour. There is NO `runvm` command (the VM is
   `run`'s default engine). Run the differential for the new example; the full correctness gate
   before "done". The only exceptions are the ones invariant 1 lists; a new one goes through the
   ladder (invariant 14) — surfaced to the developer, never decided alone.
4. **Transpile AND lift in the same change** — invariant 17 "Always-current surfaces": `phg transpile`
   and `phg lift` both handle the feature; a feature that runs but does not transpile or lift is not done.
5. **LSP + both editors + grammar** — invariant 17 "THE 100% RULE": `src/lsp/` surfaces the feature
   everywhere it can appear (completion, hover, go-to-definition, find-usages, document symbols,
   diagnostics with the right tags, signature help); `editors/vscode/` and `editors/phpstorm/`
   (LSP4IJ) updated in the SAME change; new syntax → `editors/vscode/syntaxes/phorj.tmLanguage.json`
   too. `phg check` ≡ LSP diagnostics (same pipeline).
6. **Exhaustiveness** — invariant 3: a new `Op` extends `vm::exec_op`, `BytecodeProgram::validate` and
   `compiler::stack_effect` in the same commit, no `_` arm; a new `Expr`/`Stmt`/`Pattern`/`Item` form
   extends every total walk (leaf sets in `src/ast/leaves.rs`), no catch-all, named or not.
7. **VM compile paths** — invariant 6: anything compiled for the VM goes through
   `check_and_expand_reified` + `compile_with`. Invariants 7 (CTy operand: a differential case shaped
   `expr + 1`) and 8 (scratch slots: TWO such constructs in one expression) when the slice touches them.
8. **Perf, WIN-OR-FLAG** — invariants 11 and 18: anything with a PHP equivalent gets
   `phg benchmark --vs-php <file>` (needs `php` on PATH; output-identity-gated). A win is recorded; a
   loss is FLAGGED and fixed, never suppressed; an unmeasurable bench is an OWED verdict, never
   "passed", never re-baselined via `--emit`. No perf claim above [Inferred] without a measured
   before/after.
9. **Fresh release binary** — `cargo build --release` in THIS repo (there is no `/stack/projects/phorge`;
   older notes that cd there are stale), then tell the developer the path: `target/release/phg`. A
   full-tree build is heavy on this machine — see `/long-run`.
10. **Sabotage** — `/sabotage-check` on the slice's guarantee (the differential case, the LSP test),
    through the same harness as the baseline; for cargo, mind the mtime trap in that skill's stack notes.
11. **Status row** — the plan's `## Status` row gets `done` + the evidence sha (by pathspec commit;
    never `--amend` a commit whose sha is already recorded as evidence).

## Recipes (reference — re-read the code before following; a stale recipe is worse than none)

Recipes for adding a native module, a collection native, or an `Op` live in the project memories
`native-stdlib-wave`, `stdlib-collection-breadth` and `op-variant-match-coupling`; confirm the paths
they cite against `src/native/` and `src/vm/` before reuse.

$ARGUMENTS

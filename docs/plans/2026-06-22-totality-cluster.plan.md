# Totality Cluster — Implementation Plan

> Design SSOT: `docs/specs/2026-06-22-totality-cluster-design.md`. Front-end-only; no new `Op`/`Value`.
> Pace: **fully autonomous** (developer-authorized, all 4 sub-features). TDD throughout.

## Decisions Log
- [2026-06-22] AGREED: build the totality cluster next (before method overloading) — locked in the
  roadmap-completeness audit. All four sub-features in one slice.
- [2026-06-22] AGREED: pace = fully autonomous, all 4 sub-features.
- [2026-06-22] DESIGN: one structural `terminates` engine drives return-on-all-paths + `W-UNREACHABLE`;
  `never` is the bottom type (erased to a PHP return hint); two lints ride the warning channel.
  Soundness-conservative: `terminates` claims divergence only for provable shapes.

## Formal Plan (TDD, ordered)

**Task 1 — `Ty::Never` core (types.rs).** Failing test: `assignable(Never, Int)` true, `assignable(Int,
Never)` false, `Never` Display `"never"`. Add variant + `assignable_with` arms (Never-from before Null
arms) + Display. Gate.

**Task 2 — `never` resolution + reservation (checker.rs).** Failing test: `function f() -> never { while
(true) {} }` checks clean; a user `class never` / `enum never` is rejected. Add `resolve_type` arm +
`is_builtin_type_name`. (Return-totality for never lands in Task 4.)

**Task 3 — termination engine (checker.rs).** Failing tests on `block_terminates`/`stmt_terminates` via
the public check surface (Task 4 exercises them). Implement `stmt_terminates`, `block_terminates`,
`breaks_this_loop`, `is_true_lit`, `expr_is_never`, `stmt_span`. Pure `&self`. Gate (no behavior yet).

**Task 4 — `E-MISSING-RETURN` + `E-NEVER-RETURN` (checker.rs).** Failing tests: `-> int` fn falling off
the end errors; `if/else` both-return passes; `if`-no-else falls through → error; `while(true){}` tail
passes; `-> never` that can return → `E-NEVER-RETURN`; `-> never` infinite loop passes; `unit`/no-annot
fn exempt; interface signature exempt. Wire the post-body gate in `check_function`. Run **full gate** —
this is the Phase-0 latent-bug scan; triage every example/inline-test failure (add returns / fix engine).

**Task 5 — `check_body` + `W-UNREACHABLE` (checker.rs).** Failing test: a statement after `return`
warns once with code `W-UNREACHABLE`; clean code has zero warnings; dead region warns once not N times.
Extract `check_body`; route free-fn/ctor/set-hook bodies through it; `check_block = scope + check_body`.
Gate (warnings are non-fatal; differential unaffected).

**Task 6 — `W-MATCH-UNREACHABLE` (checker.rs).** Failing tests: arm after `_` warns; duplicate `Int`
literal arm warns; duplicate variant arm warns; an exhaustive non-redundant match has no warning. Extend
`check_match`'s arm loop. Gate.

**Task 7 — `emit_type` PHP `never` + explain codes (transpile.rs, cli.rs).** Failing test: transpiling a
`-> never` fn emits `: never`; `explain_text` returns a paragraph for each of the 4 codes. Gate.

**Task 8 — example + docs.** `examples/guide/totality.phg` + `examples/README.md` entry (byte-identity
glob auto-gates it); KNOWN_ISSUES deferrals; CHANGELOG/MILESTONES/CLAUDE.md note. Full gate + clippy +
fmt. **Rebuild release binary** (`cargo build --release`) and report its path.

## Acceptance
`PHORGE_REQUIRE_PHP=1 cargo test` green · `cargo clippy --all-targets -- -D warnings` clean ·
`cargo fmt --check` clean · `examples/guide/totality.phg` byte-identical run/runvm/real PHP ·
all 4 diagnostic codes covered by `phg explain`.

**STATUS: COMPLETE** — all 8 tasks landed; full gate green (600 lib + PHP-oracle differential +
integration), clippy + fmt clean; `examples/guide/totality.phg` byte-identical run/runvm/real PHP;
release binary rebuilt. Phase-0 scan clean (no latent typed-fn-falloff in any example/inline test).

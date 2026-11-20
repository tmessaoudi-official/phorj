# TRY-UNWRAP-OPERAND: a `?`-unwrapped `var` is an arithmetic operand on the VM (2026-11-20) Plan

> Defect-queue entry TRY-UNWRAP-OPERAND (P0 proposed 2026-11-19, ratification owed; SLICE-STATE § DEFECT QUEUE),
> taken next in the away window (autonomous). Reproduced on the release binary at `423c927e`:
> `var h = half(n)?; int y = h + 1;` inside a function returning `Result<int, string>` type-checks, then fails on
> `phg run` and `phg run --no-jit` with `` `h` is not numeric ``; the tree-walker and PHP print `ok 106 odd`. Same for
> a float payload (`f * 2.0`), a string payload (`s + "!"`) and two `?` in one function. Root cause [Verified: read
> the code]: a `var` local is typed by `ctype(init)` (`src/compiler/stmt/core.rs`, `Stmt::VarDecl`), and `ctype`'s
> `Expr::Propagate` arm returns `CTy::Other` (`src/compiler/cty.rs`); the checker computes the unwrapped `Success`
> payload in `check_var_decl` (`src/checker/stmt/var_decl.rs`) but never hands it over, because its operand
> recording runs in `check_expr` for `Call`/`Member`/`Index` nodes and the let-initializer `?` never goes through it.

## Decisions Log
- [2026-11-20 10:56] ASSUMED (review): plan gate (autonomous, away window): the checker records the unwrapped type of a Result-mode `?` let-initializer in a dedicated `Reified::propagates` map keyed by the `?` node's span (its own map, not `operands`, because `desugar_db` synthesizes `?` nodes with a shared `self.sp()` span that may equal an inner call's; a span recorded twice with different types becomes unknown, the `pattern_binds` rule); the compiler's `ctype` fallback reads it only where its own answer is `Other`. Basis: advisor() 3C round 1.

## Formal Plan

1. **Failing test first** (`tests/differential.rs`, pinned output, three legs): int, float and string payloads; two
   `?` in one function; a non-call operand (`var h = r?;` on a parameter); a class payload (`p.x + 1`); a list
   payload (`xs[0] + 1`); a `?` in a method body and in a lambda body; a bare-`T` generic payload left as today; the
   forms that already work (typed `int x = f()?;`, throws-mode `var v = g()?; v * 2`).
2. **Checker**: one recording site after the `match` in `check_var_decl`, for the two Result-mode arms only
   (throws mode is erased before any backend); reifiable types only; disagreement on one span → `Ty::Error`.
3. **Compiler**: `ReifiedCty` carries the map (so `is_empty` must cover it: it guards the fallback); `ctype`'s
   fallback keys `Expr::Propagate` by its span into it. `cty.rs`'s `Propagate` comment updated.
4. **Sabotage, example, SSOT, gate**: one mutant per guard; a `var … ?` line in `examples/guide/result.phg`;
   KNOWN_ISSUES bullet, queue row out, CHANGELOG; full all-features gate; release binary rebuilt and re-probed.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing three-leg differential test | S | done | `try_unwrapped_var_is_an_arithmetic_operand` red on the VM only (`` `h` is not numeric ``); the tree-walker printed the pinned 9 lines | tests/differential.rs |
| 2 | Checker records the unwrapped type | S | done | `Reified::propagates` + `record_propagate` (span len 0 skipped, disagreement → unknown); one site after the Result-mode arms of `check_var_decl` | src/checker/resolutions.rs src/checker/stmt/var_decl.rs |
| 3 | Compiler consumes it | S | done | `ReifiedCty::propagate`; `is_empty` covers both tables; `ctype` fallback keys `Expr::Propagate`; targeted run 27/27 green | src/compiler/reified.rs src/compiler/cty.rs |
| 4 | Sabotage, example, SSOT, gate | S | doing | four mutants, each compiling and red on its own assertion (no record and no `ctype` key: `` `h` is not numeric ``; no disagreement rule and `is_empty` on operands only: their unit tests), restored 31/31; `examples/guide/result.phg` `bumped`: old release binary `` `x` is not numeric ``, fixed build `ok 41` | examples/guide/result.phg examples/README.md KNOWN_ISSUES.md CHANGELOG.md docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- Perf OWED (Invariant 11): a program that already ran with a `var x = f()?` used outside arithmetic (interpolation,
  comparison) may now get typed ops instead of generic ones [Inferred: same mechanism as GENERIC-PAYLOAD-OPERAND];
  no before/after measured.

# R9-K1 match-arm bindings in the compiler's CTy resolver (2026-11-17) Plan

> Defect-queue entry R9-K1 (P0, ratified 2026-11-17; SLICE-STATE § DEFECT QUEUE): a `match` whose first non-throwing
> arm body names a pattern binding cannot be an arithmetic operand on the VM (`compile error: undefined variable`);
> `phg check`, the tree-walker and PHP accept it (Invariant 7). Root cause, read in the code: `ctype`'s `Expr::Match`
> arm (`src/compiler/cty.rs`) types the arm body without the arm's bindings, which `register_bindings`
> (`src/compiler/matches.rs`, `&mut self`) pushes only while the arm is compiled. Second symptom, same cause:
> `var x = match … { A(v) => v, … }; x + 1` fails with `` `x` is not numeric ``. Chosen next by the developer
> (R9-K2 plan, close-out). Forge C-5 (the compiler re-derives types instead of consuming the checker's; DEC-562
> stage 2) is the architectural root and stays open: this is the local fix.
>
> **STATE (2026-11-17, session stopped by the developer before the plan gate):** Phases 0-3C done, this plan is a
> DRAFT awaiting the developer's GO. No code written. Probes on the 2026-11-17 release binary (VM vs tree-walker / PHP):
> sleuth c1, c3, u3, u7 fail `undefined variable` on the VM; `var x = match …; x + 1` fails `` `x` is not numeric ``;
> generic `B<int>` `Holds(x) => x` + 1 fails `undefined variable x`; `1 + match …` and `Wrap(p) => p.x` + 1 already
> agree; an outer local shadowed by a binding is rejected by the checker (redeclaration), so that case is dropped.
> Reproducers: `var/claude/sleuth/2026-11-16/scratch-K/{c1,c3,u3,u7}.phg` (gitignored).

## Decisions Log

## Formal Plan
1. Failing test first (`tests/differential.rs`, `agree_out_php`, three legs plus the expected output from the
   tree-walker and PHP), confirmed red on the VM for the stated reason. Cases: two binding arms `+ 1` (the sleuth
   c1); a function return `* 2`; a `when` guard arm; a float payload `+ 0.5`; `var x = match …; x + 1`; `1 + match`;
   two binding matches in one expression (Invariant 8); a throwing first arm (DEC-532) then a binding arm; a nested
   match in an arm body naming the outer binding; a struct pattern `P { x } => x`; `Wrap(p) => p.x`; a hot function
   (the JIT compiles it, compared with `--no-jit`); a LATER function whose `float v` local is an operand (catches a
   leaked binding).
2. Fix: split `register_bindings` into a pure `&self` collector (one walker of `Pattern`, used by both typing and
   compiling) plus the push; add a `RefCell` binding overlay to `Compiler` (the `narrow_overlay` pattern); one lookup
   helper that sees the overlay then `match_bindings`, used by the `Ident` typing and `resolve_binding`; `ctype`'s
   `Match` arm collects the chosen arm's bindings (scrutinee typed as `compile_match` does), pushes them, types the
   body, and truncates unconditionally. `cty.rs` stays at or under 500 lines (it is at 494): the logic lives in
   `matches.rs`.
3. Boundary probe, recorded not claimed: a generic payload (`B<int>`, `Holds(x) => x + 1`) after the fix; an erased
   payload still typed `Other` stays the KNOWN_ISSUES generic-enum limitation.
4. Sabotage: drop the overlay push (red); skip the truncate (red via the later function).
5. Example line in `examples/guide/pattern-matching.phg` and its README row.
6. JIT explain on the hot case; full all-features gate; release binary; SLICE-STATE (R9-K1 FIXED); CHANGELOG;
   size gate after `cargo fmt`.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing three-leg differential test, red on the VM for the stated reason | S | todo | - | tests/differential.rs |
| 2 | Binding overlay and pure collector; `ctype` types the arm under its bindings | S | todo | - | src/compiler/matches.rs src/compiler/cty.rs src/compiler/cty_members.rs src/compiler/mod.rs src/compiler/emit.rs |
| 3 | Generic-payload boundary probe | S | todo | - | - |
| 4 | Sabotage the overlay push and the truncate | S | todo | - | src/compiler/matches.rs |
| 5 | Guide example line and README row | S | todo | - | examples/guide/pattern-matching.phg examples/README.md |
| 6 | JIT explain, full gate, release binary, SLICE-STATE, CHANGELOG, size gate | S | todo | - | docs/plans/SLICE-STATE.md CHANGELOG.md |
<!-- /progress-block -->
### Blocked
### Needs input
- The plan gate: GO on the approach above (step 2), or redirect.
### Needs research
### Fragile
- `src/compiler/cty.rs` is at 494 lines: any growth past 500 breaches the hard cap (Invariant 13).
### Known issues

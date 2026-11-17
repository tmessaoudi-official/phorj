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
> **STATE (2026-11-17, evening):** BUILT in autonomous mode (plan gate taken as ASSUMED, Decisions Log); all six steps
> done. Drafting-session notes follow. Probes on the 2026-11-17 release binary (VM vs tree-walker / PHP):
> sleuth c1, c3, u3, u7 fail `undefined variable` on the VM; `var x = match …; x + 1` fails `` `x` is not numeric ``;
> generic `B<int>` `Holds(x) => x` + 1 fails `undefined variable x`; `1 + match …` and `Wrap(p) => p.x` + 1 already
> agree; an outer local shadowed by a binding is rejected by the checker (redeclaration), so that case is dropped.
> Reproducers: `var/claude/sleuth/2026-11-16/scratch-K/{c1,c3,u3,u7}.phg` (gitignored).

## Decisions Log
- [2026-11-17 21:12] ASSUMED (review): plan gate taken in autonomous mode (away window 2026-11-17, keep-going on): build the approach of Formal Plan step 2 as drafted (pure collector + RefCell binding overlay, logic in matches.rs) — because it is an internal compiler fix with no user-visible surface (not Invariant 15) and the root cause was probed in the drafting session. Alternatives: wait for the developer's GO; consume the checker's types (Forge C-5 / DEC-562 stage 2, architectural, out of scope)
- [2026-11-17 21:57] ASSUMED (review): class-compatibility guard added beyond the drafted plan: arms of incompatible types (two classes) type as Other instead of the first arm's class — because advisor 3C flagged the regression surface and a probe showed the VM building a Kid for y with { x = 9 } on a Par (tree-walker par 9, VM kid 9); field and method reads of such a value probed fine on three legs. Alternatives: first-arm typing as before (keeps the divergence); type as the common ancestor (not needed once Other kept fields)
- [2026-11-17 21:57] ASSUMED (review): binding logic split out of matches.rs into src/compiler/match_bindings.rs — because matches.rs would pass the 300-line soft cap and Invariant 13 makes split-as-you-go the default. Alternatives: grow matches.rs to ~320 lines
- [2026-11-17 21:57] ASSUMED (review): IF-CLASS-ARM-TYPE recorded in the defect queue at P0 (proposed, ratification owed) — because the ratified rubric makes P0 a default-engine wrong output on a checked program (VM true 9, tree-walker and PHP false 9). Alternatives: P1

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
| 1 | Failing three-leg differential test, red on the VM for the stated reason | S | done | `match_binding_is_an_arithmetic_operand` red before the fix: tree-walker printed the expected 13 lines, VM `compile error: undefined variable` `v`; advisor 3C added a bare-binding arm, a type pattern and the class case (pre-fix probe: tree-walker `par 9`, VM `kid 9`) | tests/differential.rs |
| 2 | Binding overlay and pure collector; `ctype` types the arm under its bindings | S | done | test green on three legs (`PHORJ_REQUIRE_PHP=1`, 1 passed); type-only `(name, CTy)` overlay, every non-throwing arm typed, incompatible arms give `Other`; binding logic split to `match_bindings.rs` (matches.rs 167, cty.rs 491, mod.rs 496 lines) | src/compiler/match_bindings.rs src/compiler/matches.rs src/compiler/cty.rs src/compiler/mod.rs src/compiler/emit.rs |
| 3 | Generic-payload boundary probe | S | done | recorded, not claimed: `B<int>` `Holds(x) => x` + 1 still fails on the VM, now `cannot infer numeric type of Match {…}` (was `undefined variable x`): the erased payload types `Other`, the KNOWN_ISSUES S2.1 generic-result limitation | - |
| 4 | Sabotage the overlay push and the truncate | S | done | three mutants of `match_bindings.rs`, each landed, red, restored by `cmp`: no push → VM `undefined variable` `v`; no truncate → VM `runtime error at 64: expected int, found float` (the later `float v` line); no class guard → VM `with kid 9`; final baseline 1 passed | src/compiler/match_bindings.rs |
| 5 | Guide example line and README row | S | done | `scaledSize` line: pre-fix VM `compile error: undefined variable` `r`, tree-walker `scaled = 3.75` | examples/guide/pattern-matching.phg examples/README.md |
| 6 | JIT explain, full gate, release binary, SLICE-STATE, CHANGELOG, size gate | S | done | full all-features gate green 2026-11-17 21:47: nextest 3767 passed 2 skipped, clippy all-features and no-default-features, release build, size gate `grandfathered=54 fails=0 warns=176`, doc-guards; `PHORJ_JIT_EXPLAIN=1` declines `hot` on `unboxed GetEnumField(1)` (unrelated); `phg format` idempotent on the guide | docs/plans/SLICE-STATE.md CHANGELOG.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
- `src/compiler/cty.rs` is at 494 lines: any growth past 500 breaches the hard cap (Invariant 13).
### Known issues
- Microbench G-8 ratchet verdict OWED for this fix (pre-push 2026-11-17: 1-min load 8.86 > 2.5, skipped; `perf-gate.sh` still ran), as for R9-K2.
- IF-CLASS-ARM-TYPE (new queue entry, P0 proposed): the `if`-expression form of the class divergence, found by probe while fixing; not in this fix.
- The JIT declines `hot` (`unboxed GetEnumField(1)`), so the test's loop is not JIT coverage; nothing in this fix touches `src/jit/`.

# GENERIC-PAYLOAD-OPERAND: a binding of an erased generic payload is an arithmetic operand on the VM (2026-11-19) Plan

> Defect-queue entry GENERIC-PAYLOAD-OPERAND (P0, ratified 2026-11-18; SLICE-STATE § DEFECT QUEUE), taken next after
> IF-CLASS-ARM-TYPE in the away window (autonomous; INIT-READ-BEFORE-SET needs an E-code name, an Invariant-15
> question). Reproduced on the release binary at `0ec8e18a`: every scalar payload of an erased generic fails to compile
> on `phg run` (`` `v` is not numeric ``): `Option<int>` (`v * 2`, and `match … + 1`), `Option<float>`, `Option<string>`
> (`v + "!"`), a user `enum Box<T> { Holds(T value), Empty }`, nested `Option<Option<int>>`, `Result<int, E>`; the
> tree-walker and PHP print `opt 10 6|flt 3|str hi!|box 8|nest 8|cls 12|res 12`. A class payload (`p.x * 4`) already
> works. Root cause, read in the code: the compiler's `collect_bindings` (`src/compiler/match_bindings.rs`) types a
> variant field from the enum's `field_tags`, where `T` is erased to `Other`; the checker knows the substituted type
> (`check_pattern`, `src/checker/matches.rs`, declares each binding with it) but never hands it over.

## Decisions Log
- [2026-11-19 15:12] ASSUMED (review): plan gate (autonomous, away window): checker records each match binding's substituted type in a dedicated span+name map inside the reified value; the compiler uses it only where its own type is Other. No ruling needed: the VM is brought to the oracle (Invariant 2). Alternatives: merge into reified_operands (collides: the ufcs synthesized binding reuses a Call span); materialize a type into Pattern::Binding (touches every Pattern constructor).
- [2026-11-19 17:05] ASSUMED (review): close-out: keep the plan file and write the ledger, as the developer ruled for the INIT-CHAIN plan ('Keep + ledger'); 6C closed at round 2 (advisor: round-1 items closed, phg build verified).

## Formal Plan

1. **Failing test first** (`tests/differential.rs`, pinned output on three legs): the seven shapes above, plus a guard
   using the binding, a binding captured by a lambda in the arm, and the IF-CLASS-ARM-TYPE `om`/`oi` lines kept green.
2. **Checker**: `check_pattern` records each binding's resolved type (operand types only, a type parameter excluded) in
   a dedicated map keyed by the binding's span, with its name; a binding recorded twice with different types becomes
   unknown. The map rides inside the existing reified value (`Reified { operands, pattern_binds }`), so every VM compile
   path that threads the operand table threads it too (Invariant 6). Spans are unique: preludes are re-based above
   `INJECTED_SPAN_BASE` and files get per-file windows (`src/token.rs`); the ufcs synthesized binding reuses a `Call`'s
   span, which is why it is not merged into the operand table.
3. **Compiler**: `collect_bindings` takes the checker's type for a binding whose own type is `Other` and whose name
   matches. Grandfathered files (`checker/mod.rs`, `checker/matches.rs`) and `compiler/mod.rs` (497) keep their line
   counts.
4. **Checks before trusting**: does any binding span get checked under two type environments (instrumented run of the
   differential binary and examples); does a class payload binding behave as a local declared with that type (`Option<Par>`
   holding a `Kid`, interface payload: `with`, hook read and write, method call); which VM paths carry the table.
5. **Sabotage, example, SSOT, gate**: one mutant per guard; example line in `examples/guide/generic-enums.phg`; queue
   row out; CHANGELOG; full gate; perf.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing three-leg differential test | S | done | red on the VM only (`` `v` is not numeric ``), tree-walker prints the expected 9 lines; the lambda shape moved out (fails for any payload, queued LAMBDA-CAPTURE-MATCH-BINDING) | tests/differential.rs |
| 2 | Checker records binding types | S | done | `Reified { operands, pattern_binds }`; `declare_pattern_bind` at the one `check_pattern` binding site; grandfathered files kept flat | src/checker/resolutions.rs src/checker/matches.rs src/checker/mod.rs src/checker/plumbing.rs src/checker/expr/core.rs src/cli/pipeline.rs src/serve/handlers.rs |
| 3 | Compiler consumes them | S | done | `ReifiedCty` (new `src/compiler/reified.rs`) replaces the bare map on every compile path; `collect_bindings` uses it where its own type is unknown; both new tests green | src/compiler/reified.rs src/compiler/match_bindings.rs src/compiler/mod.rs src/compiler/program.rs src/compiler/emit.rs src/compiler/ctors.rs |
| 4 | Double-record, class-payload and VM-path checks | S | done | instrumented workspace run: 3774 passed, 0 double records; class/interface payload probe identical on VM, `--no-jit`, tree-walker, PHP (pinned as a test); `run`, `serve`, `benchmark`, `disassemble`, playground thread the table (all ten `compile_with` callers pass the checker's table), `phg build` binary of `guide/generic-enums.phg` prints `doubled 15` (6C r2), `phg test` is tree-walker only, plain `compile` only in test helpers | - |
| 5 | Sabotage, example, SSOT, gate | S | done | full all-features gate green (nextest 3774/3774, clippy x2 0, fmt, size-gate 54/0, release); no record: red (`v` is not numeric); no fallback: red; no name check and the disagreement rule had no end-to-end trigger (0 re-records, counted on stderr of a `--no-capture` workspace run), so 6C round 1 added unit tests (`checker::resolutions::pattern_bind_tests`, `compiler::reified::tests`): the name-check, disagreement, type-parameter and zero-length mutants each redden exactly their own test, restored byte-exact; example `doubled 15` on three legs | examples/guide/generic-enums.phg examples/README.md docs/plans/SLICE-STATE.md CHANGELOG.md KNOWN_ISSUES.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- Found while probing, not this defect: a nested `Option.Some(Option.Some(v))` / `Option.Some(Option.None())` / `Option.None()` arm set is rejected as non-exhaustive (`missing Some`); a struct pattern on `Cell<int>` types its field as an unsubstituted `T` (`found T and int`). Both already documented (KNOWN_ISSUES § struct destructuring deferred corners (2) and (3)); not re-queued.
- Perf OWED (Invariant 11): no before/after measured. Programs the VM used to reject now compile; a program that already ran (a generic payload binding used in interpolation or a comparison) may now get typed ops instead of generic ones [Inferred: `collect_bindings` change], output identical on the corpus. No speed claim is made.
- Queued while fixing: LAMBDA-CAPTURE-MATCH-BINDING (a lambda in an arm cannot capture the arm's binding on the VM, any payload) and TRY-UNWRAP-OPERAND (the KNOWN_ISSUES `?`-unwrap limitation, a VM-only rejection), both P0 proposed.

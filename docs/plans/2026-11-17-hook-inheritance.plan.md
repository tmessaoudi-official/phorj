# HOOK-INHERITANCE: a subclass inherits its parent's property hook (2026-11-17) Plan

> Defect-queue entry HOOK-INHERITANCE (P0, ratified 2026-11-17); audit plan row 4e
> (`docs/plans/2026-10-08-audit-remediation.plan.md`); DEC-572 RULED (a) 2026-10-26: property hooks are inherited
> like methods on both native legs, matching PHP; the silent write drop becomes a setter call.
>
> Reproduced 2026-11-17 on the release binary (PHP 8.5 oracle, so KNOWN_ISSUES' [Unverified] PHP column is now
> verified): `class Puppy extends Dog {}` where `Dog` declares `string name { get => "hooked"; }` over `Animal`'s
> stored `name` prints `p` natively and `hooked` in PHP; a hook-only `Dog.label` read through a `Puppy` faults
> ``no field `label` on `Puppy` `` natively and prints `L` in PHP; `d.label = 5` with `set(int v) { this.raw = v * 10; }`
> faults natively and gives `50 50` in PHP.
>
> Root cause, read in the code: the oracle's `hook_get` / `hook_set` (`src/interpreter/construct.rs`) look at the
> class's own members and its `use`d traits only; the compiler registers `<name>$get` / `<name>$set` per declaring
> class (`src/compiler/program.rs`), and its inherited-method aliasing reads `ast::class_method_origins`, which lists
> `ClassMember::Method` names only.

## Decisions Log
- [2026-11-17 22:35] ASSUMED (review): plan gate taken in autonomous mode (away window, keep-going on): build DEC-572 (a) through a sibling resolver class_hook_origins on the class_method_origins machinery, not by adding $get names to class_method_origins itself — because that table also feeds transpile/classes_synth.rs and trait_clauses.rs, where a label$get entry would surface as a PHP method name. Alternatives: add hook names to class_method_origins (DEC-572's literal wording) with transpiler filters; walk extends in the interpreter only (two resolvers, diverges on multi-parent)
- [2026-11-17 22:56] ASSUMED (review): a subclass that STORES a field named like its parent's hook keeps today's native behaviour (the field wins, `7 7`; PHP prints `4 4`) — because DEC-572 does not cover the shape and Invariant 15 forbids ruling it autonomously; recorded PENDING as DEC-582 (recommended (c): reject the shape) and queue entry HOOK-FIELD-SHADOW (P1 proposed). Alternatives: let the inherited hook win (PHP), reject now
- [2026-11-17 22:56] ASSUMED (review): src/interpreter/coop.rs goes 300 -> 301 lines (one struct-literal field, hook_origins); not split — because the three Interp constructors (mod.rs, entry.rs, coop.rs) each list every field and a split for one line would not be by cohesion; the size gate's warn count is unchanged (176) since class_hierarchy.rs fell to 268. Alternatives: fold hook origins into method_origins (allocates a key on every tree-walker field read), split coop.rs
- [2026-11-17 22:56] ASSUMED (review): renamed the pinning test vm_hook_on_an_extended_class_is_decided_by_the_runtime_class to hooks_are_inherited_like_methods and updated the KNOWN_ISSUES cite — because it no longer pins the runtime-class-only behaviour. Alternative: keep the old name
- [2026-11-17 23:33] ASSUMED (review): a hooked property inherited from two parents (hook + hook, hook + stored field) is reported as E-MI-FIELD-CONFLICT with property wording — basis: DEC-572 'inherited like methods' + the existing analogues (E-MI-CONFLICT for methods, E-MI-FIELD-CONFLICT for fields) + PHP fatals on both shapes; reuses the existing code, so no new diagnostic surface. Before: both native legs ran the sorted-first parent's hook while PHP fataled. Alternatives: a new E-MI-HOOK-CONFLICT code; record PENDING and leave the split
- [2026-11-17 23:59] ASSUMED (review): WITHDRAWN the 23:33 pick (E-MI-FIELD-CONFLICT for a hooked property inherited from two parents): a new compile error on a program that ran natively before is a user-visible language decision, which phg-ask-human's Invariant-15 carve-out says an autonomous session records PENDING and never builds. Also its 'before' claim was wrong: pre-DEC-572 the hook+field shape read B's field natively (4 4, inferred), only hook+hook faulted. Recorded as DEC-584 PENDING (recommended (a) E-MI-FIELD-CONFLICT) + queue HOOK-MI-COLLISION; native behaviour pinned in hook_from_two_parents_is_pending_dec_584. The checker code and explain text are back to `5a8c4d40` (`fix: a subclass inherits its parent's property hooks`)
- [2026-11-18 00:25] ASSUMED (review): pending DEC-584, a hooked property with two distinct declarations among the parents is NOT inherited (hook + hook faults no field, hook + field reads the field: the pre-DEC-572 behaviour on both orders), instead of the resolver's name-ordered pick — basis: advisor 6C round 3 agreed; it adds no new behaviour to an unruled shape and mirrors the DEC-582 treatment; a diamond or a trait both parents use stays one declaration and is inherited. Two mutants red. Alternatives: keep the name-ordered pick (new native behaviour, unruled), reject (withdrawn, Invariant 15)
- [2026-11-18 01:03] ASSUMED (review): DEC-584 interim widened after advisor 6C round 4 probes: a clash is judged per property (two direct parents bringing different declaration sets, any accessor), leaves a slot marker so subclasses keep the pre-fix behaviour, and a class's own accessor is never tagged with its trait — basis: the round-4 probes showed new native behaviour on three unruled shapes (9 9 below a clash, a get/set split, own+trait); all now give the pre-fix result; three mutants red. Alternative: pin the shapes PENDING without building

## Formal Plan
1. Failing test first: rewrite `vm_hook_on_an_extended_class_is_decided_by_the_runtime_class` (it pins the pre-ruling
   behaviour: `p p p`, the `no field` fault, `1 50`) as `agree_out_php` with PHP's outputs, plus new cases: a child
   overriding the parent's `get` while inheriting its `set`, a three-level chain, a parent whose hook comes from a
   `use`d trait, the exact subclass (compile-time hook path) and the parent-typed receiver (runtime-class path), a
   hot loop (JIT), a class with its own unrelated hook. Confirm red for the stated reason.
2. Split `class_method_origins` out of `src/ast/class_hierarchy.rs` (467 lines) into `src/ast/member_origins.rs`
   (pure move, M-Decomp), parameterize the declared-name set, and add `class_hook_origins` (names `<name>$get` /
   `<name>$set` per `ClassMember::Hook`, traits included). `class_method_origins` and its transpiler/checker consumers
   are unchanged (a `$get` name in that table would reach `transpile/classes_synth.rs` as a PHP method name).
3. Interpreter: `hook_get` / `hook_set` fall back to `class_hook_origins` (stored once at load beside
   `method_origins`). Compiler: alias `(class, <name>$get|$set)` to the declaring class's fn index exactly like the
   inherited-method aliasing, by moving that alias loop out of `program.rs` (grandfathered at 646; it may not grow)
   into a helper called for both tables.
4. Sabotage: drop the compiler aliasing (VM red), drop the interpreter fallback (oracle red).
5. `examples/guide/property-hooks.phg` line + README row; KNOWN_ISSUES HOOK-INHERITANCE marked FIXED; register
   DEC-572 built; audit plan row 4e done; SLICE-STATE queue; CHANGELOG.
6. JIT explain on the hot case; LSP probe (Invariant 17: hover/completion of an inherited hook on the subclass);
   full all-features gate; release binary.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing three-leg test, red for the stated reason | S | done | `hooks_are_inherited_like_methods`: pre-fix oracle `p p p hooked` vs PHP `hooked hooked hooked hooked` | tests/differential.rs |
| 2 | Split the origins resolver and add `class_hook_origins` | S | done | `class_hierarchy.rs` 467 to 268, `member_origins.rs` 300 + `member_names.rs` 47 (`own_storage_fields`, `hook_accessors`, shared with `class_field_layout`) | src/ast/class_hierarchy.rs src/ast/member_origins.rs src/ast/mod.rs |
| 3 | Interpreter fallback and compiler aliasing | S | done | 12 of 12 hook tests green; `program.rs` 646 to 635 (alias loop moved to `origin_alias.rs`) | src/interpreter/construct.rs src/interpreter/engine.rs src/interpreter/mod.rs src/compiler/program.rs |
| 4 | Sabotage both legs | S | done | eight mutants red for the stated reason, each restored with `cmp`: no hook alias (VM `p p p hooked`), oracle get / set fallback, VM `$get`-only, no field shadow (`4 4`), whole-property override (`1 2`), a two-parent clash inherited again, no trait identity (shared trait faults `no field`), a flat declaration count (read-over-stored `p p p hooked`), no slot marker (`9 9`), an own accessor tagged with its trait (`1` vs `7`) | - |
| 5 | Example, README, KNOWN_ISSUES, register, audit row, SLICE-STATE, CHANGELOG | S | done | `Savings` prints `savings after -3: 0, empty: true` on three legs; DEC-572 built, DEC-582 PENDING; queue HOOK-FIELD-SHADOW | examples/guide/property-hooks.phg examples/README.md KNOWN_ISSUES.md CHANGELOG.md docs/plans/SLICE-STATE.md |
| 6 | JIT explain, LSP probe, full gate, release binary | S | done | JIT: hook reads decline as an own hook does (`CallMethod on Unknown`); LSP: completion lists inherited hooks, hover/definition null for every instance member (LSP-INSTANCE-MEMBER-HOVER, pre-existing); full all-features gate green (nextest 3768, with `hook_from_two_parents_is_pending_dec_584`); release rebuilt | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
- `src/interpreter/coop.rs` 301 lines (one field over the soft cap; ASSUMED entry); `src/interpreter/mod.rs` 463.
### Known issues
- DEC-583 PENDING: the checker types a child that redeclares ONE accessor as replacing the whole property (rejects `p.label = 3` as read-only); run time and PHP are per accessor.
- DEC-584 PENDING / queue HOOK-MI-COLLISION: a hooked property inherited from two parents is NOT inherited (native legs keep the pre-fix behaviour: `no field` / the stored field); PHP fatals. Pinned natively, two mutants red.
- OWN-TRAIT-HOOK-SPLIT (queue, P1 proposed): a class's own hook and its trait's hook of one name: oracle `1`, VM `7`, PHP fatals. Pre-existing, found by the 6C round 4 probes.
- Perf (`/phg-ship-slice` row): no bench measured; the change adds a table lookup only on a hook read that misses the class's own members. Verdict OWED (box loaded all night; G-8 skipped on the last push).
- LIFT-PROPERTY-HOOKS (KNOWN_ISSUES): `phg lift` refuses any PHP property hook; hooks are outside the lift subset.
- MI-TRANSPILE-COMPOSITION (queue, P1 proposed): a diamond's shared promoted param and a trait both parents use transpile to failing PHP; native legs agree. Pre-existing.
- A fault inside a hook body renders a different line and trace on the two native legs (oracle: no getter frame; VM: `Dog::label$get`), for an own hook too: the queued TRACE-FRAME-NAMES class.
- HOOK-FIELD-SHADOW / DEC-582 PENDING: a subclass's stored field named like an inherited hook wins natively; PHP runs the hook. Pinned on the native legs.
- The JIT declines a function that reads a hook (`CallMethod on Unknown`), inherited or own alike: pre-existing, not new.
- LSP-INSTANCE-MEMBER-HOVER (KNOWN_ISSUES, P2): hover/definition answer `null` on every user instance member; completion is fine.
- SET-HOOK-DURING-CONSTRUCTION (row 4f) is untouched: promoted assignments and `with` still bypass setters.

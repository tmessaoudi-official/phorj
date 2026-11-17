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
| 2 | Split the origins resolver and add `class_hook_origins` | S | done | `class_hierarchy.rs` 467 to 268, `member_origins.rs` 299; `own_storage_fields` shared with `class_field_layout` | src/ast/class_hierarchy.rs src/ast/member_origins.rs src/ast/mod.rs |
| 3 | Interpreter fallback and compiler aliasing | S | done | 12 of 12 hook tests green; `program.rs` 646 to 635 (alias loop moved to `origin_alias.rs`) | src/interpreter/construct.rs src/interpreter/engine.rs src/interpreter/mod.rs src/compiler/program.rs |
| 4 | Sabotage both legs | S | done | six mutants red for the stated reason, each restored with `cmp`: no hook alias (VM `p p p hooked`), oracle get / set fallback, VM `$get`-only, no field shadow (`4 4`), whole-property override (`1 2`) | - |
| 5 | Example, README, KNOWN_ISSUES, register, audit row, SLICE-STATE, CHANGELOG | S | done | `Savings` prints `savings after -3: 0, empty: true` on three legs; DEC-572 built, DEC-582 PENDING; queue HOOK-FIELD-SHADOW | examples/guide/property-hooks.phg examples/README.md KNOWN_ISSUES.md CHANGELOG.md docs/plans/SLICE-STATE.md |
| 6 | JIT explain, LSP probe, full gate, release binary | S | doing | JIT: hook reads decline as an own hook does (`CallMethod on Unknown`); LSP: completion lists inherited hooks, hover/definition null for every instance member (LSP-INSTANCE-MEMBER-HOVER, pre-existing) | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
- `src/interpreter/coop.rs` 301 lines (one field over the soft cap; ASSUMED entry); `src/interpreter/mod.rs` 463.
### Known issues
- HOOK-FIELD-SHADOW / DEC-582 PENDING: a subclass's stored field named like an inherited hook wins natively; PHP runs the hook. Pinned on the native legs.
- The JIT declines a function that reads a hook (`CallMethod on Unknown`), inherited or own alike: pre-existing, not new.
- LSP-INSTANCE-MEMBER-HOVER (KNOWN_ISSUES, P2): hover/definition answer `null` on every user instance member; completion is fine.
- SET-HOOK-DURING-CONSTRUCTION (row 4f) is untouched: promoted assignments and `with` still bypass setters.

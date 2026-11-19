# INIT-CHAIN: every class's field initializers run on construction (2026-11-18) Plan

> Defect-queue entry INIT-CHAIN (P0, ratified 2026-11-17); audit plan row 4h
> (`docs/plans/2026-10-08-audit-remediation.plan.md`); DEC-575 RULED (a) 2026-10-27, refined by DEC-585 (when the
> chain runs relative to constructor bodies) and DEC-586 (a diamond's shared base), both ruled 2026-11-18.
>
> Reproduced 2026-11-18 on the release binary (`target/release/phg`, built after the last `src` commit; PHP 8.5
> oracle via `scripts/toolchain.env`; probes in the session scratchpad, re-created in step 1):
> - `open class A { int x = 1; } class B extends A { int y = 2; }`, `new B().x` faults `no field x on B` on all three
>   legs; the same with `constructor() {}` on `B`, for a trait's initializer on a subclass of its user, and for a
>   diamond whose arms have initializers (the shared base's never run).
> - Already agreeing and kept (DEC-585/586): `parent.constructor()` runs the parent's initializers at the call
>   (`B.init A.init A.body B.body`); `open class A { int d = this.x * 2; constructor(public int x) {} }` forwarded with
>   `parent.constructor(3)` prints `3 6`; a diamond base with no arm initializers runs twice (`base base`).
> - PHP-leg only: `open class A { constructor(public int x) {} } class B extends A { int y = this.x + 1; }`, `new
>   B(5)` prints `5 6` natively and fatals in PHP (a synthesized zero-arg `__construct` overrides the inherited one).
> - All legs (KNOWN_ISSUES calls it PHP-only, wrongly): `parent.constructor(7)` into a parent whose constructor comes
>   from a trait inlines an empty block (`T.body` never runs): `checker/inline_parent_ctor.rs` `find_owner` reads only
>   an own constructor.
> - PHP-leg only, found 2026-11-18: a `private` ancestor field initialized through `parent.constructor(…)` prints `4 2`
>   natively; PHP warns "Creation of dynamic property B::$x" and fatals (the inliner's spliced `$this->x = 4` runs in
>   `B`'s scope).
>
> Root cause, read in the code: `ast::field_initializers` (`src/ast/field_inits.rs`) returns only the set of the class
> that owns the constructor plan, so a class with an initializer or constructor of its own drops every ancestor's set;
> all three backends read that one list (`compiler/ctors.rs`, `interpreter/engine.rs`, `transpile/classes_synth.rs`).

## Decisions Log
- [2026-11-18 11:25] AGREED: DEC-585 (developer, AskUserQuestion): prelude unless forwarded. A constructor with no `parent.constructor(…)` statement, or an inherited constructor plan, runs the whole chain's initializers base-first in its prelude; a forwarding statement keeps running the forwarded ancestors' initializers at the call, as today. Rejected: literal prelude (faults the `d = this.x * 2` program); Java-strict (new E-code).
- [2026-11-18 11:25] AGREED: DEC-586 (developer, AskUserQuestion): a diamond's shared base runs its initializers once per arm, like the constructor plan. Rejected: once in linearized order.
- [2026-11-18 11:28] AGREED: plan gate: the developer approved all 6 steps as written (AskUserQuestion, 'Go, build all 6 steps'); 3C was self-graded with disclosure (advisor rate-limited)
- [2026-11-18 12:34] AGREED (ratified ASSUMED, developer 2026-11-19, AskUserQuestion): PHP leg uses a virtual protected __phorj_inits() per class of an initializing hierarchy (calling each chain class's __phorj_init_<C>), called from every constructor prelude, instead of the plan's literal per-prelude __phorj_init_<C> call list: an inherited parent constructor must run the SUBCLASS's chain in the native order (promote, inits, body), which a static call list in the parent's constructor cannot. New helper classified bucket 2 (195). Alternatives: emit each subclass's __construct from the plan (copies parent bodies into every subclass).
- [2026-11-19 13:07] AGREED: (ratified ASSUMED, developer 2026-11-19, AskUserQuestion, Invariant 16): the PHP leg's virtual protected __phorj_inits() dispatcher per class of an initializing hierarchy, called from every constructor prelude, ratified as built. PHP cost of the extra calls per construction is unmeasured (OWED, row 6). Rejected: per-class __construct emitted from the plan (copies parent bodies); leaving it ASSUMED.
- [2026-11-19 13:07] AGREED: DEC-587 RULED (developer 2026-11-19, AskUserQuestion, Invariant 15): an instance initializer that reads a field not yet set at its point in the chain is rejected at check time with a new E-code (c1: grandchild under a forwarding constructor; c1d: own forwarding constructor, DEC-585 (b) order; s1: non-forwarding own constructor over a promoted parent field). s1b and DEC-585 (c) keep running. Rejected: Java order below a forward (s1 still faults, reverses DEC-585 (b)); keep the fault; leave PENDING. Build queued as INIT-READ-BEFORE-SET.
- [2026-11-19 13:07] AGREED: INIT-READ-BEFORE-SET graded P0 (developer 2026-11-19, AskUserQuestion; rubric: the default engine faults on a program that checks clean); ruled by DEC-587 the same day, so it blocks (rule 1). Plan gate for the 6C close: go, all 6 steps.

## Formal Plan

**The rule, as one function** (`src/ast/field_inits.rs`, single source for all three legs and the inliner).
`inits_for(X)`: let `O` be the class whose constructor `X`'s plan runs (own, trait-composed, or the nearest
ancestor's: `ctor_plan`'s own decision). If `O`'s constructor contains a `parent.constructor(…)` statement, the
classes above `O` are initialized at that call, so `inits_for(X)` = the own sets (traits then own) of `O` down to `X`,
base-first. Otherwise `inits_for(X)` = the whole chain: each parent's `inits_for` in `extends` order (per arm,
DEC-586), then `X`'s own set. A multi-parent `X` with no constructor of its own concatenates each parent's
`inits_for`, then its own set. Cycle-safe as today. `field_initializers(program, X)` becomes `inits_for(X)`; the
forwarding inliner runs `inits_for(P)` at a call into direct parent `P`.

1. **Failing tests first** (`tests/differential.rs`, expected output pinned with the PHP leg, never a bare `agree`:
   p1-p4 fail identically on both native legs today, so a bare agreement case would be green before the fix).
   Cases: inherited initializer; subclass with its own constructor and no forwarding; a trait's initializer on a
   subclass of its user; three levels with an own constructor at the bottom (`C(9)` reads `x`, `y`, `z`, `w`); diamond
   with arm initializers (`base l base r`, DEC-586); the kept shapes as regression pins (forwarding order, `d = this.x *
   2`, diamond `base base`, conditional and double forwarding); a three-level forwarding chain (`C` forwards to `B`,
   which forwards to `A`, a middle class with initializers and no constructor between them: the inliner's recursive
   splice); `B extends A` over A's promoted constructor (the
   PHP-only split); forwarding into a trait constructor; a `private` ancestor field, inherited and forwarded. Confirm
   each red case is red for the stated reason (`no field x`, PHP fatal), not a setup error.
2. **Native fix**: rewrite `field_initializers` as `inits_for` (above), plus a `forwards(ctor_body)` helper that
   walks nested statements for a `parent.constructor` statement (`if`, `try`, blocks). Both native legs pick it up
   unchanged (`compiler/ctors.rs`, `interpreter/engine.rs` read the list).
3. **Inliner** (`src/checker/inline_parent_ctor/mod.rs`, a single file until this step split it): step ③ runs `inits_for(P)` instead of the owner's own set, so a
   middle class with initializers and no constructor is no longer skipped; `find_owner` and the snapshot count a
   composed trait constructor as the class's constructor (q2).
4. **PHP leg** (new code in a new file `src/transpile/init_chain.rs`; `classes.rs` 419 and `classes_synth.rs` 341 are
   past the soft cap and must not grow): per DEC-575's mapping, each class with an initializer set emits `protected
   function __phorj_init_<Class>(): void` holding its own set (traits then own), defined in the class so its private
   fields are in scope (inside a decomposed class's `T<name>` trait for multi-parent ancestors); every emitted
   constructor prelude calls `$this->__phorj_init_<C>()` for each class of `inits_for` in order, replacing the inline
   `$this->f = …` loops (`classes.rs` own-constructor prelude, `emit_init_only_construct`, `emit_synth_construct`). A
   class with an inherited constructor plan and a set of its own emits its `__construct` from the plan
   (`emit_synth_construct`) instead of a zero-arg one (q1). For the forwarding block: if the inliner can mark its
   spliced initializer run so the transpiler emits `__phorj_init_<C>()` calls there (a marker on the block, no new
   `Stmt` variant), the private-field split is fixed here; if that needs a new `Stmt` variant, it is recorded as a
   queue entry and left (no new AST surface in a defect fix without asking).
5. **Example + docs**: `examples/guide/init-chain.phg` (inherited initializers, a three-level chain, the forwarding
   order, the diamond) with an `examples/README.md` row, `phg format` run on it before the gate; UNIFIED-SPEC's
   instance-initializer paragraph (DEC-575/585/586); KNOWN_ISSUES INIT-CHAIN to FIXED (and the q2 correction);
   CHANGELOG; SLICE-STATE (queue entry out, cursor); audit plan row 4h done. LSP and editors: no new syntax and no new
   symbol, so no change (`/phg-ship-slice` row stated, not skipped). Lift: PHP property defaults on a parent now
   construct natively; check `tests/lift_roundtrip.rs` for a fixture to add.
6. **Gate + perf**: the full all-features gate (CLAUDE.md), `cargo check --no-default-features`, release build;
   `phg benchmark` before/after on a construction-heavy program (a 3-level class built in a loop), verdict recorded or
   marked OWED per DEC-365. Sabotage: drop the parent walk in `inits_for` and confirm the new cases redden on the
   right assertion (`/sabotage-check`).

**Out of scope, recorded:** row 4c's close round 3 (DECOMPOSED-PARENT-TRAITS) stays open; SET-HOOK-DURING-
CONSTRUCTION (row 4f, an initializer or promotion on a hooked property); STATIC-REDECL-TYPE (row 4i, a redeclared
field of another type, whose initializers now both run); the constructor-frame line difference in a fault inside an
initializer (VM names the constructor line, the tree-walker `main`'s): the TRACE-FRAME-NAMES class.

**Rollback:** each step is one commit by pathspec; `git revert <sha>` per step. Nothing is destructive.

**Acceptance:** every case of step 1 green on all three legs with its pinned output; every kept shape unchanged;
full gate green; sabotage red; INIT-CHAIN out of the queue.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing differential cases, red for the stated reason | M | done | 10 cases red before the fix (no field x, PHP fatal); 5 kept shapes pinned | tests/differential.rs |
| 2 | Native: field_initializers becomes inits_for | S | done | init_chain_classes + ClassDecl::ctor_forwards; both native legs green on all cases | src/ast/field_inits.rs |
| 3 | Inliner: inits_for at the call, trait-ctor owner | S | done | chain_inits + between(); trait ctor counted (q2); split into mod.rs + snap.rs | src/checker/inline_parent_ctor/mod.rs src/checker/inline_parent_ctor/snap.rs |
| 4 | PHP: __phorj_init_ per class, plan-built ctor | M | done | init_chain.rs: virtual __phorj_inits; q1 fixed; v1 is queue entry FORWARD-PRIVATE-INIT | src/transpile/** |
| 5 | Example, spec, KNOWN_ISSUES, CHANGELOG, SSOT | S | done | examples/guide/init-chain.phg three legs identical; lift case inherited_property_defaults | examples/** docs/** KNOWN_ISSUES.md CHANGELOG.md |
| 6 | Gate, perf before/after, sabotage | S | done | gate 3771/3771 all-features; two sabotages red; perf OWED: no native before/after (the pre-fix binary is gone; the 397 vs 397 ms best-of-9 ran on a saturated core and settles nothing), PHP cost of the extra calls per construction unmeasured | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
- FORWARD-PRIVATE-INIT (queue, P1 proposed): the spliced forwarding block carries no marker the transpiler can read; a fix needs a new `Stmt` variant or a block marker, asked first.
### Fragile
- `src/transpile/classes.rs` (415) and `classes_synth.rs` (338) shrank in step 4 and are still over the soft cap.
### Known issues
- `10a8480e` (`docs(plans, slice-state): ratify the 13 ASSUMED entries`) is local and unpushed; it goes out with this work's first push.
- INIT-READ-BEFORE-SET (P0, DEC-587 ruled 2026-11-19, reject at check time): found by 6C round 1; build queued in the defect queue, not in this plan.
- 6C round 1 (advisor) closed 2026-11-19: `__phorj_inits` ratified; DEC-575/585/586 status cells set BUILT; the `seed_init_chain` comment corrected (an unrelated class sharing a constructor-less trait gains only an empty dispatcher, three legs `4 1 t`); multi-parent forwarding and trait-constructor forwarding are rejected by the checker (probed).

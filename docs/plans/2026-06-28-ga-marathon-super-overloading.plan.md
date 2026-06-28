# GA Marathon — super/parent + must-use/overloading + tooling/stdlib

## Decisions Log
- [2026-06-28] AGREED: marathon order (user-confirmed) —
  1. **Spec super/parent dispatch** (design + doc)
  2. **must-use returns (Slice A)** + breaking codemod
  3. **return-type overloading (Slice C)** — depends on must-use
  4. **super/parent dispatch impl (Slice B)**
  5. **M4 stdlib breadth** (additive Core.* ops)
  6. **cross-file LSP + JetBrains plugin** (last — tracks the now-stable grammar)
  Autonomous; each slice green + byte-identical + committed; stop only on a genuine design fork; developer pushes.
- [2026-06-28] AGREED (must-use): any non-`void`/`Empty` expression-statement must be used or
  `discard`-ed (scope Option 1). `discard <expr>` is a contextual keyword (not `void f()`). Breaking → codemod.
- [2026-06-28] AGREED (return-type overloading): overloads may differ only in return type; resolved
  compile-time from a SHALLOW/direct-only sink set (typed binding / typed reassignment / typed field
  write / `return` / non-overloaded typed param); everywhere else needs `<type>f(...)`.
- [2026-06-28] AGREED: `<type>f(...)` is an overload SELECTOR, distinct from `as` (cast). Subtyping
  resolution = exact → unique-assignable → else `E-OVERLOAD-AMBIGUOUS-RETURN`. Sink/selector
  disagreement → `E-OVERLOAD-SELECT-CONFLICT`.
- [2026-06-28] CONCEDED to user: `discard <int>f()` is valid (compiler can't enforce side-effect
  parallelism); bare `discard f()` on a return-overload → `E-OVERLOAD-NO-CONTEXT` (missing selector).
- [2026-06-28] AGREED: PHP transpile of return-overloads via per-return name-mangling; single-return
  names stay bare (existing programs byte-identical). Param-overloads stay runtime-dispatched.

## Specs
- `docs/specs/2026-06-28-must-use-and-return-type-overloading-design.md` — written, committed `ef086bb`, user-approved.
- super/parent dispatch — design pending (step 1).

## Status
- DONE (pre-marathon-extension): M-perf S1b slot-indexed fields (`6b71232`) + S2 VM inline cache
  (`7152edf`, ~20% on field-heavy code). 1438 tests green w/ PHP-8.5 oracle.
- NEXT: step 1 — spec super/parent dispatch.

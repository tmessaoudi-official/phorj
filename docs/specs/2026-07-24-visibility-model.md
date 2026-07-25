# SPEC (RULED — BUILT, 2026-07-25) — Visibility / access model completeness

> Status: **RULED 2026-07-24; BUILT + DEC-268-CERTIFIED 2026-07-25.** DV-1+DV-2 shipped (`de75201`);
> DV-3 (member `internal`) shipped + certified (round 1 found+fixed P1 iface-vis bypass / P3 set-vis
> wider; rounds 2-3 clean — two consecutive feature-clean rounds); DV-4 verified already-fixed (W0-2).
> DV-3's `internal`-on-constructor-promoted-params follow-up is now also DONE (single-sourced via
> `Modifier::is_member_visibility`). One pre-existing item remains for a dev ruling: P-Q-B-1 (overloaded
> interface-method vis narrowing). DV-5 (global
> completeness sweep) is a separate research pass, not this build. Spawned from the
> wildcard-import design when the developer spotted the visibility matrix was incomplete/asymmetric.
> Per Inv 15 (design is the developer's) + Inv 19 (records live in-repo, ZERO divergence): mirrored
> as a QUEUED slice in `docs/plans/MASTER-PLAN.md` + `docs/plans/SLICE-STATE.md`.
> Sequencing (dev-ruled): its OWN spec + slice, built AFTER wildcard imports.

## FINAL RULED MATRIX (dev, AskUserQuestion, 2026-07-24)

**A NEW package HIERARCHY is introduced**: a dotted-prefix ancestor relation — `Acme.App` is an
ancestor of `Acme.App.Sub` (and of `Acme.App.Sub.Deep`). `internal` is REDEFINED to mean
"this package **and all its descendants**" (was: exact package). Backward-compatible for user
PROGRAMS (loosening only); a handful of negative conformance tests re-baseline.

| Level | Top-level items | Class members |
|---|---|---|
| `private` | this FILE only (not importable) | this CLASS only |
| `protected` | — (N/A: no inheritance at top level) | this class **+ subclasses** |
| `internal` | this package **+ descendant packages** (REDEFINED) | this package **+ descendant packages** (NEW — reuse same keyword, same meaning) |
| `public` (default) | everywhere | everywhere |

`internal` is the SAME concept/keyword on both axes (dev ruling). `protected` stays members-only.
Member default stays `public` [Verified: `types_decls.rs:265`]. The C#-style combos
(`protected internal`, `private protected`) were REJECTED — 4 clean levels, no combinations.

## Ground truth — what exists TODAY (all [Verified] from source)

### Axis 1 — top-level item visibility (`ast::Visibility`, exprs.rs:397; enforced `loader::vis_violation`, mod.rs:69-85)

| Keyword | Reach | Enforcement (Verified) |
|---|---|---|
| `private` | **this FILE only** — not even importable elsewhere in the same package | mod.rs:74 same-package-diff-file → E-VIS-PRIVATE |
| `internal` | **this EXACT package only** — sub-packages are DIFFERENT packages, denied | mod.rs:73 `info.package == referrer_pkg` (exact string ==); else :82 E-VIS-INTERNAL |
| `public` (default) | everywhere | mod.rs:81 → None |

Ordered lattice `Private < Internal < Public` (exprs.rs:396). **No inheritance/subtree notion.**

### Axis 2 — class-member visibility (`ast::Modifier`, exprs.rs:357; checker `MemberVis`)

| Keyword | Reach |
|---|---|
| `private` | this class only |
| `protected` | this class **+ subclasses** |
| `public` (default) | everywhere [Inferred: `MemberVis::Public` is the collect-stage default, types_decls.rs:265/354] |
| `private(set)` | reads at declared vis; ASSIGN only in owning class (DEC-241, → PHP 8.4 `private(set)`) |

## The GAPS (this is what "understand the gaps of what we have now" asked for)

- **G1 — no package-SUBTREE level on either axis.** `internal` = EXACT package. A child package
  `Acme.App.Sub` canNOT see `Acme.App`'s internals — it's "different package" → public-only. The dev
  wants a "package + sub-packages" level. **Blocker:** the loader has NO package-hierarchy relation
  (dotted-prefix parent/child) at all — packages are compared by exact string. Introducing subtree
  visibility means FIRST introducing that relation. [Verified: mod.rs:73 exact ==]
- **G2 — member axis has NO `internal`** (package-visible member) — the asymmetry the dev spotted.
  **Round-2 RULED: ADD member `internal`.** Semantics question below (DV-3).
- **G3 — top-level has no `protected`** — correct *by nature* (a free function has no subclasses, so
  the inheritance axis is meaningless). The dev's "we need it" is really a request for the SUBTREE
  level (G1), which they loosely called "protected". Two DIFFERENT axes; must not conflate.
- **G4 — [Verified: H-enforcement audit §2.1] P0 BUG in the CURRENT model:** private/protected
  **static FIELD** visibility is UNENFORCED — read AND write from outside compile clean, run on both
  engines, and the PHP leg FATALS → a live run≡vm≢PHP byte-identity break (Inv 1). Root cause:
  `classes[cls].statics` is `name→Ty` with no vis metadata (consts carry vis and DO enforce). This is
  EXISTING debt squarely in the visibility subsystem — a natural fold-in for this slice.
- **G5 — [Verified: H-audit §2.2] static-method-via-instance** (`a.staticMethod()`) accepted;
  contradicts the stated static-discipline rule (field case IS enforced). P1.
- **G6 — [Verified: H-audit §2.3 / M2] unknown import silently accepted** (`import Acme.Nothing;`
  checks OK while unused). P1 — and DIRECTLY relevant to the wildcard-import feature: that slice
  should introduce `E-IMPORT-UNKNOWN` (Go model, beats PHP). Cross-linked to the imports spec.

## RULED decisions (dev, AskUserQuestion, 2026-07-24)

- **DV-1 — package hierarchy: RULED YES.** Introduce the dotted-prefix ancestor relation
  (`Acme.App` ⊐ `Acme.App.Sub`). Blast radius: `loader::vis_violation` (mod.rs:69-85 — the exact
  `==` becomes an ancestor test), checker, LSP diagnostics (Inv 17 ≡), docs, conformance tests.
- **DV-2 — RULED: REDEFINE `internal` = package + descendants** (subtree). Not a new keyword.
  Backward-compatible for user programs (loosening); re-baseline the negative conformance/checker
  tests that assert E-VIS-INTERNAL fires from a child package (`conformance/types/visibility.phg`,
  `src/checker/tests/visibility.rs`).
- **DV-3 — RULED: member `internal` = same subtree meaning as top-level `internal`** (reuse the
  keyword). Member `internal` = PACKAGE-subtree-visible (never class-only — class-only IS `private`);
  a top-level *class* marked `internal` = referenceable only within its package subtree.
- **DV-4 — RULED YES: fold the G4 P0 static-field fix into this slice.** Store vis alongside the
  static's type (mirror consts), gate read (`calls.rs`) + write (`assign.rs`) with the
  E-CONST-VISIBILITY owner/subclass logic; extend E-FIELD-VISIBILITY. Restores run≡vm≡PHP.
- **DV-5 — RULED: global completeness sweep is its OWN research pass** (separate from this build).
  Reuse the rich existing audits (`docs/research/full-audit/`, `docs/research/roadmap-completeness/`,
  `docs/research/2026-07-16-full-reopen-audit.md`) + a fresh `/gaps` sweep, synthesized into ONE
  ranked completeness register. Ruled before any of it builds. (G5 static-method-via-instance is a
  candidate finding for that register.)
- **G6 (cross-linked) — RULED: `E-IMPORT-UNKNOWN` belongs to the WILDCARD-IMPORT slice**, not here.
  Recorded in `2026-07-24-wildcard-imports.md`.

## BUILD STATUS (autonomous, 2026-07-25)

- ✅ **DV-1 + DV-2 DONE (`de75201`)** — package hierarchy relation `pkg_is_ancestor_or_equal`
  (`loader/mod.rs`) + top-level `internal` REDEFINED to package-subtree. `vis_violation` gates
  `internal` through the ancestor test; a descendant package reaches an ancestor's internals, siblings
  and ancestors do not. Purely a loosening; top-level vis is loader-erased (no PHP/transpile change).
  `wildcard_members` inherits it via the single-sourced `vis_violation`. Tests:
  `internal_function_visible_from_descendant_package_is_allowed` (+ ancestor/sibling negatives). No
  conformance/checker re-baseline was needed — every existing negative test uses `Main` as referrer
  (never a descendant), so it stays correctly rejected.
- ✅ **DV-4 (G4 static-field visibility) — VERIFIED ALREADY FIXED [Rule 11].** The spec's G4
  ground-truth PREDATES the "W0-2" fix. Static-field vis IS collected (`static_vis`,
  `collect/types_decls.rs:449`), threaded through inheritance (`collect/inherit.rs`), and ENFORCED at
  read (`calls/methods.rs:529`) AND write (`assign.rs:214`). Live probe: an out-of-class `private
  static` read and write both reject `E-FIELD-VISIBILITY`. No byte-identity break remains — run≡vm≡PHP
  restored. Nothing to build here.
- ✅ **DV-3 (member `internal`) — DONE (2026-07-25).** Solved WITHOUT the feared loader→checker API
  threading: the merged program mangles every non-`Main` definition to `Pkg\…\Name`, so the checker
  derives each class's package straight from the mangled name it already holds (`pkg_of_mangled`) and
  tracks a `cur_package` (set at the class-body / free-function / static-init entry points). Member
  `internal` visibility = `pkg_subtree_contains(owner_pkg, cur_package)` at the 4 member-vis sites
  (const read, `enforce_member_vis`, `enforce_set_vis`, `enforce_ctor_vis`). Added `Modifier::Internal`
  + `MemberVis::Internal` + `parse_modifiers` support. Loose/`Main` → `""` package → same-package
  visible (sound). Transpile: `internal` erases to PHP `public` (empty read-vis, identical to a default
  field) — byte-identity verified VM≡tree-walker≡PHP. Formatter round-trips `internal`. **v1 carve-out:**
  `internal` on a constructor-PROMOTED param is `E-INTERNAL-PROMOTION` (supporting it needs the ~11
  promotion-detection `matches!` sites across transpile/layout/native — a follow-up; a plain `internal`
  field works). Tests: `internal_member_is_visible_from_descendant_package` +
  `_not_visible_from_unrelated_package` (project), `internal_member_within_same_package_is_visible` +
  `internal_on_promoted_ctor_param_is_rejected` (loose). Example `project/member-internal/` (Inv 9,
  byte-identity gated) + README + `explain E-INTERNAL-PROMOTION`. Full gate green.
  **DEC-268 round-1 panel found + fixed 2 full-set-coverage misses on non-exhaustive `matches!`:**
  **P1** — an interface method implemented `internal` bypassed `E-IFACE-VIS` (`interfaces.rs`), letting
  the boundary be dodged by upcasting to the interface → now rejected like private/protected; **P3** —
  `internal` + `protected(set)` wasn't flagged `E-SET-VIS-WIDER` (`types_decls.rs`, an out-of-package
  subclass could write what it can't read) → now flagged. Both with regression tests. (Pre-existing,
  out-of-scope: double-visibility token combos like `protected internal` type-check by precedence
  rather than being rejected — same as `public private` today; a strict-combo-rejection follow-up if
  desired.)
- ✅ **DV-3 follow-up — `internal` on constructor-promoted params — DONE (2026-07-25).** Instead of
  editing the 12 scattered promotion `matches!(Public|Private|Protected)` sites by hand (the exact
  drift the panel had just caught), SINGLE-SOURCED them: added `Modifier::is_member_visibility()`
  (public/private/protected/internal) and routed every promotion detector through it (transpile
  `is_promoted`, `ast/class_layout` ×2, `native`, `interpreter/construct`, `compiler`, `desugar_db`/`di`,
  `inline_parent_ctor`, `collect`, `type_bodies` ×2). Transpile `vis()` now maps `Internal` → PHP
  `public` EXPLICITLY (required — a promoted param needs a visibility keyword: `public int $x` promotes,
  bare `int $x` is just an argument → would drop the field). The `E-INTERNAL-PROMOTION` rejection +
  explain entry are removed. Byte-identity verified: `constructor(internal int x)` → `function
  __construct(public int $x)`, VM≡tree-walker≡PHP. Cross-package enforcement holds (an unrelated package
  reading the promoted internal field → E-FIELD-VISIBILITY). Tests: `internal_promoted_ctor_param_is_a_field`
  (loose), `internal_promoted_ctor_param_field_is_enforced_cross_package` (project).
- ⬚ **P-Q-B-1 (dev to rule) — overloaded interface-method visibility narrowing.** [Verified: Q-B DV-3
  round-2 panel] `E-IFACE-VIS` (`interfaces.rs`) only fires when the class provides a SINGLE overload
  of the interface method (`method_vis` records just the first overload's modifiers). With >1 overload,
  a reduced-visibility impl (`private`/`protected`/`internal`) is reachable through a plain
  interface-TYPED receiver (`Shape s = new Box(); s.m()`) with NO enforcement — the methods.rs
  access-site backstop covers only the lone class member of an INTERSECTION type, not a plain interface
  receiver. **Pre-existing and equal for all three reduced visibilities** (reproduces with `private`
  overloads) — DV-3's `internal` merely inherits it, so it is NOT a DV-3 regression. Closing it needs
  per-overload conformance tracking; the design question (must a whole overload SET be public to
  implement an interface method?) is the developer's (Inv 15). The misleading "backstop" comment has
  been corrected in place; recorded here as QUEUED.
- (historical) DV-3 was a PARSE ERROR before this slice [Verified: `expected a type name, found Internal`]. The
  BLOCKER is architectural: member visibility is enforced in the CHECKER on the loader-MERGED flat
  program, but `ClassInfo` carries NO package and `check_program(&program, &diag_src)` receives no
  package map — the checker is package-unaware post-merge (top-level `internal` works only because it
  is enforced in the LOADER, pre-merge, where packages exist). Enforcing member-`internal` subtree
  visibility needs package info in the checker. **APPROACH (identified, not yet built):** the loader
  ALREADY has the data — `DefInfo.package` in `prov_fns`/`prov_types`. (1) add `Modifier::Internal`
  (`ast/exprs.rs`) + parser support in `parse_modifiers`; (2) add `MemberVis::Internal`
  (`checker/mod.rs:262`) + map it in `MemberVis::of`; (3) loader passes a `HashMap<mangled_name,
  package>` into `check_program`; (4) checker stores the owner package on `ClassInfo` and sets a
  `cur_package` when entering each function/method; (5) gate `internal` member access via
  `pkg_is_ancestor_or_equal(owner_pkg, cur_pkg)` at the ~5 member-vis sites in `calls/methods.rs` +
  `assign.rs` (loose mode: everyone is `Main`, so `internal` is trivially visible — byte-safe);
  (6) transpile/lift: member `internal` ERASES to PHP `public` (document the rationale — PHP has no
  package concept, Inv 17); (7) TDD + a runnable example (Inv 9) + `explain` entries. ~200-300 lines
  across loader→checker interface + 5 sites; a half-enforced version (parse-accept without project-mode
  subtree gating) is a SOUNDNESS HOLE, so it is NOT sliceable into smaller green commits — built whole
  or not at all. Deferred here rather than started unfinishably at the tail of a long session.

## Invariants for the eventual build

Inv 1 (fixing G4 restores the spine). Inv 17 (checker ≡ LSP; transpile+lift same change — top-level
vis is loader-erased so PHP-invisible, but member `internal` must erase cleanly to PHP `public` with
a documented rationale). Inv 13 (split as you go). Inv 19 (mirror to MASTER-PLAN+SLICE-STATE on ruling).

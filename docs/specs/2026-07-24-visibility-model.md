# SPEC (RULED — QUEUED for build, NOT YET BUILT) — Visibility / access model completeness

> Status: **RULED 2026-07-24 (dev, AskUserQuestion), QUEUED, NOT BUILT.** Spawned from the
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

## Invariants for the eventual build

Inv 1 (fixing G4 restores the spine). Inv 17 (checker ≡ LSP; transpile+lift same change — top-level
vis is loader-erased so PHP-invisible, but member `internal` must erase cleanly to PHP `public` with
a documented rationale). Inv 13 (split as you go). Inv 19 (mirror to MASTER-PLAN+SLICE-STATE on ruling).

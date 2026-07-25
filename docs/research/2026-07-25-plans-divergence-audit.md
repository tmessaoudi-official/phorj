# Planning/Spec Divergence Audit — 2026-07-25 (post Q-A + Q-B night)

Scope: verify Invariant 19 (ZERO divergence — every roadmap item / decision / slice-status in exactly
ONE canonical home; MASTER-PLAN + decision register + SLICE-STATE mutually consistent + up-to-date)
after tonight's shipped+certified work: Q-A wildcard/group imports (DONE+certified), Q-B visibility
model (DV-1/2/3 + ctor-promoted follow-up + DV-4 already-fixed), and the LSP import-completion dup fix
(66f940b).

Method: read the six canonical/living docs + both specs + downstream surfaces (CHANGELOG, FEATURES,
MILESTONES, KNOWN_ISSUES, HISTORY, INVARIANTS); cross-checked representative claims against
`git log --oneline -30`, `git rev-parse origin/master`, and the actual `src/` tree.

Positive result (Inv-19 headline status): MASTER-PLAN (:137-148), SLICE-STATE (:27-62), and both spec
status headers all agree that **Q-A and Q-B are DONE + DEC-268-certified**. Source confirms the
as-built state (`Modifier::Internal`, `MemberVis::Internal`, `is_member_visibility` all present;
`E-INTERNAL-PROMOTION` removed from `src/` — now only in the spec doc). The DONE headline is NOT
divergent. The findings below are missing records, stale cursors, and downstream-surface gaps.

Severity: HIGH = misleads a fresh-context resume or breaks an Inv-19 canonical-home rule; MEDIUM =
real inconsistency a reader will hit; LOW = cosmetic / historical-plan-text drift.

---

## HIGH

### H1 — Whole Q-A/Q-B ruling cluster missing from the decision register [missing records / Inv 19]
- **Where:** `docs/research/full-audit/raw/C-decisions.md` (highest DEC = DEC-335; no row for the
  import/visibility cluster). Verified: `grep -niE 'wildcard.import|visibility.model|member.internal|package.hierarchy|descendant.package'` returns nothing for the cluster.
- **Divergence:** the 2026-07-24 dev AskUserQuestion rulings that authorized tonight's build (Q-A: D1–D5,
  D-process, the 7-code catalog; Q-B: DV-1..DV-5, the redefinition of `internal` = package + descendants,
  member `internal`, the G4/DV-4 fold-in) and the DEC-268 two-clean-round certifications are recorded in
  the two specs + SLICE-STATE + MASTER-PLAN, but **never in the decision register**. Inv 19 names the
  register as a canonical home for "every DEC row / ruling."
- **Correct state:** a DEC row (or an explicit dated ruling block) for the import/visibility cluster,
  cross-linked to `docs/specs/2026-07-24-wildcard-imports.md` + `-visibility-model.md`, recording the
  sub-decisions + the DEC-268 certification outcomes; MASTER-PLAN/specs then point to it.
- **Fix:** append the register entries (assign a DEC number or an explicit "2026-07-24 import/visibility
  cluster" heading matching the register's existing ruling-block style).

### H2 — SLICE-STATE "Pushed" cursor is stale/understated [staleness]
- **Where:** `docs/plans/SLICE-STATE.md:62` — `**Pushed:** origin/master @ de75201 (Q-A + Q-B DV-1/2).
  Q-B DV-3 + DONE docs pushing now.`
- **Divergence:** `git rev-parse HEAD == origin/master == 66f940b`; `git status -sb` = clean
  (`master...origin/master`, no ahead/behind). Every commit through 66f940b (DV-3, the ctor-promoted
  follow-up 48df40c/b727ee4, the DONE docs 65b7642, the LSP fix 66f940b) is already on origin. The line
  tells a resumer DV-3 + DONE docs are still unpushed — false.
- **Correct state:** `Pushed: origin/master @ 66f940b — all of Q-A + Q-B (DV-1/2/3 + ctor-promoted
  follow-up) + the LSP import-completion fix are on origin; tree clean.`
- **Fix:** update the Pushed line.

### H3 — MASTER-PLAN lists a DONE follow-up as still open [divergence]
- **Where:** `docs/plans/MASTER-PLAN.md:148` — `Bounded follow-up: internal on ctor-promoted params.`
- **Divergence:** presented as an open/queued follow-up, but SLICE-STATE:54-58, the visibility-model
  spec status header (:6-7) + §follow-up (:139-150), examples/README:238, and git (48df40c "feat…
  support internal on ctor-promoted params", b727ee4, 65b7642 "DV-3 follow-up DEC-268-certified") all
  say it is DONE + certified. Source confirms `E-INTERNAL-PROMOTION` is removed.
- **Correct state:** mark the ctor-promoted-param follow-up DONE (single-sourced via
  `Modifier::is_member_visibility`; `E-INTERNAL-PROMOTION` removed; byte-identity verified).
- **Fix:** update MASTER-PLAN:148 (change "Bounded follow-up:" to a DONE note or drop it, since it is
  no longer a follow-up).

---

## MEDIUM

### M1 — MILESTONES "Visibility modifiers — COMPLETE" section stale vs Q-B [staleness]
- **Where:** `docs/MILESTONES.md:219-229`.
- **Divergence:** the section still describes the pre-Q-B model:
  - `:222` `internal` = "this package's files" (exact package) — Q-B DV-2 REDEFINED it to
    "this package + descendant packages" (subtree).
  - `:229` "member-level Modifier visibility stays PHP-only-enforced" + Deferred — Q-B DV-3 added
    checker-enforced member `internal` (and member private/protected were already checker-enforced per
    examples/README:162 "Wave 1.1", so this line was partly stale before tonight too).
  - `:226` `run ≡ runvm ≡ real PHP` uses the retired `runvm` verb (Inv 1: "there is NO runvm command";
    pre-existing drift, in-scope as a Broken Window in the section being corrected).
- **Correct state:** reflect subtree `internal` (DV-1/2) + checker-enforced member `internal` (DV-3);
  `runvm` → `run --tree-walker`.
- **Fix:** revise the section (or append a "2026-07-25 Q-B update" note); fix the `runvm` verb.

### M2 — FEATURES.md has the Q-A row but no Q-B row [missing record / Inv 17 surface gap]
- **Where:** `FEATURES.md:94` (Q-A wildcard row present) — no row for Q-B.
- **Divergence:** member `internal` + the redefined subtree `internal` (Q-B DV-2/DV-3) are fully
  documented in `examples/README.md:238` but have no FEATURES.md row. Both files are the "living surface"
  SSOT per Inv 17 / project CLAUDE.md ("Language surface: FEATURES.md + examples/README.md"). The two
  surfaces disagree on Q-B coverage.
- **Correct state:** a FEATURES.md row for member `internal` (package-subtree-visible; erases to PHP
  `public`) + the top-level `internal` redefinition to package + descendants, pointing to
  `examples/project/member-internal/`.
- **Fix:** add the FEATURES row.

### M3 — CHANGELOG [Unreleased] missing Q-A, Q-B, and the LSP fix [missing records]
- **Where:** `CHANGELOG.md` `[Unreleased]` — top entries are DEC-331 slice 2 then slice 1; no Q-A / Q-B.
- **Divergence:** `grep -niE 'wildcard|Q-A|Q-B|visibility'` in CHANGELOG finds none of tonight's work.
  Q-A (wildcard/group imports + 7-code catalog) and Q-B (subtree `internal` + member `internal` + G4
  spine confirmation) are notable language changes; the LSP import-completion dup fix (66f940b) is a
  Fixed-worthy bug. CHANGELOG's own preamble says "All notable changes."
- **Correct state:** `### Added` entries for Q-A and Q-B under `[Unreleased]`; a `### Fixed` entry for
  the LSP dotted-import-completion dup.
- **Fix:** add the three entries.

### M4 — SLICE-STATE intra-doc "CURRENT" marker is false [staleness]
- **Where:** `docs/plans/SLICE-STATE.md:93` — `← CURRENT: S3.1 in flight (Phase 2 grounding).`
- **Divergence:** S3.1 is DONE (SLICE-STATE:14 "S3.1 (#34) COMPLETE"; git 12969d0/b52bc12) and the live
  top block (:60) says NEXT = Q-C. The accreting SLICE-STATE now carries several coexisting
  CURRENT/NEXT markers (:60, :93, :246); only the top block is authoritative, but the ":93 ← CURRENT"
  reads as the live cursor to anyone scanning for it.
- **Correct state:** strike or annotate ":93" as superseded (S3.1 DONE; live cursor is the top block).
- **Fix:** mark line 93 historical.

---

## LOW

### L1 — Wildcard spec build-plan example path is wrong vs shipped [divergence, historical plan text]
- **Where:** `docs/specs/2026-07-24-wildcard-imports.md:131` (step 7) — `examples/guide/wildcard-imports.phg`.
- **Divergence:** the shipped example is the directory `examples/project/wildcard-imports/` (verified on
  disk; `examples/guide/wildcard-imports.phg` does not exist). Wildcards need a cross-package project, so
  it landed under `project/`, correctly referenced by FEATURES:94 + examples/README:236. The spec's
  build-plan step-7 path is stale.
- **Correct state:** note the example is `examples/project/wildcard-imports/` (project, not guide).
- **Fix:** annotate the spec step-7 path (or leave as historical build-plan text with a "shipped as
  project/…" note).

### L2 — Visibility spec DV-3 "v1 carve-out" contradicts its own follow-up section [intra-spec staleness]
- **Where:** `docs/specs/2026-07-24-visibility-model.md:124-125` (DV-3 §"v1 carve-out") vs `:139-150`
  (§follow-up) + status header `:6-7`.
- **Divergence:** the DV-3 body still states `internal` on a ctor-promoted param = `E-INTERNAL-PROMOTION`
  as current behavior, while the follow-up section + header + source say it was removed
  (`E-INTERNAL-PROMOTION` is gone from `src/`). A reader of the DV-3 paragraph alone gets the superseded
  state.
- **Correct state:** annotate the carve-out paragraph as superseded by the follow-up.
- **Fix:** add "(SUPERSEDED — see the ctor-promoted-param follow-up below)" to the DV-3 carve-out.

---

## Follow-up tracking check (task item 4) — mostly consistent

- **P-Q-A-1..5** — canonical home = wildcard spec §PENDING (:167-212); pointed-to from SLICE-STATE:34,59
  and MASTER-PLAN:141. Consistent. **Sub-note (tracked, not a new finding):** P-Q-A-2 flags that D3's
  ruled wording "`*` binds public + internal" conflicts with the as-built cross-package public-only
  behavior; FEATURES:94 + examples/README:236 correctly describe the as-built ("PUBLIC member"), so the
  surfaces are consistent with reality — the D3 spec wording is the item awaiting dev confirmation (its
  canonical home, the spec, records this). No action beyond the existing P-Q-A-2 entry.
- **P-Q-B-1** — canonical home = visibility spec (:151-161); pointed-to from SLICE-STATE:52,59. **Minor
  gap:** not referenced from MASTER-PLAN (which mentions only the now-DONE ctor-promoted follow-up). LOW —
  optional to add a MASTER-PLAN pointer for symmetry with P-Q-A tracking.
- **DV-3 follow-up (internal on ctor-promoted params)** — DONE; consistently reflected in SLICE-STATE +
  spec + examples/README:238; the ONE stale reflection is MASTER-PLAN:148 (finding H3).

---

## Suggested fix order
H1 (register — the canonical-home gap) → H2/H3 (cursor + MASTER-PLAN staleness, both one-liners) →
M1/M2/M3 (downstream surfaces) → M4/L1/L2 (annotations). All are doc-only; no source change implied by
this audit (source matches the as-built claims).

# defect grading (2026-11-17) Plan

> The defect-queue grading pass owed under DEC-578 (north-star rule 1): every entry of the DEFECT QUEUE in
> `docs/plans/SLICE-STATE.md` gets a PROPOSED grade (P0-P3) with its reason and the line it rests on. Grades are the
> developer's: a proposal blocks or not only once ratified (north-star "Open"); until then the interim of SLICE-STATE
> stands (an ungraded or unratified entry is treated as P1). No `src/` change; nothing is re-run (the developer chose
> a Medium pass: an entry whose current status its text cannot settle is marked "re-probe owed").
> Follows the ground-prep plan (`docs/plans/2026-11-14-ground-prep.plan.md`, complete), whose Needs input names this
> pass and row 9's candidates.

## Decisions Log
- [2026-11-17 11:36] AGREED: Grading pass plan (developer, via AskUserQuestion, 2026-11-17): GO as a Medium pass - every grade a PROPOSAL with its reason and the line it rests on, by north-star rule 1's letter calibrated by the ruled pair; resolved entries proposed for removal with evidence; no re-runs (re-probe owed where the text cannot settle status). Rejected: Large with re-runs; Small from headings.
- [2026-11-17 11:36] AGREED: Row 9 scope (developer, via AskUserQuestion, 2026-11-17): P0/P1 AND the P2 defects of row 9 enter the queue after dedupe (not the docs or design P2s), named by class only. Rejected: P0/P1 only (recommended); none.
- [2026-11-17 11:36] AGREED: Plan home (developer, via AskUserQuestion, 2026-11-17): a NEW plan file, this one; the ledgered ground-prep plan is not reopened. Rejected: row 11 there; SLICE-STATE only.
- [2026-11-17 11:55] AGREED: Publication of row 9's security-shaped findings (developer, via AskUserQuestion, 2026-11-17, the 6C round 1 advisor finding): CLASS ONLY - R9-A-1, R9-A-2, R9-A-3 and R9-D-1 are queue rows that name the surface but no mechanism or reproduction, which stay in the gitignored row 9 report (the DEC-581 precedent). Rejected: a pointer line only; the mechanisms as drafted.

## Formal Plan
1. Re-read every queue entry's body (KNOWN_ISSUES section, audit-plan row, scout row, row 8 text). An entry the text
   shows resolved (FIXED, or accepted by the developer) is proposed for removal with that line.
2. Grade by rule 1's letter, calibrated only by the ruled pair (`DECIMAL-ORDER` blocks, `TRACE-FRAME-NAMES` does not).
   An old-scale self-grade that conflicts with the letter is listed as a conflict, not settled. "Awaits a ruling - does
   not block" is a status beside the grade, not a grade.
3. Row 9 findings (`var/claude/aggregate-findings/2026-11-16/`): P0/P1 and the P2 defects enter the queue after dedupe
   against existing entries, named by class only (no reproduction or file:line from the gitignored reports).
4. SLICE-STATE: grades, re-derived section counts, cursor lines; ground-prep Needs input bullets point here.
5. Rulings left to the developer: ratify the grades; DEC-581 (owed at this pass by the 2026-11-16 ruling); the other
   Invariant-15 items are listed for the closing check-in.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Propose a grade for every DEFECT QUEUE entry (41 seeded) and add 36 rows: 33 from row 9 by class (after dedupe: AE-5 into VALIDATION, H-1 into DEEP-INPUT-CRASHES; CJ-3 lifted from its P3 tail), row 9b V10, and two numbered serve items (SERVE-REASON-PHRASE, SERVE-SPILL); 77 entries, 4 proposed P0; Conflicts and the rulings not made listed under the tables | M | doing | - | docs/plans/SLICE-STATE.md docs/plans/MASTER-PLAN.md docs/plans/2026-11-17-defect-grading.plan.md |
| 2 | The developer ratifies the grades (one pass, exceptions marked), then rules the Conflicts and DEC-581; ratified grades replace "proposed" in SLICE-STATE and the two interims retire | S | blocked | - | docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
- Step 2 waits on the developer's ratification (north-star "Open": a grade is a proposal until ratified).
### Needs input
- Ratify the proposed grades in SLICE-STATE § DEFECT QUEUE (74 entries), and the proposed P0/P1/P2/P3 rubric with them.
- The ten Conflicts listed under the queue's tables.
- A deviation from the AGREED plan entry: it promised "the line it rests on" for each grade; the rows name the entry and
  quote its facts instead, without line numbers (they drift; the quoted-anchor convention). Accept, or ask for anchors.
- DEC-581 (Cookie guard widening), owed at this pass by the 2026-11-16 ruling; then K6, AE-14, Q-0908-3 and the other
  rulings listed under the tables (raised at the closing check-in, never decided by Claude).
### Needs research
- Row 9's P3 tail (about 41 sleuth and 38 inspect items) was not re-read against rule 1's letter; only CJ-3 was lifted.
- `KNOWN_ISSUES.md` headings were not reconciled against the queue: NARROWED-REASSIGN and LIFT-TERNARY-IN-CONCAT are
  unread candidates in neither the queue nor an exclusion; numbered items inside larger sections were not swept (two
  were found and added at 6C round 1); the five seeding exclusions were not re-read.
- Re-probes owed (marked in the queue): TRANSPILE-NS-REFLECT-TABLES, F-029, R9-H-5, R9-CJ-3.
### Fragile
- The reproductions behind every `R9-…` row exist only in the gitignored `var/claude/` reports on the developer's
  machine; the public rows name classes only (developer ruling 2026-11-17). Losing that folder loses the detail.
### Known issues

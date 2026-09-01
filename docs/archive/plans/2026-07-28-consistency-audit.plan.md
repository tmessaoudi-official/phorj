# Consistency Audit (2026-07-28) Plan

## Decisions Log
- [2026-07-28] AGREED: Option 1 — audit FIRST, feeding the question journey. Unambiguously false
  claims fixed + committed autonomously (docs-only, git-autonomy); contradictions needing a ruling
  join the pending-question batch, presented as ONE consolidated plain-text set afterwards.
- [2026-07-28] AGREED (developer addition): doc↔code gap check is BIDIRECTIONAL — code that does
  something undocumented AND docs that claim something unimplemented — plus a free-form
  "anything nasty that could bite us later" sweep.
- [2026-07-28] Proven seed finding: `CITATION.cff` abstract says "std-only, zero external crates"
  while `Cargo.toml` admits 14 vetted crates. FALSE-CLAIM, unambiguous, fix in this audit.

## Formal Plan

Six lenses, run as 5 parallel read-only subagents (LLM-cap ≤5), each writing raw findings to the
session scratchpad and returning a compact summary:

| Agent | Lens | Surface |
|---|---|---|
| A | L1 truth-vs-reality | Every factual claim in README/CITATION.cff/FEATURES/CLAUDE.md/ARCHITECTURE/MILESTONES/CHANGELOG/KNOWN_ISSUES/SEMVER/SECURITY/examples README vs code + Cargo.toml |
| B | L2 rule-vs-rule + L3 unwritten rules | INVARIANTS × CLAUDE.md × register × specs: mutual contradictions, rule-vs-implementation drift, standing rulings never written into INVARIANTS.md |
| C | L4 self-contradiction | DEC register supersession chains; MASTER-PLAN × SLICE-STATE × register zero-divergence check (Inv 19); spec-vs-spec conflicts |
| D | L5a doc→code | Documented but not implemented: phantom CLI flags (e.g. `--sequential-concurrency`), error codes, env vars, natives, features |
| E | L5b code→doc + L6 nasty sweep | Implemented but undocumented: flags/env vars/natives/features; plus TODO/FIXME landmines, broken doc links, stale paths, orphan examples |

Every finding: severity (P0–P3), claim location (file:line), reality evidence (file:line),
classification (FALSE-CLAIM / CONTRADICTION / UNWRITTEN-RULE / DOC-GAP / CODE-GAP / NASTY),
and UNAMBIGUOUS vs NEEDS-RULING.

Then: synthesis report persisted at `docs/research/2026-07-28-consistency-audit.md`;
unambiguous fixes applied + committed; NEEDS-RULING items merged with the existing pending
adjudication queue into one question batch (ask-human protocol). DEC-268 panel certifies the
fixes before commit.

Acceptance: report in repo; CITATION.cff fixed; question batch presented; zero divergence
introduced (Inv 19 — any register/SLICE-STATE touch mirrored same-change).

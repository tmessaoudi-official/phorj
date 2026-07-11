# Roadmap

This is the high-level narrative pointer for Phorj. It intentionally carries **no per-item status**
— that lives in the single sources of truth below, so nothing here can drift out of date (this file
previously accreted stale milestone markers; those now live only where they stay current):

- **Forward plan (the authority):** [`docs/plans/MASTER-PLAN.md`](docs/plans/MASTER-PLAN.md) — the
  consolidated roadmap to 100%, the waves + cursor + live percentage ledger. The per-item authority.
- **Delivered status (with commit refs):** [`docs/MILESTONES.md`](docs/MILESTONES.md).
- **Long-horizon ambition:** [`VISION.md`](VISION.md).
- **Design-decisions register:** `docs/research/full-audit/raw/C-decisions.md`.

Phorj is pre-1.0 and single-developer. Dates are intentionally omitted — milestones ship when they
ship, and the version number tracks milestone progress, not a release cadence. Design specs were
consolidated in the 2026-07-02 pass; dead `docs/specs/…` links resolve to git history (≤`60540fc`)
plus the decision register above.

## Where the project is headed

Phorj's goal is a **superset of PHP that is better on every dimension** — faster where structurally
possible, safer, better-organized, SOLID — driving toward **100% vision** (full PHP parity *plus* the
beyond-PHP programme). The active programme executing that is **the finishing wave** — see
[`docs/plans/MASTER-PLAN.md`](docs/plans/MASTER-PLAN.md) for the ordered sub-waves (web spine →
filesystem/subprocess/logging → string/Unicode → language surface → date/intl/XML → the
unplanned-stdlib long tail → beyond-PHP framework stack → perf hold → GA), the percentage ledger, and
the current cursor.

Long-term "v2 — native & systems" horizons (AOT compilation, an ownership model that removes the GC,
sized-integer performance work) are described in [`VISION.md`](VISION.md).

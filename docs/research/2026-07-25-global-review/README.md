# Global review — 2026-07-25 (raw evidence base)

**What this is.** The developer ran a review pass of his own, produced ~15 findings/questions, and asked
for them to be (a) challenged and verified against real code, (b) widened into a global project review,
and (c) turned into a decision agenda he can walk through interactively. He was asleep during the run, so
**no design decision was taken** — every fork is recorded as PENDING with a recommendation
(**Invariant 15**, ADJUDICATION RULE).

**This directory holds the RAW, per-topic evidence reports.** The synthesized, ranked, deduplicated
output — the thing to actually read — is:

> **`docs/research/2026-07-25-completeness-register.md`**

That register is the deliverable of the already-RULED **DV-5** research pass
(`docs/archive/specs/2026-07-24-visibility-model.md`: *"global completeness sweep is its OWN research pass …
synthesized into ONE ranked completeness register"*). These raw files are its citations, committed
because the remote container is ephemeral and only committed state survives (**Invariant 19**).

| File | Topic | Developer question(s) answered |
|---|---|---|
| `P0-block-shadow-byte-identity.md` | **P0**: shadowing in any nested block breaks the Invariant-1 spine on the PHP leg | (found while researching #8) |
| `A-package-enforcement.md` | `package` validation, PSR-4/PascalCase, file-level attribute hatch | #1, #2 |
| `B-lsp-editors.md` | LSP completion for UFCS receivers; TextMate grammar defects | #3, #6 |
| `C-stdlib-input-fs-clone.md` | streaming `Input`/file reads, filesystem locking, no-op clone | #4, #12, #15 |
| `D-database.md` | `Database`→`Connection` naming, `prepare`/statement reuse, savepoints | #9, #10, #11 |
| `E-language-surface.md` | wildcard imports, loop forms, UFCS promotion, the `main` reservation | #5, #7, #13, #14 |
| `F-block-visibility-research.md` | visibility/access inside function bodies — 5 readings + recommendation | #8 |
| `G-rust-quality.md` | Rust source quality, naming, structure, docs, Invariant-13 sizing | "source quality" ask |
| `H-docs-consistency.md` | doc claims vs behaviour, Invariant-19 SSOT divergence, contradictions | "flag any inconsistency" ask |
| `I-gaps-enforcement.md` | stated-but-unenforced rules, incomplete features, better-than-PHP gaps | global sweep |
| `J-claude-bundle.md` | the Claude global bundle: all 199 files earned IN or OUT, explicitly | "include the claude global bundle" ask |
| `K-inline-findings.md` | orchestrator's own probe findings + **two self-corrections** | #7 answer, ergonomics |
| `L-onhold-inventory.md` | every PENDING decision / ruled-not-built spec / deferred item | "all the specs we put on hold" ask |

## ⛔ THESE REPORTS ARE IMMUTABLE DATED SNAPSHOTS — check the register before actioning any row

**Read this before acting on anything in this directory.** Each report is the evidence trail of a
2026-07-25/26 investigation, and under the DEC-268 ladder the certification reports *are* the audit
record. They are therefore **not back-annotated** when a finding is later fixed. Four certification
rounds established that annotating them row-by-row actively causes damage: it produced a factually
inverted sentence, a duplicated table cell, prose rendered as a file path, and — worst — it once
**retro-edited a closed round's finding into a self-defeating paragraph**, destroying the ability to
audit the chain. Those edits have been reverted and the reports restored to their pristine originals.

**Consequences you must account for:**
- A report row may say **STILL OPEN** for something already fixed. **The live surfaces are authoritative**:
  `docs/research/2026-07-25-completeness-register.md` (analysis), `docs/research/full-audit/raw/C-decisions.md`
  (ruling identity + status, `DEC-339`…`DEC-365`), and `docs/plans/SLICE-STATE.md` (the cursor).
- A report may cite a **stale line number**. Line anchors into `SLICE-STATE.md` in particular drift
  constantly (one moved `:1022` → `:1083` during this review alone). Cite by quoted subject, not by line —
  which is exactly what **GR-24 / DEC-362**'s third guard proposes making mechanical.
- Reports may quote filenames or figures that a later round corrected. Where a *factual* claim inside a
  report was wrong, it carries an inline `⚠ CORRECTED` marker; where only its *status* went stale, see below.

### Superseded rows — already fixed, do NOT action
| Report row | Says | Reality |
|---|---|---|
| `L-onhold-inventory.md` `L-71` + its 40-stale-label table row · `H-docs-consistency.md` `H3` + its top-10 row · `C-stdlib-input-fs-clone.md` stale-doc flags | `CLAUDE.md` claims "four vetted, feature-gated exceptions"; **STILL OPEN**; *"present the exact diff for manual application"* | **FIXED in `b3e635e`.** `CLAUDE.md:8-16` now says **14 vetted, feature-gated crates**, enumerates them, and adds a never-restate-a-count-without-re-deriving rule. **Do not re-do this.** |
| `SLICE-STATE`'s *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* claim, wherever cited by line | cited as `:1022` | The claim is real and still stale-as-a-claim, but it now lives at **`:1083`**; `:1022` holds unrelated prose. |
| `P0-block-shadow-byte-identity.md` closing section | `foreach…as` "is retired" | **RETRACTED in place** — both loop forms are live (`for`…`in`, `foreach`…`as`); only crossed forms error. See `K-inline-findings.md` K-1 and register §1 #7. |

## How to read these

- Every claim carries an **evidence grade** (`[Verified: …]` / `[Inferred: …]` / `[Unverified: …]` /
  `[Speculative]`) per global Rule 18. Prefer `[Verified]` rows; treat `[Speculative]` as brainstorm.
- Findings are numbered per file (`A1`, `B11`, `D4`, …) and the register cites those IDs.
- **Two of my own inline conclusions were wrong and are corrected in place** (`K-1` loop syntax, `K-4`
  `var` usage). Both corrections are kept visible on purpose rather than silently edited out, so the
  reasoning failure is auditable: in both cases a zero-result or single-failing probe was over-read.

## Relationship to the three other audits dated the same day

Earlier passes on 2026-07-25 produced `2026-07-25-currency-audit.md`,
`2026-07-25-lsp-completion-audit.md`, and `2026-07-25-plans-divergence-audit.md`. This review
**reconciles against them rather than duplicating them** — the register marks each finding as NEW vs
ALREADY-KNOWN-AND-STILL-OPEN vs NOW-FIXED, with the source audit cited. Notably the UFCS-completion gap
(developer question #3) was **already** on the LSP audit's punch-list as rows #1/#2 (P1) with the same
root cause and the same proposed fix; this run independently reproduced it, which is corroboration, not a
new discovery. The genuinely new editor finding is the **TextMate grammar defect** behind the
"light blue" symptom, which no prior audit covered.

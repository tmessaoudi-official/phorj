# Archived design specs — folded into `UNIFIED-SPEC.md`

These are the **frozen original design documents**. On 2026-07-03 (unification-audit Stage D) the
developer ruled to fold all eighteen into one document — [`../../specs/UNIFIED-SPEC.md`](../../specs/UNIFIED-SPEC.md),
which is the **single spec SSOT** and carries the current, staleness-annotated text. These originals
are retained only for provenance and historical diffing; **do not treat them as current** — where an
original and the unified spec disagree, the unified spec wins (see its "Staleness" note and
Appendix A supersession chains).

Live docs cite the corresponding **section of `UNIFIED-SPEC.md`**, not these files.

| Archived original | Now lives in `UNIFIED-SPEC.md` § |
|---|---|
| `2026-06-15-phorj-language-design.md` | [Founding language design](../../specs/UNIFIED-SPEC.md#founding-language-design) |
| `2026-06-15-ecosystem-roadmap-design.md` | [Ecosystem strategy](../../specs/UNIFIED-SPEC.md#ecosystem-strategy) |
| `2026-06-30-naming-overhaul-design.md` | [Naming overhaul](../../specs/UNIFIED-SPEC.md#naming-overhaul) |
| `2026-07-01-no-wind-namespace-and-language-surface-design.md` | [Nothing in the wind](../../specs/UNIFIED-SPEC.md#nothing-in-the-wind) |
| `2026-07-03-unified-import-and-injected-type-discipline.md` | [Unified import and injected-type discipline](../../specs/UNIFIED-SPEC.md#unified-import-and-injected-type-discipline) |
| `2026-07-01-import-roots-psr4-design.md` | [Import roots and PSR-4 mapping](../../specs/UNIFIED-SPEC.md#import-roots-and-psr-4-mapping) |
| `2026-06-28-public-surface-file-rule-design.md` | [Public-surface file-naming rule](../../specs/UNIFIED-SPEC.md#public-surface-file-naming-rule) |
| `2026-06-28-statics-research-design.md` | [Comprehensive statics](../../specs/UNIFIED-SPEC.md#comprehensive-statics) |
| `2026-06-28-secret-type-design.md` | [Secret type](../../specs/UNIFIED-SPEC.md#secret-type) |
| `2026-07-01-nested-value-index-assign-design.md` | [Nested-value index-assignment](../../specs/UNIFIED-SPEC.md#nested-value-index-assignment) |
| `2026-06-29-m4-stdlib-charter.md` | [Standard library charter](../../specs/UNIFIED-SPEC.md#standard-library-charter) |
| `2026-06-19-core-html-design.md` | [Typed auto-escaping HTML](../../specs/UNIFIED-SPEC.md#typed-auto-escaping-html) |
| `2026-06-27-dependency-policy.md` | [External dependency policy](../../specs/UNIFIED-SPEC.md#external-dependency-policy) |
| `2026-06-19-extension-policy-design.md` | [PHP extension tiers](../../specs/UNIFIED-SPEC.md#php-extension-tiers) |
| `2026-06-21-php-parity-and-beyond.md` | [PHP parity and beyond gap audit](../../specs/UNIFIED-SPEC.md#php-parity-and-beyond-gap-audit) |
| `2026-06-16-m2.5-phorj-build-design.md` | [phg build master design](../../specs/UNIFIED-SPEC.md#phg-build-master-design) |
| `2026-06-16-m2.5-phase2-cross-os-design.md` | [Phase 2 cross-OS builds](../../specs/UNIFIED-SPEC.md#phase-2-cross-os-builds) |
| `2026-06-17-m2.5-phase3a-stub-registry-design.md` | [Phase 3a stub registry](../../specs/UNIFIED-SPEC.md#phase-3a-stub-registry) |
| `2026-07-14-core-db.md` *(folded 2026-07-16)* | [Core.Db — the enhanced-PDO database primitive](../../specs/UNIFIED-SPEC.md#coredatabase--the-enhanced-pdo-database-primitive-dec-208) — the original keeps the full per-slice (A–K) realization notes |
| `2026-07-15-core-mail.md` *(folded 2026-07-16)* | [Core.Mail — native mailer](../../specs/UNIFIED-SPEC.md#coremail--native-mailer-dec-223) |
| `2026-07-22-transpile-into-project.md` *(folded 2026-09-02)* | [Transpile-into-project](../../specs/UNIFIED-SPEC.md#transpile-into-project--file-by-file-adoption-inside-a-live-php-app) — v1 SHIPPED 2026-07-22 (DEC-320/329). The original keeps the five-fork analysis and the TS/Kotlin/Swift adoption survey; the folded section carries the ruled outcome plus the two disclosed deltas (classmap autoloader; the `phpInterop` knob deferred as a PENDING adjudication) |
| `2026-07-26-response-header-injection-guard.md` *(folded 2026-09-02)* | [Response-side header injection guard](../../specs/UNIFIED-SPEC.md#response-side-header-injection-guard) — DEC-363, SHIPPED (`HeaderSafety` in `src/cli/http_prelude.rs`). The original keeps the executed exploit transcript and the five-surface table; **its own status line said "not yet built" and was stale at archival** |
| `2026-07-30-using-scope-guard.md` *(folded 2026-09-02)* | [`using` — the scope guard](../../specs/UNIFIED-SPEC.md#using--the-scope-guard) — DEC-364, SHIPPED 2026-07-31. The original keeps the per-group blast-radius table for all 34 sites and the definition-of-done ledger |
| `2026-07-26-ast-exhaustiveness.md` *(folded 2026-09-02)* | [Mechanical exhaustiveness for `Expr`/`Stmt`/`Pattern`](../../specs/UNIFIED-SPEC.md#mechanical-exhaustiveness-for-exprstmtpattern) — DEC-356: D, C and the Invariant-3 widening SHIPPED 2026-07-30, **the 26-rewriter sweep is still OWED** and the folded section carries that remainder. The original keeps the per-walker miss matrix and the 17→26 decay measurement |
| `2026-07-26-block-scope-shadowing.md` *(folded 2026-09-02)* | [Block-scope shadowing — the redeclaration rule](../../specs/UNIFIED-SPEC.md#block-scope-shadowing--the-redeclaration-rule) — DEC-339, SHIPPED 2026-07-29 (`E-SHADOW-LOCAL`). **The full 23-row case list MOVED into the folded section** and the register's "canonical" citation was repointed there, so this original is provenance only. It also keeps the adjacent lifter-hoist bug found while probing |
| `2026-07-23-entry-kinds-serve-tls.md` *(folded 2026-09-02)* | [Entry kinds, `Http.ServeConfig`, and inbound TLS](../../specs/UNIFIED-SPEC.md#entry-kinds-httpserveconfig-and-inbound-tls) — DEC-331 Slice 3 (D1/D4/D5/D6/D7), SHIPPED 2026-08-29. The original keeps the full per-sub-slice BUILD STATUS ledger (S3.1→S3.5, with its own corrections in place), which is SLICE-STATE-class content and deliberately did not travel |
| `2026-07-23-invoke-tostring.md` *(folded 2026-09-02)* | [`#[Invoke]` and `#[ToString]`](../../specs/UNIFIED-SPEC.md#invoke-and-tostring) — DEC-331 D9: **slice 1 SHIPPED 2026-07-23, slice 1b DEFERRED** (function-type assignability, the PHP `__invoke` emit + multi-invoke shim, and lifting `__invoke` back). The folded section carries that deferral and its consequence — an `#[Invoke]` class does not round-trip transpile→lift |
| `2026-07-23-rich-request.md` *(folded 2026-09-02)* | [Rich `Request`](../../specs/UNIFIED-SPEC.md#rich-request--bags-uploads-eager-vs-lazy-parsing) — DEC-331 D8, SHIPPED 2026-07-24, three-leg green. The original keeps the slice-2 build ledger and the full perf history of the `queryparse` flag through its DEC-338 fix |
| `2026-07-24-visibility-model.md` *(folded 2026-09-02)* | [Visibility and access model](../../specs/UNIFIED-SPEC.md#visibility-and-access-model) — SHIPPED + DEC-268-certified 2026-07-25. The original keeps the DV-1..DV-5 slice ledger and the verified before-state tables; **P-Q-B-1 (overloaded interface-method visibility narrowing) remains open for a ruling** |
| `2026-07-24-wildcard-imports.md` *(folded 2026-09-02)* | [Wildcard and group imports](../../specs/UNIFIED-SPEC.md#wildcard-and-group-imports) — Q-A SHIPPED 2026-07-25. The original keeps the cross-language survey detail, the build re-scope note that corrected its own premise, and the P-Q-A-1..5 follow-up list |
| `2026-07-26-transaction-depth-semantics.md` *(folded 2026-09-02)* | [Transaction depth semantics](../../specs/UNIFIED-SPEC.md#transaction-depth-semantics) — DEC-340: **the rule and both Rust legs SHIPPED 2026-07-29; the PHP leg is mid-slice.** Case-1 step 1 (the SQLSTATE classifier) is built; **step 2 stopped pending a design ruling on the `__phorj_db_stmt` wrapper, and step 3 needs a `decimal` mapping ruling** — both carried into the folded section. The original keeps the live repro transcript and the full step-by-step build narrative |

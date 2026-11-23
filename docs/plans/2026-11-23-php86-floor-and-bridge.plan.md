# PHP 8.6 floor, rulings, defects, then the bridge at full strength (2026-11-23) Plan

> Developer ask, 2026-11-23, after the status report of the same day (PHP 8.6.0 GA 2026-11-19; parity ≈71 %;
> twes-in lifts 1026 of 2556 files, 29 of 313 packages check, none runs): *"target php 8.6 floor first! then we do
> option 1 and 2 [the rulings session and the defect burn]! and improve the lift/transpile to the maximum level with
> formatting and everything!"* The order below was proposed with its challenges and approved the same day
> (DEC-591, DEC-592, DEC-593). It does not amend DEC-578: lift and transpile FEATURE rows wait for step 4, their
> DEFECTS enter step 3 like any other.
>
> Inputs: the census `var/claude/twes-census-2026-11-23/REPORT.md` (gitignored; twes-in at `8a9cabf6f`, phorj
> release binary of `8c6cf04d` plus the R9-AE-4 fix); the PHP 8.6 survey of the same day (not in the repo: 8.6.0 GA,
> UPGRADING mapped to phorj by keyword, so its NOT TRACKED list is a lead, not a census).

## Decisions Log
- [2026-11-23 11:51] AGREED: DEC-591 — programme order: 0 secure R9-AE-4, 1 PHP 8.6 floor (its rulings, then the build), 2 rulings session, 3 defect burn with the lift and transpile defects pulled forward plus the census ratchet, 4 first stable pre-release, 5 lift and transpile features measured by the ratchet. DEC-578 is not amended: bridge features wait for step 4. Alternatives: amend DEC-578 for the lift; 8.6 only and re-plan.
- [2026-11-23 11:51] AGREED: DEC-592 — PHP floor policy: 8.6 is the primary oracle and gate, 8.5 stays a second CI gate until the first stable release, and generated PHP stays 8.5-compatible until then (twes-in declares php >=8.5). Alternatives: 8.6 only, dropping 8.5; 8.6 primary with 8.5 kept permanently.
- [2026-11-23 11:51] AGREED: DEC-593 — the bar for 'lift and transpile to the maximum level' is a census ratchet on a PINNED twes-in commit (files lifted, packages checking, zero drafts that do not parse, format idempotency, transpile round-trips), run on a read-only copy; bumping the pin is a deliberate step. It is the checker of north-star axis 8. Alternatives: a hand-kept percentage target; fixtures only.

## Formal Plan

0. **Secure R9-AE-4** — its fix existed only in the working tree after `target/` was wiped: sabotage, full gate,
   commit, push.
1. **PHP 8.6 floor (DEC-592)** — first the open rulings, asked in Invariant-15 form: DEC-588 (`trim`'s default set)
   and the fix style of PHP86-DEPRECATED-NAMES. Then: the toolchain image's base to `php:8.6-cli-trixie` (tag
   floats the patch), `gate-php86` becomes the primary gate and a `gate-php85` keeps 8.5 until the first stable
   release, generated PHP stays 8.5-compatible until then, `scripts/php-forward-probe.sh` targets the next PHP, the
   microbench baseline re-measured against 8.6 with its JIT (Invariant 18: a new loss is flagged, never hidden),
   every floor statement in the always-loaded files and docs updated in the same change (CLAUDE.md, INVARIANTS,
   UNIFIED-SPEC, KNOWN_ISSUES, `.claude/rules/php-oracle.md`, README, FEATURES).
2. **Rulings session** — every PENDING row that blocks a P0/P1 or a gate (DEC-587's code name, DEC-589, DEC-590,
   DEC-582, DEC-584, and the six P1 entries graded "needs a ruling"), each with its failing program.
3. **Defect burn** — the queue in north-star order; the bridge's defects pulled forward: the two lifted drafts that
   do not parse (a function named `open`; `Map<string, true>`), the ~30 raw parser errors of the census (agent-
   reported, to reproduce), MI-TRANSPILE-COMPOSITION, the twelve natives that compute something else on the PHP
   leg, a transpiled `match` arm after a catch-all, whether a directory lift formats its drafts. Plus the census
   ratchet (DEC-593), the checker of north-star axis 8.
4. **First stable pre-release** (DEC-579): gate green, no open P0/P1; the tag is the developer's instruction.
5. **Bridge features**, each measured by the ratchet: T7 `Any`/`Object` (DEC-335), T6 attributes on members and
   parameters (DEC-552), T3 vendor stubs (DEC-439/551), T12 bare `array` (needs a ruling), T4, T11, T5, T10.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 0 | Secure R9-AE-4 (`fix(transpile): a number with a trailing newline is not a number on the PHP leg`) | S | done | cc443044 | src/transpile/runtime_php.rs tests/differential.rs |
| 1 | PHP 8.6 floor: DEC-588 + deprecated-names rulings, then image, CI, docs, perf rebaseline | M | todo | - | docker/** .github/workflows/** |
| 2 | Rulings session for the PENDING rows blocking P0/P1 | M | todo | - | docs/research/full-audit/raw/C-decisions.md |
| 3 | Defect burn, bridge defects first, plus the census ratchet | L | todo | - | - |
| 4 | First stable pre-release | M | todo | - | - |
| 5 | Bridge features measured by the ratchet | L | todo | - | src/lift/** src/transpile/** |
<!-- /progress-block -->
### Blocked
### Needs input
- Step 1: DEC-588 (options a-d in the register) and the fix style of PHP86-DEPRECATED-NAMES.
### Needs research
- The PHP 8.6 features the survey found untracked (partial function application, readonly property defaults,
  `#[\Override]` on constants, the URI builders, the stream-error and polling APIs): which belong in the gap
  matrix. A lead, graded by keyword search only.
### Fragile
- `/stack`'s image tagged `php8-5` reports PHP 8.6.0 (2026-11-23): never use it as an 8.5 oracle; phorj's own image is 8.5.13.
### Known issues

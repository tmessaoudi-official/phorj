# R9-A-1 SERVE-STATIC-EXPOSURE: a symlink in the docroot never serves `.phg` source or a dotfile (2026-11-20) Plan

> Defect-queue entry R9-A-1 SERVE-STATIC-EXPOSURE (P1 ratified 2026-11-17; SLICE-STATE § DEFECT QUEUE), the first
> open P1 quick win (`var/claude/aggregate-findings/2026-11-16/report.md`, "Quick wins" #1), taken in the away window
> after both open P0s went PENDING (DEC-589, DEC-587). Reproduced on the release binary built from `969f36de`, with a
> real `phg serve <DIR>`: `public/leak.txt -> index.phg` answered `200` with the program's source, and
> `public/env.txt -> .env` answered `200` with the dotfile, while a direct `/index.phg` is `404`. Root cause [Verified:
> read the code]: `try_static` (`src/serve/static_files.rs`) runs the dot and `.phg` guards on the REQUEST path, then
> canonicalizes and checks only that the result is inside the docroot; a symlink whose target is inside the docroot
> passes, and the module promises "`.phg` source bytes are NEVER served, dot-files are invisible".

## Decisions Log
- [2026-11-20 11:59] ASSUMED (review): a symlink resolving to a `.phg` or a dot path answers `404` (the escape guard's and a direct `.phg` request's answer), not a fall-through to the program; and assets under a dot-named directory reached through a symlink stop being served (the quick-win row's fix, applied as written). Basis: advisor() 3C round 1 named both as safe and asked for one to be picked, pinned and recorded.

## Formal Plan

1. **Failing test first** (`src/serve/static_files_tests.rs`, unit, the site built by `resolve_site_dir` under a
   directory whose own path has a dot component): file symlinks to `index.phg`, `Other.PHG`, `.env`; a directory
   symlink to `.git` read as `/g/config`; an escaping symlink (control); and still served: a plain file and a symlink
   to an ordinary file inside the docroot. Body never contains the secret text. A second test keeps the request-path
   guards (`/index.phg` 404, `/.env` falls through).
2. **Fix**: after the containment check, re-run both guards on the canonical path relative to the canonical root.
3. **Sabotage, shipped probe, SSOT, gate**: one compiling mutant per guard (dot, `.phg`, relative-to-root); the real
   `phg serve` before and after on the release binary; module doc, CHANGELOG, queue row out, Conflicts item 1; gate.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Failing test | S | done | `a_symlink_never_serves_source_or_a_dotfile` red: `/leak.txt: HTTP/1.1 200 OK`; `the_request_path_guards_still_hold` green before and after | src/serve/static_files_tests.rs |
| 2 | Guards on the canonical path | S | done | both tests green; file 235 lines | src/serve/static_files.rs |
| 3 | Sabotage, shipped probe, SSOT, gate | S | doing | three compiling mutants red on their own case (no dot guard: `/env.txt 200`; no `.phg` guard: `/leak.txt 200`; absolute path: `/a.txt 404`), restored 2/2; release rebuilt; real `phg serve` after: `/leak.txt 404`, `/env.txt 404`, `/a.txt 200`, program route 200 | CHANGELOG.md docs/plans/SLICE-STATE.md |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- Not covered: a HARD link to `index.phg` inside the docroot is indistinguishable from its target and is still served
  (named in the module doc). The `W-PHG-IN-DOCROOT` startup warning does not name a symlink either.

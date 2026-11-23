# Cloud readiness fixes — sections B and C of the 2026-11-23 cloud audit Plan

> Developer ask, 2026-11-23: work sections B and C of the phorj cloud session's readiness audit (the kit's section A is
> done by the `~/.claude` session in `claude-code-setup`). Order: B0 (B0a, B0b, B0c) first, test-first, reproduced
> by simulating the cloud (a sandboxed HOME with a global `core.hooksPath`, a wrapper `rm` first on PATH), committed
> and pushed as soon as green so a cloud session can push again; then B1, B2 (with or before step 1c of
> `docs/plans/2026-11-23-php86-floor-and-bridge.plan.md`), B4, B5, B6. D1-D6 are the developer's decisions; D3 and
> D6 are benchmarks, run only when the developer says the PC is quiet. Every item is re-verified against the code
> first: the cloud evidence is a hint.

## Decisions Log

## Formal Plan

0. **B0 — the cloud pre-push is red (P0).** Reproduced on the PC 2026-11-23 with a simulated cloud (sandboxed HOME
   with a global `core.hooksPath`, three `GIT_CONFIG_COUNT` entries, an rm-guard stand-in that runs the next `rm` on
   PATH): `scripts/test-cloud-setup.sh` 39/46 (6 hooksPath checks, 1 temp-dir leak); `PHORJ_UID=0 scripts/dk`
   `scripts/test-size-gate.sh` 29/30 (unreadable baseline read by root).
   - B0a: `hooks_path()` reads `--local`; the suite points `GIT_CONFIG_GLOBAL` at its own file holding a foreign
     `core.hooksPath` (so a regression fails on the PC too), sets `GIT_CONFIG_NOSYSTEM=1` and unsets the
     `GIT_CONFIG_COUNT`/`KEY_n`/`VALUE_n`/`PARAMETERS` family.
   - B0b: the closed PATH's tools resolve from `/usr/bin:/bin` only (never the caller's PATH, where a harness wrapper
     may sit first); a tool that does not resolve is a FAIL, not a silent skip; a wrapper `rm` first on the caller's
     PATH stays in the suite as the regression case.
   - B0c: the two unreadable-file cases (`scripts/test-size-gate.sh`, `.claude/hooks/test-lint-on-write.sh`) SKIP
     loudly as root (`chmod 000` does not stop root); the class sweep found no other such case in `scripts/`,
     `tests/`, `.claude/`.
   - Class sweep of closed PATHs built by `command -v`: `scripts/test-cloud-setup.sh` (fixed),
     `.claude/hooks/test-lint-on-write.sh` (cat, bash, python3: same resolver), `scripts/test-docker.sh` (exempt: the
     host contract, not run by pre-push, no `rm` in its set, and `docker` may legitimately live outside `/usr/bin`).
   - Evidence 2026-11-23: red first in the suite itself on the plain PC (42/48, the 6 stated failures); green 48/48
     on the host, the host under the cloud simulation, the image as uid 1000, and the image as root under the
     simulation; sabotage M1 (drop `--local`) reddens the 4 "left unset" checks, M2 (resolve from the caller's PATH)
     the guard fixture and the leak check; the 15 non-cargo pre-push legs all pass as root under the simulation
     (harness `var/claude/cloudsim/`, gitignored). The rm-guard is a model of the harness's, so B0b needs a fresh
     cloud session to prove.
1. **B1** — host-runner `scripts/dk` resolves PHP as pre-push does (option per the developer; the audit leans (a)).
2. **B2** — PHP 8.6 in the cloud, with or before step 1c.
3. **B4** — the "no docker in a cloud session" wording, per D2.
4. **B5** — CLAUDE.md names the certification schedule without the PC slug.
5. **B6** — re-review `.claude/rules/expertise-core.md` and bump its review date.
6. **C** — D1-D6 asked with AskUserQuestion; D3/D6 measured only on the developer's word that the PC is quiet.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 0 | B0 cloud pre-push suites (B0a, B0b, B0c) | M | doing | - | scripts/test-cloud-setup.sh scripts/test-size-gate.sh .claude/hooks/test-lint-on-write.sh |
| 1 | B1 host-runner PHP resolution | S | todo | - | scripts/dk |
| 2 | B2 PHP 8.6 in the cloud | M | todo | - | .claude/cloud-setup.sh scripts/dk |
| 3 | B4 docker-in-the-cloud wording (per D2) | S | todo | - | - |
| 4 | B5 certification-schedule slug | S | todo | - | CLAUDE.md |
| 5 | B6 expertise-core re-review | S | todo | - | .claude/rules/expertise-core.md |
| 6 | C decisions D1-D6 | S | todo | - | - |
<!-- /progress-block -->
### Blocked
### Needs input
- D1-D6 (the developer's); D3 and D6 run only when the developer says the PC is quiet.
### Needs research
### Fragile
- B0b's rm-guard is a model of the hosted harness's guard: a fresh cloud session proves it.
### Known issues

# Cloud readiness fixes — sections B and C of the 2026-11-23 cloud audit Plan

> Developer ask, 2026-11-23: work sections B and C of the phorj cloud session's readiness audit (the kit's section A is
> done by the `~/.claude` session in `claude-code-setup`). Order: B0 (B0a, B0b, B0c) first, test-first, reproduced
> by simulating the cloud (a sandboxed HOME with a global `core.hooksPath`, a wrapper `rm` first on PATH), committed
> and pushed as soon as green so a cloud session can push again; then B1, B2 (with or before step 1c of
> `docs/plans/2026-11-23-php86-floor-and-bridge.plan.md`), B4, B5, B6. D1-D6 are the developer's decisions; D3 and
> D6 are benchmarks, run only when the developer says the PC is quiet. Every item is re-verified against the code
> first: the cloud evidence is a hint.

## Decisions Log
- [2026-11-23 18:47] AGREED: D2 — cloud runner: keep phorj.runner=host; B4 rewrites the 'no docker in a cloud session' wording to say docker may run but the host runner is used (proven, no Docker Hub limit, no cold image build; pre-push is fail-closed). Alternatives: the docker image in the cloud; host with optional docker.
- [2026-11-23 18:47] AGREED: D1 — cloud toolchain scope: zig + cargo-zigbuild, wasm-pack, node 26 and pdo_mysql/pdo_pgsql are declared CI-only in cloud sessions (cloud-setup.sh header + CLAUDE.md, pinned by a test); none is a pre-push need. Alternatives: install them all; node + wasm-pack only.
- [2026-11-23 18:47] AGREED: D4 — remote branch ccr-56056e44-aec471 deleted 2026-11-23 (gh api, no code pushed so no gate): 37/37 commits on master by patch-id, tip vs landing b8f383ba (`land.sh re-sign of the cloud branch`) differs only by the 3 PC-only commits, the re-sign map resolves every old sha. Local backup ref backup/ccr-56056e44-aec471 = c314682c; rollback: git push origin backup/ccr-56056e44-aec471:refs/heads/ccr-56056e44-aec471. Alternatives: archive tag; keep.
- [2026-11-23 18:47] AGREED: D5 — release run 45195353225 (9deb2b99, `feat(checker): no return, break or continue may leave a finally block`) failed on runner capacity, not the release environment's approval: phg-macos-aarch64 ran 0 steps and was cancelled 15 min in, publish skipped. Its failed job was re-run. Alternatives: leave it; make macos-aarch64 non-blocking.
- [2026-11-23 18:48] AGREED: D6 — the cloud pre-push SKIPS the microbench gate with an OWED verdict when CLAUDE_CODE_REMOTE=true (same mechanism as the load skip; a shared VM flipped a different bench on each run: floatmul 1.185->0.867, then mapinsert 1.139->0.855); the PC pre-push stays the perf authority. Owed when the developer says the PC is quiet: measure floatmul + mapinsert on the PC, re-emit the 4 recovered benches, record the 5 new no-baseline benches OWED (Invariant 18). Alternatives: advisory only; keep blocking.
- [2026-11-23 18:48] AGREED: D3 — the step-1c perf rebaseline (8.6 + JIT comparator) is emitted on the PC only, when the developer says it is quiet, through the gate's emit load guard (0.7). Alternatives: a CI runner.
- [2026-11-23 18:48] AGREED: B1 approach (a): scripts/dk's host branch sources scripts/toolchain.env before exec, so every host-runner call resolves PHP as pre-push does; step 1c moves that one resolver to 8.6. Alternatives: update-alternatives; both.
- [2026-11-23 19:09] AGREED: DEC-597 (option A) supersedes the D6 skip: the cloud runs the microbench benches (an output-identity break blocks, a ratio flip is recorded), every leg that cannot run where it is writes a durable OWED entry into SLICE-STATE, cleared only by a real PC pass; a test classifies every pre-push leg. DEC-598 (a): owed legs are verified first on the PC; a confirmed flip is P1, an identity break P0, carried _owed losses unchanged. DEC-599: test-docker.sh into the PC pre-push; DB/mail CI services queued (LIVE-ROUNDTRIPS-NOWHERE, P2 proposed). Alternatives: B (cloud skips, list only); C (rule text only).

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
| 0 | B0 cloud pre-push suites (`test(cloud): the pre-push suites pass in a cloud session again`) | M | done | 51aad5f8 | scripts/test-cloud-setup.sh scripts/test-size-gate.sh .claude/hooks/test-lint-on-write.sh |
| 1 | B1 host-runner PHP resolution (`fix(cloud): dk resolves the PHP oracle on the host runner`) | S | done | 9bf7fc57 | scripts/dk scripts/test-cloud-setup.sh |
| 2 | B2 PHP 8.6 in the cloud | M | todo | - | .claude/cloud-setup.sh scripts/dk |
| 3 | B4 docker-in-the-cloud wording (per D2) | S | todo | - | - |
| 4 | B5 certification-schedule slug | S | todo | - | CLAUDE.md |
| 5 | B6 expertise-core re-review | S | todo | - | .claude/rules/expertise-core.md |
| 6 | C decisions D1-D6 (all ruled 2026-11-23); D6 cloud skip built with B1, replaced by row 7 | S | doing | - | scripts/microbench-gate.sh scripts/test-microbench-gate.sh |
| 7 | DEC-597/598/599 durable OWED legs (`feat(gates): a leg that cannot run where it is becomes a durable OWED entry`), defects-first, legs classified | M | doing | 1d104c1a | scripts/owed-legs.sh scripts/microbench-leg.sh scripts/gate-legs.tsv scripts/git-hooks/pre-push scripts/git-hooks/pre-commit |
<!-- /progress-block -->
### Blocked
### Needs input
- The D6/D3 measurements (floatmul + mapinsert on the PC, re-emit the 4 recovered benches, the 5 new benches OWED): only when the developer says the PC is quiet.
### Needs research
### Fragile
- B0b's rm-guard is a model of the hosted harness's guard: a fresh cloud session proves it.
### Known issues

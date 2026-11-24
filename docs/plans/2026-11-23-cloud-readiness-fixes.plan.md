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
- [2026-11-23 20:29] AGREED: DEC-597 refinement: a cloud session neither writes an OWED entry nor refuses its push; it prints what is owed (an output-identity break still blocks). The developer's push after land.sh (which never pushes) runs the leg on the landed tree and records OWED itself. Alternatives: keep the refusal; keep it with a union-merged separate file.
- [2026-11-23 20:29] AGREED: LIVE-ROUNDTRIPS-NOWHERE graded P2, ratified (a gate that runs nowhere, not wrong output). Alternative: P1.
- [2026-11-23 20:29] AGREED: The PC is quiet now: run the D3 re-baseline (microbench-gate.sh --emit) then microbench-leg.sh --verify. Alternative: wait.
- [2026-11-23 21:25] AGREED: The owed-list flap: a RECOVERED reading opens microbench-baseline only after a re-measure confirms it, as WIN->LOSS flips are confirmed. Alternatives: open only for a bench missing from the baseline; leave as is.
- [2026-11-23 22:43] AGREED: Cloud proof run (a fresh cloud session): fix F1 (test-cloud-setup's 'no php8.6' case read /usr/bin, where B2 installs php8.6, so every cloud gate run before publishing was red), F2 (a phg failure inside the microbench harness was read as infra and printed OWED in the cloud) and F3 (the host-runner gate never handed the forward PHP to test-php-forward-probe), each test-first, one commit each, on the cloud branch; F4-F7 reported only. Certification: advisor() only. Alternatives: F1 only; publish at HEAD with no fix.

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
| 2 | B2 PHP 8.6 in the cloud (`feat(cloud): PHP 8.6 on the host runner, CI-only tools declared`) | M | done | ea126849 | .claude/cloud-setup.sh scripts/dk scripts/test-cloud-setup.sh |
| 3 | B4 docker-in-the-cloud wording (per D2) (`feat(cloud): PHP 8.6 on the host runner, CI-only tools declared`) | S | done | ea126849 | .claude/cloud-setup.sh scripts/dk scripts/test-cloud-setup.sh CLAUDE.md |
| 4 | B5 certification-schedule slug (`feat(cloud): PHP 8.6 on the host runner, CI-only tools declared`) | S | done | ea126849 | CLAUDE.md |
| 5 | B6 expertise-core re-review (every cited anchor re-checked; stale Stmt/Expr counts and the CorpusTally cite fixed) (`feat(cloud): PHP 8.6 on the host runner, CI-only tools declared`) | S | done | ea126849 | .claude/rules/expertise-core.md CLAUDE.md docs/INVARIANTS.md |
| 6 | C decisions D1-D6 (all ruled 2026-11-23); D6 cloud skip built with B1, replaced by row 7; D1 (`feat(cloud): PHP 8.6 on the host runner, CI-only tools declared`) | S | done | ea126849 | scripts/microbench-gate.sh scripts/test-microbench-gate.sh |
| 7 | DEC-597/598/599 durable OWED legs (`feat(gates): a leg that cannot run where it is becomes a durable OWED entry`), defects-first, legs classified | M | done | 1d104c1a d66c0787 15c935f6 (`fix(gates): a cloud run checks output identity through a php source mismatch`) 0ee17780 (`fix(gates): the cloud identity check needs no JIT`) 4e6b1651 (`fix(gates): a harness that cannot run in a cloud session is OWED`) a2d23a40 (`chore(bench): re-emit the G-8 microbench baseline on a quiet PC`) cef9aca1 (`fix(gates): a cloud session prints what is owed and is never refused for it`) af5b2d06 (`fix(gates): a RECOVERED reading is re-measured before it opens microbench-baseline`) | scripts/owed-legs.sh scripts/microbench-leg.sh scripts/gate-legs.tsv scripts/git-hooks/pre-push scripts/git-hooks/pre-commit |
| 8 | Cloud proof run fixes F1-F3 (a fresh cloud session, 2026-11-23): F1 the "no php8.6" suite case read `/usr/bin`; F2 a phg failure in the harness read as infra in the cloud; F3 the forward PHP never reached the host-runner gate | S | done | F1 79fd121e (`test(cloud): the "no php8.6" case runs on a closed PATH`); F2 d0d81a16 (`fix(gates): in a cloud session a phg that fails inside the harness blocks`); F3 in the commit `fix(cloud): the host-runner gate hands the forward PHP to the forward probe` | scripts/test-cloud-setup.sh scripts/microbench.sh scripts/microbench-leg.sh scripts/test-owed-legs.sh scripts/toolchain.env scripts/dk |
<!-- /progress-block -->
### Blocked
### Needs input
- DONE 2026-11-23 (the developer: "the PC is quiet now"): `scripts/microbench-leg.sh --verify` PASSED against the committed baseline (no flips, all output-identical; floatmul 1.051, dbwork 1.076: the cloud audit's floatmul flip does not reproduce on the PC), clearing `microbench`. Then a 7-run quiet-PC `--emit` (load 0.68): namedtuplefield 1.385 and nestedlist 1.003 are WINs again, the five new benches enter `_owed` (coalesceor 0.026, constmap 0.008, mapunion 0.07, throwexpr 0.025, trimascii 0.059: stable across two emits), 13 carried losses (was 10). A first 3-run emit was DISCARDED: it recorded floatmul 0.944 and dbwork 0.957, two WINs demoted to carried losses by one noisy sample (`--emit` has no re-measure); the 7-run emit kept both WINs (`scripts/dk` forwards `MICROBENCH_*`, line 75; measuring took ~6 min vs ~2). mapinsert 1.095 on the PC. `microbench-baseline` reopened at the push of `cef9aca1` (`fix(gates): a cloud session prints what is owed and is never refused for it`): deepjson RECOVERED 0.975 -> 1.269 in one sample; it flaps near parity (see Known issues).
### Needs research
### Fragile
- B0b's rm-guard is a model of the hosted harness's guard: a fresh cloud session proves it.
- Needs a fresh cloud session to prove: B0 (the rm-guard and GIT_CONFIG models), B1, B2 (the forward install inside the 120 s budget, and that sury serves the 8.6 packages), the cloud push printing its OWED lines and going through with no refusal and no SLICE-STATE edit, and the cloud identity check (which comparator the gate resolves: with docker it pulls `php:8.5-cli` from Docker Hub, without it the host php8.5 for identity only; and the leg's wall time there).
### Known issues
- The owed list FLAPS on a near-parity bench: a RECOVERED line (one sample) opens `microbench-baseline`, and the next pass without one clears it, so most pushes rewrite SLICE-STATE (deepjson: 0.967 / 0.975 at emit, 1.269 at a push). A margin alone does not absorb a 30% swing. RULED 2026-11-23 (re-measure to confirm) and BUILT: the leg re-runs the gate on the RECOVERED benches alone (`MICROBENCH_ONLY`, refused by `--emit`) and opens the entry only when the re-run agrees; a re-run that cannot measure leaves it as it was.
- `_baseline_php_digest` is empty since the 2026-11-19 dockerize: `--emit` runs in the toolchain image, which cannot see the docker digest of its own php; the build line (`PHP 8.5.13 … built: Nov 19 2026`) is the remaining provenance. P3, not fixed here.
- `--emit` takes one sample per bench (MICROBENCH_RUNS, default 3) with no re-measure, so a near-parity WIN can be written as a carried loss; use MICROBENCH_RUNS=7 and compare with a `--verify` run before committing an emit.

---
paths:
  - "scripts/git-hooks/**"
  - "Cargo.toml"
  - ".config/nextest.toml"
---

# Gate mechanics — what each git hook runs, and why the test tier is fast

Moved unchanged out of `CLAUDE.md` § "Toolchain & quality gate" on 2026-11-14 (`/rules-split`, plan
`docs/plans/2026-11-14-ground-prep.plan.md`). `CLAUDE.md` keeps the green definition, the full correctness gate
command and a one-line index entry. A new lesson about the hooks, their timings or the build profile belongs HERE,
not in `CLAUDE.md`.

- **Tiered git hooks** (speed, 2026-07-08 — `scripts/git-hooks/{pre-commit,pre-push}`): **pre-commit**
  runs the fast Rust-only tier (`fmt` + `nextest --features jit`, EXCLUDING the two heavy sweeps
  `every_repo_phg_formats_idempotently_and_safely` + `shipped_manual_example_runs_on_both_backends`) —
  ~12s vs ~126s (pre-commit also runs `phg format --check examples selftest` + doc-tests;
  pre-push also runs `fmt --check`, the size-gate, `validate-infra.sh` (DEC-388.4), doc-tests and
  `cargo build --release` — the hooks are the SSOT of their own steps). **pre-push** runs the FULL
  suite (those two included) + `clippy` (`--no-default-features`
  AND `--all-features`) + the PHP-oracle spine check + `microbench-gate`. Test-speed rests on
  `Cargo.toml [profile.dev]` (deps opt-2, workspace opt-1, **deps `debug = false`** — that last one cut
  `target/debug` from 24 GB to 7.4 GB, which is what keeps `target/` warm instead of being cleared for
  disk; phorj's own debuginfo is untouched); `cargo-nextest` is the parallel runner (fallback:
  `cargo test`) and IS installed — a warm full-suite cycle is ~43 s.

- **Claude-config guard and PHP forward probe** (ground-prep row 7, 2026-11-15). `scripts/claude-config-guard.py`
  runs in pre-commit (every commit, under 1 s) and in pre-push, and BLOCKS: C1 the rules files (a scoped file keeps
  valid frontmatter, live globs and its line in `CLAUDE.md`; the always-loaded ones are listed by name in the
  script), C2 every quoted anchor resolves on exactly one line of a tracked file, C3 the line-cite ratchet
  (`scripts/claude-config-baseline.txt`: a count may shrink, never grow — write a new citation as a quoted anchor,
  and lower a row with `--emit-baseline`, which refuses to raise one), C4 the crate and skill counts stated in
  `CLAUDE.md`, `VISION.md`, `README.md` and `SECURITY.md` equal what `Cargo.toml` and `.claude/skills/` give. It
  reads the git index for WHICH files exist and the working tree for their content, so a NEW file must be
  `git add`-ed before the guard sees it, and an unrelated config file left dirty and broken blocks any commit until
  it is fixed or stashed. pre-push also runs the two
  suites (`scripts/test-claude-config-guard.sh`, `scripts/test-php-forward-probe.sh`).
  `scripts/php-forward-probe.sh FILE.phg...` is NOT a gate step: it transpiles each file and lints the PHP on
  every installed build newer than the oracle (found by glob beside it, never named), printing what only the newer
  build reports; lint only, so compile-time deprecations and nothing at run time. A sweep of all examples took
  about 6 minutes under load on 2026-11-15: 0 findings in the 329 that transpile; the other 22 are refused by
  `phg transpile` and were not probed. Section 5 of the probe's suite asserts that `function let` IS reported on a
  machine with a newer PHP, so it turns red in the commit that fixes PHP86-DEPRECATED-NAMES and changes there.
  At write time `.claude/hooks/lint-on-write.sh` relays both, warn-only, for the file just written.

- **Owed legs and the leg table** (DEC-597, DEC-598, DEC-599, 2026-11-23). pre-push runs the microbench through
  `scripts/microbench-leg.sh`, which blocks on an output-identity break and on a phg that fails inside the harness
  everywhere (`scripts/microbench.sh` names it: a defect, never infra), and on a confirmed flip on the PC,
  and records anything it could not judge (a load skip, no comparator, a cloud session's ratios, an unconfirmed
  suspect, a bench missing from the baseline) with `scripts/owed-legs.sh` in SLICE-STATE's OWED GATE LEGS section.
  Only a real PC pass clears an entry (`microbench-leg.sh --verify`). A RECOVERED bench opens `microbench-baseline` only
  when a re-run of the gate on it alone (`MICROBENCH_ONLY`, which `--emit` refuses) agrees: one sample flapped it. pre-commit's `owed-legs.sh check-staged`
  refuses a commit that deletes an OPEN entry or leaves a new one uncommitted. A cloud session writes no entry and
  is never refused for one: the leg prints what is owed there, and the PC push after landing records it (DEC-597). Every pre-push leg has a row in `scripts/gate-legs.tsv` (everywhere / owed / pc-only / info),
  checked both ways by `scripts/test-gate-legs.sh`: a new leg fails pre-push until it is classified. `test-docker.sh`
  is the one pc-only leg: it runs before the dk re-exec, stdin closed.

- **The hooks run in the toolchain image** (2026-11-19, `docs/plans/2026-11-19-dockerize.plan.md`). Each hook's first
  step re-executes it through `scripts/dk`, which mounts the repo at its host path and forwards git's hook variables:
  a pathspec commit hands pre-commit a temporary `GIT_INDEX_FILE` inside `.git/`, which resolves in the container
  only because the path is the same. pre-push's ref list arrives on stdin, which `dk` passes through (no TTY: `-T`).
  The DEC-378 commit lock moved from `$TMPDIR` to the git dir, because every container has its own `/tmp`. A checkout
  with `git config phorj.runner host` (set only by `.claude/cloud-setup.sh`) runs the hook bodies on the host.
  `scripts/test-docker.sh` holds the wrapper's contract (tools, uid, stdin, env, exit status, the loud failure, and
  the host set: docker, bash, git and five coreutils, from a closed PATH). Two traps: this repo has
  `core.fileMode=false`, so a NEW executable is staged 100644 whatever the disk says (`scripts/dk` was, and every CI
  job died "Permission denied"); `scripts/validate-infra.sh` fails an extensionless shebang file not tracked 100755.
  And a hook guard that disagrees with dk's runner check makes the two exec each other forever:
  `scripts/test-cloud-setup.sh` §6b pins the host-runner dispatch (pre-commit; pre-push carries the same guard).

# Dockerize phorj (2026-11-19) Plan

> Developer directive, 2026-11-19: *"I think everything here should be dockerized ! nothng should depend on
> tools/versions already installed ! this project should be 100 % independent and portable easily !"*. Procedure:
> the global `/dockerize-project` skill (inventory both sides, map, build, parity against a host baseline, rewire,
> document). Infrastructure, not a feature row (north star rule 1): ruled to run before the next defect.

## Status
| # | Step | State | Evidence |
|---|------|-------|----------|
| 1 | Image + compose + `scripts/dk` wrapper | done | `scripts/test-docker.sh` 37/37 (red first: no `scripts/dk`) |
| 2 | Parity: host baseline vs the same gates in the container | done | host `de5f2395` pre-push: 3771 passed, 2 skipped; container (third run, all fixes in): 3772 passed (+1 = `83de0a3c`'s test), 2 skipped, every script suite green, microbench past the source check then SKIP for load exactly as on the host; sabotage: PHP hidden = the oracle test FAILS |
| 3 | Hooks run through `scripts/dk` (cloud: explicit host runner) | done | pre-commit + pre-push re-exec via dk; `test-cloud-setup.sh` 41/41 incl. `phorj.runner = host` (sabotaged red) |
| 4 | CI jobs run in the same image (macOS/Windows release native) | done | CI 44649680637 on `148a5717`: gate, jit, perf-gate, cross-build, unsafe-island, oracle-nightly green; playground 44649680642 and release 44649680675 green; `gate-php86` red on `trim_default_set` alone (DEC-588, PENDING), as the local 8.6 image reproduces. The first run (44643020951) built the image on the runner and died "scripts/dk: Permission denied" (exit 126: the 100644 index mode, fixed `1f5ea163`); the image step took 6 min cold, ~1 min from the layer cache |
| 5 | Docs: CLAUDE.md "Run everything in Docker", php-oracle.md, gate-mechanics.md, CONTRIBUTING | done | `claude-config-guard` fails=0 |

## Decisions Log
- [2026-11-19 23:20] AGREED: scope = full: git hooks and every gate run in Docker; the host needs Docker, bash and git only (developer, AskUserQuestion 2026-11-19).
- [2026-11-19 23:20] AGREED: CI runs the same image; macOS/Windows release builds stay on native runners (developer, AskUserQuestion 2026-11-19).
- [2026-11-19 23:20] AGREED: claude.ai cloud sessions keep .claude/cloud-setup.sh as the one declared fallback, installing the versions the Dockerfile names (developer, AskUserQuestion 2026-11-19).
- [2026-11-19 23:21] AGREED: start now, before TRY-UNWRAP-OPERAND (developer, AskUserQuestion 2026-11-19).
- [2026-11-19 23:21] ASSUMED (review): image tag php:8.5-cli-trixie floats the patch, no digest pin - because CLAUDE.md rules the oracle 'never pinned - no script may pin a patch version' (confident pick: a recorded ruling decides). Alternatives: digest pin + bump procedure (reproducible, but a pin the ruling forbids).
- [2026-11-20 01:49] ASSUMED (review): microbench-gate accepts the in-image php as the 'docker php:8.5-cli' comparator (PHORJ_PHP_SOURCE, honoured only when MICROBENCH_PHP_BIN == PHORJ_PHP) — because php.real IS the official image's binary and the benches time themselves with hrtime() inside php. Not yet measured under it on a quiet box (both parity runs skipped for load): that row is OWED, never --emit. Alternatives: keep skipping in the container (loses G-8 coverage locally), or call docker from inside the container (needs the socket mounted).
- [2026-11-20 01:49] ASSUMED (review): Linux release binaries stay on the native ubuntu runner (release.yml), like macOS/Windows — because a trixie-image build links newer glibc symbols (2.39 today, up to 2.41) and would raise the floor for users. The developer agreed to macOS/Windows only. Alternatives: build Linux releases in the image (portable build, higher floor), or a musl static build (no glibc floor, a separate ruling).
- [2026-11-20 01:49] ASSUMED (review): scripts/test-docker.sh stays a manual contract test, not a pre-push step — because pre-push itself runs inside the container, where docker is unavailable. Alternatives: run it from the host side of the pre-push wrapper before the re-exec (adds ~1 min per push).

## Design
- **Image** `docker/Dockerfile`, base `php:${PHP_MINOR}-cli-trixie` (official, NTS, non-debug, PCRE2 10.44) plus
  bcmath, pdo_mysql, pdo_pgsql; rustup with the toolchain `rust-toolchain.toml` names (read at build time, never a
  second version entry) plus llvm-tools; cargo-nextest, cargo-zigbuild, zig, wasm-pack (sha-checked), node; python3,
  git, jq, curl, util-linux (`flock`, `taskset`). Every tool version is an `ARG` in the Dockerfile: the one place.
- **Tag, not digest**: `php:8.5-cli-trixie` floats the patch, like `scripts/toolchain.env`'s `php-8.5.*` glob, because
  CLAUDE.md rules the oracle "never pinned - no script may pin a patch version".
- **Same-path mount**: the repo is mounted at its host path, so `target/` is shared with the host, fingerprints and
  paths match, git's temp-index path (`GIT_INDEX_FILE`) resolves inside the container, and microbench's paths are the
  host's. The container runs as the host uid:gid, so every file it writes stays the developer's.
- **Caches**: cargo registry and git checkouts in the named volume `phorj-cargo` (seeded world-writable from the image).
- **Runner switch**: `scripts/dk` runs a command in the `dev` service; `git config phorj.runner host` (set by
  `cloud-setup.sh`, never inferred from a missing Docker) runs it on the host instead. Docker absent and no switch =
  a loud failure.

## Declared exceptions to "100 %"
- macOS and Windows release binaries: a Linux container cannot build them (native runners).
- `oracle-nightly` (PHP 8.7-dev): no official image; non-gating canary, stays on setup-php.
- claude.ai cloud sessions: the `cloud-setup.sh` fallback (ruled above).
- Editors launch the `phg` the container built; it runs on a Linux host whose glibc is at least the newest symbol
  version the binary links: `GLIBC_2.39` for `target/release/phg` [Verified: `objdump -T`, 2026-11-20; image glibc
  2.41, host 2.43]. A macOS or Windows host runs its editors against a native build instead.

## Known issues
- Parity found five defects, each fixed with a test: dk forwarded the host `CARGO_HOME`; `compose run --build` cost
  ~30 s per call (replaced by an input-hash label, ~2 s); `php -n` could not see the shared bcmath (a `-n`-aware
  wrapper); microbench-gate SKIPped as a "source mismatch" (the image declares `PHORJ_PHP_SOURCE`, cases 15b/15c); the
  forward probe had no newer PHP (the 8.6 binary rides in the 8.5 image) and one of its tests leaned on an unset seam.
- `efa88579` committed `scripts/dk` as 100644: this repo sets `core.fileMode=false`, so staging a new file records it
  non-executable whatever the disk says, and only this machine's disk `+x` hid it. Fixed in the index, and
  `scripts/validate-infra.sh` (both hooks) now fails an extensionless shebang file not tracked 100755.
- Not proven by execution, OWED: the G-8 microbench ratchet has never MEASURED in the container (every pre-push run
  so far skipped it for load, as on the host); the fresh-clone pre-commit passed but its test counts were not kept.
  The 2 skipped tests are the same on both sides by construction: the two `#[ignore]` measurement tests.
- The Actions cache stood at 9.6 GB of GitHub's 10 GB on 2026-11-20 (119 entries: image layers plus one cargo/target
  cache per job). Least-recently-used entries are evicted, so a cold image build (~6 min) can return; fewer
  per-job target caches is the lever if that happens.
- A host `cargo` or rust-analyzer sharing `target/` with the container sees a different registry path, so each side
  rebuilds the other's dependencies. Point rust-analyzer at the container, or give the host its own target dir.
- `rustup-init` is fetched over TLS without a checksum (zig and wasm-pack are sha256-checked).
- NEEDS INPUT (global Rule 5: an agent definition is edited only on the developer's explicit go):
  `.claude/agents/completeness-reviewer.md` lines 58 and 127 still say `source scripts/toolchain.env &&
  PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features` and `cargo build --release`; the proposed text is
  `PHORJ_REQUIRE_PHP=1 scripts/dk cargo nextest run --workspace --all-features` and `scripts/dk cargo build --release`.
- `scripts/test-cloud-setup.sh` §6b holds the host runner's dispatch: with `phorj.runner=host`, dk runs the host tool
  and the hook runs its body without docker (shims; a control case proves the docker shim records calls; both
  guards sabotaged red). A hook guard and dk that disagree exec each other forever, so the fixture runs are bounded.
- `scripts/test-docker.sh` needs docker on the host, so it is a manual contract test (`bash scripts/test-docker.sh`),
  not a pre-push step: pre-push itself runs inside the container.
- `test-lint-on-write.sh` skips one case in the container: its observability check needs the developer's global
  `~/.claude/hooks/log-helpers.sh`, which the project does not (and should not) carry.
- The Claude hook `.claude/hooks/lint-on-write.sh` still uses the host's `jq` to emit its JSON to Claude Code: harness
  plumbing, like bash and git, not a project tool. Its toolchain calls (rustfmt, the config guard, the forward probe)
  go through dk.
- Linux release binaries stay on the native runner too (`release.yml`): built in the trixie image they would link
  against glibc 2.41 (2.39 symbols today), above the ubuntu runner's, raising the floor for users.
- `stub-registry.yml` keeps its own zig version and sha (it publishes, so it is never dispatched from here);
  `scripts/validate-infra.sh` fails when they differ from the Dockerfile's.
- OBSOLETE, kept until the developer agrees: the phpbrew glob in `scripts/toolchain.env` (cloud uses `php8.5`), the
  `/stack/tools/cargo` toolchain and phpbrew builds on this machine.
- The first container build of the whole tree took ~17 min under load ~30 (deps rebuild once: the registry path
  differs from the host's), the first image build ~23 min; afterwards dk costs ~2 s per call.

## Inventory (2026-11-19)
- Host-tool call sites: git hooks (cargo, python3, php via `scripts/toolchain.env`), 20 scripts (cargo, php, node,
  wasm-pack, jq, docker, taskset, python3, llvm-objcopy, curl), `.claude/hooks/lint-on-write.sh` (`phg`, `cargo fmt`),
  tests spawning `php`, `sha256sum`, `rustup`, `rustc`, `cargo-zigbuild`, `openssl`, `git`, `sh`, CI (`setup-php`,
  `rustup`, zig/wasm-pack downloads), editors (`npm`), playground (`wasm-pack`, `node`).
- Docker already provided: none of its own; `scripts/microbench*.sh`, `perf-gate.sh` and `validate-infra.sh` run
  `docker` for the PHP baseline and the compose check.

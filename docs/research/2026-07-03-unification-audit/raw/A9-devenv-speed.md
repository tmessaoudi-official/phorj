# A9 — Dev-environment / dev-loop speed audit (2026-07-03)

Auditor: batch-2 agent A9. Scope: what makes the *developer's* day-to-day loop slow or annoying —
build times, test speed, git hook overhead, tooling friction. NOT Phorj's own runtime speed.
State audited: HEAD `0691228`, clean tree, 8-core Linux box, cargo 1.96.0, cargo-nextest 0.9.138.
All timings measured on this machine during the audit (evidence grades inline).

## Executive summary

The dev loop is in **good shape at the edit-compile scale and bad shape at the gate scale**:

| Loop stage | Measured | Verdict |
|---|---|---|
| Single test (`nextest -E 'test(name)'`) | **0.26 s** | Excellent (beats memory's ~1 s claim) |
| No-op `cargo build` | 0.08 s | Excellent |
| Real edit in a leaf file (`src/dump.rs`) → `cargo build` | 4.5 s | Good |
| Real edit in god-file `src/value.rs` → `cargo build` | 7.6 s | Good |
| Same edit → `cargo build --all-targets` (23 test bins) | +4.2–4.8 s on top | Good |
| Warm no-op `cargo clippy --all-targets` | 0.08 s | Excellent |
| Cold-ish clippy after lib recompile | 19.3 s | Moderate |
| `cargo fmt --check` (84 kLoC) | 0.7 s | Excellent |
| **Full pre-commit hook** (fmt+clippy+nextest+doc) | **112.8 s, 111 s of it = nextest** | **The bottleneck** |
| **Full pre-push hook** (same + PHP oracle) | **196.6 s, 190.7 s = nextest (9 slow)** | The publish tax |

**Single biggest dev-loop time sink: the 111 s test-suite run inside pre-commit, which achieves
only ~2.5× CPU utilization on 8 cores with 16 test threads** (user 4 m 44 s / real 1 m 53 s) —
[Verified: `time bash scripts/git-hooks/pre-commit` output below]. The suite is structurally
serialized somewhere (see §4); fixing the serialization is worth far more than any build-side tweak.

Second headline: **mold is now installed (`/bin/mold`) but NOT wired in — there is no
`.cargo/config.toml`** — [Verified: `which mold` → `/bin/mold`; `ls .cargo/config.toml` → no such
file]. Project memory recorded mold as "pending dev's install"; the install happened, the
activation step didn't. Free link-time win sitting unused (23 test binaries relink on every lib
change; the +4–5 s `--all-targets` delta above is mostly link).

---

## 1. Inner loop: single test + incremental builds — HEALTHY

- `cargo nextest run -E 'test(tokenizes_sample_without_error)'` → **0.26 s wall**
  (`Summary [0.006s] 1 test run: 1 passed, 1660 skipped`) — [Verified: measured]. The memory
  claim of "~1 s" is conservative; reality is 4× better when binaries are warm.
- No-op `cargo build`: 0.08 s (two runs: 0.076 s / 0.078 s) — [Verified: measured].
- Touch-without-content-change of ANY src file: ~1.5 s (incremental fast path) — [Verified].
- **Real content edit** (appended comment, then reverted):
  - leaf `src/dump.rs` → `cargo build` **4.5 s** — [Verified: measured]
  - god-file `src/value.rs` (92.9 KB, the value kernel everything depends on) → **7.6 s** — [Verified]
  - then `cargo build --all-targets` (rebuild+relink all 23 integration-test binaries):
    **4.2 s / 4.8 s** additional (two samples; user time 15–17 s spread across cores) — [Verified]

**Structural note (single-crate layout)**: `src/` is one 84,227-line crate (`find src -name '*.rs' |
xargs wc -l`) + a separate `playground` workspace member. Any src edit recompiles the whole lib —
but at 4.5–7.6 s measured, this is NOT currently a pain point. The "god module forces full
rebuild" hypothesis is true in mechanism but benign in magnitude at today's size — [Verified:
leaf-vs-core delta is only 4.5 s vs 7.6 s]. Revisit if the crate doubles; a lexer/parser/checker
crate split would cap the blast radius but is not worth the churn today.

## 2. mold installed but dormant — FREE WIN NOT TAKEN

- `/bin/mold` exists — [Verified: `which mold`].
- No `.cargo/config.toml` anywhere in the repo — [Verified: `ls` fails].
- Project memory (test-gate-speed topic) says the plan was a gitignored `.cargo/config.toml`
  post-install. The install happened; the config was never created.
- Impact: every lib change relinks the `phg` bin + up to 23 test binaries with the default `ld`.
  The measured `--all-targets` tail (4–5 s) and part of the 111 s gate's rebuild phase are link
  time. Expected saving with mold: typically 30–70 % of link time — [Inferred: standard mold
  results; not yet measured here because the config doesn't exist].
- Fix (2 minutes, gitignored file):
  ```toml
  # .cargo/config.toml
  [target.x86_64-unknown-linux-gnu]
  linker = "clang"
  rustflags = ["-C", "link-arg=-fuse-ld=mold"]
  ```
  (or `linker = "cc"` if clang absent; verify `cc` accepts `-fuse-ld=mold`.)

## 3. The commit gate: 112.8 s, 98 % of it is one nextest run

Measured, full run of `scripts/git-hooks/pre-commit` (Rust-only gate, `PHORJ_SKIP_PHP=1`):

```
[pre-commit] cargo fmt --check              (~0.7 s)
[pre-commit] cargo clippy --all-targets     (~0.1 s warm no-op; 19.3 s if lib was recompiled)
[pre-commit] cargo nextest run (parallel)
     Summary [ 111.055s] 1660 tests run: 1660 passed (4 slow), 1 skipped
[pre-commit] cargo test --doc               (0.00 s — zero doctests)
real  1m52.794s   user  4m44.021s   sys 0m18.865s
```
[Verified: timed directly.]

- The 228 s → ~118 s improvement recorded in memory is real and current: **112.8 s today**.
- **4 tests flagged slow (>20 s)** by nextest's slow-timeout — these dominate the tail.
- **Parallelism efficiency is poor**: user/real ≈ 2.5× on 8 cores with `test-threads = 16`
  (.config/nextest.toml). If the suite ran at even 6× efficiency the same work would finish in
  ~47 s. This matches the prior session's finding and is the #1 leverage point — [Verified:
  user/real ratio from the measured run].

## 4. The serialization lead re-verified: it is NOT a literal global lock

Memory claims "`cmd_run` has a global lock (~60 % serialized)". Re-checked the code:

- `cmd_run`/`cmd_runvm` (src/cli/mod.rs:1113/1133) contain **no mutex** — [Verified: read the code].
- `on_deep_stack` (src/cli/mod.rs:300) spawns a **fresh scoped thread with a 256 MB stack per
  call** — every single `cmd_run`/`cmd_runvm` invocation in every test pays a thread
  spawn + 256 MB stack reservation. Thousands of invocations per suite run — [Verified: read].
- Global state that *does* serialize: `static PROCESS_ARGS: RwLock` (src/native/process.rs:26),
  `static RANDOM_STATE: RwLock<u64>` (src/native/random.rs:35), `static FROZEN: RwLock`
  (src/native/time.rs:27), plus per-suite test mutexes (`ARGS_LOCK` tests/process.rs:17,
  `RNG_LOCK` tests/random.rs:15, `CLOCK_LOCK`, `RNG_LOCK` in src/native/*_tests.rs). RwLock reads
  are concurrent, so these are contention points only under write-heavy programs — [Verified: grep].
- So the ~60 % serialization is real (the 2.5× utilization confirms it) but the mechanism is
  **unconfirmed** — candidate suspects, in order: (a) per-call 256 MB thread spawn overhead
  (mmap + guard page setup under the process's mmap lock — kernel-level serialization when 16
  threads spawn workers simultaneously), (b) RwLock write contention on RANDOM_STATE in
  random-heavy tests, (c) allocator contention on the huge AST/Value churn. Root cause is the
  sibling perf-auditor's dimension — [Unverified: not profiled here; suspects are Inferred].
- **Actionable regardless of root cause**: `on_deep_stack` could keep a lazily-spawned worker
  per OS thread (thread_local) instead of spawn-per-call — removes thousands of 256 MB
  thread creations per suite run — [Speculative: design suggestion, needs the sibling's profile
  to confirm the win].

## 5. Hook redundancy: pre-push re-runs everything pre-commit just proved

Read end-to-end: `scripts/git-hooks/pre-commit` + `scripts/git-hooks/pre-push`
(wired via `git config core.hooksPath scripts/git-hooks` — [Verified: `git config core.hooksPath`]).

- pre-push re-runs `cargo fmt --check` (0.7 s) and `cargo clippy --all-targets` (0.1 s warm) —
  **negligible, and deliberately justified in the hook's own comment** ("a push may carry
  --no-verify commits"). Not worth changing — [Verified: read both hooks].
- pre-push re-runs the **entire Rust test suite** as part of the PHP-oracle run. This is NOT
  cheaply separable: the oracle legs live inside the same test functions (differential harness
  runs interpreter+VM+PHP per case), so "only the PHP delta" isn't a nextest filter away.
  Structural, acceptable — the split-gate design already moved the expensive leg to the
  publish boundary, which is the right shape.
- **Real measured pre-push cost**: see the timing block appended in §6.
- One genuine gap: **neither hook is a no-op when nothing changed since the last green run**.
  A push immediately after a green commit gate re-pays the full Rust suite + oracle. A
  content-hash stamp file (hash of `git rev-parse HEAD^{tree}` written on green, checked on
  entry) would make back-to-back commit→push free. Cheap to build, zero risk (stamp keyed on
  exact tree hash) — [Speculative: proposal].

## 6. Pre-push measured: 196.6 s (3 m 16.6 s)

Full run of `scripts/git-hooks/pre-push` (PHP oracle, 8.5 floor, `PHORJ_REQUIRE_PHP=1`):

```
[pre-push] cargo nextest run (full PHP oracle, 8.5 floor)
     Summary [ 190.714s] 1660 tests run: 1660 passed (9 slow), 1 skipped
real  3m16.636s   user  6m30.485s   sys 0m42.770s
```
[Verified: timed directly (script run standalone with stdin drained — no ref was pushed).]

- The PHP-oracle delta over the Rust-only gate is **~80 s** (190.7 s vs 111.1 s nextest phase).
- **9 tests flagged slow** (>20 s) with the oracle on, vs 4 without — the oracle roughly doubles
  the slow-outlier population.
- CPU utilization is again ~2× (user 6 m 30 s / real 3 m 17 s) — the php-subprocess legs are
  I/O-bound (hence `test-threads = 16`), but even with oversubscription the tail is long.
- Total publish cost for a dev: commit (113 s) + push (197 s) = **~5.2 min of gates around a
  one-commit publish**, plus CI repeating it all again server-side. The split-gate design is
  sound; the absolute numbers are what the serialization fix (§4) should attack.

## 7. target/ bloat: 23 GB, with stale cross-target and scratch dirs

- `du -sh target/` → **23 GB** total; breakdown: `target/debug` **22 GB**, `target/release`
  623 MB, `target/wasm32-unknown-unknown` 409 MB, `x86_64-unknown-linux-musl` 256 MB,
  `x86_64-pc-windows-gnu` 223 MB, `aarch64-unknown-linux-gnu` 4.7 MB — [Verified: du].
- 22 GB of debug artifacts for an 84 kLoC crate is heavy but explainable: 23+ test binaries ×
  full debuginfo × incremental caches. It is all reclaimable churn.
- `/target` is gitignored (line 1 of .gitignore) — [Verified].
- CI caches it sensibly (`Swatinem/rust-cache@v2` in all ci.yml jobs) — [Verified: read ci.yml].
- Stray dirs inside target/: `s2c_php_check/`, `s2d_php_check/` (leftover session-scratch
  projects with `out.php`/`phorge.toml` — note the pre-rename "phorge" spelling, so they predate
  2026-07), and an empty `target/tmp/` — harmless but junk; safe to delete — [Verified: ls].
- Two cheap improvements:
  1. `debug = "line-tables-only"` in `[profile.dev]` — shrinks the 22 GB substantially and
     speeds linking; backtraces keep file:line (full debugger stepping degrades — the project
     has a DAP debugger for *Phorj* programs, and rust-gdb sessions on phg itself appear rare)
     — [Speculative: trade-off is the dev's call].
  2. Periodic `cargo sweep -t 30` or just a documented `rm -rf target/debug` escape hatch
     — [Speculative].

## 8. Formatter / on-save latency: NOT a problem

- `cargo fmt --check` over the whole 84 kLoC crate: **0.7 s** — [Verified: measured]. Fine for
  pre-commit; editors use rust-analyzer's in-process rustfmt anyway.
- Phorj-side: VS Code extension (editors/vscode) wires formatting through `phg lsp`
  (package.json: "…and formatting via the `phg lsp` language server") — [Verified: read].
  Measured `phg format --check`: **7 ms** on the largest `.phg` in the corpus (161 lines,
  examples/guide/pattern-matching.phg) and **19 ms for the entire examples/ corpus** —
  [Verified: timed with the release binary]. Sub-perceptual; on-save formatting is a non-issue.
- Trivial CLI paper-cut: the verb is `format`, not `fmt` — `phg fmt` prints the usage line and
  exits. Given `cargo fmt`/`go fmt` muscle memory, an alias would remove a recurring stumble —
  [Verified: `phg fmt` → usage error; Speculative: the alias suggestion].
- PostToolUse hooks (shellcheck/hadolint/yamllint/shfmt on write) fire only on shell/Docker/YAML
  files, not on the Rust/phg hot path — [Verified: .claude/settings.json coverage per project docs].

## 9. WASM / playground iteration: BLOCKED locally

- `wasm-pack` is **not installed** (`which wasm-pack` → nothing) — [Verified].
- Consequence: no local playground WASM rebuild; iteration relies on the prebuilt
  `playground/web/pkg/` (frontend-only changes OK over `python3 -m http.server`) or on pushing
  to master and letting `.github/workflows/playground.yml` build+deploy. Any change to the
  Rust side of the playground is **untestable locally end-to-end** — a real dev-env gap.
  Fix: `cargo install wasm-pack` (or the curl installer CI uses) — one-time cost.
- CI playground workflow is well-scoped (path-filtered triggers, concurrency-cancel) but
  installs wasm-pack via curl every run (seconds — the installer downloads a prebuilt binary;
  not a compile) and does NOT use rust-cache, so the wasm release build recompiles the world
  every deploy — [Verified: read playground.yml — no cache step at all]. Adding
  `Swatinem/rust-cache@v2` (keyed on the wasm target) would cut several minutes per deploy.

## 10. CI config review (ci.yml, playground.yml, release.yml, stub-registry.yml)

Good hygiene overall: pinned toolchain via rust-toolchain.toml, `Swatinem/rust-cache@v2` on all
ci.yml jobs, concurrency-cancel on ci.yml and playground.yml, path-filtered playground triggers.
Findings, ordered by wasted minutes:

1. **cross-build job: `cargo install --locked cargo-zigbuild` runs BEFORE the cache step**
   (ci.yml step order: Install Zig → Install cargo-zigbuild → Cache cargo registry+target).
   Two problems: (a) rust-cache restores at *its* step, so the install compiles cargo-zigbuild
   from source with a cold registry every single run — multiple minutes; (b) even reordered,
   rust-cache does not cache `~/.cargo/bin`, so the install recompiles regardless. Fix: use
   `taiki-e/install-action@v2` (prebuilt binary, seconds) or `cargo-binstall` —
   [Verified: read ci.yml lines 139–159; the ordering is in the file].
2. **gate job uses `cargo test --workspace`, not nextest** (ci.yml line 66). Locally nextest
   is worth 228 s → 147 s on the full oracle (the .config/nextest.toml comment's own numbers).
   On a 2-core GitHub runner the win is smaller but real (process-per-test isolation +
   oversubscription of the I/O-bound oracle legs). `taiki-e/install-action: nextest` +
   `cargo nextest run --workspace` is a drop-in — [Verified: read; Inferred: magnitude].
3. **`oracle-nightly` is not nightly** — it runs on every push/PR (same `on:` triggers as the
   whole workflow), doing a second full differential run against 8.6-dev. It is
   `continue-on-error` so it never blocks, but it burns a full runner per push. A
   `schedule: cron` trigger (true nightly) would keep the canary and cut per-push runner
   minutes ~25 % — [Verified: read ci.yml — no schedule trigger exists].
4. release.yml triggers only on `release: published` + manual dispatch — fine, no waste —
   [Verified: read `on:` block].
5. stub-registry.yml: not fully audited (release-path only, not per-push) — no per-push cost.

## 11. Minor friction notes

- **Zero doctests but pre-commit runs `cargo test --doc` anyway** — costs 0.0 s today
  ("0 passed; finished in 0.00s"), explicitly future-proofed per the hook comment. Fine.
- **Host shell startup is very slow** (~3–5 s: SDKMAN + nvm + sdk-use banners from the /stack
  shellrc binding) — this taxes every new interactive shell a developer opens, though NOT the
  git hooks (git runs them in a reduced environment that skips the profile) — [Verified:
  observed on every audit shell; hooks unaffected per their own `command -v cargo` guard].
- `.config/nextest.toml` is well-documented (measured numbers in comments) and `slow-timeout
  = "20s"` keeps outliers visible — good practice, keep.
- **Observed during the audit (parallel-agent context): `playground/web/examples.js` turned up
  modified mid-audit** (150 insertions / 125 deletions — the embedded example sources differ in
  formatting from the committed file). Nothing in `tests/*.rs` or my commands touches it
  ([Verified: grep for `examples.js|gen_examples` in tests → no hits]); the likely cause is a
  sibling auditor running `playground/web/gen_examples.py`. The *substance* matters for the
  dev loop though: if regenerating produces a diff, **the committed examples.js is stale
  relative to the Phase-1-reformatted `.phg` corpus** (commit 479dee4 reformatted all 121
  examples; gen_examples.py was evidently not re-run after). Nothing local gates this —
  only the Pages deploy regenerates it — so the live playground and the repo copy drift
  silently. A pre-commit (or CI) staleness check (`gen_examples.py && git diff --exit-code
  playground/web/examples.js`) would close it — [Inferred: diff content matches the
  formatter's canonical style; not confirmed which agent ran the generator].

## Priority list (dev-loop ROI order)

| # | Action | Cost | Expected win | Grade |
|---|---|---|---|---|
| 1 | Root-cause + fix the test-suite serialization (2.5×/8 cores) — sibling's profile; then e.g. persistent `on_deep_stack` workers | M | 111 s gate → plausibly 50–60 s | [Inferred] |
| 2 | Wire mold via `.cargo/config.toml` (already installed!) | 2 min | 30–70 % of link time on every rebuild + gate | [Inferred] |
| 3 | CI: reorder/replace cargo-zigbuild install (binstall/install-action) | 10 min | minutes per cross-build run | [Verified: ordering bug] |
| 4 | CI: make oracle-nightly actually nightly (cron) | 5 min | ~25 % of per-push runner minutes | [Verified: trigger] |
| 5 | CI: nextest in the gate job; rust-cache in playground.yml | 15 min | minutes per push | [Inferred] |
| 6 | Install wasm-pack locally (unblocks playground Rust iteration) | 1 min | removes a hard local gap | [Verified: absent] |
| 7 | Green-tree stamp to skip redundant pre-push after green commit | 30 min | full oracle skipped on back-to-back commit→push | [Speculative] |
| 8 | `debug = "line-tables-only"` + periodic target sweep (23 GB) | 5 min | disk + link speed | [Speculative] |

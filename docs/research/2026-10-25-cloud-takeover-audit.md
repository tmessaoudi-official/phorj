# Cloud takeover audit (2026-10-25)

> Read-only audit of the repo at `d1e26e6` (= `origin/master`, last commit 2026-10-08), made at the start of the
> first cloud session since the 2026-08-18 de-containerization. Question asked by the developer: *is everything
> needed to continue at the same (or better) quality present here, or does the repo point at config that no longer
> exists?* Evidence grades follow the global Rule 18 scale (Verified / Inferred / Speculative).
> This is a research note, not an SSOT: roadmap, decisions and the slice cursor stay in the quartet (Invariant 19);
> anything below that becomes work must be mirrored there first.

## 1. What phorj is (the thesis, in one paragraph)

phorj is a statically-typed language that keeps PHP's ergonomics and ecosystem reach while removing PHP's
runtime-only failure modes. It is a Rust compiler front end (lexer → parser → checker, with all compile-time sugar
expanded out at the single `cli::check_and_expand` chokepoint) feeding **three legs that must agree byte for byte**:
the tree-walking interpreter (the reference oracle, by definition right), the bytecode VM (with a Cranelift JIT tier
hooked at `Op::Call`), and a transpiler to idiomatic PHP 8.5 run under a real `php`. A PHP→phorj **lifter** closes
the loop so existing PHP code (the scout and twes-in corpora) can migrate in, and every language feature must reach
the LSP and both editors in the same change. The engineering bet is that the spine — `tests/differential.rs` over
every example on all legs, plus the fault-parity tables — makes a three-backend language tractable for one
developer, and that real applications ("forcing functions"), not wish-lists, pick what to build next. The process
around it (test-first with sabotage proof, an adjudication rule that keeps every user-visible language decision with
the developer, an SSOT quartet, certification panels) exists because the cost of a silent divergence between legs is
a language whose guarantees are false. [Verified: CLAUDE.md, VISION.md, docs/ARCHITECTURE.md, src/ tree]

## 2. What this cloud box can and cannot run (measured this session)

| check | result |
|---|---|
| toolchain | rustup installed the pinned 1.98.1 + clippy/rustfmt/wasm32 on first `cargo` call [Verified] |
| `cargo test --workspace` with `PHORJ_SKIP_PHP=1` (the pre-commit tier's oracle setting) | **3645 passed, 0 failed, 2 ignored**, 51 s after a 4 min cold compile [Verified] |
| `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (default features) | clean [Verified] |
| PHP oracle | **absent**: only `/usr/bin/php` 8.3.6 *without* bcmath; no `php8.5`, no `/stack/tools/phpbrew`. Adding the ondrej PPA was blocked by this session's safety classifier; the docker CLI is present but the daemon is not running. So the third leg of the spine, `PHORJ_REQUIRE_PHP=1`, pre-push and `--vs-php` benches cannot run here [Verified] |
| `cargo-nextest` | not installed (hooks fall back to `cargo test`) [Verified] |
| git hooks | not wired: `core.hooksPath` unset in a fresh clone [Verified] |
| clone depth | the harness clone was shallow (50 commits); `git fetch --unshallow` restored the full 2202 [Verified] |
| machine shape | 4 cores, 15 GB RAM, ~30 GB writable; `target/` is 8.8 GB after one test build. `.config/nextest.toml` and the microbench "core 7" pinning assume the developer's 8-core box [Verified] |
| git identity | the harness default is `Claude <noreply@anthropic.com>` with SSH commit signing; the repo needs the developer's identity, unsigned (§ 6) [Verified] |

**Consequence:** in this environment, "green" can mean at most the Rust-only legs (interpreter ≡ VM, every unit and
integration test, fmt, clippy). The PHP leg is not "probably fine". It is **unverified** until an 8.5+bcmath oracle exists
here or the developer runs pre-push on the PC. Every cloud commit must say so.

## 3. Everything the repo assumes that is missing here

### 3.1 The global framework (`~/.claude/`) exists in three generations

| generation | where it survives | status |
|---|---|---|
| **G1, container-era copy** (bundle `claude-setup-global-20260722`, phorj-adapted 2026-07-22/27) | this repo's history: `git show e13b7199:scripts/claude-bootstrap/{CLAUDE-global.md,THINKING.md,BLAST-RADIUS.md,install.sh,hooks/*}`, and 14 global skills at `c5cded43^:.claude/skills/` | recoverable, but **adapted the wrong way for today**: it says `AskUserQuestion` is FORBIDDEN and `advisor()` does not exist; both were reversed on 2026-08-18 [Verified] |
| **G2, machine export of 2026-08-30** (`claude-setup-global-20260830-214852`, 299 files) | public repo `tmessaoudi-official/stack`, `claude-setup/claude-setup-global.tar.gz` (sha256 `b14e2b19…4cee` matches its `.sha256`) | **best available source.** Post-de-containerization (AskUserQuestion mandatory, `advisor()` gates). It has 49 skill dirs, and 20 hook scripts plus 2 hook dirs, including `ask-bash-firewall.sh`, `bin/claude-import.sh`, `refs/SKILLS.md`, THINKING, BLAST-RADIUS and a `settings.json.template`. `.claude.json` appears scrubbed (`oauthAccount` null, no token patterns) [Verified] |
| **G3, the live install after 2026-08-30** | only on the developer's PC | **not recoverable from anywhere reachable.** It is referenced but absent from G2 (§ 3.2) [Verified by grep of G2] |

### 3.2 Referenced by phorj but in no recoverable generation (G3-only)

- global CLAUDE.md **§ "Mode — spec or autonomous"** (2026-09-27; CLAUDE.md:113, phg-ask-human). It decides whether a
  question stops the turn. Absent from G2 (0 hits).
- skills **`/sabotage-check`**, **`/long-run`** (both cited by `/phg-ship-slice` steps 9–10) and **`/ci-watch`**
  (dependency-refresh plan). Absent from G2. `/next`, `/progress` and `/certify` only appear inside G2's `/converge`, not as skills.
- **`~/.claude/bin/project-state.sh`** (`--append-decision`, the Decisions-Log stamper; probably also reads the
  `<!-- progress-block v1 -->` tables). Absent from G2.
- the 2026-09-27 hardening of `ask-bash-firewall.sh` (CLAUDE.md:164). G2 has an earlier version.
- `~/.claude/projects/-stack-projects-phorj/certification-schedule` ("asked once") and the reviewer-model /
  override-table files used by `/phg-qa-sweep`.
- **the project memory directory** `~/.claude/projects/-stack-projects-phorj/memory/` (MEMORY.md index, about 100 files).
  Source comments cite named memories as rationale: `op-variant-match-coupling` (`src/chunk/validate.rs:18`,
  `src/cli/pipeline.rs:679`), `cty-tracks-operand-types` (`tests/differential.rs:2603`),
  `core-json-and-injected-types` (`src/cli/preludes.rs:21`), `contextual-var-and-reserved-names`
  (`src/parser/mod.rs:264`), `perf-benchmarking-truth` (`src/jit/tests/boxed.rs:319`), `economize-tokens`,
  and others. G2 ships only the empty `.claude-template/memory/`.

### 3.3 Gitignored state the plans cite as if present (`var/` is absent)

- `var/claude/review-2026-10-07/` (MASTER-REPORT, DECISIONS.md, `raw/6-strictness-neutrality.md` + `probes6/`).
  **DEC-567's six probe programs live only there**, and Invariant 15 requires the failing program inside the question,
  so DEC-567 cannot be asked from this box without re-deriving them.
- `var/claude/review-2026-10-01/MASTER-REVIEW.md` (the PENDING items of the 2026-10-01 parity review).
- `var/phorj-app` (DEC-259 macro-bench app; never delete), `var/claude/{handoff,sleuth,inspections,reports,forge,…}`.

### 3.4 `/stack` paths and sibling projects

- `/stack/tools/cargo/bin`: harmless here (cargo is on PATH; the hooks prepend it only when missing).
- `/stack/tools/phpbrew/php/php-8.5.*`: the oracle. This is the real blocker (§ 2).
- `/stack/projects/{scout,twes-in/api}` (the forcing functions) are public repos `tmessaoudi-official/scout` and
  `…/twes-in`. They can be cloned read-only from a cloud session. Both moved after the last census (pushed 10-17 and
  10-25), and no census records a sibling sha, so the latest census figures are not reproducible as-is [Inferred].
- `/stack/projects/invoiceninja` is a private repo, attachable via `add_repo`. `/stack/projects/phorj-app` is PC-only.

## 4. Drift found while mapping (candidate plan rows — not built here)

All verified this session, and none in the 2026-10-08 remediation plan's text unless noted:

1. **wasm dependency placement**: everything from `corosensei` down, *including `regex`, `fancy-regex`, `ctrlc`*,
   sits under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` (Cargo.toml:168). So
   `playground/Cargo.toml:19`'s advice ("add `features = ["regex"]` … regex compiles to wasm32") cannot work.
2. **6 phantom `size-baseline.txt` rows** naming files that no longer exist (`src/cli/explain.rs`, `src/cli/tests.rs`,
   `src/loader/tests.rs`, `src/parser/items/{decls,types}.rs`, `src/parser/tests/items.rs`). The gate only walks
   existing files, so a new file at one of those paths would silently inherit a 584–1998-line allowance. This overlaps
   remediation row 9's "fails on dead rows".
3. **Stale dependency counts** ("14 crates, 9 default") in README.md:6, VISION.md:60, SECURITY.md:44-46 and the
   Cargo.toml `[features]` comment. The truth is 15 admitted, 10 default (CLAUDE.md, FEATURES, THIRD-PARTY-NOTICES).
   `tests/dependency_claims.rs` only guards the two correct files. Overlaps row 7.
4. **docs/ARCHITECTURE.md** predates the module splits. Its diagram uses `parser.rs`/`checker.rs`/…, there are no rows
   for `jit/`, `lift/`, `lsp/`, `format/`, `pm/`, `bundle/`, `green/`, and it describes the old `cmd_*` dispatch.
   `src/jit/mod.rs`'s doc still says "not yet wired into `phg run`". Overlaps row 7.
5. CLAUDE.md's cited match sites drifted: `vm::exec_op` is `src/vm/exec.rs:12` (cited :9) and `stack_effect` is
   `src/compiler/emit.rs:76` (cited :75). All three remain wildcard-free.
6. `ci.yml` runs default features only. `--all-features`, `--no-default-features`, size/surface/doc-guards and the
   gates' self-tests run only in the local pre-push. Worse, `ci.yml` triggers only on pushes and PRs to `master`.
   **So a cloud commit on a session branch today gets no automated gate at all.** Pre-push cannot run here (no
   oracle) and CI never sees the branch until the developer lands it. Overlaps row 9's CI feature jobs and makes
   them urgent.
7. `http-server-tls` is a Cargo feature with no `src/ext/registry.rs` row and no stated exemption.
8. `editors/vscode` registers no debugger, so `phg debug --dap` has no editor wiring (Invariant 17 surface).

## 5. Recommendation

**Continue, but restore the framework first and make the cloud gate honest.** The repo alone (CLAUDE.md, the four
`phg-*` skills, three reviewer agents, the SSOT quartet, the hooks and gates) carries enough to build correctly. The
global layer is what carries the *workflow* (phases, mode, sabotage, stamping) and the *memory*. Concretely:

1. **Framework**: install G2 (the 2026-08-30 export) into the session's `~/.claude` at session start. Prefer that the
   developer first re-exports the live G3 (`claude-export.sh`) into the stack repo, so the source is current. This keeps
   the 2026-08-18 *global-is-reference* ruling intact: the framework arrives through the environment and is never
   vendored back into phorj. **Import it with the cloud adaptations, not as-is.** The `ask-human-question-guard.sh`
   and `ask-human-gate-*` Stop hooks were ruled OUT for containers (DEC-354/387) because they double-gate against
   the harness's own `stop-hook-reply-gate.py` and `stop-hook-git-check.sh`. And `~/.claude/settings.json` must be
   *merged*, never overwritten: the old `install.sh` did an unconditional `cp -f`, and the harness keeps its own keys
   there (e.g. `advisorModel`).
2. **Memory**: export `~/.claude/projects/-stack-projects-phorj/memory/` alongside, or accept losing it. The
   rationale it held is partly re-derivable from the register, but the named-memory citations in `src/` would dangle.
3. **`var/claude/review-2026-10-07/`**: copy the DEC-567 probes (raw/6 + probes6) somewhere a cloud session can read,
   or accept re-deriving them before asking DEC-567.
4. **Environment setup script** (cloud environment settings → Setup script; runs one directory above the repo):
   ```sh
   add-apt-repository -y ppa:ondrej/php && apt-get update -qq \
     && apt-get install -y php8.5-cli php8.5-bcmath php8.5-mbstring php8.5-sqlite3
   cargo install cargo-nextest --locked
   cd phorj && git config core.hooksPath scripts/git-hooks
   ```
   With `php8.5` on PATH, `scripts/toolchain.env`'s existing fallback resolves it; no repo change is needed. The
   PPA's php8.5 package names could not be checked from this box (the PPA add was blocked) [Unverified]. Check the
   exact extension list against the transpiled output's needs on the first gated run [Inferred].

## 6. Cloud-session git protocol (developer ruling, 2026-10-25)

Recorded in CLAUDE.md § Git autonomy in the same change as this note. Cloud work goes to the session's own branch
(never `master`). Commits carry the developer's name and email, are unsigned, and have no trailers of any kind. The
developer re-signs them on the PC and lands them on `master` there.

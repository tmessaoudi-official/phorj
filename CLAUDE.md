# CLAUDE.md — phorj

> This file holds the RULES for how Claude delivers code here — quality, carefulness, gates.
> The language itself (surface, roadmap, milestones, decisions, history) lives in the docs
> files under "Where things live". Boundary test before adding anything: *does Claude need
> this to deliver correct code?* If not, it belongs in docs, not here.

Phorj is a statically-typed, PHP-inspired language implemented in Rust (edition 2021; core is
std-only with four vetted, feature-gated exceptions — `argon2`, `regex`, `ctrlc`, `corosensei` —
per `docs/specs/UNIFIED-SPEC.md` §"External dependency policy"): lexer → parser → type-checker → tree-walking
interpreter (the reference oracle) + bytecode compiler/stack VM + Phorj→PHP transpiler, plus a
PHP→Phorj lifter, LSP, formatter, test runner, and debugger. Single developer, commits direct to
`master`, remote is GitHub (`tmessaoudi-official/phorj`). The binary is `phg`; sources are `.phg`.

## Routing

This sub-project is handled with the global reasoning framework (`~/.claude/CLAUDE.md`). It is
NOT `/stack` infrastructure — never route work here to `global-stack-lead-dev`. The parent
`/stack/CLAUDE.md` is excluded via `/stack/projects/.claude/settings.json` `claudeMdExcludes`.

## Toolchain & quality gate

- `export PATH=/stack/tools/cargo/bin:$PATH`.
- **Green means ALL of:** `cargo test --workspace` + `cargo clippy --all-targets`
  + `cargo fmt --check` + `cargo build --release`, clean. **`jit` is a DEFAULT feature** (developer-ruled
  2026-07-09) — so bare `cargo test`/`build`/`clippy` include the JIT (the `--features jit` still written
  in the hooks/commands below is now a harmless redundant no-op). Also verify the jit-off path still
  compiles: `cargo check --no-default-features`. Run without native codegen via `phg run --no-jit`
  (byte-identical VM fallback, no rebuild). Warnings fail the build (`[lints] warnings = "deny"`);
  `#![deny(unsafe_code)]` on both crate roots — the JIT's audited `unsafe` (confined to `src/jit/`) is the
  sole island; toolchain pinned by `rust-toolchain.toml`.
- **Tiered git hooks** (speed, 2026-07-08 — `scripts/git-hooks/{pre-commit,pre-push}`): **pre-commit**
  runs the fast Rust-only tier (`fmt` + `nextest --features jit`, EXCLUDING the two heavy sweeps
  `every_repo_phg_formats_idempotently_and_safely` + `shipped_manual_example_runs_on_both_backends`) —
  ~12s vs ~126s. **pre-push** runs the FULL suite (those two included) + `clippy` (`--no-default-features`
  AND `--all-features`) + the PHP-oracle spine check + `microbench-gate`. Test-speed rests on
  `Cargo.toml [profile.dev]` (deps opt-2, workspace opt-1); `cargo-nextest` is the parallel runner
  (fallback: `cargo test`).
- **Full correctness gate — ALL-FEATURES (developer-ruled 2026-07-16)** (before claiming any feature
  done, and always before a push):
  `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features`
  + `cargo clippy --all-targets --all-features` + `cargo clippy --all-targets --no-default-features`
  + `cargo fmt --check` + `cargo build --release`. **`--all-features` is mandatory**: the non-default
  features (`http-client`, `mail`, `database-postgres`, `database-mysql`) are otherwise NEVER compiled/linted/tested
  by the gate — the `--features jit`-only gate hid real clippy lints in those files (DEC-264 build).
  The live DB/mail/http round-trips self-skip when their `PHORJ_*_TEST_DSN`/server env is absent
  (skip-loud). The oracle php path lives in `scripts/toolchain.env` (the single editable knob; bump it
  there when the stack php version changes). With `PHORJ_REQUIRE_PHP=1` a missing `php` FAILS the oracle (never skips).
  Transpile floor = **PHP 8.5** (currently `php-8.5.8`); the bare `php` on PATH is 8.6-dev and too
  permissive — never gate against it (CI runs it only as a non-gating canary).
- **Perf:** `phg benchmark <file>` (median-of-N, output-identity gated) for before/after numbers;
  CI regression gate: `scripts/perf-gate.sh`.
- **After each shipped feature:** `cargo build --release` and report the binary path
  (`target/release/phg`) — standing developer rule.

## Certification ladder (DEC-268, 2026-07-16 — governs every 3C/6C gate in this project)

**MAXIMAL tier, all task sizes.** Every 3C pre-work and every 6C pre-completion gate = a
**3-lens fresh-context reviewer PANEL** (correctness+regression / security+safety-promises /
completeness+blast-radius), each lens adversarial and **evidence-based** (the reviewer reads the
actual diff/tests/specs itself — never certify from the author's narrative). **TWO consecutive
fully-clean rounds** required; any finding → fix → the clean counter resets; cap 5 rounds →
ask-human, never silently proceed. Availability chain: `advisor()` if it activates → read-only
reviewer subagents → 3 distinct-lens self-passes + mandatory disclosure. The quality gate above
is always the floor, never the certification.

## Git autonomy (overrides global Rule 10 — authorized by the developer, 2026-06-16)

Autonomous `git add` + `git commit` are **authorized**: stage and commit ready work without
asking, when the quality gate above is green. Limits:
- **Authorized:** `git add`, `git commit` — descriptive messages, `feat:`/`fix:`/`docs:`/`test:`
  prefixes matching history; no `Co-Authored-By` line.
- **NOT authorized without an explicit request:** `git push` (force-push stays denied globally).
- Commit only green, self-contained changes — never a broken build or red tests.
- If the safety classifier blocks a `git commit`, present the exact command for manual execution;
  do not retry or bypass.

## Delivery invariants (the rules — details in `docs/INVARIANTS.md`)

1. **Byte-identity spine.** `phg run` ≡ `phg runvm` ≡ transpiled PHP under a real `php` —
   identical stdout AND identical failure behaviour, for every program and every example.
   Enforced by `tests/differential.rs` (globs `examples/**/*.phg`, project-aware). Nothing is
   "done" until the full correctness gate above has run green. The ONE disclosed exception:
   concurrency (see rule 14 — its PHP leg is excluded, never silently degraded).
2. **The interpreter is the reference oracle.** When backends disagree, the interpreter is right
   by definition; validate the VM against it, never the reverse.
3. **A new `Op` variant extends three exhaustive matches in the same commit:** `vm::exec_op`
   (`src/vm/exec.rs`), `BytecodeProgram::validate` (`src/chunk.rs`), `compiler::stack_effect`
   (`src/compiler/mod.rs`). All three are wildcard-free — never reintroduce a `_` arm.
4. **Value kernels are single-sourced** in `src/value.rs` (checked int/float arithmetic,
   `compare_ord`, canonical fault strings). Never re-inline them in a backend; fault bodies are
   parity-affecting.
5. **Compile-time-only sugar is expanded OUT of the AST before any backend** (type aliases,
   generics erasure, html — all via the single `cli::check_and_expand` chokepoint). New sugar
   follows the same discipline: backends and the PHP output must never see it.
6. **Reified operands thread ALL vm-compile paths.** Anything that compiles for the VM
   (playground runvm, `disassemble`, `benchmark`, …) must go through
   `check_and_expand_reified` + `compile_with`, never plain `compile` — a miss hides run≠runvm
   off the differential's CLI path.
7. **CTy-operand trap (MUST-CHECK).** Un-rejecting an expression form, or adding one whose result
   can be an arithmetic operand, requires the compiler's `CTy` resolver to type it — and a
   differential case shaped `expr + 1`. Otherwise the VM rejects what the interpreter accepts.
8. **Mid-expression scratch slots (MUST-CHECK).** Ops that stash a receiver (`??`/`?.`/`!`-unwrap
   family) must use `self.height - 1`, not `locals.len() - 1`; any new such construct needs a
   differential case with TWO of them in one expression.
9. **Examples ship with features** (developer rule, definition-of-done): every shipped feature
   lands, in the same change, a runnable example under `examples/` (auto-gated by the
   differential glob) + an `examples/README.md` entry. CLI/tooling features get a walkthrough
   README + a small companion `.phg`. Faults can't be runnable examples — capture them in a
   README instead.
10. **Determinism.** `run`/`check`/`transpile` never touch the network (`phg vendor` is the only
    network command); examples use only deterministic inputs; any user-facing list derived from
    `HashMap`/`HashSet` iteration is sorted before rendering.
11. **No perf change without a measured before/after** from `phg benchmark` (and no perf claim
    above [Inferred] without one).
12. **Naming in code Claude writes:** packages/types/type-params PascalCase (`package Main;`,
    `Core.` reserved); functions/natives camelCase (`Output.printLine`); keyword `function`
    (never `fn`), return types `: T`, mandatory `new` for construction, explicit `this.field`.
    The naming SSOT is `docs/specs/UNIFIED-SPEC.md` §"Naming overhaul".
13. **File-size anti-regrowth** (ratified 2026-07-02; AMENDED 2026-07-16, DEC-262): **soft cap
    300 lines / hard cap 500** per source file — "everything organized/structured/decoupled into
    clear many files". Split-as-you-go is the DEFAULT: a feature that would push a file past the
    soft cap STARTS by splitting it. Split by cohesion into `foo/mod.rs` + sub-files (M-Decomp
    pattern, `pub(super)` for moved methods) — never by line count alone; genuinely-cohesive
    exhaustive-match units comply via index/dispatcher patterns. Applies to new code immediately,
    to existing files as M-Decomp reaches them.
14. **THE LADDER RULE** (ratified 2026-07-02 — governs every feature with no PHP analog).
    When a feature has no faithful idiomatic PHP mapping, SURFACE it to the developer with a
    ladder analysis — never decide alone. Ladder: (1) faithful idiomatic PHP exists → transpile;
    (2) no faithful mapping → native-only: `E-TRANSPILE-<FEATURE>` hard error on transpile,
    differential-harness quarantine, and a disclosure paragraph wherever byte-identity is
    claimed; (3) silent semantic downgrade: FORBIDDEN. Every exclusion is a tracked, tested,
    register-recorded artifact. (First application: concurrency — hard error +
    explicit `--sequential-concurrency` opt-in with warning.)
15. **ADJUDICATION RULE** (ratified 2026-07-02 — governs design decisions). User-visible
    language/design decisions are the developer's, made interactively — an autonomous session
    records them as PENDING questions, never rules on them. Every design question ships with a
    minimal current-syntax failing program embedded IN the question text and the after-state in
    per-option previews (prose outside the question dialog is invisible to the developer while
    answering). Recommended option first, with the why.

16. **CROSS-LANGUAGE SCAN + BYTE-IDENTITY-IS-A-TOOL** (META-7, ratified 2026-07-16). Before
    designing anything meant to beat PHP, survey how other languages (Rust/Kotlin/Swift/TS/Go/C#…)
    solved it. Byte-identity is NOT the priority ordering: emitting a `__phorj_*` helper to keep
    the PHP leg identical is always an acceptable tool — but the trade is ALWAYS surfaced with an
    explanation and ruled by the developer, never self-decided.
17. **Always-current surfaces** (ratified 2026-07-16): `phg check` ≡ LSP diagnostics (same
    pipeline, never diverge — DEC-252); **transpile AND lift updated in the same change** as every
    language/stdlib feature (a feature that runs but doesn't transpile/lift, or vice versa, is not
    done); editors both-same-change (DEC-181) unchanged.
18. **Perf-bench doctrine** (DEC-259, ratified 2026-07-16): everything with a PHP equivalent is
    benched against it (I/O modules via fixtures — no blanket carve-out); real-application MACRO
    benches (whole programs/pipelines) join the suite; `var/phorj-app` (gitignored) is the
    developer's live real-world comparison app — never propose deleting it. WIN-OR-FLAG applies
    to all of it.

## Where things live (pointers — read these instead of duplicating them here)

- **THE ROADMAP (single source of truth):** `docs/plans/MASTER-PLAN.md` — waves 0–6, stdlib
  charter, percentage ledger, rejected-with-reasons appendix. Read it before starting any work.
- **Correctness invariants (detail):** `docs/INVARIANTS.md` — read before touching backends,
  value kernels, or the `Op` set.
- **Architecture / module map:** `docs/ARCHITECTURE.md`.
- **Language surface:** `FEATURES.md` + `examples/README.md` (living showcase);
  frozen designs in `docs/specs/`.
- **Decisions:** the decision register `docs/research/full-audit/raw/C-decisions.md` (canonical —
  all DEC rows + supersession chains, DEC-267/META-7 as of the 2026-07-16 audit) + `## Decisions
  Log` sections in living `docs/plans/*.plan.md`.
- **Completion status:** `docs/MILESTONES.md`; per-change detail in `CHANGELOG.md`; the parity %
  model in `docs/research/full-audit/raw/M-gap-matrix.md` §4 (recompute at every milestone close).
- **History (chronological narrative):** `docs/HISTORY.md`.
- **Known limitations / deferred work:** `KNOWN_ISSUES.md`.
- **Session-level gotchas:** auto-memory index (`MEMORY.md` in the project memory dir).

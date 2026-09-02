# CLAUDE.md — phorj

> This file holds the RULES for how Claude delivers code here — quality, carefulness, gates.
> The language itself (surface, roadmap, milestones, decisions, history) lives in the docs
> files under "Where things live". Boundary test before adding anything: *does Claude need
> this to deliver correct code?* If not, it belongs in docs, not here.

Phorj is a statically-typed, PHP-inspired language implemented in Rust (edition 2021; the core
pipeline stays std-only, and **15 vetted, feature-gated crates** are admitted across the policy's
approved domains — crypto `argon2`, `regex` + `fancy-regex` (DEC-461), signals `ctrlc`, stackful coroutines `corosensei`,
graphemes `unicode-segmentation`, TLS `rustls`+`webpki-roots`, SQL `rusqlite`/`postgres`/`mysql`,
mail `lettre`, native codegen `cranelift`+`cranelift-jit`+`cranelift-module`. **`Cargo.toml` and
`docs/specs/UNIFIED-SPEC.md` §"External dependency policy" are the SSOT — never restate a count here
without re-deriving it from `Cargo.toml`;** the previous "four exceptions" wording had drifted to a
~3× understatement, and the policy section itself warns that understated dependency claims "must not
be repeated"): lexer → parser → type-checker → tree-walking
interpreter (the reference oracle) + bytecode compiler/stack VM + Phorj→PHP transpiler, plus a
PHP→Phorj lifter, LSP, formatter, test runner, and debugger. Single developer, commits direct to
`master`, remote is GitHub (`tmessaoudi-official/phorj`). The binary is `phg`; sources are `.phg`.

## Questions — `AskUserQuestion`, sparingly

Questions to the developer use the **`AskUserQuestion` tool**, per the global framework: options with
the recommended one FIRST (labelled, with its reason) and a visible *"none of these / challenge the
premise"* escape. Invariant 15's ADJUDICATION RULE governs a question's *shape* — its five parts,
after-states-inside-options, and the DEC-row discipline are unchanged. Protocol details:
`.claude/skills/phg-ask-human/SKILL.md`.

> The container-era plain-text protocol and the `❓`/`⏹` end-of-reply markers (developer-ruled
> 2026-07-30) are **RETIRED** (2026-08-18). They existed because `AskUserQuestion` silently failed
> in the dead cloud container; on this machine it works, `askUserQuestionTimeout` is `"never"`
> globally, and the marker's rationale (a prose question being indistinguishable from a pause) dies
> with the prose protocol.

## Routing

This sub-project is handled with the global reasoning framework (`~/.claude/CLAUDE.md`) — the
developer's own persistent install; this repo never writes it (the container-era
`scripts/claude-bootstrap/` reinstaller was removed 2026-08-18). It is
NOT `/stack` infrastructure, and there is **no orchestrator agent to route to** — the
`global-stack-lead-dev` agent was deleted 2026-08-19 at the developer's request; never recreate it,
and never route work here (or in `/stack`) to it. Work is done directly in this conversation; the
read-only reviewer agents in `.claude/agents/` are unaffected. The parent
`/stack/CLAUDE.md` is excluded via `/stack/projects/.claude/settings.json` `claudeMdExcludes`.

The repo carries exactly THREE skills, all repo-specific by name and content (global-is-reference
ruling, 2026-08-18 — a repo may not duplicate anything that exists in `~/.claude/`):
`/phg-ask-human` (the question protocol with this repo's extra rules), `/phg-lenses` (the mandatory
review dimensions + sleuth lens K), and `/phg-qa-sweep` (end-to-end QA on the shipped `phg`
binary). Every other skill — `/sweep`, `/sleuth`, `/inspect`, `/gaps`, `/forge`, `/cross-check`,
`/converge`, `/pre-commit`, `/aggregate-findings`, `/handoff`, `/retrospective`,
`/expanding-context` — comes from the developer's global install. **Before running ANY of those
global review skills here, load `/phg-lenses` first**: it carries the phorj invariants-as-dimensions,
lens K and the repo conventions that the deleted repo-local copies used to enforce. Reviewer agents
stay in `.claude/agents/`.

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
  ~12s vs ~126s (pre-commit also runs `phg format --check examples selftest` + doc-tests;
  pre-push also runs `fmt --check`, the size-gate, `validate-infra.sh` (DEC-388.4), doc-tests and
  `cargo build --release` — the hooks are the SSOT of their own steps). **pre-push** runs the FULL
  suite (those two included) + `clippy` (`--no-default-features`
  AND `--all-features`) + the PHP-oracle spine check + `microbench-gate`. Test-speed rests on
  `Cargo.toml [profile.dev]` (deps opt-2, workspace opt-1, **deps `debug = false`** — that last one cut
  `target/debug` from 24 GB to 7.4 GB, which is what keeps `target/` warm instead of being cleared for
  disk; phorj's own debuginfo is untouched); `cargo-nextest` is the parallel runner (fallback:
  `cargo test`) and IS installed — a warm full-suite cycle is ~43 s.
- **Full correctness gate — ALL-FEATURES (developer-ruled 2026-07-16)** (before claiming any feature
  done, and always before a push):
  `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features`
  + `cargo clippy --all-targets --all-features` + `cargo clippy --all-targets --no-default-features`
  + `cargo fmt --check` + `cargo build --release`. **`--all-features` is mandatory**: the non-default
  features (`http-client`, `mail`, `database-postgres`, `database-mysql`) are otherwise NEVER compiled/linted/tested
  by the gate — the `--features jit`-only gate hid real clippy lints in those files (DEC-264 build).
  The live DB/mail/http round-trips self-skip when their `PHORJ_*_TEST_DSN`/server env is absent
  (skip-loud). **Locally** the oracle is resolved by `scripts/toolchain.env` and nothing else — it globs
  `php-8.5.*` newest-first, requires `bcmath` (probing with a DRAINING pipe: `grep -q` closes the pipe
  early, php dies with SIGPIPE, and under `pipefail` a valid oracle is rejected ~13% of the time —
  measured, DEC-456), and capability-checks even an inherited `PHORJ_PHP`, so a stale export from a
  long-lived shell is announced and ignored rather than trusted. **CI does NOT source it** — the
  workflows set only `PHORJ_REQUIRE_PHP=1` and rely on the test-side `PHORJ_PHP`-or-`php` fallback
  present in 9 test files, with `setup-php` supplying an 8.5+bcmath build; that is correct today but
  is a SECOND resolution path, so a change here is not automatically a change there. No script may pin
  a patch version — `scripts/validate-infra.sh` enforces that over every tracked shell script and
  workflow (a variable-built path is a resolver, not a pin, and is deliberately not flagged), and
  `scripts/test-validate-infra.sh` (run by pre-push) keeps the check itself from going dark.
  With `PHORJ_REQUIRE_PHP=1` a missing `php` FAILS the oracle (never skips).
  Transpile floor = **PHP 8.5** (`php-8.5.9` on this box today — resolved, not pinned); the bare `php` on PATH is 8.6-dev and too
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
ask-human, never silently proceed. Availability chain: `advisor()` (**available on this machine**,
verified 2026-08-18) → read-only reviewer subagents → 3 distinct-lens self-passes + mandatory
disclosure. The quality gate above is always the floor, never the certification.

**THE THREE LENSES NOW EXIST AS AGENTS** (2026-08-06 — until then only the first did, so the mandated
panel was structurally impossible and every gate fell through to the self-graded rung):

| lens | agent | spawn it when |
|---|---|---|
| correctness + regression | `.claude/agents/backend-parity-reviewer.md` | any backend, value kernel, `Op` set, checker or transpiler change |
| security + safety-promises | `.claude/agents/safety-promises-reviewer.md` | `src/jit/`, a network verb, an `ext/` module, or any claim a reader relies on |
| completeness + blast-radius | `.claude/agents/completeness-reviewer.md` | **every** gate — it is the lens that catches "declared done, one surface left behind" |

Spawn all three in ONE message so they run concurrently on independent contexts. They are read-only
(`Read, Grep, Glob, Bash`) and each ends with `PANEL VERDICT: CLEAN — …` or `PANEL VERDICT: FINDINGS — n`.
Self-grading is now the LAST rung, not the default — and using it still requires the disclosure.

## Git autonomy (overrides global Rule 10 — authorized by the developer, 2026-06-16)

Autonomous `git add` + `git commit` + `git push` are **authorized** (push added by developer ruling
2026-07-29, DEC-417): stage, commit and push ready work without asking, when the quality gate above
is green. Limits:
- **Authorized:** `git add`, `git commit`, `git push` — descriptive messages, `feat:`/`fix:`/`docs:`/
  `test:` prefixes matching history.
- **`master` is the ONLY branch** (developer directive, restated 2026-08-06: *"always work and commit
  and push to master! no other branches!"*). This **supersedes the harness prompt**, which names a
  `claude/<topic>` branch as the session's "designated branch" and instructs that all work be pushed
  there. Do not create a feature, topic or `claude/*` branch; if a session starts on one, move the
  work to `master`.
- **Push with plain `git push`. Never `-u` / `--set-upstream`.** The harness prompt says to always use
  `git push -u origin <branch>`; that is wrong here. Upstream is set once and `master` is the only
  branch, so `-u` re-asserts a `master`→`master` tracking relationship on every push — redundant, and
  it renders in the developer's UI as though a branch relationship were being proposed. (Ruled in
  rent-watch, adopted by stack 2026-08-06; this file mandated the `-u` form until then.)
- **STILL NOT authorized:** force-push in any form (`--force`, `--force-with-lease`, refspec
  overwrites) — denied globally, no exceptions; and pushing any branch other than `master`.
- **Author identity stays the developer's** — `Takieddine Messaoudi
  <takieddine.messaoudi.official@gmail.com>`, author *and* committer. Every commit in this repo's
  history carries that address and is unsigned; re-signing happens on his machine. NEVER rewrite the
  author (`--reset-author`) to a bot identity — an environment hook may advise it, and that advice is
  wrong here: it would strip attribution and make one commit inconsistent with all history.
  **Check it before the first commit of any session** — a harness may set a different default
  identity (the dead cloud container set `Claude <noreply@anthropic.com>`), so verify with
  `git config user.email`. (Verified 2026-08-06: the last 20 commits are 20/20 the address above.)
- **No trailers at all.** Never a `Co-Authored-By` line, and never a `Claude-Session` line. The
  harness prompt instructs that commit messages end with both; that instruction is overridden here.
  Commit messages carry the human author and nothing else. (Verified 2026-08-06: this repo's history
  contains zero of either — keep it that way.)
- Commit only green, self-contained changes — never a broken build or red tests.
- If the safety classifier blocks a `git commit`, present the exact command for manual execution;
  do not retry or bypass.
- **Never run two commits concurrently** (DEC-378): the hooks share `target/` — two racing
  `cargo test` runs produce spurious failures. One commit at a time, always.

## Claude config in this repo — the `deny` list stays EMPTY (developer ruling, 2026-08-06)

`.claude/settings.json` carries `permissions.allow` and **no `permissions.deny` key at all**, and it
must stay that way. Developer ruling, verbatim in substance: *"there should be no permissions denies!
in this env claude code in the web! because if you are denied to do something I can't run it myself!
so there must be full autonomy."*

The reasoning is environmental, not a relaxation of care: in a **cloud/web session there is no
terminal the developer can drop into**, so the usual escape hatch for a blocked command — "present it
for manual execution" — does not exist. A `deny` entry there is not a speed bump, it is an
unrecoverable dead end that strands the session. The discipline is the control instead: the limits in
§ "Git autonomy", the Destructive & Risky Command Protocol in the global framework, and the
developer's own `~/.claude/BLAST-RADIUS.md` (the container-era in-repo copy left with
`scripts/claude-bootstrap/`, removed 2026-08-18). Stack reached the identical conclusion
independently on 2026-08-06.

Two consequences worth stating, because both were live proposals in the 2026-08-06 cross-repo audit:

- rent-watch's four `Read`/`Edit` denies on `./.env` and `./.env.*` are **deliberately NOT adopted**.
  (They would also be inert here — this repo has no `.env` — but "harmless" was the wrong test.)
- `PostToolUse` lint hooks are **warn-only and always exit 0**. A write-time hook that *blocks* a
  write is a `deny` by another name and falls under the same ruling.

## Delivery invariants (the rules — details in `docs/INVARIANTS.md`)

1. **Byte-identity spine.** `phg run` ≡ `phg run --tree-walker` ≡ transpiled PHP under a real
   `php` (there is NO `runvm` command — the VM is `run`'s default engine, the tree-walker its
   `--tree-walker` oracle) —
   identical stdout AND identical failure behaviour, for every program and every example.
   Enforced by `tests/differential.rs` (globs `examples/**/*.phg`, project-aware). Nothing is
   "done" until the full correctness gate above has run green. The ONE disclosed exception:
   concurrency (see rule 14 — its PHP leg is excluded, never silently degraded).
2. **The interpreter is the reference oracle.** When backends disagree, the interpreter is right
   by definition; validate the VM against it, never the reverse.
3. **Mechanical exhaustiveness — `Op` AND `Expr`/`Stmt`/`Pattern`** (widened 2026-07-30, DEC-356).
   A **new `Op` variant extends three exhaustive matches in the same commit:** `vm::exec_op`
   (`src/vm/exec.rs:9`), `BytecodeProgram::validate` (`src/chunk/validate.rs:21`),
   `compiler::stack_effect` (`src/compiler/emit.rs:75`). All three are wildcard-free (verified
   2026-07-25) — never reintroduce a `_` arm.
   **The same rule governs `Expr` (37) / `Stmt` (15) / `Pattern` (11) — and `Item` (8) since CD-31,
   2026-09-02:** a rewriter's total walk carries
   NO catch-all, and a *named* one (`other => other`, `leaf => leaf`) is worse than `_` because it reads
   as deliberate and greps as handled. Leaf sets are single-sourced as or-pattern macros in
   `src/ast/leaves.rs` so `rustc` still enforces exhaustiveness; exemptions are recorded as CD rows,
   never silent. This class already panicked the compiler on valid user code (an `html"…"` inside a
   tuple → `unreachable!("html literal not resolved before compilation")`).
4. **Value kernels are single-sourced** in `src/value/` — checked int/float arithmetic + the canonical
   fault consts in `src/value/arith.rs`, `compare_ord` alongside. Never re-inline them in a backend; fault bodies are
   parity-affecting.
5. **Compile-time-only sugar is expanded OUT of the AST before any backend** (type aliases,
   generics erasure, html — all via the single `cli::check_and_expand` chokepoint). New sugar
   follows the same discipline: backends and the PHP output must never see it.
6. **Reified operands thread ALL vm-compile paths.** Anything that compiles for the VM
   (the playground VM pane, `disassemble`, `benchmark`, …) must go through
   `check_and_expand_reified` + `compile_with`, never plain `compile` — a miss hides a VM≠tree-walker
   divergence
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
10. **Determinism.** `run`/`check`/`transpile` never touch the network (the DEC-316 package-manager
    verbs `phg add`/`install`/`update`/`remove`, plus `phg build --target`'s sha256-verified
    cross-compile stub download, are the only network commands; `phg vendor` is
    retired and errors — DEC-282); examples use only deterministic inputs; any user-facing list derived from
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
    register-recorded artifact. (First application: concurrency — the `E-CONCURRENCY-NO-PHP`
    hard error + differential quarantine; DEC-369 deleted the never-shipped
    `--sequential-concurrency` opt-in from this rule.)
15. **ADJUDICATION RULE** (ratified 2026-07-02; question FORM amended 2026-07-27). User-visible
    language/design decisions are the developer's, made interactively — an autonomous session
    records them as PENDING questions, never rules on them. Every design question ships with a
    minimal current-syntax failing program embedded IN the question text and the after-state stated
    INSIDE each option (prose written outside the option list is missed while options are being
    compared). Recommended option first, with the why, and a visible *"none of these / challenge the
    premise"* escape. Questions are delivered via **`AskUserQuestion`** (re-inverted 2026-08-18 —
    the container-era plain-text ban existed because the tool silently failed in the dead cloud
    container; on this machine it works and the global Stop hook requires it), then STOP and wait;
    never assume an answer, never proceed on a default. The protocol is
    `.claude/skills/phg-ask-human/SKILL.md`.

16. **CROSS-LANGUAGE SCAN + BYTE-IDENTITY-IS-A-TOOL** (META-7, ratified 2026-07-16). Before
    designing anything meant to beat PHP, survey how other languages (Rust/Kotlin/Swift/TS/Go/C#…)
    solved it. Byte-identity is NOT the priority ordering: emitting a `__phorj_*` helper to keep
    the PHP leg identical is always an acceptable tool — but the trade is ALWAYS surfaced with an
    explanation and ruled by the developer, never self-decided. Standing rule (DEC-371): PHP's
    lack of a feature is never a reason against building it; the only PHP-shaped question is
    which Invariant-14 ladder case the transpile leg takes.
17. **Always-current surfaces** (ratified 2026-07-16; **LSP/editor bar raised to 100% by developer
    ruling 2026-07-29, DEC-417**): `phg check` ≡ LSP diagnostics (same pipeline, never diverge —
    DEC-252); **transpile AND lift updated in the same change** as every language/stdlib feature (a
    feature that runs but doesn't transpile/lift, or vice versa, is not done); editors
    both-same-change (DEC-181) unchanged.
    **THE 100% RULE: the LSP and both editor integrations must support EVERYTHING we implement —
    no exceptions, no lag.** A language or stdlib feature is NOT done until the LSP surfaces it
    everywhere it could appear (completion, hover, go-to-definition, find-usages, document symbols,
    diagnostics **with the right LSP tags**, signature help) AND both editors (VS Code + the LSP4IJ
    JetBrains path) are updated in the SAME change — including the TextMate/syntax grammars when new
    syntax lands. "The compiler knows it but the editor doesn't" is an incomplete feature, and the
    definition-of-done checklist for every slice carries an explicit LSP+editors row.
18. **Perf-bench doctrine** (DEC-259, ratified 2026-07-16): everything with a PHP equivalent is
    benched against it (I/O modules via fixtures — no blanket carve-out); real-application MACRO
    benches (whole programs/pipelines) join the suite; `var/phorj-app` (gitignored) is the
    developer's live real-world comparison app — never propose deleting it. WIN-OR-FLAG applies
    to all of it. NO-HIDDEN-LOSS (DEC-365): an unmeasurable or failing bench is recorded as an
    OWED verdict, never reported as passed, never re-baselined via `--emit`; a confirmed real
    loss gets fixed — refactor or implement the win — never suppressed.
19. **Plans live in the repo; ZERO divergence from the SSOT** (ratified 2026-07-21; SSOT quartet
    made explicit and MANDATORY by developer ruling 2026-07-29). Every plan
    or spec Claude produces is persisted IN the repo — the out-of-repo plan-mode file
    (`.claude/plans/*`) is an ephemeral scratchpad, NEVER the record of truth (a plan in the repo
    survives any one machine and lands beside the code it governs). **The SSOT quartet — every session MUST read these
    before working and write through them, never around them:**
    - `docs/plans/MASTER-PLAN.md` — the roadmap SSOT (waves, priorities, percentage ledger);
    - `docs/specs/UNIFIED-SPEC.md` — the language/spec SSOT (surface, naming, dependency policy);
    - `docs/plans/SLICE-STATE.md` — the current-slice SSOT (the live cursor: what is being built
      NOW + the queue — a session that starts or finishes a slice updates it in the same change);
    - the decision register `docs/research/full-audit/raw/C-decisions.md` (every DEC row/ruling).
    Any other document that states roadmap, spec, slice, or decision content is a POINTER to
    these, never a second copy. A
    plan approved for build is mirrored into these BEFORE or IN the commit that starts the work; a
    spec ruled-but-not-yet-built is recorded there too (as QUEUED), so a fresh context resumes
    purely from repo state. **No divergent artifact** (extends Invariant 17's unified-docs
    discipline to plans): every roadmap item, decision, and slice-status lives in exactly ONE
    canonical place and everything else points to it — MASTER-PLAN + register + SLICE-STATE are
    kept mutually consistent in the SAME change, never forked into a parallel doc.

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

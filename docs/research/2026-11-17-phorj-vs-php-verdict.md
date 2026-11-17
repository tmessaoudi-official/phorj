# Phorj versus PHP: the verdict, with its flaws (2026-11-17)

> **What this is.** Ground-prep row 10 (`docs/plans/2026-11-14-ground-prep.plan.md`): an expert opinion on where
> Phorj stands against PHP today, on every axis of `.claude/rules/north-star.md` and in every domain, flaws first.
> **A frozen snapshot of HEAD `59210fae`** (docs: ground-prep row 9b closed), release binary built 2026-10-29, no
> `src/` change since. It is an assessment, not a ruling: it grades nothing in the defect queue, answers no open
> question and orders no work. The defect queue (`docs/plans/SLICE-STATE.md`, section DEFECT QUEUE) and the decision
> register stay the only lists of record; this file points at them and never replaces them.
>
> **Author.** The row-10 session (an AI model's assessment, written at the developer's request).

## How to read the grades

- **[Verified: …]** I ran it or read it on 2026-11-17, and say what.
- **[Verified 11-16: …]** reproduced or re-checked by the main session of row 8 or row 9 on the same binary, not
  re-run by me.
- **[Reported 11-16: …]** a hunter agent's finding from those rows that no main session reproduced; it stands at
  the agent's word and grade.
- **[Inferred: …]** follows from evidence named, not directly confirmed.
- **[Unverified: …]** a checkable fact I could not check (mostly PHP-side facts from recall).
- **[Speculative]** design judgement; no fact settles it.

Inputs: the eight local report sets of rows 8, 9 and 9b under `var/claude/` (gitignored: `phg-qa-sweep`,
`cross-check`, `inspect`, `sleuth`, `gaps`, `forge`, `aggregate-findings`, `ratify-assumed`), the checkers the north
star names, and the reference documents. Severity words on row 9 findings ("proposed P0", "proposed P1") are
proposals: the developer ratifies grades (north star, "Open"). Counts of row 9 findings are the plan's reconciled
figures (plan row 9), not the individual reports', which disagree with each other.

Re-run by me today on `target/release/phg`, oracle PHP 8.5.12 (`php -n`, a debug build, used for semantics only):

| Probe | VM and `--no-jit` | Tree-walker | Transpiled PHP |
|---|---|---|---|
| a catch body throws while a `finally` is pending | the `finally` block does not run | runs | runs |
| `Validation.isInt` and `isAlpha` on a string ending in a newline | `false false` | `false false` | `true true` |
| any verb given a non-UTF-8 argument | panic, exit 101 | - | - |

## The verdict

1. **As a language design, Phorj is ahead of PHP.** Static types checked before the program runs, null safety,
   immutability by default, exhaustive `match`, checked exceptions, generics, sealed hierarchies and typed HTML
   remove whole classes of error that PHP finds only when the line executes. [Speculative, resting on verified
   behaviour: see "Language design"]
2. **As an implementation, it is strong in the middle and not yet trustworthy at the edges.** The three native
   legs agree byte for byte on the 264 example programs the differential runs, and transpiled PHP agrees on 261 of
   them (3 are quarantined by ruling; 22 impure programs and 18 database, interop and mail files are outside that
   run). The committed benchmark baseline records more wins than losses against PHP with its JIT. But one core
   control-flow construct behaves differently on the default engine than on the reference oracle, twelve
   standard-library natives compute something else on the PHP leg, and five inputs crash the toolchain. The gate
   caught none of them (3,752 tests passed at the pre-push run of 2026-11-14, plan row 4). Two were already
   recorded as known issues; the rest were first found by the hunters of rows 8 and 9.
3. **As a product someone could choose over PHP today, it is not there.** There is no standard-library reference,
   no tutorial and no migration guide; the language server answers nothing for several everyday constructs; a
   multi-file project cannot be built into a binary; no real application runs on it; it has one developer and no
   known outside user.
4. **By the project's own rule, the goal is met on none of the fifteen axes.** Ten axes have a checker that is
   armed in whole or in part and a bar that is not met; five have no checker at all, and for two of those the bar
   itself is not ruled. The scorecard below derives that count.
5. **The distance is mostly depth, not breadth.** The surface is already wide. What is missing is proof at the
   edges, documentation, editor intelligence and one real user. More language would not move the verdict.

## Scorecard: the fifteen axes

States: **met** · **armed, not met** (a checker exists, the bar is not reached) · **owed** (no checker) · **owed, bar
not ruled** (no checker and no budget to check against). "Against PHP" is my reading of today's position.

| # | Axis | State | Measured today | Against PHP today |
|---|------|-------|----------------|-------------------|
| 1 | Speed | armed, not met | committed baseline: 56 micro-benchmarks, 46 above 1.0, 10 losses carried | per the committed baseline: ahead on most compute shapes, behind on ten; no macro bench |
| 2 | Correctness | armed, not met | corpus green; 41 queue entries, 39 ungraded; row 9 adds 1 proposed P0 and 22 proposed P1 | stronger guarantees by design, weaker trust in practice |
| 3 | Robustness and security | armed in part, not met | five crash inputs and three security classes open; no fuzzing, soak, footgun suite or advisory check | behind on evidence |
| 4 | Learnability and tooling | armed in part, not met | 389 stdlib members: 307 with an example, 219 named in a doc; no reference, tutorial or migration guide | far behind |
| 5 | Code cleanliness | armed, not met | 54 files over the 500-line hard cap, 176 over 300; target architecture unbuilt | not comparable (internal) |
| 6 | Memory | owed | nothing against PHP | unknown; one structural risk (no cycle collector) |
| 7 | Startup and latency | owed, bar not ruled | nothing | unknown |
| 8 | Migration | armed for fixtures, not met | 27 lift fixtures: 26 drafts check, 25 match PHP's output; no corpus census check | no PHP counterpart: this axis is the way out of PHP |
| 9 | Parity completeness | owed | hand-computed 2026-09-25: parity about 71 %, raw floor about 59 % | by definition behind until 100 % |
| 10 | Diagnostics | armed for coded errors, not met | 321 of 325 codes asserted; everyday errors carry no code | ahead in kind, bar unmet |
| 11 | Stability | armed as regression, not met | 84 conformance fixtures; 9 of 16 stable stdlib modules have one | far behind |
| 12 | Deployment | armed in part; size ceiling not ruled | cross-build job green; binary 23.2 MB; projects do not build | ahead for one file, behind for a project |
| 13 | Supply chain and docs truth | armed in part, not met | crate and config guards green; advisory check owed | ahead in kind on dependencies |
| 14 | Web throughput | owed, bar not ruled | nothing | unknown |
| 15 | Real application | owed | none on this machine | behind |

**Count: met 0 · armed in whole or part and not met 10 (axes 1-5, 8, 10-13) · owed with no checker 3 (6, 9, 15) ·
owed with the bar not ruled 2 (7, 14).** Axis 12 also waits on a ruling for its size ceiling.

Evidence for the "measured" column:

- Axis 1 [Inferred: `bench/micro-baseline.json`, last committed 2026-09-28, comparator `docker php:8.5-cli`, PHP
  8.5.10 with JIT]. I did not measure. A `src/` change landed after the baseline (2026-10-29, the runtime-hook and
  JIT work of audit row 4b, whose bench entries are still owed per ratified entry V21), and the row 8 sweep records
  that the microbench gate skipped itself under load on every push of that session. The CI `perf-gate` job, which
  guards the VM against its own regression and not against PHP, is green on HEAD [Verified: `gh run view` on run
  43976850873, six jobs, all success]. Carried losses, by ratio: `namedtuplefield` 0.027, `nestedlist` 0.030,
  `fslines` 0.130, `queryparse` 0.291, `fsforeachline` 0.302, `jsonround` 0.323, `spaceshipsort` 0.487, `strappend`
  0.527, `listcontains` 0.837, `deepjson` 0.972. Two of those are slower than PHP by more than thirty times. Four
  rows (`mapget`, `mapinsert`, `floatloop`, `dbwork`) left the loss list on 2026-09-09 because the comparator
  changed, not because anything was fixed (DEC-516, disclosed in the baseline itself).
- Axis 2 [Verified: queue section headings A 23, B 3, C 4, D 11; two entries carry a grade, both anchors of
  DEC-578] and [Verified 11-16: plan row 9]. The corpus: 264 example programs agree on the VM, `--no-jit` and the
  tree-walker; 261 of them agree with transpiled PHP and 3 are ladder-quarantined [Verified 11-16: QA journey 1].
- Axis 4 and 10 and 11 [Reported 11-16: `/gaps` axis numbers, agent-measured and grep-based]; the 321 of 325
  figure [Verified: `scripts/surface-ratchet.sh` today].
- Axis 5 [Verified: `scripts/size-gate.sh` today, `grandfathered=54 fails=0 warns=176`]; `src/` is 890 files and
  203,612 lines in one crate [Verified: `find` and `wc` today].
- Axis 8 [Verified 11-16: QA journey 5]. The real-application figure on record is 82 of 153 files of one PHP
  application lifting, measured 2026-09-25 (gap matrix section 4.22) [Inferred: not re-measured].
- Axis 9 [Inferred: gap matrix section 4.22; its own text says the full re-tally of the 631 rows is still owed].
- Axis 12 [Verified: `ls -l target/release/phg`, 23,236,840 bytes] and [Verified 11-16: QA finding K-3, 22 of the 23
  example projects fail `phg build`, a recorded single-file limit].
- Axis 13 [Verified: `scripts/doc-guards.sh` and `scripts/claude-config-guard.py` green today].
- Axis 15 [Verified: `var/phorj-app` is absent on this machine].

## Domain by domain

### 1. Language design

**Ahead of PHP.**
- Type errors are compile errors. PHP reports a wrong argument type when the call executes [Verified: a function
  with an `int` parameter called with a non-numeric string throws `TypeError` at run time on the oracle].
- Integer overflow is a clean runtime error; PHP silently changes the value's type [Verified: `PHP_INT_MAX + 1` is
  a `float` on the oracle].
- Non-optional types are never null; `match` is exhaustiveness-checked; exceptions are checked; bindings are
  immutable unless marked. PHP has none of these as language guarantees and gets them only from external static
  analysers [Unverified: recall of PHP 8.5 and its tooling].
- Generics, sealed hierarchies, union and intersection types with narrowing, named-field tuples, typed HTML that
  escapes by construction, uniform call syntax for extension functions [Inferred: `FEATURES.md`, each row naming a
  runnable example].
- Every symbol is import-gated; nothing is ambient. PHP's global function namespace is the opposite choice
  [Unverified: recall].

**Behind PHP.**
- Recursion stops at 4,096 frames (`src/limits.rs`); PHP ran a 5,000-deep recursion that the native legs refused
  [Verified 11-16: sleuth AE-14]. Whether to raise, quarantine or disclose is an open question.
- A fault (division by zero, overflow) cannot be caught, and on the default engine it does not run pending
  `finally` or `using` cleanup, while the oracle and PHP do [Verified 11-16: sleuth K6]. The semantics are unruled.
- Generics are erased: an instance carries no type argument at run time. This is a deliberate decision (ADR-0002),
  and it is still a place where the language says less than its syntax suggests.

**Flaws.**
- *Breadth has outrun the proof of how features interact.* Multiple inheritance, traits, property hooks,
  overloading, statics and constructor promotion all coexist. Of the 23 wrong-behaviour queue entries, the first
  seven all sit in how those features compose (static composition, hook inheritance, set hooks during construction,
  bare names in subclasses, the initializer chain, static redeclaration, decomposed parent traits) [Verified: queue
  table A]. "Power is additive" has a cost that grows faster than the feature count. [Speculative]
- *Two error models are both called the default.* The specification says `throws` is "the default surface" in one
  place and "Result-first" in another [Verified 11-16: cross-check Q3-10].
- *The specification cannot be trusted section by section.* 84 findings in one pass; its own pending-change banners
  outrank correct prose below them and still call shipped features unbuilt [Verified 11-16: cross-check X-1].
- A retired form is removed outright before 1.0 (DEC-416) while `SEMVER.md` promises a grace release; the stance
  after the first stable release is open (north star, "Open").

### 2. Engine and parity

**Ahead of PHP.**
- A reference interpreter, a bytecode VM with a JIT and a transpiler are held to identical output by a differential
  harness. PHP has one implementation and nothing to disagree with; Phorj's arrangement turns a whole class of
  engine bug into a test failure. This is the project's best engineering asset. [Speculative]
- The three exhaustive `Op` matches carry no catch-all, and a sample of malformed inputs through `check`, `run`,
  `format` and `lift` produced no panic [Reported 11-16: inspect H-4 and D-0].
- First-party `unsafe` is confined to `src/jit/` and a CI job checks it [Verified: job `unsafe-island` green on
  HEAD]. The same job's pattern is reported to have a hole and not to check the crate roots' `deny` attribute
  [Reported 11-16: sleuth F-2], so the job is evidence, not proof.

**Flaws.**
- **The default engine skips `finally` when a catch body throws** [Verified: probe above; the reports place it in
  `compile_try`, `src/compiler/stmt/loops.rs`]. A rollback or a release written in `finally` silently does not
  happen under `phg run`, and does happen under the oracle and under PHP. Proposed P0. It is in neither
  `KNOWN_ISSUES.md` nor the queue [Verified: grep of both today].
- **The VM compiler re-derives types** (`src/compiler/cty.rs`), so a program the checker accepts and the oracle runs
  can be a compile error on the VM: `match` arm bindings are out of scope when the match is an arithmetic operand
  [Verified 11-16: sleuth K1]. This is the class Invariant 7 exists for, and it is structural: the same fact is
  computed twice.
- **A green differential proves the corpus, not the language.** The harness is driven by examples; it compares
  failures three different ways; it has no case for several fault kinds [Reported 11-16: forge H2, H3; gaps F-2].
  That is why row 9 could find an engine defect in a core construct and twelve library divergences with every
  gate green.
- A JIT code-generation error is swallowed as if it were an intended decline [Reported 11-16: forge G-4, code read
  only]. The fallback is total, so output is safe; a real JIT bug would show only as lost speed.
- Shared mutable instances are reference-counted with no cycle collector. One is ruled and queued (DEC-205). Until
  it is built, a cyclic object graph leaks, which matters most under a long-running `phg serve`. PHP has a cycle
  collector [Unverified: recall].

### 3. The PHP bridge (transpile and lift)

**Ahead of PHP.** No counterpart exists: this is Phorj's adoption strategy, and transpiled output agreeing with the
native legs on 261 programs is real evidence that the bridge works for the corpus.

**Flaws.**
- **Twelve natives compute something else on the PHP leg** [Verified 11-16: plan row 9; one of them re-run by me
  above]. The shapes: a regular expression whose `$` also matches before a trailing newline, an overflow inside a
  helper, a different math library on the last digit, the order of signed zeros in a sort, edge rules in CSV and
  base64 decoding, integer power. One of the twelve (the Validation predicates) has been a known issue since
  2026-07-19 and is in the queue. `KNOWN_ISSUES.md` also records the last-digit risk of transcendental functions as
  a disclosed corner, which the two math-library cases fall under. The rest are in neither list by name
  [Verified: grep join of `KNOWN_ISSUES.md` and the queue today].
- **A transpiled `match` with an arm after a catch-all does not parse in PHP** [Verified 11-16: sleuth AE-12].
- A fault exits 1 natively and 255 under PHP (a known, recorded difference) [Verified 11-16: QA K-7].
- Transpiled output using certain user names prints a deprecation line on PHP 8.6, which would land in a program's
  output once 8.6 is the floor (queue entry PHP86-DEPRECATED-NAMES).
- **The lifter can produce a draft whose output differs from the PHP it came from, without saying so**: an `echo` of
  a bool-returning call prints `true` or `false` where PHP prints `1` or nothing [Verified 11-16: QA N-6; whether the
  lift contract covers it is recorded as unconfirmed]. For a migration tool, a silent difference is worse than a
  refusal.
- The lifter refuses common PHP: `switch`, `do`/`while`, traits, `global`, statement-level `static` and `const`,
  the `**` operator [Reported 11-16: gaps LATER list, sleuth AE-15]. It aborts on a long expression chain at about
  a fifth of the formatter's limit [Verified 11-16: QA N-5].
- `Core.Database`, mail, the HTTP client and sessions do not transpile at all (hard errors by ruling). That is the
  ladder working as designed; it also means a database application cannot use the bridge for incremental
  deployment, which is the bridge's stated purpose.

### 4. Standard library

**Ahead of PHP.** Consistent naming and argument order, typed results instead of `false`-on-failure, typed error
taxonomies, injection-safe mail addresses and cookie defaults, a `Secret` wrapper, exact `decimal`. PHP's library
is larger and far less regular [Unverified: recall].

**Behind PHP.**
- Function parity is the drag: about 57 % usage-weighted, against about 86 % for syntax (gap matrix section 4.22)
  [Inferred]. Named gaps: XML, streams, internationalisation, the remaining multibyte functions.
- 170 of 389 members are named in no document; about 55 have no example [Reported 11-16: `/gaps`, grep-based].

**Flaws.**
- A native and its PHP twin are two free-text implementations with no shared table of edge cases and faults
  [Reported 11-16: forge C-1; the twelve reproduced divergences are its evidence]. This is the root of that class,
  and every new native can add one.
- Feature-gated code (HTTP client, mail, Postgres, MySQL, TLS serving) is compiled and linted only by the local
  pre-push hook, never by CI, and the live database round-trips run nowhere automated [Verified 11-16: inspect F-1,
  F-2; CI job list above shows no all-features job].
- Byte-oriented natives replace invalid UTF-8 natively and keep the raw byte in PHP [Reported 11-16: sleuth AE-10,
  agent-graded P2].

### 5. Web runtime

**Ahead of PHP.** A built-in concurrent server with TLS, a router, typed requests and responses, sessions and safe
cookie defaults, with no separate web server or process manager. Hostile requests in the QA sweep did not bring it
down; 60 concurrent requests were answered correctly [Verified 11-16: QA journey 10].

**Behind PHP.** Nothing is measured: no requests-per-second figure, no tail latency, no memory soak (axes 6 and 14).
PHP's deployment model has decades of production evidence [Unverified: recall].

**Flaws.**
- **Three security classes are open** (named by class only; reproductions are kept out of this public file):
  a static-file exposure in `phg serve` through a symlink inside the document root, where the help text promises
  the opposite unconditionally [Verified 11-16: inspect A-1, proposed P1]; cookie attribute injection through a
  separator character, on every leg, awaiting DEC-581; three raw response-header shapes awaiting DEC-568.
- Two further classes are **unconfirmed**: control characters in an HTTP-client URL (code read only; the release
  binary lacks the feature) and an unbounded session store under one lock (a flood run was not completed)
  [Unverified: inspect A-2, A-3].
- Handler captures persist across requests while PHP starts every request clean [Unverified: recall of PHP's
  model]. Combined with the missing cycle collector, a long-lived server is the configuration with the least
  evidence behind it. [Speculative]
- The help text still describes one request per connection although the server keeps connections alive
  [Verified 11-16: inspect E-1, QA N-8]. Shutdown can be held by one idle client, and there is no structured access
  log [Reported 11-16: sleuth H-5, forge H4].

### 6. Tooling and editors

**Ahead of PHP.** One binary carries the checker, formatter, test runner, debugger, language server, package
manager and builder. The compiler does what PHP needs separate analysers for. Every emitted diagnostic code has a
`phg explain` entry [Reported 11-16: `/gaps` axis 10], and `phg check` equals the language server's diagnostics on
the five cases tried [Verified 11-16: QA journey 3].

**Behind PHP.** PHP's editor support, debugger integration, test framework and package registry are mature and
widely used [Unverified: recall; no figures quoted].

**Flaws.**
- **The 100 % editor rule (Invariant 17) is far from met.** The language server answers nothing for enum variants,
  pattern and lambda bindings, generic type uses, import aliases and named-argument labels; signature help covers
  free functions only; completion after a typed scalar or collection receiver is empty [Reported 11-16: `/gaps`
  N3-N8, probed over the server's stdio by the agent; the hover, completion and constructor-signature gaps of QA
  N-3 are Verified 11-16]. Three providers (code actions, semantic tokens, inlay hints) are not advertised at all
  [Verified 11-16: QA K-6].
- **It can answer wrongly, not only silently.** A method and a free function with the same name are treated as one
  symbol, so a rename edits both; nothing inside a string interpolation is served, so a rename leaves the program
  broken [Verified 11-16: QA N-1, N-2].
- Loader-stage errors (an unused import, for one) fail `phg check` and never reach the editor [Verified 11-16:
  sleuth CJ-6]. `phg check --json` prints plain text on lex, parse and loader errors.
- Everyday errors (type mismatch, unknown function, every lex and parse error) carry no code [Reported 11-16:
  `/gaps` axis 10; three of four type errors in the QA diagnostics journey carried none, Verified 11-16].
- **The toolchain crashes on five inputs**: a non-UTF-8 argument [Verified: probe above], a non-UTF-8
  environment variable, a garbled `Content-Length` to the language server or debug adapter, a reader that closes
  the pipe (`phg run … | head`), and a long expression chain given to `phg lift` [Verified 11-16: sleuth G-1, G-2,
  H-1, B-5; QA N-5]. The `Content-Length` and deep-input cases are already in the queue (DEEP-INPUT-CRASHES,
  QA-N5); the other three are in neither list [Verified: grep join today].
- A failed `phg add` leaves the dependency written in the manifest, and the lock file writes a version field
  nothing reads [Verified 11-16: QA N-4, forge G-1 spot check]; `phg install` never prunes `vendor/` [Reported
  11-16: sleuth B-9].
- The formatter moves a trailing comment out of its function [Verified 11-16: QA N-7]; the debug adapter is not
  wired into either editor [Reported 11-16: gaps B/C-1].

### 7. Release, deployment and ecosystem

**Ahead of PHP.** A single static binary for a single-file program; fifteen vetted crates against an ecosystem
where an application's dependency tree is usually large [Speculative on the comparison; the crate count is
guarded by `tests/dependency_claims.rs`].

**Flaws.**
- **A multi-file project cannot be built**: `phg build` is single-file, and 22 of the 23 example projects fail it
  although they run, check and transpile [Verified 11-16: QA K-3]. The deployment advantage exists only for
  programs that fit in one file. macOS targets are not supported.
- The manifest reports `1.0.0-nightly.0` and no stable release exists; release numbering is open.
- `STABILITY.md` still promises that a removed module works with a deprecation warning, and claims every stable
  construct has a conformance fixture when 7 of 16 stable stdlib modules have none [Reported 11-16: sleuth CJ-11,
  `/gaps` DE E-3; the removed-module sentence is in `STABILITY.md` today, Verified by reading it].
- **One developer, no known outside user, no third-party package, no application.** Every axis above can be closed by
  work in this repository except this one. PHP's ecosystem is the thing PHP is actually chosen for. [Speculative]

## The three root causes

1. **Parity is proven by example, not by contract.** The spine is real, and it certifies exactly what the examples
   exercise. A native's two implementations share no specification; the harness compares at hand-picked points;
   failure is compared three ways. Result: one engine defect in a core construct and a dozen library divergences
   behind a fully green gate. [Inferred: forge C-1, H2, H3 against sleuth's reproductions]
2. **The same fact is derived more than once.** Types by the checker and again by the VM compiler; diagnostics by
   `check` and again, partly, by the language server; JSON escaping in three places; a feature's status in the
   specification, its banners, the register and the plan. Each duplicate is a place where two answers can differ,
   and row 8 and row 9 found a difference at every one of them. [Inferred: forge C-5, H6, F4; cross-check X-1]
3. **The records grew faster than their checkers.** `KNOWN_ISSUES.md` is 4,299 lines; the defect queue was seeded
   on 2026-11-13 and is still ungraded; 51 unratified decisions had accumulated before row 9b; the specification
   holds 84 findings. The project's own guards say documentation rot is its dominant defect class
   (`scripts/doc-guards.sh`, header). The guards added in ground prep help; the volume is still a cost every
   session pays before it writes a line. [Speculative on the conclusion; the figures are Verified today or 11-16]

## What would change the verdict

Recommendations only, each [Speculative]. The order of work is DEC-578's and the developer's.

1. **Grade the queue with row 9's findings joined in.** The aggregate report lists eight quick wins at 160 minutes
   in total (its estimate, not measured): the `phg serve` exposure, three of the five crash inputs (the argument,
   the environment variable, `Content-Length`) and four PHP-twin fixes. The proposed P0 is in its next tier, "up to
   two hours"; the closed pipe and the lifter's deep chain are in neither tier. Until a finding is graded it does
   not exist for the ordering rule.
2. **Fix the class, not the twelve instances**: one edge-case and fault table per native, driving both the Rust
   test and the PHP-twin run, plus boundary inputs through the differential.
3. **One three-leg comparison function** in the harness, and **one check-equals-editor equivalence test** over the
   conformance corpus including loader-stage errors.
4. **Arm the cheap owed checkers.** Startup and memory are a script each once a budget is ruled; parity can be
   recomputed by script from the native registry; throughput needs one fixed route set and a comparator.
5. **Generate the standard-library reference** from the native registry, then write the tutorial and the migration
   guide by hand. Axis 4 is the widest gap a newcomer meets.
6. **Make `phg build` project-aware**, then build the one real application. It is the only test that exercises
   every axis at once, and the only one PHP users will read.
7. **Retire the stale banners in the specification** in one pass; one fix serves the nine banner items of
   cross-check finding X-1.

## Open questions this file does not answer

Each is the developer's (Invariant 15) and is recorded where its owner is:
- Does a fault run pending `finally` and `using` cleanup (row 9, K6)? Not yet a register row.
- The 4,096 call-depth bound against PHP: raise, quarantine or disclose (row 9, AE-14)? Not yet a register row.
- DEC-568 (three raw response-header shapes) and DEC-581 (the cookie guard), both PENDING.
- Whether the lifter maps docblock pseudo-types instead of refusing them (row 9b, V50).
- Where the defect-queue grading pass falls, and every item under "Open" in `.claude/rules/north-star.md`
  (release numbering, DEC-416 after a stable release, whether a perf row is a feature row, the budgets of axes 7,
  12 and 14).

## Not assessed

- Performance of any kind, freshly: no benchmark, memory, startup or throughput run was made for this file.
- Feature-gated code at run time (HTTP client, mail, Postgres, MySQL, TLS serving): read by the hunters, never run.
- Live editor behaviour in VS Code or a JetBrains IDE; the playground (the build tool is not installed here).
- macOS and Windows binaries; crate advisories (offline); the 91 `unreachable!` sites; a full read of `src/` (the
  hunters sampled it).
- The reverse direction of the specification check (code the specification never mentions).
- PHP-side figures: ecosystem size, market share and production track record are stated from recall and carry no
  number here on purpose.
- Every row 9 P2 and P3 finding is as its agent graded it; only the proposed P0 and P1 were reproduced by a main
  session, and only three of those by me.

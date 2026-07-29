# Consistency audit — 2026-07-28

**Scope**: 6 lenses over the whole tracked repo (`claude-bundle/`, `var/`, `target/` excluded) —
L1 truth-vs-reality, L2 rule-vs-rule / rule-vs-implementation, L3 unwritten rules, L4 record
self-contradiction (register/plans/specs), L5a doc→code gaps, L5b code→doc gaps, L6 nasty sweep.
**Method**: 5 parallel evidence-based read-only reviewer agents (A=L1, B=L2+L3, C=L4, D=L5a,
E=L5b+L6), every claim verified against the file/binary itself; this synthesis re-checked every
fact the lenses disagreed on. Raw lens reports: session scratchpad (`audit/raw/{A..E}.md`);
this file is the persisted record.

**Totals**: **112 findings** (A 36 · B 20 · C 25 · D 11 · E 20) — **P0 0 · P1 27 · P2 48 ·
P3 37** — deduplicated into **17 root-cause clusters**. Verdicts: the bulk is UNAMBIGUOUS
(mechanical doc/label fixes, many already ruled but unexecuted); 27 NEEDS-RULING questions were
consolidated for the developer (10 of them the pre-existing open adjudications).

**Cross-lens fact checks resolved by this synthesis** (both sides re-read):
- Default-build linked-crate count: Lens A said "~10", Lenses B/D said 9. **Re-derived from
  `Cargo.toml [features] default`: 9** (argon2, regex, ctrlc, corosensei, cranelift,
  cranelift-jit, cranelift-module, rusqlite, unicode-segmentation). B/D right, A wrong.
- `conformance/README.md` `phorj.toml` project marker: Lens D scored it CLEAN (doc matches code —
  `tests/conformance.rs:99,122` really keys on `phorj.toml`), Lens A scored it NEEDS-RULING.
  Both facts confirmed; the doc is NOT false — whether the harness marker should migrate
  post-DEC-282 is a developer call (→ Q26).
- Trait count: re-ran `grep -rnE '^\s*(pub )?trait ' src/ | grep -v test` → **5** (Transport,
  DebugFrontend, Suspend, Task, DbObject) — confirms A-024.
- Real dispatch functions: `cmd_run` / `cmd_treewalk` / `cmd_transpile` (+`cmd_run_exit`/
  `cmd_treewalk_exit`) live in `src/cli/pipeline.rs`, re-exported via `phorj::cli` — confirms
  A-022's corruption diagnosis.

---

## Root-cause clusters

### CL-01 · The "four / zero external crates" false-claim family + THIRD-PARTY-NOTICES license gap — **P1, UNAMBIGUOUS** (one member NEEDS-RULING)

The single most-replicated disease: dependency reality is **14 vetted, feature-gated crates in
`Cargo.toml`, 9 linked in the default build**; CLAUDE.md's own header warns the "four exceptions"
wording "must not be repeated" — yet it survives on 11 surfaces, including *inside both declared
SSOTs* and inside the legal attribution file (10 crates unattributed; cranelift is
Apache-2.0-WITH-LLVM-exception, breaking the "all MIT OR Apache-2.0" claim; default-on rusqlite
bundles SQLite's own license surface). Flagged as far back as the 2026-07-03 corpus audit (B1);
still live.

Evidence sites: `CITATION.cff:5-6` ("std-only, zero external crates") · `README.md:6`, `:92-96`,
`:321-323` ("exactly four…") · `SECURITY.md:42-43` ("links zero external crates") ·
`THIRD-PARTY-NOTICES.md:5-24` (4-row table + "All four…") · `VISION.md:59-61` ·
`CONTRIBUTING.md:15` · `Cargo.toml:46-48` (features header names 2 domains), `:211-212`
(workspace comment "four vetted") · `docs/specs/UNIFIED-SPEC.md:875-876` ("four … ship by
default, three more domains approved"), `:920-928` (section header + 5-row table — 9 admitted
crates have NO table row, so the policy's own "table entry above" admission process is unmet),
`:1410` ("the four vetted feature-gated deps") · `GOVERNANCE.md:32` ("the std-only line" —
NEEDS-RULING wording, → Q21).

Members: A-001 A-002 A-003 A-004 A-005 A-006 A-018(partial) A-034(NR) B-002 D-003 D-004
E-001 E-002 E-003 E-004.
Fix (one line): re-derive every count from `Cargo.toml` (14 total / 9 default-linked), regenerate
THIRD-PARTY-NOTICES with all 14 crates + verified licenses + transitives, and point every other
surface at THIRD-PARTY-NOTICES/UNIFIED-SPEC instead of restating a number.

### CL-02 · Retired `phg vendor` / `phorj.toml` still advertised as live (DEC-282/DEC-316 never swept) — **P1, UNAMBIGUOUS** (one member NEEDS-RULING)

`phg vendor` hard-errors (`src/cli/pm.rs:23-27`); the live network-capable commands are
`phg add / install / update / remove` (`src/main.rs:181-184`, "the ONLY network-capable
commands"); the manifest is `phorj.json` (DEC-316). Yet the security policy, the README command
table, project Invariant 10, MILESTONES M5, FEATURES.md and an ADR still present the retired
mechanism as current — and KNOWN_ISSUES' own punch-list correction ("now NONE do [touch the
network]") is itself stale post-DEC-316.

Evidence sites: `SECURITY.md:44-50` (whole present-tense vendor subsection: "the only command
that touches the network") · `CLAUDE.md:121` (Invariant 10 parenthetical) · `README.md:141`
(command table row; table also omits add/install/update/remove/lsp/debug/extensions) ·
`docs/MILESTONES.md:263-271` (M5: "`phorj.toml` manifests … `phg vendor` is the sole network
command", no supersession note) · `FEATURES.md:98` ("fetching = a future package-manager
extension" — the PM SHIPPED; zero DEC-316/phorj.json/phg-add hits in FEATURES.md) ·
`docs/adr/0005-offline-only-vendor.md:3` (Status: Accepted — KNOWN_ISSUES:77 already instructs
marking it superseded) · `KNOWN_ISSUES.md:73` ("now NONE do, a STRONGER stance" — false),
`:1618`, `:1626` (M5 S3 section, `[require]`) · `conformance/README.md:29` +
`tests/conformance.rs:99,122` (phorj.toml as harness project marker — doc matches code,
NEEDS-RULING whether to migrate, → Q26).

Members: A-010 A-012 A-013 A-027 A-028 A-030(NR) A-031(item 3) D-002 D-008 E-006 E-007 E-008.
Fix: one sweep replacing the vendor/manifest story with DEC-316 reality (network surface =
add/install/update/remove; `phorj.json`; vendor errs loudly); mark ADR-0005 superseded; add the
four PM verbs + lsp/debug/extensions to the README table; rewrite FEATURES.md row 98 + add a
package-manager row.

### CL-03 · `forbid(unsafe_code)` → `deny` unsafe-posture drift (JIT island, 2026-07-06) — **P1, UNAMBIGUOUS**

Both crate roots are `#![deny(unsafe_code)]` (`src/lib.rs:10`, `src/main.rs:5`) with one audited
`#![allow]` island at `src/jit/mod.rs:80` behind the CI `unsafe-island` gate. Five surfaces —
including SECURITY.md, the exact document a security reviewer reads — still claim `forbid`
crate-wide / "no unsafe in this crate". `Cargo.toml` contradicts itself: `:90-91` correctly
describes the relax while `:140`, `:202-203`, `:212` still say `forbid`.

Evidence sites: `CONTRIBUTING.md:36` · `SECURITY.md:40-41` · `THIRD-PARTY-NOTICES.md:7` ·
`Cargo.toml:140` (corosensei comment), `:202-203` (lints comment), `:212` (workspace comment).
Members: A-007 A-008 A-009 B-006 D-005 E-005.
Fix: state the real posture everywhere — `deny(unsafe_code)` on both roots + the audited
`src/jit/` island, CI-gated.

### CL-04 · Phantom `--sequential-concurrency` flag (DEC-369 ruled the deletion 2026-07-26; sweep never executed) — **P1, UNAMBIGUOUS**

`grep -rn 'sequential-concurrency' src/ tests/` → zero hits; the shipped mechanism is the
unconditional `E-CONCURRENCY-NO-PHP` hard error (`src/transpile/expr.rs:548`). The flagship
LADDER RULE's sole recorded "first application" cites an opt-in that never existed. DEC-369
already orders the deletion — unexecuted at HEAD.

Evidence sites: `CLAUDE.md:144` (Invariant 14) · `docs/plans/MASTER-PLAN.md:726-727` (G-1.1),
`:2382` (rulings table) · `docs/research/full-audit/raw/C-decisions.md:948` (DEC-225 row's
present-tense "Current status").
Members: B-003 C-019 D-001 E-023.
Fix: execute the DEC-369 docs-only edit at all four sites (hard error + differential quarantine
stays; the flag clause goes).

### CL-05 · SECURITY.md's `phg serve` concurrency model is a retired architecture — **P1, UNAMBIGUOUS**

`SECURITY.md:51-52`: "single-threaded by design … one connection at a time". Reality: M6 W3
shipped `--workers N`, **default = one worker per CPU core** (`src/main.rs:349-351`, `:436-440`
`available_parallelism()`; `src/serve/mod.rs`), `--workers 1` restores the single-threaded path.
The security doc's DoS/slowloris reasoning is built on the old model (its 8 MiB body-cap claim
in the same paragraph is accurate).
Members: A-011 D-006.
Fix: rewrite the paragraph around the bounded worker-pool default (per-worker `Rc` heap, values
never cross threads, `--workers 1` fallback).

### CL-06 · README prebuilt-binary instructions don't match released assets — **P1, UNAMBIGUOUS**

`README.md:100-106` says "statically-linked" binaries and `chmod +x phg-*-linux-x86_64-musl`.
Actual release assets (`.github/workflows/release.yml:31-44,70,80`): `phg-linux-x86_64.tar.gz`
(gnu target, dynamically linked), windows zip, two macOS tar.gz — no musl asset, and archives
must be extracted, not chmod-ed. A user following the README verbatim fails. (musl is a
`phg build` cross-target for *user programs* — the likely confusion source.)
Members: A-014.
Fix: rewrite the block around the 4 real archive names + extract-then-run steps.

### CL-07 · `runvm` → "the VM leg" find-replace corruption + leftover `runvm` claims — **P1, UNAMBIGUOUS**

A global find-replace mangled code identifiers in prose: `cmd_the VM leg` (no such identifier —
real: `cmd_run`/`cmd_treewalk`/`cmd_transpile` in `src/cli/pipeline.rs`), "there is no `phg run`
subcommand" (self-denying typo for `runvm`), MASTER-PLAN G-1 stating the spine tautologically as
"`phg run` ≡ `phg run`". `docs/INVARIANTS.md` §1 was already repaired; these sites were missed.

Evidence sites: `docs/ARCHITECTURE.md:88-89` · `docs/adr/0001-no-shared-run-vm-ir.md:13` ·
`docs/plans/MASTER-PLAN.md:717` (G-1 tautology), `:1246` ("run+the VM leg"), `:1683` ("DAP over
the VM leg") · `docs/plans/SLICE-STATE.md:2404` (inside a PREVIOUS/history log — recorded, NOT
fixed: history stays as written) · `KNOWN_ISSUES.md:64-65` (the `phg run` typo), plus hook
comments `scripts/git-hooks/pre-commit:10` / `pre-push:4` ("run == runvm parity") and
`docs/MILESTONES.md:35,59` (`phg runvm` + `src/cli.rs` in dated sections — historical-marker fix
only).
Members: A-022 A-031(item 2) B-010 D-007 D-011.
Fix: repair the living-doc sites to the real identifiers; annotate the dated MILESTONES/ADR
mentions as historical names; fix the KNOWN_ISSUES typo to "no `phg runvm` subcommand".

### CL-08 · MASTER-PLAN / MILESTONES / bootstrap not reconciled with the newest rulings (Invariant-19 breaches) — **P1, UNAMBIGUOUS**

The dominant live failure mode C.md names ("ruled → docs never reconciled in the same change")
recurred in the 2026-07-26/27 batches themselves:
- **W2-3 vs DEC-343** — `MASTER-PLAN.md:1555` still orders "foreach REPLACES for-in (REPLACE,
  not add)"; DEC-343 (RULED 2026-07-26) amended DEC-248 to **keep both** and closed Conflict
  C-2. A session executing Wave 2 from the plan would retire 87 corpus sites against the ruling.
  The DEC-248 row (`C-decisions.md:1372-1382`) also carries no supersession note. (C-002)
- **G-4 vs DEC-387** — `MASTER-PLAN.md:742` still mandates "interactive AskUserQuestion";
  Invariant 15 + DEC-387 FORBID it (it silently fails in this container). (B-004)
- **MILESTONES inverts the threads supersession** — `docs/MILESTONES.md:291-295` says the
  serve worker pool "superseded the planned green-threads"; the register's SUPERSEDED table
  (`C-decisions.md:309`) records the opposite direction (green threads DEC-132 shipped —
  `corosensei`, `FEATURES.md:73`; the OS-thread *serve pool plan* is what was superseded on
  06-29). (C-004)
- **DEC-388 not mirrored** — `MASTER-PLAN.md:34` "BUILT so far: Wave 5.5 only" + zero DEC-388
  hits in MASTER-PLAN and the build order; register + SLICE-STATE record DEC-388.1–.5 BUILT
  2026-07-27 (incl. 388.2 *reversing* a DEC-354 sub-ruling). (C-018)
- **Bootstrap skills inventory** — `scripts/claude-bootstrap/CLAUDE-global.md:475` tells every
  fresh session "12 skills; `/forge` is NOT installed here"; disk has 13 incl. `forge/`
  (DEC-388.2). (C-003)
- **Cursor off-by-one** — `MASTER-PLAN.md:34` "plus 21 more … (DEC-366…386)" vs
  `SLICE-STATE.md:49-50` "plus 22 more … (DEC-366…387)". (C-025)

Members: B-004 C-002 C-003 C-004 C-018 C-025.
Fix: one reconciliation pass — amend W2-3 + annotate DEC-248, rewrite G-4's protocol sentence
per DEC-387, correct the MILESTONES supersession direction, mirror DEC-388 into MASTER-PLAN §0 +
the build order, update the bootstrap skills line to 13 incl. `/forge`, align the counts.

### CL-09 · Dangling DEC / section references — **P1, UNAMBIGUOUS**

- **13 DEC ids referenced repo-wide have no row in the canonical register** (CLAUDE.md claims
  it holds "all DEC rows"): DEC-185, 187, 188, 189, 190, 192, 193, 194, 195, 196, 198, 199, 303.
  Live references incl. `CHANGELOG.md:1491`, `examples/README.md:98`, `MASTER-PLAN.md:2181-2245`
  (DEC-188…193 register-forked into §13.1.1 — DEC-190's ruling exists nowhere else),
  `KNOWN_ISSUES.md:580`, `M-gap-matrix.md:144`, `SLICE-STATE.md:1628`. Known since H40
  (2026-07-25), still open. (C-005)
- `C-decisions.md:372` cites "MASTER-PLAN §12" for the 2026-07-02 rulings; renumbering moved
  them to **Appendix B** (`MASTER-PLAN.md:2367`). (C-023)
- `docs/MILESTONES.md:127-128` points at "`CLAUDE.md` (Active plan)" — CLAUDE.md has been
  rules-only since DEC-016; the live cursor is SLICE-STATE. (C-024)

Members: C-005 C-023 C-024.
Fix: backfill the 13 register rows (pointer rows to where each ruling actually lives is
sufficient), repoint §12→Appendix B, repoint MILESTONES at SLICE-STATE/MASTER-PLAN §0.

### CL-10 · Unwritten rules — standing rulings absent from CLAUDE.md/INVARIANTS — **P1 (top member), 3 UNAMBIGUOUS + 5 NEEDS-RULING**

Rulings with force *now* that a fresh session reading only CLAUDE.md/INVARIANTS cannot know:
- **B-101 (P1, UNAMBIGUOUS — placement already ruled)**: DEC-378's "never run two commits
  concurrently — the hooks share `target/`" one-liner, ruled to live in CLAUDE.md; grep of
  CLAUDE.md/INVARIANTS → zero hits. The evidenced failure (two racing `cargo test` runs) can
  recur on any autonomous green commit today.
- **B-102 (P2, UNAMBIGUOUS)**: DEC-365 NO-HIDDEN-LOSS semantics (unmeasurable = verdict OWED,
  never "passed"; a real loss gets fixed, never suppressed) — absent from Invariant 18 /
  INVARIANTS #14.
- **B-103 (P2, UNAMBIGUOUS — placement ruled)**: DEC-371 "PHP's lack of a feature is never a
  reason against building it", ruled to sit beside Invariant 16; absent.
- **B-104 (NR → Q16)**: the spine→FRESH-context standing rule — applied by name in DEC-294/302,
  normatively written nowhere.
- **B-105 (NR → Q15)**: DEC-377's 3-bucket `__phorj_*` helper test is *sharper than and in
  tension with* Invariant 16's "always an acceptable tool" wording.
- **B-106 (NR → Q17)**: "default-recommend the more-correct answer" lives only in MASTER-PLAN
  G-4.
- **B-107 (NR → Q18)**: DEC-375 expert-companion bar not pointed at from Invariant 17.
- **B-108 (NR → Q19)**: DEC-326/346 UFCS house style absent from Invariant 12.

Members: B-101 B-102 B-103 B-104(NR) B-105(NR) B-106(NR) B-107(NR) B-108(NR).
Fix: write the three ruled one-liners into CLAUDE.md (+INVARIANTS #14 pointer); the five homing/
wording calls go to the question batch.

### CL-11 · Stale status/supersession labels (~55: the review's 40 + 15 newer) — **P2, UNAMBIGUOUS label flips** (one reconciliation NEEDS-RULING)

The census: the known **40** (26 open-but-built + 14 done-but-not) enumerated at
`docs/research/2026-07-25-completeness-register.md:515-536`, spot-verified still unfixed at HEAD
(e.g. `2026-07-24-wildcard-imports.md:1-3` header "NOT YET BUILT" vs its own `:228` "✅ Q-A DONE";
DEC-216 still "PENDING" though DEC-316 shipped). **Plus 15 newer** found by this audit:
- Register: DEC-286 PENDING (superseded by DEC-380, C-008) · DEC-206 unannotated (DEC-386/353,
  C-007) · DEC-337 header lacks the DEC-353 note (C-006) · CONFLICTS table C-2/C-8/C-9 still
  "Open" though closed/shipped (DEC-343 / DEC-245 / DEC-196-Q3+DEC-047 partial, C-010, incl.
  DEC-094/057/047 rows) · five "ALL **PENDING**" section headers over fully-RULED tables
  (`C-decisions.md:3378, 3423, 3454, 3478, 3514` — sole true PENDING beneath them: DEC-366,
  C-011) · totals footer "147 rows … 10 conflicts, 33 supersessions" vs 226 rows/≥40
  supersessions (C-022).
- KNOWN_ISSUES stale-PENDING cluster: `:530` cycle-leak (→DEC-205), `:544` using/defer
  (→DEC-203/364), `:554` shutdown (→DEC-204), `:674` DEC-200 "until ruled, avoid naming"
  (→DEC-202 guard SHIPPED), `:151` Response-CRLF "PENDING adjudication" (→DEC-363, build
  queued), `:726/:758/:779` retry surface (→DEC-249). (C-012)
- MASTER-PLAN stale-PENDING cluster: `:79` serve-TLS (→DEC-331 D7), `:375-378` DEC-208-S2 /
  DEC-220 S2-S3 (shipped), `:874`+`:1101-1102` (→DEC-203/204/205), `:1663` W4-10 XML (→DEC-382),
  `:1951` DEC-197 (→DEC-274), `:2249` DEC-200 (→DEC-202), `:1490` IntelliJ scope (→DEC-181).
  (C-013)
- `docs/MILESTONES.md:237` + `SLICE-STATE.md:357,363` + register `:3261` — P-Q-B-1 "pending dev
  ruling" (→DEC-379). (C-017)
- `SLICE-STATE.md:120` RULED header over `:129-131` "PENDING a ruling" body. (C-021)
- `UNIFIED-SPEC.md:166-176`+`:204` "(all stand)" over two superseded rows — `is` identity
  (→DEC-051/184; `is` is a shipped TYPE TEST, `src/parser/exprs/climb.rs:132-160`) and
  "single inheritance + traits" (→DEC-062 real multiple inheritance). The adjacent table already
  marks other rows "superseded" — the mechanism exists, these two were missed. (C-014)
- Register DEC-184 shipped column "📐 (slice 3)" though `is` is shipped; `FEATURES.md:51`
  documents only `instanceof`. (C-015)
- README status table "M3+ 🔲 planned" (A-015) and MILESTONES "M2.5 … Phase 3 next / Phase 3 🔲"
  though Phase 3a (CI stub registry) shipped (`.github/workflows/stub-registry.yml`) (A-026).

NEEDS-RULING member: **C-001/DEC-383** — the register queues re-ruling of lifetime forks (a)/(c)
that DEC-205/DEC-204 already ruled 2026-07-12 (premise error traced to the L-84 row citing the
stale KNOWN_ISSUES PENDING blocks). Reconcile before Wave 7.5 asks anything (→ Q1).

Members: A-015 A-026 C-001(NR) C-006 C-007 C-008 C-009 C-010 C-011 C-012 C-013 C-014 C-015 C-016
C-017 C-021 C-022.
Fix: execute Wave-0 item 0.4 extended to the full census (40 + 15) — flip each label per the
census; the systemic guard is already ruled (DEC-362) and unbuilt.

### CL-12 · JIT-default flip (2026-07-09) + database-default (DEC-227) never propagated to comments — **P2, UNAMBIGUOUS**

`jit` and `database` are in `default = […]` (Cargo.toml:50-54), yet: `Cargo.toml:88` "OFF by
default", `:94` "Off by default" — self-contradicting the same file; both git hooks assert
"`--features jit` … (not a default feature)" as the flag's rationale
(`scripts/git-hooks/pre-commit:16-18`, `pre-push:9`); `scripts/microbench-gate.sh:10` still says
"Today every feature LOSES" while `bench/micro-baseline.json` records many WINs ≥1.0 (the
ratchet is armed and protecting real wins).
Members: A-018(:88) A-020 A-021 B-005.
Fix: correct the four comment sites (flags stay as harmless belt-and-braces).

### CL-13 · Quality-gate meta-doc drift (hooks/gates described wrongly by the rule docs) — **P2, mixed**

- **B-001 (UNAMBIGUOUS)**: `docs/INVARIANTS.md:116-117` claims pre-commit runs
  `clippy -Dwarnings`; the hook contains no clippy (moved to pre-push 2026-07-08, per its own
  header). CLAUDE.md describes the split correctly — the two rule docs contradict each other.
- **B-009 (part UNAMBIGUOUS / part NR)**: CLAUDE.md's hook description omits steps the hooks run
  (`phg format --check`, doc-tests, size-gate, validate-infra, `cargo build --release`) —
  one-line additions; AND `pre-push:74` runs nextest **without `--workspace`** while the written
  gate says `--workspace` (playground member untested at push) — whether to add the flag or fix
  the text is a call (→ Q14).
- **B-011 (UNAMBIGUOUS)**: "90 grandfathered" vs 78 baseline rows; 6 stale baseline rows the
  gate itself says to drop; `MASTER-PLAN.md:755` G-6 "size-gate.sh CI gate still to build" is
  half-stale (script exists, pre-push-wired; only CI wiring absent).
- **B-007 (NR → Q13)**: Invariant 13's "per source file" gate only scans `src/`; six
  `tests/*.rs` files exceed the 500 hard cap ungated (differential.rs = 4737 — MASTER-PLAN G-1
  still says 3308).
- **B-012 (UNAMBIGUOUS, minor)**: imported skills carry unadapted template text —
  `.claude/skills/pre-commit/SKILL.md:72` greps `~/.claude/` for blast radius (finds nothing for
  repo symbols) + unrewritten manual-commit posture vs the autonomy note; `converge/SKILL.md`
  double-"(default)" + advisor remnants.

Members: B-001 B-007(NR) B-009(part NR) B-011 B-012.

### CL-14 · examples/README dead `db/` paths — **P2, UNAMBIGUOUS**

`examples/README.md:217-232` names 11 examples under a `db/` prefix that does not exist; the
directory is `examples/database/` and all 11 files exist there (line 232's own body already uses
the correct path). Members: E-009 E-020.
Fix: `db/` → `database/` on the 11 table rows.

### CL-15 · 71 dead backtick path references (class already ruled — DEC-362 guard unbuilt) — **P2, UNAMBIGUOUS**

Markdown *links* are 0-dead; backtick `dir/file.ext` references include 71 dead ones:
`docs/MILESTONES.md` 29 (16 deleted plans/specs + 7 pre-M-Decomp paths `src/vm.rs` etc.),
`KNOWN_ISSUES.md` 10 (`src/fmt/*` → `src/format/`, `src/cli/explain.rs` → dir, …),
`docs/specs/*` 16 (6 are QUEUED-spec forward references — arguably intentional, need a marker),
MASTER-PLAN 6, SLICE-STATE 3, UNIFIED-SPEC 3, `examples/README.md` 3, `docs/INVARIANTS.md` 1.
Members: E-018.
Fix: the DEC-362 pre-push markdown reference-checker build, seeded with this inventory; repair
the renamed-path subset now, mark QUEUED-spec forward refs as planned.

### CL-16 · Misc stale-doc singletons — **P2 (max), UNAMBIGUOUS**

- `README.md:231-238` LSP list omits references/highlight/rename/formatting (capabilities at
  `src/lsp/mod.rs:462`). (A-016)
- `SEMVER.md:4` "current series is `0.x`" — binary reports 1.0.0-nightly.0. (A-017)
- `.github/workflows/ci.yml:33` comment "channel 1.96.0" — pin is 1.97.1. (A-019 E-022)
- `docs/ARCHITECTURE.md:55` cli row lists `vendor`, omits 10 live commands (A-023); `:89-90`
  "finds 4 traits" → 5 incl. `DbObject` (A-024); `chunk.rs`/`value.rs`/`serve.rs` named as
  single files + `stack_effect` "in compiler/mod.rs" → all now dirs / `compiler/emit.rs:75`
  (A-025).
- `conformance/README.md:19` credits ddd with "cross-package `import type`" — retired syntax,
  zero corpus hits. (A-029)
- Retired `import type` named in comments: `examples/project/shapes/src/Acme/Geometry/
  Shape.phg:3`, `examples/project/visibility/src/main.phg:5`, `src/loader/resolve.rs:8,109,133`.
  (A-035)
- `MASTER-PLAN.md:106` "`Core.Sandbox` BUILDS in v1" reads as a completion report —
  `E-TRANSPILE-SANDBOX` has zero src hits; mark QUEUED. (D-009)
- 4 env vars read by code, documented nowhere: `PHORJ_GIT` (`src/pm/fetch.rs:40`,
  security-relevant), `PHORJ_HANDOFF_DIR`, `PHORJ_BLESS`, `PHORJ_JIT_DISASM`. (E-010)
- `Core.Environment` module absent from FEATURES.md/examples README (registered in
  `src/native/`; only UNIFIED-SPEC:313 names it). (E-014)
- Report-only / tracked: 3 flags missing from their own `--help` (tracked as MASTER-PLAN B3-14 ☐,
  E-012); 1 real TODO in `src/lift/lifter/exprs.rs:343` (disclosed, E-016); panic-surface smell —
  3 tokenizer `from_utf8().unwrap()` sites on untrusted input worth an audit pass
  (`src/tokenizer/{ident.rs:10,scan.rs:139,strings.rs:90}`), no P0 claimed (E-021).

Members: A-016 A-017 A-019 A-023 A-024 A-025 A-029 A-035 D-009 E-010 E-012 E-014 E-016 E-021
E-022.

### CL-17 · Wording/scope calls routed to the question batch — **NEEDS-RULING**

Standalone adjudication-shaped findings (each → a numbered question in the batch):
- A-032 — "every example gated by the differential" claim vs the impure/DB/mail quarantines
  (broaden the caveat vs re-scope) → Q20.
- A-033 — CHANGELOG "~340 shipped examples" matches no corpus (266 examples / 330 w/
  conformance / 383 all) → Q27.
- A-036 E-026 — README serve row names only the `handle(Request)` bridge; the real entry is
  `respond(bytes): bytes` → Q22.
- D-010 — `phg extensions --help` exits 2 while README promises per-command help universally
  (code fix vs doc caveat) → Q23.
- E-011 — `green` (default!) and `database-all` features absent from the EXTENSIONS listing
  (jit precedent says list them) → Q24.
- E-013 — `phg rewrite-new`: shipped, in-place-rewriting, documented nowhere (document vs
  delete) → Q25.
- C-020 — DEC-247 tz-dependency "PENDING-BLOCKED" vs the review's "dependency ruling already
  done" — one of the two records is wrong → Q4.
- C-006(wording half) — whether the "nothing in the wind" GLOBAL TENET gets an explicit
  injected-symbol carve-out per DEC-353 → Q5.

Members: A-032 A-033 A-036 C-006(part) C-020 D-010 E-011 E-013 E-026.

---

## Findings appendix — every lens ID → cluster

| Lens ID | Cluster | | Lens ID | Cluster | | Lens ID | Cluster |
|---|---|---|---|---|---|---|---|
| A-001 | CL-01 | | B-001 | CL-13 | | D-001 | CL-04 |
| A-002 | CL-01 | | B-002 | CL-01 | | D-002 | CL-02 |
| A-003 | CL-01 | | B-003 | CL-04 | | D-003 | CL-01 |
| A-004 | CL-01 | | B-004 | CL-08 | | D-004 | CL-01 |
| A-005 | CL-01 | | B-005 | CL-12 | | D-005 | CL-03 |
| A-006 | CL-01 | | B-006 | CL-03 | | D-006 | CL-05 |
| A-007 | CL-03 | | B-007 | CL-13 (Q13) | | D-007 | CL-07 |
| A-008 | CL-03 | | B-008 | CL-10/B-102 kin — DEC-365 gate divergence (build queued 0.2) | | D-008 | CL-02 |
| A-009 | CL-03 | | B-009 | CL-13 (part Q14) | | D-009 | CL-16 |
| A-010 | CL-02 | | B-010 | CL-07 | | D-010 | CL-17 (Q23) |
| A-011 | CL-05 | | B-011 | CL-13 | | D-011 | CL-07 |
| A-012 | CL-02 | | B-012 | CL-13 | | E-001 | CL-01 |
| A-013 | CL-02 | | B-101 | CL-10 | | E-002 | CL-01 |
| A-014 | CL-06 | | B-102 | CL-10 | | E-003 | CL-01 |
| A-015 | CL-11 | | B-103 | CL-10 | | E-004 | CL-01 |
| A-016 | CL-16 | | B-104 | CL-10 (Q16) | | E-005 | CL-03 |
| A-017 | CL-16 | | B-105 | CL-10 (Q15) | | E-006 | CL-02 |
| A-018 | CL-01+CL-12 | | B-106 | CL-10 (Q17) | | E-007 | CL-02 |
| A-019 | CL-16 | | B-107 | CL-10 (Q18) | | E-008 | CL-02 |
| A-020 | CL-12 | | B-108 | CL-10 (Q19) | | E-009 | CL-14 |
| A-021 | CL-12 | | C-001 | CL-11 (Q1) | | E-010 | CL-16 |
| A-022 | CL-07 | | C-002 | CL-08 | | E-011 | CL-17 (Q24) |
| A-023 | CL-16 | | C-003 | CL-08 | | E-012 | CL-16 (tracked B3-14) |
| A-024 | CL-16 | | C-004 | CL-08 | | E-013 | CL-17 (Q25) |
| A-025 | CL-16 | | C-005 | CL-09 | | E-014 | CL-16 |
| A-026 | CL-11 | | C-006 | CL-11 + CL-17 (Q5) | | E-015 | clean |
| A-027 | CL-02 | | C-007 | CL-11 | | E-016 | CL-16 (informational) |
| A-028 | CL-02 | | C-008 | CL-11 | | E-017 | clean |
| A-029 | CL-16 | | C-009 | CL-11 | | E-018 | CL-15 |
| A-030 | CL-02 (Q26) | | C-010 | CL-11 | | E-019 | clean |
| A-031 | CL-02 + CL-07 | | C-011 | CL-11 | | E-020 | CL-14 (≡E-009; reverse dir clean) |
| A-032 | CL-17 (Q20) | | C-012 | CL-11 | | E-021 | CL-16 (smell) |
| A-033 | CL-17 (Q27) | | C-013 | CL-11 | | E-022 | CL-16 |
| A-034 | CL-01 (Q21) | | C-014 | CL-11 | | E-023 | CL-04 |
| A-035 | CL-16 | | C-015 | CL-11 | | E-024 | clean |
| A-036 | CL-17 (Q22) | | C-016 | CL-11 | | E-025 | clean |
| | | | C-017 | CL-11 | | E-026 | CL-17 (Q22) |
| | | | C-018 | CL-08 | | E-027 | clean |
| | | | C-019 | CL-04 | | | |
| | | | C-020 | CL-17 (Q4) | | | |
| | | | C-021 | CL-11 | | | |
| | | | C-022 | CL-11 | | | |
| | | | C-023 | CL-09 | | | |
| | | | C-024 | CL-09 | | | |
| | | | C-025 | CL-08 | | | |

(B-008 note: the microbench-gate's exit-0-on-unmeasurable divergence from DEC-365 is RULED and
queued as build-order item 0.2 — recorded here so it isn't lost; the doc-side half is CL-10's
B-102 write-up.)

---

## Verified clean (checked and found TRUE — no finding)

- **Wildcard-free triple `Op` match** re-verified 2026-07-28: `exec_op` (`src/vm/exec.rs:9`),
  `validate` (`src/chunk/validate.rs:21`), `stack_effect` (`src/compiler/emit.rs:75`) — no `_`
  arms on the top-level matches (the 3 `_` hits in exec.rs are nested inner matches).
- **CLAUDE.md's 14-crate count + non-default feature list** — exact vs Cargo.toml.
- **Env vars**: every documented `PHORJ_*`/`PHG_*` var is read in src/tests/scripts (doc-set
  minus src-set = ∅).
- **Cargo features**: every doc-named feature exists; `http-server-tls` properly future-tensed.
- **phorj.json keys**: all doc-claimed keys parsed in `src/pm/manifest.rs` (incl. `edition`).
- **Example paths**: 48/55 doc-referenced `.phg` paths exist; the 7 "missing" are future-tense
  deliverables inside QUEUED specs.
- **Stdlib/natives 43-member sample**: all present.
- **docs/EXTENSIONS.md** byte-identical to `phg extensions --docs`.
- **Markdown relative links**: 0 dead across every tracked `*.md`.
- **Scripts**: every hook/CI/doc-referenced script exists; no orphans.
- **`#[ignore]`d tests**: all carry tracking notes. **Dead feature gates**: none (22 cfg names
  all exist).
- **Register parity percentages** (≈68/69/53) consistent across MASTER-PLAN, M-gap-matrix §4,
  SLICE-STATE; wave assignments consistent across the three planning docs; the six 2026-07-26
  rule specs all exist with the referenced sections.
- **`E-CONCURRENCY-NO-PHP`** exists at `src/transpile/expr.rs:548`; `.claude/settings.json` is
  allow-list-only (DEC-354); ask-human skill consistent with DEC-387.
- **W5-13 interpolation fault-line caveat** reproduced live (still current); SECURITY 8 MiB body
  cap accurate; toolchain.env php-8.5.8 accurate; README "no tower-lsp/serde" accurate; ci.yml
  "four Phase-2 cross targets" and SEMVER "four platform archives" accurate.
- **DEC-226 vs DEC-255** — the one supersession chain recorded *with* an explicit correction
  note; the model the other chains should follow.

---

## PENDING-question inventory (reproduced from Lens C — feeds `QUESTIONS.md`)

### Genuinely OPEN adjudications (developer ruling owed) — 10 → **6 remain**

> **Batch 1 RULED by the developer 2026-07-29** — items **9 → DEC-390** (DEC-383 closed as
> bookkeeping; build-order 7.5 is a BUILD slice), **5 → DEC-391** (`srcs` ratified), **1 → DEC-392**
> (as-built wildcard visibility ratified, D3's wording rewritten to the unifying principle) and
> **4 → DEC-393** (pipe-lambda trailing-op fork closed, loud error stays). All four were Option-1
> recommendations, accepted as recommended. Remaining: items 2, 3, 6, 7, 8, 10.

| # | Item | Where recorded |
|---|------|----------------|
| 1 | **L-19 / P-Q-A-2** — confirm D3's ruled wording ("public+internal" cross-package binding) | `C-decisions.md:3686`; `2026-07-24-wildcard-imports.md` §PENDING; build-order "Still OPEN" |
| 2 | **L-22 / DEC-334** — runtime-config catalog (php.ini-equivalent knob enumeration; multi-round research with the dev) | `C-decisions.md:3104`, `:3686`; MASTER-PLAN (DEC-334 QUEUED campaign) |
| 3 | **L-25 / DEC-320-F2** — `phpInterop { namespaceRoot, sourceRoot }` / `App\`-prefixing knob: transpiler-wide namespace plumbing vs no-prefix law for GA | `C-decisions.md:2805-2809`, `:3686`; `MASTER-PLAN.md:96` |
| 4 | **L-28** — pipe-lambda trailing tight-op binding (`x \|> (v => v) + 1`): keep loud `E-PIPE-LAMBDA-CONTEXT` or bind trailing ops to the pipe result (additive) | `C-decisions.md:1230-1236`, `:3686`; `MASTER-PLAN.md:441` |
| 5 | **L-31** — `VirtualModule.src`→`srcs` rename | `C-decisions.md:3046`, `:3686` |
| 6 | **L-33** — DEC-324's 7 remaining TOP items (php-gap round-2) | `C-decisions.md:3686`; `MASTER-PLAN.md:65-85` |
| 7 | **L-86** — DB column naming (slice B2) + cross-prelude error-namespace convention | `C-decisions.md:3686` |
| 8 | **DEC-366 ride-along** — does the lifter hoist ride in the DEC-339 slice or get its own? Asked 3×, unanswered; provisional default adopted by the build order without a ruling | `C-decisions.md:3503` (status **PENDING**); `SLICE-STATE.md:226-228`; build-order 1.1 |
| 9 | **DEC-383 forks (a) and (c)** — queued for ruling (Wave 7.5), **but see C-001**: (a) = DEC-205 and (c) = DEC-204 appear already ruled 2026-07-12; reconcile before re-asking | `C-decisions.md:3649`; build-order 7.5 |
| 10 | **`maxBy`/`minBy` representation lever** (0.19-0.20× hard flag) — nullable arena kind / non-empty-list peephole / accept the flag; "not a night call … dev to rule" | `C-decisions.md:3230-3236` |

Items 1-7 are the register's own "Still OPEN — deliberately not ruled" seven
(`C-decisions.md:3684-3693`, developer-approved to defer 2026-07-26); 8-10 are open outside that
list.

### Owed measurements (developer-facing verdicts, "none optional" — build-order tail) — 5

1. **DEC-339** — count of `examples/`+`tests/` sites the redeclaration rule breaks (report
   BEFORE migrating).
2. **DEC-357** — in-tree captured-local write scan (any hit = a bug found).
3. **DEC-365** — the two owed bench verdicts: `floatloop` (WIN→LOSS on discarded cpuset) and
   `queryparse` (0.146 vs DEC-338's ~0.88× ⇒ DEC-338's near-parity claim stays UN-CERTIFIED).
   Dev-box run required.
4. **DEC-370** — copy-at-boundary cost + per-thread interpreter/VM instantiability (before the
   parallelism slice).
5. **DEC-377** — classification of all 168 `__phorj_*` helpers into the 3 buckets.

### Auto-ruled, explicitly REOPENABLE (developer may ratify or reopen; not blocking) — 3

- **DEC-224** MongoDB — admission shape ruled, build deferred (`C-decisions.md:1031`).
- **DEC-225** concurrency PHP leg — hard error stands; PHP 8.1 **Fibers** recorded as the first
  non-downgrading candidate, spike-gated (`:1044`).
- **DEC-226** `#[UncheckedOverflow]` transpile — stays `E-TRANSPILE-UNCHECKED`; its "checked
  default transpiles faithfully" claim was corrected by DEC-255 (`:1961/:1971`).

### Bookkeeping owed (no design question) — 2

- Record the Appendix-A **PENDING-REJECT** rows (SOAP · IMAP · SNMP · dba+SysV IPC ·
  pspell/enchant · ext/calendar · tidy; +LDAP as candidate) — `MASTER-PLAN.md:82-85`; currently
  still silent drops.
- `/qa-sweep` CLI-mode-only import — queued after Wave 0 (DEC-388.5).

### Recorded-as-PENDING but actually RESOLVED (do NOT re-ask — flip the label instead)

serve-TLS posture (→DEC-331 D7) · DEC-208-S2 `queryInto` + DEC-220 S2/S3 (shipped) ·
`using`/`defer`, `Runtime.onShutdown`, Rc cycle-leak (→DEC-203/204/205 — subject to the C-001
reconciliation) · W4-10 XML (→DEC-382) · DEC-197 (→DEC-274) · DEC-200 (→DEC-202) · DEC-216
(→DEC-316) · DEC-286 (→DEC-380) · P-Q-B-1 (→DEC-379) · P-Q-A-1 (→DEC-384) · P-Q-A-3
W-UNUSED-IMPORT (→DEC-360) · P-Q-A-4 group-sort (→DEC-386) · P-Q-A-5 Inv-13 debt (→ done
2026-07-25) · response-CRLF (→DEC-363) · DEC-208 retry surface (→DEC-249) · native IntelliJ
plugin scope (→DEC-181) · the five register "ALL PENDING" headers (C-011) · Q-J1…Q-J8
(→DEC-354/386/388) · DEC-247 tz-dep (per L-36 "already done" — reconcile per C-020/Q4).

## Certification-panel addendum (DEC-268 round 1, 2026-07-29)

The 3-lens fresh-context panel (correctness+regression / security+safety-promises /
completeness+blast-radius) reviewed the applied fix batch and returned **20 findings — all fixed
before commit**. The ones that add NEW knowledge beyond the fix batch:

- **`phg build --target` downloads cross-compile stubs over HTTPS on cache miss**
  (`src/bundle/cross.rs` via `curl`, sha256-verified against a baked manifest before cache
  publish; `PHORJ_CURL`/`PHORJ_STUB_REGISTRY` select the binary/registry). The "PM verbs are the
  only network commands" claim — including the one this audit first wrote — was false; corrected
  in SECURITY.md / README / FEATURES.md. None of the audit's five lenses had found this path.
- **The DEC-316 PM git path lost the retired vendor path's argument hardening** (no `--`
  separator, no `protocol.ext.allow=never`, no `ext::`/`file::` rejection — `src/pm/fetch.rs`
  passes `git`/`ref` as given; the retired path's guards were verified property P6 —
  `2026-07-03-unification-audit/raw/A7-security.md`, shipped per CHANGELOG under 1.0.0-nightly.0,
  2026-06-24 — and also scrubbed `GIT_*` env, which the live path inherits). Recorded as
  KNOWN_ISSUES §Rich-Request item 4b; re-porting the guards = audit **Q28** (queued hardening
  slice).
- **The JIT unsafe island is ~48 audited sites** (fn-ptr call + `extern "C"` trampoline pointer
  derefs), not "a single call" — magnitude corrected in SECURITY.md / THIRD-PARTY-NOTICES /
  Cargo.toml; confinement + CI gate claims verified TRUE.
- **`E-VENDOR-MISSING`'s `phg explain` text still taught `phg vendor`/`[require]`** (corrected to
  DEC-316) — and the code has ZERO emit sites (dead diagnostic; DEC-362-guard class, see Q25's
  dead-surface family).
- Residual `import type`/vendor-era teaching in `examples/project/` (wildcard-imports,
  visibility, inherit, withdeps) — all corrected; `docs/adr/0001`'s rename aside had the
  `cmd_run`/`cmd_runvm` swap backwards — corrected; THIRD-PARTY's transitive license enumeration
  gained ISC / Unicode-3.0 / 0BSD / BSL-1.0-option (full 305-package lockfile sweep: zero
  copyleft, headline claim holds).

Process note: beyond FIXES.md and the three fixer reports, a fourth (parent-session) pass edited
`.github/PULL_REQUEST_TEMPLATE.md`, `docs/plans/SLICE-STATE.md` (cursor), and six `src/` files
(the `import type` comment sweep) — same disease classes, attributed here for the record. The
build-order 7.5 row carries the Q1 tension note, and (round 2) the MASTER-PLAN DEC-203/204 and
DEC-205 mirrors now cross-reference Q1 explicitly instead of silently answering it.

## Certification-panel round 2 (2026-07-29) — 20 further findings, all fixed in the follow-up commit

Highlights beyond round 1: `E-MODULE-NOT-FOUND` explain/hint still taught the pre-DEC-316 story
(fixed in `imports_casts.rs` + `loader/entry.rs`); `CLAUDE.md` Invariant 10 and three more
surfaces missed the `build --target` network disclosure; SECURITY.md now flags `PHORJ_REGISTRY`,
`PHORJ_STUB_MANIFEST` (the trust anchor of the stub sha256 check), `PHORJ_OBJCOPY`, and the
`GIT_*` env inheritance, and scopes the runtime outbound surface (non-default `http-client`/
`mail`/DB features); `STABILITY.md` still said "exactly these four" deps and listed retired
`vendor` as experimental; `tests/project.rs` taught `import type` in 7 doc-comments;
`withdeps/README` self-contradicted after the round-1 fix; `selftest/README` claimed a
`phorj.toml` root marker (it is `src/` — DEC-282). ERRATA for this report: (1) all
`C-decisions.md:NNNN` line references cite PRE-batch coordinates — the fix batch inserted +112
lines into that file, shifting everything after line ~58; re-derive by content, not line. (2)
The batch commit message says "one phg-explain string"; two explain strings changed
(`E-VENDOR-MISSING` + `E-VIS-INTERNAL`). (3) The addendum's original P6 cite ("CHANGELOG
~2026-07-03") was wrong: P6 lives in `2026-07-03-unification-audit/raw/A7-security.md`; the
CHANGELOG hardening entry sits under 1.0.0-nightly.0 (2026-06-24). (4) "(deleted; see git
history)" annotations: this clone is shallow (grafted 2026-07-19) — the deleted specs are only
in UPSTREAM history; annotations now say so.

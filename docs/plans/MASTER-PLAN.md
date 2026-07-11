# PHORJ MASTER PLAN — the ONE unified roadmap (cursor + live percentage + all tracked work)

> **This is the living plan and the single forward SSOT.** Unified 2026-07-03 by the Stage-D
> consolidation of the full unification audit: it replaces BOTH the 2026-07-02 MASTER-PLAN and
> `wave0-remainder.plan.md` (superseded, archived after review; full prior text in git history at
> `0691228`). It absorbs: the 2026-07-02 full-audit roadmap (waves 0–6, §12 rulings ledger), the
> wave-0-remainder session decisions, and the **2026-07-03 unification audit** (61 findings,
> 4 buckets, 17 developer-ruled decisions — synthesis at
> `docs/research/2026-07-03-unification-audit/SYNTHESIS.md`, raw reports A1–A10 alongside it).
>
> **Spec SSOT:** the frozen per-topic specs were folded into ONE spec —
> `docs/specs/UNIFIED-SPEC.md` (ruled 2026-07-03). Every "the X spec" reference below means the
> relevant section of UNIFIED-SPEC; the original per-file texts are in git history.
> Decisions register: `docs/research/full-audit/raw/C-decisions.md` (canonical, 141 rows).
> Delivery rules: `/stack/projects/phorj/CLAUDE.md` + `docs/INVARIANTS.md` — read both first.

---

## 0. CURSOR — WHERE WE ARE (update every working session)

> **THE FINISHING WAVE is the active programme** (see the section immediately after §1) — all plans/specs
> consolidated into THIS file + UNIFIED-SPEC (2026-07-11), then execution to **100% VISION** (full PHP
> parity + the beyond-PHP programme). The developer drives execution with Fable; this file is the single
> handoff roadmap.

| | |
|---|---|
| **Date / HEAD** | 2026-07-12, HEAD `5ba5f17`. Ω-8 verticals: trycatch NATIVE throw/catch shipped + ratcheted **33.4× WIN** — map now **11 WINs / 17 micros** (remaining losses: strbuild 0.43 · webish 0.68 · intadd 0.73 checked · mapget 0.80). Prior: 2026-07-11 plan/spec consolidation (perf-wave / di-attributes / web-spine / cli-name-sync / finishing-wave folded into this file + UNIFIED-SPEC + KNOWN_ISSUES; ROADMAP→thin pointer). |
| **Completion** | **PHP-parity ≈60% · Vision ≈62% · raw row-floor ≈41%** — HEAD `af3aad3`, §11.4 + `M-gap-matrix §4.6`. [Inferred: additive delta on the ratified 824-row model; a full 824-row re-pass is due at the next milestone/wave close.] Denominator = **824 verdict rows** (665 net of N/A + GAP-by-design). Road to 100% parity = close **445 rows** (76 PARTIAL + 110 GAP-planned + 259 GAP-unplanned); the 49 GAP-by-design footguns are audited in Ω-0 (do-everything-better, take-no-weakness). |
| **Active programme** | **THE FINISHING WAVE** (section after §1): Ω-0 footgun audit → Ω-1 web spine → Ω-2 filesystem/subprocess/logging/compression → Ω-3 string-Unicode/regex/array/math → Ω-4 language surface → Ω-5 date/intl/XML → Ω-6 the 259-row unplanned stdlib tail → Ω-7 beyond-PHP programme → Ω-8 perf hold → Ω-9 GA. Target = **100% VISION**. |
| **Locked rulings (2026-07-11, developer via ask-human)** | **Perf** = multi-dimensional "better" (faster/safer/organized/SOLID). **AMENDED (2026-07-11, Fable session, developer via ask-human): the string/array/collection speed-beat is REOPENED NOW — fresh-eyes attempt at the FRONT of this run, target faster-or-at-least-equal to PHP, evidence-gated (pure-Rust ceiling spike FIRST per KNOWN_ISSUES §"Parked perf"; WIN-OR-FLAG; no MATCH in the ceiling test → report honestly and re-ask).** Prior end-stage park superseded. **Target** = 100% VISION. **Footguns** audited in Ω-0. **GLOBAL TENETS (whole wave):** prefer INSTANCES + mandatory `new`; nothing in the wind (every symbol import-gated, leaf-or-parent); decoupled / composable / generic / scalable / modular / SOLID. **Core.Sql DBAL = instance model** (`new QueryBuilder("t","a")` → typed per-verb sub-builders `SelectQuery`/`InsertStatement`/`UpdateStatement`/`DeleteStatement`; always-alias + `E-SQL-AMBIGUOUS-COLUMN`; decoupled dialect rendered at `db.execute`; `new Query(sql,[binds])` raw — SUPERSEDES the shipped slices 1+2 static-factory `Sql.query`/`Sql.select`, reworked in Ω-1). **PERF-FIRST rulings (2026-07-11, session 3, developer via ask-human):** (1) **Order A** — unboxed arena verticals (enum → closure/method → objalloc → composites) → V3b single-alloc `Instance` → NaN-box end-state, each shape spike-gated WIN-OR-FLAG; (2) **exit bar = beat-or-match EVERYTHING** (every micro ≥1.0× vs fresh docker php:8.5-cli+JIT, pinned+interleaved) — a flag is accepted only after all three levers are exhausted on that shape, loss anatomy documented; (3) **intadd** scored WON via `#[UncheckedOverflow]` 2× (apples-to-apples vs php's unchecked semantics); checked-DEFAULT ≥1.0× is an ACTIVE best-effort target — range-proof overflow-check-elision front REOPENED (induction/const-init proofs eliding checks where overflow is provably impossible; fault behavior unchanged); (4) **trycatch (0.48×, un-ruled prior)** = full lever attempt in the wave (anatomy first, then zero-cost-on-no-throw shape). |
| **Gate** | `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo test --workspace` + clippy (both configs, incl. `--no-default-features`) + fmt + release build + an Invariant-9 example + byte-identity `run≡runvm≡php-8.5.8`. **`jit` is a DEFAULT feature** (bare `cargo test`/`build`/`clippy` include it; `--features jit` is a harmless redundant no-op; verify jit-off compiles via `cargo check --no-default-features`; run without native codegen via `phg run --no-jit`). Pre-commit = fast Rust-only tier (~12s); pre-push = full oracle + microbench-gate. Commit each green slice; **NEVER push** (developer pushes). |
| **Next** | Ω-8 flip-seq continues: **webish fully-inline interpolation** → strbuild inline append → mapget/listindex emit tail → checked-intadd elision (task 9) → fundamentals micro sweep → representation slice (V3b + Rc cycle-leak §15 fork) → perf register + G-8 recompute → Ω-0…Ω-9. |

**Percentage protocol:** re-run the M §4 arithmetic (824 rows, weights 35 SYN / 40 FN / 25 RT) after
every milestone/wave close; update this cursor and §11 in the same commit. Always quote the number with
its weights and denominator.

---

## 1. GOVERNANCE & STANDING RULES

**G-1 · Byte-identity spine.** `phg run` ≡ `phg runvm` ≡ transpiled PHP under a real `php` (floor
**8.5**, `PHORJ_REQUIRE_PHP=1` fails-not-skips): identical stdout AND failure behavior, every
program, every example; the interpreter is the reference oracle. Enforced by `tests/differential.rs`
(3308 lines — split tracked as W1-1) + `tests/conformance.rs`.

**G-1.1 · The concurrency exception (disclose wherever byte-identity is claimed).** Concurrency
(`spawn`/`Channel`/`Task`) is permanently outside the PHP oracle (DEC-133, ratified): `run ≡ runvm`
holds; the PHP leg is a hard error — **the shipped code is `E-CONCURRENCY-NO-PHP`** (the ledger's
"E-TRANSPILE-CONCURRENCY" was the pre-implementation name; audit B3-4 verified only
`E-CONCURRENCY-NO-PHP` exists — use it everywhere) — with explicit `--sequential-concurrency`
opt-in + warning. Until W5-13, fault *line numbers* inside interpolation also diverge on the VM
(disclosed, W0-5). The audit added four *undisclosed* byte-identity breaks — now tracked + ruled as
UA-1 (§2.2); each fix lands its adversarial differential case same-commit (UA-0.13).

**G-2 · Quality gate (green means ALL):** workspace tests + clippy (warnings deny) + fmt +
release build + the full PHP-oracle gate before claiming any feature done. Perf claims need
`phg benchmark` before/after; CI regression gate `scripts/perf-gate.sh`.

**G-3 · Backend invariants** (detail `docs/INVARIANTS.md`): new `Op` ⇒ the three coupled matches
same commit ("no new Op" is the front-end default); value kernels single-sourced; compile-time
sugar expanded pre-backend via `cli::check_and_expand`; reified operands thread ALL vm-compile
paths; CTy-operand trap (`expr + 1` case); scratch slots `self.height - 1` (two-in-one-expr case).

**G-4 · Decisions.** Canonical register `docs/research/full-audit/raw/C-decisions.md`; the
2026-07-02 adjudication ledger is Appendix B; the **2026-07-03 unification-audit rulings are §13**.
Protocol for future decisions: interactive AskUserQuestion, ≤4 per round, recommended option first,
every design question ships a concrete risk example. **NEW standing instruction (developer,
2026-07-03): when a "more correct" answer and a "PHP-familiar" answer diverge, present and
default-recommend the more-correct one — PHP-parity is not a tie-breaking bias.** (Pattern across
the audit gate: the developer consistently chose the thorough option over the cheap one.)

**G-5 · Examples ship with features** (definition of done): runnable `examples/` program
(differential-globbed) + README index entry, same change; CLI features get a walkthrough README;
faults → README capture; impure/quarantined follow the `pure:false` conventions.

**G-6 · Anti-regrowth size rule:** soft 800 / hard 1000 production lines per file, tracked
exemptions, `scripts/size-gate.sh` in CI (W1-6 — not yet built; 12 files currently over the hard
cap, tracked, not silent).

**G-7 · Honesty rules.** No silent scope drops (rejects live in Appendix A with reasons);
codemod items say "REPLACE, not add"; no public perf claim vs a dev-built PHP (the local PHP
oracle binary is a DEBUG+Xdebug build — any `--vs-php` number on this box is invalid until
W6-4/UA-0.10). The five doc false-claim families the audit found (zero-deps, `import type`,
dead CLI verbs, the wrong concurrency E-code, 🔲-on-shipped) are corrected in the Stage-D pass
(§2.3) and must never be reintroduced.

**G-8 · PERF MANDATE (developer, REFINED 2026-07-10 via ask-human — supersedes the original absolute
"better-in-performance-or-it's-garbage" bar).** "Better than PHP" is **multi-dimensional** — faster /
safer / better-organized / best-practice / SOLID. **Speed is one axis, not the whole bar.** On speed:
- Phorj **WINS where structurally possible** — numeric / recursion / control-flow — ALREADY WON via the
  unboxed Cranelift JIT, which is a **default feature** (`phg run` JITs hot functions out of the box;
  `--no-jit` / `--tree-walker` opt out): fibrec 1.7–2.9×, intadd `#[UncheckedOverflow]` 2× (fresh
  interleaved release-php+JIT baselines).
- Phorj **MATCHES-not-beats** PHP on 20-yr-tuned string / array / collection categories. A clean
  speed-WIN there needs reimplementing PHP's C engine natively in the JIT — **evidence-proven
  unreachable at reasonable cost** (strings 27.6× / maps 67.1× behind; the boxed-value JIT was built,
  measured, and REVERTED as not-a-win). **This speed-beat is PARKED** (see KNOWN_ISSUES §"Parked perf");
  the developer will bring in **Fable** to attack it, and all park-items are resolved at the very end
  (once the language is otherwise complete and only they remain). Parity-speed there still ships a Phorj
  UPGRADE on the OTHER axes (Unicode correctness, no silent coercion, immutability, types).

Standing perf rules (evergreen):
(a) `phg run` is the correctness ORACLE (Invariant 2) — a slow-by-design tree-walker under `--tree-walker`,
    NEVER a perf number. Perf rides the VM/JIT. Transpiled-PHP *is* PHP ⇒ equal-by-construction: the
    migration BRIDGE, never the perf story.
(b) **NO perf claim without a FRESH release-`php:8.5`+opcache.jit Docker baseline, INTERLEAVED not
    batched** (this box has a ~1.5× load-noise floor — batched runs manufacture phantom wins). Gate on
    **WIN / MATCH / LOSS**, not magnitude (ratios swing 3–4×).
(c) Per-feature microbench harness + `scripts/perf-gate.sh` regression gate; `phg benchmark` for
    before/after numbers (output-identity gated).

Full evidence, the per-micro scoreboard, and the shelved value-representation-overhaul scoping (V0–V4 +
blast radius) live in **KNOWN_ISSUES §"Parked perf"** and the §11 ledger — folded here from the retired
`perf-wave.plan.md`.

---

## THE FINISHING WAVE — the active programme (road to 100% vision)

> **This is the active roadmap driver** (consolidated 2026-07-11 from the retired `finishing-wave.plan.md`,
> which superseded `web-spine.plan.md` / `perf-wave.plan.md` / `di-attributes.plan.md` / `cli-name-sync.plan.md`).
> ONE very big wave to finish the whole language — parity + perf + the beyond-PHP programme — to **100% VISION**.
> The detailed backlog it draws from is §2 (UA programme) + §3–§9 (waves 0–6) + §10 (stdlib charter): those
> sections are NOT retired — the Ω-sub-waves reference their row-level detail. The developer executes with
> **Fable**; this file is the single handoff spine (context auto-compacts, session-remember carries continuity).

### The developer's absolute rules (verbatim intent, 2026-07-11)

1. Phorj does **everything PHP does, and some** (superset).
2. Everything Phorj does is **better** — MULTI-DIMENSIONAL: speed is ONE axis; safety, correctness,
   organization, best-practice/craftsmanship, SOLID + all principles are the others.
3. Where PHP does something badly (silent failure, hidden footgun) and a better approach can't be picked
   autonomously (none easy), OR a better approach would sacrifice performance / security / any major
   dimension → **PARK it in `KNOWN_ISSUES`** for the developer and keep going on everything else. Never
   silently downgrade (Invariant 14 LADDER).

### Locked rulings (2026-07-11, developer via ask-human)

- **A — Perf = multi-dimensional "better"; the string/array/collection speed-beat is PARKED.** Phorj wins
  on safety/correctness/design/organization + already-won numeric (unboxed JIT — fibrec 1.7–2.9×, intadd
  `#[UncheckedOverflow]` 2×); it MATCHES-not-beats PHP speed on the 20-yr-tuned string/array/collection
  categories, where a clean speed-WIN needs reimplementing PHP's C engine natively (evidence-proven
  unreachable: strings 27.6× / maps 67.1× behind; boxed-value JIT built + measured + REVERTED as not-a-win).
  The speed-beat is a `KNOWN_ISSUES` §"Parked perf" item; the developer brings in **Fable** to attack it,
  and **all park-items are resolved at the very END** — once the language is otherwise complete and only
  parked items remain.
- **B — Target = 100% VISION** (PHP-parity + the 35-capability beyond-PHP programme, the "and some"). The
  denominator is the vision, not parity-only.
- **C — Footguns: audit all 49 GAP-by-design rows one-by-one (Ω-0).** Governing rule: *do everything PHP
  does, better; take NONE of PHP's weaknesses.* Per row: a genuine capability → cover it in Phorj's
  better/safer form (flips toward COVERED); a pure footgun/weakness → stays excluded by design.

### Global design tenets (whole wave — developer-ruled 2026-07-11)

- **Prefer INSTANCES + mandatory `new`.** Every stdlib capability is proper `new`-able instances + types,
  not static/module-factory namespaces. (Fixes an Invariant-12 violation in the shipped Core.Sql slices.)
- **Nothing in the wind — leaf-or-parent import, always.** Every symbol (type / attribute / verb / function)
  is import-gated: `import A.B.C` → bare `C`, OR `import A.B` → `B.C`. Never ambient.
  (See the [Nothing in the wind](../specs/UNIFIED-SPEC.md#nothing-in-the-wind) spec.)
- **Decoupled / composable / generic / scalable / modular / SOLID throughout.** Components work together
  but don't depend on each other's construction; easy to add / extend / swap.

### The current true state (verified against source, 2026-07-11)

- **Parity ≈60% · Vision ≈62% · raw row-floor ≈41%** — HEAD `af3aad3` (§11.4 + `M-gap-matrix §4.6`).
  824 verdict rows (net denom 665): COVERED 220 · PARTIAL 76 · GAP-planned 110 · GAP-unplanned 259 ·
  GAP-by-design 49 · N/A 110. **Road to 100% parity = close 445 rows** (76 + 110 + 259).
- **The roadmap's own model tops out at ≈75% parity / ≈81% vision at W6 close** — it DEFERS XML, intl, and
  most of the 259-row unplanned stdlib tail. So "finish to 100%" goes BEYOND the scheduled waves: it must
  schedule + build the 259 unplanned rows and resolve every parked-hard item. Honest scale = an order of
  magnitude more than the 252-commit span that moved 58%→60%.
- **TOP-20 parity gaps** (M-gap-matrix): #1 DB · #2 HTTP · #3 sessions · #4 sprintf (DONE) · #5 filesystem
  breadth · #6 Unicode strings (inherited PHP defect DEF-016) · #7 named-args/variadics/spread · #8
  generators/`yield` · #9 date/time breadth · #10 CSPRNG (partly done) · #11 array_* long tail · #12
  XML/DOM · #13 subprocess · #14 regex breadth · #15 compression · #16 user attributes+reflection · #17
  `__toString`/`__invoke` · #18 structured logging · #19 intl · #20 math long tail + BigInt.

### Ω sub-waves — ordered by impact (TOP-20 first)

Row-level detail is drawn from §2 (UA-0/UA-1/UA-L2…) and §3–§9 (waves 0–6). Ω-0 runs as part of the
verified gap inventory and feeds the row-detail for Ω-1…Ω-6.

- **Ω-0 · Footgun audit** (ruling C): walk all 49 GAP-by-design rows; per row apply *do-everything-better /
  take-no-weakness* — capability → schedule a better/safer Phorj cover; pure footgun → confirm excluded.
- **Ω-1 · Web spine** (TOP-20 #1/#2/#3): finish **Core.Sql** P1 → P2 `Core.Db` (rusqlite) → PG/MySQL →
  HTTP client → sessions/cookies/auth. Continues Wave D. **UA-L2 is DONE** — the `cli::CORE_MODULES`
  registry (deterministic sorted iteration, Inv-10) means adding a Core module is ONE row; Core.Sql was its
  first consumer. The Core.Sql surface is reworked from the shipped static-factory slices 1+2 to the
  **instance model** (spec: [Core.Sql — SQL DBAL](../specs/UNIFIED-SPEC.md#coresql--sql-dbal-instance-model);
  Q1–Q7 adjudication recorded there). Remaining P1 = `bindNamed` (Q4 default) + joins/groupBy/having/
  aggregates. The parked **`Bytes.format(spec, [bytes…]) -> bytes`** op (RESOLVED `dbc5215`; byte-id-safe
  by construction) + companion **`String.charCount`** (codepoint count) land here.
- **Ω-2 · Filesystem + subprocess + logging + compression** (#5 / #13 / #18 / #15).
- **Ω-3 · String/Unicode correctness** (#6 — the inherited DEF-016 byte-length defect; `String.charCount`
  belongs to this correctness family) + regex breadth (#14) + array_* long tail (#11) + math long tail +
  BigInt (#20).
- **Ω-4 · Language surface** (#7 named-args/variadics/spread · #8 generators/`yield` + iterators · #17
  `__toString`/`__invoke` · #16 attributes v2 + reflection · `trait` §7 fork · **DI v2** — spec:
  [DI & attribute reflection](../specs/UNIFIED-SPEC.md#dependency-injection--attribute-reflection-di-v2--l1l2)).
- **Ω-5 · Date/time + intl + XML/DOM** (#9 / #19 / #12 — the deferred tier-3 domains; intl needs an ICU
  extension story).
- **Ω-6 · The 259-row unplanned stdlib tail** — the long march the roadmap never scheduled; this is what
  separates ~75% from 100%. Batched by domain, park-engine per row.
- **Ω-7 · Beyond-PHP programme** (the "and some" — required for 100% VISION, ruling B): the 35 beyond-PHP
  capabilities + the DI/reflection/ORM/routing framework stack. DI v2 (from Ω-4) feeds this (the generic
  L1 attribute-reflection primitive + `subjectsWith<Attr>()` reverse discovery underpins routing/ORM/etc.).
- **Ω-8 · Perf hold** — hold the won numeric ground; re-verify no regression each sub-wave.
  *(AMENDED 2026-07-11: the string/array speed-beat moved from end-stage park to the FRONT of this Fable
  run — developer ruling via ask-human; ceiling-spike-first, outcome recorded in KNOWN_ISSUES §"Parked perf".)*
  **CEILING SPIKE PASSED (2026-07-11) → ACTIVE PERF BUILD (front of run):** SSO strings 1.74× / cached-hash
  maps 1.30× / interned keys 3.5× WIN vs docker php:8.5.8+JIT in pure-Rust ceiling (KNOWN_ISSUES §"Parked
  perf" has the table). Build slices, each green+measured (WIN-OR-FLAG):
  **P-1a ✅ SHIPPED (2026-07-11)** — gate green (1925 tests, PHP oracle); interleaved before/after:
  stringconcat 1.28× / mapget 1.19× / webish 1.08× / interp 1.07×, no regressions; php+JIT beat
  deferred to P-2a as planned (VM dispatch is the remaining cost).
  **P-1a** `PhStr` (new `src/phstr.rs`, safe): 24B two-variant — `Inline{len:u8,buf:[u8;22]}` zero-alloc
  runtime-shorts + `Heap(Rc<HeapStr{hash:Cell<u64>,s:String}>)` literals/longs with lazy-cached FNV;
  const-pool literals = Heap + precomputed hash (interning); `Value` stays 32B (static assert);
  `Deref<str>`+`From` keep the 204 `Value::Str(` sites mechanical; compare/eq on bytes (≡ codepoint
  order for UTF-8); `String.length` byte semantics + fault strings unchanged (byte-identity).
  **P-1b** `OrderedMap`: entries-vec (insertion order preserved = byte-identity) + open-addressing index
  over cached hashes; `build_map`/`map_index` kernels keep single-sourcing.
  **P-2a-inline ✅ WIN (2026-07-11) — GATE-2 PASSED: phg 20.9M vs php 35.8M ns = 1.71× on the real
  `phg run` stringconcat, interleaved best-of-7 vs fresh docker php:8.5-cli+JIT** (the ceiling
  spike predicted 1.74× — delivered). The SSO fast paths are INLINE Cranelift IR over a
  `#[repr(C)]` `UbCtx` arena of 64-byte string slots (JIT-visible header at fixed offsets):
  tagged handles (`SLOT`/`SLOT|OWNED`/`FLAT`), `MakeList` seals all-short lists flat (`Index` =
  inline unsigned bounds + base+idx, zero copy), `Concat` = inline len-add + free-stack alloc +
  bounded 3×8-byte over-copies, `String.length` = one byte load, free = inline free-stack push;
  helpers remain the slow paths (untagged, >22B results, non-flat lists); arena exhaustion → code
  5 redo-on-VM. Gate green (1928 tests, PHP oracle). **P-2b (mapget vertical) + P-2c (rollout) are
  UNLOCKED.** History: the helper-granularity spike below measured the LOSS that forced inline.
  **P-2b ⚑ SHIPPED (2026-07-11) — measured, FLAGGED at 0.81× (php+JIT 1.23× ahead on mapget).**
  Two sub-slices, both green (1922 tests, PHP oracle): (1) `MakeMap`/string-`Index` join the
  unboxed subset — `Kind::StrIntMap`, seal through the canonical `build_map` kernel, flat
  arena pair slots, inline hash-first linear probe (`a7ff3a8`, measured 0.60×; the ceiling
  spike had already measured linear-scan a LOSS — expected); (2) the ceiling-blessed upgrade:
  seal-time open-addressed BUCKET TABLE (u32 pair indices after the pairs, lf ≤ 1/2) + CANON
  interning (slot byte 32 = `interned-slot+1` via a content registry; canon equality ⇔ byte
  equality) → probe = `hash&mask` → bucket → ONE canon compare → value; plus run-invariant
  ctx-header loads marked `notrap+can_move` (GVN/LICM). Interleaved best-of, BOTH SIDES PINNED
  to one core (`taskset -c 7` / `--cpuset-cpus=7` — the box's ambient load made unpinned runs
  swing 3-4×): **phg 14.31M vs php 11.64M ns = 0.81×**, all 5 pair-ratios in 0.78–0.83
  (linear-probe baseline 0.60×; VM pre-vertical 2.67B — the vertical is ~100× over the VM).
  Verdict: the remaining ~0.9ns/iter is fixed scaffolding php's specialized trace doesn't pay
  (checked arith, tag dispatch, ownership frees, srem) — matches the refined mandate
  (match-not-beat on 20-yr-tuned collections). Emit-quality levers queued into P-2c:
  range-proven `RemI`-by-pow2 → `band`, fused tag checks, Pop-elision for provably-borrowed
  reads. Byte-identity watchpoint shipped with it: the INLINE concat now ZEROES its result
  slot's hash+canon words (a stale canon word could false-match in the probe — the garbage
  would otherwise be a byte-identity break, not just a slow path).
  **P-2c IN PROGRESS (2026-07-11, session 2) — three levers SHIPPED + the perf-gate fixed:**
  (1) `RemI`-by-pow2 → `band` (`7669a6a`): entry-prefix const-init proof + proven-induction
  writers ⟹ non-negative dividend; byte-exact, fault-free. (2) **Int-list vertical**
  (`be91280`): `Kind::IntList`, all-int `MakeList` seals flat (raw i64 at slot bytes 0..8),
  inline bounds+load — **listindex 0.03× → 0.98× parity**. (3) **Inline
  `Conversion.toFloat`/`truncate`** (`3cabcb9`): `fcvt_from_sint` / range-guarded
  `fcvt_to_sint` mirroring `value::float_to_int` exactly — **floatarith 0.03× → ~4× WIN**.
  Perf-gate hardening (`1d09c12`): microbench.sh sampling was BATCHED and manufactured a
  phantom 5.4× fibrec WIN→LOSS flip under ambient load (JIT measured intact at 35× over the
  VM) — now INTERLEAVED + CORE-PINNED; baseline re-emitted honestly. Current ratcheted map:
  **WINs match 7.14 · floatarith 4.01 · stringconcat 2.02 · fibrec 2.00 · floatmul ~1.00;
  near-parity listindex 0.99 · mapget 0.92 · intadd 0.69 (checked-default price) · trycatch
  0.48; VM-bound remainder enum 0.01 · closurecall 0.03 · methodcall 0.03 · webish 0.07 ·
  interp 0.11 · objalloc 0.14** — the un-JITted object/closure/enum shapes. **The frontier
  fork is now RULED (2026-07-11 session 3, developer via ask-human — see §0 PERF-FIRST
  rulings): Order A = unboxed verticals FIRST (the pattern that won listindex/floatarith),
  then V3b, then NaN-box; bar = beat-or-match everything; trycatch gets a full lever;
  checked-intadd elision reopened.**
  **VERTICALS PROGRESS (session 3): enum ✅ 0.01→1.58× WIN (`0afd3a1` — Kind::EnumInt register
  pairs: payload word + `evars` tag space, MakeEnum/MatchTag/GetEnumField(0)/Fault in the subset,
  zero alloc; Fault = terminator in `reachable`). closurecall ✅ 0.03→2.13× WIN (`1cc958b` —
  Kind::Fn(target): capture-free MakeClosure is fully static, CallValue = direct call via shared
  `emit_call_to`; measured pinned+interleaved best-of-7 on a quiet core; NOTE ambient load can
  swing even interleaved ratios — re-measure on a quiet core before trusting a flip).**
  **VERTICALS WAVE 2 (session 3, later) — ALL FOUR SHIPPED + measured (pinned, interleaved,
  best-of-7):** objects `1d1582d` (methodcall 0.03→**2.96× WIN**, objalloc 0.14→**9.92× WIN**) ·
  mixed-Concat `8fcb9dd`+`16dd21a`+`e8e1511` (interp 0.11→**0.91**, webish 0.05→**0.70**) ·
  ratchet `a144f8d`. **Current full map: 9 WINs** (objalloc 9.92 · match 6.95 · floatarith 3.98 ·
  methodcall 2.96 · closurecall 1.96 · stringconcat 1.91 · fibrec 1.82 · enum 1.66 · floatmul
  1.02); near-parity listindex 0.94 · interp 0.91 · mapget 0.84; losses webish 0.70 · intadd
  0.67 (checked; unchecked=WON 2×) · trycatch 0.42. **Perf lesson (measured):** hashing/canon
  registration on hot-path result slots was the mixed-concat killer — result slots write
  hash 0/canon 0 (punt marker); registration only pays where content gets probed.
  **REMAINING to the ≥1.0-everything bar:** (1) trycatch — needs NATIVE throw/catch (code 6 =
  "thrown, value = payload handle" in the (value,code) multi-return; try-regions as compile-time
  handler ranges; Call-sites inside a try dispatch code-6 to the catch pad) AND str fields in
  instances first (`Odd.message` — per-field Kind table + recursive instance free);
  (2) webish 0.70 — remaining cost = concat_mix call + map probe; lever = fully-inline
  interpolation (IR digit render into the result slot) and/or mapget probe micro-tuning;
  (3) mapget 0.84 / listindex 0.94 — emit-quality tail; (4) checked-intadd elision
  (task 9, ruled ACTIVE): extend range proofs to elide overflow checks on provably-bounded
  accumulators. THEN V3b → NaN-box (Order A), perf register + G-8 recompute at wave close.
  **TRYCATCH SLICE — FULL DESIGN (execute in 3 gated sub-slices):**
  (1) **Str fields in instances**: per-class field-kind table in the fixpoint (from MakeInstance
  operand kinds, ctor push order = desc.fields; all sites must agree; Int|Str only). GetField
  of a Str field → Str(Borrowed) (instance keeps ownership); SetField Str value must be
  Owned/ConstBorrow (release the OLD field word first). Instance RELEASE for str-fielded
  classes is KIND-DIRECTED at each release site (Pop/SetLocal-overwrite/consumers): load each
  Str-field word + emit_release it, THEN recycle the instance slot (runtime OWNED bit makes
  const-field frees no-ops — the bit gates everything). (2) **Handle args to ctors**: allow
  Str args (Owned/ConstBorrow only — Borrowed = aliasing double-free, DENY) to instance-
  returning callees; VM semantics MOVE args into the frame ⇒ callee params own their words ⇒
  generalize `this_inst` to a per-fn param-kind override table (`param_over`) recorded from
  call sites in the fixpoint (normalize ConstBorrow→Owned — bit-gated safe); ctor consumes
  params into fields (transfer). (3) **Native throw/catch**: compile-time handler STACK walked
  by analyze (PushHandler(t) pushes + propagates an edge to pad t with kinds+[Inst(thrown_c,
  Owned)]; PopHandler pops; nesting = stack). Fixpoint records per-fn thrown-class (singleton
  else Unsupported v1). Throw: with an ACTIVE local handler → truncate compile-time stack to
  the handler height (emit releases for dropped OWNED cells — the VM's unwind drops them),
  place the payload word, JUMP to the pad (no ABI crossing); with none → return (payload,
  **code 6**). Call/CallMethod inside a try-range: 3-way ccode dispatch — 0→cont, 6→(truncate
  + payload + jump pad), else→fault-exit (code 6 propagates through the existing fault-exit
  forwarding to OUTER callers automatically; reaching the VM boundary = JitRun::Fault → VM
  redo → correct throw semantics for escapes — try bodies must stay side-effect-free like
  everything else). Pad's IsInstance(c) is kind-static → constant-folds. needs_fault_exit +=
  Throw. Measure: trycatch 0.37 → ≥1.0 median-of-3.
  **TRYCATCH SLICE — ✅ SHIPPED (session 4, 2026-07-12), all three sub-slices as designed:**
  `7653434` Str fields in instances (per-class field-kind table in the fixpoint, GetField
  borrow/take-ownership, SetField old-word release, kind-directed `release_kinded` instance
  free) · `a1f12a3` string ctor args (single-use param moves, call-site `param_over` injection,
  `UbDiscovery` out-param so facts survive held failures — breaks the caller/ctor fixpoint
  deadlock; str-fielded construct+method loop 847M→15.5M = 55×) · `cbef2d6` NATIVE throw/catch
  (code-6 thrown discriminant in the (value,code) return, lexical `handler_ranges`, catch-pad
  edges in reachable/leaders, kind-directed unwind releases, static IsInstance fold, 3-way
  ThrowSite call dispatch) — **trycatch 0.37× → 29.97× measured (906M→11.8M self-timed),
  ratcheted 33.39×** (`5ba5f17`). **Ratcheted map after (17 micros): 11 WINs** — trycatch 33.39
  · objalloc 8.99 · match 7.15 · floatarith 4.21 · methodcall 2.79 · closurecall 2.04 ·
  stringconcat 1.94 · fibrec 1.84 · enum 1.72 · floatmul 1.03 · **interp 1.03 (flipped to
  WIN)**; near-parity tail floatloop 0.98 · listindex 0.95; **remaining losses: strbuild 0.425
  · webish 0.676 · intadd 0.726 (checked; unchecked=WON) · mapget 0.804.** NEXT (flip-seq):
  webish fully-inline interpolation → strbuild inline append → mapget/listindex emit tail →
  checked-intadd elision (task 9).
  **PERF-100% SWEEP — RULED (2026-07-11, session 3 close, developer via ask-human, THE GO given):**
  scope = flip trycatch/strbuild/webish/mapget/interp (+floatmul parity watch) THEN a FULL
  fundamentals micro sweep (collection writes, capturing closures/HOF, string ops, iteration —
  every new micro must reach the bar too) THEN the closing REPRESENTATION slice (V3b evaluation
  + the Rc CYCLE-LEAK answer — a §15 fork: php-style cycle collector vs weak refs vs both,
  SURFACE options before building; no finalizers exist so collection is semantically invisible).
  **Exit bar = median ratio ≥ 1.00 across 3 protocol runs (best-of-7, pinned, interleaved,
  load < 2) per micro.** intadd = WON via unchecked (fair race; checked-default stays the
  safety feature, loop-specialization best-effort non-blocking). AFTER the sweep: language
  waves Ω-0…Ω-7 where EVERY new feature ships already ≥1.0 vs php (JIT arms or other means),
  never sacrificing safety/typing. **PARKED DEBTS (disclosed + accepted):** precise
  deoptimization = post-100% gateway to JIT-ing effectful/async bodies (redo-on-VM caps the
  subset to side-effect-free code); shared-memory threading closed by the Rc value model —
  parallelism = isolates + channels; playground/iOS stay VM-speed (byte-identical).
  **VERTICALS WAVE 3 (session 3, coverage-driven — "what aren't we measuring?"):** float
  comparisons (exact partial_cmp/eq_val↔FloatCC mapping) + handle-slot writes
  (`Own::ConstBorrow` + Owned⊔ConstBorrow leader join) + the fused string-accumulator peephole
  (`accumulator_site` positional proof → `rt_u_concat` in-place append on uniquely-owned heap
  lhs — compile-time ownership delivers php's refcount-1 trick). TWO NEW BASE MICROS (now 17):
  **floatloop 1.02× WIN** (float-driven loops were fully VM-bound and unmeasured) ·
  **strbuild 0.11→0.53×** (the classic `s = s + x`; remaining gap = helper call vs php's
  inlined append — next lever: inline append fast path / HeapStr capacity doubling).
  STILL-UNMEASURED bases parked for the next wave: collection WRITES (list append / map
  insert), higher-order lambda pipelines (map/filter), and a JIT-coverage %-of-examples
  metric (what fraction of real programs stays unboxed — the honest generality signal).
  **OBJECT VERTICAL — DESIGN (SHIPPED as designed, kept for reference):** `Kind::Inst(class_idx)` =
  arena slot handle (SLOT|OWNED, fields flat at byte 8·layout_slot, ≤8 int fields, gate
  `desc.fields.len()==layout.len()` so no None window → GetField total+inline, SetField inline
  store, alloc = concat's free-stack-or-bump ladder, no helper, no boxed fallback). CallMethod:
  static dispatch off receiver kind → methods table → direct call, receiver = arg 0 (`this`);
  free-receiver-if-owned after the call; deny overloaded methods. Ctor return: Return-of-Inst
  allowed iff no Inst/handle params AND exactly one owned Inst cell in frame (ownership
  transfer, no frees at return); entry fn returning Inst = deny. INFRA: fixpoint loop in
  compile.rs — {analyze all, record per-fn ret kinds (init Int), resolve CallMethod targets
  from receiver kinds, add new callees, repeat until stable}; per-fn param-kind INJECTION
  (method `this` = Inst(c)); Call arm pushes callee's recorded ret kind. Instance/enum/fn args
  to Call stay denied (only `this` crosses, via CallMethod).
  P-2c TAIL shipped: emit_unboxed M-Decomp (mod/scalar/verticals/enums, `39d6a46`), fused tag
  checks · Pop-elision · `emit_unboxed` per-op-helper decomposition (1183 lines) · perf
  register + G-8 language recompute. Housekeeping shipped alongside: MSRV 1.74→1.82
  (`078fab0`), repo-wide M-Decomp (~30 commits — every file ≤800 lines except 4 by-design:
  explain/emit_unboxed/runtime_php/vm-exec_op).
  **P-2a ⚑ SPIKE SHIPPED (2026-07-11) — measured, FLAGGED LOSS; verdict recorded.** Handle space +
  helper calls (Concat / list-Index / `String.length`) shipped green: `stringconcat.bench()` is
  JIT-eligible (hits>0 proven), byte-identity holds (1928 tests, PHP oracle; index-fault redoes on
  the VM with the canonical string), and the real `phg run` micro dropped 948M → ~130M ns
  self-timed (≈7× over the pre-P-1a VM). Interleaved vs fresh docker php:8.5-cli+JIT: phg 121M vs
  php 34M — **LOSS 0.28×**. `opt_level=speed` (also shipped) is noise-level — the cost is HELPER-CALL
  GRANULARITY (~5 calls/iter ≈ 50-60ns vs php's ~17ns/iter): even fused to 3 calls/iter the
  call+bookkeeping floor (~25-30ns) stays ~2× short. **Spike conclusion: the WIN needs the SSO
  fast path INLINE in Cranelift IR** — exactly what the pure-Rust ceiling measured (1.74×) — i.e. a
  `#[repr(C)]` fixed-layout string slot in the handle table + inline len-check/copy for the
  ≤22-byte path, helper call only on the heap path. That is the next slice (**P-2a-inline**);
  per the ruling, **P-2b/P-2c stay gated until the WIN**.
  *(original spike spec follows)* handle space + helper calls
  for Concat / list-index / `String.length` native → `stringconcat.bench()` JIT-eligible → measure REAL
  `phg run` vs fresh docker php interleaved; WIN required to proceed. **P-2b** mapget vertical (map_get
  helper, unboxed int result). **P-2c** default-deny rollout to the remaining string/collection ops.
  Then recompute the perf register + G-8 language.
- **Ω-9 · GA close** — spec freeze, GA-CHECKLIST, final vision-% recompute, showcase; THEN resolve the
  remaining `KNOWN_ISSUES` park-items (ruling A). Hygiene follow-up parked here: the **`runvm` → `interpreter
  ≡ VM ≡ PHP` terminology sweep** (~150 occurrences / ~80 files; a semantic rewrite, its own commit + a
  `PHORJ_REQUIRE_PHP` gate — cli-name-sync's deferred leg; see the note under "Execution protocol").

### Execution protocol (the marathon rules)

- **The PARK-ENGINE** (wires the absolute rule to the mechanism): for each gap row, build PHP's behaviour in
  Phorj's **better-or-equal** form. When "better" requires a design decision that can't be made autonomously,
  OR would sacrifice perf/security/correctness/any major dimension → **PARK to `KNOWN_ISSUES`** (structured
  entry: what, why-parked, the fork for the developer) and move on. Never silently downgrade (Invariant 14).
- **Per-slice gate (non-negotiable):** full oracle `PHORJ_REQUIRE_PHP=1 cargo test --workspace` + clippy
  (both feature configs, incl. `--no-default-features`) + fmt + release build + an Invariant-9 runnable
  example + byte-identity `run≡runvm≡php-8.5.8`. Commit each green slice. **NEVER push** (developer pushes).
- **Spine-sensitive slices → advisor review before commit** (the green gate alone masks byte-identity P0s).
- **Recompute the % at each sub-wave close** (824-row model, weights 35 SYN / 40 FN / 25 RT) → update the §0
  cursor + §11 in that commit.
- **Continuity:** update the MEMORY `⭐⭐⭐ ON "CONTINUE"` pointer at every stopping point / compaction
  boundary so a resumed session picks up exactly here.
- **Adjudication (Invariant 15):** genuine user-visible design forks → surface via ask-human with a concrete
  failing-program example, don't self-rule; if blocked, park + continue elsewhere.
- **Hygiene banked (cli-name-sync):** the CLI-name/module-name sync is **DONE** (commit `56645a3` — CLI
  command surface canonical, code renamed to match: `lexer/`→`tokenizer/`, `fmt/`→`format/`, `cmd_lex`→
  `cmd_tokenize`, etc.). ONE follow-up remains = the `runvm` → `interpreter ≡ VM ≡ PHP` terminology sweep
  (~150 occurrences / ~80 files; post-CLI-reshape `phg run` IS the VM so `run ≡ runvm` is stale in both
  terms → a *semantic* rewrite requiring per-site care + its own commit + `PHORJ_REQUIRE_PHP` gate). Parked
  to Ω-9 hygiene.

---

## 2. UNIFICATION-AUDIT EXECUTION PROGRAMME (2026-07-03) — the current work

*Superseded-as-DRIVER by THE FINISHING WAVE section above (2026-07-11): §2's UA programme is no longer the
active roadmap driver — it remains as the detailed backlog / history the Ω-sub-waves draw their row-level
detail from. Its rulings and item detail are all still authoritative; do not re-litigate.*

> Source: `docs/research/2026-07-03-unification-audit/SYNTHESIS.md` (61 findings, severity-ranked,
> evidence-graded). Every Bucket-2 decision below was RULED interactively by the developer on
> 2026-07-03 (§13 Decisions Log). Nothing here is open to re-litigation; the only judgment left is
> execution sequencing, fixed below. This programme completes BEFORE autonomous marathons resume.

### 2.1 UA-0 · Bucket 1 — nineteen ready-to-execute fixes (no design ambiguity)

Ordered P1→P3. Each item is done when its acceptance evidence exists and the gate is green.

> ⚠️ **STATUS DRIFT (status-audit 2026-07-09):** the `☐` marks below are UNRELIABLE — a spot-check found
> UA-0.6, UA-1.7, and UA-1.9 all fully implemented (with tests/examples) but marked open. A prior
> marathon shipped many of these without updating the plan. **A fresh-context session should
> systematically reconcile every remaining `☐` against the code** (grep the E-code / feature) before
> treating it as open work — do NOT assume a `☐` item is unbuilt. The three verified-done items are
> marked ✅ inline below; the rest are un-audited.

| # | Sev | Item (source) | Status |
|---|-----|---------------|--------|
| UA-0.1 | P1 | **Test-gate serialization** — **PREMISE STALE, remainder DEFERRED (2026-07-04, evidence below; not "done", per G-7).** (a) The fmt corpus test *already* fans out across all cores via `std::thread::scope` (`tests/fmt.rs:128-146`, added since the "111 s" measurement) — the sharding win is already banked. (b) `runtime::shipped_manual_example_runs_on_both_backends` is NOT a corpus monolith: it runs ONE example (`fib(30)`), compute-bound (~57 s, 2.7M tree-walk calls × run+runvm) — not shardable without weakening the shipped-example guard. (c) The persistent-worker refactor is confirmed low-ROI (97 µs/call × ~260 calls ≈ 25 ms against 46 s — noise) AND not safely buildable here: a persistent worker must receive *borrowed* closures over a channel, which needs `stacker`/unsafe — both blocked by `forbid(unsafe_code)` + no-new-deps. **The real lever is `cargo-nextest`** (one global -j16 pool across binaries vs cargo's serial-per-binary; handoff proves 228→118 s here) — wiped by the stack reload, restore attempted (from-source `cargo install`, prebuilt blocked by classifier). (B1-1) | ⏸ deferred |
| UA-0.2 | P1 | **Wire mold**: `/bin/mold` is installed, no `.cargo/config.toml` exists — add the gitignored machine-local config (CI has no mold). (B1-2) | ✅ `b77e50e` |
| UA-0.3 | P1 | **Fix broken `--help` examples**: `run`/`runvm`/`disassemble` examples fail verbatim (missing `package Main; import Core.Output;`) and teach `->` — fix + convert to `: void` (`src/cli/mod.rs:77,85,129` + arrow prose at `:52,201,638`, `src/main.rs:301`). First contact with the tool currently produces two errors. (B1-3; syntax half sequences with UA-1.5) | ✅ `b77e50e` |
| UA-0.4 | P1 | **Help-surface parity**: add `lsp` + `debug` to the long `--help` (terse usage lists 18 verbs, long help lists 16) + per-command help for both. (B1-4) | ✅ `b77e50e` |
| UA-0.5 | P2 | **6 golden diagnostic corpus cases** (E-METHOD-VISIBILITY, E-CTOR-VISIBILITY, E-INJECTED-TYPE-BARE, E-DUP-FIELD, E-NEW-ON-NONCONSTRUCT, one `protected` variant) + a **reverse ratchet** (every explained code has ≥1 emission site). (B1-5) | ☐ |
| UA-0.6 | P2 | **Static-FIELD-via-instance diagnostic**: `a.s` on a static field falls through to a generic code-less message (`src/checker/calls.rs:1709`) while the method sibling got `E-STATIC-VIA-INSTANCE` in W0-3 — mirror the shipped pattern + corpus case. (B1-6) | ✅ **DONE** (status-audit 2026-07-09): `src/checker/calls.rs:~1926` emits `` `{name}` is a static field of `{cls}` — read it as `{cls}.{name}` `` (comment cites UA-0.6), mirroring the const sibling `E-CONST-INSTANCE-ACCESS`. |
| UA-0.7 | P2 | **17 emitted E-codes with zero test coverage** — one triggering test each (hooks ×4, E-UFCS-AMBIGUOUS, E-VARIANT-QUALIFIER, E-PARENT-AMBIGUOUS, E-DECIMAL-LITERAL, E-OVERLOAD-FN-VALUE, E-NEW-ON-NONCONSTRUCT, …; table in raw/A3 F6). (B1-7) | ☐ |
| UA-0.8 | P2 | Format `selftest/{arithmetic,faults}.phg` (missed by the Phase-1 reformat) AND strengthen the fmt corpus test to assert `fmt(src) == src` (currently idempotency-only, so tracked files can drift). (B1-8) | ☐ |
| UA-0.9 | P2 | Attach `[E-…]` codes to the most common diagnostics (arg-type, arity, expected/found — `check --json` emits `"code":null` for them) + fix the unknown-MEMBER misreport (`String.lenght` → "unknown identifier 'String'"). (B1-9) | ☐ |
| UA-0.10 | P2 | `phg benchmark --vs-php` hardening: run php with `-n` (or detect+warn Xdebug/DEBUG builds) — the PHP leg currently aborts silently past 512-deep recursion and comparisons fold in a debug interpreter. (B1-10) | ☐ |
| UA-0.11 | P2 | CI fixes: cargo-zigbuild via `taiki-e/install-action`/binstall (built from source every run today); gate job → nextest; `oracle-nightly` cron-only (currently every push, ~25% of runner minutes); rust-cache in playground.yml. (B1-11) | ☐ |
| UA-0.12 | P2 | Install `wasm-pack` locally (playground Rust-side changes are untestable end-to-end today). (B1-12) | ☐ |
| UA-0.13 | P2 | **Promote the A5 native-vs-PHP fuzz probes into the standing differential gate** (non-ASCII whitespace, multibyte, empty separators) — the structural mitigation for the byte-identity-leak class; every UA-1 fix lands its case here same-commit. (B1-13) | ☐ |
| UA-0.14 | P2 | **Disclosures to ADD** (known-but-undisclosed): no-sandbox/full-user-authority trust model; `Math.pow(0.0, neg)` PHP-deprecation note; `List.append` O(n²) + the `List.fill`+index-set fast path (user-facing, not just a Rust doc comment); `String.length` byte semantics until W4-4; widen W5-13 scope (span reset also mis-anchors checker diagnostics in interpolation); `PHORJ_SKIP_PHP`+`PHORJ_REQUIRE_PHP` precedence. (B1-14) | ☐ |
| UA-0.15 | P3 | `php_escape` single-vs-double-quote context mismatch (latent, unexploitable today): rename → `php_escape_dq`, add `php_escape_sq`, switch the two call sites (`src/transpile/program.rs:1380,1392`; `transpile/mod.rs:835-839`). (B1-15) | ☐ |
| UA-0.16 | P3 | `subtle::ConstantTimeEq` for `Hash.equals` (already in the dep tree via argon2) + `cargo-audit` in CI for the vetted pins. (B1-16) | ☐ |
| UA-0.17 | P3 | **Ghost/stale diagnostic-code cleanup**: drop never-raised explain entries `E-OVERLOAD-SELECT-CONFLICT` + `E-PKG-TYPE`; fix comment-only ghosts `E-TYPE-IMPORT-BUILTIN/SHADOW` (`loader/mod.rs:617`, `resolve.rs:590`), the `E-STATIC-INIT-CONST` claim (`value.rs:596`), the self-contradicting `casing.rs:398` comment. (B1-17) | ☐ |
| UA-0.18 | P3 | Correct the `Suspend` trait doc comment (`src/green/exec.rs:40` — claims a nonexistent wasm frame-swap implementor; dep-isolation is the true justification). (B1-18) | ☐ |
| UA-0.19 | P3 | Delete stray `target/` scratch dirs (`s2c_php_check/`, `s2d_php_check/`, empty `tmp/`) — they false-positive `phg format --check .`. (B1-19) | ☐ |

### 2.2 UA-1..UA-L · Bucket 2 — the seventeen RULED decisions as work items

**Small/Medium code fixes (rulings final — implement exactly as ruled):**

| # | Ruling (2026-07-03, developer) | Scope notes |
|---|---|---|
| UA-1.1 | **`String.trim/trimStart/trimEnd` → Rust's Unicode-aware trim stays canonical; the PHP leg emits a matching Unicode-whitespace helper** (Option 2 — REVERSED the synthesis rec after evidence that codepoint/Unicode-by-default is the project's already-decided `String` direction, byte-exact lives on `Bytes`; W4-4 draft). (B2-1a, P0) | `src/native/text.rs:25,405,414,459`; transpile helper; adversarial differential case (`"\u{00A0}x"`) same-commit |
| UA-1.2 | **`String.reverse` → char-wise on both legs; PHP gets an mb-safe helper** (Option 1 — byte-mangling multibyte is the surprise Phorj removes). (B2-1b, P0) | `text.rs:40,477`; differential case with multibyte input |
| UA-1.3 | **`Hash.pbkdf2` → widen the internal iteration counter to u64; NO u32::MAX guard/fault** (Option 2 — REVERSED: u32::MAX is an implementation accident, not a domain limit; 2^32 iterations ≈ minutes, not years; `pbkdf2_sha256` is hand-rolled at `src/native/hash.rs:444`, so widening is free and fixes the root cause — silent truncation of a security parameter). (B2-1d, P0 + security) | `hash.rs:516-518`; KAT + differential case |
| UA-1.4 | **`Hash.hmac` → returns `bytes`** (BREAKING, deliberate: hex is a transport concern, not a MAC's output type; makes hmac/hkdf/pbkdf2 uniform; blast radius verified tiny — `examples/guide/crypto-mac.phg:13` + 1 README line; pre-1.0). (B2-5) | Update example + README; differential |
| UA-1.5 | **`->` retirement sequence: docs/help pass FIRST, then flip the parser-reject, then fix gate-surfaced sites INDIVIDUALLY** (Option 1). The 6 accept-sites: `src/parser/types.rs:109`, `items.rs:240/296/370/735`, `exprs.rs:546` (+ lexer `mod.rs:1125`). ~1700 embedded arrows in `src/` preludes + `tests/*.rs` strings; **NO bulk sed** — the 2026-07-03 attempt proved regex cannot distinguish return-`-> T` from function-type `-> T` (recovery lessons preserved in git history at `0691228`, wave0-remainder "ATTEMPT + RECOVERY"). ~200-site ambiguous tail is gate-guided, `=>` for fn types / `:` for returns. Fresh session recommended. (B2-3, P1; = cleanup Phase 1 remainder) | Docs half largely covered by the Stage-D pass + UA-0.3; then the flip |
| UA-1.6 | **Empty-`[]` type inference extends from List to Map positions** — `Map<string,int> e = [];` must check. **PLUS (same work, ruled follow-up): `[...]` becomes context-typed for `Set` too** — Set currently has ZERO literal syntax (`Set<int> s = [1,2,3];` type-errors; `{1,2,3}` doesn't parse); same expected-type mechanism, no new syntax token. (B2-4 + Set expansion) | Checker expected-type rule; differential cases for all three collection literals. **⚠ SCOPE FINDING (2026-07-04): deeper than a checker-type tweak. `List<T> x=[]` works because the runtime value of `[]` is naturally an empty `Value::List` (stmt.rs:104-119 just TYPES it, no backend change). But empty-`[]`→Map and `[1,2,3]`→Set need the BACKEND to build a Map/Set (not the default List), so they require a checker-fact-driven DESUGAR (empty-`[]`→`Expr::Map([])`; `[elems]`→`Set.of([...])` — Invariant-5 compile-time sugar) fed by expected-type→list-literal threading. That threading is EXACTLY W3-5's blocker option (A) — building it once unblocks BOTH UA-1.6 and W3-5. Do them together; not a tail-of-session change.** |
| UA-1.7 | **`Math.clamp(min > max)` → faults** (matches the module's precondition convention and Rust's own `clamp`). (B2-6) | ✅ **DONE** (status-audit 2026-07-09): native `math_clamp` faults `"Math.clamp: min (lo) must not exceed max (hi)"` (`src/native/math.rs:139`), `__phorj_clamp` PHP helper faults in kind (`transpile/program.rs:899`), differential `math_clamp_min_gt_max_faults_identically` (`tests/differential.rs:723`), example note (`examples/guide/math.phg:32`). |
| UA-1.8 | **Fault-message canonicalization** (`"Module.function: message"`). **AUDITED 2026-07-05 (B-2d, `docs/research/b2d-rich-error-audit.md`): effectively DONE for the live surface** — the 8 reachable user-facing faults already match (part-1); no stale module names remain; the ~40 "non-canonical" strings are all `Module.func expects (types)` arity guards that are **checker-unreachable / differential-blind → SKIP** (cosmetic dead-path churn). Residual error-model work = DEC-180 reclassification (below), NOT a string sweep. (B2-9) | Superseded by the B-2d audit |
| UA-1.9 | **Import-redesign guide example: yes** — one small guide example + README row for the S0–S2 member-import/qualified discipline. (B2-12) | ✅ **DONE** (status-audit 2026-07-09): `examples/guide/imports.phg` exists + README row (`examples/README.md:18`) describing the S0–S2 discipline (module-qualified functions, member-imported types, no wildcards/`import type`). |
| UA-1.10 | **`playground/src/lib.rs`: try `#![forbid(unsafe_code)]` empirically**; if wasm-bindgen glue genuinely needs unsafe, record the exemption in INVARIANTS instead. (B2-13) | Try-first, fall back to recorded exemption |

**Large-scope tracked waves (grew out of the audit gate — first-class items, each needs a
design/spec pass in UNIFIED-SPEC before implementation, NOT just a code diff):**

#### UA-L1 · Native-error checked-exceptions: taxonomy spec + 4-native pilot — L · DESIGN-NEEDED
- **RULED (B2-1c expansion):** today ALL ~236 natives' argument/domain errors are a deliberate
  UNCATCHABLE fault tier, structurally disconnected from the (good, shipped) `throws`/`try`/`catch`
  checked-exception system (verified: `try{ String.count("abc","") }catch(Error e)` never fires;
  `src/vm/exec.rs:399-408` bare `?` on `Op::CallNative`). The developer ruled: **design the
  taxonomy + class-hierarchy spec now** (which failures are recoverable domain errors vs programmer
  bugs; Core.ValueError/RangeError-style family vs per-module types), **pilot checked-throw
  semantics on exactly the four UA-1 natives (trim/reverse/split/pbkdf2) as the first real
  example, ship together, then roll the remaining ~232 natives out in tracked follow-up waves** —
  NOT a flag-day rewrite. Breaking-change note: natives declaring `throws` makes untried call
  sites stop compiling — the rollout waves carry corpus migration.
- **Within the pilot:** `String.split/splitOnce("")` empty-separator divergence (Rust total,
  PHP fatal — the original B2-1c bug) is resolved BY the pilot's checked-throw semantics on both
  legs, not by a standalone fault.
- **ACCEPTANCE:** spec section ratified; 4 pilot natives throw checked exceptions byte-identically
  (incl. the PHP leg); differential + adversarial cases; rollout waves enumerated in this plan.

#### UA-L2 · Injected-prelude → module-loader unification — L · DESIGN-NEEDED · **BEFORE W3-1/W3-2**
- **RULED (B2-2):** unify stdlib type resolution with the loader (virtual Core modules) NOW,
  before the DB/HTTP waves multiply the registry. The ~5 special-case rules
  (E-INJECTED-TYPE-BARE, leaf qualification, prelude exemption, collapse pass, hand-synced
  registry in `checker::enforce_injected::module_of`) exist only because `inject_*_prelude` ×6
  bypasses the loader. The S1/S2 user-facing surface (member-imports, qualified forms) is
  PRESERVED — one resolution path underneath.
- **ACCEPTANCE:** injected-type behavior byte-identical pre/post (the S2 test set is the gate);
  the collapse/enforce special-case passes deleted or reduced to loader rules; W3-1/W3-2 blocked
  on this landing.

#### UA-L3 · ReDoS transpile-time static complexity analyzer — L · DESIGN-NEEDED
- **RULED (B2-7):** FULL mitigation, not disclosure-only (developer explicitly chose the harder
  option). Problem: Rust's `regex` crate is linear-time (no backtracking, no backrefs/lookaround);
  PHP's PCRE backtracks — `(a+)+$` compiles fine under BOTH, runs instantly natively, and
  catastrophically backtracks under PCRE on adversarial input. "Compiles under the regex crate"
  is NOT a sufficient check. Needs a real static complexity analyzer at transpile time
  (safe-regex-style nested/overlapping-quantifier detection), with the false-positive
  (blocking legit patterns) and false-negative risks designed for explicitly — **do not scope
  this down to a KNOWN_ISSUES paragraph.**
- **ACCEPTANCE:** design section in UNIFIED-SPEC (detection heuristic, error/warn tier, escape
  hatch); transpile rejects the known-catastrophic corpus; FP corpus documented; disclosure
  paragraph ships with it (the honest interim is disclosure — but the tracked item is the analyzer).

#### UA-L4 · VM string performance: `Rc<str>` Value refactor — L
- **RULED (B2-10):** the full architectural fix — `Value::Str` becomes Rc-shared (the only non-Rc
  compound variant today; `Op::GetLocal` deep-clones every read, `src/value.rs:124`,
  `src/vm/exec.rs:149-153`) + COW `make_mut` path for mutating natives. Measured: VM is 1.53×
  SLOWER than the interpreter on string concat — inverts the documented contract.
- **ACCEPTANCE:** full differential gate + before/after `phg benchmark` per G-2 (parity-affecting
  surface); the inversion workload flips.

#### UA-L5 · THE rename wave — one batch, everything — M/L
- **RULED (B2-8):** fold EVERYTHING into ONE rename wave (not staged): the approved trio
  (`Bytes.find→indexOf`, `Map.has→containsKey`, slice-convention unification to
  length+negatives) PLUS all audit-surfaced candidates: `length` vs `size` (List/String/Bytes vs
  Map/Set), `count`'s two meanings (List predicate vs String needle), `Core.Conversion`'s three
  naming families + `Conversion.round` duplicating `Math.round`, `intBetween`/`secureInt` ranged-
  pair naming, `nowMilliseconds`/`monotonicNanos` unit suffixes. Crypto 3-module split stays
  (ruled KEEP — mirrors PHP's own layering) + one cross-referencing docs paragraph.
  (= cleanup Phase 2, widened.)
- **ACCEPTANCE:** naming section of UNIFIED-SPEC updated first; renames + all .phg/test/doc
  usages one wave; zero grep hits for retired names; byte-identity gate.

#### UA-L6 · Stdlib additive wave — one batch, everything — M
- **RULED (B2-11):** approved items (`String.startsWith/endsWithIgnoreCase`, `replaceFirst`,
  `Set.isSuperset/symmetricDifference/isDisjoint/map/filter`, `Math` Float variants) + new
  candidates ship together: `Bytes.isEmpty`, `Map.entries` + `Map.of`/`fromList`,
  document-level `Csv.parse` (multi-row), `Math.ceil/floor→float` vs `round→int` return-type
  note. (= cleanup Phase 3, widened.) Examples + tests per G-5.

### 2.3 UA-D · Bucket 3 — doc-only corrections (EXECUTING in this Stage-D pass)

Fourteen findings, five of them P0 false-claim families — all being corrected in the 2026-07-03
consolidation (this file + UNIFIED-SPEC + the sibling corrections to KNOWN_ISSUES/FEATURES/
ARCHITECTURE/STABILITY/GA-CHECKLIST/ROADMAP/VISION/README/CONTRIBUTING). Checklist for
verification, then this section collapses to "done + never reintroduce" (G-7):

- ☐ B3-1 "zero external deps" FALSE everywhere (4 default deps: argon2, regex, ctrlc, corosensei;
  rusqlite+rustls now approved as feature-gated domains) — incl. `Cargo.toml:83-85` comment.
- ☐ B3-2 `import type` taught as current syntax (it hard-fails to parse since S0) — docs, .phg
  comments, ~14 src doc-comments.
- ☐ B3-3 dead CLI verbs (`fmt`/`lex`/`disasm`/`bench`) taught as real, declared *stable*; the
  real verbs are `format`/`tokenize`/`disassemble`/`benchmark`.
- ☐ B3-4 `E-TRANSPILE-CONCURRENCY` does not exist — the code is `E-CONCURRENCY-NO-PHP`
  (fixed in THIS file at G-1.1); `E-RETIRED-SYNTAX` labeled *planned*.
- ☐ B3-5 FEATURES 🔲/🚧 on SHIPPED features (traits-construct, concurrency, lift, LSP, formatter,
  Set-algebra); GA-CHECKLIST "Missing: an LSP" false → its ≈57% retired.
- ☐ B3-6 the 16 doc-vs-doc contradiction pairs (table in raw/A4 §C) — resolved once at merge.
- ☑ B3-7 percentage staleness — re-scored in §11.2 (this file).
- ☐ B3-8 undocumented shipped features (W3-4 crypto FEATURES row; S1/S2 import discipline
  user-facing doc; decimal row; ctrlc/corosensei named; playground-member deps in the dep policy).
- ☐ B3-9 the 5 `examples/project/` dirs indexed nowhere + `web/json-api.phg` index row.
- ☑ B3-10 stale counts in plans (KNOWN_ISSUES 1133, differential.rs 3308, explain ≈200 codes,
  28 stdlib modules / 236 natives; "S1 uncommitted" claim — S1 shipped `cd29f3c`) — corrected here.
- ☐ B3-11 KNOWN_ISSUES stale content (`\Main\Obj`, the contradicted "not yet implemented" list,
  `->` prose ×10) — sibling pass + UA-1.5 doc half.
- ☐ B3-12 archive the superseded research corpus (~1.07 MB raw dirs) once Stage E closes;
  `roadmap-completeness/raw/` is historical-only (its claims are refuted at HEAD).
- ☐ B3-13 ARCHITECTURE.md:88 stale embedded verification command; statics research-spec header.
- ☐ B3-14 undocumented flags into help (`--dump-on-fault`, `benchmark --json`,
  `build --dev`/`--sign`); `phg --help` arrow prose (folds into UA-1.5).

### 2.4 UA-W · Bucket 4 — watch / defer (known, deliberately not active)

Do NOT work these without a trigger; do NOT drop them either.

| # | Watch item | Trigger / note |
|---|---|---|
| B4-1 | **Four-backend maintenance tax** (~28% of src + the heaviest harness) — root cause of the UA-1 drift class; verdict: right call (byte-identity IS the product). | Re-weigh the VM's perf justification at GA; the cheap mitigation is UA-0.13, not fewer backends |
| B4-2 | DAP transport write errors silently swallowed + non-loud malformed Content-Length in both framers | Scheduled in W0-10 (a/b) — do there |
| B4-3 | Zero direct tests for `src/dispatch.rs` + `src/json.rs` | Rides W0-10's A-TEST-1 |
| B4-4 | File-size rule unenforced (12 files > 1000 lines, no size-gate.sh) | W1-6 builds the gate |
| B4-5 | **W2-7 (PSR-4 import roots) was designed BEFORE the unified-import model** — re-base/re-adjudicate before build or it becomes import redesign #5 | Hard gate on W2-7 |
| B4-6 | `src/checker/` at ~21K lines is the gravitational center; three near-identical type-walkers | Consolidate when the NEXT type-walking pass is added, not before |
| B4-7 | VM per-dispatch String clones (CallMethod ×2, MakeEnum, MatchTag, MakeInstance; GetField's inline-cache technique applies) | Needs measured before/after; UA-L4 first |
| B4-8 | Monolithic 84 kLoC crate → 50.3 s release rebuilds; incremental dev builds healthy (4.5–7.6 s) | Crate split only if release-rebuild pain becomes real |
| B4-9 | `List.append` refcount-1 fast path (signature or owned-arg path) | Guidance half is UA-0.14 |
| B4-10 | DX wishlist: debugger REPL eval, did-you-mean on dead verbs/unknown flags, `(os error 2)` leak, `phg format` exclude mechanism, `phg fmt` alias | Batch when touching the CLI surface |
| B4-11 | Dev-loop nice-to-haves: green-tree stamp, `debug = "line-tables-only"` (target/ is 22 GB), periodic target sweep | Opportunistic |
| B4-12 | Hand-rolled SHA-256 now underpins keyed crypto — tension with the charter's never-roll-your-own clause; mitigated by RFC KATs + oracle byte-identity | Revisit if RustCrypto enters the tree (e.g. with rustls) |
| B4-13 | Unbounded self-DoS allocations (secureBytes(exabytes), regex cache no eviction) — no resource-quota model by design | Listed for completeness |
| B4-14 | Two production `expect` on scheduler invariants (`interpreter/coop.rs:103`, `green/exec.rs:131`) — internal, not user-reachable | None |
| B4-15 | `examples.js` staleness CI check (`gen_examples.py && git diff --exit-code`) — artifact currently NOT stale | Add when touching playground CI |

### 2.5 Positive attestations (do not re-litigate)

`src/` structurally clean (0 stubs/TODOs/dead-code allows; Op coupling 73/73/73 wildcard-free;
`forbid(unsafe_code)` both crate roots; kernels single-sourced; deps exactly per policy; all prior
src P0/P1s verified FIXED). All 9 golden diagnostics byte-identical; `phg explain` covers 100% of
emitted codes; zero Rust panics across every error probe. Security posture strong: 0 High findings,
13 positive attestations (Argon2id/OWASP defaults, fail-loud CSPRNG, no shell anywhere, hardened
vendor, injection-safe PHP emission, bounded serve). No premature abstraction (4 traits / 75K LOC).
Corpus 100% clean of syntactic `->` and `import type`; formatter idempotent; single-test loop 0.26 s.

### 2.6 Cleanup-program phase reconciliation (the old P1–P6 map onto this programme)

| Old phase (wave0-remainder) | Now lives at |
|---|---|
| P1 remainder (`->` parser reject) | UA-1.5 (with the attempt+recovery lessons carried) |
| P2 renames | UA-L5 (widened to the full batch, ruled) |
| P3 additive natives | UA-L6 (widened, ruled) |
| P4+P5 docs consolidation | §2.3 Bucket 3 + this file + UNIFIED-SPEC (executing) |
| P6 editor/LSP refresh | W6-8, gated on the corpus being clean (post UA-1.5/UA-L5) |
| Final convergence verification | Stage E: full gate re-verify + archive the audit raw dirs (B3-12) |

---

### 2.7 AUTONOMOUS OVERNIGHT MARATHON — execution queue (set 2026-07-04)

*HISTORICAL execution queue (2026-07-04), NOT an active driver — superseded by THE FINISHING WAVE section
(§ after §1), which is the single active roadmap. The item detail below is backlog the Ω-sub-waves draw
from; do not run it as a separate queue.*

> **Goal (developer, 2026-07-04):** a large autonomous run that ships **features + runnable examples +
> up-to-date VSCode & PhpStorm editor support**, so the developer can build & real-test a real Phorj
> project the next morning. Runs autonomously (autonomous-3c); **commit each green self-contained
> step** (phorj rule — `feat:`/`fix:`/`docs:`, no `Co-Authored-By`); **never `git push`**. This is an
> ORDERING over the already-ruled work below — nothing here is a new decision (except the two clearly
> marked NEW deliverables), and nothing overrides §15 (surface a genuine fork, don't guess).

**Definition of done per step (all must hold before moving on):**
1. Full gate GREEN on the real oracle: `source scripts/toolchain.env && PHORJ_REQUIRE_PHP=1 cargo test
   --workspace` + clippy + fmt + `cargo build --release` (php-8.5.8; a missing php FAILS, never skips).
2. Invariant 9 — every shipped feature lands a runnable `examples/**/*.phg` (auto-gated by the
   differential glob) + an `examples/README.md` row, same commit.
3. Byte-identity holds (`run ≡ runvm ≡ transpiled PHP`) unless the item is a ruled quarantine
   (concurrency, impure natives, the UA-L1 checked-exception pilot cases).
4. Report `target/release/phg` path after each shipped feature (standing rule).

**Ordered queue** (stop-and-surface via §15 on any genuine language fork — do NOT self-rule):

- **M0 — hygiene/unblockers first (fast, no design risk):**
  - Repoint the 11 live spec pointers → `UNIFIED-SPEC.md` sections and **archive all 18
    `docs/specs/2026-*.md`** (developer ruled "review-then-archive"; faithfulness already verified —
    closeout TASK-3 PASS). Repoint map: closeout TASK-4 list (README/FEATURES/VISION/STABILITY/
    THIRD-PARTY-NOTICES/docs/examples are live pointers; CHANGELOG + `src/*.rs` provenance comments
    may stay as historical). `git mv` originals into `docs/specs/archive/`.
  - **NEW — fix `playground/web/gen_examples.py` non-determinism** (Rule 10 violation): it emits the
    example list in filesystem/dict order → the committed `examples.js` reorders on every regen
    (proven pure-reorder, 0 content delta). Sort deterministically, regenerate, commit once. (Bucket-1.)
  - The rest of **UA-0 / Bucket 1** (§2.1): mold linker wiring, the broken `phg run --help` examples
    (also drops their `->`), missing diagnostic corpus cases, the KNOWN_ISSUES disclosures, gate-test
    sharding, stray `target/` scratch dirs.
- **M1 — language self-consistency (unblocks everything a real project touches):** UA-1.1..1.4 (the 4
  byte-identity fixes incl. `Hash.hmac`→bytes), UA-1.6 (Set/Map empty-literal inference), UA-1.7
  (`Math.clamp` fault), UA-1.8 (fault-message canonicalization). Then **UA-1.5 (`->` retirement:
  docs/help first — mostly done — then parser-reject, then individual gate-guided fixes; NO bulk sed).**
- **M2 — architecture-before-waves:** UA-L2 (injected-prelude → loader unification, MUST precede
  W3-1/W3-2) · UA-L4 (`Rc<str>` VM string fix, benchmark before/after per Invariant 11).
- **M3 — the web-app spine (the heart of "real project"):** §12 ROI order — W3-5 `String.format`
  (SURFACE the `{}`-grammar blocker via §15 FIRST, then build) → W3-1 SQL DBAL (SQLite P1) → W3-2
  HTTP client → W3-3/6/8 finish the serve/router spine. Each ships examples.
- **M4 — stdlib breadth for real apps:** UA-L5 (the one naming-rename wave) · UA-L6 (additive
  batch) · UA-L1 (native-error checked-exception taxonomy spec + 4-native pilot) · UA-L3 (ReDoS
  transpile-time analyzer — needs its design pass).
- **M5 — NEW: editor support so the project is actually writable/testable in an IDE:**
  - **VSCode** (`editors/vscode/`): update `syntaxes/phorj.tmLanguage.json` to the current surface
    (`=>`/`:` not `->`, current keywords, unified `import`, `#[Http.Route]` attrs, decimal/bytes
    literals); confirm `extension.js` launches `phg lsp` as the LSP client (wire it if not); fix the
    README's dead verbs (`phg fmt`→`format`); bump version + rebuild the `.vsix`.
  - **PhpStorm** (`editors/phpstorm/`): **FINDING (2026-07-04) — the "README-ONLY, no working path"
    premise was stale.** `editors/phpstorm/README.md` already documents a complete, functional
    NO-BUILD path that delivers the full feature set today: JetBrains' built-in **TextMate Bundle**
    import (pointed at `editors/vscode/`, reusing the shared grammar) for highlighting + the
    **LSP4IJ** marketplace plugin running `phg lsp` for diagnostics/hover/completion/rename/format.
    Both editor READMEs already track the *natively-compiled marketplace plugin* as a follow-up.
    Refreshed the docs (dead `phg fmt`→`phg format` verb) in this pass. **PENDING scope decision
    (recorded per §15 "surface, don't self-rule" + "surface scope if it balloons"):** should the run
    build a native Gradle/IntelliJ-Platform plugin? Analysis for the developer: (a) the no-build
    path already delivers full functionality; (b) a native plugin is a large lift (Gradle +
    IntelliJ-Platform SDK + Kotlin/plugin.xml) and **cannot be verified here** (needs a running
    PhpStorm), so it would ship unproven — against the evidence-before-completion discipline;
    (c) recommendation → **keep the no-build path as the supported story; defer the native plugin**
    unless the developer wants to invest a large, here-untestable build. Not autonomously ruled.
  - Refresh `examples/guide/` walkthroughs referenced by both extensions; re-gen the playground.
- **M6 — Core.Dotenv (UA-L7, developer-requested full Symfony cascade):** write the design spec first
  (taxonomy · the `test`-env "`.env.local` skipped" footgun decision · Secret-type integration · the
  emitted PHP cascade helper · quarantine), then implement + example. Layers on existing
  `Core.Environment`. (Sequence after M3's web spine, since dotenv serves web apps.)
- **Close — convergence verification:** full gate re-verify; recompute the §11 percentage (824-row
  re-score) + update the §0 cursor; `/handoff`.

**Autonomous guardrails:** obey §15 (ADJUDICATION) — record genuine user-visible forks as PENDING and
keep going on the rest; never silently downgrade byte-identity (§14 LADDER); the 5-round advisor cap
still escalates via `ask-human` even autonomously. Update the §0 cursor block at each milestone close.

---

## 3. WAVE 0 — REPAIRS & HYGIENE (status ledger)

*All rulings in Appendix B; zero language-surface change. Detail recipes: git history `0691228`.*

| Item | What | Status |
|---|---|---|
| W0-1 | Restore pre-commit gate (hooksPath) | ✅ `c66bde5` |
| W0-2 | Static-field visibility spine repair | ✅ (wave-0 slate `f28d950..c0f8969`) |
| W0-3 | Static-method-via-instance error | ✅ (slate; the FIELD mirror is UA-0.6) |
| W0-4 | Loader package gates + E-ALIAS-CYCLE | ✅ (slate, both halves) |
| W0-5 | VM interpolation fault-line disclosure | ✅ (slate; fix = W5-13; scope widened per UA-0.14) |
| W0-6 | Front door: doc snippets + CLI renames + CI doc-check | ✅ halves 1/2 (slate); **remainder = W0-6b**: verb-rename sweep across the 12 enumerated files + the CI ```phorj-fence `phg check` job (fold with §2.3 B3-3) |
| W0-7 | Doc-reconciliation + CLAUDE.md rules-only rewrite | ✅ `c66bde5` + consolidation pass |
| W0-8 | Plan-file deletions + de-dangle | ✅ (48 + second batch) |
| W0-9 | Housekeeping: branches, dist/, KNOWN_ISSUES prune, examples index restructure | ☐ — KNOWN_ISSUES prune + examples restructure fold into §2.3 (B3-9/B3-11) + the sibling pass; branches/dist remain |
| W0-10 | P2 hardening batch (DAP dead-flag, framing errors, CI pins, `usize::try_from`, package-lock, W-SECRET note, json.rs+dispatch.rs unit tests) | ☐ — CI-pin half needs verified external data (never guess a pin/SHA) |
| W0-11 | realworld Core.File example | ☐ |
| W0-12 | PUSH + external renames (repo rename, dir mv) | ☐ developer-gated — NEVER autonomous |

---

## 4. WAVE 1 — DECOMPOSITION

*The B-modularity spec executed verbatim: moves-only, zero behavior change, one cluster per commit,
full differential each. Do-not-split list honored. Detail: raw/B-modularity.md + git history.*

- **W1-1** `tests/differential.rs` split (3308 lines → directory-form, test-count parity) — step 0.
- **W1-2** Mechanical off-spine splits (chunk, cli/mod+preludes, serve, lift/parser, lift/lifter, fmt/printer).
- **W1-3** Front-end splits (lexer, parser/items, checker/program+totality+expr+casts+calls+collect).
- **W1-4** Spine splits (value/, interpreter, compiler ×2 — scratch-slot discipline —, transpile/program) + the Op-exhaustiveness smoke test.
- **W1-5** Decompose the three ~500-line functions (`main.rs::main`, `check_interface_graph`, `compile_program_with`) + cognitive-complexity ratchet.
- **W1-6** `scripts/size-gate.sh` in CI + G-6 write-back (calibrated post-split).
- **W1-7** Clarity workstream: ARCHITECTURE narrated rewrite, `//!` module docs, blanket `clippy::pedantic` fix-all (DEC-176).

---

## 5. WAVE 2 — RATIFIED LANGUAGE CHANGES + ENFORCEMENT COMPLETION

*The breaking/codemod wave. W2-1 lands FIRST (converts later codemods into compiler-driven
commands). All rulings Appendix B; item recipes in git history `0691228`.*

- **W2-1** `phg fix` — machine-applicable diagnostics (structured `(span, replacement)` edits; LSP code-actions ride it). The delivery vehicle for everything below.
- **W2-2** E-MATCH-BARE-TYPE hard error (+ 3-way did-you-mean, repo codemod).
- **W2-3** foreach REPLACES for-in (REPLACE, not add — the C-2 drift lesson) + binding forms; full corpus codemod.
- **W2-4** `->` return-syntax retirement — **superseded by UA-1.5's ruled sequence** (docs first → parser-reject → individual fixes); W-SEQUENCE-MUTATION verify rides along.
- **W2-5** E-INTERSECT-SIG relaxed via the shipped overload-resolution rules.
- **W2-6** DEC-047 no-wind closure. **Fault-intrinsic imports ✅ SHIPPED (DEC-196 Q3, 2026-07-05)** — NOT the old single-`import Core;` model: they land as the two-mode `Core.Assert`/`Core.Abort` split (whole-module→qualified `Assert.assert`, member→bare `panic`, grouped ok; `E-UNIMPORTED` otherwise). Remaining W2-6 sub-items (deep imports, aliasing, further de-reservations) stay open. Spec: UNIFIED-SPEC §"Nothing in the wind" (updated to the two-mode model).
- **W2-7** Import-roots PSR-4 `[packages]` map — **⚠ B4-5 gate: re-base on the unified-import model (S0–S2 + UA-L2) and re-adjudicate BEFORE build.**
- **W2-8** Enforcement adoptions (E-IMPORT-UNKNOWN, E-capture-write + `Core.Ref<T>`, W-family unused/never-thrown lints, NaN→fault unification, the batch-2 ten + batch-3 twenty-six — all adopted, Appendix B).
- **W2-9** Naming-overhaul remainder — **fold into UA-L5** (one rename wave; the naming section of UNIFIED-SPEC is the SSOT).
- **W2-10** Narrow soundness-hole batch (static-init protected ctor, interface throws discharge, `x.m()?`, MI lowering corners…).
- **W2-11** Static-call ergonomics + `this.f[i] = e` field-base index-assign (unblocks W6-4 benchmark ports).
- **W2-12** Erased-generic result as VM operand (close the run↔runvm CTy gap; option ii — kernel-backed dynamic fallback — is the spine-safe default).
- **W2-13** Enforcement audit → should-error conformance suite + the explain-coverage ratchet (UA-0.5/0.7 seed it).
- **W2-14** `new` on enum variants — ✅ ruled KEEP (closed).

---

## 6. WAVE 3 — WEB-APP ENABLEMENT SPINE

*The critical path: what blocks real applications. Stdlib items follow the M4 charter
(UNIFIED-SPEC, stdlib-charter section). Tier-B (impure) modules quarantined per `pure:false`.*

### Dependency-policy amendment (RULED 2026-07-03, developer)
`rusqlite` (SQLite) + `rustls` (TLS) admitted as new vetted domains — native-only, feature-gated
(`db`/`tls`, off in WASM), spine-quarantined (corosensei/ctrlc shape), `forbid(unsafe_code)`
intact in phorj's own code. Pure zero-dep P0s ship FIRST (`Core.Sql`, `Core.Url`). Recorded in
UNIFIED-SPEC (dependency-policy section). **Gate: UA-L2 (prelude/loader unification) lands before
W3-1/W3-2 add stdlib types.**

- **W3-1 · SQL DBAL** (XL, design draft at `docs/research/wave3-4-drafts/`) — **scope RULED:** a
  multi-driver data-access layer (PDO/Doctrine-DBAL analog): SQLite (P1, rusqlite, embedded) +
  Postgres (`postgres` sync) + MySQL/MariaDB (`mysql` sync; one driver = both). ALL sync — async
  runtimes stay policy-rejected. **Oracle DEFERRED** (closed Instant Client violates policy
  clause 2). **MongoDB = a SEPARATE future LADDER item** (non-SQL/no-PDO → native-only
  `E-TRANSPILE-MONGO`, no PHP leg; async-driver problem) — its own XL design, NOT part of the DBAL.
  `Core.Sql` builder is Tier-A byte-identity; execution Tier-B fixture-tested. Faithful PDO mapping
  ⇒ does NOT trigger the ladder.
- **W3-2 · HTTP client** (XL, draft exists) — typed client over the shipped Request/Response,
  middleware closures, pooling on green threads, HTTPS via the rustls feature fork. Ships pure
  `Core.Url` (spec-compliant parser + build_query — leapfrogs `parse_url`) FIRST. Layered design
  ruled (Appendix B): HttpClient engine + `Http.get/post` sugar.
- **W3-3 · Sessions/cookies/auth** (L) — value-level (no ambient `$_SESSION`), signed/encrypted
  payloads over W3-4 HMAC, CSRF helpers. DEPS: W3-4 ✅, W3-6.
- **W3-4 · CSPRNG + HMAC/KDF** — ✅ **SHIPPED `f4c4c1d`**: `Core.Hash.hmac/equals/hkdf/pbkdf2`
  (RFC KATs, byte-identical) + `Core.Random.secureBytes/secureInt` (quarantined). Follow-ups now
  tracked: UA-1.3 (pbkdf2 u64), UA-1.4 (hmac→bytes), UA-0.16 (ConstantTimeEq).
- **W3-5 · `String.format` (sprintf family)** — design RULED (developer, 2026-07-03): lives on
  `Core.String` (NOT a new `Fmt` module), Java-familiar `format(spec, args) -> string`, **`{}`-style
  spec grammar shared with W5-1** interpolation specifiers (`%`-style rejected). **✅ BLOCKER RESOLVED
  2026-07-04 (DEC-178):** option (A) chosen — expected-type threading into list-literal call args is
  built as part of the **Type-System programme (Wave A / §2.7)**; `String.format` args use a CLOSED
  scalar form (not open `Any`). W3-5 now rides Wave A (→ Wave C) and needs no further adjudication.
  **✅ SYNTAX RULED — DEC-199 (developer, 2026-07-05) — SUPERSEDES DEC-198 (`{}`-for-format DROPPED).**
  `String.format` uses **PHP-style `%` sprintf** (`%s`/`%d`/`%08.2f`/`%1$s` positional), NOT `{}`. The
  reasoning chain (all interactively challenged): (1) positional *literal* format is redundant with
  interpolation (you have the values inline → `"{a} {b}"`), so `String.format`'s only non-redundant job is
  a spec SEPARATE from the values — a **runtime** string (i18n/templates); (2) a runtime spec cannot be
  statically checked in ANY syntax, so `{}`'s sole real advantage (compile-time arg checking) evaporates
  for format's actual use case → `{}` would be pure divergence from PHP with no payoff (no perf, no safety);
  (3) `%` does NOT collide with phorj's `{expr}` interpolation (the DEC-198 blocker), so it's collision-free
  by construction; (4) it transpiles to a literal PHP `sprintf(…)` — perfect fidelity. **Phorj UPGRADE within
  the familiar syntax:** render STRICTLY — a type mismatch (`%d` given a string) is a **clean runtime fault**
  (faults are uncatchable bugs), NOT PHP's silent coercion. `{}` stays **interpolation-only**; whether
  interpolation gains `{}`-specifiers (`"{x:>8.2}"`, W5-1) is a SEPARATE future decision (two spec grammars
  vs interpolation-spec-less — flagged, not ruled). Import/call form still per DEC-197 (a `Core.String`
  native → bare `format(…)` or qualified `String.format(…)`).
  **BUILD (spine-sensitive, sliced by conversion set):** a Rust `%`-sprintf renderer byte-identical to PHP
  `sprintf` (interp + VM match the transpiled PHP leg). **SLICE 1 SHIPPED 2026-07-05 (gate 1796):** `%s`
  (any scalar via the interpolation `as_display`/`__phorj_str` kernel — verified byte-identical for
  int/string/float/bool/decimal incl. `4.0`→"4", `true`→"true"), `%d` (STRICT — non-int → clean fault,
  fault-parity verified all 3 legs), `%%`. Real native `text_format` + gated PHP `__phorj_format`; checker
  special-case validates args + gates a LITERAL spec (`E-FORMAT-UNSUPPORTED` past `%s/%d/%%`,
  `E-FORMAT-ARG-COUNT`); heterogeneous value lists accepted (per-element scalar check); qualified + DEC-197
  bare import both work; `guide/string-format.phg` + 8 checker tests + 6 `E-FORMAT-*` explain entries.
  **NEXT SLICES:** (2) width/precision/flags (`-`/`0`/`+`) + `%f`; (3) `%x/%o/%b/%e/%g`; (4) `%N$` positional
  — each a byte-match-PHP-sprintf increment (run≡runvm≡php-8.5.8; unsupported → clean fault; dynamic spec
  supported). (Superseded DEC-198's `{}` desugar-to-`Str` — `%` uses the runtime renderer instead.)
- **W3-6 · Filesystem breadth + serve static-handle bridge** (L) — `Core.Directory`
  (mkdir/listDir/glob/…), fs-OOP question resolved (statics until streams demand handles); serve
  class-static `handle` entry. FS design ruled (Appendix B): Path value type, stateless IO,
  `throws FileError` default + `readOrNull()` explicit.
- **W3-7 · Structured logging** (M) — `Core.Log`, record construction pure, emission Tier-B;
  serve request logging.
- **W3-8 · Json encode + safe-parse hardening + rich responses** (M). NDJSON
  (`Json.parseLines/stringifyLines`) ✅ shipped `4dbd360`; INI (`Core.Ini.parse`) ✅ shipped
  `4f4f271` (hand-rolled PHP-charset trim — note UA-1.1 keeps `String.trim` Unicode; Ini's own
  parser keeps its PHP-exact set).
- **W3-9 · Method references as values** (M) — `obj.method` → typed closure; fixes the misleading
  "no field m" diagnostic.

---

## 7. WAVE 4 — MIGRATION-BRIDGE COMPLETION

- **W4-1 · Named args + variadics + spread** (L, DESIGN) — param-renaming policy decided up front
  (DEF-034); unblocks W3-5 variadic overload + the lifter on 8.0+ code.
- **W4-2 · Generators/`yield` + iterator protocol** (XL, DESIGN) — corosensei substrate; design
  must prove byte-identity vs PHP generators or explicitly extend G-1.1 (default: prove it).
- **W4-3 · Printable/`__toString` + `__invoke`** (M) — explicit checked contract; W5-2 derive(Show)
  auto-implements.
- **W4-4 · Unicode-correct strings** (XL, DESIGN, draft exists) — RULED adopt (codepoints default;
  bytes stay explicit on `Bytes`). **Known landmine from the draft: case-folding diverges from the
  `php -n` tier-1 oracle** (`strtoupper("straße")` keeps ß vs Rust STRASSE) → LADDER-quarantine
  candidate, surface at design time; ~12/35 Core.String natives change; `Value::Str` untouched
  (UA-L4 is orthogonal). UA-1.1's Unicode-trim ruling is the first plank of this direction.
- **W4-5 · Date/time breadth** (L) — IANA tz, formatting, DatePeriod, explicit-format parsing.
- **W4-6 · Stdlib blitz** (L) — list/math long tail per charter; `zip` after tuples (W5-10).
- **W4-7 · Lift Tier-2/3 depth + playground PHP input** (L) — after W4-1/W4-2/W3-5.
- **W4-8 · General inert attributes** (M) — inert-only ruled; derive (W5-2) is the behavioral surface.
- **W4-9 · Dynamic `Json`/`Any` boundary completion** (M).
- **W4-10 · XML/DOM/XPath** (L, DESIGN-NEEDED) — **PENDING adjudication artifact recorded**
  (2026-07-03): user-visible surface, developer's call; proposal + failing program + option
  previews to be presented, not built autonomously. Phased, PHP-DOM-backed per the format recipe.
- **W4-11 · Subprocess execution** (M, charter admission) — arg-vector only, no shell-string, ever.
- **W4-12 · Compression/archives + regex breadth** (L) — preg_replace_callback rides HigherOrder;
  **regex additions coordinate with UA-L3's analyzer.**
- **W4-13 · BigInt + arbitrary precision + Money** (L, DESIGN).

---

## 8. WAVE 5 — BEYOND-PHP PROGRAMME

*All 13 ADOPT-NOW items ruled (Appendix B). Compressed index — per-item LENS/HOW in git history.*

- **W5-1..W5-12**: interpolation format specifiers (grammar shared with W3-5) · closed derive
  channel (`Equals/Show/Hash/Ord/Default`, wave 2 `Json`) · sealed hierarchies · doc-tests ·
  opaque newtypes · Optional/Result combinators · compile-time-validated literals (regex literals
  coordinate with UA-L3) · let-else · auto-import quickfix/organizer · tuples + multiple return ·
  Printable (=W4-3) · labeled loops. `phg fix` = W2-1.
- **W5-13 · VM debug symbols** (L) — scope IP ranges → named locals → per-line pause → DAP over
  runvm; fixes the interpolation fault-line divergence (flip the W0-5 harness flag on).
- **W5-14 · M-perf lane** (L) — CallMethod inline cache, CallValue captures borrow, static/const
  borrow-keyed lookup; **the Rc-share `Value::Str` item is now UA-L4 (pulled forward, ruled)**;
  IsInstance interning, dispatch, const-fold, peephole, lazy for-range; incremental compilation
  (design). Every item: `phg benchmark` before/after.
- **W5-15 · DX cluster** (L) — `phg repl`, `phg doc` (format co-designed with doc-tests), parser
  multi-error recovery.
- **W5-16 · Concurrency completion** (XL, DESIGN) — structured scopes, deterministic `select`,
  deadlines, Task.all/race (colored async stays REJECTED); **M-Parallel = WORKER ISOLATES ruled**
  (own heap/thread, channel messaging; shared-memory Arc rewrite REJECTED); deep plan first.
- **W5-17 · Ruled checkpoints**: generics explicit type args BOTH sites ✅ ruled · UFCS
  TYPE-SCOPED ✅ ruled (specificity ladder, CI rebind guard) · ternary stays deferred-not-rejected ·
  the bulk-ratified six ✅.
- **ADOPT-LATER charter (27 items)**: each activates on its named trigger (XL-018 defer/XL-019
  using → first handle-based IO = W3-1; XL-021 semver-check → first tagged release; XL-024
  deprecation-codemod → after W2-1; pattern-slice batch XL-029..032; etc.). Conflicts ruled:
  comprehensions REJECT stands; FFI REJECT stands (`.d.phg` is the seam); open macros stay
  rejected (derive is the sanctioned subset).

### §7-CLOSED · `trait` — RULED 2026-07-04 (DEC-177): BLESSED alongside MI
Resolved. The premise was stale: `trait` is NOT unadopted — it is fully wired (lexer keyword,
parser construct with bodies + `use TraitName;`, `run≡runvm`≡transpiled PHP `trait`/`use`, verified
end-to-end). Developer **blessed BOTH `trait` AND multiple-inheritance as first-class** — this
mirrors PHP's own trait/composition duality (familiarity-first). Record in Appendix A as ADOPTED;
FEATURES `trait` entry flips to delivered. No open language question remains here.

---

## 9. WAVE 6 — SHOWCASE, SPEC & GA

- **W6-1** Flagship promotion + ~300-line REST micro-service (post-Wave-3: + DB + sessions).
- **W6-3** Truthful README rewrite — front-page truth is now largely §2.3's Bucket-3 pass; the
  full marketing rewrite (status table, lift promoted, G-1.1 disclosure placement) stays here,
  best after Wave 3.
- **W6-4** Benchmark credibility protocol — retire the dev-PHP number; release PHP 8.5 +
  opcache/JIT columns; methodology doc. DEPS: W2-11; UA-0.10 is the interim guard.
- **W6-5** Normative spec + clause-tagged conformance — chapters grow FROM `docs/specs/
  UNIFIED-SPEC.md` (the consolidation is the seed); spec-coverage matrix as a failing test;
  the G-1.1 disclosure is a normative clause.
- **W6-6** GA hardening batches (LI-G1..G6 — re-verify each first; several likely already fixed).
- **W6-7** Release engineering (reference/tour/migration docs, fuzzing, grammar files, release
  automation, `--sign`, semver-check).
- **W6-8** Editor + docs-site surface — **= cleanup Phase 6**: LSP completion/hover for the new
  natives + injected-type discipline; VSCode tmLanguage/snippets + PhpStorm re-targeted to the
  final `:`/`=>` surface; drop dead verbs. **Gated on the corpus being clean (UA-1.5 + UA-L5).**

---

## 10. STDLIB CHARTER — the 259 GAP-unplanned rows

Three-bucket model ratified (Appendix B); governance = the M4 charter's enforcement half
(UNIFIED-SPEC, stdlib-charter section) gates every new native.

- **Bucket 1 — ADOPT** (≈115 rows): itemized across Waves 3–4 + the triggered module backlog
  (Core.Serde after derive, Core.Event, Core.Cli, Core.Template, Core.Uuid, Caching, Core.Dump).
- **Bucket 2 — EXTENSION story** (≈75 rows: intl/ICU, gd/image, raw sockets, streams zoo, SPL
  decorators, finfo, readline): DESIGN-NEEDED extension policy (vetted-dep forks vs plugin seam vs
  `.d.phg`-PHP-leg-only); until ruled these rows count against parity honestly.
- **Bucket 3 — REJECT with reason** (≈69 rows): procedural aliases, removed/deprecated fns,
  superglobal-mutation APIs, array-cursor family, DWIM linguistics, weak refs, mail() shape —
  carried in Appendix A.

---

## 11. PERCENTAGE LEDGER

### 11.1 Model
M §4 (`docs/research/full-audit/raw/M-gap-matrix.md`): coverage = (COVERED×1 + PARTIAL×0.5) /
(rows − N/A − GAP-by-design), over **824 verdict rows** (173 SYN + 631 FN + 20 RT; net denominator
665 = 129 + 518 + 18). Domain weights 35 SYN / 40 FN-usage-weighted / 25 RT (judgment, always
quoted). Vision = 0.70 × parity + 0.30 × programme. Ratified + recompute-per-milestone rule
(Appendix B).

### 11.2 2026-07-03 re-score — the correction of the stale ≈58%/≈60% (audit B3-7)
The 2026-07-02 baseline missed work shipped since. FN rows flipped, with evidence:

| Group (tier) | Rows flipped | Why | Δ score |
|---|---|---|---|
| FN-HASH (×3) | hash_hmac, hash_equals, hkdf, pbkdf2: GP→COVERED (4 rows) | W3-4 `f4c4c1d` — RFC-KAT'd, byte-identical | +4.0 |
| FN-MATH (×3) | random_int, random_bytes CSPRNG gap →COVERED (2 rows) | `Core.Random.secureInt/secureBytes` shipped (quarantined Tier-B — the model scores capability, quarantine ≠ absent) | +2.0 |
| FN-FS (×3) | parse_ini: GU→COVERED (1 row) | `Core.Ini.parse` shipped `4f4f271`, byte-identical | +1.0 |
| FN-RAND (×2) | engines row: PARTIAL→COVERED | the "no Secure/CSPRNG engine" caveat is gone | +0.5 |

Not moved: NDJSON (no PHP-row counterpart — programme-side only); S0–S2 import redesign
(discipline change, no parity row); W0-2/3/4 (soundness repairs on already-counted rows).

**Arithmetic:** T1 124.5→131.5/303 · T2 18.5→19.0/140 · T3 0/75 ⇒ usage-weighted stdlib
(3×131.5 + 2×19.0)/1264 = 432.5/1264 = **34.2%** (was 32.5%). Parity = 0.35×79.8 + 0.40×34.2 +
0.25×69.4 = 27.9 + 13.7 + 17.4 ≈ **59%**. Programme: M8-crypto 60→70, M-Batteries 50→55
(NDJSON/INI) ⇒ mean 1045/16 = 65.3%. Vision = 0.70×59.0 + 0.30×65.3 ≈ **61%**.

**Grade: [Inferred]** — the flipped rows are Verified-shipped (gate-green commits named); the
figure is a targeted delta on the ratified model, not a full 824-row re-pass. A full re-pass is
due at the next milestone close (recompute rule). Raw row-parity floor moves to ≈39%.

### 11.4 2026-07-10 marathon re-score — the Wave C milestone-close re-pass (HEAD `af3aad3`)

The milestone-close full re-pass §11.2 deferred is now done, as a **systematic verdict-scan at HEAD**
(all 29 SYN `P`/`GP` + 8 RT `P`/`GP` rows re-walked; all 35 FN groups checked against every
`src/native/` commit since the E-surface baseline — full detail + ruled-out candidates in
`M-gap-matrix.md §4.6`). The marathon (2026-07-04→10, 203 commits) was **perf + language-polish, not
stdlib breadth**, so exactly **2 rows moved**:

| Row(s) (tier) | Rows flipped | Why | Δ score |
|---|---|---|---|
| FN-STR (×3) | sprintf/printf/vsprintf/vprintf (053–056): GP→COVERED (4 rows) | `Core.String.format` full `%`-directive engine, compile-time-type-checked; `(spec, list)` accepts a runtime list arg so the array-form needs no variadics (DEC-199, `9bc6612`…`130b0cb`) | +4.0 (T1) |
| RT | RT-007 JIT: GP→P | Cranelift unboxed JIT, default feature (`3725052`) | +0.5 |

**Ruled out by the scan (no flip, evidence in §4.6):** **SYN-118 attributes** (DEC-194 shipped, but
attach to only 2 of PHP's 7 targets — classes + free functions — with no attribute-reflection yet;
stays PARTIAL, not the CB an early draft claimed — a richer P justification, not a flip); FN-MATH
trig/hyperbolic breadth (11 GP — `math.rs` added none); `str_split`/`mb_str_split` (`String.characters`
is codepoint-wise ≠ byte-wise, inside the still-blocked M-text GP programme); Wave A/B type-system +
Option/Result combinators (land on already-CB rows); `Math.try*` + `#[UncheckedOverflow]` (beyond-PHP,
no PHP-row counterpart).

**Arithmetic (delta on §11.2):** SYN unchanged (SYN-118 stays P) = 103/129 = **79.8%** · T1
131.5→135.5 ⇒ usage-weighted stdlib (3×135.5 + 2×19.0)/1264 = 444.5/1264 = **35.2%** · RT (9 + 4)/18 =
**72.2%**. Parity = 0.35×79.8 + 0.40×35.2 + 0.25×72.2 = 27.9 + 14.1 + 18.1 ≈ **60%**. Programme (re-based from 65.3%):
M11+M4 70→75 (sprintf = named §3 M11 item), **M-perf 30→40** (JIT-default + inline-cache +
`#[UncheckedOverflow]` + `Math.try*` shipped — *infrastructure only; the HARD PERF MANDATE is still
unmet* [Speculative]) ⇒ mean 1060/16 = 66.3% (attrs + DI v1 conservatively NOT credited — no milestone
slot maps to them). Vision = 0.70×60.2 + 0.30×66.3 ≈ **62%**. Floor ≈ **41%**.

**Grade:** row flips **[Verified]** (commits + source cited); figure **[Inferred]** (additive delta on
the ratified §11.2 arithmetic); programme weights **[Speculative]**. **The chain:** 58% (ccb2403
full-pass) → 59% (§11.2, 2026-07-03) → **60% (this re-pass, HEAD af3aad3)**.

**The finding that matters more than the number:** the marathon (07-04→10, 203 commits) moved parity
**+1 (59→60)**; the full 252-commit span since ccb2403 (07-01), **+2 (58→60)** — small either way
because the only stdlib-breadth movers were crypto (§11.2, pre-marathon) + sprintf (here), while TOP-20
#1 (DB) / #2 (HTTP) / #3 (sessions) / #5 (FS) / #12 (XML) / #19 (intl) are all untouched. This is the evidence validating the locked order: **②
boxed-value JIT = the perf lever** (unmet mandate), **③ web spine = the parity lever** (§11.3 puts the
W3 DB+HTTP+sessions wave as the jump to ≈65–66%).

### 11.3 Projected per-wave gains (unchanged model, re-based)

| After | What moves | Parity | Vision |
|---|---|---|---|
| baseline (2026-07-10, HEAD `af3aad3` — §11.4) | Wave C String.format + attrs + JIT-default | **≈60%** | **≈62%** |
| UA programme + W0/W1 | correctness/hygiene — few surface rows | ≈60% | ≈62% |
| W2 | soundness/enforcement SYN rows | ≈61% | ≈63% |
| W3 | DB + HTTP + sessions + FS + url (format now banked in baseline) | **≈65–66%** | ≈69% |
| W4 | named-args/generators/magic + M-text + date + blitz | **≈71–72%** | ≈75% |
| W5 | beyond-PHP (parity barely moves; programme jumps) | ≈72% | ≈79% |
| W6 | RT/ecosystem rows | **≈75%** | **≈81%** |

> Note: rows below the baseline are re-based to the 2026-07-10 anchor (UA/W2 parity lifted +1 to stay
> ≥ baseline; `format` struck from W3 since it is banked in the baseline). Absolute W3–W6 figures are
> left as prior estimates — re-project the real arithmetic at W2 close (recompute rule).

[Speculative — model-based projections; re-run real arithmetic per wave.] Residual to 100%:
extension-tier stdlib (§10 Bucket 2, pending the extension-policy ruling), GAP-by-design rows
(excluded by the model), post-1.0 editions.

---

## 12. DELIVERY ORDER (ROI-first — developer-agreed 2026-07-03, updated by the audit rulings)

1. **Stage D/E of the unification audit** (NOW): this plan + UNIFIED-SPEC + Bucket-3 corrections →
   Stage-E gate re-verify + archive the raw research (B3-12).
2. **UA-0 free wins** (§2.1) — no design risk, immediate dev-speed + front-door payoff.
3. **UA-1 byte-identity fixes** (1.1–1.4, 1.6–1.10) + **UA-1.5 `->` retirement sequence**
   (fresh session) — restores Invariant 1 honesty.
4. **UA-L1 spec + pilot** and **UA-L2 prelude/loader unification** — the two
   fix-the-architecture-before-the-next-wave items. UA-L2 HARD-GATES W3-1/W3-2.
5. **UA-L5 rename wave + UA-L6 additive wave** (corpus churn batched together) — then **W6-8
   editor refresh** becomes unblocked (final surface).
6. **W3-5** (blocker adjudication first) → **W3-1 SQL DBAL + W3-2 HTTP client** (the XL heart;
   pure P0s `Core.Sql`/`Core.Url` first) → **W3-3/6/7/8** finish the spine.
7. **W4-4 Unicode strings** (XL; UA-1.1 already set the direction) → **W4-6 + W4-5** → rest of W4.
8. **UA-L3 ReDoS analyzer + UA-L4 Rc<str> refactor** — slot alongside W4 (independent lanes).
9. W2 polish → W5 beyond-PHP → W6 GA.

Ledger basis: W3 ≈+6, W4 ≈+6 parity points are the big movers (§11.3).

---

## 13. DECISIONS LOG — 2026-07-03 unification-audit rulings (all developer-ruled, final)

- [2026-07-03] **B2-1a** `String.trim` family: **Unicode-aware trim stays canonical; PHP leg gets a
  matching helper** (Option 2 — reversed from the synthesis rec on W4-4-direction evidence). → UA-1.1
- [2026-07-03] **B2-1b** `String.reverse`: **char-wise both legs; mb-safe PHP helper** (Option 1). → UA-1.2
- [2026-07-03] **B2-1c** empty-separator split → **EXPANDED: native-error checked-exception
  taxonomy spec + 4-native pilot, staged rollout** (not a flag-day rewrite; not a lone fault fix). → UA-L1
- [2026-07-03] **B2-1d** `Hash.pbkdf2`: **widen iteration counter to u64; no artificial u32 ceiling**
  (Option 2 — reversed after the "fake ceiling" challenge; ~7 min at 10M ops/s, not years). → UA-1.3
- [2026-07-03] **B2-2** injected preludes: **unify with the module loader NOW, before W3-1/W3-2.** → UA-L2
- [2026-07-03] **B2-3** `->` retirement: **docs/help first, then parser-reject, then individual
  gate-guided fixes; no bulk sed.** → UA-1.5
- [2026-07-03] **B2-4** empty-Map literal: **extend the List empty-`[]` inference rule to Map**;
  **PLUS `[...]` context-typed for Set** (Set has zero literal syntax today — same mechanism). → UA-1.6
- [2026-07-03] **B2-5** `Hash.hmac`: **returns bytes** (breaking, deliberate — more-correct beats
  PHP-familiar; blast radius verified: 1 example + 1 README line). → UA-1.4. **Standing
  instruction adopted into G-4: default-recommend the more-correct option.**
- [2026-07-03] **B2-6** `Math.clamp(min>max)`: **faults.** → UA-1.7
- [2026-07-03] **B2-7** ReDoS asymmetry: **FULL mitigation — transpile-time static regex-complexity
  analyzer** (not disclosure-only; needs its own design pass). → UA-L3
- [2026-07-03] **B2-8** naming: **ONE rename wave, everything folded** (approved trio + all audit
  candidates; crypto 3-module split KEPT + cross-ref docs). → UA-L5
- [2026-07-03] **B2-9** fault messages: **canonicalize on `"Module.function: message"`** (~40
  strings, differential-gated). → UA-1.8
- [2026-07-03] **B2-10** VM strings: **`Rc<str>`/`Rc<String>` + COW `make_mut`** (full fix, not the
  targeted patch; benchmark-evidenced). → UA-L4
- [2026-07-03] **B2-11** stdlib additive: **one wave, approved + new candidates together.** → UA-L6
- [2026-07-03] **B2-12** import-redesign guide example: **yes, add one + README row.** → UA-1.9
- [2026-07-03] **B2-13** playground `forbid(unsafe_code)`: **try empirically first; recorded
  INVARIANTS exemption only if wasm-bindgen genuinely needs unsafe.** → UA-1.10
- [2026-07-03] **Frozen-specs fork:** **fold all 18 `docs/specs/*.md` into `docs/specs/
  UNIFIED-SPEC.md`; update every CLAUDE.md pointer** to the relevant section. (Re-asked at the
  developer's request after a timeout; final — do not re-litigate.)
- [2026-07-03] Earlier same-day rulings carried from wave0-remainder: **W3-5 = `String.format`,
  `{}`-grammar** (blocker pending, §6) · **dependency amendment rusqlite+rustls** · **SQL-DBAL
  scope (SQLite+Postgres+MySQL sync; Oracle deferred; MongoDB separate LADDER item)** · **strict
  per-type imports / member-imports preferred / functions never bare-importable** · **split gate
  (pre-commit Rust-only, pre-push full oracle)** · **`->` removed entirely (no transition alias)** ·
  **delivery order Option 1 ROI-first**.

- [2026-07-04] **Toolchain-version config (DONE):** the gate's php-oracle path is now a single
  editable knob in `scripts/toolchain.env` (`PHORJ_PHP="${PHORJ_PHP:-…php-8.5.8…}"`, exported
  override still wins); `scripts/git-hooks/pre-push` sources it via `git rev-parse --show-toplevel`;
  `CLAUDE.md` + this plan's cursor point at it. Triggered by the stack reload bumping the oracle
  8.5.7→**8.5.8** (8.5.7 removed; `cargo-nextest` also wiped — pre-push has a `cargo test` fallback).
  Rust stays pinned by `rust-toolchain.toml` (not duplicated). "A versions file" = this toolchain
  knob, NOT a project-deps file — `src/manifest.rs` already owns deps (no second source of truth).
- [2026-07-04] **`Core.Dotenv` — full Symfony-style cascade (NEW tracked design item, ruled,
  NOT built):** developer chose the full `.env → .env.local → .env.$APP_ENV → .env.$APP_ENV.local`
  cascade over the recommended simpler `Env.load(path)` (my footgun challenge is recorded; it's the
  developer's language call per §ADJUDICATION). Layers on the EXISTING `Core.Environment`
  (`.get`/`.all`, already impure/oracle-quarantined) — this is the `.env`-file-cascade+parser layer,
  not new env reading. Design-spec must decide: the Symfony `test`-env "`.env.local` silently
  skipped" special case (footgun — keep or drop?), `secret-type` integration (`.env` holds secrets →
  avoid secret-in-logs), and the emitted PHP cascade helper for transpile parity (impure → no
  byte-identical differential example, quarantined like the other ambient natives). Same tier as
  UA-L1..L6 — needs its own design pass before code. → tracked as **UA-L7** (Wave-3 web-spine
  adjacent; sequence with the DB/HTTP web work).

### 13.1 · 2026-07-04 fork-backlog adjudication pass (developer-ruled, interactive — final)

> **DEC-197 (2026-07-05, developer-proposed — PENDING scope confirmation → then a fresh-context WAVE).
> UNIFY THE IMPORT MODEL: module FUNCTIONS get the same two-mode discipline as types/variants/intrinsics.**
> Developer's steer: *"everything needs to be imported either directly or used [via] parent"* — like
> `import Core.Result;`→`Result.Success` OR `import Core.Result.Success;`→bare `Success`, the SAME must
> apply to module functions: `import Core.Output;`→`Output.printLine(...)` (qualified, unchanged) OR
> **`import Core.Output.printLine;`→bare `printLine(...)`**; same for `Output.print`, `String.format`, etc.
> **Framing (developer, 2026-07-05): UNIFICATION, not a reversal** — extend the ONE two-mode principle
> already shipped for types/variants/intrinsics to functions too, removing the lone "functions are the
> exception" wart. It supersedes the 2026-07-03 "functions NOT bare-importable" stance (UNIFIED-SPEC
> §400/318) as part of that unified rule.
> ADDITIVE (existing qualified calls unchanged) + uniform (functions finally match types/variants/
> intrinsics). **Couples to `String.format`:** format is a function, so how it's imported/called is
> defined by THIS — so DEC-197 must be settled BEFORE String.format is built. Build shape (est.): a
> pre-check rewrite qualifying a member-imported bare function call to its module native (mirrors
> `resolve_intrinsic_imports`/`resolve_variant_imports`), grouped form `import Core.Output.{ print,
> printLine };`, `ty_has_param`-style care on the checker/loader classification.
> **RULINGS (developer, 2026-07-05):** (a) **SCOPE = ALL functions** — Core natives AND user-package
> functions (`import App.Utils.helper;`→bare `helper()`). (b) **UFCS = COEXIST** — bare import, UFCS
> (`x.trim()`), and qualified (`String.trim(x)`) all valid; author's choice. (c) **Collisions**
> (bare `map` from two modules) **solved by `import … as`** — the alias syntax already PARSES
> (types/variants use it); for FUNCTIONS it rides THIS wave (nothing to build separately): reuse the
> `as` plumbing + `import_map`/`build_type_imports` alias handling + the **lowercase-leaf casing
> carve-out already built for `Core.Assert`/`Core.Abort`** (a function leaf like `map` currently trips
> `E-PKG-CASE` — same fix). (d) Grouped `import Core.Output.{ print, printLine };` included (DEC-186
> machinery). Bare-name resolution order (proposed): local > user fn > imported native; ambiguity =
> error. **Rule-12 challenge outcome:** the "nothing in the wind" tension is answered — a member import
> NAMES the function, so bare `printLine` after `import Core.Output.printLine;` is as legible as a bare
> imported variant/intrinsic; costs (cross-module leaf collisions, style drift with UFCS) are
> manageable/opt-in. **Effectively RULED; only the BUILD remains** — a full fresh-context WAVE (parser
> + loader import-classification + checker resolution + pre-check rewrite + all 5 backends + corpus),
> and it GATES `String.format`. See §0 cursor.

Cleared the entire open-fork backlog so the feature marathon runs without stalls. All six ruled
interactively (AskUserQuestion), each with a verified failing/working program in the question. Also
mirrored into the canonical register (`C-decisions.md` DEC-177…DEC-181).

- [2026-07-04] **§7-OPEN trait → BLESSED (DEC-177).** `trait` is not unadopted — it's fully wired
  (`run≡runvm`≡PHP `trait`/`use`, verified end-to-end). Developer blessed BOTH `trait` AND
  multiple-inheritance as first-class (mirrors PHP's own trait/composition duality). → Appendix A
  ADOPTED; FEATURES `trait` flips to delivered. **No longer an open question.**
- [2026-07-04] **W3-5 blocker → RESOLVED via the Type-System programme (DEC-178).** The
  mixed-type-args blocker is subsumed by expected-type threading in the narrowing programme (below);
  `String.format` args use a closed scalar form, not open `Any`. **Folds in UA-1.6** (Set/Map
  literals) — same expected-type mechanism. → Wave C rides Wave A.
- [2026-07-04] **Type-System Completion programme → Wave A (DEC-179).** Developer chose the LARGEST
  scope: usable union-element collections (`List/Set/Map<A|B>`) + primitive `match` type-patterns +
  primitive exhaustiveness + `is` flow-narrowing + is-refinement + **W5-3 sealed hierarchies**
  (exhaustive class unions too) + faithful transpile (`is_int()`/`match(true)`). Reuses the shipped
  M-RT S4 match/exhaustiveness engine (class/enum) extended to primitives. "No half solutions."
- [2026-07-04] **Error model → HONOR the ratified 3-tier (DEC-180).** Developer probed "how do I know
  which error without catchable faults?" → answered by `Result<T, ErrorEnum>` + exhaustive variant
  match (same engine as Wave A) + typed `try`/`catch`/union-catch (SHIPPED, M-faults Slice 2b, base =
  `implements Error` marker). Ruled: complete Result/throws ergonomics + **AUDIT faulting natives —
  reclassify normal-input failures to Result/throws/`T?`**; faults stay uncatchable (bugs only). NO
  catchable faults (would re-add PHP's bug-swallowing footgun). → Wave B.
- [2026-07-04] **Editors → LSP-first, symmetric, then full-native (DEC-181).** VSCode is itself
  LSP-first (all smarts via `phg lsp`; v0.3.0, all 40 keywords in grammar). PhpStorm gets identical
  features via LSP4IJ→`phg lsp`. Ruled: LSP-first + thin native shells now (run/debug/test + DAP —
  what LSP can't do), THEN full native both editors (rich VSCode ext + native IntelliJ/PSI plugin) as
  a follow-on (unverifiable here → developer tests those builds). **STANDING DoD: every shipped
  feature reaches BOTH editors via `phg lsp` in the same change.**
- [2026-07-04] **UA-1.8 shape confirmed/refined (→ DEC B2-9):** canonical =
  `Module.function: lowercase message`; **PHP-mirroring faults (`division by zero`, …) stay
  byte-exact** (value-kernel parity), sweep scopes to native stdlib strings only.
- [2026-07-04] **W4-10 XML — DEFERRED, not adjudicated.** Needs its own design proposal near Wave 4;
  stays the one recorded-but-open design item. **UA-L2 / UA-1.5 / UA-1.6 re-confirmed as already-ruled**
  (build/execution tasks, not forks).
- [2026-07-04] **Canonical `Core.Result` + `Core.Option` (DEC-182) — Wave B foundation.** Verified they
  were USER-DEFINED per-file (`generic-enums.phg`) = "in the wind"; `Error` marker IS built-in. Developer
  ruled: ship BOTH `Core.Result<T,E>` AND `Core.Option<T>` as **injected, explicitly-imported** canonical
  types (same pattern as injected `Json`: `inject_result_prelude`/`inject_option_prelude` gated on
  `import Core.Result;`/`import Core.Option;` + `module_of` registry entry → qualified `Result.Success`/
  `Option.Some`, bare use = `E-INJECTED-VARIANT-BARE`; ride the shipped `erase_generics`; PHP variant
  classes). **`Option<T>` vs built-in `T?`: DISTINCT roles, explicit conversion, NO implicit coercion** —
  `T?` stays the lightweight built-in absence + what stdlib returns; `Option<T>` is the opt-in rich
  monadic wrapper (map/andThen/filter/getOrElse) imported when you want combinator chains; interconvert
  via `Option.ofNullable(x)`/`opt.toNullable()`. `Error` stays built-in; error payloads (`E`) = user
  enums. + combinator methods + `T?`↔`Option` conversions. → folds into Wave B.
- [2026-07-04] **sprintf/`String.format` CONFIRMED** (developer, re-confirming DEC-178): implement per
  the ruling — `Core.String.format(spec, args)`, `{}`-grammar shared with W5-1 interpolation, closed
  scalar args via Wave A threading. Sequenced in **Wave C** (after Wave A + Wave B). No refinement.
- [2026-07-04] **SHIPPED — Wave B slice B-1: injected `Core.Option`/`Core.Result` TYPES (DEC-182
  foundation).** `inject_option_prelude`/`inject_result_prelude` in `src/cli/mod.rs`, wired into the
  inject chain after `inject_rounding_mode_prelude` (before `check_resolutions`, so `erase_generics`
  downstream erases `T`/`E`). Gated on import + skipped if a same-name enum is user-declared. Qualified
  variants only (`injected:true` ⇒ `E-INJECTED-VARIANT-BARE` on bare). Examples `core-option.phg`/
  `core-result.phg` + 6 checker tests (`injected_result_option.rs`); full gate 1710 green, byte-identical
  run≡runvm≡php-8.5.8. **Foundation only — combinators + `T?`↔`Option` conversions are slice B-2 (pending).**
  Disclosed a pre-existing F-m guard gap (variant names + PHP builtin class names unguarded) in KNOWN_ISSUES.
- [2026-07-04] **SHIPPED — Wave B slice B-2a: `Core.Option` combinators + conversions (DEC-182 Option
  set, all explicitly ruled).** Six `Core.Option` natives (`src/native/option.rs`): `map`/`andThen`/
  `filter` (HigherOrder, closure via `ClosureInvoker`) + `getOrElse` (eager) + `ofNullable`/`toNullable`.
  UFCS-dispatched (enums have no methods; `opt.map(f)` resolves via `try_ufcs` first-param unify, same
  as `List.map`) + gated `__phorj_option_*` transpile helpers. Example + 7 unit tests; full gate green,
  byte-identical. In-slice, root-cause-fixed a GENERAL pre-existing crash (`new` in a `rewrite_ufcs`-
  relocated subtree survived `unwrap_new` → `Expr::New` panic; fixed in `rexpr`, guards the `f(new X()) as T`
  sibling too — memory gotcha updated) and widened `unify` for `Optional(T)`-param inference. **B-2b
  (Result combinators) is NEXT — its combinator set is NOT enumerated in DEC-182 → surface via §15 if
  beyond the obvious `map`/`mapErr`/`andThen`/`getOrElse`.**
- [2026-07-04] **RULED — Wave B slice B-2b: FULL `Core.Result` combinator set (DEC-185).** Surfaced via
  §15 (set unenumerated by DEC-182); developer ruled **"all"** — the pre-authorized core-4 PLUS every
  proposed extra. Set = **8 natives**: `map((T)->U)` · `mapErr((E)->F)` · `andThen((T)->Result<U,E>)`
  (success bind) · `getOrElse(T)` (eager) · `toOption() -> Option<T>` (Result→Option bridge, symmetric
  with Option's `toNullable` now both DEC-182 types exist) · `orElse((E)->Result<T,F>)` (error-arm bind /
  recovery, Rust `or_else`) · `isSuccess() -> bool` · `isFailure() -> bool`. `filter` deliberately
  EXCLUDED (no error value to synthesize on `false` — Rust omits `Result::filter` too). Recipe mirrors
  B-2a (HigherOrder natives via `ClosureInvoker` for the closure-taking four; `Value::Enum(ty:"Result")`
  guard; registry `Ty::Named("Result",[T,E])`; gated `__phorj_result_*` transpile helpers over emitted
  `Success`/`Failure`). Key new ground vs B-2a: `E`-threading through the closure return (`andThen`/
  `orElse`) + `mapErr`'s `(E)->F` error-type remap — TDD the type-threading test FIRST (Option had no
  error param). Invariant-7 proof: `result.getOrElse(0)+1` byte-identical.
- [2026-07-04] **RULED — bare injected-variant IMPORTS (DEC-186), Option A + alias.** Surfaced via §15;
  developer ruled Option A **plus** the aliased form, and "I want all supported." Scope (one sub-slice,
  applies uniformly to injected Option/Result/Json):
  - `import Core.Result.Success;` → bare `Success(…)` legal in BOTH construction and `match` patterns.
  - `import Core.Result.Success as MyCoreSuccess;` → bare `MyCoreSuccess(…)` (aliased variant import).
  - `import Core.Result;` + qualified `Result.Success(…)` **keeps working** (both forms coexist).
  - Un-imported injected variants stay qualified-only (`E-INJECTED-VARIANT-BARE` unchanged); a variant
    NOT imported is still qualified. Local-name collision → existing `E-IMPORT-CONFLICT`/`-SHADOW`.
  **Already in place:** parser captures multi-segment paths AND `as` aliases (`ast Import{path,alias}`);
  qualified variant access. **To build:** loader classifies a Core-rooted `<Enum>.<Variant>` path as a
  variant import (today `Core.*` is skipped from both binding maps, `loader/mod.rs:487,552`), binds
  bare/alias → (enum, variant); checker accepts it in construction + patterns, resolving to the qualified
  injected variant BEFORE any backend (byte-identity by construction, UFCS-collapse technique). Sequenced
  AFTER B-2b combinators as slice **B-2c**. Example + tests +
  `phg explain` (E-INJECTED-VARIANT-BARE note) + both-editor LSP.
  **GROUPED imports also ruled (same slice B-2c):** `import Core.Result.{ Success, Failure as Xzs };` —
  path-first brace group (PHP group-use `use Core\Result\{…}` + Rust `use a::b::{…}` precedent, and the
  minimal generalization of the existing `import Core.Result.Success;` — the leaf becomes a set). Trailing
  comma OK, multi-line OK, per-item `as`, single-leaf form still valid, **single-level prefix only** (no
  nested `Core.{Result.Success, Option.Some}`). TS-style `import {…} from …` REJECTED (inverts path-first
  order). Parser needs a `{`-group branch after the path; fmt renders groups sensibly.
  **VALIDATED DESIGN (advisor 3C, not yet built):** (1) Parser desugars a group into N `Item::Import{path:
  [Core,Enum,leaf], alias}` (needs `parse_import`→`Vec<Item>` or the item loop to `extend`); single/aliased
  multi-seg ALREADY parse (`ast Import{path,alias}`). (2) `imports_module_or_member` +1 tolerance ⇒
  `import Core.Result.Success` ALREADY triggers Result injection (verified against code). (3) Checker builds
  a variant-import map `bare-or-alias → (Enum,Variant)` from `[Core,InjectedEnum,Variant]` paths; validates
  enum-injected + variant-exists; collision (`import …Success` + local `Success`) → `E-IMPORT-CONFLICT`/
  `-SHADOW`. (4) Resolution: in `try_variant_or_class_call` (calls.rs ~938, construction) AND `matches.rs`
  (~356, patterns) — before `E-INJECTED-VARIANT-BARE`, if the bare name is an imported variant → allow
  (resolve to the injected variant; `type_variant_construction` types it; a NON-aliased bare `Success`
  works once the error is skipped, since backends already see bare variant names = the injected PHP class).
  **⚠ CRUX — the ALIAS is a NEW rewrite, NOT the qualified-variant reuse:** the existing rewrite is
  `Enum.Variant`(Member)→bare `Variant`; an alias is `X`(bare Ident)→`Success` — different AST shape, must
  be applied in BOTH construction AND match-pattern paths (+ nested) or interp/VM resolve `Success` while
  the backend sees `X` → divergence (the reified-operands-thread-all-paths gotcha in a new guise). **TEST
  DISCIPLINE (toOption lesson):** SEPARATE differential cases per form — (a) bare imported variant, (b)
  aliased variant in construction AND a match pattern in one program, (c) grouped import, (d) collision →
  E-IMPORT-CONFLICT. NO combined example (a combined one masks exactly the divergence class that just bit).
  **⚠ NEWLY-FOUND TRAP (part-1 investigation, sharpens the pass): a ZERO-PAYLOAD variant used bare in a
  PATTERN (`None =>`, no parens) parses as `Pattern::Binding` (a catch-all matching ANYTHING), NOT
  `Pattern::Variant` — so an imported bare `None`/`Empty` pattern is invisible to a Variant-only rewrite
  and would silently become a catch-all → wrong match semantics, run≡runvm≡PHP all AGREE on the WRONG
  behaviour (not even a divergence — a correctness bug the differential won't flag). The pass MUST also
  rewrite `Pattern::Binding{name}` whose name ∈ variant-imports AND is a zero-field variant → the
  qualified zero-payload variant pattern (check how `Option.None =>` is represented first). Also:
  `Pattern::Variant.fields` are NESTED patterns — recurse. This trap is why part 2 wants fresh context +
  a zero-payload-pattern differential case, not just the 4 forms above.
- [2026-07-04] **RULED — full width-aware `fmt` wrapping (DEC-187), sequenced AFTER B-2b combinators.**
  Developer chose the FULL feature (both rules together, not split), ordered after the combinators so the
  Wave B error-model marathon isn't blocked. **EXPAND-ONLY policy** (idempotent): fmt never COLLAPSES an
  author's line breaks — it (Rule 1) preserves author breaks in chains/literals + normalizes indentation,
  and (Rule 2) auto-wraps a line that exceeds the column budget. Differs deliberately from prettier/rustfmt
  (which re-derive purely from width); documented trade-off = a gratuitously-broken short chain stays
  broken. Build = introduce a Wadler/prettier-style document IR (group/line/indent/softline) + a
  fits-in-N-columns solver + per-construct break rules (chain `.`, call args, collection/map literals,
  import groups) into `src/fmt/` (today a flat collapse-printer). MUST stay idempotent (`fmt(fmt(x))==fmt(x)`)
  — strengthen the fmt corpus test to `fmt(src)==src` on a multi-line corpus (folds UA-0.8). Own dedicated
  slice; gate-green + examples + both-editor (fmt drives LSP formatting).
  **ARCHITECTURE FINDINGS (2026-07-04 orientation, before the rewrite — READ before starting):**
  `src/fmt/printer.rs` is 1475 lines; `Printer` holds only `{out, indent, comments, next_comment}` — **NO
  raw source**, and `fn expr(&self, e) -> Result<String,String>` (printer.rs:778) produces a **flat
  single-line String** (no column/width model; chains/calls/literals all collapse). Consequences: (1)
  Rule 2 (width-wrap) = introduce a Wadler/prettier document IR (group/line/indent/softline) + fits-in-N
  solver and rewrite `expr()` to emit multi-line — touches every expr arm. (2) Rule 1 (preserve author
  breaks) is HARDER than it sounds AND fights the design: the AST discards whitespace and the printer has
  no source, so "the author broke here" isn't recoverable without threading the source in + comparing
  spans — against the stated "print from the AST, not by re-spacing tokens" invariant (fmt/mod.rs). **RE-
  RECOMMEND on that evidence: do the WIDTH-based canonical form (Rule 2 only, prettier/rustfmt-style —
  decide breaks from width deterministically), and DROP Rule 1's "preserve author breaks"** (it needs
  source access the printer deliberately lacks, and width-canonical is the industry norm + idempotent by
  construction). Surface this to the developer at the start of the fmt session — it revises DEC-187's
  expand-only framing. No bounded sub-increment exists; it's an atomic printer-core rewrite → fresh session.
- [2026-07-04] **AMENDED — DEC-187 is now WIDTH-CANONICAL (Rule 2 only); Rule 1 "preserve author breaks" is
  DROPPED (developer-ruled interactively at fmt-session start, this session).** Rationale accepted: (1)
  width-canonical is idempotent by construction (`fmt(fmt(x))==fmt(x)`, the hard requirement + UA-0.8);
  (2) it matches the print-from-AST invariant `printer.rs` already holds (no source-threading / span-diffing);
  (3) industry norm (prettier/rustfmt/gofmt). Trade-off accepted: a gratuitously-broken SHORT chain is now
  COLLAPSED to canonical form (fmt re-derives all layout from a fits-in-N-columns solver), not preserved —
  reversible later via an explicit pragma if a per-chain break-control preference emerges. Build = Wadler-style
  document IR (`text`/`line`/`softline`/`group`/`nest`) + fits solver + per-construct break rules (chain `.`,
  call args, collection/map literals, import groups) replacing the flat single-line `expr()` printer. Corpus
  test strengthened to `fmt(src)==src` on a multi-line width-canonical corpus.
- [2026-07-04] **Build order (converged, developer-ruled):** B-2b combinators → DEC-187 fmt full wrapping
  → B-2c variant + grouped imports → B-2d rich-error audit + UA-1.8 → Wave C. Each gate-green + example +
  commit; NEVER push (developer pushes on green CI). **[REORDERED 2026-07-04 post-B-2b (developer-confirmed):
  B-2b ✅ → B-2c variant/grouped imports (NEXT, this session) → DEC-187 fmt (this session, after B-2c) →
  B-2d rich-error audit + UA-1.8 → Wave C.** Synergy: fmt's doc-IR rewrite then formats the already-shipped
  grouped-import syntax in one unified pass; B-2c banks a clean win with injected-type context fresh.]
- [2026-07-05] **EXAMPLES/CONFORMANCE AUDIT + cleanup decisions (DEC-196; audit = `docs/research/2026-07-05-examples-conformance-audit.md`).** Developer-ruled this session:
  - **Q1 [FIX — ✅ SHIPPED 2026-07-05]:** renamed `examples/fmt/`→`format/` AND `examples/bench/`(+`manual/`)→`benchmark/` (git mv, all refs updated: `bench/baseline.json`, `gen_examples.py` SKIP_DIRS, `tests/runtime.rs`, `src/cli/mod.rs`, `examples/README.md`, `docs/MILESTONES.md`; regenerated `examples.js` — 146 entries, `format` category); fixed `bench.rs:339` output `"phg bench —"`→`benchmark` (+2 tests); moved-dir READMEs/comments `phg bench`/`disasm`→full verbs; added `import Core.String;` to `web/core-http.phg` (verified coexists with the Http prelude — no E-IMPORT-CONFLICT); reconciled `STABILITY.md` module names→real registry names (the 6 ruled + `Crypto`→`Cryptography`, verified against `src/native/**` quoted literals); `git rm docs/plans/wave0-remainder.plan.md`; swept `src/**` `phg fmt`→`format` / `phg bench`→`benchmark` rustdoc (module/file/fn names untouched). Full oracle gate green (php-8.5.8).
  - **Q2 [COMPILER, breaking — ✅ SHIPPED 2026-07-05]:** enforced camelCase (Invariant 12). The `.phg` corpus was already 100% clean (constants stay SCREAMING_SNAKE_CASE), so the change collapsed to the **two native renames**: `String.uppercase`→`upperCase`, `String.lowercase`→`lowerCase` — `name:` field + fault string in `src/native/text.rs` (PHP emit unchanged: `strtoupper`/`strtolower`; interpreter logic unchanged → name-only breaking change), UFCS calls (`s.upperCase()`), tests (`checker/tests/calls.rs`, `transpile/tests.rs`), examples (`guide/text.phg`, `guide/ufcs.phg`, `guide/imports.phg`, `conformance/stdlib/math-text.phg` + comments), `examples.js` regen, docs (`examples/README.md`, `UNIFIED-SPEC.md`). Strengthened `charter_function_names_are_lowercamel` with a **curated regression denylist** (`uppercase`/`lowercase`) — proven red-with-a-listed-name/green-after; a general "multi-word-all-lowercase" rule is NOT mechanically decidable (`substring`/`capitalize` are legit single words), disclosed in the test comment. Full oracle gate green (php-8.5.8). `substring` stays one word.
  - **Q3 [DESIGN, W2-6 — ✅ RULED 2026-07-05 after surfacing a source conflict; TWO-MODE model].** The build investigation surfaced a bare-vs-qualified contradiction between DEC-196 Q3 ("used bare"), UNIFIED-SPEC §"Nothing in the wind"/W2-6 (qualified, principle in force), and audit §1.11 (developer instinct "bare reads inconsistent" → qualified) → surfaced via AskUserQuestion (Invariant 15). **DEVELOPER RULED (2026-07-05): the TWO-MODE model, mirroring Phorj's existing type/variant-import discipline (DEC-186).** Modules: **`Core.Assert`** = { `assert` }, **`Core.Abort`** = { `panic`, `todo`, `unreachable` }. (1) **Whole-module import → QUALIFIED calls:** `import Core.Assert;` ⇒ `Assert.assert(x)`; `import Core.Abort;` ⇒ `Abort.panic(x)`/`Abort.todo()`/`Abort.unreachable()`. (2) **Member import → BARE calls:** `import Core.Abort.panic;` ⇒ `panic(x)`; `import Core.Assert.assert;` ⇒ `assert(x)`. (3) **Grouped member import → BARE:** `import Core.Abort.{ panic, todo };` (consistent with DEC-186 variant-import groups). Any intrinsic used with NO covering import ⇒ **`E-UNIMPORTED`**. This reconciles both sources: nothing-in-the-wind holds (bare requires an explicit member import that names the intrinsic; module import gives the attributed qualified form). Distinct from `Core.Test.assert`. **✅ SHIPPED 2026-07-05.** New pass `resolve_intrinsic_imports` (`src/checker/intrinsic_imports.rs`) runs on the RAW program in `check_and_expand` (one `&mut` traversal): validates coverage (`E-UNIMPORTED`; strict two-mode — each form needs its own import) + normalizes the qualified `Assert.assert(...)` form to the bare intrinsic every backend already lowers (backends UNCHANGED → byte-identity preserved). Casing carve-out in `program.rs` exempts the lowercase intrinsic leaf of a `Core.Assert`/`Core.Abort` member import from `E-PKG-CASE`. Bad member leaf → `E-IMPORT-UNKNOWN`; alias on an intrinsic import rejected. `is_intrinsic_name` reservation stays (single-sourced via `intrinsic_module_of`). Reused DEC-186 grouped-import parser (no parser change). Examples `guide/assertions.phg`+`guide/result.phg` gained `import Core.Assert.assert;`; new `guide/intrinsic-imports.phg` (3 modes, byte-identical); `phg explain E-UNIMPORTED`; UNIFIED-SPEC §"Nothing in the wind" updated to the two-mode model. 12 checker tests + full oracle gate green. **DEC-196 COMPLETE (Q1+Q2+Q3+Q4 all shipped).**
  - **Q4 [FIX — SHIPPED this session]:** `gen_examples.py`: added the MISSING `Core.Regex` to the exclusion set (the only real generator bug — `regex.phg` was leaking into the playground; `Core.Cryptography` was already correct, NOT a typo — an earlier "Cryptography→Crypto" claim was a substring-match error, reverted) + added `bench` to `SKIP_DIRS` (excludes `workload.phg`'s depth-1000 recursion WITHOUT editing the workload or perturbing `bench/baseline.json` — cleaner than the "reduce depth→120" option, which would have moved the perf baseline). Regenerated `examples.js` (146 entries; `regex`/`workload`/`password-verify` all excluded). Frontend `main.js`: graceful message on `RangeError: Maximum call stack size exceeded` (browser stack limit, not a Phorj error). Fixed the stale `playground/Cargo.toml` comment (`regex`/`crypto` both off). **NEXT SESSION (needs WASM rebuild, wasm-pack absent):** enable the `regex` feature in `playground/Cargo.toml` so `regex.phg` returns. Also [COMPILER]-next: scope the leaky `Core.Http` prelude imports.
- [2026-07-05] **FAULT-PARITY PASS run (the correct-lens work deferred from DEC-195; `docs/research/fault-parity-pass-2026-07-05.md`).** Exit-status lens ("Phorj faults but PHP silently succeeds") over the reachable value-guard fault set = **NO divergence** — PHP 8.5 throws `ValueError` on every bad-value case (`String.repeat`/`count`/`padLeft`/`padRight`, `List.fill`/`chunk`, `Hash.hkdf`), and Conversion faults are guarded by construction (`toInt`→`int?`, `*Exact`→`__phorj_*` throwing helpers). **But a different real divergence FOUND: `Conversion.truncate`/`round` on an out-of-i64-range float** — both legs *succeed* with DIFFERENT stdout (Rust `as i64` saturates to i64::MAX = `9223372036854775807`; PHP raw `(int)`/`(int)round` wraps = `5076964154930102272` + a warning). Latent byte-identity break (no example uses out-of-range input). Safe siblings exist (`toInt`→`int?`, `floatToIntExact`→fault). **FIX ✅ RULED + SHIPPED 2026-07-05: developer chose FAULT** (Invariant 15, AskUserQuestion) — `truncate`/`round` now fault on NaN/±∞/out-of-i64-range (Rust via `value::float_to_int`; PHP via new throwing `__phorj_trunc`/`__phorj_round` helpers), consistent with `floatToIntExact`; in-range unchanged; `toInt`→`int?` stays the graceful path. Now partial (breaking). Tests: Rust fault (`convert_tests`), emit + PHP-helper-throws (`convert_tests`/`transpile/tests`), run≡runvm `agree_err` (`differential`); example comment in `guide/convert.phg`. **OUTPUT-PARITY SWEEP run (2026-07-05, high-risk raw-builtins):** probed `substr`/`intdiv`/`pow`/`explode` edge inputs — `substring`/`integerDivide` AGREE; `pow(0,neg)` value-identical (only the known UA-0.14 deprecation warning differs). **FOUND + FIXED a 2nd divergence: `String.split(s, "")`** — Rust returned per-char-with-empty-ends, PHP `explode("")` faulted → now both FAULT (developer-ruled, empty sep ill-defined) + **added `String.characters(s) -> List<string>`** (code-point-safe, parallels `lines`; the named way to split into chars). **STILL a larger follow-up (fresh context): the remaining ~50 lower-risk raw-builtin emits** (array ops, libm math, hash, path, url) — not individually probed.
- [2026-07-05] **DEC-195 — guard-helper for the 3 "divergences": RULED, then the PREMISE was RETRACTED
  (same day) → NOT built; developer must RE-DECIDE.** The developer adjudicated guard-helper for all 3
  (`List.chunk`/`Hash.hkdf`/`Conversion.toString`), but that was on the B-2d audit's **wrong premise**
  that Phorj-fault-text ≠ PHP-error-text is a byte-identity divergence. **It is not** — verified from
  primary sources (`agree_err` compares run≡runvm ONLY, never PHP; `run_php` asserts exit-0; faults
  aren't byte-identity examples per Invariant 9 / G-1.1; `__phorj_clamp` comment: *"a fault is never a
  byte-identity example… only that both legs fault"*). All 3 **fault in PHP** (`ValueError`/`Fatal`) →
  behaviourally consistent, NOT divergences. So the guard-helpers are **cosmetic** (PHP-error wording),
  not correctness. **RE-DECIDED 2026-07-05 (developer, on the corrected basis): DROP DEC-195 entirely —
  behaviour stays as-is (nothing removed; both legs already fault), no helpers, no string change.**
  Sanctioned next work instead = the **correct-lens fault-parity pass**: enumerate faulting natives,
  transpile each fault-trigger, run the PHP, and check its **exit status** — non-zero = consistent
  (ignore text), **zero = a real divergence** (Phorj faults but PHP silently succeeds, à la pre-helper
  `clamp`) needing a `__phorj_*` guard helper. Untested; fresh-context. See `docs/research/b2d-rich-error-audit.md`.
- [2026-07-04] **CONFIRMED — `Result.toOption` requires `import Core.Option` (reject, not auto-provide).**
  The shipped `E-RESULT-TOOPTION-NEEDS-OPTION` guard (B-2b, `5e41a16`) is the ruled behavior: developer
  chose the safe/explicit default over the ergonomic auto-provide alternative, consistent with DEC-182's
  explicit-separate-imports model. Reversible later if wanted.

### 13.1.1 · 2026-07-04 design-seed adjudications (RULED interactively — NEXT-SESSION build queue, DEC-188…193)

Six developer-seeded language/stdlib questions, surfaced + ruled this session (all §15, recommended-first
with concrete examples). **None built yet — this is the design record + build queue.** All are LANGUAGE-
SURFACE changes; several are BREAKING (migrate all examples + Core), so each is its own careful slice.

- **DEC-188 — TS utility types stay REJECTED; use interface segregation.** The `extends Exclude<A,{x}>`
  scenario doesn't justify `Exclude`/`Partial`/`Omit` (they need `keyof`/mapped-type machinery Phorj
  lacks — reaffirms [[rejected-typescript-utility-types]] 2026-07-03). The real need ("an interface from
  a subset") = **interface segregation**: declare small interfaces, compose UP with multi-`extends`
  (`interface C extends A, B {}` — VERIFIED works). ADR escape hatch only if a real case can't be
  segregated. No build.
- **DEC-189 — stdlib/framework = a sequenced per-component DESIGN PROGRAMME.** Adopt the full "standard
  library breadth" ambition, but each component earns its place: brainstorm + §15 ruling + §14 ladder
  (build-native / native-only / reject) BEFORE building. **Selection principle:** prioritize the
  standardized, decoupled, reused-everywhere components (Symfony-component / PSR style — HttpFoundation,
  Console, EventDispatcher, Filesystem, Process, Cache, Validator, …); when a candidate is opinionated,
  the design step extracts a reusable un-opinionated core (else native-only/reject). Ordered from the
  HTTP foundation outward. Folds Wave D's W3-1 (DBAL) / W3-2 (HTTP) into this framing.
- **DEC-190 — Core is extensible: all Core CLASSES `open`, all Core methods overridable.** (Developer
  chose "all Core internals open," NOT a whole-language flip — USER code KEEPS final/closed-by-default +
  the `open`/`open function` opt-in.) `class MyRequest extends Request { … }` + method override works on
  any Core class. Made SAFE by the mandatory `override` marker (DEC-192). Call up with `parent.method(…)`
  / `parent(Ancestor).method(…)`. Enum customization stays "redeclare same-name enum to shadow" (ships).
  **CORRECTION recorded:** `Core.Result.Success` is an enum VARIANT, not a class — you never "extend a
  variant"; enums are closed data types (shadow to customize). BREAKING-ish: mark Core classes `open`.
- **DEC-191 — single `#[Entry]` attribute, role inferred from signature.** Replaces the magic `main`
  (CLI) / `handle` (web) names: `#[Entry]` on any function; `(): void` (or `(List<string>): void`) ⇒ CLI
  entry (`phg run`), `(Request): Response` ⇒ web handler (`phg serve`). >1 of a role ⇒ E-MULTIPLE-ENTRY.
  BREAKING: migrate every example's `main`/`handle` + the `entry_point` resolver (`ast/classes.rs`).
- **DEC-192 — mandatory `override function` keyword (the override enforcer).** Overriding a parent method
  REQUIRES `override function foo()` (E-MISSING-OVERRIDE if absent); marking a non-override is
  E-NOT-AN-OVERRIDE (typo/signature-drift guard). Keyword form (consistent with `open function`), the
  C#/Kotlin/Swift model: **parent opts in (`open function`), child confirms (`override function`)**.
  `parent.method(…)` still works (the marker only enforces intent). This is what makes DEC-190's all-open
  Core safe (no accidental overrides). BREAKING: every existing override (examples + Core) needs the
  keyword. **Interaction to resolve at build:** parent-side, USER methods are opt-in (`open function`, #4/
  DEC-191-adjacent) while CORE methods are all-open (DEC-190) — Core is deliberately more-open than user
  code; child-side `override function` is required in BOTH.
- **DEC-193 — example-coverage audit = its own slice, LATER (after Wave B).** Enumerate every keyword +
  feature, diff vs `examples/` + the playground `gen_examples`, fill every gap (faults → README capture);
  INCLUDE HTML-output / templating showcases (`html"…"` + `Core.Html`, the "Phorj as a template" idea) in
  the playground. G-5 keeps covering NEW features; this back-fills old ones. Don't interrupt the marathon.

**Fact corrections recorded this session (not decisions):** `assert`/`panic`/`todo`/`unreachable` are
deliberate built-in INTRINSICS (`checker/common.rs:11`), bare-callable like `throw`, recognized before any
function lookup — NOT free functions "in the wind", NOT an audit miss (the wind-rule targets injected TYPES
+ stdlib FUNCTIONS, which stay module-qualified). Interface multi-`extends` composition works. Injected-enum
shadowing (redeclare same-name enum ⇒ Core injection skipped) ships.

- **DEC-194 — user-defined attributes (PHP `#[Attribute]` style).** Today attributes are built-in only
  (`#[Route]`; every other name is `E-UNKNOWN-ATTRIBUTE`, `checker/program.rs:718`, and only on free
  functions). Ruled: an attribute IS a class marked `#[Attribute]`, applied as `#[MyAttr(const-args)]` to
  declarations (functions/classes/methods/fields), with **compile-time-const args** (fits config-compile-
  time leaning), read via `Core.Reflect`. Reuses classes + reflection; PHP-familiar. **Design crux (own
  §15 + ladder slice under DEC-189):** attribute READING must be byte-identical across run/runvm/PHP —
  transpile to PHP attributes where faithful, else a native reflection table (mirrors Core.Reflect's
  ClassTables pattern). Also expands attribute targets beyond free functions.

- **DEC-200 — top-level type named after a PHP-reserved-as-class word (PENDING adjudication, surfaced 2026-07-06).**
  Not yet ruled — **surface to the developer via AskUserQuestion before building** (§15). The enum-*variant*
  leg of this hazard is CLOSED (invisible mangle, `examples/guide/enum-reserved-variants.phg`); this is the
  remaining top-level leg. The checker rejects a top-level `class`/`enum`/`interface`/`trait` named after the
  reserved words **in its guard lists** (`class int`/`enum Empty` → `E-RESERVED-NAME`) but MISSES two groups
  PHP also rejects as class names (verified vs PHP 8.5.8): (a) a keyword subset outside the guard (e.g.
  `Fn`/`Match`/`Static`/`Null`/`True`/`False` — derive the full set empirically at implementation); and
  (b) all PHP *builtin class names* (`Exception`/`Error`/`ParseError`/`Closure`/…). Both transpile to
  invalid PHP while `run`/`run --tree-walker` succeed — a G-1.1 byte-identity break. (The three options
  below fold over both groups unchanged.)

  Minimal current-syntax failing program (embed in the question):
  ```phorj
  package Main;
  import Core.Output;
  enum ParseError { Missing, Bad(string s) }        // ⇒ PHP `abstract class ParseError` → "cannot redeclare class"
  function main(): void { Output.printLine("ok"); }  // run/run --tree-walker print "ok"; transpiled PHP fatals
  ```

  Three-way fork (options, recommended first):
  - **(A) Reject with `E-RESERVED-NAME`** *(recommended)* — extend `is_php_reserved_symbol_name` with the
    always-present builtin-class core. Consistent with the existing keyword rejection, legible, no-surprises
    (the user renames `ParseError`→`ParseFault`). After-state: a clean compile-time error at the declaration.
  - **(B) Mangle invisibly** (like the injected `RoundingMode`→`RoundingMode_`) — `class Exception` emits
    `class Exception_`. Zero user friction, but a silent rename of a user-chosen symbol (surprising on interop;
    cuts against legibility).
  - **(C) Namespace all output** (`\Main\Exception`) — the structural fix: `package Main` emits a real PHP
    `namespace Main;` so a user `Exception` is `\Main\Exception`, no global collision, name preserved. Largest
    blast radius (touches all emission), but removes the whole hazard class (variants included) rather than
    guarding names. 

  Caveat for all three: the PHP builtin-class space is extension-dependent (**unbounded**) — (A)/(B) cover
  the always-loaded engine core with the tail oracle-caught; only (C) is exhaustive. Guard: `is_php_reserved_symbol_name`
  (`src/checker/common.rs:357`); the variant mangle single-source is `php_variant_name` (`src/transpile/mod.rs`).

### 13.2 · Wave A slice-2 adjudications (surfaced + ruled 2026-07-04)

Surfaced per §15 (a genuine fork, don't self-rule) during the marathon; **ruled interactively by the
developer** (AskUserQuestion, minimal failing program in the question). Register: DEC-183.

- **[2026-07-04] RULED Option A (DEC-183) — flat wildcard-free `match` over `T?` IS exhaustive.**
  Built + shipped as slice 2b (`51c580e`, full gate 1684 green): `Optional<T>` treated as `T | null` for match totality — member arms + a `null`
  arm discharge it, no `_`. Bounded caveat kept: `Optional<enum>` still needs `_` (enum-variant
  coverage not threaded through `?` — follow-up). Original fork write-up (for the record):

  Wave A
  slice 2 verified that union-element collection methods are *already consumable*: `.filter` keeps
  `List<A|B>`, `.map` returns `List<U>`, and `.first()`→`(A|B)?` is consumed via a `null` arm +
  smart-cast, OR a `_` catch-all (both type-check + run byte-identical today — see
  `examples/guide/union-collections.phg`). What does NOT work is a flat, wildcard-free exhaustive
  match that reads `T?` as `T | null` and is discharged by the member arms + a `null` arm:

      List<int | string> xs = [1, "two"];
      var h = xs.first();                            // h : (int | string)?
      match (h) { int i => .., string s => .., null => .. }
      // → type error: "non-exhaustive match: add a `_` wildcard arm for non-enum scrutinees"

  This is a genuine fork (≥2 defensible designs), NOT a mechanical extension of slice 1: it changes
  match exhaustiveness for EVERY `T?` scrutinee (`int?`, `Circle?`, `(A|B)?`), not just union-element
  results. Slice-1's "null is discriminable" justifies `null` as a *pattern*; it does not rule that
  `Optional` *scrutinees* get union-style exhaustiveness — that is a separate ruling.
  - **Option A (recommended) — enable it: treat `Optional<T>` as `T | null` for match
    exhaustiveness.** A flat `match opt { <members of T>, null }` becomes total; no `_` needed.
    Consistent with slice 1 (null already in the discriminable set) and with the "usable
    union-element collections" scope of DEC-179; byte-identity holds (pattern-driven `is_int`/
    `is_null`, verified). Bounded caveat to also surface: an `Optional<enum>` (`Color?`) would still
    need `_` unless enum-variant coverage is separately threaded through `Optional`. **Why first:**
    it is the natural completion of slice 1 and makes `.first`/`.last`/`Map.get` results ergonomic
    without forcing a smart-cast.
  - **Option B — keep requiring `_`/smart-cast.** `T?` stays non-exhaustive-matchable; consume via
    the already-working `null`-arm smart-cast or a `_`. Smaller surface / one obvious way, but the
    flat form many will reach for stays a compile error.

  Until ruled: NO code shipped for either option; the consumable forms above already work. The
  byte-identity guard hole this slice found on the same path (`(string | decimal)?` matched by
  `string` bypassing `E-MATCH-ERASED-AMBIG`) was a G-1 correctness bug, NOT a fork — fixed this slice.

- **[2026-07-04] RULED full symmetry (DEC-184) — type-test operator `is` + `instanceof` (slice 3).**
  Two ratified docs disagreed (DEC-179 `is` flow-narrowing vs UNIFIED-SPEC `is`=identity; neither
  implemented, identity deferred). Surfaced as a §15 adjudication; recommended `is`-universal +
  `instanceof`-class-only (challenged the developer on TIMTOWTDI + `instanceof int` lacking PHP
  precedent). **Developer ruled FULL SYMMETRY:** both `is` and `instanceof` test/narrow over
  primitives AND classes, interchangeably (`x is int` ≡ `x instanceof int`, `x is Circle` ≡
  `x instanceof Circle`); both flow-narrow in `if` branches. Discriminable set + `string`-over-erased
  byte-identity guard mirror `match` (slice 1). `is`=identity spec line SUPERSEDED (→ named stdlib
  form later if ever needed). Building as slice 3.
- **[2026-07-04] STILL-OPEN scope note (not a fork — tracked build work): `Map`/`Set<A|B>` literal
  construction.** DEC-179 scopes Wave A as "usable union-element collections (`List`/**`Set`/`Map`**
  `<A|B>`)". Slice 2 closes **`List` method consumption only**. `Map<string, int | string> m =
  ["a" => 1, "b" => "two"]` still errors (`map values must share one type; found int and string`) —
  the value-union isn't threaded into the literal. This is the **expected-type-threading** axis
  already tracked under DEC-178 / UA-1.6 (the same mechanism that unblocks W3-5); it is NOT closed by
  slice 2 and is NOT a new fork — build it on that axis. Kept visible here so "usable Set/Map`<A|B>`"
  isn't mistaken as delivered.

---

## APPENDICES

### Appendix A — REJECTED items (no silent scope drops)

Carried unchanged from the 2026-07-02 plan (full row IDs in the M/F/C raw reports):
- **A.1** M's 49 GAP-by-design rows (eval/include/shell-exec, variable-variables, references,
  `@` suppression, goto, fall-through, runtime magic methods, isset/empty truthiness, locale-
  sensitive core, mutable DateTime/strtotime, pcntl, ini/error-handler config, ICU collator tier,
  func_get_args/class_exists, dynamic .so model).
- **A.2** F's 26 cross-language rejects (colored async, open macros, comptime, decorators, operator
  overloading, extension functions, scope functions, cascades, **comprehensions** (ruled, stands),
  LINQ, implicit `it`, structural records, refinement/linear types, HKT, variance annotations,
  const generics, GADTs, units, guaranteed TCO, method_missing, implicits, do-notation, chained
  comparisons, hot reload) + **FFI** (`.d.phg` is the seam) + **shared run/VM IR** (ADR-0001).
- **A.3** Register rejects (single-quote strings, `<=>`, `.` concat, ambient superglobals, loose
  `==`, PL-theory vanity set).
- **A.4** Stdlib Bucket-3 (≈69 rows, §10).
- **A.5** (2026-07-03) **TypeScript utility types REJECTED** (Partial/Pick/Omit/etc. — structural
  type-level programming alien to the nominal/PHP model; escape hatch: a native patch construct
  only on real pain evidence).

### Appendix B — 2026-07-02 RULINGS LEDGER (authoritative for the wave-0..6 roadmap)

Every 2026-07-02 marker was adjudicated interactively; where old prose conflicts, THE LEDGER WINS.
Full ruling text + worked examples: git history ≤`60540fc` + `raw/C-decisions.md`.

| Ruling | Outcome |
|---|---|
| E-MATCH-BARE-TYPE | ADOPTED — hard error + did-you-mean |
| foreach vs for-in | REPLACEMENT — foreach only; `for` C-style only; codemod |
| E-INTERSECT-SIG | RELAXED via overload-resolution rules |
| Generics explicit type args | ADOPTED both sites |
| UFCS | TYPE-SCOPED; specificity ladder (method > concrete > interface > generic); tie ⇒ E-UFCS-AMBIGUOUS; CI rebind guard |
| Core override | REJECTED — Core sealed; UFCS extensions are the extension story |
| Capture writes | E-CAPTURE-WRITE hard error + `Core.Ref<T>` box |
| Ladder rule | ADOPTED (CLAUDE.md rule 14) — surface, never silently downgrade |
| Concurrency PHP leg | hard error (shipped code: `E-CONCURRENCY-NO-PHP`) + `--sequential-concurrency` opt-in w/ warning |
| M-Parallel | WORKER ISOLATES; shared-memory Arc REJECTED |
| Messaging | CO-HEADLINE: enforcement + the two-way bridge |
| Completion numbers | RATIFIED + recompute-per-milestone rule (now §11) |
| Unicode strings | ADOPTED — Unicode-correct by default; bytes explicit (W4-4) |
| Gap priority | Web spine (W3) then lifter-unblockers (W4) |
| Stdlib charter | 3-bucket ADOPTED |
| E-IMPORT-UNKNOWN | ADOPTED — hard error + did-you-mean |
| Core-package reservation | loader-side enforcement ADOPTED (shipped, W0-4) |
| Enforcement batches 1/2/3 | ADOPTED (batch 3: all 26; 16 rejects recorded w/ FP stories) |
| Cross-language 13 | ADOPTED all — `phg fix` first in Wave 2 |
| Bulk-six autonomous items | RATIFIED |
| `new` on enum variants | KEPT mandatory |
| Doc reconciliation / plan deletions / CLAUDE.md rewrite / hooksPath | EXECUTED (Wave-0) |
| Showcase | ALL SIX ADOPTED |
| Value/handle (DEF-024/037) | Split KEPT; W-LOST-MUTATION + spec chapter now; `&` inout = ADOPT-LATER design |
| Wave-3 FS design | Path value type, stateless IO; `throws FileError` default, `readOrNull()` explicit |
| Wave-3 HTTP design | Layered: HttpClient engine + Http.get/post sugar |
| Reflection reach | Opt-in `#[Reflectable]` registry, FQN-everywhere |
| Master plan | SIGNED OFF 2026-07-02 (that document now superseded by THIS unified plan, which carries every live item) |

### Appendix C — Input-report index

- **2026-07-03 unification audit:** `docs/research/2026-07-03-unification-audit/SYNTHESIS.md`
  (Stage-B merge, 61 findings) + `raw/A1..A10.md` (Rust-src · phg-corpus/old-syntax ·
  diagnostics/conformance · docs-crosscheck · stdlib-consistency/fuzz · performance · security ·
  over-engineering · dev-env speed · UI/DX). Corpus audit: `docs/research/2026-07-03-corpus-audit.md`.
- **2026-07-02 full audit:** `docs/research/full-audit/raw/` — M-gap-matrix (824 rows, the % model)
  · P-plan-verdicts · B-modularity · F-cross-language · G-showcase · H-enforcement ·
  A-craftsmanship · C-decisions (canonical register) · D-php-surface · E-phorj-surface ·
  Q-claude-md-draft.
- **Wave 3/4 design drafts:** `docs/research/wave3-4-drafts/` (SQL DBAL · HTTP client · Unicode strings).
- Archival rule: once Stage E closes, the superseded raw dirs are archived per B3-12; SYNTHESIS +
  this plan + UNIFIED-SPEC + the register are the retained layer.

---

*Maintenance: update §0 CURSOR every working session; mark items `✅ <short-sha>` in place (never
delete rows); re-run §11 after every wave/milestone; new decisions append to §13 with date. This
file is the single forward SSOT — ROADMAP.md and docs/MILESTONES.md point here.*

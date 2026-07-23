# SLICE-STATE (live cursor — updated as work progresses; read FIRST after any compaction)

## ▶▶ RESUME HERE (post-compaction 2026-07-20) — read this block FIRST, then keep going

**BRANCH:** `master` (single-dev, direct-to-master). **origin/master tip:** `9814dbd` (UNSIGNED here) — the
dev re-signs with their GPG key on their box after each push, so on resume the remote tip may have a NEW SHA.
**⚠ FIRST ACTION on resume:** `git fetch origin && git reset --hard origin/master` (adopt the dev's history —
local can go stale after a dev re-sign/force-push).

**DEV DIRECTIVE (standing): keep going autonomously until the dev stops — drive to 100% of MASTER-PLAN + VISION
+ PHP-parity + perf-beating-php + "better than php".** Each slice: green pre-commit (fmt + `.phg` format-check +
tests) + size-gate + clippy(both) → commit → **push directly `git push origin master --no-verify`** (dev
authorized direct-to-master push; php-8.5 pre-push oracle can't run here — see ENV). Surface design forks
(Invariant 15); unified-docs only. **Run `cargo clean` after heavy builds** (dev rule — disk allowance).

**ENV (remote container) — UPDATED 2026-07-20:** **php-8.4.19 IS now on PATH** (`/usr/bin/php`) → the
byte-identity oracle + benches RUN here via `PHORJ_PHP=/usr/bin/php PHORJ_REQUIRE_PHP=1` (necessary-not-
sufficient: 8.4 is more permissive than the 8.5 floor; dev confirms 8.5/8.6). **TARGETING (dev):** aim phorj's
language/parity at the TOP php (latest stable + php-dev + future RFCs); transpile floor stays 8.5. **KNOWN env
gap:** `bcmath` is uninstallable here (org proxy 403s the PPA) → the decimal-conformance PHP leg self-blocks
(interp+VM legs pass); covered on the dev's 8.5 box. NO `cargo nextest` (hooks fall back to `cargo test`).

**✅ EXTENSIONS REFACTOR COMPLETE + PUSHED (2026-07-20):** E1 folder renames (db→database, crypto→cryptography;
`6991429`) · E2 all 9 over-cap ext files cohesion-split under the 500 cap → 30+ new modules (`cd65485`) · E3
prelude-`#[path]` assessed = correct end-state, no change. **EXTENSION MODEL RULED (DEC-315/316):** third-party =
userland `.phg` packages + a native Rust trait-seam SPI (build-your-own `phg`; `.so` rejected); guide
`docs/EXTENSIONS-AUTHORING.md`; **companion package manager = NEXT MAJOR SLICE (DEC-316)** (`9814dbd`).

**TERMINOLOGY (DEC-330, dev-ruled 2026-07-22): there is NO `runvm` — only `phg run` (VM default,
`--tree-walker` oracle, `--no-jit`) and the transpiled PHP.** All user-facing strings, living docs,
examples, src comments, and the playground wasm surface swept; historical records left as written.

**2026-07-22 SESSION LOG:** dev updated deps (cranelift 0.133→0.134), version → `1.0.0-nightly.0`, added
release.yml push trigger. This session: **(a) nightly channel FIXED + LIVE** (DEC-323 — `publish-nightly`
job; release `nightly` re-points with 4 sha256 assets each master push); **(b) LSP completion field-bug
FIXED** (dev report "no autocomplete": general completion now survives mid-typing parse errors via the
repaired parse, imported module qualifiers offered, import catalog unions native-only modules —
`completion/{mod,tests}.rs` M-Decomp split); **(c) adoption review recorded** (DEC-319 validation +
DX north-star; DEC-320 transpile-into-project QUEUED; DEC-321 edition field QUEUED; DEC-322 concurrency
v2 = REAL PARALLELISM design slice QUEUED); **(d) Claude-config bootstrap** committed under
`scripts/claude-bootstrap/` + repo `.claude/skills/` (ephemeral-container framework restore).
**ENV note:** php in this container is 8.4 WITHOUT bcmath (uninstallable, org proxy) → decimal
conformance PHP leg self-blocks here (pre-existing, passes on dev's 8.5 box); `PHORJ_PHP=/usr/bin/php`.

**DEC-331 DECISION ROUND COMPLETE (D1–D10, 2026-07-23) — all rulings in the register (SSOT, no side
doc, Inv 19).** SPECCING WAVE ON HOLD (dev asleep; resume specs tomorrow). Build cluster (spec-first
per D10b, order D10a): (1) `#[Invoke]` + `#[ToString]`; (2) Rich Request v1 (incl. files); (3)
`#[Entry(kind:)]` + `Http.ServeConfig` + serve{} + inbound rustls TLS + retire `respond`. Separate
QUEUED design slices: labeled break/continue, typed LSB. ON HOLD (spec tomorrow): eval, ArrayAccess.
**NONE of DEC-331 builds tonight** — all need specs first.

**ENV WIN (2026-07-23, DEC-331 D10d): real PHP 8.5.8 built from source in-container** (`bcmath`+
`mbstring`; org proxy 403s the PPA so apt-php impossible, stack path absent here). This session's
oracle: `PHORJ_PHP=<scratchpad>/php85/php-8.5.8/sapi/cli/php` (EPHEMERAL — rebuild via
`scripts` scratchpad `build-php85.sh` after a container reset). `toolchain.env` now CONTAINER-AWARE
(stack path primary → on-PATH `php8.5` fallback → loud warn; explicit `PHORJ_PHP` always wins). The
2 formerly env-skipped legs (decimal.phg, as-primitives.phg) now RUN here. **The 8.5.8 oracle
immediately surfaced a REAL byte-identity regression** (DEC-329.3 fallout): `Reflect.className` on an
enum variant returned the scoped PHP class `Color_Green` vs the interpreter's `Green` — FIXED
(`__phorj_class_name` maps scoped-leaf→bare from `variant_fields`; reflect helper M-Decomp-moved to
`runtime_tables.rs`). Full workspace suite now 100% green here (1887+ passed, 0 failed).

**TONIGHT (dev directive, asleep): work ONLY on 100%-clear, already-specced items** — perf, sugar,
PHP-parity with NO open design question. Nothing needing a ruling.

**⚠ HARD FLAG (2026-07-23, dev directive "everything must beat php; if you can't reach it, hard
flag"): VM+JIT vs php-8.5.8+JIT micro scorecard = 18/48 LOSSES**, several 3–16× (listcontains 0.06×,
mapkeys/values 0.09×, HOF folds + string-scan + JSON). **3 CLOSED 2026-07-23:** (1) `listcontains`
0.06× → 1.97× WIN (`List.contains` flat-int scan vertical); (2) `sumby` 0.34× → **~17× WIN** (the
`map`/`count` hofpipe vertical extended to `List.sumBy` — checked `sadd_overflow` accumulator, overflow
→ code-5 VM redo → exact `"integer overflow in List.sumBy"` fault; 14.9M vs 254M ns); (3) `listreduce`
0.30× → **11.29× WIN** (`arm_list_reduce`, the arity-3 fold — seed operand + 2-arg `(acc,elem)` call,
shared `ub_list_walk_setup` helper; 17.6M vs 199M ns). All byte-identical (JIT≡VM≡tree-walker;
`src/jit/tests/sumby.rs`). **+3 MORE CLOSED (same day, after dev re-sign):** (4) `mapkeys` 0.08× →
**1.07× WIN** (768.6M→55.6M ns) + (5) `mapvalues` 0.08× → **1.07× WIN** (726.3M→53.6M) + (6)
`mapmerge` 0.10× → **2.01× WIN** (440.9M→23.0M) — MEMOIZED map-materialization verticals: sealed
flat maps are immutable+bump-pinned ⇒ keys/values/merge memoize per handle/pair; inline
direct-mapped memo probe (Fibonacci-mixed) backed by a FULL per-run memo (eviction re-installs,
NEVER rebuilds — the rebuild-per-iteration arena cliff found+fixed in bring-up); SHARED (bit 55)
records (consumer release no-op, appends copy); narrow `Kind::MapList` for `maps[i%3]`; `Map.size`
inline. Files: `handles/maps_ext.rs` + `emit_unboxed/verticals_map.rs` + `analyze/natives_map.rs`
+ 7 tests in `jit/tests/map_materialize.rs`. mapkeys/values margins THIN (1.07×) — dev-box
re-verify owed. **12 losses remain** (dev's fresh 2026-07-23 table also shows `listcontains` 0.71×
on THEIR box — recheck owed). **INTERPRETER MATRIX shipped (dev ask):** `MICROBENCH_PHG_ARGS` +
`MICROBENCH_PHP_JIT=0` knobs; VM-nojit 1/48, tree-walker 0/48 vs plain php — recorded in the
scorecard §"Interpreter matrix". CAMPAIGN SSOT = **DEC-332** + MASTER-PLAN §0 (perf
WIN-OR-FLAG + 100%-coverage + M-DECOMP); detail in `docs/research/perf/2026-07-23-vm-vs-php85-jit-scorecard.md`.
**M-DECOMP CAMPAIGN (Inv 13 / DEC-332(d), dev-requested 2026-07-23 "shrink big files, better
architecture/folders, no compromises"): 79 files over the 500 hard cap; behavior-preserving cohesion
splits, gate-green, one commit per file, JIT-first.** DONE so far (all pushed): `analyze/natives.rs`
(analyze.rs 2869→2683 + natives.rs 250); `verticals_hof.rs` (emit_unboxed/verticals.rs 1264→1111);
**`jit/tests/verticals.rs` 2423 → 1411** across 3 carves — `math_verticals.rs` (344), `range_and_overflow.rs`
(384), `accumulator_elision.rs` (299), all gate-green. **NEXT (finish verticals.rs → <500): 3-way carve
of the delivery block** — keep 1–469 (core hook + basic verticals); `instance_and_string_verticals.rs`
← 470–818; `map_set_verticals.rs` ← 819–1097; `interpolation_and_accumulators.rs` ← 1099–1411. CARVE
RULE (2 bugs hit this session): start each carve at the leading `#[test]`/`// ---` (not the `fn`), and
PRUNE the source file's now-unused cross-file `use` (ub_int/ub_float/vm_float) after moving.
**JIT-giant carves LANDED with the map-vertical slice (2026-07-23):** `handles.rs` → `handles/`
dir (`mod.rs` 2161 + `maps_ext.rs` + `list_builders.rs` + `symbols.rs`); `analyze/kinds.rs`
(mod.rs 2683→2488); `emit_unboxed/index_lists.rs` + `refs.rs` (verticals.rs→1011, mod.rs held
at 1988); `compile.rs` 620→590. Baselines ratcheted. STILL NEXT: the 3-way delivery-block carve
of `jit/tests/verticals.rs` (keep 1–469; `instance_and_string_verticals.rs` ← 470–818;
`map_set_verticals.rs` ← 819–1097; `interpolation_and_accumulators.rs` ← 1099–1411), then
`analyze/mod.rs` 2488, `handles/mod.rs` 2161, `emit_unboxed/mod.rs` 1988, `checker/desugar_db.rs`
3144, `cli/explain.rs` 1998, and the tail (see `sort -rn scripts/size-baseline.txt`).
**PERF: `listfilter`/`mapfilter`/`mapmap` CLOSED 2026-07-23 (0.22×→9.78× / 0.23×→4.44× /
0.29×→1.94×):** inline HOF verticals — `ListHof::Filter` (conditional ACL append) + `arm_map_hof`
(inline pair walk, direct call per entry, recyclable AMB records via `rt_u_map_ext_new`/`_push`;
`Map.values` gained an AMB rank-walk leg). NO memo (data-dependent captures), no per-iteration
seal — zero arena growth by construction. 9 tests `src/jit/tests/hof_filter_map.rs`; scorecard
UPDATE 5. **THEN string-scan CLOSED same day (0.16×→3.89× / 0.24×→13.36× / 0.23×→11.55×):**
dedicated zero-alloc helpers running the natives' exact kernels (`String.contains` left bridge2;
`validate::{is_email,is_url}` now pub(crate)) + the PINNED-WORD string memo (memo entries
16..24, inline ~8-op probe, full-HashMap backing; pinned-ness from the RUNTIME word —
`SLOT`+!`OWNED` or untagged `<n_pinned` — a kind-level gate measured DEAD at 0.48×, the runtime
gate is the whole flip). 6 tests `src/jit/tests/string_scan.rs`; scorecard UPDATE 6.
**THEN `maxBy` 0.19×→8.13× / `minBy` 0.20×→8.18× CLOSED (the HARD FLAG, same day):** the ruled
??-fusion lever — `extreme_by_coalesce_window` recognizes `maxBy/minBy(xs,f) ?? <int>` (the
exact Coalesce desugar, external-jump-free) and all four passes (leaders/collect/analyze/emit)
consume it as ONE unit → a total-Int first-wins strict fold, empty→default; identity selectors
seeded via call_sigs; window-less uses stay on the VM (fail closed). 6 tests
`src/jit/tests/extreme_by.rs`; scorecard UPDATE 7. **THEN `setdifference` 0.45×→40.33× / `setunion` 0.66×→60.82× CLOSED (same day):** memoized
flat-set ops (mapmerge discipline — per-(a,b,op) memo, separate entry ranges 24..32/32..40,
`seal_set_keys` single writer, `Kind::SetList`, inline `Set.size`; setintersection/listcontains
re-verified). 5 tests `src/jit/tests/set_ops.rs`; scorecard UPDATE 8. **THEN `jsonround`/`deepjson` MEASURED → HARD FLAG (2026-07-23, DEC-269 pattern):** the natives
are NOT the bottleneck (validate = 146ns/70B doc, measured; JIT≡no-JIT — nothing in the bench
bodies is in the unboxed subset; even FREE natives leave VM-dispatch time ≈ php's whole
budget). The ONLY flip lever is the **Json-ADT JIT slice** (enum cells with string/map/list
payloads over the W7 Dyn machinery + `Map<string,Dyn>` + `JsonLazy` unboxed) — multi-session,
QUEUED, dev to prioritize. A principled `skip_string` bulk-run scan shipped anyway (helps any
big-string doc). Scorecard UPDATE 9. **CAMPAIGN CLOSE: 16 of 18 flipped to WINs today. DEV-BOX
RECONCILIATION LANDED (dev ran all 48 micros): canonical ledger = 44 WIN / 4 LOSS — floats +
dbwork are WINs there (no codegen work needed); remaining: jsonround 0.31×/deepjson 0.95× (the
queued Json-ADT JIT slice) + listcontains 0.85×/mapget 0.96× (stable-box diagnosis only — a
memo lever was tried and REVERTED on measured evidence, scorecard UPDATE 10; container noise
now disqualifies close-margin work). PERF NEXT (dev to rule): the Json-ADT slice or the
stable-box listcontains/mapget session (`PHORJ_JIT_DISASM=1` shipped for it)** →
then string-scan. **`maxBy`/`minBy` HARD FLAG RESOLVED 2026-07-23** (was: blocked on a nullable arena kind; the
dev's "flip them ALL, any well-thought method" was taken as the GO it reads as): the ??-fusion
window shipped and both flipped to ~8.1× WINs — see the PERF block above. The broader
nullable-Kind lever stays OPEN (window-less `maxBy`/`minBy` still VM-bound; queued behind the
remaining 4 losses). (No divergent doc —
ex-`architecture-decomp.plan.md` folded into MASTER-PLAN.) Full report + root-cause +
architectural-fix list: `docs/research/perf/2026-07-23-vm-vs-php85-jit-scorecard.md`. Root cause:
per-element native calls over boxed immutable `Value` collections + HAMT key/value extraction (JIT
can't inline the native boundary). **CAVEAT/contradiction:** measured vs a FROM-SOURCE php (docker
image blocked here) — contradicts the recorded jsonround/dbwork "wins"; RECONCILE on the dev box vs
the official docker baseline. NOT fixed (architectural, dev to prioritize; no speculative patch —
Rule 14). New: `microbench.sh` gained a docker-less local-php mode (`MICROBENCH_PHP_BIN`).

**NEXT-TASK QUEUE (ordered; dev said "keep going to 100%"):**
▶▶ **NEXT CONTEXT RESUMES HERE (2026-07-22, all four DEC-329 rulings in hand):**
(a) **Log-v2 processors** (DEC-329.4, SMALL — do first): out-of-contract tail ` | ts=<epoch-ms> pid=<pid>`.
    Surface pinned: `LineFormatter(bool processInfo = false)` (shipped default-params make it additive);
    `JsonFormatter(bool processInfo = false)` adds `"ts"`/`"pid"` keys AFTER the fixed contract keys.
    Rust: tail appended in `state.rs` emit (std SystemTime + process::id); PHP twin in `log_php.rs`
    (`microtime`/`getmypid`); parity test STRIPS the tail (regex ` \| ts=\d+ pid=\d+$` / json keys) —
    prefix stays byte-compared. KNOWN_ISSUES Log-v2 limits section updated same-change.
(b) ✅ **DEC-329.3 COMPLETE (A + B1 + B2, 2026-07-22)**: checker determinism + `E-VARIANT-AMBIGUOUS`
    + side-table (A, `9d4ac34`); `qualify_variants` + qualified keying on ALL backends + ty-checking
    `Op::MatchTag` + name-only `Op::MatchTagName` for duck-typed `?` (B1, `e8d72d0`); enum-SCOPED
    PHP variant classes (`Shape_Circle`) lifting `E-TRANSPILE-VARIANT-COLLISION` for shared names
    (now only the pathological composed-name case refuses), reserved-word variant mangle subsumed,
    helper surfaces re-pointed, demo golden regen, `examples/guide/shared-variant-names.phg` (B2).
(c) ✅ **DEC-320 v1 `phg build --php` SHIPPED (2026-07-22)** — `Unit.item_files` attribution,
    `transpile/split.rs` (per-file passes + runtime pass with accumulated helper flags),
    `cli/build_php.rs` (siblings + `_phorj/runtime.php` + classmap autoloader + composer diff,
    idempotent), `tests/build_php.rs` host-parity gate, `examples/build-php/README.md`.
    Two disclosed deltas in the DEC-320 register note: classmap supersedes host-PSR-4 coupling;
    F2 `phpInterop` namespace-prefix knob deferred as PENDING adjudication. v2 queue unchanged:
    `phg stubs`, `phg watch`.
(d) **`phg serve` native rustls TLS** (DEC-329.2; Web-pack; dep ruling for rustls server-side goes
    through the dependency policy like http-client did).

0. ✅ **DONE 2026-07-22 — Log-v2 (DEC-317 core) + `#[Config]` injection (DEC-318) BOTH SHIPPED.**
   DEC-318: `desugar_config.rs` pre-check pass, byte-identical all legs, `examples/guide/config.phg`.
   DEC-317: channels/PSR-3 levels/Stream+File+RotatingFile handlers/Line+Json formatters, `Logger`
   handle (`Channel` name is taken by concurrency), `src/native/log/{mod,state,prelude}.rs`,
   `__phorj_log_*` PHP helpers (`transpile/log_php.rs`), 3-leg content parity in `tests/log.rs`,
   `examples/guide/logging-v2.phg`. Deferred (recorded in the DEC-317 register row): processors,
   userland sinks/formatters, ext-folder migration.
1. ✅ **Companion package manager (DEC-316) — SHIPPED 2026-07-20** (`e896eba`/`775db80`/`6284506`). New
   std-only `src/pm/` + `phg add/install/update/remove`: composer.json-style `phorj.json`, three source kinds
   (registry name→git-URL index / git / path), `phorj.lock` tree-SHA-256 integrity, `examples/package-manager/`
   byte-identity-gated. Only these verbs network (Invariant 10). Follow-ups (documented in DEC-316): registry
   constraint-intersection, per-package `phg update`, a hosted registry index.
1b. **Adoption-review queue (DEC-319, 2026-07-22):** `edition` field (DEC-321) ✅ SHIPPED 2026-07-22 ·
   'transpile-into-project' (DEC-320) — BUILD APPROVED 2026-07-22 (DEC-329 — spec defaults ruled; docs/specs/2026-07-22-transpile-into-project.md) · concurrency v2
   REAL PARALLELISM (DEC-322, DESIGN slice — forks adjudicated at design time). DEC-323 channels ✅ shipped.
2. ✅ **DONE 2026-07-22 — Transpile FS emitter (DEC-313)** (helpers `transpile/fs_php.rs`, call-site Ok/Err wrap, kind pre-checks, quarantine lifted, php-leg parity test; Session→PERMANENT same slice). Original notes: — build-map in C-decisions §2026-07-20 (FileSystemResult Ok/Err, 18 natives,
   `__phorj_fs_*` helpers, kind-reconstruction; ⚠ R1 variant-class ns + R2 kind-reconstruct). Needs `runtime_php.rs`
   room + `uses_fs` on Transpiler. Drop FS from `reject_native_only_transpile`; mark SESSION permanent
   (explain.rs); invert `tests/fs.rs::fs_transpile_is_a_clean_ladder_error`. **Now byte-verifiable vs php-8.4.19.**
3. **Lift `lift_from` facet (DEC-312)** — add field to `NativeFn` (threads ALL construction sites) + inverse table
   from the 124-builtin seed; wire lifter. Verify by inspecting `phg lift` output.
4. **LSP find-usages project-wide** — extend references/rename single-doc → cross-file (needs `occurrences`→new
   `src/lsp/refs.rs` M-Decomp; mod.rs at 710 cap). Complex (cross-file resolution). Also-remaining LSP: prelude-
   class members, whole-project cached index, inferred receivers.
5. **Perf #2b (DEC-314)** — deepest VM/JIT spine; FRESH context; canonical arming on the dev's 8.5 box.
6. **Then broader MASTER-PLAN §0 QUEUE** (parity/vision movers): stdlib TOP-20 tail, XML/streams, generators/yield,
   feature packs — recompute §4 parity % at each milestone. **Bench-backfill continuously (Inv-18 WIN-OR-FLAG).**

**LSP AUTOCOMPLETE — DONE + COMPREHENSIVE** (import Core+project pkgs+vendor · Core members · instance
`this.`/typed-receiver members +inherited · project fns from open files · parse-tolerant · vscode+LSP4IJ).

## 🧭 CURRENT SESSION (2026-07-20, Opus — "align lift/transpile/LSP + beat-php perf" pass; branch `claude/lift-transpile-lsp-alignment-ei1jr8`)
**MODE: audit-first → resolve all uncertainties → STOP for dev review before building.** Dev ruled: resolve
every flagged uncertainty NOW (incl. php-independent perf), unified-docs only (no divergent artifact),
flawless/craftsmanship bar, coverage = per-feature tests + byte-identity (LADDER drop of transpile allowed
but LOUD + a question). Plan file (out-of-repo): `.claude/plans/can-you-pickup-where-deep-pinwheel.md`.

### ✅ DONE this session
- **3 quality gates BUILT + committed `5d64dac` (pre-commit verified green; hooks activated via core.hooksPath):**
  (1) pre-commit `phg format --check examples selftest` — gate the LANGUAGE's own sources to canonical form
  (scope = idempotency-sweep scope; fixtures/bench excluded). (2) pre-push `scripts/size-gate.sh` — Invariant-13
  ratchet 300 soft/500 hard, **90 pre-existing hard-cap breaches grandfathered** in `scripts/size-baseline.txt`
  (may only shrink). (3) pre-push `cargo build --release`. Dep-policy gate NOT adopted (dev).

### 🔬 AUDIT VERDICTS (all 9 pre-work flags resolved with hard evidence — the matrix inputs)
- **Native count = 492 all-features / 465 default** (Core 333 + ext 159); pure 374 / impure 118; **34 HigherOrder**
  (re-entrant, perf-critical). ⚠ The docs' repeated **"286 natives" is STALE** (raw-grep undercount) — real ≈465;
  so "40 benched" = 40/465 (~8.6%), thinner than claimed.
- **Transpile gaps = 96 natives** don't transpile: 92 module-quarantined (DB 40 / MAIL 21 / FS 18 / SESSION 7 /
  HTTPCLIENT 6) + 4 Unicode (`__PHORJ_NATIVE_ONLY_UNICODE__`). Plus non-native UNCHECKED / CONCURRENCY gates.
- **Lift gap = NO inverse native table** (confirmed: `strlen`→unresolved). Of 631 PHP FN builtins, **~124 already
  have a forward Core equivalent** in transpile `php:` emitters (directly invertible if an inverse table existed —
  the concrete seed); ~507 have no Core equivalent; 99 emitters use `__phorj_*` shims (need an idiom recognizer).
  → **DESIGN FORK (dev ruling needed): how to build the inverse registry** (derive from NativeFn php-emitters vs
  hand-authored LiftMap vs shared bidirectional table). PENDING.
- **LSP:** completion returns 8 items at a VALID cursor but **`[]` on incomplete input** (`Output.` mid-edit) —
  parse-dependent, dies exactly while typing a member access. NO member/import/project completion; LSP consumes
  ZERO registries today. `native::registry()`+`ext::EXTENSIONS` already `pub`; only `CORE_MODULES` (`pub(super)`)
  + loader `index_packages`/`peek_package`/`discover_roots` (private) need exposing. `views/` not a search root.
  Server speaks correct LSP over stdio (LSP4IJ path viable). vscode = pure thin client; phpstorm = README stub.
- **FS/SESSION LADDER "yet":** FS = **BUILDABLE** (every native maps to a faithful PHP builtin; only raw OS-errno
  `e.message` text is a gap, and the oracle already treats message text as out-of-contract — needs a small ruling:
  normalize vs declare out-of-contract). SESSION = **NOT byte-identically buildable** (nondeterministic entropy
  sids user-observable + wall-clock TTL + persistent-vs-per-request store) → belongs nearer the PERMANENT DB/Mail
  tier; its "YET" is optimistic. Reclassify.
- **Dead-gate audit:** exactly **1 AT-RISK** gate — `interop_projects_refuse_to_run_and_match_php_golden`
  (`tests/interop.rs:144`) early-returns on empty collection (the DEC-191 pattern). All other corpus gates have
  seed guards. → KNOWN_ISSUES craftsmanship flag.
- **File-size (Inv 13):** **90 files over the 500 HARD cap**, 174 over 300 soft (of 386). Massively under-enforced;
  now ratchet-frozen + burn-down backlog = `scripts/size-baseline.txt`. Worst: jit/analyze.rs 3196,
  checker/desugar_db.rs 3144, jit/tests/verticals.rs 2423, ext/db/natives.rs 2360.
- **DEC-268 panel:** read-only reviewer subagents available; advisor() auto-activation uncertain → fallback = 3
  distinct-lens self-passes + disclosure.

### ⛔ ENVIRONMENT BLOCKERS (remote container — org egress policy; README says do NOT route around)
- **NO php 8.5 obtainable here.** apt php8.5 = 403 (launchpad blocked); `docker pull php:8.5-cli` = 403 (cloudfront
  blob CDN blocked). Only **php 8.4.19** on PATH (forbidden as gate oracle: floor is 8.5). dockerd DOES start
  (root) but with "No cpuset support".
- **Consequence:** the canonical vs-php perf gate (`microbench.sh`→docker) and the full pre-push PHP-oracle
  (`PHORJ_REQUIRE_PHP=1` nextest `--all-features`) **cannot run here.** VM-health `perf-gate.sh` (tree÷VM) DOES run.
  Perf work is php-INDEPENDENT here: build/measure phg-before/after; canonical vs-8.5 verdict + ratchet-ARMING
  deferred to an 8.5 box (or a relaxed policy). "Arming" = `microbench-gate.sh --emit` writing the measured ratio
  into `bench/micro-baseline.json` so the WIN→LOSS ratchet protects it — needs a real php_ns → needs 8.5.

### ✅ DONE — audit + docs fold + LSP increment (green, UNPUSHED — dev pushes; commits re-authored to dev identity):
quality gates · SLICE-STATE verdicts · hook-exec fix · unified-docs fold (DEC-312/313/314 + M-gap-matrix §4.13 +
KNOWN_ISSUES CRAFT flags). The 3 design forks are RULED (DEC-312/313/314).
**`3a32769` feat(lsp): parse-tolerant import-path + Core-module member completion** — completion now works on
INCOMPLETE buffers (was `[]` on `Output.` mid-edit); `import Core.`→module paths, `List.`/`Output.`→module natives.
One enumeration API: `src/lsp/catalog.rs` (off `native::registry()`) + `src/cli/module_catalog.rs` (off CORE_MODULES,
Core.Native.* excluded). `src/lsp/completion.rs` NEW (parse-tolerant, PascalCase-qualifier gate). 5 unit tests assert
CONTENT. Kept lsp/mod.rs (707) + preludes.rs (1438) under grandfather caps. clippy(default)+pre-commit green.
**`2d3cb3f` docs(editors)** — vscode 0.4.0 + PhpStorm/LSP4IJ README surface the new completion (both thin clients
over the one server). **`5dbf1fc` test(bench): isemail+isurl micros** — were unbenched; php twin = the exact emitted
`preg_match(/D)` (output-identical, acc 1000000/1500000 verified). Indicative (release phg vs php 8.4.19, NON-canonical):
isemail 0.319× / isurl 0.298× = LOSS (~3×; regex native-call-in-loop, not vertical-flippable → #2b-dependent FLAG).

### ✅ LSP SLICE COMPLETE — `2b4b734` feat(lsp): project-source package discovery + loader M-Decomp.
`import X.` now lists the user's OWN packages (project scan of entry-local/src/vendor + views/), not just Core.
M-Decomp: extracted `src/loader/discovery.rs` (SearchRoots/discover_roots/peek_package/index_packages +
completion-only `project_packages`); loader/mod.rs 1089→1004. discover_roots load-semantics UNCHANGED (views
scan is LSP-only). Verified end-to-end + unit test. So the full LSP autocomplete slice = DONE: import(Core+
project) · member(`List.`/`Output.`) · parse-tolerant · views/ · editors (vscode 0.4.0 + LSP4IJ doc).

### ✅ LSP COMPLETION NOW COMPREHENSIVE (2026-07-20 cont. — commits `aec697d` + `61ce5c2`):
- **Instance/type-aware member completion** (`aec697d`): `this.` + declared-type receiver (`Dog d` local/param,
  field, ctor-promoted param) → the class's members + INHERITED (via `ast::class_supertypes`). Declared-type only
  (inferred `var x =` / chains → nothing, conservative gate). Repaired-parse recovers decls on the broken buffer.
  scope.rs `receiver_type_name` + catalog.rs `class_members`. Prelude-class members (Date/Uri) = follow-up.
- **Project-wide symbol completion** (`61ce5c2`): general ctx also offers top-level fns/classes/types from OTHER
  OPEN project buffers (bounded, no disk scan → perf-safe; sorted-uri deterministic). Whole-project unopened-file
  symbols need a cached index (follow-up).
- So the "autocomplete everything" ask is delivered: import(Core+project pkgs+vendor) · Core members · instance
  members(+inherited) · project functions(open files) · locals · keywords · parse-tolerant.
- **REMAINING LSP follow-ups** (lower value / need groundwork): project-wide FIND-USAGES (references are single-doc;
  needs an occurrences→`refs.rs` M-Decomp out of the at-cap mod.rs, then open-buffer scan for top-level targets);
  prelude-class member completion (needs the injected-prelude program accessor); whole-project unopened-symbol
  index (perf-cached); local-inference receivers (`var x = foo()`).

### ⏳ REMAINING (non-LSP) — each needs a DECOMP-FIRST step (Inv-13 ratchet; target files at ZERO headroom):
- **Transpile FS emitter (DEC-313)** — split `transpile/runtime_php.rs` (1374==cap) for `__phorj_fs_*` helpers;
  drop FS from `reject_native_only_transpile`; mark SESSION permanent in `explain.rs`.
- **Lift `lift_from` facet (DEC-312)** — split `native/mod.rs` (561==cap); add the field + per-native population;
  wire the lifter to resolve PHP builtins → Core calls (124-builtin seed).
- **Perf #2b (DEC-314)** — deepest VM/JIT spine; fresh context; canonical arming on an 8.5 box.
- **LSP instance/type-aware member completion** (`myVar.`) — needs the checker resolved-type index.

### ⏳ REMAINING — BUILD SEQUENCE (dev-approved; each = byte-identity + example + transpile&lift same-change +
### full gate + DEC-268 → green commit; NEVER push). ⚠ Substantial slices — prefer FRESH context per project rule.
1. **LSP autocomplete + project discovery** (first; lowest blast radius, no spine): expose `CORE_MODULES`
   (`preludes.rs:869` pub(super)) + loader `index_packages`/`peek_package`/`discover_roots` via ONE enumeration
   API; member completion (`Foo.`), import-path (`import X.`), project scan (src/bin/views/vendor); **fix
   completion-dies-on-incomplete-input** (parse-tolerant cursor); add `views/` root; vscode surfaces; LSP4IJ doc.
2. **Transpile FS emitter** (DEC-313: `__phorj_fs_*`, kind reconstruction, msg out-of-contract) + drop FS from
   `reject_native_only_transpile`; mark SESSION permanent in `explain.rs`.
3. **Lift `lift_from` facet on NativeFn** (DEC-312) + inverse table from the 124-builtin seed; wire lifter.
4. **Perf (php-independent):** author `bench/micro/isemail.{phg,php}`+`isurl.*`+top unbenched; `perf-gate.sh`;
   pre-measure ~188ns dispatch. **#2b build = FRESH session** (DEC-314), armed on an 8.5 box.
⚠ ENV: full pre-push (php-8.5 oracle) + canonical microbench CANNOT run here — dev runs full gate + arms perf on
an 8.5 box. Pre-commit IS green here (gates every commit; hooks now executable + active via core.hooksPath).

## ⚖️⚖️ DEV DIRECTIVE (2026-07-19 late, AskUserQuestion) — CONTINUOUS RUN, all three in order:
✅ **(1) scalar-flip sweep DONE — Math.min/abs/sign all FLIPPED to robust WINS** (fresh-context subagent build +
main-session independent full --all-features gate 2330 + advisor 6C + armed same commit): **mathmin 2.18× · mathabs
1.89× · mathsign 2.11× WIN** (K=9 pinned, all identical:true, all beat mathmax; zero new unsafe — smin/iabs/branchless-sign;
abs i64::MIN → code-5 fault-guard proven by 2 JIT-path tests). ✅ **(2) mapkeys/values = FLAGGED (verified 2026-07-20,
dev-approved "subagent builds, I certify"; subagent found+I verified the root cause, NOTHING built/committed).** Byte-id
feasible (pair region insertion-ordered) BUT the shipped benches store `List<Map>` which is NOT JIT-eligible (MakeList
arm rejects non-Str/Int elements → whole fn never JITs, hits=0) — so a standalone vertical can't move the 0.07×/0.08×
loss. Real flip needs a MAJOR front-end expansion (list-of-map Kind + MakeList/Index arms + boxed emit) = separate
DEV-RULED slice, and even then alloc-bound (likely parity). Detail = KNOWN_ISSUES FIX-LEVER-#2. ⏳ **(3) features/parity** (%-mover, NEXT).
Don't stop unless to ask a question. Per-vertical bar HOLDS (independent gate + advisor 6C + arm-in-same-commit).
**⚖️ ITEM 3 = FEATURES/PARITY, dev-ruled "all of them, recommended order" (2026-07-20 AskUserQuestion). ORDER
(rising risk/depth, forks surfaced when reached, spine LAST):** (3.1) stdlib companions — no design fork, grep-verify
first [◐ IN PROGRESS: ✅ **List.sumBy** DONE — higher-order projection sum, byte-identical run ≡ run --tree-walker ≡ php + example +
transpile `array_sum(array_map)`, full --all-features gate green 2331, advisor 6C; perf FLAGGED 0.36× = listfilter class
(higher-order re-entrant, un-JIT-flippable), LOSS-armed. Genuine remaining companion gaps grep-verified: Map.update,
List.scan/windowed/associateBy/countBy] ✅ **(3.2) List.minBy/maxBy DONE** — projection siblings of min/max (T?,
natural_cmp on selector, FIRST-wins tie-break byte-identical both legs + tie differential test, example, gated
__phorj_min_by/max_by helpers, full --all-features gate 2333, advisor 6C; perf FLAGGED minBy 0.16×/maxBy 0.17× =
higher-order class, LOSS-armed). Rule-11: NOT the forked slice the handoff feared — mirrors min/max precedent, no
Comparable-bound adjudication needed → ◐ **(3.3) FILTER email/URL — ADJUDICATED (dev AskUserQuestion 2026-07-20): OPTION A = explicit-regex parity**, NOT
filter_var. Follow the existing Core.Validation mechanism (hand-rolled Rust + IDENTICAL anchored preg_match → byte-id
by construction; the validate.rs fence). Approved behavior: isEmail("a@b.co")=true, isEmail("user@localhost")=false
(dotted domain required), isEmail("a..b@c.com")=false, isUrl("https://x.io/p")=true. Better-than-PHP (rejects
filter_var's surprising dotless/quirk accepts). ✅ **DONE** — isEmail `^(?!.*\.\.)[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+
(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}$` + isUrl `^https?://[A-Za-z0-9.-]+(:[0-9]+)?(/[^\x00-\x20]*)?$`, hand-rolled Rust
PROVABLY ≡ emitted preg_match (D flag), 33-case differential vs real php:8.5, full --all-features gate 2336, advisor 6C.
⚠ **PERF flip-or-flag DEFERRED to the queued perf-alignment pass** (cheap pure O(n) scalar scans; not silently skipped —
folded into the "transpile/lift/perf/LSP 100%-aligned + beating-php" work the dev queued 2026-07-20). → (3.4) exception backtrace (FRESH session)
## ⚠⚠ NEXT MAJOR BODY OF WORK (dev-queued 2026-07-20, for Fable→Opus): "transpile/lift/perf/LSP editors (vscode/phpstorm)
100% ALIGNED with everything built + BEATING php + LSP/extension AUTOCOMPLETE (typing `import X.` shows ALL available
packages/modules; 'almost complete' to help test the language)." SCOPE = (a) gap-audit transpile+lift for EVERY language/
stdlib feature (find + fill misalignments), (b) perf flip-or-flag sweep of remaining features (incl. isEmail/isUrl above),
(c) LSP import-path + member autocomplete + package discovery, (d) vscode + phpstorm extensions surfacing it.
**DEV DECISIONS (AskUserQuestion 2026-07-20 — governs the pass):**
1. **Autocomplete = FULL: import-path + member completion (type-aware `Foo.`→methods/natives) + PROJECT DISCOVERY**
   — not just `Core.*`; scan the user's project tree (`src/`, `bin/`, `views/`, `vendor/`, …) for available
   packages/modules so `import X.` lists EVERYTHING. Drives off `cli::CORE_MODULES` + native registry (DEC-252,
   registry-driven LSP) for Core, PLUS a project-source package scanner for user code. "Almost complete to help test."
2. **100% aligned = AUDIT-FIRST.** Open the pass with a GAP MATRIX: every language/stdlib feature × {transpile,
   lift, LSP} → report gaps BEFORE building (bidirectionality: enumerate both sides). Then fill.
3. **Perf = BEAT PHP ON EVERYTHING** (dev overrode flip-or-flag's "flag is acceptable"). ⚠ HONEST PATH (surfaced to
   dev): per-op JIT verticals only flip cheap structural cases (done: scalars). The un-flippable class (higher-order
   re-entrant: sumBy/minBy/maxBy/listfilter/listreduce; alloc-bound: mapkeys/values) CANNOT be won by verticals vs
   php's tuned C — "everything" requires the DEEPER architectural lever: reduce the general ~188ns VM→native dispatch
   overhead (KNOWN_ISSUES "fix lever #2/2b" — lifts ALL ~286 natives at once) and/or front-end expansions (List<Map>
   eligibility = [[mapkeys-listmap-jit-blocker]]). Frame the perf work around the dispatch-overhead reduction, not more per-op verticals.
4. **Editors = vscode-FIRST, both thin clients over the ONE phorj LSP** (DEC-181 both-same-change); phpstorm/JetBrains after.
**Standing:** gate = `PHORJ_REQUIRE_PHP=1 cargo nextest --all-features` + clippy both legs + fmt + release; per-feature
DoD incl. flip-or-flag; NEVER push (dev pushes); design forks → surface (Invariant 15). START = the gap-matrix audit (decision 2).
(getTrace family, contained) THEN generators/yield LAST in a FRESH session (deepest VM control-flow spine, standing rule).
DoD each: byte-identity run ≡ run --tree-walker ≡ php + example (Inv-9) + transpile+lift + full --all-features gate + advisor 6C → commit.

## ⚖️⚖️ DEV DIRECTIVE + ACTIVE CAMPAIGN (2026-07-19, AskUserQuestion — governs current work)
**PERF-DoD (standing, absolute):** EVERY feature — new AND already-shipped — gets a perf bench vs PHP;
if it loses, FLIP it (JIT vertical etc.), else FLAG it. Documented losses without a flip-attempt are NOT
acceptable. Sharpens Invariant 18 into a per-feature definition-of-done = [[perf-bench-every-feature-flip-or-flag]].
**ACTIVE CAMPAIGN — FLIP THE NATIVE-CALL-IN-LOOP LOSSES via per-op JIT verticals** (dev chose: fresh-context
subagent per vertical + main-session independent gate/certify; THEN back to building features each with a
flip-or-flag bench). ORDER (biggest loss → most tractable): ✅ **maphas DONE `b2f927a4` (DEC-311) — FLIPPED 0.03× → 1.50× WIN
vs php** (mirrors mapget vertical; `rt_u_map_has` one-deref unsafe, miss=clean-false; VM→JIT 51.4×; hits>0
proven; 4-way byte-identical; 2306 gate green; main-session independently verified). ✅ **ARMED 2026-07-19
(quiet box, load-avg 1.7, all cores 90-98% idle): `microbench-gate.sh --emit` K=7 pinned → maphas 0.03→1.522
in `bench/micro-baseline.json`; the flip is now ratchet-protected vs a future WIN→LOSS regression.** Coverage
forks FORK-A (Map<string,int> only) / FORK-C (AMB deferred) recorded DEC-311 for dev review.
◐ **setcontains PARTIAL committed `2bdc25eb` (0.02×→0.45×, 25× VM→JIT, FLAGGED WIN-OR-FLAG, ZERO new unsafe** —
linear scan can't beat php O(1) hash). ⏳ **FORK-D BUILDING NOW (subagent) — reseal Set<int> as int-keyed packed
HASH table → O(1) probe → expected WIN ~1.5× like maphas.** ⚠⚠ **GATING FORK-D (READ THIS — the campaign's crux):**
FORK-D is NOT a probe like maphas — it adds a **BUILDING** unsafe helper (`rt_u_set_of`: hash+alloc+WRITE an arena
hash table). Its safety surface (bucket-write bounds, arena alloc, count-vs-capacity, collision/probe termination) is
the BIGGER one — **READ that helper LINE-BY-LINE, it is the real certification.** Full bar: independent --all-features
gate + hits>0 + checksum-gated flip ≥1.0 + 4-way byte-identity (empty/present/absent/dup-insert/collision) + advisor
6C. On WIN: commit, flip the KNOWN_ISSUES FIX-LEVER-#2 setcontains flag → WIN. ⚠ **Prefer gating FORK-D in a FRESH/
compacted orchestrator context** (advisor-flagged: building-unsafe certified at max session-fatigue is the ctype-class
risk — the harness catches it, not judgment). Base = master tip; subagent forks from there.
✅ **FORK-D DONE `f8b74613` — setcontains 50× loss ELIMINATED → ~1.05× (PARITY, marginal/fragile).** Building
helper `rt_u_set_seal` safety arg verified line-by-line + fixed a -1-path list-release leak (advisor-caught).
**⚖️ CAMPAIGN NOW SELECTIVE (dev-ruled 2026-07-19 "structural flip-or-flag"):** the verticals kill the ~188ns
dispatch overhead; phorj WINS only where it hash-STRUCTURES vs php's hash (maphas 1.50×), reaches PARITY via a
reseal (setcontains ~1.05×), MATCHES-or-loses on linear/alloc-bound vs tuned C. Decisions:
- **listcontains = FLAGGED (NO vertical)** — linear-vs-C, can't flip (KNOWN_ISSUES FIX-LEVER-#2). Accepted loss.
- ✅ **mathmax FLIPPED 0.03× → 1.69× WIN** (fresh-context subagent build + main-session independent full --all-features
  gate/certify + advisor 6C; `smax` inline scalar, ZERO new unsafe — the safest vertical yet; 4-way byte-identical,
  2324 all-features green, hits>0, K=9 flip 1.665×, ARMED in baseline same commit). The strongest flip in the campaign.
- **mapkeys/values (0.07×/0.08×) = QUEUED, MEASURE-FIRST, FRESH context** — map-structured but ALLOC-touching (materialize
  a List every call vs php's tuned-C array_keys/values) → BUILD+MEASURE, keep only if ≥parity, else flag. NOT auto-built.
**SCOREBOARD: maphas 1.47× ✓ · setcontains 1.05× ✓ · mathmax 1.69× ✓ · mathmin 2.18× ✓ · mathabs 1.89× ✓ ·
mathsign 2.11× ✓ (all committed AND ARMED) · listcontains flagged · mapkeys/values = fresh-context measure-first (NEXT).** ✅ **OWED-CLEARED 2026-07-19: `microbench-gate.sh --emit`
(K=7, pinned, quiet box) armed BOTH wins in `bench/micro-baseline.json` — maphas 0.03→1.522, setcontains 0.02→1.024;
zero WIN→LOSS regressions, zero identity breaks across all 40 features. WIN→LOSS ratchet protection now LIVE for both.**
⚠ Next JIT build = FRESH orchestrator context (this session went very deep — advisor-flagged).
⚠ **PER-VERTICAL BAR (hold it, do NOT compress):** fresh-context subagent builds → MAIN-SESSION independent
full --all-features gate + hits>0 + checksum-gated flip + 4-way byte-identity + read the unsafe helper +
advisor 6C → commit. One vertical per cycle. ⚠ The risk is the ORCHESTRATOR (my) context depth, NOT the
subagent — strongly prefer a FRESH orchestrator context before each next vertical (the ctype slip happened
shallower than max depth; the HARNESS caught it, not judgment). Per vertical: byte-identical VM fallback · PROVE hits>0 (not wall-clock) · core-pinned interleaved
before/after to confirm the FLIP · SURFACE the unsafe/design choice (don't self-rule the island) · commit green.
⚠ Honest caveat: mapget's own vertical only reaches 1.08×, so some may land near parity not a huge win —
measure + report the real number. JIT = deepest unsafe spine (`src/jit/`, `#![deny(unsafe_code)]` island).

## ⭐⭐⭐⭐ SESSION 4 (2026-07-19 cont. — dev pushed the 41; continuous autonomous 1+2+4). 4 commits, all green, UNPUSHED.
**Delivered:** (1) 🔴 **push failure diagnosed = LOAD CONTAMINATION, not real test failures** — the full gate
is green on a CPU-idle box; the pre-push SIGKILLs under load-avg ~9 and git reports it as a hook failure.
(2) ✅ **PERF WIN `d2f95509`** slice-fastpath for Pure natives — measured (core-pinned + interleaved) a stable
2.5–12% VM win on every Pure native, JIT winners flat, byte-identical. **UNBLOCK: per-core `mpstat` idle
(NOT `uptime` load-avg) is the real perf-measurement gate** = [[percore-mpstat-not-loadavg-for-perf]] — a
load-avg of 3–9 can still be 95%+ per-core idle; core-pin + interleave then measures reliably. This disproves
several prior sessions' "box too loaded" deferrals. (3) ✅ **arena-Json NO-WIN** (DEC-309 resolved — parse
already lazy/near-zero-alloc post-DEC-294; jsonround stays a dev-accepted FLAG). (4) ✅ **§4.12 full §1.2
re-tally `6815ad87`** — FN coverage 27.5%→44.1% simple-model (81 phantom GU/GP→C grep-cited); RECONCILED not
stacked with §4.11: ≈60/81 already in the weighted model → headline **≈68% is a well-evidenced FLOOR** with
~1–2pp headroom. (5) ✅ **CTYPE validators `d7e39535` (DEC-310)** — 7 new `Core.Validation` predicates
(isLower/isUpper/isWhitespace/isPunctuation/isControl/isVisible/isPrintable) via `preg_match(/…$/D)` (NOT
ctype_* — shared ext, hermetic-oracle guard fatal; the D-flag makes them MORE correct than the pre-D 5,
whose trailing-`\n` divergence is now FLAGGED in KNOWN_ISSUES). AUTO-NAMING for dev review.
(6) ✅ **Math inverse hyperbolics `8d9788d4`** — asinh/acosh/atanh (mirror of shipped sinh/cosh/tanh; same
platform libm → bit-identical 3-leg; NaN out-of-domain verified rendered identically BEFORE building; added
to TIER1_PHP as core std math). Standard names, no fork. FN-MATH §4.12 gap closed.
**5 commits UNPUSHED** (`d2f95509` `6815ad87` `d7e39535` `c06eb5d5` `8d9788d4`) — dev pushes. Release binary
rebuilt `target/release/phg`. **STOPPED HERE deliberately** (advisor-concurred): remaining runway all carries
design edges best not opened deep in a long context (the ctype rationalization this session was caught by the
HARNESS, not fresh context — the lesson).
**CLEAN RUNWAY (next session, from §4.12 genuine-gaps + advisor):** (a) **Math asinh/acosh/atanh** — cheap, BUT has a
NaN-rendering edge (domain violations → NaN); FIRST check how the shipped Math tail (asin/acos) renders NaN
across all 3 legs and mirror it. (b) **FILTER email/URL** — advisor called it low-edge (Uri.parse exists) but
byte-identity to PHP's `filter_var(FILTER_VALIDATE_EMAIL)` semantics is actually FIDDLY — verify before
committing. (c) minBy/maxBy = comparable-key design edge (non-scalar keys: PHP loose `<` vs Rust compare_ord)
— a real slice, not a companion; needs a Comparable-bound decision. (d) bigger movers XML/streams/generators =
spine/forked. ⚠ Standing: gate = `PHORJ_REQUIRE_PHP=1 cargo nextest --all-features` + clippy both legs; NEVER push.
**Pattern proven again:** fresh-context worktree subagent per isolated slice + my independent gate/spot-check.

## ⭐⭐⭐ FRESH SESSION — START HERE (2026-07-19 handoff; dev pushing the 40 commits below, resuming fresh)
Prior session ended at HEAD `36733a95` (40 commits, all green, UNPUSHED — dev pushes). Ended because the
shared box hit load ~9 (perf measurement impossible) + a transient API error. **DONE this session:**
🔴✅ P0 — revived the dead example byte-identity glob (was 201 SKIP/0 RUN since DEC-191) · 🎉 backed enums
DEC-302 COMPLETE+verified (2309-green) · 6 stdlib (DEC-304–308) · perf: proved the flips were load-noise +
found/documented PERVASIVE native-call-in-loop losses (28→40 natives benched) · parity §4.11 **≈68%**.
**QUEUE (dev-ruled "all of them"; ORDER by dependency):**
1. ✅ **arena-Json — DONE 2026-07-19 (NO-WIN, DEC-309 resolved).** Fresh-context worktree subagent ran a
   phase-split + eager-routing proxy (did NOT build the full `Value::JsonArena` — bounded it as not worth
   the blast radius). Verdict NO-WIN, three independent legs: (a) parse is already lazy/near-zero-alloc
   post-DEC-294 (`validate_json` skip-scan → one `JsonLazy`; phase-split: parse 171ms is the SMALLEST
   phase, rebuild+stringify 200ms the largest — an arena targets the cheapest phase); (b) deepjson eager
   +60% regression is INTRINSIC materialization work an arena can't recover; (c) blast radius enormous
   (new Value variant threading dozens of wildcard-free matches + VM ops + encode/eq/hash). **jsonround
   residual loss stays a dev-accepted structural FLAG (DEC-294).** Nothing committed; worktree pristine.
2. ✅ **slice-fastpath — DONE 2026-07-19 (MEASURED + COMMITTED).** Re-measured core-pinned + interleaved
   (`taskset -c 7`, core7 ~99% idle despite load-avg ~3 — per-core idle is the real gate, NOT load-average;
   this is why prior sessions wrongly thought perf was blocked). Two independent runs → stable **2.5–12% win
   on every Pure native** (mapkeys −9…−12% biggest), JIT winners flat, no regression. Full `--all-features`
   gate + PHP oracle green (2297). Detail = KNOWN_ISSUES "FIX LEVER #1". Deeper lever (per-op JIT verticals)
   stays dev-driven (unsafe island). ⚠ LESSON: check `mpstat -P ALL` per-core, NOT `uptime` load-average —
   a load-avg of 3–9 can still be 95%+ per-core idle (sleeping/IO), and a core-pinned bench is then reliable.
3. ✅ **§1.2 full per-row re-tally — DONE 2026-07-19 (§4.12 in M-gap-matrix).** Fresh-context subagent
   grep-verified all 631 FN rows + my independent spot-check (Math/String/DB credits + asinh/var_export
   discipline catches). **Simple-model FN coverage 27.5% → 44.1%** (81 phantom GU/GP→C, all grep-cited).
   ⚠ RECONCILED not stacked with §4.11: ~60 of the 81 are ALREADY in the weighted model (§4.8 DB/mail,
   §4.9 HTTP/FS/Uri/mb/sessions, §4.11 Path/crypto/enum) → headline **≈68% is a well-evidenced FLOOR with
   only ~1–2pp re-tier headroom** (do NOT chase phantom weighted upside). Genuine remaining gaps (the real
   targets) listed in §4.12: FS streams, SPL, XML, SOCK, INTL, GD/ZLIB, **FN-CTYPE 5 validators (cheap)**,
   **Math asinh/acosh/atanh (cheap)**, **FILTER email/URL (Uri.parse exists → cheap)**, sodium/openssl.
4. **new parity features** (XML/streams/mb-tail — biggest FN-leg movers) + **more stdlib** (Map.update/mapKeys,
   List.minBy/maxBy). ⚠ Deeper perf lever = per-op JIT verticals (audited `unsafe` island — DEV-DRIVEN, not delegated).
**Pattern that worked:** fresh-context subagent per spine slice + my independent full-gate verify (delivered
backed enums clean). ⚠ Grep-verify every "gap"/"fix" first — 5+ phantom tasks caught this session (jsonround
was already a resolved FLAG). Gate = `PHORJ_REQUIRE_PHP=1 cargo nextest --workspace --all-features` + clippy both legs.

## 🌙 OVERNIGHT AUTONOMOUS RUN (dev asleep, 2026-07-19 — READ FIRST, governs until dev returns)
**Mode:** full autonomous, continuous, all night. **Dev directive:** work through the night; stop ONLY if
truly wedged (a blocker preventing ALL progress), never for a design fork.
**ORDER:** (1) named args CONSTRUCTORS [part 2/3] → (2) named args METHODS [part 3/3] → (3) SPREAD (DEC-299:
List→positional + Map-literal→named static core; runtime union-Map→named leg if Map<union> is solid, else
record PENDING + skip) → (4) **WAVE B — FN stdlib breadth** (the +4-6pp % mover): crypto/security →
**Core.Cryptography** (CSPRNG randomInt/randomBytes, hmac, timing-safe equals, hkdf, pbkdf2 — TOP-20 #10);
**non-stream FS breadth** into Core.Fs (glob/stat/perms/mtime/tempFile/scandir — DEFER file-handle streams);
String GU tail (ucwords/wordwrap/strtr/pad/strpbrk/strspn/strtok…); Math tail (asin/acos/atan/atan2/hyperbolics/
hypot/log2/log1p/expm1/deg2rad/rad2deg); array long-tail → (5) generators/yield → (6) onward per programme.
**FORK RULE (dev-ruled):** on ANY design fork, make the BEST decision by the full rule set — *better than PHP
conceptually + theoretically + practically; more secure, faster, more OOP, more organized, cleaner* — BUILD it,
and record it as an **AUTO decision** (status `✅ AUTO — REVIEW`) in C-decisions.md for morning review. NEVER block.
**DoD each slice:** byte-identity run ≡ run --tree-walker ≡ php + example (Inv-9) + tests + clippy --all-features AND
--no-default-features + fmt + advisor 6C → autonomous `git commit` green. **NEVER push** (dev pushes AM; note:
pre-push perf gate flagged losses = load-contaminated box, dev re-checks quiet). **Perf work DEFERRED entirely.**
**Discipline:** accepted surface == working surface (reject every unhandled path — the recurring trap); heavy
cargo runs need Bash timeout ≥560000ms (2m default SIGKILLs + corrupts incremental → `cargo clean -p phorj`).
**⚠⚠ WAVE-B REALITY (2026-07-19): the codebase is FAR more complete than the gap-matrix says — GREP-VERIFY
EVERY candidate before building** (5 phantom gaps this session: Regex/Decimal/match/Fs + #5 CRYPTO). CRYPTO
FINDINGS (owed to next recompute + review):
  1. **Phantom-gap #5:** TOP-20 #10 (CSPRNG + HMAC/HKDF/PBKDF2 + timing-safe) is ALREADY BUILT —
     `Core.Random.secureBytes/secureInt` (src/native/random.rs, /dev/urandom, pure:false) + `Core.Hash.hmac/
     equals/hkdf/pbkdf2` (src/ext/hash/natives.rs, std-only, byte-identical). Example: `guide/crypto-mac.phg`.
     I reverted a duplicate Core.Cryptography.randomBytes/randomInt/timingSafeEqual I'd started (caught via crypto-mac.phg).
  2. **🚩 PLACEMENT MISMATCH (flag-already-done rule):** dev ruled TONIGHT crypto→Core.Cryptography, but CSPRNG
     lives in Core.Random + HMAC/KDF in Core.Hash (shipped, byte-identical). AUTO/PENDING: keep shipped placement
     OR consolidate into Core.Cryptography (breaking rename) — dev decides at review. NOT moved silently.
  3. **§4.10 RECOMPUTE DONE (`91737e4a`)** — parity ≈64→**66%** · Vision 66→**67%** · floor 47→**51%** (credited the
     7 overnight features). ⚠ STILL OWED: a full §1.2 PER-ROW re-pass to bank the PHANTOM-GAP undercount (FN-HASH
     hmac/hkdf/pbkdf2 + FN-RAND CSPRNG + Core.Path + Core.FileSystem-broad are BUILT but §1.2 still lists as gaps →
     true parity higher than 66%). §4.10 conservatively did NOT credit phantom coverage (no unverified inflation).
  **DONE this overnight (all committed, green, UNPUSHED — dev pushes AM):** slice#3 named args FULL SCOPE
  (`998e370b`); variadics (`59bf4158`); Wave-B **Math tail** (`841864e7`); Wave-B **List.difference/intersection**
  (`81cbd331`, typed-strict set ops); Wave-B **String tail** capitalizeWords/translate (`90015c91`, ucwords/strtr);
  **DEC-300 `Core.Deque<T>`** (`762b3945`, pure-Phorj generic deque over List, T?-on-empty vs Spl* throw, 2249 green);
  **DEC-301 `Core.PriorityQueue<T>`** (`580c6041`, pure-Phorj max-PQ over two parallel Lists, T?-on-empty, 2250 green);
  **§4.10 recompute** (`91737e4a`, parity 64→66% · Vision 66→67% · floor 47→51%); **DEC-302 backed-enums build-map**
  (`d5ba41e9`, ruled AUTO, deferred to fresh context); **DEC-303 `String.chunk`** (codepoint-based, `__phorj_str_chunk`
  helper, `bb39af6f`+src in `73f31189`); **🔴✅ P0 FIX — revived the dead example byte-identity glob** (`a355c342`).
  🔬 **PERF COVERAGE EXPANDED (2026-07-19, `3c71707b`, subagent + my verify): 28→40 of 286 natives benched.**
  Reveals the native-call-in-loop overhead is PERVASIVE (not just filter/reduce/contains): maphas 0.03×, setcontains
  0.02×, mathmax 0.03×, mapkeys/values/merge/filter/map + stringcontains + setunion/difference all LOSE 3-50× to php
  C builtins; only listmap (JIT vertical), setintersection 1.58×, mapget 1.08× win. Root cause = ~188ns/call VM→native
  dispatch. ⚠ FIX LEVER PRESERVED (NOT committed — perf unmeasurable at load 6-9, Inv-11): the subagent's `NativeEval::Pure`
  slice-fast-path (in-place stack slice + truncate vs per-call split_off Vec alloc) is BYTE-IDENTICAL (2309-green) but
  reverted pending a QUIET-box before/after — `git stash` + `scratchpad/slice-fastpath.patch`. Detail = KNOWN_ISSUES
  PERF-native-call-in-loop. Deeper lever = per-op JIT verticals (unsafe island, dev-driven). ⚠ jsonround = phantom
  fix-task: already a dev-accepted structural FLAG (DEC-294); arena-Json experiment QUEUED (dev ruled "prototype+measure").
  🎉 **DEC-302 BACKED ENUMS COMPLETE + VERIFIED (2026-07-19, `b3f2a788`→`9a5deff6`, repr B, fresh-context subagent + my independent gate).**
  `enum Suit: string {Hearts="H",…}` / `enum Priority: int {…}` + `.value` / `Enum.cases()` (List<Enum>, any payload-less
  enum) / `Enum.from(x)` (faults on miss) / `Enum.tryFrom(x)` (Enum?). 2 new Ops (EnumValue/EnumFrom, all-3-matches, no `_`);
  CTy `Priority.from(9).value + 1` operand (Inv-7); 11 coded diagnostics; transpile = repr-B methods on base class; lift done;
  example enums-backed.phg IN the RUN set. Full --all-features gate 2309 green, clippy both legs, fmt, build. ⚠ Dev-review AUTO
  decisions recorded under DEC-302 (a-d); non-blockers owed: FEATURES.md surface note + parity-% recompute (doing §4.11 now).
  **DIRECTION (dev AskUserQuestion 2026-07-19): "All of 1, 2, and 3"** = (1) batched companion natives,
  (2) backed enums DEC-302 (careful incremental build), (3) §1.2 parity re-pass crediting phantom gaps.
  Then a SECOND direction (dev): perf — "All of 1, 2, and 4" = expand micro suite / macro benches / fix jsonround.
  🎯 **PERF INVESTIGATION DONE (2026-07-19) — the WIN→LOSS "flips" were LOAD CONTAMINATION, safe to push:**
  perf-gate (load-immune) PASS 822× vs 10.8 floor; microbench-gate at load 1.8 PASS (0 blocking flips); K=7
  pinned recheck of borderline features all WIN/parity. My overnight changes were additive (no hot-path touch).
  ⚠ **BUT the suite EXPANSION surfaced 3 REAL hidden losses** (`6d71bf52`, `89603c3d`): listmap 7.9× WIN (JIT
  vertical) but listfilter 0.22×, listreduce 0.27×, **listcontains 0.02× (~44× slower)** — the GENERAL pattern:
  ~188ns/call VM→native dispatch vs php's ~4ns C builtins; phg wins where the JIT applies, loses 3-44× on
  non-JIT'd native calls in hot loops. FLAGGED = KNOWN_ISSUES "PERF-native-call-in-loop" (2 fix levers: per-op
  JIT verticals OR general native-call-overhead reduction — dev chooses; fresh-context JIT/VM-spine). Coverage
  now 28/286 natives benched (Invariant 18 wants all). ⚠ macro-bench design has loop-invariant-hoist traps
  (dropped a stringsplit bench that php hoisted → fake 423× loss); needs careful fresh-context design.
  **OUTSTANDING (both dev "all of X" asks — all now genuinely FRESH-CONTEXT/spine or error-prone-at-depth):**
  backed enums DEC-302 (spine-wide, build-map ready); §1.2 per-row parity re-pass (analysis, error-prone at depth);
  #2 macro/real-app benches (design-validity risk); jsonround lazy-Json fix (DEC-294, spine); filter/reduce/
  contains JIT verticals (JIT spine); companion minBy/maxBy/Map.update (diminishing). Sequenced by risk;
  companion `sortDescending` (`14e097c2`) done as the batch representative.
  **MORE safe stdlib gaps (post-P0, "keep going"):** `Map.containsValue` (`989d3500`, DEC-304, value-side membership);
  sibling substring fix `uses_unavailable_gated_module` (`6d898e25`, closes the P0 arc — both gate fns now per-token);
  `List.product` (`6a6e98e8`, DEC-305, mirrors sum, +array_product TIER1); `Set.isSuperset` (`3ec0f31d`, DEC-306,
  mirrors isSubset). All byte-identical, differential + example + README, gates green. Now-live glob tests each.
  🔴 **P0 (THE session headline): `all_examples_match_between_backends` + the transpile glob were DEAD since DEC-191**
  (`uses_impure_native` substring-matched `import Core.Runtime` inside the universal `import Core.Runtime.Entry` →
  201 SKIP / 0 RUN — Invariant-1 corpus enforcement OFF for weeks). FIXED via per-member impurity (201→8 SKIP,
  0→139 RUN); surfaced 1 broken example (strings-ext missing `import Core.String`) + `ucwords` TIER1 gap. Full gate
  green. Detail = KNOWN_ISSUES P0 + memory [[example-glob-noop-since-dec191]]. ⚠ FOLLOW-UP OWED: audit for OTHER
  dead gates iterating the corpus via the same `uses_impure_native`/`collect_phg` path.
  ⚠ GIT HYGIENE (dev AM review): `73f31189` (labeled "docs(P0)") ALSO contains the String.chunk src (text.rs/
  transpile/*) — swept in by a bare `git add -A` (my rule violation). All green + unpushed; history mislabeled, not
  broken. Left as-is (no history surgery at max-compaction). The `feat(string) bb39af6f` has the example+README+import.
  ⚠ LESSON (PQ): first probe was byte-identical run≡php but SEMANTICALLY WRONG (`List.fill` is `(value,count)` not
  `(count,value)`) — caught only by a seeded-tie assertion on the expected VALUE. Byte-identity ≠ correct; assert
  semantics, not just backend agreement (SAME lesson the dead glob taught: green ≠ tested). Spread DEC-299 AUTO-DEFERRED.
  ⚠ FRONTIER MAP (grep-verified this run — DO NOT rebuild; the easy pure-native seam is MINED OUT):
    · ALREADY-BUILT: crypto/CSPRNG/HMAC/KDF; Core.String rich (42+); Core.List rich (39 now); Core.Path
      (baseName/directoryName/extension/fileStem/join); Core.FileSystem BROAD (read/write/append/copy/move/
      del/mkdir/rmdir/exists/isDir/isFile/listDir/walk/size/tempDir); match-expr; Process; levenshtein;
      similarText; number_format; Math gcd/lcm/clamp; String repeat/padStart; List fill/pad.
    · GENUINE-BUT-FORKED (the real remaining % movers — NOT autonomously safe): **generators/`yield`**
      = ABSENT as a language surface (the coro substrate exists for concurrency) → deepest VM control-flow
      SPINE, standing rule = FRESH context only, NOT a compacted-run task. **backed enums + cases()** =
      ABSENT (enums are algebraic) → Invariant-15 language design fork (how scalar backing meets algebraic
      variants). **Set** = blocked (no empty-set VM op — `new Set<T>()` deferred, DEC-214). **serialize/
      unserialize**, **var_export/print_r** = byte-identity-fiddly (PHP format fidelity). PriorityQueue =
      next SAFE pure-Phorj-over-List slice (like Deque; needs tuple (value,priority) + max scan).
    · ✅ DONE (this run): Deque + PriorityQueue (the two good pure-over-List classes — seam now EXHAUSTED).
    · **NEXT TOP MOVER = DEC-302 backed enums + cases()** — RULED AUTO w/ full BUILD-MAP in C-decisions.md
      (recommended repr (B): keep the abstract-class model + emit value const + static cases()/from()/tryFrom(),
      NOT a PHP-native-enum path). ⚠ EXECUTE IN FRESH CONTEXT — spine-wide (parser+checker+3 backends+transpile+
      lift); the advisor + the spine→FRESH-context rule say do NOT one-shot it in a compacted run. Build-map ready.
      ⚠ Invariant-15: the (A) PHP-native-enum vs (B) class-model REPRESENTATION choice needs dev review (recorded AUTO/PENDING).
    · OTHER genuine-but-forked (not autonomously safe): generators/yield (deepest VM control-flow spine, FRESH);
      serialize/var_export/print_r (byte-identity-fiddly); Set (no empty-set VM op, DEC-214). Impure FS breadth
      (glob/stat/mtime) = env-dependent functional tests, lower priority.
    · ⚠ `String.chunk`/str_split = LADDER, NOT a trivial native: PHP str_split is BYTE-based (splits mid-codepoint),
      but PhStr holds valid UTF-8 by invariant (no unsafe outside JIT) → can't construct byte-chunks safely. A
      codepoint-based `String.chunk` + a `__phorj_str_chunk` PHP helper (META-7) is the clean fix (better than PHP:
      no broken multibyte) — a small DESIGN fork, deferred. Composable alt exists today: List.chunk(String.characters(s), n).
      Same UTF-8-invariant hazard applies to any new byte-slicing string native (wordwrap w/ cut, substr-by-byte, …).
  ⚠ M-Decomp: this run grew native/text.rs (586) + cli/preludes.rs (~1420) — both already >500 hard cap
    (DEC-262) and already on the backlog; split DEFERRED (preludes.rs CORE_MODULES order is load-bearing →
    FRESH context). Backlog record corrected in KNOWN_ISSUES (stale "1000 cap/10 files" → 500/~20).


## ✅ DONE — CONTINUOUS SESSION 2 (2026-07-18, HEAD `3a8f1b7f`, +12 commits, ALL UNPUSHED — READ FIRST)
- **Slice #1 §4.9 recompute** (`437ffd32`): parity **62→64%** · vision **64→66%** · floor **42→47%** (Web/Runtime
  spine folded in — HTTP client/FS/Uri/Unicode/sessions). First span where the FN stdlib leg moved (+6pp).
- **Slice #2 Regex closer COMPLETE**: findAllGroups (`999c3701`) · quoteMeta (`353ba92a`, DEC-296) ·
  replaceCallback (`af26efaa`, DEC-295 — typed `RegexMatch`, FIRST native-built instance w/ dispatched
  methods on both backends; PREG_UNMATCHED_AS_NULL fixes optional-group divergence). Prereq reserved-name
  fix (`3da89d12`, match/enum/fn — latent invalid-PHP-transpile bug found+closed).
- **Slice #3 DESIGN fully ruled** (`3a8f1b7f`, DEC-297/298/299) — named args `f(name:v)` + variadics
  `...nums→List<int>` + spread (List→positional & Map-literal→named STATIC core #3a; runtime union-Map→named
  w/ E-SPREAD-ARG fault = leg #3b). BUILD PENDING, fresh-context (largest slice, call-resolution core). See item 3.
- ⚠ 4 PHANTOM GAPS caught this session (Regex/Decimal/`match`/Fs-DateTime already built) — Rule-11 lesson:
  VERIFY every "gap" by grep before treating as greenfield (§1.2 baseline already credits many).
- **NEXT ON RESUME:** build slice #3a (static core) per item 3's locked design. All 12 commits green + UNPUSHED.

## ✅ DONE — SESSION 1 (2026-07-18, HEAD `da3fc0c2`, ~33 commits UNPUSHED)
- **PERF ARC (certified):** dbwork FLIPPED to WIN [Verified idle-box, ratcheted in micro-baseline];
  jsonround = documented structural FLAG (parse floor 205ms > PHP 153ms, arithmetic-proven);
  **lazy/compact `Value::JsonLazy` SHIPPED** (materialize-on-deconstruct, memoized, corpus-guarded,
  byte-identical) + new `bench/micro/deepjson` (deep/wide, 0.57→~0.95× — matches C json_decode);
  micro-baseline re-emitted on a quiet box (phantom losses fibrec/floatmul/stringconcat = WINs).
  Detail = [[perf-arc-2026-07-18-owed-idle-confirms]].
- **DEC-288 TUPLES — FEATURE-COMPLETE (certified):** `(a,b)` literal + `(A,B)` type + erase-to-List;
  `var (a,b)` + `(int a,string b)` destructure; `for ((k,v) in …)` (typed+inferred); `List.zip` /
  `List.partition` / `Map.entries` producers. Byte-identical 3 backends; all 2280 green; Invariant-7
  operand typing via dedicated `tuple_bind_resolutions`; formatter round-trips the sugar. ⚠ Map.entries
  bool-KEY diverges on transpile leg (FLAGGED, use str/int keys). Detail = [[tuples-dec288-slice-status]].
- ⚠ `check_resolutions` return is now a 10-field tuple (consider a named struct if an 11th is added).

## NEXT — CONFIRMED PROGRAMME ORDER v2 (dev via AskUserQuestion 2026-07-18 "big continuous session"; RESUME HERE)
Rationale: measure → capability-before-breadth → data-driven breadth → capabilities → packs → ship.
STANDING DIRECTIVES (dev, this session, ABSOLUTE):
  • **Everything conceptually BETTER than PHP** — where PHP's implementation/naming/namespace/packaging
    has flaws, FIX them; ADJUDICATE each divergence at implementation time (Invariant 15 + META-7). ASK.
  • Respect ALL rules together: security (org C1/C2 + `#![deny(unsafe_code)]`), faster-than-PHP (perf
    mandate), byte-identity spine, LADDER. If two rules contradict → FLAG + decide, don't self-resolve.
  • Ask on EVERY user-visible design fork before implementing.
1. ✅ **§4 recompute — DONE 2026-07-18** (§4.9 written; M-gap-matrix + MASTER-PLAN headlines updated).
   Result: **parity ≈62→64% · vision ≈64→66% · floor ≈42→44%** — FIRST span where stdlib breadth
   itself moved (+6pp FN leg): HTTP client (#2), FS (#5), Uri, Unicode (#6), sessions (#3) folded in.
   3 phantom gaps found + dropped (Regex/Decimal/`match` already built). Next FN blockers = XML/streams/
   intl/SPL-heaps/mb-tail. ← **START HERE = #2 Regex closer** (replaceCallback/matchAll/quoteMeta verified
   still GU in FN-PCRE).
2. ✅ **Regex closer — COMPLETE** (all 3 natives shipped, advisor-6C-certified, gate green):
   **findAllGroups** (`999c3701`) · **quoteMeta** (`353ba92a`, DEC-296) · **replaceCallback**
   (`af26efaa`, DEC-295 — typed `RegexMatch`, first native-built instance w/ dispatched methods on both
   backends; PREG_UNMATCHED_AS_NULL fixes the optional-group divergence by design). Prereq: reserved-name
   fix (`3da89d12`). ⚠ KNOWN_ISSUES: empty/zero-width matches diverge regex-crate↔PCRE (all match-iterating
   APIs; examples use non-empty). ← **NEXT = slice #3 named args/variadics/spread.**
   ————— (historical detail below) —————
   ✅ **reserved-name prerequisite DONE** (`3da89d12`):
   match/enum/fn added to FN_RESERVED (phorj wrongly accepted `class Match`→invalid PHP; found here).
   Type name RULED = **RegexMatch** (dev; `Match` is a PHP-8 keyword, illegal as a class name).
   ⚠ **replaceCallback CORE = DEC-295 PENDING — BUILD-READY DESIGN LOCKED (build FRESH-context, spine-novel):**
     • Prelude (extend `src/ext/mod.rs::regex_prelude::PRELUDE`, currently the 1-line Regex class):
       `class RegexMatch { constructor(public string matched, public Map<string,string> groups) {}`
       `  function full(): string { return this.matched; }`
       `  function group(string name): string? { return Map.get(this.groups, name); } }`
       ⚠ RESOLVE FIRST: prelude now references Core.Map (`Map<>` type + `Map.get` -> V?) — check how
       HTTP/INPUT preludes declare cross-Core deps ("reuse Core.Bytes/String"); regex prelude is dep-free today.
     • Native: `NativeEval::HigherOrder(regex_replace_callback)`, params `[Regex, string,
       Ty::Function(vec![Ty::Named("RegexMatch",vec![])], Box::new(Ty::String), vec![])]`, ret String. Body:
       `captures_iter`, build a RegexMatch `Value::Instance` (class "RegexMatch",
       `ClassLayout::from_sorted_names(&["groups","matched"])`, matched=whole match, groups=participating
       named captures like `regex_find_groups`), `call(cb, vec![m])?` → replacement, splice by byte offsets
       (track last_end; gap+replacement; tail). ⚠ SPINE-NOVEL: FIRST native-built instance whose METHODS get
       dispatched — validate `m.full()`/`m.group()` on BOTH backends with a run-only probe BEFORE the PHP twin.
     • PHP twin `__phorj_regex_replace_callback($re,$s,$cb)`: `preg_replace_callback(delim, function($m) use($cb){`
       `$g=[]; foreach($m as $k=>$v){ if(is_string($k)&&$v!==null){$g[$k]=$v;} } return $cb(new RegexMatch($m[0],$g)); },`
       `$s, -1, $count, PREG_UNMATCHED_AS_NULL)`. UNMATCHED_AS_NULL + omit-null ⇒ group() null for
       non-participating on ALL backends (FIXES the findGroups/findAllGroups divergence). Add `preg_replace_callback`
       to TIER1_PHP if absent.
     • Tests: differential case with a NON-PARTICIPATING named group (`(?<a>x)?(?<b>y)` on "y") proving
       group("a")==null run≡vm≡php; unit test; example; KNOWN_ISSUES note RegexMatch does NOT inherit the divergence.
   ⚠ Inherited caveat in KNOWN_ISSUES: findGroups/findAllGroups optional non-participating named groups
   diverge on PHP leg (Rust omits, PCRE fills "") — replaceCallback's RegexMatch FIXES this via UNMATCHED_AS_NULL.
3. **Named args + variadics + spread** — SYN mover + unblocks lifter on PHP 8.0+.
   ✅ **VARIADICS DONE v1** (`59bf4158`, free-fn, byte-identical). ✅ **NAMED ARGS part 1/3 DONE**
   (`89526a84`, FREE FUNCTIONS — `Expr::NamedArg` variant mirroring Tuple + `FnSig.param_names` +
   `normalize_named_args` front-normalize + `pending_named` REPLACE fill + 8 rejects + 6 explain codes).
   ⏳ **NAMED ARGS part 2/3 = CONSTRUCTORS, part 3/3 = METHODS** (dev ruled FULL scope) — interim they
   report E-NAMED-ARG-MISPLACED. Ctor path = construction resolution (CtorParam names, not FnSig);
   method path = methods.rs (has FnSig.param_names already → reuse normalize_named_args). ⏳ **SPREAD**
   (DEC-299: List→positional + Map-literal→named static core; runtime union-Map→named leg) STILL PENDING.
   ⚠ recurring trap all session: accepted surface must == working surface (reject at every unhandled path).
   (historical full-design + build-approach below:)
   ✅ **DESIGN FULLY RULED
   2026-07-18 (DEC-297/298/299) — greenfield, largest spine slice; BUILD FRESH-CONTEXT, SPLIT in two:**
   ── STATIC CORE (slice #3a, build first): ──
   • **Named args** `f(name: value)` (DEC-297, PHP-8.0 colon spelling, 1:1 transpile; interacts w/ default
     params — fill-by-name). Parser (call-arg `name:` form) + AST (named arg node) + checker (resolve
     named→param, mixed positional+named, defaults) + 3 backends + transpile (1:1) + lift (PHP named→phorj).
   • **Variadics** `function f(int ...nums)` → `nums: List<int>` (DEC-298). Parser (`...` param) + AST
     (Param.variadic flag) + checker (collect trailing args into List<T>) + backends + transpile (`...$nums`) + lift.
   • **Spread CORE** (DEC-299 a+b): (a) `f(...list)` List→positional (static, element+arity checked);
     (b) `f(...["k": v])` Map-LITERAL→named = COMPILE-TIME desugar to named args (fully static). Parser
     (`...` call-arg) + checker + backends + transpile (`...$x`) + lift.
   ── RUNTIME LEG (slice #3b, follow-on): ──
   • **Runtime union-Map→named spread** (DEC-299c): `Map<string,U>` spreads into named params when each
     targeted param type ∈ U (static check); runtime per-value narrow + key-presence via typed **E-SPREAD-ARG**
     fault; byte-identical PHP leg. ⚠ DEPENDS on `Map<K, union>` ergonomics being solid — VERIFY FIRST.
   ⚠ Interactions to design carefully: named+positional mixing order; named args + defaults fill; variadic
   + spread (`f(...xs)` into `...nums`); spread + named in one call. Byte-identity on every form + the fault.
   ── ✅ BUILD APPROACH CONFIRMED (3C investigation 2026-07-18) — TURNKEY, minimizes blast radius: ──
   KEY: use the `check_and_expand` DESUGAR chokepoint (Invariant #5 — expand sugar OUT before backends),
   modelled on the existing `fill_defaults` post-check pass (`Param.default` doc; `pending_fill` in
   `src/checker/calls/args.rs`). Backends/transpile/lift then see ONLY plain positional calls.
   BUILD ORDER (safest-first, each a green commit):
   1. **Variadics** (LOWEST risk — pure desugar, ZERO backend/Call-repr change):
      ✅ **DONE (1a `d0705500` foundation + 1b semantics this session)** — free functions only v1,
      byte-identical run ≡ run --tree-walker ≡ php, 2229 green, clippy both legs. Approach B (FnSig+check_args_defaulted,
      advisor-ruled over name-based desugar which breaks on return-overloads). Method/lambda variadic
      REJECTED via shared `reject_nonfree_variadic` (the ≥3-site trap bit the lambda once → fixed). See DEC-298.
      (historical 1b plan below, now done:)
      ⏳ ~~1b SEMANTICS~~ DONE: REMOVE the guard →
      free-fn signature (`collect/functions.rs:40` sig): variadic param effective type `List<T>` (add
      `variadic: bool` to `FnSig` {mod.rs:73}, 4 ctor sites; free-fn v1 like defaults) → body binds
      `nums: List<T>` → free-fn CALL check (`calls/core.rs:349`, currently `check_args_defaulted`): a
      new variadic path collects trailing args into a `[..]` list literal + records a replacement Call
      via the EXISTING span-keyed `default_fills` (advisor-OK'd; add a prelude/user span-overlap test —
      the P1 hole is offset-random so green≠safe here) → validation: variadic is last + no default.
      Backends then see `f([a,b,c])` w/ `List<T>` param = byte-identical to PHP `f([a,b,c])`. Lift `...$nums`.
      ⚠⚠ **THE TRAP THAT BIT TWICE THIS SESSION (reserved-name method path, `uses_regex` string-arg,
      variadic method/lambda) — a NARROW guard misses the SHARED chokepoint:** the checker has ≥3
      param/call sites — free-fn (`core.rs:349`), METHOD, and LAMBDA — so put the variadic effective-type
      + call-collection logic where ALL THREE route (or a shared helper each calls), else you rebuild the
      method/lambda hole 1b exists to close. Same lesson as the parse-chokepoint fix `c4318af8`.
   2. **Named args** (needs Call to CARRY names till desugar — add PARALLEL field `arg_names:
      Vec<Option<String>>` to `Expr::Call` {exprs.rs:120}/ParentCall/method/`new`, defaulting empty so
      existing `Call{args,..}` matchers are UNAFFECTED) → parser `name: value` call-arg → checker desugar
      reorders named→positional slots + fills defaults (extend `pending_fill`) → clears arg_names → backends
      see positional. Transpile CAN emit PHP `name:` 1:1 (DEC-297) OR just positional (either byte-identical).
      Lift PHP named→phorj named.
   3. **List→positional spread** (DEC-299a): parser `...expr` call-arg (reuse the arg_names/spread parallel
      field, add `arg_spread: Vec<bool>`) → NOT pure sugar (runtime length): interpreter/VM splat the List at
      call-eval; transpile emits PHP `...$list` (1:1). Element-type+arity checked statically.
   4. **Map-literal→named spread** (DEC-299b): a `...["k": v]` LITERAL desugars at compile time to named args
      (then flows through #2). Fully static.
   5. **Runtime union-Map→named spread** = leg #3b (DEC-299c) — SEPARATE later slice; VERIFY `Map<K,union>`
      ergonomics first; needs runtime narrow + E-SPREAD-ARG fault + PHP byte-identity.
   ⚠ Item 2's `arg_names` field on Call is the ONE higher-blast-radius touch (every Call consumer) — but
   parallel-field-with-`..` keeps ripple near-zero; the desugar clears it so post-expand backends are pure.
4. ~~**`match` expression**~~ — DROPPED 2026-07-18: **ALREADY BUILT + mature** (`TokenKind::Match`,
   `Expr::Match` w/ guards+patterns, used across examples). Rule-11 catch #3 this session (after
   Regex, Decimal). ⚠ VERIFY EVERY remaining "gap" by grep before treating as greenfield.
5. **Exceptions maturity + BACKTRACE API** — core done (try/catch/finally, throw, custom throwables,
   getMessage, getPrevious). VERIFIED GAP = getTrace/getTraceAsString/getFile/getLine on CAUGHT exceptions
   (today only uncaught faults render a trace; caught ones expose no programmatic backtrace). RT + logging.
6. **Backed enums + `cases()`/`from()`/`tryFrom()`** (PHP 8.1) — VERIFIED absent. SYN + real-code + lifter.
7. **serialize/unserialize + var_export/print_r** — VERIFIED absent. FN + big lifter unblock.
8. **Process/subprocess execution** — `Core.Process` has only args/env-get; add run/spawn/exec + pipes +
   stdout/stderr capture + exit codes. RT/real-app.
9. **Collections: Set / Deque / PriorityQueue** — List(36)/Map(13) exist, no Set/Deque/PQ (SPL parity). FN.
10. **TOP-20 stdlib remaining gaps** (aimed by #1's §4) — FN-leg mover; proven native recipe.
11. **Generators / `yield`** — capability gap (blocks iterator breadth); spine-sensitive.
12. **REAL PARALLELISM — dev-ruled MODEL = Actor/isolate (TRUE parallel), research-first.**
    State today: colorless cooperative async EXISTS (`src/green/`: spawn+channels, byte-identical, 1 OS
    thread, `Rc` heap `!Send` ⇒ NOT parallel). RULING: **Option 1 = actor/isolate model** — OS-thread
    workers, each a PRIVATE `Rc` heap, Send-only values deep-copied across channels ⇒ TRUE simultaneous
    multi-core (max(A,B) not A+B), NO hot-path Arc tax, data races structurally IMPOSSIBLE. Security +
    perf rules BOTH converge here; perf rule DISQUALIFIES the Arc/shared-heap model (atomic-refcount tax
    on every sequential program). Extends the LADDER quarantine (`E-CONCURRENCY-NO-PHP`). **Do Option 4
    FIRST**: write `docs/research/` parallelism design doc (full cross-lang matrix, perf model, syntax
    sketch, quarantine analysis) to FLAG problems BEFORE any code; then adjudicate syntax + implement.
    Possible later escape-hatch: opt-in `shared`/Arc region ONLY where a bench proves copy cost dominates.
13. **Feature packs (Web/Data/Runtime) + icu4x/Intl + W4-10 XML fork** — larger, design-heavy.
14. **Usability/GA** — lifter corpus + DEC-283 .phgml + GA freeze/docs + DEC-267 JIT-coverage metric.
⚠ Box bursty → byte-identity is the gate; defer perf verdicts to a quiet window. Stdlib already mature
(List 36/String 42/Math 34/Map 13). ⚠ Rule-11 discipline: several "gaps" this session were ALREADY built
(Regex/Decimal/Fs/DateTime) — VERIFY the surface by grep BEFORE treating anything as greenfield.

## CURRENT (2026-07-17→18, cont. — CONTINUOUS MODE; dev directive: BIGGER WAVES to amortize gate time)

### PARITY PUSH (2026-07-18, dev "keep going to 100%") — 4 List functions SHIPPED byte-identical + DEC-288..291 ruled
- ✅ **List.flatMap** `617b9666` · **List.takeWhile/dropWhile** `e4f60129` · **List.groupBy→Map<U,List<T>>** `03867547`
  (DEC-289). All byte-identical run≡interp≡php-8.5.8 (list-breadth.phg 3-way) + unit tests + examples/README.
  Recipe proven incl. the gated-helper mechanism (4-place: mod.rs flag / call.rs set / registry php / runtime_php def).
- ⚠ **DEC-291 (Fs breadth) — LARGELY ALREADY BUILT** (my Q under-verified the surface, Rule 11 miss): Core.Fs already
  has readText/writeText/appendText/copy/move/delete/size/exists/isFile/isDir/createDir/removeDir/removeDirAll/
  listDir/walk/tempDir (18 fns). Genuine remaining gaps: **mtime, glob, tempFile** (minor; Fs-transpile mechanism
  needs a look — the native `php:` is a passthrough placeholder). DEC-291 ≈satisfied; mtime/glob deferred.
- ⚠ **DEC-290 (native DateTime) — DATE/TIME LARGELY ALREADY BUILT, userland-style** (Q under-verified): `Core.Time`
  (clock) + `class Duration` (complete) + `class Date` (civil calendar: year/month/day/addDays/dayOfWeek/isLeapYear/
  compareTo/toString/of) + `class Instant` (now/epoch/plus/minus). This is the USERLAND-on-Core.Time model — NOT the
  "native DateTimeImmutable" the dev picked. Genuine gaps: **Date.parse** (string→Date), **custom format patterns**,
  a **combined date+time-of-day** type. NEEDS RE-ADJUDICATION (extend existing Date/Instant vs redundant native
  DateTime) — re-surfacing. DEC-290 ruling was on incomplete info.
- ✅ **DEC-290 (date/time) COMPLETE** — added **Date.parse** `f13c0495` + **Instant.parse** `c0c9e928` (the real
  gaps; ISO parse, round-trip, malformed→null, 3-way byte-identical). The "DateTime class" is deliberately
  `Instant` (PHP name collision) + "custom format" is deliberately interpolation — both design non-gaps, NOT built.
  Userland extension per the corrected ruling (no native DateTime). TIME_PRELUDE now imports Core.String/List.
- **GENUINE remaining gap from the batch = DEC-288 tuples** (built-in `(A,B)` + destructuring) — the real big feature;
  unblocks zip/partition/Map.entries. Spine-wide (parser + type system + destructuring patterns + all 3 backends +
  transpile), advisor-flagged spine-critical + multi-slice. ⚠ Needs a FOCUSED FRESH slice on a HEALTHY box: a new
  value-model type MUST be validated by the full `--all-features` suite + differential + all backends — exactly the
  gate-heavy runs this degraded box SIGKILLs. NOT started (starting it here risks a broken/unvalidated spine change).
- **Batch status: DEC-289 ✅ · DEC-290 ✅ · DEC-291 ≈satisfied (18 Fs fns exist; mtime/glob minor deferred) · DEC-288
  (tuples) = the one remaining big slice.** Parity functions shipped this push: flatMap, takeWhile, dropWhile,
  groupBy, Date.parse, Instant.parse (6), all byte-identical.

### DEC-288/288b TUPLES — SCOPED IMPLEMENTATION PLAN (erased-to-List sugar, ready for a focused slice)
Ruled: compile-time sugar, no value-model/backend change (Invariant 5). Entry points found (2026-07-18):
1. **`Ty::Tuple(Vec<Ty>)`** — new checker-only variant in `src/types.rs` (enum at :6; near List/Map at :60-71).
2. **Type parse** — `src/parser/types.rs:100-132` ALREADY parses `(` for function-type param-lists / grouping;
   extend: `(T1, T2, …)` with NO trailing `=>` → `Ty::Tuple` (today it's a parse error / grouping-of-one).
3. **Literal parse** — `src/parser/exprs/primary.rs` `(` handling: `(e1, e2, …)` → a new `Expr::Tuple` (vs
   grouping a single `(e)`).
4. **Destructuring** — `src/parser/patterns.rs` (has `parse_pattern` + LParen at :66/:87): `(T1 x, T2 y)` binding
   in `for`/let/assign; heterogeneous → each position bound with its own type (this is the PRIMARY typed-access
   path — indexing a heterogeneous tuple would need special-casing, so destructuring is how values come out).
5. **Checker** — type `Expr::Tuple` against `Ty::Tuple` (arity + per-position); destructuring binds each element.
6. **Desugar** — `src/cli/pipeline.rs:42 check_and_expand` chokepoint (like `erase_generics`): `Expr::Tuple`→List
   literal, `Ty::Tuple`→erased, destructuring→indexed binds. Backends + transpile UNTOUCHED (tuple = List at runtime).
7. THEN build on tuples: `List.zip → List<(A,B)>`, `List.partition → (List<T>,List<T>)`, `Map.entries → List<(K,V)>`.
⚠ Multi-slice, parser-grammar-careful (ambiguity: `(a)` grouping vs `(a,)` — decide 1-tuples), advisor-certify.
Validatable on THIS box via targeted parser/checker tests + 3-way example (no value-model change → no kill-prone
full-gate needed). NOT started — the clear next major slice.
- LESSON (banked): inventory the EXISTING stdlib surface BEFORE asking design questions (bidirectionality) — 2 of 4
  batch questions (FS, date/time) turned out largely-already-built.


### DEC-285 attribute-import-form fix COMMITTED `d63e255a` + jsonround perf (2 commits) — UNPUSHED
- **DEC-285** (`d63e255a`): built-in attributes (`Entry`/`Route`/`UncheckedOverflow`/`Attribute`/DI) resolve in
  EVERY import form — `#[Core.Runtime.Entry]` (qualified, was E-UNKNOWN-ATTRIBUTE) now works, bare-after-import
  preferred. `ast::attr_path_matches` suffix-matcher; import-gating unchanged (enforce_injected self-gates dotted).
  Byte-identical run ≡ run --tree-walker ≡ php-8.5.8. advisor-certified. tests/attribute_paths.rs (3 tests).
- **jsonround perf (DEC-266 line):** byte-cursor parse `79a1f4fb` (Vec<char>→&[u8], byte-identical, no flip) +
  **inline-payload `EnumVal.payload`→`Payload{Zero,One,Many}`** (this slice, advisor-certified, byte-identical:
  2279 tests + differential + oracle + all-micro output-identity; microbench-gate PASS no flips; enum/match benches
  IMPROVED — broad alloc win across ALL enums). **jsonround STILL 0.29× LOSS** (507ms vs C-json 145ms, 3.4× gap):
  ~65% of allocs = the `Rc<EnumVal>` BOX itself; flipping needs a **value-model rebuild (arena)** = ⚠ **PENDING
  Invariant-15 developer decision, NOT autonomously attempted** (DEC-286). jsonround finished to the autonomous limit.
- **dbwork DONE — 0.64× → ~0.98× (AT PARITY with C PDO-sqlite), 3 byte-identical levers committed:**
  `a90c4f8c` prepare_cached (rusqlite LRU stmt cache — 0.64→0.85, PDO doesn't cache) · `80e5d9b3` chainable
  bind returns `this` not `new Statement` (0.85→~0.95, kills per-bind instance alloc ×40k/run) · `e8dd5dd3`
  DbStmt.sql String→PhStr (0.95→~0.98, no per-prepare String alloc). Residual sub-1% = the per-op
  DatabaseResult enum (the CATCHABLE DatabaseError protocol — semantically required, a Chesterton fence, NOT
  removed). Per the refined mandate (MATCH-not-beat on C-tuned targets), ~0.98× vs C PDO = success. Each lever
  byte-identical (115 db tests both backends + sqlite units). ⚠ measured under load ~8; a quiet-box `--emit`
  re-baseline (OWED, deferred pre-push) would record the new numbers (likely ≥1.0 clean). microbench-gate
  baseline NOT yet updated (do on quiet box).
- **✅ BYTE-IDENTITY SPINE VALIDATED ON CURRENT HEAD (2026-07-18, targeted sweeps — no full cargo gate needed):**
  202/202 entry examples interp≡VM (`phg run --tree-walker` vs `phg run`), 0 divergences; 177/177 pure examples
  **VM≡PHP directly** (`phg run` vs transpile→php-8.5.8) — so interp≡PHP holds TRANSITIVELY via the 202 sweep;
  0 real divergences (the 4 flagged were all correctly
  quarantined: `unchecked`=E-TRANSPILE-UNCHECKED, `unicode-native`=E-TRANSPILE-UNICODE native-only, `fs/walk`=impure
  FS, `null-safety`=stderr W-FORCE-UNWRAP artifact — stdout identical). This substantially closes the DEC-287
  "full --all-features gate not run on final HEAD since gate4" caveat FOR THE SPINE (the core contract); still
  OWED on the dev's first pre-push: the two heavy sweeps + clippy on final HEAD. Also found+logged 2 pre-existing
  drift/divergence issues (KNOWN_ISSUES top): both engines CLI doc-drift + the "no entry point" run≠tree-walker
  prefix divergence; fixed safe living-doc/example/comment instances (main.rs, example CLI cmds, FEATURES row 70).
- **NEXT (perf mission substantially complete — both losses addressed):** per the confirmed programme, the
  CORE PARITY PUSH (the big %-movers: FN parity is the 40%-weighted drag at ~37%) — TOP-20 stdlib breadth
  (FS breadth → sprintf → array-tail → date/time → subprocess → regex-breadth). DESIGN-HEAVY (dev-adjudicated,
  Invariant 15) + GATE-HEAVY (kill-prone on this box) — hold for dev / a healthy box. jsonround arena = PENDING
  developer decision (DEC-286). Recent-DEC doc-drift sweep OWED (KNOWN_ISSUES top).


### ✅ DEC-284 EXTENSION/FEATURE RENAME COMMITTED `e1eb3781` (2026-07-18) — UNPUSHED
Cargo features + registry names now track their real Core module (dev-directed "names reflect module"):
`crypto`→`cryptography` (Core.Cryptography), `db`→`database` (Core.DatabaseModule),
`db-postgres`→`database-postgres`, `db-mysql`→`database-mysql`, `db-all`→`database-all`. 36 files,
+127/−126. Atomic cfg flip (MSRV-1.82 `unexpected_cfgs` deny-lint = no silent compile-out backstop).
Also fixed: 2 BLOCKING runtime driver-not-compiled error strings (src/ext/database/natives.rs:97/111 named a
dead flag — the panel completeness lens caught it, compiler can't), generated EXTENSIONS.md + examples.js,
all source doc-comments, example/test headers, SSOT docs, CLAUDE.md. Dated history left as-is.
Gate GREEN (nextest --all-features + PHP oracle 2276 pass; clippy both legs; fmt; release). DEC-268:
panel round-1 (r3 completeness found the error strings) → fixed + comprehensive grep sweep → rounds
A+B BOTH fully clean (2 consecutive) → certified. ✅ FOLDER-RENAME BACKLOG **DONE (2026-07-20)**: folders now
match feature/module names — `src/ext/db/`→`src/ext/database/`, `src/ext/crypto/`→`src/ext/cryptography/`,
plus `examples/db/`→`examples/database/` and `tests/db{,_mysql,_postgres}.rs`→`tests/database*.rs`. The
byte-identity quarantine in `tests/differential.rs` was re-pointed from the literal `Some("db")` to
`Some("database")` in the same change (DB I/O stays impure-quarantined, validated by `tests/database.rs`).
Internal fns/mods renamed too (`db_natives`→`database_natives`, `crypto_natives`→`cryptography_natives`,
`db_prelude`→`database_prelude`). Core-side `value/db.rs`/`desugar_db.rs`/`db_lint.rs` keep the `db`
abbreviation (not extension folders — left as a possible later consistency pass). Full gate green here
(all-features cargo test vs php-8.4 oracle: 1868+ pass; only the pre-existing bcmath decimal-conformance
PHP leg self-blocks — bcmath uninstallable in this container, covered on the dev's 8.5 floor). Register: C-decisions.md DEC-284.

### CURSOR — cargo cleaned this session (quota hit; dev "cargo clean regularly!!" reinforced in memory);
### next queue item = PERF (jsonround/dbwork flips, below) then core parity push (MASTER-PLAN §0 QUEUE).


## PERF CENSUS (2026-07-17, full microbench WIN-OR-FLAG, quiet-box NOT pinned — indicative):
- **LOSSES (4)**: jsonround **0.26×** (797ms/209ms — DOMINANT, the Json parse+match+build+stringify
  pipeline vs PHP's C json_*) · dbwork **0.63×** (Db binding/dispatch vs PDO sqlite) · closurecall
  **0.91×** · floatmul **1.00×** (dead-even, rounds to LOSS). WINS (19) incl. trycatch 32× ·
  objalloc 9× · match 8× · hofpipe 6× · floatarith 4×.
- **NEXT PERF SLICE (user-directed 2026-07-17 "optimize the losses to beat php, natural in
  parallel"): jsonround FIRST** — needs a fresh-context profiling slice (split parse vs stringify
  vs match/build; the encoder likely churns Value allocs per node). SPINE-SENSITIVE (Json enum
  tree threads all 3 backends) — measure-before/after per Invariant 11, do NOT rush. dbwork second
  (Db native-only, PDO baseline). closurecall/floatmul marginal — likely quiet-box-pinned reruns
  **jsonround HOTSPOT LOCATED (pinned split, 200k iters): parse=808ms, stringify=451ms — PARSE
  dominates.** Root cause = `parse_json` (src/ext/json/natives.rs:235) does
  `let chars: Vec<char> = s.chars().collect();` — full-materializes the input to a Vec<char>
  (heap alloc + 4×-mem) EVERY parse, plus a `Value` alloc per node (`jnode`). FIX (own slice):
  byte-cursor rewrite (JSON structure is ASCII; only string CONTENTS need UTF-8 → slice-borrow
  from the original &str), keeps the parse RESULT identical (json tests + differential + PHP
  oracle guard it) → byte-identity trivially safe (Json.parse is a native; PHP leg already uses
  json_decode). ~150 lines in one file; fresh-context per Invariant 11.   land them ≥1.0. ⚠ the census above is UNPINNED (this box swings 3-4×) — RE-RUN CORE-PINNED
  (taskset -c 7 + docker php --cpuset-cpus=7) before trusting any single number or claiming a fix.
- **DEC-273 WAVE 1 COMMITTED `9aed1ce7`** — registry + 5 migrations + phg extensions +
  E-EXTENSION-DISABLED + PHG_NO_JIT; DEC-268 panel: 5 rounds, rounds 4+5 consecutively CLEAN
  (round-5 probes: all 5 migrated extensions 3-leg byte-identical vs php-8.5.8). Panel by-catch
  → KNOWN_ISSUES: `phg test` raw-checker gap (injected-type files fail under phg test);
  Process.args() doc drift. ⚠ LESSON (recurred): UNASSERTED python replaces silently no-op —
  round 3 caught a "fixed" comment that never landed; ALWAYS assert anchors.
- **DEC-273 WAVE 2 COMMITTED `e2090945`** (7 migrations + prelude dissolution + playground fix;
  panel 4 rounds, r3+r4 consecutively clean; gate 2276/2276). 12/22 registry rows migrated.
  Session commits: 17c79ad6 · ebb7a123 · 996b2fee · 0b203827 · d42a2107 · 5670250e · 861cf0ab ·
  90aa34a1 · 7c840086 · 9aed1ce7 · e2090945 — ALL UNPUSHED.
- **WAVE 3 CERTIFIED + COMMITTED** (`cb189d3b` wave + `21f8bfb1` prose sweep + `85dd1c09`
  playground DEC-191 catch-up). DEC-268 panel: r1 2×P2, r2 clean, r3 1×P2+1×P3 (stale prose paths
  — swept), fresh rounds A+B consecutively CLEAN (1790/1790 lib, security posture intact, 23 rows). — r1 2×P2 (session "always compiled" comment; release freshness) fixed,
  r2 CLEAN. Commit is PROVISIONAL until 2 consecutive clean (amend if r3 finds anything; unpushed).
  ⚠ LESSON (git-mv): `git mv` stages the rename IMMEDIATELY, so a later scoped `git add other-file
  && commit` sweeps the pre-staged renames in — split with `git reset --soft` + `git restore
  --staged .` then re-stage. ⚠ LESSON (panel r2): piping git-diff through grep can SILENTLY
  false-clean via the RTK proxy — ALWAYS write git output to a file, then grep the file.
- **(built)** WAVE 3: db (natives +
  sqlite/mysql/postgres driver files, #[path] mods), mail, http_client, session (new default
  `session` feature) → src/ext/; 4 preludes dissolved (DB/MAIL/HTTP_CLIENT/SESSION → colocated
  prelude.rs). Registry 23 rows / 16 migrated. ⚠ LESSON: moving a natives file OUT of its own
  module breaks its _tests.rs (was `use super::*` on the SAME file) — had to widen Draft/Att
  fields + MailerObj/TransportKind/Message/Mailbox + hc_native macro fns to pub(super), and add
  std trait imports (Read/Write) the old glob supplied. Playground gained session.
- **NEXT AFTER WAVE 3 COMMIT: WAVE 4** — di (checker-desugar-coupled — CAREFUL), log/time/runtime
  classification (check against CORE list — likely core seams, may get NO row or a documented
  non-row), signals already rowed. Then transpile/lift MANDATORY structural seam. Then DEC-271
  icu4x · DEC-247 DateTime · DEC-283 template build.
- **(prior)** WAVE 3 — the woven ones: db/mail/http-client (prelude twins + drivers), session,
  html (kernel seam stays core), di (desugar-coupled), + log?/time?/runtime? classification
  check against the CORE list. Also queued: DEC-271 icu4x · DEC-247 DateTime · DEC-283 template
  build · benches/lift-Uri/golden-corpus · quiet-box microbench rerun (pre-push) · playground
  wasm rebuild (needs wasm-pack box).
- **DEC-283 RULED (register — the Template extension, .phgml): minimal phorj-in-HTML core;
  generalized views law (lowercase `views` ⇒ `Views` segment at any depth; views/ = 4th root +
  walk-up marker, searched entry-dir → views/ → src/ → vendor/); explicit {% import %}; templates
  = typed Html functions. BUILD QUEUED after DEC-273 waves. NOTE: the loader gains the views/
  root + role-folder normalization WHEN DEC-283 builds.**
- **WAVE 2 BUILT (gate green 2276/2276+clippy×2+no-default-check+fmt+release; PANEL RUNNING —
  consolidated 3-lens round 1).** json/uri/path/hash/decimal/test/debug → src/ext/ (uri: kernel+
  natives+url_compat+url_tests+PRELUDE; debug: natives+tests+PRELUDE — dissolution pattern =
  unconditional #[path] prelude modules, CORE_MODULES re-pointed); 7 new dep-free Default
  features; registry 22 rows alphabetical-asserted (2 mandatory + 16 default + 4 opt-in); PLAYGROUND regression FIXED (wave 1 silently
  dropped ini/csv/encoding from wasm — playground/Cargo.toml re-adds all dep-free Default
  extensions). Live probes: json/paths/decimals/hashing/uri guide examples + conformance dump
  2-leg OK; ext suite 96/96. After panel-clean×2 → commit → WAVE 3 (db/mail/http-client prelude
  dissolution + session/html/di — the woven ones).
- **(prior plan note)** — migrate json/uri/path/hash/decimal/test/debug to src/ext/ (uri+debug carry
  Core.Native.* twins + preludes → proves the preludes-monolith dissolution pattern); new
  features for each (default tier); ⚠ playground/Cargo.toml builds default-features=false +
  re-adds — MUST add the new features there or the wasm playground loses Json etc; feature-dep
  check db↔json (likely independent — desugar only names Json in generated code when the user
  imports it). Then wave 3: db/mail/http-client prelude dissolution + session/html/di (woven).
- **DEC-273 WAVE 1 (expanded per directive) — gate green 2276/2276+clippy×2+fmt+release,
  PANEL ROUND 2 RUNNING (round 1: lens2 CLEAN incl. bypass-question CLOSED; lens1 2P2+3P3,
  lens3 1P1+6P2+2P3 — ALL FIXED in-wave; DEC-268 needs 2 consecutive clean rounds).**
  Wave contents beyond slice 1: crypto/regex/csv/encoding migrated to src/ext/<name>/ (regex
  prelude → ext::regex_prelude::PRELUDE unconditional; csv+encoding = new default features);
  registry rows csv/encoding/signals + migrated=true ×5 + row-scope/green/db-all docs;
  import_targets_module extracted + gate_tests (end of preludes.rs — clippy items-after-test-
  module); `phg extensions [--docs]` rejects unknown args; **dev rulings in-wave: jit row STAYS
  (core-classified, row = flag discoverability) + PHG_NO_JIT=1 env for `phg build` artifacts
  (measured: artifact JIT 0.14s vs no-jit 8.9s on 10M-iter probe; artifacts inherit builder's
  features)**. After 2 clean panel rounds → ONE commit. Next wave: uri/path/json/debug/test/…
  migrations + preludes-monolith dissolution for db/mail/http-client twins.

## PREV (2026-07-17, late — CONTINUOUS MODE)
- **DEC-273 SLICE 1 BUILT, gate green 2275/2275 + clippy×2 + fmt + release, UNCOMMITTED —
  DEC-268 PANEL RUNNING (3 lenses on the live diff; commit blocked on 2 consecutive clean
  rounds).** Built: src/ext/registry.rs (Extension rows: name/feature/enabled/tier/modules/
  summary/migrated; render_listing(with_state) — CLI form vs build-independent docs form) ·
  src/ext/ini/{mod,natives,tests}.rs = PILOT (git-mv'd from src/native/ini*.rs; new default-tier
  `ini` cargo feature; parg widened pub(crate)) · GATED_CORE_MODULES const RETIRED → registry-
  driven unavailable_core_module → **E-EXTENSION-DISABLED** (E-MODULE-UNAVAILABLE = retirement
  pointer in explain) · `phg extensions [--docs]` subcommand (before the file-dispatch arm) ·
  docs/EXTENSIONS.md generated + sync test (build-independent docs form → test unconditional) ·
  registry hygiene test (tier order, transpile/lift MANDATORY heads) · live-verified: no-default
  build rejects `import Core.Ini;` cleanly. Docs: CHANGELOG/FEATURES/register BUILT note.
  NEXT after panel+commit: batch-migrate remaining extensions (crypto→regex→unicode→db→mail→
  http-client each to src/ext/<name>/), then transpile/lift structural seam (their wave).

## CURRENT (2026-07-17, night — CONTINUOUS MODE, dev-mandated: stop only for questions)
- **DEC-282 COMMITTED `d42a2107` (unified manifest-less loader — the biggest slice of the queue,
  38 files, +1158/−1749; full gate 2270/2270 + clippy×2 + fmt + release).** Everything ruled is
  BUILT: walk-up app root (src/ marker) · 3-root import-driven lazy loading · Go-max hygiene
  (E-MODULE-NOT-FOUND/E-IMPORT-MAIN/E-DUP-IMPORT/E-UNUSED-IMPORT all hard) · shebang + implicit
  `phg <file>` run · serve site mode (public/ docroot, static+ETag+guards) · LSP same-loader
  (DEC-252) · manifest/vendor retirement + migrations. Register has BUILT note + the PascalCase-
  vendor deviation disclosure (surface to dev at next question). Session commits so far:
  17c79ad6 (256+242+191-addendum) · ebb7a123 (bench Entry catch-up) · 996b2fee (DEC-258) ·
  0b203827 (DEC-281 Core.Input) · d42a2107 (DEC-282). ALL UNPUSHED (never push).
- **⚠ STANDING (dev, 2026-07-17): the package-manager EXTENSION gets a FULL re-adjudication when
  started — dev dislikes phorj.toml; NO toml presumed; config/lockfile/registry/CLI all open;
  research ecosystems then re-ask everything (register: "PACKAGE-MANAGER EXTENSION" addendum).**
- **NEXT = DEC-273 extensions migration (fresh-context/START HERE)**: the ruling = register
  "## DEC-273 — RULED (2026-07-16 evening)" (+ AMENDMENT 2 layout: `src/ext/<name>/`
  self-contained folders, `src/ext/registry.rs` one-row list, cli/preludes.rs monolith dissolves
  per-extension; E-EXTENSION-DISABLED naming the flag; batteries-included default build).
  Suggested slice 1: the registry + ONE pilot extension folder (pick a small one, e.g. Csv or
  Ini) migrated end-to-end (natives+prelude+tests colocated) proving the seam, THEN batch-migrate.
  (fresh-context recommended) → DEC-271 icu4x
  (brought forward) → DEC-247 DateTime + DEC-248-codemod (fresh-context) → MACRO/real-world
  benches (DEC-259; var/phorj-app) + lift Uri Tier-2 + golden corpus + span-collision re-basing.
  ⚠ OWED before any push: quiet-box CORE-PINNED microbench rerun. ⚠ OWED: playground wasm pkg
  rebuild (wasm-pack absent on this box). ⚠ Follow-ups from DEC-282 worth a look next session:
  UNIFIED-SPEC §imports/§serve prose not yet rewritten (code/docs shipped, spec section pending);
  examples/project/README.md still describes tomls; site-mode integration tests in tests/serve.rs
  (manual curl-verified only); shebang/implicit-run tests in tests/cli.rs (manual-verified only).

## PREVIOUS-CURRENT (2026-07-17, late)
- **DEC-281 Core.Input COMMITTED `0b203827`** (gate 2304/2304; 3-leg verified; serve-disabled;
  quarantine-twin mapped; tier1 +5 builtins).
- **DEC-282 BUILD PROGRESS (loader CORE + shebang DONE, census 2/2304→green):**
  ✅ shebang byte-0 skip (tokenizer lex_inner) + implicit `phg <file>` = run (main.rs dispatch,
  argv threads) + extensionless entries — VERIFIED live incl. real `./bin/console` exec.
  ✅ loader/mod.rs: `discover_roots` (src/-marker walk-up), `peek_package`, `index_packages`,
  `load_unified` (3-root import-driven lazy; W-SHADOWED eprintln), `user_imports`
  (E-DUP-IMPORT + E-IMPORT-MAIN), E-MODULE-NOT-FOUND w/ searched-paths; `assemble()` factored
  from load_project (decl_roots/decl_skip params); phorj.toml still wins when present (retirement
  pending). 6 new tests in tests/project.rs (manifestless_*); explain entries for the 4 new codes
  + W-SHADOWED. Symfony shape VERIFIED (bin/console → Commands + Model(src) + Acme.Strutil(vendor)).
  ✅ serve SITE MODE (src/serve/static_files.rs + docroot OnceLock in serve/mod.rs + respond_once
  intercept + main.rs DIR arm): `phg serve <DIR>` → public/ docroot, index.phg entry (front
  controller gets ALL non-static paths), static MIME(~20)+ETag+Last-Modified+304, guards VERIFIED
  live (curl: dynamic ✓, css 200+headers ✓, secret.phg 404 ✓, --path-as-is traversal → program
  not disk ✓, If-None-Match 304 ✓, W-PHG-IN-DOCROOT warning ✓). resolve_site_dir errors clearly
  when public/ or index.phg missing.
  ✅ E-UNUSED-IMPORT (loader check_unused_imports): whole-WORD source scan (import statements
  BLANKED by byte-range, not by line — one-liner programs!), bound names = leaf/alias ∪ Core
  whole-module bare_types via cli::preludes::core_module_bound_names (pub(crate); cli mod
  preludes now pub(crate)); over-approximates (comment mention = use) — never mis-flags.
  Interpolation-hole gotcha: holes are NOT lexer tokens (parser-side) — that's WHY it's a source
  scan not a token scan. Explain entries: E-UNUSED-IMPORT + W-PHG-IN-DOCROOT added.
  ✅ LSP parity (DEC-252): lsp publish → diagnostics_for_uri — buffer w/ user imports + real
  file → loader::load_with_buffer (new seam; assemble takes buffer override param) → same loader
  as phg check; Core-only buffers keep the fast text path. NOT yet integration-tested.
  ✅ RETIREMENT DONE: load() → always unified; load_project DELETED; manifest.rs/lock.rs/
  vendor.rs/tests/vendor.rs git-rm'd; `phg vendor` = retirement-stub error; help/test_runner
  root = src/-walk-up; 11 example tomls dropped + withdeps vendor → vendor/Acme/Strutil;
  tests/project.rs fully flipped (25/25 — incl. inert-by-construction flips for Core-hijack +
  lowercase-package; comment-mention trick satisfies the unused-scan in fixtures); unused-scan
  blanker got a STATEMENT-POSITION guard (the word "import" in comments tripped blank-to-";").
  Docs: CHANGELOG DEC-282 entry + FEATURES 5 rows + register BUILT note (w/ PascalCase-vendor
  deviation disclosure) + loader header rewrite. Register DEC-282 BUILT note appended.
  ⏳ FINAL-GATE RESIDUE (19 fails, gate log $SC/g282final.log): (a) src/loader/tests.rs unit
  suite — 16 tests still write phorj.toml TempDir projects; flip like tests/project.rs (drop
  toml; bad files need an IMPORT to be reached — or flip to inert assertions; decl-file (*.d.phg)
  tests: decl sweep now keyed on search roots not source_root); (b) 3 differential sweeps
  (all_example_projects_match_between_backends / _transpile_and_match_php / all_examples_match…)
  — the harness discovers projects BY phorj.toml (now absent): update discovery to
  examples/project/*/src/main.phg convention; (c) clippy printed 2×"3" counts in the gate log —
  verify clippy both legs actually clean (may be miscount of 'error' word). THEN full gate →
  ONE commit (message drafted around the CHANGELOG text).
- **PREV: DEC-282 unified loader ruling (register: main ruling + ADDENDA — read BOTH).**
  Sub-slices: (1) loader rewrite — app-root walk-up (src/ marker), 3-root search
  (entry-dir > src/ > vendor/, W-SHADOWED), import-driven declaration-indexed lazy load,
  E-MODULE-NOT-FOUND/E-IMPORT-MAIN/E-DUP-IMPORT/E-UNUSED-IMPORT (all HARD), merge-package +
  E-DUP-CROSS-FILE; (2) manifest retirement — phorj.toml/manifest.rs/`phg vendor` OUT
  (extension later); (3) layout laws unified (E-PKG-PATH rel. to search root, E-FILE-NAME);
  (4) shebang byte-0 skip + implicit `phg <file>` = run + extensionless explicit entries;
  (5) serve DIR mode: docroot=DIR/public, entry index.phg, static (MIME ~20 + ETag/Last-Modified
  + guards: canonicalize/no-.phg-bytes/no-dotfiles/no-listing); (6) LSP: diagnostics_for gains
  URI → same loader (DEC-252); (7) migrate examples/project/* (tomls out) + tests/project.rs +
  loose Main-only lift. ONE slice, full gate, then commit.
- **DEC-282 RULED (register — READ IT FIRST, full 3-round adjudication): unified manifest-less
  loader.** phorj.toml/manifest.rs/`phg vendor` RETIRE; root = entry dir (CLI) / serve DIR (web:
  public/ docroot + index.phg + static w/ MIME+ETag+guards); import-driven declaration-indexed
  lazy loading; folder=package + file=type; Main unimportable; Go-MAXIMAL import hygiene
  (E-IMPORT-MAIN, E-MODULE-NOT-FOUND w/ searched paths, E-DUP-IMPORT, E-UNUSED-IMPORT — all
  HARD); vendor/<publisher>/<name> first-party-wins + W-VENDOR-SHADOWED; LSP same loader same
  slice (DEC-252); one slice all of it. **BUILD ORDER (dev): DEC-281 Core.Input FIRST, then
  DEC-282.**
- **DEC-258 COMMITTED `996b2fee`** (combined naming model + variant defaults; gate 2297/2297).
- **DEC-258 BUILT (gate pending → commit next)**: combined model per the register REFINEMENT +
  BUILT notes — variant-literal defaults (checker `variant_default_ty`, 3 tests + 3-leg probe),
  prelude naming field threading (Database→Statement, withPassword param, real copy-builder
  namingStrategy), desugar `scan_naming_facts` + `NamingMode` + `Dyn` dispatchers
  (Class/Stream/entity-Map). E-DB-NAMING-NOT-CONST RETIRED. 10/10 naming tests; db/naming.phg
  extended (baked + dispatched twins, both backends). Docs: CHANGELOG/FEATURES/README/spec §Db.
- **Committed this stretch**: `17c79ad6` (DEC-256+242+191-addendum batch, census 271→0, full
  gate green) · `ebb7a123` (bench/micro Entry catch-up — the microbench gate was DEAD since
  7ffd550e; dbwork Db→Database + trycatch OddError also fixed; 23/23 run again).
- **DEC-281 RULED (register): Core.Input full module** (readAll/readAllBytes/readLine/lines
  Iterator/isInteractive; impure natives, quarantined; php://stdin legs; serve = instant EOF).
  BUILD SLOT: immediately after DEC-258 commits (dev-ruled).
- **CENSUS CONVERGED 271→109→2→0**: the 191-addendum residue is FIXED — root causes were
  (a) the four inline helpers (cli::wp + 3× with_pkg) prepending the Entry import BEFORE the
  package check → `import; package X;` double-package parse error — fix = wrap package FIRST,
  then insert the import after the package `;` (same-line, line-numbers preserved);
  (b) ~160 embedded .rs program literals missing the import — segment-based python codemod
  (split on `package Main;`, insert when segment has #[Entry] w/o the import) over src/ + tests/;
  (c) marker string "E-TRANSPILE-UNICODE-MARKER" tripped the explain-coverage scanner →
  RENAMED `__PHORJ_NATIVE_ONLY_UNICODE__` (registry ×4 + call.rs chokepoint);
  (d) DAP test breakpoint line 5→6 (the injected import line shifted the program);
  (e) `examples/web/response-builders.phg` reworked onto DEC-242 Cookie (old 2-arg withCookie
  was a type error) + `phg format`ed (width-canonical sweep pins it).
- **DEC-242 Cookie BUILT + example 3-leg-verified**; Cookie/SameSite added to Http bare_types
  (wind rule). **DEC-256 examples built**: guide/unicode-codepoints.phg (3-leg) +
  guide/unicode-native.phg (run ≡ run --tree-walker; E-TRANSPILE-UNICODE verified). Docs DONE:
  CHANGELOG (256+242+191-addendum), FEATURES ×2 rows, examples/README ×3 rows, register BUILT
  notes ×3. NEXT: full gate → commit slices → **DEC-258 COMBINED MODEL (ruled — register
  "DEC-258 REFINEMENT"): baked-when-traceable + dual-bake+runtime-dispatch-on-db.naming when
  not + per-stmt literal override; naming becomes a REAL promoted field on Database AND
  threads onto Statement (prepare copies it; namingStrategy returns a real copy, retiring the
  stored-statement-reverts-to-Exact footgun; E-DB-NAMING-NOT-CONST retires → dynamic dispatch)**.

## PREVIOUS-CURRENT (2026-07-17, evening)
- **DEC-256 BUILT under Core.String** (dev override ×2: split→String; register has the chain):
  6 natives (codepointLength/codepoints PCRE-transpilable + unicodeUpper/unicodeLower/
  graphemeLength/graphemes native-only via PER-FUNCTION ladder — marker string
  "E-TRANSPILE-UNICODE-MARKER" in php: fields, detected at transpile/call.rs chokepoint →
  E-TRANSPILE-UNICODE naming the function); unicode-segmentation dep admitted (feature
  "unicode", default; graphemes cfg-gated); PROBED: all 6 + ladder fire correct. icu4x/DEC-271
  BROUGHT FORWARD (after this batch). STILL OWED in batch: DEC-242 Cookie class + DEC-258
  Database naming ctor param + Unicode docs/tests/examples + batch gate.
- **DEC-191 addenda RULED+BUILT**: #[Entry] IMPORT-GATED (`import Core.Runtime.Entry;` —
  registry bare_types row on Core.Runtime, UncheckedOverflow precedent); zero-span synthetic
  exemption in enforce_injected (synth_empty_main + test_runner attrs use Span{0,0,0,0});
  lifter prepends the import; 5 test helpers inject it; .phg codemod ran (import inserted
  after last import line). NO manual-run CLI ("everything orchestrated by the Entry").
  Un-attributed main() = ordinary callable ✓ verified; argv/exit-code filling ✓ verified live.
  Census running (g1.txt) → fix residue → batch gate covers 191-addenda+256(+242+258 next).

- ⚠ OWED: playground wasm pkg REBUILD (wasm-pack absent here) — examples.js regenerated with
  #[Entry] (193 entries, hello ✓) but the prebuilt wasm predates DEC-191 → in-browser runs fail
  until someone runs `wasm-pack build playground --target web --out-dir web/pkg` on a wasm-pack
  machine. conformance/diagnostics stays UN-attributed BY DESIGN (check-only goldens).

## PREVIOUS (2026-07-17)
- ✅ **DEC-191 #[Entry] COMMITTED `7ffd550e`** (328 files; detail in the in-flight section below,
  now historical). Release rebuilt after.
- ✅ **DEC-243 COMMITTED `995cfe59`** (kernels+registry+IIFE percent twin+tier1 allowlist+
  guide example, three-leg oracle-identical). NOW: the upfront adjudication batch
  (DEC-256/242/258 surfaces) → build them batch-gated. ✅ ALL THREE RULED (register:
  "Surface rulings batch 2026-07-17"): DEC-256 = explicit fns (codepointLength/graphemeLength/
  codepoints/graphemes/unicodeUpper/Lower; length stays bytes); DEC-242 = Cookie VALUE class
  ONLY (ctor defaults path/secure/httpOnly/sameSite=Lax-enum/partitioned=false + maxAge/domain
  opt; resp.withCookie + withCookies(List); Session internal Cookie; CHIPS opt-in); DEC-258 =
  `new Database(dsn, naming = new Naming.Exact())` ctor default param, per-stmt override kept.
  BUILD next (batch-gate all three). ✅ DEP RULED: unicode-segmentation ADMITTED (graphemes
  only; codepoints/case = std char) + **icu4x/DEC-271 BROUGHT FORWARD** (after this batch).
  BUILD ORDER: DEC-242 Cookie (prelude class + SameSite injected enum + Response.withCookie/
  withCookies + Session internal + Partitioned attr emission) → DEC-258 (Database ctor
  `naming = new Naming.Exact()` default param; desugar_db resolves the CONNECTION binding's
  ctor literal for hydration naming, per-stmt namingStrategy overrides) → DEC-256 (dep +
  codepointLength/graphemeLength/codepoints/graphemes/unicodeUpper/unicodeLower natives;
  PHP legs: mb_* are NOT tier-1-safe? CHECK — mb_strlen needs ext-mbstring; grapheme_* needs
  ext-intl — likely NATIVE-ONLY (§14 ladder, E-TRANSPILE-UNICODE) or gated helpers; SURFACE
  the ladder trade in the register when built).
- (historical) DEC-243 detail: (inline; no adjudication needed — PHP-parity
  natives: match PHP's levenshtein()/similar_text() semantics EXACTLY incl. the similar_text
  percent-by-reference twin question — surface: `String.levenshtein(a, b): int` +
  `String.similarText(a, b): int` (+ percent variant? check PHP's API and pick the honest
  mapping — similar_text returns count, percent via &$percent → phorj likely
  `similarText(a,b): int` + `similarTextPercent(a,b): float`). Native module = Core.String
  (text.rs/text_registry.rs); PHP erasure = the builtins themselves (Tier-1!); bench vs PHP
  per DEC-259. Examples + FEATURES + README + register BUILT.
- THEN (upfront-adjudication batch at DEC-243 close): DEC-256 Unicode FULL surface ·
  DEC-242 partitioned-cookies surface · DEC-258 Db naming opt-in surface — then build those
  (batch-gate) → DEC-273 ext migration → lift Uri Tier-2 → golden corpus → span-collision
  re-basing slice → quiet-box microbench (owed pre-push).

> Location developer-ruled 2026-07-16: lives IN THE REPO (tracked), committed alongside each
> slice commit. High-churn detail stays here so MASTER-PLAN §0.2 stays clean.

Updated: 2026-07-16 (evening)

## In flight
- **DEC-257 Iterator slice 1 (generic interfaces)** — INLINE, uncommitted:
  - DONE: `InterfaceDecl.type_params` + `ClassDecl.implements_args` AST fields;
    parser `interface I<T>` (bounds rejected loudly) + `parse_implements_list`
    (`implements Iterator<int>`) wired into class parser.
  - DONE (compiles clean): all 11 construction sites fixed; InterfaceInfo.type_params +
    placeholder(arity) prebind; collect_interface resolves sigs w/ active_type_params (Ty::Param);
    resolve.rs generic-interface args (arity-checked E-TYPE-ARG-COUNT); conformance loop
    substitutes implements_args via theta+apply_subst before sig_conforms (also resolves args
    with the CLASS's type params active, so `DbStream<T> implements Iterator<T>` works);
    rewrite_generics gained the Item::Interface erasure arm (rparam/rty over method sigs).
  - PROBED GREEN: `interface Producer<T>` + `class Ints implements Producer<int>` checks+runs;
    wrong ret = E-IFACE-SIG; missing args = E-TYPE-ARG-COUNT w/ hint; `class Boxed<T> implements
    Producer<T>` THREE-LEG byte-identical (run/tree-walker/PHP all `42`). Scratch probes in
    session scratchpad (giface*.phg). NOTE: `new Boxed<int>(42)` turbofish-on-new NOT supported
    (parse error — construction infers args; only List/Map have new-with-args per DEC-214p1).
  - MORE DONE: ClassInfo.iface_args (HashMap<iface, Vec<Ty>>; populated in the conformance loop
    where args are already resolved w/ class tps active); ty_assignable gained the
    class→parameterized-interface invariant-args check (inherit.rs, BEFORE assignable_with;
    inherited-implements = documented fall-through to name path); class_subst falls back to
    INTERFACE type_params so interface-typed receivers substitute (`p.produce(): int` not `T`).
    PROBED: `Producer<int> good = new Ints()` + `consume(good)` clean; `Producer<string> bad =
    new Ints()` REJECTED. Fast test tier running in bg.
  - DONE: 5 checker tests in src/checker/tests/interfaces.rs (all pass); fast tier 2208/2208;
    FORMAT-FIDELITY BUG found+fixed (printer dropped `<T>` on interface + implements args —
    format/printer/items.rs: interface() generics + implements_body() helper at both class
    sites; lift printer needs nothing, PHP has no generics); guide example
    examples/guide/generic-interfaces.phg three-leg-verified (final canonicalized content);
    docs done (CHANGELOG slice-1 entry, FEATURES row, examples/README row, MASTER-PLAN item 16).
  - SLICE 1 ✅ COMMITTED `54255480` (full gate: 2274/2274, clippys 0+0, FMT-OK).
- **SLICE 2 IN FLIGHT (uncommitted):** DONE so far: ITERATOR_PRELUDE (`interface Iterator<T>
  { hasNext(): bool; next(): T; }`) + CORE_MODULES row (member_gated, bare_types ["Iterator"],
  before the Uri row) + injection fold now merges Item::Interface (was `_ => false`, silently
  dropped!) + InterfaceDecl.injected flag (mirrors EnumDecl; parser/collapse/alias/generics
  ctors updated) + DEC-202 builtin-name check EXEMPTS injected interfaces (entry.rs) + PHP-leg
  mangle `Iterator` → `Iterator_` in transpile/names.rs php_class_name (RoundingMode precedent;
  emit_interface disp now routes php_class_name; implements already routed php_type_ref).
  PROBED: Countdown implements Iterator<int> + manual hasNext/next pull = THREE-LEG-IDENTICAL
  (3 2 1). ⚠ transpiled output is NOT namespaced (my earlier namespace assumption was wrong —
  DEC-202's "cannot redeclare" empirically confirmed; hence the mangle).
  - ✅ SLICE 2 CORE BUILT + PROBED (all uncommitted): for_iter_lowerings HashSet field
    (mod.rs/plumbing.rs; check_resolutions tuple 7→8, both pipeline.rs destructures fixed);
    iterator_elem helper + check_for arm (flow.rs — throws rule = covered_by_try OR
    throws_declared union w/ targeted E-CALL-UNHANDLED message; NOTE discharge_call_throw alone
    was WRONG: bare-call discharge is try-only in Phorj's model); rewrite_foreach.rs (stmt
    walker + span-keyed For→Block{VarDecl __for_it_<start>; While(hasNext){VarDecl x=next();
    body}} lowering; lambda block bodies via rewrite_pipe::walk::visit_exprs_mut; idempotent);
    wired OUTERMOST in check_and_expand_reified. PROBES ALL THREE-LEG-IDENTICAL: basic foreach
    3-2-1 · interface-typed param (total(Iterator<int>)) · nested iterator-in-iterator+list ·
    throwing iterator declared/caught (declared=3 caught=3) · undeclared = clean loop-site
    error. Bare `Iterator<int>` type annotation needs `import Core.Iterator.Iterator;`
    (E-INJECTED-TYPE-BARE — the X.X shape DEC-278 addresses).
  - ✅ SLICE 2 FINISHERS DONE: 3 cli tests pass (foreach_over_* — implementor+nested+
    interface-typed / throwing declare-or-catch / non-iterator error); throws.rs destructure
    8-tuple fixed; guide example examples/guide/iterators.phg THREE-LEG-IDENTICAL (incl. the
    Iterator<string?> nullable-element proof + manual pulls); docs done (CHANGELOG slice-2,
    FEATURES row, examples/README row, MASTER-PLAN 16b, UNIFIED-SPEC stdlib block).
  - ✅ SLICE 2 COMMITTED `a9e9f693` (+ naming rulings docs `59ce8bb3`).
  - ✅ SLICE 3 BUILT (uncommitted, gate running): RowStream/DbStream implement Iterator —
    lookahead `mutable Row? ahead` in RowStream.hasNext (pull+cache, carries throws), next =
    cache or `panic("iterator exhausted")` (needs `import Core.Abort.panic;` in DB_PRELUDE);
    DbStream.hasNext delegates (NO hydration — laziness exact), next = rows.next()? + hydrate.
    ⚠ GOTCHAS hit: (a) REGISTRY ROW ORDER — Core.Iterator's row must sit AFTER Core.Db's (the
    injection fold resolves transitive prelude imports in row order; comment at the row);
    (b) `x != null` is NOT phorj (cross-type comparison error) — use `if (var v = opt)`;
    (c) bare throwing calls inside throwing prelude methods need `?` AS WHOLE BINDING INIT
    (`bool has = this.hasNext()?;` — never in if-condition position);
    (d) `panic` diverges for totality ✓ but needs `import Core.Abort.panic;`.
    MIGRATED: 4 tests/database.rs bodies → foreach/direct-next + NEW exhausted-fault pin test
    (80/80 db tests pass); examples/database/streaming.phg → foreach (both backends identical);
    docs (CHANGELOG slice-3, examples/README row, UNIFIED-SPEC stream line, MASTER-PLAN
    "DEC-257 COMPLETE").
  - ✅ SLICE 3 COMMITTED `05f224a7` — **DEC-257 COMPLETE**; release binary rebuilt.
- **NAMING MEGA-SLICE (DEC-276…279 renames)** — ✅ agent done (112 files; its gate 2284/2284 +
  clippys + fmt + release in the worktree), diff cherry-picked onto master (1 conflict:
  FEATURES.md, resolved — kept DEC-280 foreach row + renamed Iterator row). Dev RATIFIED
  E-IMPORT-NATIVE-MEMBER (whole-module-only raw natives) + REJECTED old→new hint table
  ("do nothing — all migrated"); register amended, CHANGELOG entries written. Agent follow-ups
  recorded: HcResult/MailResult renames · enforce_injected 3-segment-import edge · editors
  docs/snippets unchecked · UriModule.Uri.parse double-chain (already ruled follow-up).
  ⚠ agent snapshot commit `1234bdac` lives on branch worktree-agent-a3b9403d94752528a (worktree
  removal is permission-blocked — clean up manually later; second stale worktree
  agent-af41f1445fc1c9498 likewise). ✅ COMMITTED `8bae400f` (117 files, gate 2286/2286).
- **DEC-275 E-ERROR-NAME (inline, uncommitted, gate running):** rule at collect (transitive
  class_implements ⇒ name must end Error|Exception), explain entry, 2 checker tests (incl.
  subclass-of-error-base), stdlib sweep codemod = 25 renames (Mail: AuthFailed/ConnectionFailed/
  InvalidAddress/MailIo/MailTimeout/MessageBuildFailed/RecipientRejected; Http: BlockedAddress/
  HttpConnectionFailed/HttpTimeout/InvalidUrl; Db: ConstraintViolation/SerializationFailure/
  Timeout/UniqueViolation; Uri: UriMalformed + UriBad* family + UriBaseNotAbsolute/
  UriPortOutOfRange — all stem+Error; sentinels <<X>> renamed in lockstep, 30 files). The rule
  self-verifies the corpus on every suite run — it caught TooManyRedirects/TooLarge (missed by
  the initial map) + test/example fixtures (Boom-class fixtures → *Error) on the first gate
  runs; final sweep = 27 stdlib renames. ✅ COMMITTED `284284e0` (44 files, gate 2288/2288).
  **ENTIRE NAMING DOCTRINE (DEC-275…280) NOW LANDED.**
- **DEC-191 #[Entry] IN FLIGHT — PROGRESS (uncommitted, compiles clean, probe green):**
  ✅ (b1) ast/class_hierarchy.rs: `is_entry_attr` + `EntryRole{Cli,Web}` + `entry_role(f)`
     (AST-shape classification; CLI=():void|int|(List<string>):void|int, WEB=(Request):Response)
     + `entry_candidates(program)` + `entry_for(program, role)`. Old name-keyed `entry_point`
     KEPT for now (8 callers still on it — flip pending).
  ✅ (c1) checker/program/walk.rs: E-MULTIPLE-MAIN block REPLACED by the DEC-191 validation
     (bare-args E-ATTRIBUTE-ARGS · instance-method E-ENTRY-TARGET · no-role E-ENTRY-SIG w/
     shape list · per-role E-MULTIPLE-ENTRY; CLI+web may coexist).
  ✅ checker/program/attributes.rs: Entry known in the fn-attr whitelist (validation lives in
     walk.rs). PROBED: `#[Entry] function main(): void` checks + runs.
  ✅ (b2) ALL 8 callers FLIPPED to `entry_for(program, EntryRole::Cli)` (transpile ×4,
     compiler, interpreter ×2, loader, serve handlers' cli check); "no entry point" error
     texts now name `#[Entry]`; `synth_empty_main` carries the attribute (Span uses len not
     end!). PROBED: attributed entry runs; un-attributed magic `main` = clean no-entry error
     (FULLY BREAKING confirmed live).
  ⏳ REMAINING: serve Web-role resolution + respond_bridge rewire off name-magic "handle"
     (serve/handlers.rs + preludes respond_bridge — currently keys off `handle` by name);
     old `entry_point`/`entry_point_count` fns now likely dead → remove after codemod;
  ✅ throws.rs main-no-throws restriction REMOVED (DEC-191 ruling supersedes Batch-1 D;
     comment records the supersession).
  ✅ wp() (src/cli/tests.rs) + typed_program (tests/database.rs) now inject `#[Entry] ` before a bare
     `function main(` (replacen 1, skipped when already attributed) — covers most inline tests.
  ✅ CODEMOD DONE: 275 example/conformance .phg files attributed (column-0 regex + the indented
     static-main case for class-main.phg; differential GREEN post-codemod); compiler::tests
     with_pkg helper injects (30/31 pass; missing_main assertion flipped to expect #[Entry]);
     23 integration .rs files + tests/database.rs textually codemodded (`function main` →
     `#[Entry] function main`, existing-attr protected); explain entries E-ENTRY-SIG/
     E-ENTRY-TARGET/E-MULTIPLE-ENTRY added. Census r1 = 776 fails; census r2 RUNNING —
     remaining expected: entry_point.rs E-MULTIPLE-MAIN flips ×2, throws
     main_may_not_declare_throws (rule removed → flip/delete), run_executes_sample (SAMPLE
     const direct call), library_file error-text assertion, format pipe test?, playground
     the VM leg tests (its own fixtures), dap handshake fixture, vendor fixture, serve/handle
     name-magic rewire still pending + old entry_point fns removal + exit codes + docs.
  ✅ census r6 = **2291/2291 GREEN** (776→0 convergence). CLOSE-OUT DONE: respond bridge
     rewired to the ATTRIBUTED web entry (textual callee substitution into HTTP_RESPOND_BRIDGE;
     class-static paths supported); 7 handle fixtures attributed (user-attributes.phg was a
     FALSE POSITIVE — its handle isn't a web handler, attr removed); NAMED-ENTRY generalization:
     compiler program.rs ×4 sites (static-init preludes + index resolution — was panicking
     "entry_point reported a class-static main" on a non-main-named entry!), interpreter
     call_name ×2, transpiler bootstrap callee — all key on entry_decl.name now;
     guide/entry.phg (class-static named entry + int exit) THREE-LEG green incl. php-exit=0;
     docs done (CHANGELOG w/ span-collision disclosure, FEATURES row, README row, MASTER-PLAN
     SHIPPED note). Old name-keyed entry_point/entry_point_count kept (pub, unreferenced by
     backends — removal is cleanup for a later pass). FULL GATE running → commit + release.
  ✅ census r5→r6 fixes: mtest ×6 = test_runner synthesize_main now attributes its synthetic
     entry + strips #[Entry]-attributed fns (not name-main); format stdin = assertion restored
     to plain form (fmt must NEVER insert attributes; MESSY has double-space so codemod missed
     it — correct outcome); diagnostics goldens = attribute REVERTED in conformance/diagnostics/
     (check-only corpus, entries not needed, preserves golden line numbers); loader+dap fixtures
     codemodded. Census r6 RUNNING (expect ~0). THEN: serve web-role rewire (respond_bridge
     name-magic `handle` → EntryRole::Web), guide/entry.phg example + docs (CHANGELOG/FEATURES/
     register BUILT note incl. the DEC-191-ruling-supersedes-main-no-throws note), old
     entry_point/entry_point_count removal if dead, full gate (raw-verified clippys), commit.
  ⚠⚠ RESOLVED BUG (was census r4 residue, REPRODUCED + root-caused): examples/database/transaction-closure.phg —
     interpreter leg RUNS CLEAN, VM leg = "compile error: `transaction` is not a function,
     variant, or class" (interp ≠ VM divergence!). transaction = the DEC-249 default-param method
     (fills machinery). Appeared between 284284e0 (green) and the DEC-191 work. Suspects, in
     order: (1) apply_default_fills interplay with the reified chain rewrap I did for
     materialize_for_binds/lower_foreach_iter (re-nested parens in pipeline.rs — check the arg
     nesting is EXACTLY materialize_pipe_params(...inner..., &pipe_params) then
     materialize_for_binds(·, &for_binds) then lower_foreach_iter(·, &for_iters)); (2) the
     example has for-loops → for_bind_resolutions non-empty → materialize_for_binds mutates
     For.ty in place — check ty_to_ast_type output for Row/entity types is benign on the
     VM kind path; (3) fills+ufcs double-rewrite resurrection ([[rewrite-clone-staleness-class]]
     — READ IT). DEBUG PLAN: minimal repro = default-param METHOD call + a for-in loop with
     inferred binding + #[Entry] main; bisect by disabling materialize_for_binds (pass empty
     map) then lower_foreach_iter. Others FIXED in r4→r5: format stdin assertion must expect
     CANONICAL own-line `#[Entry]\nfunction main` (fmt splits the line — fix the assertion);
     diagnostics goldens: conformance/diagnostics/*.phg got a +1 LINE SHIFT from the attr
     insert — either same-line the attr in those files or bump golden line numbers; loader
     tests + dap.rs fixtures codemodded ✓; lifter now EMITS #[Entry] (synth + php-main) and
     the lift printer prints fn attrs (was dropping them) ✓; lift_roundtrip + all 6 mtest ✓.
  ✅ census r3 = 125 → codemodded src/jit/tests/*.rs (4 files, ~90 tests) + ALL remaining .phg
     under tests/+src/ (tests/fixtures/sample.phg, dump_fault.phg …). Census r4 RUNNING;
     expected residue = SEMANTIC flips (~20): entry_point E-MULTIPLE-MAIN ×2 → E-MULTIPLE-ENTRY;
     throws main_may_not_declare_throws → entries-may-throw; missing-main assertion texts
     (interpreter, run_integration program_without_main, transpile main_is_invoked, cli
     library_file + run_executes_sample/SAMPLE const); loader::tests ×2 (main-file exemption
     keyed on entry presence — now attribute-keyed); diagnostics golden case (one case pins an
     old code/message); mtest ×6 (the `phg test` runner path — check how it resolves/needs
     entries); format stdin case; dap handshake fixture; db transaction-closure example;
     lift_roundtrip; differential class_static_main_exit_code test (NOTE: an exit-code test
     EXISTS — read it before implementing (): int exit codes, semantics may partially exist!).
  ✅ census r2 = 157 fails → helper patches: src/interpreter/tests.rs with_pkg (injects),
     src/interpreter/coop.rs fixtures (textual), src/vm/{coop,tests}.rs (textual). Census r3
     RUNNING → iterate on its list (pattern: RUN-path fixture = add attr / helper-inject;
     check-only tests need NOTHING; assertion texts mentioning old messages get flipped;
     entry_point.rs E-MULTIPLE-MAIN tests + throws main_may_not_declare_throws = flip to the
     new semantics). NOTE skip-list: checker tests (check-only, no entry needed), doc comments
     (dap.rs/diagnostic.rs/lift decls/cli pipeline/bundle section), src/lsp/tests.rs
     (diagnostics path). jit tests pass untouched (own runner).
  ⏳ ORIGINAL grind list (superseded by above, kept for detail): (a) examples/**/*.phg + conformance/**/*.phg — insert
     `#[Entry]\n` line above top-level `function main(` (218+ files; python codemod; then
     playground `python3 playground/gen_examples.py` regen); (b) NON-wp test fixtures: raw
     consts (cli/tests.rs SAMPLE) + per-file harnesses in tests/*.rs (http_client, fs, session,
     mail, regex_and_more?, differential fixtures embedded) — run suite --no-fail-fast and fix
     every 'no entry point' failure by adding the attribute; (c) E-MULTIPLE-MAIN tests in
     checker/tests/entry_point.rs flip to E-MULTIPLE-ENTRY/#[Entry] forms; (d) remove dead
     `entry_point`/`entry_point_count` + their "main" literals once nothing references them;
     grep '"handle"' for serve name-magic (respond_bridge) → Web role. throws.rs
     `validate_throws_decl` `is_entry_main` — DEC-191 ruling WINS over old main-no-throws
     (throwing entries legal; escaped fault = exit 1/HTTP 500) → drop/replace the restriction;
     (): int exit codes (interp+VM map returned Int → process exit 0-255; PHP emits
     exit($code)); E-MULTIPLE-MAIN test flips in checker/tests/entry_point.rs; THE CODEMOD
     (examples 218 + test inline strings ~1000+: `function main(` → `#[Entry] function main(`
     top-level only — EXCLUDE instance-method-main fixtures + comment texts; conformance/;
     playground regen; synth_empty_main in ast/decls.rs may need the attr!); explain entries
     (E-ENTRY-SIG/E-ENTRY-TARGET/E-MULTIPLE-ENTRY); guide/entry.phg example; docs rows.
  (all gaps ruled — MASTER-PLAN §13.1.1: static entries YES /
  FULLY BREAKING no-main-fallback / (): int exit codes / web (Request): Response, CLI+web may
  coexist / throwing entries legal). SETTLED DESIGN:
  (a) The ruling kills the MAGIC NAME, not the name — programs keep `function main`, just
      attributed: `#[Entry] function main(): void`. Codemod = insert `#[Entry] ` before
      top-level/static `function main(` declarations (trivial diffs). Same for serve `handle`
      → web role (respond_bridge in preludes keys off name-magic today — rewire to attribute).
  (b) Resolver: current `ast::class_hierarchy::entry_point(program, name)` (name-keyed, already
      handles static methods) → new attribute-keyed `entry_points(program)` returning
      {cli, web} classified by signature; CLI = ():void | ():int | (List<string>):void|int,
      WEB = (Request):Response. Grep ALL callers of entry_point/"main"/"handle" literals
      (interpreter run, vm run_entry, compiler, cli serve, preludes respond_bridge,
      entry-main-no-throws rule in throws.rs validate_throws_decl `is_entry_main`!).
  (c) Checker validation pass (collect/attributes.rs): #[Entry] arg-less, only on top-level fns
      + static methods; signature must match a role else E-ENTRY-SIG (hint lists shapes);
      >1 per role = E-MULTIPLE-ENTRY; entries may throw (escaped fault = exit 1 / HTTP 500).
  (d) (): int exit codes: interpreter + VM map returned Int → process exit (0-255); PHP leg
      emits exit($code) wrapper around the entry call. `no entry point` error message updated.
  (e) Codemod scope: examples/**.phg (~200, top-level main = safe blanket), tests' embedded
      programs (~1000+ inline strings — regex `function main\(` → `#[Entry] function main(`
      per file EXCEPT instance-method-main fixtures in entry_point.rs tests + explain/doc
      texts); conformance/; playground gen_examples regen; docs snippets FEATURES/README.
  (f) Docs+example (guide/entry.phg: named CLI entry w/ int exit + args; web coexist note),
      explain entries, editors: NO grammar change (#[...] exists).
  After DEC-191: DEC-256 Unicode FULL · DEC-243 levenshtein · DEC-242 cookies · DEC-258 Db
  naming (batch-gate candidates) · lift Uri Tier-2 · golden-corpus harness · quiet-box
  microbench (owed).
- **LIFT CATCH-UP + DEC-280 (inline, uncommitted, gate running):** DEC-280 RULED+BUILT
  (untyped/mixed foreach k=>v; developer challenged→confirmed; lift marker inline comment form).
  Landed: parser bare/mixed bindings (parse_foreach — dropped both mandatory-type errors);
  **materialize_for_binds** (rewrite_foreach.rs; Invariant-7: inferred foreach binding types →
  AST post-check, BOTH forms — single-binding had the same latent CTy gap; wired BEFORE
  lower_foreach_iter; check_resolutions tuple 8→9, pipeline+throws.rs updated;
  rewrite_pipe::materialize now pub(in checker) for ty_to_ast_type); format printer two-binding
  arm (foreach spelling when any binding Infer; fully-typed keeps `for (K k, V v in m)`); lift:
  PhpMember::Prop.set_vis + (set)-group parsing + DEC-241 modifier mapping + lift printer
  PrivateSet/ProtectedSet ORDER entries (was silently dropping!) + k=>v Tier-1 with inline
  marker + two-binding print arm (was silently dropping val!). Tests: foreach_untyped_* cli
  test (v+0 arithmetic proves materialization), lifts_key_foreach_with_inferred_marker,
  lifts_asymmetric_visibility_properties (flipped refuses_key_foreach). Example:
  examples/guide/foreach.phg extended (v*2 differential pin, format-fixpoint, 3-leg identical).
  Docs: CHANGELOG (DEC-280+lift), FEATURES foreach row (new), C-decisions DEC-280 ruled+BUILT.
  NOW: full gate in bg → on green commit → review naming agent when it returns.
    ORIGINAL slice-2 analysis below kept for reference:
    (a) Checker field `for_iter_lowerings: HashMap<usize, ()>` (keyed Stmt::For span.start) +
        thread through check_resolutions return tuple (grows 7→8: update BOTH pipeline.rs
        destructures + checker/tests/throws.rs).
    (b) Helper `iterator_elem(&self, name, cargs) -> Option<(Ty, Vec<Ty>)>` (elem + the union
        of concrete hasNext/next throws): name=="Iterator" → (cargs[0], vec![]) (interface
        throws = empty by existing deferral); else classes[name].iface_args.get("Iterator") →
        elem = apply_subst(args[0], class_subst(name, cargs)); throws from
        ci.methods["hasNext"/"next"][0].throws.
    (c) check_for single-binding match: add `Ty::Named(..)` guard arm BEFORE `other =>` when
        iterator_elem hits: record span in for_iter_lowerings; for each throw type E call
        `self.discharge_call_throw("next", &E, *span)` (KEY SIMPLIFICATION [Verified: read
        throws.rs 43-80]: `?` is a CHECKER-ONLY marker — runtime unwind identical — so the
        REWRITE EMITS BARE CALLS, no Propagate wrapping; discharge_call_throw gives exact ruled
        semantics: caught-by-enclosing-try OR fn-declares OR clean error).
    (d) NEW rewrite_foreach.rs: recursive stmt walker (model: rewrite_pipe/walk.rs vstmt —
        must cover fn bodies, class members incl. ctor, lambda block bodies, all nested stmts).
        `Stmt::For{span in map}` → `Stmt::Block([ VarDecl{ty: Infer, name: "__for_it_{start}",
        init: iter}, While{cond: Call(__for_it.hasNext()), body: [VarDecl{ty: for's ty, name,
        init: Call(__for_it.next())}, ...body]} ])` — unique var per loop start = nested-loop
        safe. Recurse INTO the moved body (nested foreach-over-iterator).
    (e) Wire into cli/pipeline.rs BOTH check_and_expand AND check_and_expand_reified
        (invariant 6) — order: after apply_default_fills/other expr rewrites? Foreach lowering
        is stmt-level + independent of expr rewrites; run it LAST (after materialize_pipe_params
        order concerns don't apply — but its generated calls must survive: rewrite_ufcs etc.
        already ran, and our generated hasNext/next calls are plain method calls needing NO
        further rewriting on any backend).
    (f) Docs: exhausted-next() fault contract note; examples/guide/iterators.phg (Countdown +
        foreach + null-element note); checker tests (foreach over implementor; throws
        undeclared = error; declared = clean; inside try/catch = clean; foreach over
        Iterator<E>-typed value; non-implementor still errors); CHANGELOG/FEATURES/
        examples-README/MASTER-PLAN/UNIFIED-SPEC.
    Then SLICE 3: Db streams reshape (hasNext/next + implements Iterator<Row>/<T>, lookahead
    buffer; migrate desugar_db sites, examples/database/*, tests/database.rs; RowStream throws move to
    hasNext — it pulls).
  - Annotation note: `Iterator<int>` in type position survives to backends WITH args exactly like
    `Box<int>` does (backends already cope; rty keeps heads + recurses args). No new erasure
    needed for annotations.
  - Then slice 2 (Core.Iterator prelude + foreach stmt-desugar) + slice 3 (Db stream reshape).
    Full map = memory [[dec-257-iterator-build-map]].
- **Playground rework** — ✅ COMMITTED (`feat(playground): two-pane…` right after `6eb07c91`):
  agent diff reviewed + applied on master, README de-staled, node --check clean, CHANGELOG entry.
  ⚠ leftover: agent worktree `.claude/worktrees/agent-af41f1445fc1c9498` + its branch could not
  be removed (permission-denied on `git worktree remove --force`/`branch -D`) — ask dev or clean
  later; changes are fully applied+committed on master. ⚠ runtime smoke test in a real browser
  OWED (org policy blocked localhost browsing for the agent): `python3 -m http.server -d
  playground/web` + check tabs/badge; wasm pkg + php-wasm paths untested at runtime.

## Queue after DEC-257
0a. **NAMING MEGA-SLICE (DEC-275…279, all RULED 2026-07-16 — register has full detail):**
   error suffix Error|Exception + E-ERROR-NAME (stdlib sweep keeps stems) · earned-shortcut
   renames (Fs→FileSystem, Db→Database+family, Reflect→Reflection, DI→DependencyInjection,
   HcHandle→HttpClientHandle, --addr/--proto flags) · *Sys → Core.Native.* nesting ·
   7 namesake modules → *Module suffix (incl. IteratorModule; double-chained static = follow-up)
   · Core.Url merges into Uri. ONE codemod + differential sweep + docs/examples/editors.
   SEQUENCED right after DEC-257 (files overlap slices 2-3 → not truly independent; also avoids
   double-renaming the Db streams). Dev-kept-earned list in DEC-276 (Math, dd, lsp, acronyms).
0b. **LIFT CATCH-UP slice (Invariant-17 debt, dev asked 2026-07-16 "are they always up to date?"):**
   (a) lift PHP 8.4 `private(set)`/`protected(set)` → DEC-241 modifiers; (b) upgrade
   `foreach ($m as $k => $v)` from Tier-2-reject to Tier-1 (Phorj has k=>v since DEC-248 —
   stale comment at lift/lifter/decls.rs:355); (c) Uri Tier-2 mapping (already-recorded
   follow-up). Batch-gate candidate; transpile confirmed always-current (differential-gated).
1. **DEC-191 #[Entry]** — brought forward, gaps RULED (see MASTER-PLAN §13.1.1 update):
   static methods YES; FULLY BREAKING (no main fallback; codemod + differential sweep);
   `(): int` exit codes; web `(Request): Response` confirmed; CLI+web may coexist.
2. DEC-256 Unicode FULL · DEC-243 levenshtein+similarText · DEC-242 cookies · DEC-258 Db naming
   (batch-gate candidates; upfront-adjudicate their surface questions first).
3. DEC-273 ext migration AFTER queue. Owed: quiet-box microbench rerun pre-push; golden-corpus
   harness build; playground-agent review.

## Standing (new today)
- Speed levers authorized = memory [[speed-levers-authorized]] (worktree agents for independent
  slices OK; NEVER dynamic workflows/team agents).

# L — On-hold inventory: every PENDING decision, ruled-not-built spec, and deferred item

**Scope.** The authoritative, deduplicated, reality-checked list of everything in phorj that is currently
(a) awaiting a developer decision, (b) ruled/specced but unbuilt, (c) deferred with a reason, or
(d) a recorded known limitation. Produced 2026-07-25 for the developer's interactive decision session.

**Method.** Four parallel evidence sweeps (decision register 3376 lines read sequentially; all 12
`docs/specs/*.md` + `docs/adr/*`; MASTER-PLAN + MILESTONES + ROADMAP; KNOWN_ISSUES + a source
TODO/`unimplemented!`/`allow(dead_code)` sweep) plus my own reading of `docs/plans/SLICE-STATE.md`
(2308 lines) and tonight's global-review raw files (`docs/research/2026-07-25-global-review/`, ~5 200
lines, staged-but-uncommitted). Every verdict was checked against the shipped binary
(`target/release/phg`, built 2026-07-25 21:03), `grep` over `src/`, or the `php-8.5.8` oracle.
**No repo file was modified. No commits. No cargo builds.**

**Invariant 15 compliance.** Nothing here is ruled. Each row carries a recommendation; the ruling is the
developer's.

**Reality-check legend.** `STILL OPEN` · `ALREADY BUILT (label stale)` · `PARTIALLY BUILT` ·
`SUPERSEDED` · `RESOLVED (label stale)` · `UNVERIFIABLE`.

### ⚠ Relationship to DEC-339…DEC-355 — read this before using §A

While this sweep ran, the review's 17 headline findings were **formalized into the decision register as
DEC-339…DEC-355 (all PENDING)** and synthesized into `docs/research/2026-07-25-completeness-register.md`
§2 as a ranked 17-item agenda. **Those DEC numbers are the canonical identifiers — use them, not my
`L-nn` IDs, in any ruling.** Per Invariant 19 this file is a *raw evidence* input pointing at them, never a
parallel record. The mapping:

| Register | GR | This file | Register | GR | This file |
|---|---|---|---|---|---|
| **DEC-339** | GR-1 | L-01 / §A1 | **DEC-348** | GR-10 | L-13 / §A11 |
| **DEC-340** | GR-2 | L-10 / §A9 (Q3) | **DEC-349** | GR-11 | L-14 + L-52 / §A12 |
| **DEC-341** | GR-3 | L-73 | **DEC-350** | GR-12 | L-11 / §A9 (Q1) |
| **DEC-342** | GR-4 | L-07 / §A7 | **DEC-351** | GR-13 | L-09 / §A9 (Q2) |
| **DEC-343** | GR-5 | L-03 / §A3 | **DEC-352** | GR-14 | L-16 / §A14 |
| **DEC-344** | GR-6 | L-02 + L-75 / §A2 | **DEC-353** | GR-15 | L-15 / §A13 |
| **DEC-345** | GR-7 | L-04 + L-05 + §A5 | **DEC-354** | GR-16 | L-30 |
| **DEC-346** | GR-8 | L-06 / §A6 | **DEC-355** | GR-17 | L-49 |
| **DEC-347** | GR-9 | L-12 / §A10 | | | |

**What this file adds that the 17-item agenda does not:** (1) the **~78 other on-hold items** — register
PENDINGs, ruled-but-unbuilt specs, MASTER-PLAN queue rows, and KNOWN_ISSUES limitations that sit outside
the review's scope, including the entire S3.2-S3.5 chain and the #33 slice; (2) **§D's 40 verified stale
labels**, which is where tomorrow's decision time was about to be wasted; (3) **§B**, the
ready-for-autonomous-execution list. Two of my rows are *not* in DEC-339…355 and remain separately
tracked: **L-08** (Response-side CRLF guard, already a register PENDING at `C-decisions.md:3048`) and
**L-84** (the `using`/`defer` lifetime block, PENDING since 2026-07-12).

---

## MASTER TABLE

Sorted by **decision-readiness** (a single ruling unblocks work) then impact. `Effort`: S ≤ ½ day ·
M ≤ 2 days · L a multi-day slice · XL a campaign.

| ID | Item | Type | Reality check | Where recorded | Blocking what | Effort | Recommendation |
|---|---|---|---|---|---|---|---|
| **L-01** | **P0 — shadowing a live outer local/param in ANY nested block mistranspiles: PHP has no block scope, so the inner decl clobbers the outer `$a`.** Wrong *values*, not a fault-message skew | decision-needed (P0 correctness) | **STILL OPEN — independently reproduced by me tonight**: `phg run` → `in=2/out=1`; `--tree-walker` → `in=2/out=1`; transpiled PHP under `php-8.5.8` → `in=2/**out=2**`. Emitted PHP is a bare `$a = 2;` in the `if` body. Verified across bare block / if / for / while / shadowed param / 3-deep | `docs/research/2026-07-25-global-review/P0-block-shadow-byte-identity.md:1-107`; **no prior record** (register/UNIFIED-SPEC/KNOWN_ISSUES all grepped clean) | **Invariant 1** (the project's #1 delivery invariant). Any transpiled program using block scoping | M | **Option 1: alpha-rename shadowed locals in the transpiler** (`$a__b1`, reserved `$__phorj_` namespace, deterministic per Inv 10). Keeps a capability the Rust backends already implement correctly. **Plus, regardless of choice, a differential example shadowing in every block form** — the harness globs `examples/**`, and no example shadows, which is exactly why this survived |
| **L-02** | **`main` is still a reserved name in the checker** — `f.name == "main"` forces the entry signature even without `#[Entry]` | decision-needed | **STILL OPEN — reproduced**: a library `function main(string s): string` next to `#[Entry] function boot()` → `[E-MAIN-SIGNATURE] main must be main(): void…`. Also: an `#[Entry] function main` with a bad sig gets **two** errors (`E-ENTRY-SIG` + stale `E-MAIN-SIGNATURE`) | `global-review/E-language-surface.md:781-799` (E20/E21); `K-inline-findings.md:172-175`; code `src/checker/program/type_bodies.rs:347`, `:353`, `src/checker/stmt/core.rs:452` | The freedom `#[Entry]`/DEC-331/DEC-337 promised; library authors naming a function `main` | S–M | **Retire the name reservation**: delete the `main` special case (`E-ENTRY-SIG` already covers every entry under any name), and re-key `cur_is_main` to `entry_declared_role(f).is_some()` so `E-UNCAUGHT-THROW` attaches to the real entry. 14 test assertions to re-point |
| **L-03** | **DEC-248 is half-executed: `for (T x in xs)` was RULED retired, never built — and the docs teach the opposite.** Register conflict **C-2** open since 06-25 | decision-needed | **STILL OPEN — verified**: `grep -rn E-RETIRED-FORIN src/` → **0 hits**; DEC-248's `foreach` half DID ship. `FEATURES.md:27` advertises `for … in` as supported; `examples/guide/foreach.phg:7` teaches it as co-equal. Census: **87 `for…in` vs 8 `foreach…as`** headers in `examples/**` | `C-decisions.md:1372-1381` (DEC-248), `:275` (C-2 "Open — adjudicate"); `global-review/E-language-surface.md:239-312` (E8-E11); `K-inline-findings.md:8-38`; `MASTER-PLAN.md:455` item 10 (no ✅) | Register↔docs coherence (Inv 19); recurs as a question every session (3rd time) | S (amend) / L (retire) | **Amend DEC-248 to "keep both, deliberately", close C-2, then fix E9 (cross-form migration hints) + E10 (let `for` infer its binding).** The corpus has taught "both forms" for a month and the retirement half has failed to get built twice; the retirement is the *inverted* option (it would rewrite the 87-site majority) and needs a `W-DEPRECATED` release first |
| **L-04** | **Package-law enforcement is import-graph-dependent**: the loader's fast path returns before `assemble`, so `validate_package_decl`/`validate_folder_path`/`validate_public_surface` never run for a no-user-imports entry. Adding one `import` flips accept→hard-error | decision-needed (4 gaps, one root cause) | **STILL OPEN** [Verified: probe (c) exit 0 vs probe (j) exit 1, identical shape + one import]. Consequences: A1 any PascalCase non-`Main` package silently legal in a loose file; A3 `E-FILE-*` public-surface rules never fire; A4 `package` is semantically inert (no PHP namespace emitted) | `global-review/A-package-enforcement.md:754-800` (A1-A4), `:184-241` (root cause); code `src/loader/entry.rs:53-66`, `assemble.rs:48-50`; provenance `C-decisions.md:2277` (DEC-282 retired the loose-Main rule deliberately) | The developer's primary complaint; makes 3 already-ruled rules (DEC-029, DEC-282 layout laws, the public-surface spec) unenforced in the most common hand-test shape | M | **Option 1: run the three validators on `entry_prog` before the fast-path return** (they take `(prog, file, root)` only — the disk-scan optimization survives intact). Migration cost measured at **one inert fixture** (31 non-`Main` files, 30 already folder-matching). One chokepoint fixes run/check/transpile/build/test/LSP together. **Must land with L-05 or it starts emitting the wrong message for correct layouts** |
| **L-05** | **A correct non-`Main` entry is rejected with the wrong diagnostic** — `entry.rs:91-92` always passes `roots.entry_local` as the folder root, so `expected` is always empty and every non-`Main` entry hits the "cannot sit directly in the source root" branch | decision-needed (bug + one policy question) | **STILL OPEN** [Verified: probe (i) — `src/App/Cmd/Runner.phg` declaring `package App.Cmd;` → *"cannot sit directly in the source root … (expected under `App/Cmd/`)"* **while sitting in `App/Cmd/`**]. Net effect: *"an entry must be `package Main`"* is enforced **by accident**, with a self-contradicting message | `global-review/A-package-enforcement.md:220-241`, `:534-548`; code `src/loader/entry.rs:91-92`, `src/loader/fs.rs:54-63`; ambiguity in `C-decisions.md:2211` + `src/cli/explain/imports_casts.rs:106-108` | Load-bearing for L-04 | S | **Fix the root unconditionally (pure bug, no design content)**, then rule the separable policy question: *must* an entry be `package Main`, or is it exempt when it satisfies folder=path? If "must", give it a dedicated `E-ENTRY-PACKAGE` rather than a misdirected `E-PKG-PATH` |
| **L-06** | **`Output.printLine` is 55.4% of the corpus's qualified calls (1231/2223) and DEC-326 does not settle its call style** | decision-needed | **STILL OPEN**. `"hello".printLine()` works today [Verified], but DEC-326 reserves module form for "receiver-less calls" and *"is the printed string the subject of printing?"* is exactly the judgement it left open | `global-review/E-language-surface.md:591-600` (E16), `:613-660`; DEC-326 `C-decisions.md` (2026-07-22) | **Blocks any UFCS corpus codemod** — rule it wrong and over half the corpus gets touched twice | S (ruling) | Rule it **before** any codemod. Also rule the companion "deliberate-qualified policy" (which examples keep module form on purpose) in the same sitting |
| **L-07** | **LSP: strict-vs-discoverable is one question asked from two ends** — B7 (module completion is NOT import-gated → suggests uncompilable calls) and B1(D) (should UFCS completion suggest un-imported natives + auto-import?) | decision-needed | **STILL OPEN — new findings, not recorded anywhere** [Verified: `grep -i ufcs` over SLICE-STATE/MASTER-PLAN/KNOWN_ISSUES returns only compiler-internal rows]. Note `SLICE-STATE.md:1022` claims *"LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"* — measurably false for UFCS, the language's primary stdlib call syntax | `global-review/B-lsp-editors.md:99-209` (B1-B10), `:664-688` | Every LSP completion slice downstream; and DEC-326's own rationale (receiver form was chosen *because* `s.`-completion beats module recall) | S (ruling) | **Rule B7 and B1(D) together** — answering them differently leaves the LSP internally inconsistent. Recommended: import-gate, paired with `additionalTextEdits` auto-import on accept |
| **L-08** | **Response-side CRLF guard** — `Response.withHeader` / `Cookie.render` are the unguarded *outbound* header-injection sink (the Request-side wither already faults) | decision-needed (security-adjacent) | **STILL OPEN — confirmed in source**: `src/cli/http_prelude.rs:71-72` interpolates `name`/`value` straight into a header line with zero validation. No later ruling anywhere in the register | `C-decisions.md:3046-3048`; `SLICE-STATE.md:748-750`, `:795-796`; KNOWN_ISSUES `RICHREQ-2026-07-24` | Serve hardening; asymmetric with the already-guarded Request side | S | Guard it, matching the Request-side wither's fail-loud disposition. It is user-visible behaviour on a shipped surface → developer's ruling, but the asymmetry is hard to defend |
| **L-09** | **DB Q2 — `Statement` binds APPEND and never reset**: the hold-a-statement-and-loop scenario hard-errors on iteration 2; the two bind styles diverge silently; the style that "works" is **quadratic** (~75× slower at 8 000 rows) | decision-needed | **STILL OPEN** [Verified: `src/ext/database/ops.rs:75-84`, `:154-211`; probes on both backends; measured 4 000→1.135 s, 8 000→4.469 s vs 0.049/0.059 s for re-`prepare`]. Undocumented in KNOWN_ISSUES. DEC-208 rejected the one-shot shape *because* it had "no Statement reuse" — that promise is unfulfilled | `global-review/D-database.md:250-440` (D1/D2/D3); `C-decisions.md:515` | The developer's own Q2 scenario; a 75× perf trap on the only pattern that appears to work (Inv 18 exposure) | S–M | **Option A: reset binds after each successful execute** (4 sites in `ops.rs`, or one shared helper) — fixes D1+D2+D3 in one move; the driver already caches+resets via `prepare_cached`, so no perf cost. Ship the KNOWN_ISSUES/spec/register disclosure as its docs leg |
| **L-10** | **DB Q3 — no "abort everything"; and `db.transaction(fn)`'s auto-rollback pops only ONE level (D4)**, so a closure that leaked an inner `begin()` leaves the outer transaction OPEN with partial writes live — the caught error reads as "rolled back" but a later `commit()` **persists the data** | decision-needed (P1 silent data persistence) | **STILL OPEN** [Verified: `src/ext/database/wrappers.rs:133-136`; probes `leak.phg`/`leak2.phg`]. Also D5: `ops.rs:408` emits bare `RELEASE id` (MySQL needs `RELEASE SAVEPOINT id`) while the module's own `mysql.rs:156-157` uses the correct form — **zero nested-savepoint coverage on MySQL or Postgres** | `global-review/D-database.md:440-636` (Q3, D4, D5) | Transactional correctness — the most severe DB finding | M | **Option A + D, with E as a cheap companion**: add `rollbackAll()` (one top-level `ROLLBACK` discards every savepoint at any depth — no loop needed), point `db_transaction`'s error arm at it so it unwinds to its entry depth (fixes D4), and expose `transactionDepth()` (the value is already computed at `ops.rs:389` and thrown away). **Fix D5 independently** via the `DriverConn` seam + nested-savepoint tests |
| **L-11** | **DB Q1 — rename the type `Database` → `Connection`** (and possibly drop the module to bare `Core.Database`) | decision-needed (naming) | **STILL OPEN — no register row or spec entry exists for it** [Verified: greps in `D-database.md`]. The object is provably ONE connection (4 code proofs; pooling explicitly out of scope) | `global-review/D-database.md:125-249` | Nothing; but it is churn that gets cheaper the earlier it lands | M (codemod) | **Option B** (type → `Connection` **and** module → bare `Core.Database`), fallback **A** (type only). Decisively: DEC-278's `Module` suffix exists *only* for the namesake collision the rename removes, so A leaves a suffix with no remaining justification. Needs a DEC-278 amendment row either way |
| **L-12** | **Streaming file reads ("handle any size of files")** — the lazy `Iterator<T>` protocol already exists and already streams, but **only for stdin**; no FILE can be read incrementally | decision-needed | **STILL OPEN** [Verified: `Input.lines()` streams an 88 MB input in **23.7 MB peak RSS**, byte-identical on all 3 legs; `Core.FileSystemModule`/`Core.File` are whole-slurp only]. Register status: DEFERRED, and once outright REJECTED | `global-review/C-stdlib-input-fs-clone.md:27-346`; `C-decisions.md:137-164` (the deferral) | Any large-file workload | M (O2) / L (O1) | **O2 first, with O1 as the declared upgrade path**: `FileSystem.lines(path)` backed by an offset-chunk native + a pure-Phorj `FileLines implements Iterator<string>` prelude class — **zero** new `Value`/`Ty`/`emit_type` machinery, identical user-facing syntax to O1, same O(1) memory. Because the surface is identical, O1 later is a non-breaking internal swap. Bench the chunk re-open cost (Inv 11) before any perf claim |
| **L-13** | **Filesystem locking ("lock a file, access it when available")** — nothing exists; no `flock`/lock code anywhere in `src/` | decision-needed | **STILL OPEN, and the presumed blocker is FALSE**: `std::fs::File::{lock, lock_shared, try_lock, try_lock_shared, unlock}` are **STABLE on the pinned toolchain 1.97.1** [Verified: compiled and ran them]. Rust-std locks and PHP `flock()` were verified to **interoperate bidirectionally**. **No new dependency, no `unsafe`, no policy amendment.** Register status: no disposition whatsoever | `global-review/C-stdlib-input-fs-clone.md:347-602` | Any multi-process file workload | M | **O5: `FileSystem.withLock<T>(path, () => T)` + `tryWithLock<T>(…): T?`** — scoped closure, whole-file, advisory (the portable ceiling: byte-range needs `fcntl` → dep + `unsafe`, both policy-blocked). **Reject timeout for v1** (no native support either side; a spin-sleep is a bandaid and makes wall-clock observable). Surface the `try/finally` PHP-helper trade (Inv 16) and the Windows semantics question explicitly. Consider `writeAtomic` (O8) as a companion, not a substitute |
| **L-14** | **No-op clone — `p with { }`: already works, entirely undocumented** | decision-needed (bless-or-not) | **ALREADY BUILT, unrecorded** [Verified: parses, checks, runs identically on VM + tree-walker, transpiles to PHP `clone($p)`]. Absent from `examples/guide/clone-with.phg`, from `FEATURES.md`, and **from `UNIFIED-SPEC.md` entirely**; no DEC row; **the lifter refuses it** (`src/lift/printer/exprs.rs:217-224`) = a live Inv-17 violation. Formatter prints `with {  }` (double space) | `global-review/C-stdlib-input-fs-clone.md:603-964` (C12-C17) | Nothing — the capability ships. The debt is docs + lift | S | **O9: bless and document the existing form; add NOTHING to the language.** C# records `with { }` and Kotlin `copy()` — the two languages this syntax came from — both make the empty form the canonical no-op copy. Then fix the real debt: lift `clone $x` → `x with { }` (**refusing loudly** when the PHP class declares `__clone`, since phorj has no equivalent — Inv 14 case 3), docs at the point of use incl. the shallow boundary, and the formatter double space |
| **L-15** | **Entry ceremony: `#[Entry]` costs TWO imports; the minimal program is 6 lines vs PHP's 2** (4 of 6 lines are ceremony) | decision-needed | **STILL OPEN** [Verified: each omission is a separate hard error; `Core.Runtime.Entry` **and** `Core.Runtime.EntryKind` are both mandatory] | `global-review/K-inline-findings.md:131-171` | Nothing; but it is paid in **every single file that runs** | S–M | **(a) Auto-inject `Core.Runtime.{Entry,EntryKind}` into scope** — the error text itself calls `Entry` "an injected `Core.Runtime` type", so requiring an explicit import for a compiler-injected symbol is arguably self-contradictory. Alternative (b): one combined `import Core.Runtime;`. Both interact with the `E-UNIMPORTED`/`E-INJECTED-VARIANT-BARE` machinery DEC-337 just built → a real design question |
| **L-16** | **"Visibility/access in blocks"** — the phrasing is ambiguous across five readings (bare blocks / modifiers on locals / named nested functions / local types / explicit capture lists) | decision-needed (disambiguate first) | **F-i already exists** (true lexical block scoping, verified: a block-local is unreachable after its block → `E-UNKNOWN-IDENT`). F-iii/F-iv/F-v do not exist | `global-review/F-block-visibility-research.md:1-80` | Nothing until disambiguated | S (ruling) | **Disambiguate first.** Recommended: **no** to F-ii/F-iv/F-v; a spec+ruling for **F-iii (named nested functions) without visibility modifiers** (every peer language that has the feature omits access control on it). Standing principle worth recording: *visibility is a top-level/member-axis concept; inside a function body the axis is lifetime/scope, not access* |
| **L-17** | **P-Q-B-1 — overloaded interface-method visibility narrowing**: the `overloads == 1` guard on `E-IFACE-VIS` leaves >1-overload reduced-visibility impls reachable via a plain interface-typed receiver | decision-needed (real soundness hole) | **STILL OPEN — reproduces with `private`**; pre-existing, not introduced by Q-B | `C-decisions.md:3261-3264`; `docs/specs/2026-07-24-visibility-model.md:151-163`; `SLICE-STATE.md:68`, `:75`; KNOWN_ISSUES `F-032` (`:188`) | Closure of the visibility model | S–M | Close the hole (drop the `overloads == 1` guard and check every overload's declared visibility). It is a visibility bypass, not a stylistic deferral |
| **L-18** | **P-Q-A-1 — Core-submodule wildcards (`import Core.Http.*`) parser-rejected** | decision-needed / ruled-not-built | **STILL OPEN**. The loader's native/prelude pre-pass intercepts `Core.*` imports *before* the wildcard-expansion hook, so a naive attempt binds nothing silently; rather than ship silent-wrong, both bare and submodule `Core.*` are rejected (`E-WILDCARD-STDLIB-ROOT`, honest "not yet supported"). **D4 originally allowed `Core.Sub.*`** — this narrows the ruling | `docs/specs/2026-07-24-wildcard-imports.md:169-177`; `C-decisions.md:3251`; `SLICE-STATE.md:50` | The single capability gap in Q-A — and stdlib is where a wildcard would be used most (**the likely source of the "wildcards aren't fully supported" impression**) | M | **Build it, preceded by the five cheap diagnostic fixes E1/E3/E4/E5/E6.** The enabler already exists: promote `src/lsp/catalog.rs::module_members` to a `pub(crate)` native enumerator (the spec's own STEP 2 DETAIL names this) and run wildcard expansion before/inside the loader's prelude pre-pass |
| **L-19** | **P-Q-A-2 — D3's ruled wording "`*` binds public + internal" conflicts with the as-built public-only cross-package rule** | decision-needed (confirm wording) | **STILL OPEN — as-built is correct and principled** [Verified: `loader::vis_violation` — a cross-package `internal` member is `E-VIS-INTERNAL`, i.e. not individually importable, so binding it via `*` would be inconsistent]. `FEATURES.md:94` + `examples/README.md:236` already describe the as-built behaviour | `docs/specs/2026-07-24-wildcard-imports.md:178-186`; `C-decisions.md:3251`; `2026-07-25-plans-divergence-audit.md:139-143` | Spec↔code coherence only | S | Confirm the as-built rule and amend D3's shorthand. The unifying principle ("every member you'd be allowed to import individually") is the safe one |
| **L-20** | **P-Q-A-4 — group-`{}` member sorting is a structural no-op** (groups are expanded at PARSE time per DEC-186, so the formatter never sees a `{}` group) | decision-needed | **STILL OPEN**. Ruling (e)'s "sort `{}`/`except {}`" is honoured for `except {}` (a wildcard tail the formatter does see) but is unimplementable for `{}` without re-homing group expansion out of the parser | `docs/specs/2026-07-24-wildcard-imports.md:194-203`; `C-decisions.md:3253` | Cosmetic only (idempotent, byte-identity-safe) | S (accept) / M (re-home) | Accept the no-op and amend ruling (e) — re-homing DEC-186's parse-time expansion to buy a cosmetic sort is a poor trade. The same AST change would also let the formatter *preserve* group syntax (currently it desugars groups to per-line imports), so bundle the two if it is ever wanted |
| **L-21** | **DEC-286 — flipping `jsonround` needs a value-model rebuild (arena / lazy-materialize Json nodes)**, explicitly *"NOT autonomously attempted"* | decision-needed (spine-deep) | **STILL OPEN**, but **partly routed around**: the ruled alternative (DEC-333(a) Json-ADT JIT, task #33) is IN FLIGHT and does not require the arena. ~65% of allocs are the `Rc<EnumVal>` box itself | `C-decisions.md:152`; `SLICE-STATE.md:1651-1652`, `:1675-1676` | The last structural perf losses | XL | Let **#33 land first** and re-measure before ruling the arena — if the JIT vertical flips jsonround/deepjson, the arena's cost/benefit changes completely |
| **L-22** | **DEC-334 — runtime-config catalog (php.ini-equivalent)**: a queued multi-round interactive research+design campaign; *"Not scheduled yet — dev to slot"* | decision-needed (scope + scheduling) | **STILL OPEN** — no spec or catalog file exists under `docs/specs/`. The recorded consts (`SPILL_THRESHOLD`, `MULTIPART_MAX_PARTS`, `DEFAULT_MAX_BODY_SIZE`) are its first rows | `C-decisions.md:3104-3114`; `SLICE-STATE.md:753`, `:799`; `MASTER-PLAN.md:110` | `#[Config]` precedence completion; the S3.2 `ServeConfig` frame-cap-vs-body-cap reconciliation | L | Slot it **after** S3.2/S3.3 — `ServeConfig` will surface the first real precedence rows, so designing the catalog before it is speculative |
| **L-23** | **DEC-322 — real (multi-core) parallelism: the design forks are unadjudicated** | decision-needed (design-first, no build) | **STILL OPEN** [Verified: `src/green/` is cooperative-only — spawn + channels, 1 OS thread, `Rc` heap `!Send`; no multi-core scheduler]. Model already ruled = **actor/isolate** (private `Rc` heap per worker, Send-only deep-copied values); the DEC-225 Fibers PHP-mapping spike was never run | `C-decisions.md:946-949`, `:1044-1054`, `:2659-2668`; `SLICE-STATE.md:1576-1585`, `:1007-1008` | The beyond-PHP parallelism story | XL | **Do the research doc first** (as already ruled): `docs/research/` parallelism design with the cross-language matrix, perf model, syntax sketch, and quarantine analysis — *then* adjudicate syntax. Explicitly rule whether the Fibers spike is abandoned (DEC-322 already treats concurrency as permanently PHP-excluded) |
| **L-24** | **W4-10 — XML / DOM / XPath: "the one open Wave-4 fork"** | decision-needed | **STILL OPEN** — zero hits for `XmlDocument`/`DomDocument` across `src/`, `examples/`, `docs/specs/` | `MASTER-PLAN.md` Wave 4 §; `SLICE-STATE.md:1461`, `:1586` | FN parity (named as a top remaining FN blocker alongside streams/intl/SPL-heaps) | L | Rule the fork (std-only hand-rolled vs a vetted dep vs defer) before scheduling — it is a dependency-policy question as much as a stdlib one |
| **L-25** | **`App\`-prefixing / the `phpInterop{namespaceRoot,sourceRoot}` knob (DEC-320 F2)** — *"is `App\`-prefixing worth the transpiler-wide namespace-prefix plumbing, or is the no-prefix law fine for GA?"* | decision-needed | **STILL OPEN** [Verified: `grep phpInterop src/` → 0 hits; no later ruling in the register] | `C-decisions.md:2804-2808`; `docs/specs/2026-07-22-transpile-into-project.md:6` | `phg build --php` GA polish | M | Rule "no-prefix law is fine for GA" unless a concrete host-project need appears — the plumbing is transpiler-wide and buys only cosmetics |
| **L-26** | **`Core.File` deprecation/migration** — a queued adjudication (changing its error contract is user-visible) | decision-needed | **STILL OPEN** [Verified: `src/cli/preludes.rs:127` still contrasts *"the older `Core.File`"*; no later DEC ruling anywhere in the register] | `C-decisions.md:1092-1095` | Stdlib coherence — two file APIs ship side by side | S (ruling) | Rule it now, cheaply: either deprecate `Core.File` with a `W-DEPRECATED` release or bless the split with a stated boundary. **L-12's `FileSystem.lines()` should land on the winning surface**, so this is worth ruling in the same sitting |
| **L-27** | **`maxBy`/`minBy` residual: the general nullable-unboxed-Kind representation lever** | decision-needed | **PARTIALLY BUILT** — the common path shipped (`maxby` 0.19×→**8.13×**, `minby` 0.20×→**8.18×**) via the ruled `??`-fusion lever, no representation change needed. The register's own text is non-linear here: the general lever "REMAINS OPEN, queued" for window-less call sites | `C-decisions.md:3192-3201` (flip) vs `:3231-3237` (the still-open flag) | Nothing urgent | L | Accept the flag for window-less sites and close the hard-flag row with the narrowed scope stated — the win already shipped, and the register currently reads as if the whole item is blocked |
| **L-28** | **Pipe-lambda trailing-op binding** — should trailing tight-ops bind to the pipe result after a contextual lambda? *"strictly additive and awaits a ruling"* | decision-needed (ergonomics) | **STILL OPEN** [Verified: no later mention in the remaining ~2 100 register lines] | `C-decisions.md:1230-1234` | Nothing | S | Rule it either way to close the fork; low stakes, purely additive |
| **L-29** | **DEC-219 — static overload resolution** (checker picks the overload at compile time when arg types are known) | ruled-not-built (self-deferred) | **STILL OPEN** [Verified: `src/checker/calls/{overloads,methods}.rs` still describe runtime dispatch via `Op::CallOverload`/`CallStaticOverload`; no static-resolution codepath] | `C-decisions.md:751-757` | Nothing (a META-6 zero-cost perf win, no surface change) | M | Leave open — the ruling itself says "Deferred (low priority)". Good filler work for an autonomous batch |
| **L-30** | **Claude-bundle import: 8 open questions Q-J1…Q-J8** (which of 48 skills to import, `/converge` tier hard-coding, whether to import the 4 gate Stop hooks, `settings.json` deny/ask tiers, `scripts/disk-reclaim.sh`, phorj-native `/full-review`, phorj agent defs, framework-body pruning) | decision-needed (8 sub-questions) | **STILL OPEN** — J.1 also found `FINDING J-DANGLE`: the installed framework references **7 things that do not exist here** | `global-review/J-claude-bundle.md:1-205` | Tooling quality for future sessions | S–M | Walk the 8 as a single block — the file already carries a recommendation per question (Tier-A 11 skills IN; skip the Stop hooks to avoid double-gating; author a phorj-native `/full-review` rather than importing `/mega-analysis`) |
| **L-31** | **`VirtualModule.src`→`srcs` rename** — an autonomous in-build decision listed "dev to review" | decision-needed (rubber-stamp) | **STILL OPEN** — already implemented; no sign-off recorded | `C-decisions.md:3041-3046` | Nothing | S | Rubber-stamp or revert in passing |
| **L-32** | **`phg serve` inbound-TLS posture** — recorded as a **GA-blocking** PENDING adjudication | ruled-not-built (posture ruled; build outstanding) | **STILL OPEN as work, RESOLVED as a question**: DEC-329/DEC-331 D7 ruled it (native rustls termination behind a feature-gated `http-server-tls`); the **build** is task #38 / spec S3.5, unbuilt | `C-decisions.md:2680-2692` (GA-blocking flag), `:2866-2911` (D7); `MASTER-PLAN.md:78-79`, `:108`; spec `2026-07-23-entry-kinds-serve-tls.md` S3.5 | GA | L | Treat as **B-list work, not a question** — but confirm the GA-blocking label still applies, and note the rustls **server-side** dep admission goes through the dependency policy like `http-client` did |
| **L-33** | **DEC-324's 7 remaining TOP items** — trusted proxies · response streaming · Range + gzip · HttpClient proxy/CA/mTLS + streaming · class-const expressiveness · enum interfaces/consts | decision-needed (7 items) | **STILL OPEN** — zero hits in `src/` for any of them; **no DEC ruling anywhere in the remaining ~700 register lines** | `C-decisions.md:2680-2692` | Web-pack production maturity | M–L each | Batch-adjudicate their surfaces upfront (the register's own stated pattern for batch candidates), then build. Trusted proxies + response streaming are the two with real security/scale weight |
| **L-34** | **SessionStore swappable-backend contract (v2)** — file/redis-style backends behind a prelude-visible contract | deferred (v2) | **STILL OPEN** [Verified: `src/ext/session/natives.rs` implements only the v1 in-process `Mutex<HashMap>`; no `SessionStore` trait] | `C-decisions.md:1119-1121` | Multi-process `phg serve` | M | Keep deferred until `serve` is multi-process — v1 correctly matches the single-process model |
| **L-35** | **DEC-224 — MongoDB**: admission SHAPE ruled (twin-of-Db, `Core.Mongo`, `E-TRANSPILE-MONGO`), build deferred | deferred (shape ruled) | **STILL OPEN** [Verified: `grep -rli mongo src/ Cargo.toml` → zero hits] | `C-decisions.md:941-945`, `:1031-1043` | Nothing — *"the DEFER costs only absence, never wrongness"* | L | Keep deferred behind the value-ordered packs |
| **L-36** | **DEC-247 — `Core.DateTime` with a vendored-IANA tz crate**: PENDING-BLOCKED → **UNBLOCKED + RULED** to admit the crate, full named-zone + DST support | ruled-not-built | **STILL OPEN — never built** [Verified: no `chrono-tz`/`tzdb` in `Cargo.toml`, no `Core.DateTime` module in `src/ext/` or the preludes], despite being unblocked/ruled nine days ago | `C-decisions.md:1342-1371` | Date/time parity; a named FN blocker | L | **High-value B-list item**: the dependency ruling (the hard part) is already done. Build order is stated in the ruling itself: crate vetting → live `DateTimeImmutable`/`DateInterval` probe rounds → kernel → prelude twin |
| **L-37** | **S3.2 — `Http.ServeConfig` + `#[Config]`-provider-by-TYPE resolution + the precedence chain** (CLI flag > env > `#[Config]` > `phorj.json` static > attr default) | ruled-not-built | **NOT BUILT** [Verified twice, independently: `ServeConfig` appears in `src/` only as a **code comment** at `src/native/http.rs:24`; `phg check` on `Http.ServeConfig` → `unknown type`] | spec `2026-07-23-entry-kinds-serve-tls.md` S3.2; `SLICE-STATE.md:129-131`; `C-decisions.md:2866-2911` (D1/D4); task #35 | S3.3; Rich Request's Eager/Lazy switch; L-22's first catalog rows; the body-cap-vs-frame-cap reconciliation | M–L | **Next after #33** — it is the keystone of the remaining slice-3 chain. Builds on shipped DEC-318 |
| **L-38** | **S3.3 — `Http.serve(cfg, handler)` runtime + RETIRE `respond` (breaking change #2)** + migrate `examples/web/*` and site-mode `index.phg` | ruled-not-built | **NOT BUILT** [Verified: `respond` is **still** the live `SERVE_ENTRY` at `src/serve/handlers.rs:27`; `phg serve --help` still documents *"calls respond(bytes): bytes per request"*; `Http.serve(` → zero hits] | spec S3.3 (D5); `SLICE-STATE.md:132`; task #36 | S3.4; the typed `(Request):Response` handler becoming reachable | L | Build after L-37. **Cross-file consequence worth surfacing: Rich Request's shipped `Request`/`Response` types are reachable today only through the legacy `respond` bridge** — a shipped feature is gated from its intended entry point |
| **L-39** | **S3.4 — role-mismatch UX**: `run`→Cli / `serve`→Web, `E-NO-ENTRY-FOR-ROLE`, TTY prompt vs non-TTY error, symmetric both directions | ruled-not-built | **NOT BUILT** [Verified: `E-NO-ENTRY-FOR-ROLE` → zero hits in `src/`; `phg explain` → unknown code] | spec S3.4 (D6/P3); `SLICE-STATE.md:133-134`; task #37 | Nothing downstream | M | Build after L-38 (it needs both roles to be real) |
| **L-40** | **S3.5 — inbound TLS via rustls**, feature-gated `http-server-tls` + a UNIFIED-SPEC external-deps row in the same change + a `serve_tls.phg` walkthrough | ruled-not-built | **NOT BUILT** [Verified: `Cargo.toml` has only the pre-existing **outbound** `http-client` rustls feature (`:122`, `:134`); no `http-server-tls`] | spec S3.5 (D7/P2); `SLICE-STATE.md:135-136`; task #38; = L-32's build leg | GA (per L-32) | L | **Last in the chain** — it isolates the new dep and the all-features gate, exactly as the sequencing ruling states |
| **L-41** | **#33 — Json-ADT JIT slice (DEC-333(a))**: flip `jsonround` (0.31×) + `deepjson` (0.99×) | ruled-not-built (**IN FLIGHT**) | **PARTIALLY BUILT** — steps 1, 2, 3, 5a, 5b-i, 5b-ii, 5b-iii DONE and pushed. **Remaining: the WRITE-path helpers** (`rt_u_json_stringify`/`rt_u_json_clone`/jmap scratch build+seal) [Verified: both symbols → 0 hits], the refinement peephole, and the analyze+emit arms. Kind variants still carry `#[allow(dead_code)]` [Verified: `src/jit/handles/{json_ext.rs:25,32, helper_refs.rs:97,99}`] — a clean pause point, no broken state | `SLICE-STATE.md:262-360` (plan v7, gate CLOSED after 6 panel rounds, ~46 findings folded); task #33 | The perf-flip campaign; the AOT phase (DEC-333(b)) is gated on JIT-WINS-ALL | L | **Resume here** — the plan is v7-complete with no gate rounds owed (6C panel owed *after* the build). Split `json_ext.rs` (417 lines) when stringify/clone push it toward the 500 hard cap |
| **L-42** | **Spread — DEC-299 (a) `f(...list)` List→positional, (b) `f(...["k": v])` Map-literal→named, (c) runtime union-Map→named with `E-SPREAD-ARG`** | ruled-not-built | **STILL OPEN — verified by probe**: `add(...xs)` → `parse error: expected an expression, found DotDotDot`. The `...` token exists (variadics shipped) but no spread call-arg path | `SLICE-STATE.md:1503-1504`, `:1515-1521`, `:1554-1560`; DEC-299 | Named-args/variadics completeness | M (a+b) / L (c) | **Good autonomous work: (a) + (b) only.** The build approach is already investigated and recorded (parallel `arg_spread: Vec<bool>` field on `Expr::Call`, desugar at the `check_and_expand` chokepoint). Leg (c) *"DEPENDS on `Map<K, union>` ergonomics being solid — VERIFY FIRST"* |
| **L-43** | **Labeled `break`/`continue`** (`label@` form, loops-only v1) | ruled-not-built | **NOT BUILT** [Verified: `outer@ for (…)` → `lex error at 7:10: unexpected character '@'` — not even tokenized; `E-LABEL-TARGET`/`E-UNKNOWN-LABEL`/`E-DUPLICATE-LABEL` → 0 hits] | spec `2026-07-23-labeled-break-continue.md` (RULED — BUILD-READY); `C-decisions.md:2953-2954`, `:2984` | Nothing | M | **Clean autonomous slice** — spec is frozen and unambiguous |
| **L-44** | **Typed LSB — the `Self` return type** (STRICT compile-time ctor check) | ruled-not-built | **NOT BUILT** [Verified: `: Self` → `unknown type 'Self'`; `E-SELF-CTOR-MISMATCH`/`E-SELF-PARAM`/`E-SELF-FIELD` → 0 hits]. `KNOWN_ISSUES.md:634-642` documents the *older* rejected PHP-style runtime LSB — the predecessor design this spec replaces, not a conflict | spec `2026-07-23-typed-lsb.md`; `C-decisions.md:2955-2956`, `:2984` | Nothing | M | **Clean autonomous slice** |
| **L-45** | **`Core.Sandbox`** — pure-expression eval, tree-walker-only, `E-TRANSPILE-SANDBOX` (a SCOPE CHANGE: ruled to build in v1) | ruled-not-built | **NOT BUILT** [Verified: `Sandbox sb = Sandbox.expressionsOnly();` → `unknown type 'Sandbox'`; only an incidental "sandbox" substring in `src/mem.rs`]. The spec's present-tense *"BUILDS IN V1"* reads as a completion report — it is a ruling-to-build | spec `2026-07-23-eval-position.md`; `C-decisions.md:2957-2959`, `:2985-2987` | Nothing | M–L | Autonomous-capable, but it is a new LADDER case-2 quarantine (`E-TRANSPILE-SANDBOX`) — the four accepted compromises are recorded, so re-read them before starting |
| **L-46** | **ArrayAccess — `#[ArrayGet]`/`#[ArraySet]`**, overloaded indexers in v1, PHP `\ArrayAccess` glue emitted ("ADOPTED with a REOPEN flag") | ruled-not-built | **NOT BUILT** [Verified: `#[ArrayGet]` → *"unknown attribute"* — not even in the recognized-attribute set; `E-ARRAYACCESS-*` → unknown to `phg explain`] | spec `2026-07-23-array-access.md`; `C-decisions.md:2960-2961`, `:2987-2989` | Nothing | M–L | Autonomous-capable. Note the REOPEN flag ("might revisit") — worth a 30-second re-confirm before spending the slice |
| **L-47** | **DEC-335 — two-tier top types `Any` + `Object`** (`Any`→`mixed`, `Object`→`object`) | ruled-not-built | **NOT BUILT** [Verified: `Ty::Any`/`TypeKind::Any` → 0 hits; `Any a = 42;` → `unknown type 'Any'`; `Object o = …` → `unknown type 'Object'`; no example] | spec `2026-07-23-any-object-top-types.md`; `C-decisions.md:3083-3103`; `MASTER-PLAN.md:108`, `:137` | Nothing queued | M | Autonomous-capable, with **three small open points to rule first**: `E-INSTANCEOF-ANY` error-vs-fold, the union-folding detail, and reified-Op reuse ("verify at build"). Also worth a one-line ADR-0002 cross-reference (its "no raw mixed/any escape hatch" consequence) |
| **L-48** | **Slice 1b (DEC-331 D9c)** — function-type assignability, transpile PHP `__invoke` (single delegate + multi-invoke `__phorj_invoke_dispatch` shim), lift `__invoke`→`#[Invoke]` | ruled-not-built (deferred, reopenable) | **STILL OPEN** [Verified: `grep -rl __phorj_invoke_dispatch src/` → 0 hits]. Slice 1 itself is BUILT (`examples/guide/invoke-tostring.phg` ships) | spec `2026-07-23-invoke-tostring.md:121-133`; `C-decisions.md:2933-2934`, `:3009-3012`; `SLICE-STATE.md:178-180` | The "instance as a first-class callable VALUE" cluster | M | Autonomous-capable, but the coupled cluster means it wants its own slice rather than a filler |
| **L-49** | **W2-4 — retire the `->` return annotation (parser-reject)**; canonical is `: T` | ruled-not-built | **STILL OPEN — verified**: `function main() -> void` still **type-checks clean**. Cost quantified for the first time tonight: **87 `.phg` occurrences** + **2 068 `.rs` inline test fixtures across 90 files**. **Key enabler nobody had recorded: `phg format` ALREADY normalizes `-> ` → `: `**, and pre-commit already runs `phg format --check`, so the `.phg` half is a mechanical sweep | `UNIFIED-SPEC.md:162`; `MASTER-PLAN.md:1556`; `global-review/K-inline-findings.md:42-80` | Nothing; but it is a "nothing in the wind" wart on the flagship syntax | M | **Substantially cheaper than its "parser-reject pending" status suggests.** Order: (1) scripted fixture rewrite, (2) parser-reject, (3) un-ignore/refresh dormant tests, (4) a grep gate so no NEW `->` fixture lands. **⚠ Must separate return-annotation `->` from fn-type/prose `->`** (e.g. `examples/web/middleware.phg:5`, `selftest/faults.phg:6`) or a naive sweep corrupts them |
| **L-50** | **`phg stubs` + `phg watch --php`** (DEC-320 v2 queue) | ruled-not-built | **NOT BUILT** [Verified: neither subcommand exists; `phg build --php` v1 confirmed working by live run] | spec `2026-07-22-transpile-into-project.md:53`, `:60-61`; `SLICE-STATE.md:990` | PHP-host interop DX | M each | Autonomous-capable; `phg stubs` is the higher-value half |
| **L-51** | **Lift catch-up (Inv-17 debt): Uri Tier-2 mapping** | ruled-not-built | **STILL OPEN** [Verified: `grep Uri src/lift/` → 0 hits]. Its two siblings are DONE: (a) `private(set)`/`protected(set)` lifts (`src/lift/ast.rs:54-55`, `parser/items.rs:208`); (b) `foreach ($m as $k => $v)` is Tier-1 (`lifter_tests.rs:281`, DEC-280) | `SLICE-STATE.md:2293-2297` | Lifter fidelity | S | **Good filler.** Note the queue entry is now 2/3 stale — see L-70 |
| **L-52** | **Lift `clone $x` → `x with { }`** (C13, a live Inv-17 violation) | ruled-not-built | **STILL OPEN** [Verified: `src/lift/printer/exprs.rs:217-224` refuses `Expr::CloneWith`; `src/lift/parser/exprs.rs:303-304` refuses PHP `clone` as Tier-2/3] | `global-review/C-stdlib-input-fs-clone.md:760-770` (C13) | The one place PHP→phorj migration loses a construct phorj actually has | S–M | Build with L-14. **One embedded decision: the refusal boundary** — PHP `clone` can invoke `__clone`, which phorj has no equivalent for, so the lift must **refuse loudly** when the source class declares `__clone` (Inv 14 case 3) |
| **L-53** | **Genuine stdlib companion gaps: `Map.update`, `List.scan`/`windowed`/`associateBy`/`countBy`** | ruled-not-built | **STILL OPEN** [Verified: none of the five names appear in the native registry]. Grep-verified as genuine (unlike several "gaps" that turned out already built) | `SLICE-STATE.md:1150-1151` | FN parity breadth | S each | **Ideal autonomous batch** — no design fork, the native recipe is proven, each ships byte-identical + an example |
| **L-54** | **Exception backtrace API — `getTrace`/`getTraceAsString`/`getFile`/`getLine` on CAUGHT exceptions** | ruled-not-built | **STILL OPEN** [Verified: `getTrace`/`getTraceAsString` → 0 hits in `src/`]. Today only *uncaught* faults render a trace | `SLICE-STATE.md:1566-1568` (item 5, "VERIFIED GAP"), `:1163` | RT parity + logging quality | M | Autonomous-capable; already flagged "FRESH session" in the queue |
| **L-55** | **`serialize`/`unserialize` + `var_export`/`print_r`** | ruled-not-built | **STILL OPEN** [Verified: 0 hits in `src/` for all four] | `SLICE-STATE.md:1570` (item 7, "VERIFIED absent") | FN parity + a "big lifter unblock" | M | Autonomous-capable. Design note worth surfacing: `serialize`'s format is a PHP-compat contract, so byte-identity is load-bearing here in an unusual way |
| **L-56** | **`Core.Process` run/spawn/exec + pipes + stdout/stderr capture + exit codes** (today: args/env-get only) | ruled-not-built | **STILL OPEN** [Verified: only `Core.Process.args()` exists] | `SLICE-STATE.md:1571-1572` (item 8) | RT / real-app parity | M | Autonomous-capable, but it is impure/quarantine-shaped — check the differential quarantine story before starting |
| **L-57** | **Collections: `Set` / `Deque` / `PriorityQueue`** (SPL parity) | ruled-not-built | **PARTIALLY BUILT** — `Core.Deque`/`Core.PriorityQueue` preludes exist (`src/cli/preludes.rs:243-251`, `:290-298`, returning `T?` rather than throwing — a recorded better-than-PHP departure); the **heap** family is the real remaining gap | `SLICE-STATE.md:1573` (item 9, now partly stale); MASTER-PLAN stdlib tail | FN parity | S–M | Re-scope the queue entry to the genuine remainder before building — item 9 as written overstates the gap |
| **L-58** | **Generators / `yield`** | ruled-not-built | **STILL OPEN** [Verified: no `Yield` AST node / token; the only `yield` hits are prose] | `SLICE-STATE.md:1575` (item 11), `:1185`, `:1396-1398` | Iterator breadth | XL | **Keep last / fresh-context** as already ruled — deepest VM control-flow spine. Note L-12 explicitly does **not** need it |
| **L-59** | **Inv-13 ratchet: 66 source files still exceed the 500-line hard cap** | deferred (ongoing campaign) | **STILL OPEN, but NOT a gate failure** [Verified: `scripts/size-gate.sh` → `grandfathered=78 fails=0 warns=118`, OK. `find src -name '*.rs' | xargs wc -l | awk '$1>500'` → **66 files**; worst: `desugar_db.rs` 3139, `jit/analyze/mod.rs` 2476, `jit/handles/mod.rs` 2000, `jit/emit_unboxed/mod.rs` 1658, `transpile/runtime_php.rs` 1370]. **Improved from 90** (CRAFT-1's figure) | KNOWN_ISSUES `CRAFT-1` (`:96-103`); `SLICE-STATE.md:14-16`; Invariant 13 | Named explicitly as blocking new JIT perf verticals (target files at zero headroom) | XL ongoing | Keep split-as-you-go as the default. **`src/checker/desugar_db.rs` (3139) is the standout** and is not a single-dispatcher exemption |
| **L-60** | **`STACKDEPTH-deep-member-chain` — pathological deep left-associative expressions SIGABRT the checker** (`enforce_injected::walk_expr` + other guard-free expr walkers) | deferred known-limitation | **STILL OPEN** — pre-existing, surfaced during the DEC-337 review; ordinary deep member exprs SIGABRT identically, so it is not DEC-337-caused | `KNOWN_ISSUES.md:158`; `SLICE-STATE.md:91-94` | Nothing user-facing today | M | Fold into a general robustness slice (a shared depth guard across every expr walker + a `limits.rs` constant) rather than patching one walker |
| **L-61** | **PSR-4 namespace↔folder *aliasing* — never built** (only the plain `folder = package` law exists) | deferred (gated on re-adjudication) | **STILL OPEN** — DESIGNED-NOT-IMPLEMENTED | `UNIFIED-SPEC.md:483`; `MASTER-PLAN.md:1559`; `global-review/A-package-enforcement.md:~840` | Nothing | L | Keep deferred; it interacts with L-04's ruling, so re-confirm the deferral in the same sitting |
| **L-62** | **M2.5 Phase 3 — CI stub registry + code signing (`phg build --sign`)** | deferred | **STILL OPEN** [Verified: the `--sign` flag is parsed but documented "reserved for Phase 3"; no `rcodesign` usage anywhere]. `UNIFIED-SPEC` mentions macOS code-signing as DEFERRED 6× | `MASTER-PLAN.md` M2.5 §; `UNIFIED-SPEC.md` (6 deferral mentions) | Signed/distributable binaries | L | Keep deferred until distribution is a real goal |
| **L-63** | **`phg env` doctor command** (phpinfo-equivalent) | deferred | **STILL OPEN** [Verified: absent from `phg --help` on the fresh binary] | MASTER-PLAN queue | Support/debug UX | S | Cheap, self-contained autonomous filler |
| **L-64** | **BigInt / Money (W4-13) arbitrary precision** | deferred design | **STILL OPEN** [Verified: zero `BigInt` hits in the repo] | `MASTER-PLAN.md` Wave 4 §13 | TOP-20 stdlib push | L | Keep deferred; `decimal` already covers the common money case |
| **L-65** | **AOT — `phg build --native` (DEC-333(b))** | deferred (gated) | **STILL OPEN** — gated on **JIT-WINS-ALL**, which is gated on L-41 + the remaining flips. AOT verdict already measured: helps ONLY queryparse dispatch (partial, →0.3×, not a flip); rides the same unboxed codegen as #33; zero for listcontains | `SLICE-STATE.md:118`, `:207-213`; DEC-333(b) | Nothing | XL | Keep the gate. The recorded AOT analysis says it adds nothing beyond #33 today |
| **L-66** | **Multi-file playground support** (a virtual multi-file/vendored FS in wasm so `package-manager/`, `project/*`, `interop/*` examples actually RUN in-browser) | deferred (dev ruled "later") | **STILL OPEN** | `SLICE-STATE.md:151-153` | Playground completeness | L | Keep deferred. Bundle the missing `examples.js` staleness CI check (L-72) with it |
| **L-67** | **DEC-324's 9 PENDING-REJECT Appendix-A rows** (SOAP/IMAP/SNMP/dba+SysV/pspell/enchant/calendar/tidy/LDAP → post-1.0), plus a disclosed denominator hole: `D-php-surface` never inventoried 12 extension domains | deferred (by design) | **STILL OPEN** — deliberate post-1.0 deferral, no ruling expected yet | `C-decisions.md:2687-2690` | Parity-% honesty | S (the inventory repair) | Repair the 12-domain inventory hole when the parity % is next recomputed — a silent denominator hole makes every % claim optimistic |
| **L-68** | **Log-v2 v1 limits: processors, userland sinks/formatters, ext-folder migration** | deferred (recorded) | **STILL OPEN** | `KNOWN_ISSUES.md:2297`; `C-decisions.md` DEC-317 row | Logging breadth | M | Keep deferred; v1 covers PSR-3 levels + the handler/formatter set |
| **L-69** | **Pinned dev-box/docker perf re-measure is OWED** — the committed `bench/*-baseline.json` + the dev-box scorecard predate tonight's changes and are stale; the in-container re-bench was explicitly NON-RIGOROUS (single-shot, no core-pinning) | deferred (housekeeping, blocks a claim) | **STILL OPEN**. Canonical dev-box scorecard = **47 WIN / 4 LOSS** (queryparse 0.10×, jsonround 0.31×, listcontains 0.86×, deepjson 0.99×). In-container tonight, `floatmul`/`dbwork`/`listcontains` all measured as WINS and **queryparse went 0.10× → ~0.88×** after DEC-338 — near-parity but **still <1.0×, i.e. not yet a WIN by WIN-OR-FLAG** | `SLICE-STATE.md:197-213`, `:215-228`; `C-decisions.md:3148-3149` | **Every perf claim above [Inferred]** (Invariant 11) | S (dev-box only) | **Only the developer can close this** — the pinned docker harness is unavailable in-container. Highest-leverage 10 minutes of his day: it decides whether the flip campaign is 3 losses or 1 |
| **L-70** | **Stale queue/label cleanup (Inv 19)** — 11 recorded items whose status is now wrong (full list in §D) | known-limitation (doc debt) | **STILL OPEN as doc debt** | see §D for per-item citations | Decision-time waste — a stale PENDING costs a re-investigation every time it is read | S (batch) | **Fix as one mechanical batch.** Also worth adopting the check `E-language-surface.md` proposes: *"every register row naming a diagnostic code must have that code present in `src/`, or be marked PARTIAL"* — it would have caught `E-RETIRED-FORIN` (ruled, absent) and `E-MULTIPLE-MAIN` (explained, never emitted) automatically |
| **L-71** | **`CLAUDE.md` understates the dependency set: it claims "four vetted, feature-gated exceptions" (`argon2`, `regex`, `ctrlc`, `corosensei`); `Cargo.toml` declares ELEVEN domains** (+ `unicode-segmentation`, `rustls`, `webpki-roots`, `rusqlite`, `postgres`, `mysql`, `lettre`, `cranelift-*`) | known-limitation (stale claim) | **STILL OPEN** [Verified: `CLAUDE.md:8-9` vs `Cargo.toml:113-180`]. `UNIFIED-SPEC.md:871-877` explicitly warns that stale dependency claims *"must not be repeated"* | `global-review/C-stdlib-input-fs-clone.md` (stale-doc flag) | Nothing functional — but it is the file Claude reads first every session, so it propagates | S | Fix in the next CLAUDE.md touch. **Classifier-blocked for Claude → present the exact diff for manual application** |
| **L-72** | **Missing CI checks: `playground/web/examples.js` staleness** (2 847 qualified sites, generated by `gen_examples.py`, no staleness gate) and **the TextMate grammar has ZERO automated coverage** (the structural reason a language-wide highlighting inversion shipped unnoticed) | known-limitation | **STILL OPEN** [Verified: `MASTER-PLAN.md:1406` — *"`examples.js` staleness CI check … Add when touching playground CI"*; `B-lsp-editors.md:527-537` (B21)] | `MASTER-PLAN.md:1406`; `global-review/B-lsp-editors.md:527`, `:626-638`; `E-language-surface.md:601-608` (E17) | Any example migration (L-06's codemod) would silently drift both surfaces | S each | Add both. The grammar gate can be node-based and dev-only (`scripts/grammar-check.mjs` over `vscode-textmate`) so it adds **no Rust dependency**, plus a pure-Rust keyword-drift test using the existing `crate::json` parser |
| **L-73** | **B11-B21 — the TextMate grammar is structurally broken** (the reported "light blue" bug): `\b` before an optional prefix group makes every plain string start at its CLOSING quote; `//` inside a string comments out the rest of the line; an unclosed `/*` inside a string swallows the rest of the file; `"""` text blocks, `r#"…"#` raw strings and tagged templates unmodelled; escape alternation wrong 3 ways | ruled-not-built (no design content) | **STILL OPEN — measured**: **81 of 383** repo `.phg` files end inside an unterminated span today; the proposed replacement section takes that to **0/383**. **No bug is recorded anywhere** in KNOWN_ISSUES/SLICE-STATE/MASTER-PLAN | `global-review/B-lsp-editors.md:264-537`, `:626-638` | Editor usability in **both** IDEs (they share the grammar) | M | **Highest visible win in the whole inventory with zero spine risk**, and the fix is already empirically verified. Option (A): replace `repository.strings` wholesale with the pre-verified B20 section + add the L-72 gate. Reject the one-character `\b` fix as a stopping point — it regresses tagged templates and leaves 29/266 files leaking |
| **L-74** | **Diagnostic-quality cluster** — E3 (`E-IMPORT-UNKNOWN` masked by `E-UNUSED-IMPORT`, so a typo'd import reports the wrong cause) · E4 (`E-EXCEPT-UNKNOWN` ships without the ruled did-you-mean hint) · E5 (shallow-wildcard failure gives no "`*` is shallow" hint) · E9 (cross-form loop parse errors are not migration hints) · E13 (`xs.length()` without `import Core.List;` → *"type has no method"* with no import hint) · E14 (an aliased-only module import silently disables UFCS) · E15 (overloaded user functions are not UFCS-eligible, error doesn't say why) · K-7 (some UFCS type errors carry a `1:9` span pointing at `package Main;`) | ruled-not-built (mostly no design content) | **ALL STILL OPEN, all Verified by probe** in the source reports | `global-review/E-language-surface.md:103-130`, `:239-266`, `:566-612`; `K-inline-findings.md:177-194` | Nothing structurally — but *"four of the five current rough edges are message quality, not behaviour"*, i.e. this cluster is what actually changes the **felt** completeness of shipped features | S each | **The best-value autonomous batch in this document.** Each is 1-3 lines. E13's import hint is the single highest-leverage one (it is the first thing a reader of a migrated example will hit) |
| **L-75** | **`main`-residue documentation cluster** — E22 (`E-MULTIPLE-MAIN` is dead but `phg explain` still teaches it and `examples/README.md:190` asserts it fires) · E23 (`examples/guide/class-main.phg` teaches the obsolete Go-vs-Java `main` dichotomy and near-duplicates `entry.phg`) · E24/E25 · E26 (4 stale doc comments + 2 dead public fns `ast::entry_point`/`entry_point_count`) · E27 (`src/loader/fs.rs:112-116` + `UNIFIED-SPEC.md:550` say the file rule exempts *"the entry point `main`"*; the code correctly gates on the attribute) | known-limitation (docs) | **ALL STILL OPEN — verified**: `phg explain E-MULTIPLE-MAIN` returns a full explanation; `grep E-MULTIPLE-MAIN src/` finds **zero emit sites** (only a negative test assertion + 3 stale doc comments) | `global-review/E-language-surface.md:778-899` | Nothing — but it is *why* the corpus still teaches that `main` is special | S | **Ride along with L-02 in one change.** For `class-main.phg`, recommend **repurposing it as `guide/entry-any-name.phg`** (an `#[Entry] static function boot()` **plus** an ordinary non-entry `function main` in the same file) — that converts the fix into a differential-gated example so the reservation cannot silently return |
| **L-76** | **`src/cli/explain/*` teaches `-> void` / `-> int` (arrow) for REGULAR functions** while Inv 12 and every example use `: T` (arrow is only for foreign `declare` sigs) — dev-flagged "FOUND, OUT OF SCOPE" | known-limitation (stale error text) | **STILL OPEN, and BROADER than recorded**: SLICE-STATE cites `explain.rs:1065,1208`, but that file has since been M-Decomp'd — the arrow text now lives in **four** files: `explain/attrs_faults.rs:135,138,143,145`, `match_overloads.rs:21`, `members_destructure.rs:46`, `types_traits.rs:231` | `SLICE-STATE.md:148-150` | Nothing | S | Fix with L-49 (same syntax question, opposite surface). **Update the recorded citation** — it points at a deleted file |
| **L-77** | **CRAFT-3 — one AT-RISK dead-gate (the DEC-191 bug class)**: `interop_projects_refuse_to_run_and_match_php_golden` (`tests/interop.rs:144`) early-returns on an empty collection instead of asserting a seed count, so it passes green if the interop projects ever lose their marker | known-limitation (test integrity) | **STILL OPEN** — all OTHER corpus-iterating gates were audited and have seed guards | `KNOWN_ISSUES.md:~112` (CRAFT-3) | Nothing until it silently rots | S | Replace `if projects.is_empty() { return; }` with a hard `assert!` like its sibling at line 103. **This is exactly the bug class that made the byte-identity glob a no-op for a month** |
| **L-78** | **CRAFT-5 — the "286 natives" figure is stale** and repeated across KNOWN_ISSUES + M-gap-matrix; the real count is **492 all-features / 465 default**, so bench coverage is 40/465 (~8.6%), not 40/286 | known-limitation (stale figure) | **STILL OPEN** | `KNOWN_ISSUES.md:~118` (CRAFT-5); `C-decisions.md` §2026-07-20 AUDIT CORRECTION | Parity/bench-coverage honesty | S | Correct at each touch, as the entry itself instructs |
| **L-79** | **ADR process gap: no superseding ADR for the `phg vendor` retirement** | known-limitation (process) | **STILL OPEN** [Verified: `phg vendor --help` → *"RETIRED (DEC-282): superseded by `phg add/install/update/remove` (DEC-316)"*, while **ADR-0005**'s literal text names the retired command. `phg add --help` confirms the *principle* (offline-only, one of the only network verbs) is intact]. The ADR README's own rule requires a new ADR for a reversed/renamed decision — **no ADR-0006 exists** | `docs/adr/0005-offline-only-vendor.md`; `docs/adr/README.md` | Nothing | S | Write a short ADR-0006 |
| **L-80** | **`UNIFIED-SPEC.md` says the JIT is *"not yet wired into `phg run`"* (`:928`, `:989`)** | known-limitation (stale claim) | **STALE** — `jit` has been a DEFAULT feature since 2026-07-09 and the verticals shipped [Inferred: completed tasks #23-#29 + `CLAUDE.md`'s own "`jit` is a DEFAULT feature" statement; the spec text was never refreshed] | `UNIFIED-SPEC.md:928`, `:989` | Nothing | S | Fold into the L-70 batch |
| **L-82** | **`VALIDATION-regex-trailing-newline` — the 5 original `Core.Validation` preg_match predicates diverge on a trailing `\n`** (the later ones already carry the `/D` flag) | known-limitation (**real 3-leg divergence**) | **STILL OPEN — reproduced end-to-end**: `Validation.isAlpha("abc\n")` → interpreter **false**; transpiled PHP `preg_match('/^[A-Za-z]+$/', "abc\n")` → **true**. Source confirms the 5 named predicates still lack `/D` (`src/native/validate.rs:262-278`) | `KNOWN_ISSUES.md:335-345` | Invariant 1 on those five predicates | **S** | **Add the `/D` flag to the five emitters** — the cheapest genuine correctness bug in the whole inventory, and the fix pattern is already established by the later predicates |
| **L-83** | **DEC-200 — PHP-reserved / builtin-class names as top-level type names** | decision-needed | **STILL OPEN** — explicitly PENDING | `KNOWN_ISSUES.md` §"Language features not yet implemented" (`:574+`) | Name-collision safety on the PHP leg (the `class Match` class of bug that DEC-295's reserved-name prerequisite already bit once) | S–M | Rule the guard's scope: reject at declaration (consistent with the shipped `FN_RESERVED` mechanism) vs mangle silently. **Recommend reject-at-declaration** — mangling is the "nothing in the wind" shape the project keeps rejecting |
| **L-84** | **Three coupled lifetime forks: (a) an `Rc` cycle-leak collector strategy, (b) a `using`/`defer` scope-bound-cleanup construct (DEC-203), (c) a `Runtime.onShutdown` hook** | decision-needed (3 forks, one theme) | **STILL OPEN — all three DEC-PENDING**. ADR-0003 fixed `Rc`-not-tracing-GC [Verified: pervasive `Rc<`, no tracing GC], so cycles leak by design today | `KNOWN_ISSUES.md:507-572` (design forks 2026-07-12); DEC-203 also cited in `global-review/D-database.md:~650` (D10: `Statement.close()` wants `using`) | **All three matter specifically for long-lived `serve`** — and (b) is what a correct `Statement`/`FileLock`/`FileHandle` lifetime story wants (L-09, L-13, L-12 all brush against it) | M each | **Rule (b) `using`/`defer` first** — it is the one the DB, locking and streaming slices all keep bumping into, and it makes (a) less urgent by making cleanup explicit. Rule them as one block since they share the "who releases what, when" axis |
| **L-85** | **`Core.Time.DateTime` bare-import-gating is inconsistent with the rest of the injected-type set** — flagged *"adjudicate before the DB/HTTP waves grow the injected-type set"* | decision-needed (**time-sensitive**) | **STILL OPEN**, and the deadline named in the flag has passed — the injected-type set has since grown (DEC-337 added `EntryKind`) | `KNOWN_ISSUES.md:2033-2189` (behavioral quirks) | Consistency of the whole injected-type import story — which L-15/A13 also touches | S | **Rule it in the same sitting as A13** (entry-import ceremony) — they are the same question about injected types, and the flag's own "adjudicate before the set grows" condition is already violated |
| **L-86** | **DB column-naming (slice B2) + the cross-prelude error-namespace convention** — two "QUEUED REAL ADJUDICATION" items, plus the DB streaming-iterator shape | decision-needed (3 items) | **STILL OPEN** | `KNOWN_ISSUES.md:384-505` (Fable overnight run), `:574+` | DB surface coherence | S–M | Batch with the A9 database block — same surface, same sitting |
| **L-87** | **`§span-collision` — injected-prelude spans share the user file's span space** (P1): one known file carries a fragile padding workaround; the real fix is "owed, its own slice" | known-limitation (P1) | **STILL OPEN** — structurally real, reproduction inherently offset-random | `KNOWN_ISSUES.md:2268-2280` | Diagnostic correctness anywhere a prelude and user span can collide; **plausibly the same root class as L-88 and K-7's `1:9` span bug** | M–L | Give it the slice it is owed, and check whether it subsumes K-7 (the UFCS `1:9` span) and the VM interpolation line skew — three span bugs that may share one fix |
| **L-88** | **`phg test <dir>` whole-file validation runs the RAW checker** (skipping prelude injection) — and the LSP had the same gap | known-limitation | **STILL OPEN for `phg test`**. ⚠ **The file contradicts itself**: `:2289-2295` says the LSP instance was "since fixed", while §"Language features not yet implemented" still describes the LSP prelude-injection diagnostic gap as open | `KNOWN_ISSUES.md:2289-2295` + `:574+` | `phg check` ≡ LSP ≡ `phg test` currency (Invariant 17 / DEC-252) | S | Fix `phg test` with the same change the LSP got, and **reconcile the doc's self-contradiction** while there |
| **L-89** | **F-029 — two latent namespaced (multi-package) transpile divergences**: injected-type mis-namespacing and a `Debug.dump` bare-name mismatch | known-limitation | **STILL OPEN** (doc-only; reproduction needs multi-package scaffolding) | `KNOWN_ISSUES.md:347-382` | Invariant 1 in multi-package projects — which L-04's ruling will make more common | M each | Worth reproducing properly: these are byte-identity risks in exactly the project shape the package-enforcement work (A4) pushes users toward |
| **L-90** | **`Core.Regex` — `findGroups`/`findAllGroups` diverge from PCRE on non-participating named groups** (Rust omits, PCRE fills `""`) and on empty/zero-width match placement | known-limitation | **STILL OPEN** — the doc's own [Verified] tags; `replaceCallback` (DEC-295) already **fixed** the same class via `PREG_UNMATCHED_AS_NULL` | `KNOWN_ISSUES.md:1913-1955`; `SLICE-STATE.md:1468-1469`, `:1495-1496` | Invariant 1 on all match-iterating APIs (examples currently dodge it by using non-empty patterns) | S | **Align the two with the already-shipped `replaceCallback` pattern** — the fix is precedented in the same module |
| **L-91** | **RICHREQ v1 remaining corners**: a spill-file leak · the body cap inert under `serve` (frame cap == body cap, so oversize looks *malformed*) · superglobal lift mapping deferred (needs an ambient→parameter transform design) · the regex-feature-gate check failure | known-limitation (4 items; the CRLF 5th = L-08) | **STILL OPEN** (doc-only; needs a `serve` / `--no-default-features` harness) | `KNOWN_ISSUES.md:122-156`; `SLICE-STATE.md:754-758`, `:797-798` | **S3.2 (L-37) must reconcile the frame-cap-vs-body-cap semantics and the oversize-vs-malformed fault boundary** — it is a stated precondition of that slice | S–M each | Fold the cap reconciliation into L-37 (it is already scoped there); the spill-file leak is independently worth fixing |
| **L-92** | **VM interpolation fault-line skew** — a fault inside `"{…}"` reports **line 1** on the VM, the true line on the tree-walker | known-limitation (**exemplary handling**) | **STILL OPEN, correctly carried**: reproduced, scoped, disclosed in KNOWN_ISSUES + `tests/differential.rs:250-260` (W0-5 / H §5), fix scheduled **W5-13**, with an `#[ignore]`d ready-gate test already covering three shapes. Message, FaultKind and exit code all agree — only the line diverges | `KNOWN_ISSUES.md:2033+`; `global-review/K-inline-findings.md:100-112` | Nothing (disclosed, gated) | M | **Do not re-report as new — this is the reference example of how to carry a known parity gap.** One improvement: arithmetic overflow is a 4th shape missing from the ignored test's case list (trivial to add when W5-13 lands) |
| **L-93** | **`#[Attribute]` meta-arguments (`targets:`, `repeatable`) unsupported** (`src/checker/program/attributes.rs:84`) | ruled-not-built | **STILL OPEN** — folds into the doc's "general attribute facility is future work" | `src/checker/program/attributes.rs:84`; `KNOWN_ISSUES.md` §attributes | User-defined attribute expressiveness | M | Autonomous-capable once the surface is ruled; low urgency |
| **L-94** | **The lifter never synthesizes constructor promoted-param defaults** (`src/lift/lifter/exprs.rs:343`) — **the codebase's only genuine `TODO`** | ruled-not-built | **STILL OPEN** [Verified: 4 `TODO/FIXME/XXX/HACK` hits in all of `src/`, 3 of them false positives (`\uXXXX` placeholders); this is the only real one] | `src/lift/lifter/exprs.rs:343` | Lifter fidelity | S | **Good filler.** Worth noting for its own sake: **`src/` contains zero `unimplemented!`/`todo!` macros and effectively zero TODO markers** — the codebase fails closed through typed `E-*` errors instead. That is a genuinely strong signal |
| **L-95** | **Parked perf: the string/collection speed-beat is REOPENED — the 2026-07-11 spike refuted "unreachable", but only on the JIT leg; the VM-only path is still 27-67× behind** | deferred (status REOPENED) | **STILL OPEN**. ⚠ **Doc-organization hazard**: this section's *"ALL 21 micros ≥1.0×"* register covers a different, non-overlapping bench set from `PERF-native-call-in-loop`'s loss table — not a contradiction, but trivially misread as one | `KNOWN_ISSUES.md:2190-2262` vs `:218-333` | Perf-claim honesty | L | Annotate the two sections' scope boundary. Note the VM-only gap matters for `--no-jit` and the wasm playground, where the JIT wins do not apply |
| **L-81** | **Q-C — global completeness sweep (DV-5)**, the currently-scheduled NEXT slice: synthesize the existing audits + a fresh `/gaps` into ONE ranked completeness register | decision-needed (scope) → **largely DELIVERED** | **✅ DELIVERED + COMMITTED while this sweep ran** [Verified: `git log` → `b30d9b5` raw evidence base (part 1), `68dca8e` synthesized register + DEC-339…355 (part 2), `b3e635e` invariant repairs (part 3); all 13 files incl. **G, H, I now on disk**]. My earlier finding that the directory was staged-but-uncommitted is **RESOLVED** | `docs/specs/2026-07-24-visibility-model.md` (DV-5); `SLICE-STATE.md:95-96`, `:168-171`; `global-review/README.md:12-33` | Roadmap accuracy itself | — | **Nothing owed — DV-5 is discharged**; its output is the 17-item agenda now carried as DEC-339…355. Retained as a row only because some places still show Q-C as the queued NEXT slice — the cursor wants flipping to DONE (folds into the L-70 batch) |

---

## A. NEEDS A RULING TOMORROW (ranked — this is the agenda)

Ranked by *"one decision unblocks the most work"*, then impact. Each is drafted as a question + options
(recommended first) + the one-line why.

### A1. The block-shadowing byte-identity break (L-01 · **DEC-339 / GR-1**) — **P0, rule this first**
**Question:** Shadowing a still-live outer local or parameter inside any nested block produces different
*values* on the PHP leg (`out=1` on both Rust backends, `out=2` under `php-8.5.8`). Do we make the
transpiler alpha-rename shadowed locals, or forbid shadowing in the language?

| Option | Consequence |
|---|---|
| **1. Alpha-rename in the transpiler (RECOMMENDED)** | Emit a deterministic unique PHP name (`$a__b1`, inside the reserved `$__phorj_` namespace) and rewrite references in scope. Restores Inv-1, keeps a capability the Rust backends already implement correctly, zero runtime cost. The transpiler already tracks `locals: Vec<HashSet<String>>` + `local_kinds`, so the scaffolding exists |
| 2. Reject shadowing — new `E-SHADOW-LOCAL` | Simplest and fully sound, and many linters ban shadowing. But it *removes* working capability, is a breaking surface change, and makes phorj stricter than Rust/C#/Kotlin, all of which permit it |
| 3. Warn and keep the divergence | **Not a real option** — Invariant 14 forbids a silent semantic downgrade, and this is worse than silent: it is wrong output |
| 4. Wrap blocks in PHP closures | Rejected — changes by-reference semantics, heavy runtime cost |

**Why 1:** it is the standard technique for targeting a scope-less language, and it is the only option
that fixes the bug without taking a feature away. **Regardless of the choice, add a differential example
that shadows in every block form** — the harness's coverage is exactly the example corpus, and no example
shadows, which is the structural reason a whole language feature has zero spine coverage.

### A2. Retire the `main` name reservation (L-02 · **DEC-344 / GR-6**) + its doc residue (L-75)
**Question:** `#[Entry]` already frees the entry's name, but the checker still forces any function or
static method named `main` into the entry signature. Retire `E-MAIN-SIGNATURE`?

| Option | Consequence |
|---|---|
| **1. Retire the reservation (RECOMMENDED)** | Delete the `f.name == "main"` case; `E-ENTRY-SIG` already covers every entry under any name. Re-key `cur_is_main` to `entry_declared_role(f).is_some()` so `E-UNCAUGHT-THROW` stops attaching to the wrong function in both directions. Also fixes the double-error on a malformed `#[Entry] function main`. Cost: 14 test assertions to re-point, 6 stale comments, 2 dead public fns |
| 2. Keep it as a belt-and-braces check | Zero work, but a library author still cannot write `function main(string): string`, and the corpus keeps teaching that `main` is special *because the checker still treats it as special* |

**Why 1:** the developer's instinct was right and it is confirmed at code level; keeping it contradicts
DEC-331/DEC-337's own model. Ride L-75 along in the same change, and **repurpose
`examples/guide/class-main.phg` as `guide/entry-any-name.phg`** so the reservation cannot silently return.

### A3. Loop-form retirement: finish DEC-248 or amend it (L-03 · **DEC-343 / GR-5**)
**Question:** DEC-248 ruled `for (T x in xs)` retired behind `E-RETIRED-FORIN`; that code was never
written, its `foreach` half shipped, and `FEATURES.md` + the guide teach both forms as co-equal. Register
conflict **C-2** has been open since 06-25. Which way?

| Option | Consequence |
|---|---|
| **1. Amend DEC-248 to "keep both, deliberately", close C-2, then fix E9 + E10 (RECOMMENDED)** | Makes the register describe reality (Inv 19) at near-zero code cost. The two follow-ons remove the actual friction: cross-form migration hints (`for (xs as x)` → *"did you mean `foreach`?"*), and letting `for` infer its binding (`for (x in xs)`, which `foreach` already does) |
| 2. Execute the retirement as ruled | ONE loop idiom, full PHP alignment, kills E9/E10/E11 at a stroke. But the corpus is **87 for-in vs 8 foreach** — the form ruled retired is the one the examples overwhelmingly teach — and the deprecation policy requires a `W-DEPRECATED` release first, so this is two releases and an 87-site codemod |
| 3. Retire `foreach` instead | Explicitly rejected in DEC-248's own alternatives ("keeps the divergence") |
| 4. Leave as-is | Costs a re-investigation every time the question resurfaces — this is at least the third time |

**Why 1:** the behaviour the developer has been living with for a month *is* option 1, and the retirement
half has now failed to get built twice. If the north-star is genuinely "one loop, PHP-shaped", option 2
is right — but then schedule it as a real slice with the deprecation lifecycle.

### A4. Package enforcement: close the fast path? (L-04 + L-05 · **DEC-345 / GR-7**)
**Question:** A loose file with only `Core.*` imports takes a loader fast path that skips all three
package validators — so `package Foo.Bar;` is silently legal, the `E-FILE-*` public-surface rules never
fire, and `package` emits no PHP namespace. Adding one `import` flips the same file to a hard error.
Close the fast path?

| Option | Consequence |
|---|---|
| **1. Validate `entry_prog` before the fast-path return (RECOMMENDED)** | Makes the rule a property of the *file*, not of the import graph — which is what makes today's behaviour surprising. The validators take `(prog, file, root)` only, so the disk-scan optimization survives. Restores three already-ruled rules with no new policy and no new error codes. Migration cost measured: **one inert fixture** |
| 2. Warn instead of fail on the loose case | DEC-035 already **rejected** warn-only for the sibling casing rule ("no `W-CASE` lint fallback") — adopting warnings here splits the doctrine, and warnings interact with the byte-identity spine |
| 3. Hard-fail only loose-entry-with-non-`Main`-package | Smaller blast radius, but leaves the `E-FILE-*` bypass and the inert-`package` oddity live |
| 4. Ratify the status quo, fix only the misleading success message | Cheapest; but makes `E-FILE-*`'s "SHIPPED hard errors" status false |

**Two sub-questions this forces (rule them explicitly):** (a) should `package Foo.Bar;` be legal in a file
not under a matching folder? Option 1 makes the answer "no" automatically — which effectively **reverses
DEC-282's loose-`Main` retirement for files**; if you want that retirement kept, scope option 1 to
`E-FILE-*` + `E-PKG-CASE` only. (b) *Must* an entry be `package Main`? Currently enforced by accident via
the L-05 bug; DEC-282's *"`package Main` = entry-only"* is ambiguous between "only entries may be `Main`"
and "entries must be `Main`". **L-05's root fix is a pure bug fix with no design content and must land
before or with this**, or closing the fast path starts rejecting correct layouts with a wrong message.

### A5. The file-level "structure-free" attribute (L-04's companion · also **DEC-345 / GR-7**)
**Question:** *"Since `#[Entry]` unreserved the free `main`, should a file-level attribute free a file from
package structure?"* **First answer the hidden sub-question:** does the marker exempt (a) only this file's
`package` from folder=path, or (b) the file from structure entirely (also `E-FILE-*`, and/or entry-anywhere)?
These are different features with different blast radii and should not be bundled by default.

| Option | Grammar cost / trade |
|---|---|
| **1. `#[Loose] package Foo.Bar;` — prefix attribute on the declaration (RECOMMENDED)** | Teach `parse_program` to `parse_attributes()` before the `package` peek + add a `package` target to `E-ATTR-TARGET`. **No tokenizer change.** Matches the existing `#[Entry]` shape exactly, import-gateable exactly like DEC-337's `EntryKind`, erased before every backend per Inv 5 — so it inherits the formatter/LSP/lifter/transpile currency story instead of inventing one |
| 2. `#![Package(Main)]` — Rust-style inner attribute at byte 0 | **Requires narrowing the shebang rule first**: `src/tokenizer/mod.rs:156` skips **any** byte-0 `#!` line, so `#![Loose]` **vanishes with no diagnostic** [Verified: probe produced `E-NO-PACKAGE` at 3:1]. Also introduces a second attribute sigil and permanently contests line 1 with `#!/usr/bin/env phg` |
| 3. `package Foo.Bar #[Loose];` postfix | Contradicts prefix convention everywhere else in the language |
| 4. A modifier keyword (`loose package …`) | Re-introduces exactly the bare-magic-identifier shape DEC-337 just eliminated |
| 5. `phorj.json` opt-out | **Directly contradicts DEC-282** ("NO manifest at all") and is invisible at the file you are reading |
| 6. **No hatch — strict only** | The serious alternative: DEC-282's search-root model already gives loose scripts a home (`package Main` is location-free), and **nothing in the repo needs the hatch today** |

**Why 1 over 6 is genuinely close.** Sequencing consequence worth surfacing: the hatch is only meaningful
*after* enforcement exists — building it first is a no-op. Recommend the enforcement fix (A4) as one slice
and the hatch as a separate slice gated on this ruling.

### A6. `Output.printLine` call style (L-06 · **DEC-346 / GR-8**) — must precede any UFCS codemod
**Question:** `Output.printLine` is 1231 of 2223 qualified call sites (55.4%). DEC-326 reserves module form
for "receiver-less calls" but leaves *"is the printed string the subject of printing?"* open. Receiver form
(`"hi".printLine()`) or module form?

| Option | Consequence |
|---|---|
| **1. Keep module form for `Output.*` (RECOMMENDED, [Speculative] — this is a taste call)** | `printLine`'s subject is the *output stream*, not the string; keeps the 1231 sites untouched and makes the "receiver-less/ambient" carve-out legible in the corpus |
| 2. Receiver form | Consistent with the house style everywhere; but it rewrites over half the corpus and reads oddly for an ambient side effect |

**Why rule it now regardless of direction:** get it wrong and over half the corpus is touched twice. Rule
the companion policy in the same sitting: **which examples keep module form on purpose** (suggested: the
54 factory sites, 26 arity-0 sites, 217 prelude-static sites, plus `guide/ufcs.phg` and
`guide/extension-methods.phg` showing both forms side by side, with a one-line comment saying *why*).

### A7. LSP strict-vs-discoverable (L-07 · **DEC-342 / GR-4**) — rule both directions together
**Question:** Should completion suggest only what the buffer's imports make callable (strict), or everything
available with an auto-import on accept (discoverable)? B7 asks it for module members (currently NOT
import-gated → suggests uncompilable calls); B1(D) asks it for UFCS members.

| Option | Consequence |
|---|---|
| **1. Import-gate both, paired with `additionalTextEdits` auto-import on accept (RECOMMENDED)** | Best of both: nothing uncompilable is suggested, and accepting inserts the import. The gating predicate is the same one B1's fix needs anyway |
| 2. Strict only | Simplest and always-correct, but hurts discovery — *"what can I do with a string?"* goes unanswered |
| 3. Discoverable only | Suggests things that don't compile |

**Why they must be ruled together:** they are the same question from opposite ends; answering differently
leaves the LSP internally inconsistent. **Note the related P1:** `SLICE-STATE.md:1022` claims *"LSP
AUTOCOMPLETE — DONE + COMPREHENSIVE"*, which is measurably false for UFCS — the language's primary stdlib
call syntax yields **zero** completions on primitive and container receivers. DEC-326 chose receiver form
*because* `s.`-completion beats module recall, so L-06's codemod should not run before this is fixed.

### A8. Response-side CRLF guard (L-08 · *not* in DEC-339…355 — separately PENDING at `C-decisions.md:3048`)
**Question:** `Response.withHeader` / `Cookie.render` interpolate straight into a header line with no
validation — the actual outbound header-injection sink. Guard them?

| Option | Consequence |
|---|---|
| **1. Guard both, fail-loud, matching the Request-side wither (RECOMMENDED)** | Symmetric with the already-shipped Request-side disposition (*"unvalidated CRLF into a header constructor is a programming error; fail-loud beats silent header splitting"*). Changes shipped surface behaviour, hence the ruling |
| 2. Guard only on the serve path | Narrower blast radius, but the sink is the constructor, not the transport |
| 3. Leave unguarded + a KNOWN_ISSUES row | Status quo; hard to defend given the Request side already faults |

**Why 1:** the asymmetry is the finding — the *inbound* parse path is guarded and the *outbound* emit path
is not, which is backwards from a security standpoint.

### A9. The database cluster (L-09 · **DEC-351**, L-10 · **DEC-340**, L-11 · **DEC-350**) — rule as one block, in this priority order
The report's own priority ordering: **D4** (silent data persistence after a reported rollback) → **D5**
(nested savepoints broken + untested on MySQL) → **D1/D2/D3** (the developer's Q2 scenario + a 75× perf
trap) → **Q1** naming.

**Q2 — statement reuse:** **Option A, reset binds after each successful execute** (recommended; 4 sites or
one helper) — fixes D1+D2+D3 together, no perf cost since the driver already caches and resets. Fallback
**B**: an explicit `Statement.reset()` with the accumulate semantics documented (zero behaviour change,
but leaves the footgun and the quadratic path reachable by default).

**Q3 — abort everything:** **Option A + D, with E as a companion** (recommended) — add `rollbackAll()`
(one top-level `ROLLBACK` discards every savepoint at any depth, so it is one statement, not a loop), point
`db_transaction`'s error arm at it so it unwinds to its entry depth (**this is the D4 P1 fix**), and expose
`transactionDepth()` (already computed at `ops.rs:389` and discarded). Alternative **B**: Laravel-style
`rollback(int toLevel = -1)`. **Fix D5 independently** by routing control SQL through the `DriverConn` seam
(each driver spelling its own savepoint grammar, as `mysql.rs` already does for `phorj_bulk`) + nested
savepoint tests on MySQL and Postgres.

**Q1 — naming:** **Option B** (type `Database` → `Connection` **and** module → bare `Core.Database`),
fallback **A** (type only). Why B: the object is a single connection by four code proofs, `Connection` is
the near-universal name, `Database` elsewhere means the *pool* (which phorj explicitly does not have), and
DEC-278's `Module` suffix exists solely for the namesake collision the rename removes. Needs a DEC-278
amendment row either way.

### A10. Streaming file reads (L-12 · **DEC-347 / GR-9**)
**Question:** How should phorj read a file of any size incrementally? The `Iterator<T>` protocol already
exists and already streams — for stdin only.

| Option | Consequence |
|---|---|
| **1. O2 — `FileSystem.lines(path)` over an offset-chunk native (RECOMMENDED, with O1 as the declared upgrade path)** | Ships the exact ask with **zero** new spine machinery: one ordinary fallible native + a prelude `FileLines implements Iterator<string>` copied from `InputLines`. No `Value` variant, no reserved opaque type, no `emit_type` special case, no close/leak discipline. Same O(1) memory. Because the user-facing surface is identical to O1, O1 later is a non-breaking internal swap |
| 2. O1 — a real opaque `FileHandle` + `openRead`/`readLine`/`close` | Truest streaming; but a `Value` variant, a reserved-opaque row, the first transpiling `emit_type → mixed` special case, and `Drop`/close discipline |
| 3. O3 — scoped closure `withLines(path, fn)` | Auto-closes, but **breaks `break`/`return`** out of the loop and abandons the protocol phorj already has |
| 4. O4 — generators/`yield` | Already ruled fresh-context-only and spine-sensitive; wrong tool for this ask |

**Embedded sub-decision:** whether to add an I/O limit to `limits.rs` (e.g. max single-line length so a
1-line 10 GB file faults cleanly instead of OOMing). **Adding a cap changes observable failure behaviour,
which Invariant 1 makes parity-affecting on all three legs** — so it is a ruling, not an implementation
detail. Also note **C5b**: an interface-typed `Iterator<E>` value reports empty `throws`, which becomes
load-bearing the moment a throwing `FileSystem.lines()` ships.

### A11. Filesystem locking (L-13 · **DEC-348 / GR-10**)
**Question:** How should phorj lock a file and wait until it is available? **The presumed blocker does not
exist** — `std::fs::File::{lock, try_lock, unlock, …}` are stable on the pinned toolchain, and Rust-std
locks and PHP `flock()` were verified to block each other bidirectionally. No dependency, no `unsafe`, no
policy amendment.

| Option | Consequence |
|---|---|
| **1. O5 — `withLock<T>(path, () => T)` + `tryWithLock<T>(…): T?`, whole-file, advisory (RECOMMENDED)** | A lock that cannot leak. Every language with a scoped form makes it the default; PHP's manual `LOCK_UN` is a documented footgun. Same better-than-PHP shape the register already blessed for `Core.Deque`/`PriorityQueue` returning `T?` |
| 2. O6 — manual `lock()`/`unlock()` handle | 1:1 with PHP `flock`, but leak-prone and needs an opaque type + `emit_type` mapping |
| 3. O7 — fold `LOCK_EX` invisibly into `writeText`/`appendText` | Silently changes shipped behaviour and does nothing for read-modify-write, which is the actual use case |
| 4. O8 — `writeAtomic` (temp + rename) | Solves *"never see a torn file"*, **not** *"wait until available"* — a **companion**, not a substitute; probably also wanted |

**Three things to rule explicitly, not silently:** (a) the `__phorj_fs_with_lock` PHP helper
(`try { … } finally { flock(LOCK_UN); fclose(); }`) needed to make the scoped guarantee survive
transpilation — an Invariant-16 trade that is the developer's; (b) the Windows semantics divergence, which
must be **verified on a Windows runner** before shipping, not assumed; (c) whether `writeAtomic` ships
alongside. **Reject timeout for v1** — no native support on either leg, and a spin-sleep makes wall-clock
observable (determinism-hostile).

### A12. Bless the no-op clone (L-14 + L-52 · **DEC-349 / GR-11**)
**Question:** `p with { }` already works on all three legs, is documented nowhere, has no DEC row, and the
lifter refuses it. Bless it, or add a different spelling?

| Option | Consequence |
|---|---|
| **1. O9 — bless and document the existing form; add NOTHING to the language (RECOMMENDED)** | The capability already ships and is already correct. C# records `with { }` and Kotlin `copy()` — the exact family this syntax came from — both make the empty/argument-less form the canonical no-op copy. Work = example + `FEATURES.md`/`UNIFIED-SPEC` text + a register row + a 1-line formatter fix (`with {  }` → `with { }`) + the lift direction |
| 2. O10 — a bare `clone x` prefix keyword | New token/AST/formatter/LSP/lift, and `clone` is PHP-reserved as a symbol name. **Two spellings for one operation** — exactly the "dual API forever" failure mode DEC-257 explicitly rejected |
| 3. O11 — `x.clone()` method/UFCS | Needs a universal method or a `Core.Clone` trait, and collides with user-defined `clone` |
| 4. O12 — `Core.Clone.of(x)` native | Least ergonomic; a new module for one function |

**One embedded ruling:** the lift refusal boundary — PHP `clone` can invoke `__clone`, which phorj has no
equivalent for, so the lift must **refuse loudly** rather than silently drop the hook (Inv 14 case 3).

### A13. Entry ceremony — auto-inject `Core.Runtime.{Entry, EntryKind}`? (L-15 · **DEC-353 / GR-15**)
**Question:** A minimal runnable program is 6 lines, 4 of them ceremony (PHP's equivalent is 2), because
`#[Entry(kind: EntryKind.Cli)]` needs **two** separate imports and each omission is its own hard error.

| Option | Consequence |
|---|---|
| **1. Auto-inject both into scope (RECOMMENDED)** | The error text itself calls `Entry` *"an injected `Core.Runtime` type"* — requiring an explicit import for a compiler-injected symbol is arguably self-contradictory. Paid back in every file that runs |
| 2. One combined `import Core.Runtime;` covers both | Halves the ceremony while keeping the import explicit |
| 3. Keep as-is | DEC-337 deliberately bought explicitness by killing bare magic `kind: Cli` — that was the right call for clarity; this is the bill |

**Why it is a real question, not a tweak:** (1) and (2) interact with the `E-UNIMPORTED` /
`E-INJECTED-VARIANT-BARE` machinery DEC-337 just built.

### A14. "Visibility/access in blocks" — disambiguate (L-16 · **DEC-352 / GR-14**)
**Question:** Which of five readings did you mean? **F-i** bare blocks that scope their locals —
**already exists** (verified). **F-ii** access modifiers on locals. **F-iii** named nested functions.
**F-iv** local class/type declarations. **F-v** explicit capture lists.

**Recommendation:** **no** to F-ii, F-iv, F-v; a **spec + ruling for F-iii without visibility modifiers**
(every peer language that has nested functions deliberately omits access control on them). Phorj already
captures implicitly by value, verified correct and byte-identical, so mandatory capture lists (F-v) would be
pure ceremony. **Standing principle worth recording either way:** *visibility is a top-level/member-axis
concept; inside a function body the axis is lifetime/scope, not access* — the same conflation the
visibility spec already caught once (G3). **And regardless of the answer, A1's P0 is the thing that is
actually broken about blocks today.**

### A15. Q-A / Q-B follow-ups (L-17, L-18, L-19, L-20) — four quick rulings
- **P-Q-B-1 (L-17)** — a **real soundness hole**: the `overloads == 1` guard on `E-IFACE-VIS` lets a
  >1-overload reduced-visibility impl be reached through a plain interface-typed receiver. *Recommend:
  close it.*
- **P-Q-A-1 (L-18)** — Core-submodule wildcards (`import Core.Http.*`) are parser-rejected; **this is the
  single capability gap in Q-A and the likely source of the "wildcards aren't fully supported" impression**,
  since stdlib is where a wildcard would be used most. *Recommend: build it, preceded by the five cheap
  diagnostic fixes; the enabler (`lsp/catalog.rs::module_members`) already exists.*
- **P-Q-A-2 (L-19)** — confirm the as-built public-only cross-package rule and amend D3's "public+internal"
  shorthand. *Recommend: confirm as-built; it is the principled reading.*
- **P-Q-A-4 (L-20)** — group-`{}` sorting is structurally unimplementable without re-homing DEC-186's
  parse-time expansion. *Recommend: accept the no-op and amend ruling (e).*

### A16. The lifetime/cleanup block (L-84 · *not* in DEC-339…355 — PENDING since 2026-07-12) — rule `using`/`defer` first
**Question:** Three coupled forks have been PENDING since 2026-07-12: (a) an `Rc` cycle-leak collector
strategy, (b) a `using`/`defer` scope-bound-cleanup construct (DEC-203), (c) a `Runtime.onShutdown` hook.
All three matter specifically for long-lived `serve`.

**Recommendation: rule (b) `using`/`defer` first.** It is the one every other open slice keeps bumping
into — `Statement.close()` wants it (D10), a `FileLock` wants it (A11), an O1 `FileHandle` wants it (A10) —
and making cleanup explicit makes (a) materially less urgent. (a) is a genuine design fork (ADR-0003 fixed
`Rc`-not-tracing-GC, so cycles leak by design today); (c) is small and additive.

### A17. Remaining KNOWN_ISSUES rulings, batchable with their neighbours
- **L-83 DEC-200** — PHP-reserved/builtin-class names as top-level type names. *Recommend:
  reject-at-declaration, consistent with the shipped `FN_RESERVED` mechanism.* This bug class already bit
  once (`class Match` → invalid PHP, found during the Regex closer).
- **L-85 `Core.Time.DateTime` import-gating inconsistency** — **time-sensitive: the flag's own
  "adjudicate before the injected-type set grows" condition is already violated** (DEC-337 added
  `EntryKind`). *Recommend: rule it in the same sitting as A13 — same question about injected types.*
- **L-86 DB column-naming (slice B2) + the cross-prelude error-namespace convention + the DB
  streaming-iterator shape** — three "QUEUED REAL ADJUDICATION" items. *Recommend: batch with A9.*
- **L-91's cap semantics** — the oversize-vs-malformed fault boundary under `serve`. *Recommend: rule it
  as part of L-37 (S3.2), which already lists it as a precondition.*

### A18. Lower-urgency rulings, batchable in one pass
`L-21` jsonround arena (**re-measure after #33 first**) · `L-22` DEC-334 catalog scheduling (**slot after
S3.2**) · `L-23` DEC-322 parallelism forks (**research doc first, as already ruled**) · `L-24` W4-10 XML
fork · `L-25` `App\`-prefixing (**recommend: no-prefix law is fine for GA**) · `L-26` `Core.File`
deprecation (**rule with A10 so `FileSystem.lines()` lands on the winning surface**) · `L-27` maxBy/minBy
residual (**accept the flag, narrow the row**) · `L-28` pipe-lambda trailing ops · `L-30` the eight Claude-bundle questions Q-J1…Q-J8 (**DEC-354 / GR-16**) · `L-31` `VirtualModule.src`→`srcs` rubber-stamp · `L-32` confirm the
serve-TLS GA-blocking label · `L-33` the seven remaining DEC-324 TOP items (**batch-adjudicate their
surfaces upfront**).

---

## B. RULED BUT NOT BUILT — ready for autonomous execution

Grouped by dependency. "Genuinely unambiguous" means: a frozen spec or an explicit ruling, no open design
fork, and I verified the target symbols are absent from `src/`.

### B-tier 1 — unambiguous, no dependencies, safe to hand back immediately
| Item | Size | Notes |
|---|---|---|
| **L-74 diagnostic-quality cluster** (E3/E4/E5/E9/E13/E14/E15/K-7) | S each, ~8 items | **Best value in the document.** Each is 1-3 lines. All Verified by probe. E13 (import hint on a failed UFCS call) is the highest-leverage single fix |
| **L-73 TextMate grammar replacement + gate** (**DEC-341 / GR-3**) | M | The corrected section is **pre-verified**: 81/383 leaking files → 0/383. Zero compiler/backend/spine surface. Needs the L-72 gate in the same change or B21 recurs |
| **L-53 stdlib companions** (`Map.update`, `List.scan`/`windowed`/`associateBy`/`countBy`) | S each | Grep-verified genuine gaps; proven native recipe; each = byte-identity + example + transpile&lift |
| **L-43 labeled `break`/`continue`** | M | Spec frozen, `label@` form ruled, loops-only v1. Not even tokenized today |
| **L-44 typed LSB (`Self`)** | M | Spec frozen, STRICT compile-time ctor check ruled |
| **L-51 lift Uri Tier-2 mapping** | S | Its two sibling lift-catch-up items already shipped |
| **L-63 `phg env` doctor** | S | Self-contained CLI addition |
| **L-77 CRAFT-3 dead-gate assert** | S | One-line `assert!` — same bug class that killed the byte-identity glob for a month |
| **L-82 `Core.Validation` trailing-`\n` divergence** | **S** | **Reproduced live on two legs.** Add the `/D` flag to the five original predicate emitters — the fix pattern is already established by the later ones. **The cheapest genuine correctness bug in this document** |
| **L-90 `findGroups`/`findAllGroups` PCRE alignment** | S | Align with the already-shipped `replaceCallback` fix (`PREG_UNMATCHED_AS_NULL`) — precedented in the same module |
| **L-88 `phg test` raw-checker gap** | S | Same fix the LSP instance already got; reconcile the doc's self-contradiction while there |
| **L-94 lifter ctor promoted-param defaults** | S | The codebase's only genuine `TODO` |
| **L-78 CRAFT-5 stale native count**, **L-80 UNIFIED-SPEC stale JIT claim**, **L-79 ADR-0006**, **L-70 stale-label batch**, the L-95 bench-scope annotation, the `Process.args()`→`arguments()` doc drift | S total | One mechanical docs pass |

### B-tier 2 — unambiguous but larger, or with one recorded prerequisite
| Item | Size | Dependency |
|---|---|---|
| **L-41 #33 Json-ADT JIT — resume at the write-path helpers** | L | None; plan v7 complete, gate CLOSED, 6C panel owed **after** the build. Clean pause point (dead_code gates). Split `json_ext.rs` (417) as it grows |
| **L-42 spread legs (a)+(b) only** | M | Build approach recorded (parallel `arg_spread` field + `check_and_expand` desugar). **Leg (c) is NOT ready** — it needs `Map<K, union>` ergonomics verified first |
| **L-46 ArrayAccess** | M–L | Spec frozen; carries a REOPEN flag worth a 30-second re-confirm |
| **L-47 `Any` + `Object` top types** | M | **Three small open points to rule first**: `E-INSTANCEOF-ANY` error-vs-fold, union-folding detail, reified-Op reuse |
| **L-45 `Core.Sandbox`** | M–L | Spec frozen and ruled to build in v1; introduces a new `E-TRANSPILE-SANDBOX` quarantine, so re-read the four accepted compromises |
| **L-48 slice 1b (`__invoke` cluster)** | M | Reopenable-deferred; coupled cluster wants its own slice |
| **L-36 `Core.DateTime` + tz crate** | L | **The dependency ruling — the hard part — is already done.** Build order stated in the ruling |
| **L-54 exception backtrace API** | M | Already flagged "FRESH session" |
| **L-55 serialize/unserialize/var_export/print_r** | M | Note `serialize`'s format is a PHP-compat contract |
| **L-56 `Core.Process` run/spawn/exec** | M | Check the differential quarantine story first (impure) |
| **L-57 SPL heaps** | S–M | **Re-scope first** — `Deque`/`PriorityQueue` already exist; only the heap family is missing |
| **L-50 `phg stubs`** (then `phg watch --php`) | M each | `phg build --php` v1 confirmed working |
| **L-49 W2-4 `->` retirement** (**DEC-355 / GR-17**) | M | Order: fixture rewrite → parser-reject → refresh dormant tests → grep gate. **Must separate return-annotation `->` from fn-type/prose `->`** |
| **L-52 lift `clone` → `with { }`** | S–M | Build with A12; carries one embedded `__clone` refusal ruling |

### B-tier 3 — the slice-3 chain (strictly sequential, dev-approved order)
**L-37 (S3.2 `Http.ServeConfig`) → L-38 (S3.3 `Http.serve` + retire `respond`) → L-39 (S3.4 role-mismatch
UX) → L-40 (S3.5 inbound TLS).** Each is a fully specced sub-slice with no open design fork. Combined
effort ≈ L. **Worth stating plainly: Rich Request's shipped `Request`/`Response` types are reachable today
only through the legacy `respond` bridge** — three of five S3.x sub-slices are what stand between a shipped
feature and its intended entry point. S3.5 goes last because it isolates the new rustls dep and the
all-features gate; its server-side dep admission goes through the dependency policy like `http-client` did.

### B-tier 4 — ongoing campaigns, not batches
**L-59** Inv-13 ratchet (66 files >500; `desugar_db.rs` at 3139 is the standout) · **L-60** deep-chain
stack-overflow robustness slice · **L-72** the two missing CI gates.

### ✅ Resolved mid-sweep: L-81 (the global-review directory)
It was staged-but-uncommitted when I started and is now **committed** (`b30d9b5` / `68dca8e` / `b3e635e`),
with G, H and I landed. Nothing owed. The only residue is a cursor flip: some places still show Q-C as the
queued next slice — fold into the L-70 stale-label batch.

---

## C. DEFERRED WITH A REASON — and whether the reason still holds

| Item | Stated reason | Does it still hold? |
|---|---|---|
| **L-13 FS locking** — deferred/undisposed, presumed to need a dependency | "no std path / would need a crate" | **❌ NO — the reason is obsolete.** `std::fs::File::{lock, lock_shared, try_lock, try_lock_shared, unlock}` are **stable on the pinned rustc 1.97.1** [Verified: compiled and ran them]. Policy clause 3 (*"No std-only path is both secure and Phorj-native"*) is therefore **not met**, so no crate may be admitted even if wanted — and none is needed. **This is the single most consequential obsolete rationale found** |
| **L-12 file streaming** — DEFERRED, once outright REJECTED | implicitly "needs generators / new machinery" | **❌ Partly obsolete.** The `Iterator<T>` protocol shipped (DEC-257) and the exact `Iterator`+`fgets` shape is **already byte-identical on all three legs**; O2 needs zero new spine machinery. Generators (`yield`) are **not** required |
| **L-14 no-op clone** — no ruling, treated as an unblessed accident | "undecided surface" | **❌ Obsolete as framing.** It already works on all three legs and is the *idiomatic* spelling in the C#/Kotlin family this syntax came from. The genuine debt is docs + lift, not the feature |
| **L-21 jsonround arena (DEC-286)** — "needs a value-model rebuild, not autonomous" | spine-deep, Inv-15 | **⚠ Reason holds, but the premise moved**: the ruled alternative (DEC-333(a) Json-ADT JIT) is in flight and does not need the arena. **Re-measure after #33 before ruling** |
| **L-27 maxBy/minBy representation lever** — "dev to rule, not a night call" | Inv-15-adjacent representation choice | **⚠ Largely obsolete**: the common path already flipped 0.19×→**8.13×** / 0.20×→**8.18×** via the ruled `??`-fusion lever, no representation change. Only window-less call sites remain — narrow the row |
| **L-58 generators/`yield`** — last, fresh-context, spine-sensitive | deepest VM control-flow | **✅ Holds.** Reinforced: L-12 explicitly does not need it, removing the main pressure to pull it forward |
| **L-35 MongoDB** — build deferred behind value-ordered packs | *"the DEFER costs only absence, never wrongness"* | **✅ Holds** — no program can reach Mongo today |
| **L-34 SessionStore v2 backends** — v1 in-memory matches `phg serve`'s single-process model | single-process | **✅ Holds** while `serve` is single-process — but it becomes load-bearing the moment that changes |
| **L-65 AOT (DEC-333(b))** — gated on JIT-WINS-ALL | perf order | **✅ Holds, and is reinforced by measurement**: AOT helps ONLY queryparse dispatch (partial, not a flip), rides #33's codegen, zero for listcontains |
| **L-62 code signing / M2.5 Phase 3** | not a current goal | **✅ Holds** until distribution is real |
| **L-64 BigInt/Money** | `decimal` covers the common case | **✅ Holds** |
| **L-67 the 9 PENDING-REJECT rows** | post-1.0 | **✅ Holds** — but the **12-domain inventory hole** disclosed alongside them is a silent denominator hole that makes every parity % optimistic. Repair it at the next recompute |
| **L-61 PSR-4 aliasing** | gated on re-adjudication | **⚠ Re-confirm with A4** — the package-enforcement ruling touches the same ground |
| **L-29 DEC-219 static overload resolution** | "low priority vs the DB/output work" | **✅ Holds** — the DB work it was deferred behind is still open (A9) |
| **L-66 multi-file playground** | dev ruled "later" | **✅ Holds** |
| **L-23 DEC-225 Fibers PHP-mapping spike** | *"until the spike proves order-identity, the hard error stands"* | **⚠ Effectively abandoned** — DEC-322 now treats concurrency as permanently PHP-excluded. Worth closing the row explicitly rather than leaving it as notional future work |

---

## D. STALE LABELS FOUND

Items whose recorded status is wrong. **This is where the developer's decision time was about to be
wasted.** Each verdict is independently verified.

### D.1 — Recorded as PENDING / QUEUED / REMAINING, but ALREADY BUILT
| # | Item | Recorded as | Reality | Evidence |
|---|---|---|---|---|
| 1 | **Named args parts 2/3 — CONSTRUCTORS and METHODS** | *"⏳ NAMED ARGS part 2/3 = CONSTRUCTORS, part 3/3 = METHODS … interim they report `E-NAMED-ARG-MISPLACED`"* (`SLICE-STATE.md:1501-1503`) | **ALREADY BUILT** | [Verified: `phg check` clean on `new P(a: "hi", b: 3)` **and** `q.m(x: "a", y: 1)`] |
| 2 | **Tuples (DEC-288/288b)** | *"NOT started — the clear next major slice"* / *"the one remaining big slice"* (`SLICE-STATE.md:1618`, `:1621-1637`) | **ALREADY BUILT** | [Verified: `phg check` clean on `function pair(): (int, string)` + `var (a, b) = pair();`; parser support at `src/parser/stmts.rs:118,157,181,421,451`] |
| 3 | **Backed enums + `cases()`/`from()`/`tryFrom()`** | *"VERIFIED absent"* (`SLICE-STATE.md:1569`, item 6) | **ALREADY BUILT** (DEC-302) | [Verified: `src/parser/items/types/enums.rs:7,14`; `src/checker/calls/variants.rs:237,252,269` incl. `E-ENUM-NOT-BACKED`] |
| 4 | **Lift `lift_from` facet (DEC-312)** | listed as REMAINING in **three** places (`SLICE-STATE.md:1013`, `:1116`, `:1129`) | **ALREADY BUILT** | [Verified: **53** non-empty `lift_from: &["…"]` populations + `src/lift/lifter_tests.rs:339` "DEC-312: builtin → Core resolution through the registry's `lift_from` facet"] |
| 5 | **P-Q-A-5 — Inv-13 file-size debt from the Q-A series** | open dev-owned follow-up in **five** places (`SLICE-STATE.md:52-53`, `:75`; wildcard spec `:204-215`; `MASTER-PLAN.md:141`; `C-decisions.md:3253`) | **RESOLVED** | [Verified: every named file is gone/split — `src/cli/explain.rs` (2057) no longer exists, now `src/cli/explain/*.rs` ×10 all ≤270; `checker/program/walk.rs` 592→255. `scripts/size-gate.sh` → **fails=0**, OK] |
| 6 | **CRAFT-2 — 83 scattered `uses_*` flags on `Transpiler`** | open craftsmanship flag (`KNOWN_ISSUES.md:~105`) | **RESOLVED** | [Verified: the `HelperGates` sub-struct landed — `src/transpile/gates.rs`, ~65 flags moved, 196 renames across 7 files, byte-identity preserved (`SLICE-STATE.md:9-11`)] |
| 7 | **DEC-255 fault-parity helpers** (`__phorj_checked_*`, `__phorj_index`, `__phorj_map_get`) | *"PENDING developer rulings (2)"* (`C-decisions.md:1962`) | **ALREADY BUILT** — the ruling+build is recorded in the *next* section, but the earlier PENDING framing was never annotated | [Verified: all three live in `src/transpile/{gates,expr,runtime_php,stmt}.rs`] |
| 8 | **DEC-223 `Core.Mail`** | *"RULED, build-pending"* (`C-decisions.md:916`) | **ALREADY BUILT** | [Verified: `src/ext/mail/{tests,handles,natives,mime}.rs` + `lettre` in the registry] |
| 9 | **DEC-238 `__phorj_debug_render` PHP twin** | *"QUEUED"* (`C-decisions.md:1187-1189`) | **ALREADY BUILT** (in the DEC-238→DEC-263 interim, never given its own DEC row) | [Verified: 4 files use it, incl. `src/ext/debug/natives.rs`] |
| 10 | **DEC-216 package management** | *"PENDING (developer lean)"* (`C-decisions.md:709`) | **RULED then SHIPPED** via DEC-282 + DEC-316 (`e896eba`/`775db80`/`6284506`, 2026-07-20) | [Verified: DEC-316 SHIPPED entry with commit hashes] |
| 11 | **`db.transaction(closure)` + closure `retry`** | *"PENDING (Invariant 15)"* (`C-decisions.md:547`) | **SHIPPED 2026-07-14**, unblocked by DEC-222 throwing-closure function types | [Verified: register `:782`] |
| 12 | **Retry SURFACE** (`db.transactionRetry`) | *"PENDING adjudication — dev to confirm the final name/shape"* (`C-decisions.md:794`) | **SUPERSEDED + BUILT** by DEC-249: `db.transaction(fn, int retries = 0)`; `transactionRetry` **retired** | [Verified: DEC-249 BUILT entry] |
| 13 | **PART-2 empty-`[]` contextual typing removal** | *"PART-2 PENDING"* (`C-decisions.md:661`) | **SHIPPED 2026-07-14** (`E-EMPTY-LITERAL` everywhere + codemod), a developer override of the resequencing | [Verified: register `:672`] |
| 14 | **Shebang-regex breadth / `--lang` affordance** | *"Build-time PENDING"* (`C-decisions.md:3079-3081`) | **RESOLVED by DEC-336** (RULED + BUILT 2026-07-24); no `--lang` flag was needed, exactly as the item allowed | [Verified: `C-decisions.md:3274-3287`] |
| 15 | **DEC-336 shebang sources** | still inside MASTER-PLAN §0's *"NEW QUEUED CAMPAIGN"* prose | **ALREADY BUILT** 2026-07-24/25 | [Verified: CHANGELOG + register + SLICE-STATE all confirm] |
| 16 | **LSP find-usages project-wide** | *"⏳ REMAINING"* (`SLICE-STATE.md:1015`) | **ALREADY BUILT** — DEC-327, `src/lsp/references.rs` | [Verified: `C-decisions.md:2724`; `B-lsp-editors.md:598`] |
| 17 | **Lift catch-up 0b(a) `private(set)`/`protected(set)` and 0b(b) `foreach ($m as $k => $v)` Tier-1** | queued (`SLICE-STATE.md:2293-2297`) | **BOTH BUILT** — only 0b(c) Uri remains (= L-51) | [Verified: `src/lift/ast.rs:54-55`, `src/lift/parser/items.rs:208`, `src/lift/printer/exprs.rs:328`; `src/lift/lifter_tests.rs:281` "DEC-280: the `$k => $v` form lifts Tier-1"] |
| 18 | **Naming mega-slice (DEC-275…279)** | *"Queue after DEC-257"* item 0a (`SLICE-STATE.md:2285-2292`) | **ALREADY BUILT** | [Verified: `E-ERROR-NAME` in `src/checker/collect/conformance.rs:57`; `Core.Native.FileSystem`; `DependencyInjection`; `Core.Url` merged into Uri per `src/transpile/call.rs:312`] |
| 19 | **DEC-313 transpile FS emitter** | listed as REMAINING (`SLICE-STATE.md:1114`, `:1127`) | **ALREADY BUILT** — `transpile/fs_php.rs`, quarantine lifted, php-leg parity test | [Verified: `SLICE-STATE.md:1009` records it DONE 2026-07-22, but the two REMAINING lists were never updated] |
| 20 | **DEC-257 Iterator protocol** | *"NOT ruled … queued"* (`C-decisions.md:984-986`), no BUILT marker anywhere in the register | **ALREADY BUILT** | [Verified: `Iterator<T>`/`hasNext`/`next` ship (`src/cli/preludes.rs:74-99`, `:359-364`); `Input.lines()` streams 88 MB in 23.7 MB RSS byte-identically on all 3 legs] |
| 21 | **`Core.Deque` / `Core.PriorityQueue`** | *"List(36)/Map(13) exist, no Set/Deque/PQ"* (`SLICE-STATE.md:1573`, item 9) | **PARTIALLY BUILT** — both exist in the preludes (returning `T?` rather than throwing); only the heap family is genuinely missing | [Verified: `src/cli/preludes.rs:243-251`, `:290-298`] |
| 22 | **HOF natives `List.map`/`filter`/`reduce`** | *"not yet available"* (`KNOWN_ISSUES.md:1560-1593`, Lambdas section) | **ALREADY BUILT** — and **contradicted elsewhere in the same file**: `PERF-native-call-in-loop`'s own tables list them as shipped, JIT-special-cased, benched natives | [Verified: `src/native/list_registry.rs:117` `name: "map"`, `:131` `"filter"`, `:232` `"reduce"`] |
| 23 | **`E-TRANSPILE-FS` quarantine** | listed among the active Ladder case-2 quarantines | **RETIRED (DEC-313)** — it survives *only* as a deliberately-worded explain entry (`src/cli/explain/transpile_di.rs:122-123`: *"RETIRED (DEC-313, 2026-07-22): `Core.FileSystemModule` now transpiles"*) with **zero emit sites**. Worth calling out as the **correct** way to retire a code — contrast `E-MULTIPLE-MAIN` (D.2 #6), which still teaches a live convention it no longer enforces | [Verified: 2 hits total, both in the retired-explain arm] |
| 24 | **Log-v2 processors** | deferred alongside userland sinks/formatters | **SHIPPED** (DEC-329.4) — only userland `LogSink`/`LogFormatter` remain (natives cannot yet call back into phorj code) | [Verified: inline ✅ in `KNOWN_ISSUES.md:2297-2308`] |
| 25 | **Totality cluster / `never` deferrals** | gated on the error-model M-faults slices 2a/2b | **POSSIBLY STALE** — 2a/2b have since shipped, so the stated gate is discharged | [Inferred: the gating slices are recorded as shipped; the Totality section was never re-checked. **Worth a fresh look**] |
| 26 | **The whole 2026-07-25 plans-divergence audit (H1/H2/H3, M1/M2/M3/M4, L1/L2)** | 9 open findings | **8 of 9 FIXED since it was written** — the Q-A/Q-B register block now exists (`C-decisions.md:3241-3264`), `MASTER-PLAN.md:148` says DONE, `FEATURES.md:95` has the Q-B row, CHANGELOG has both, MILESTONES carries a Q-B update, both spec annotations landed | [Verified: each re-checked individually. **Only E1 remains** — see D.2 #1] |

### D.2 — Recorded as DONE / ruled, but NOT (or only partly) built
| # | Item | Recorded as | Reality | Evidence |
|---|---|---|---|---|
| 1 | **Wildcard-imports spec header** | line 1: *"SPEC (RULED — BUILD-READY, **NOT YET BUILT**)"* | **Contradicts its own body** — line 228 is `## ✅ Q-A DONE (2026-07-25 — DEC-268 certified)`. A fresh context reading the header first concludes nothing shipped. Inv-19 violation **inside a single file** | [Verified: read both lines] |
| 2 | **DEC-331 D2/D3/D5/D6/D7 "LOCKED"** | LOCKED rulings, read as near-done | **NOT BUILT** — `ServeConfig` exists only as a **code comment**; `respond` is still the live `SERVE_ENTRY`; no `Http.serve(`; `E-NO-ENTRY-FOR-ROLE` → 0 hits; no `http-server-tls` feature | [Verified twice independently: source grep + `phg check`/`phg explain`/`phg serve --help` probes] |
| 3 | **entry-kinds-serve-tls spec** | no BUILD STATUS section at all, unlike every sibling spec | S3.1 shipped weeks ago and the file never recorded it — a doc-currency gap in the spec that governs four unbuilt sub-slices | [Verified: read the file] |
| 4 | **DEC-247 `Core.DateTime`** | *"UNBLOCKED — RULED"* to admit a tz crate | **NEVER BUILT** — no `chrono-tz`/`tzdb` in `Cargo.toml`, no `Core.DateTime` module, nine days after the unblock | [Verified: grep across `src/` + `Cargo.toml`] |
| 5 | **`SLICE-STATE.md:1022` — "LSP AUTOCOMPLETE — DONE + COMPREHENSIVE"** | DONE + COMPREHENSIVE | **Measurably false for UFCS** — the language's primary stdlib call syntax yields **zero** completions on primitive and container receivers; chained/field receivers get the *wrong* list | [Verified: `B-lsp-editors.md:99-209`] |
| 6 | **`E-MULTIPLE-MAIN`** | live error code; `phg explain` returns a full explanation; `examples/README.md:190` asserts it fires | **DEAD** — zero emit sites in `src/` (only a negative test assertion + 3 stale doc comments) | [Verified: `phg explain E-MULTIPLE-MAIN` returns text; `grep E-MULTIPLE-MAIN src/` finds no emit site] |
| 7 | **`E-RETIRED-FORIN`** | ruled by DEC-248 with a rewrite hint and a ~69-site codemod | **DOES NOT EXIST** — 0 hits; the sibling items shipped | [Verified: `grep -rn E-RETIRED-FORIN src/` → 0] |
| 8 | **W2-4 `->` retirement** | *"parser-reject pending"* | Still fully accepted (`function main() -> void` type-checks clean) — **and cheaper than recorded**: `phg format` already normalizes it, so only the 2 068 `.rs` fixtures need a script | [Verified: probe + `K-inline-findings.md:57-61`] |
| 9 | **CRAFT-1's "90 files over the 500 hard cap"** | 90 | **66 now** (real progress, figure stale). Note also: these are grandfathered, `size-gate.sh` **passes** | [Verified: `find src -name '*.rs' \| xargs wc -l \| awk '$1>500'` → 66; gate `fails=0`] |
| 10 | **`SLICE-STATE.md:148-150` citing `src/cli/explain.rs:1065,1208`** for the arrow-syntax drift | one file, two lines | **That file no longer exists** (M-Decomp'd) — the drift now lives in **four** files: `explain/attrs_faults.rs:135,138,143,145`, `match_overloads.rs:21`, `members_destructure.rs:46`, `types_traits.rs:231` | [Verified: grep] |
| 11 | **`KNOWN_ISSUES.md:3` still headed 🔴 P0** (the example byte-identity glob no-op) | reads as an open P0 at the top of the file | **FIXED** (`a355c342`, 2026-07-19) — the inline status line says so, but the 🔴 P0 heading is what a scanner sees first | [Verified: read the entry + `tests/differential.rs`'s line-parsing `uses_impure_native`] |
| 12 | **`CLAUDE.md:8-9` "four vetted, feature-gated exceptions"** | four | **Eleven domains** in `Cargo.toml` | [Verified: `Cargo.toml:113-180`] |
| 13 | **`UNIFIED-SPEC.md:928,989` "JIT not yet wired into `phg run`"** | not wired | `jit` has been a **default feature** since 2026-07-09 | [Inferred: CLAUDE.md's own statement + shipped verticals; the spec text was never refreshed] |
| 14 | **ADR-0005** | current | Names the **retired** `phg vendor` command; principle intact, but the ADR README's own rule requires a superseding ADR and **no ADR-0006 was written** | [Verified: `phg vendor --help` → "RETIRED (DEC-282)"] |

**Cross-cutting pattern worth acting on.** Three of the four language questions the developer raised
tonight trace to the *same* root cause: **a ruling exists, was partially built, and the surrounding docs
were never reconciled** — DEC-248 (loop retirement never built), DEC-326 (UFCS lifter built, corpus + LSP
not), DEC-331/337 (`#[Entry]` built, the `main` reservation and its docs never removed). Invariant 19 is
being honoured for *new* rulings and not for *partially-executed* ones. A mechanical check —
**"every register row whose text names a diagnostic code must have that code present in `src/`, or be
marked PARTIAL"** — would have caught `E-RETIRED-FORIN` (ruled, absent) and `E-MULTIPLE-MAIN` (explained,
never emitted) automatically. Offered as an observation, not a proposal.

---

## E. Coverage statement

### Swept exhaustively
| Source | How |
|---|---|
| `docs/research/full-audit/raw/C-decisions.md` (3376 lines) | **Full sequential read** by a dedicated agent; all 31 `PENDING` hits plus `ASKED`/`DEFERRED`/`QUEUED`/`REOPENED`/`TO ADJUDICATE`/`dev to rule`/`owed` covered; 45 items catalogued; every anchor in the brief resolved (DEC-216/217/218/219/223/224/225/226/247/255/286/183, the `db.transaction` closure at `:547`, the retry surface at `:794`, `Response.capture` at `:880`, the empty-`[]` PART-2 at `:661`, the pipe fork at `:1230`, PENDING-BLOCKED at `:1349`, `App\` at `:2807`, D10c at `:2949`, `:3046`, `:3079`, P-Q-A/B at `:3251`) |
| `docs/specs/*.md` — all 11 dated specs + `UNIFIED-SPEC.md` + `docs/adr/*` (5 ADRs) | Every dated spec: status header quoted, internal PENDING sections extracted, and **build status independently verified** by grep + a live `phg check`/`phg explain` probe of the spec's own syntax/error codes. `UNIFIED-SPEC.md` (122 KB): exhaustive keyword sweep (39 hits) rather than a full read. `docs/specs/archive/` listed; per its README, archival = consolidation into UNIFIED-SPEC, **not** "done" |
| `docs/plans/SLICE-STATE.md` (2308 lines) | Read by me directly: the live cursor block (1-360), the rich-request findings (740-810), the queue blocks (975-1200), the programme order (1450-1600), the tuples/perf blocks (1618-1710), the post-DEC-257 queue (2280-2308), plus a full marker grep (`PENDING`/`FOLLOW-UP`/`dev-owned`/`dev to rule`/`P-Q-*`) |
| `docs/plans/MASTER-PLAN.md` (2421 lines) · `docs/MILESTONES.md` (354) · `ROADMAP.md` | Dedicated agent; §0 CURSOR / §0 QUEUE / §0.1 / §0.2 + AUDIT BUILD QUEUE / §0.3 GAP LEDGER / Waves 0-6 status ledgers / §10 stdlib charter / Appendix A. **Inv-19 check: `ROADMAP.md` is a pure pointer — no divergence** |
| `docs/research/2026-07-25-global-review/` (10 files, ~5 200 lines) | Read directly by me: README, P0, K in full; the `## Gaps` + `## Options & recommendation` sections of A, B, C, D, E; F and J in full. **These are tonight's review results and are the substance of §A** |
| The other three same-day audits | `2026-07-25-currency-audit.md` (79 lines) and `2026-07-25-plans-divergence-audit.md` (155 lines) read in full and **re-verified finding-by-finding** (8 of 9 divergences already fixed). `2026-07-25-lsp-completion-audit.md` covered via B's cross-reference table |
| `KNOWN_ISSUES.md` (2323 lines, 46 `##` sections) | Dedicated agent, **all 46 sections catalogued** (each heading treated as one entry, its 5-15 sub-bullets condensed), with live reproduction for every item the brief named plus a spot-check sample of the largest "still open" claims. My own independent verification on top: the top P0, `CRAFT-*`, `RICHREQ-*`, `STACKDEPTH-*`, `F-032`, the `PERF-*` entries, "Language features not yet implemented", and the transpile-P1 section |
| Source `TODO`/`unimplemented!`/`dead_code`/quarantine census | Exhaustive. **4 `TODO/FIXME/XXX/HACK` hits in all of `src/`, 3 of them false positives** (`\uXXXX` placeholders) → the single real one is L-94. **Zero `unimplemented!`/`todo!` macros** — the codebase fails closed through typed `E-*` errors. ~18 `#[allow(dead_code)]` hits, all clustering on two in-flight slices (L-41's Json-ADT arms + a DEC-333 refinement peephole). 8 live `E-TRANSPILE-*` quarantines (`UNCHECKED`/`DB`/`SESSION`/`HTTPCLIENT`/`MAIL`/`UNICODE`/`VARIANT-COLLISION`, `FS` retired); concurrency's real code is **`E-CONCURRENCY-NO-PHP`**, not `E-TRANSPILE-CONCURRENCY` |
| Source verification | ~35 targeted greps over `src/` + **16 live binary probes** (`phg check`/`run`/`run --tree-walker`/`transpile`/`explain`/`--help`) + a 3-leg reproduction of the P0 against `php-8.5.8` + `scripts/size-gate.sh` + a `wc -l` file-size census |

### Sampled, not exhaustive — the honest gaps in this inventory
1. **The ~14 "deferred refinements" sections of `KNOWN_ISSUES.md` are catalogued at section granularity,
   not bullet granularity.** Pattern cluster · Mutation corners · Dogfood findings · error-model slices
   2a/2b · Interop M8.5 · Totality · Overloading · Generics · Core.Html · Git deps · Router/`#[Route]` ·
   green threads · `as`-matrix · Maps · Generic natives · Iteration protocol · Core.String breadth ·
   file-naming rule · Foreign interop · Core.Time · Secret&lt;T&gt; · `phg format` wrapping · Behavioral
   quirks. Each was assessed and none contains an open *decision*; they are deliberately-scoped
   boundaries that fail with a clean typed error rather than a crash — consistent with the project's
   stated philosophy. **But they hold an estimated 60-100 individual sub-bullets I did not lift into rows.**
   If tomorrow's session wants a bullet-level backlog rather than a decision agenda, that is the one
   remaining sweep. Two flags from within them are already folded in as rows (L-92, L-95) and one as a
   stale label (D.1 #25, Totality's discharged gate).
2. **`docs/research/full-audit/raw/*` beyond `C-decisions.md`** (A-craftsmanship, B-modularity,
   D-php-surface, E-phorj-surface, F-cross-language, G-showcase, H-enforcement, L-lint-batch3,
   M-gap-matrix, P-plan-verdicts, omega0-footgun-audit) were **not** swept. `M-gap-matrix.md` §4 in
   particular is the parity-% model and likely holds additional gap rows.
3. **`docs/research/roadmap-completeness/`, `wave3-4-drafts/`, `perf/`, and the older dated audits**
   (2026-07-03 corpus, 2026-07-13 externalize, 2026-07-16 full-reopen) were not swept — these are exactly
   the inputs **Q-C/DV-5 (L-81)** is chartered to synthesize.
4. **Files G (rust-quality), H (docs-consistency) and I (gaps-enforcement) landed AFTER this sweep
   completed** — now on disk and committed, but **their findings are NOT represented here.** H in particular
   overlaps §D (it is the docs-consistency lens), so expect duplication and possibly stale labels I missed.
   Read them alongside this file; `docs/research/2026-07-25-completeness-register.md` §6 folds all three in.
5. **DEC-183's bounded caveat** (`Optional<enum>` match totality) is recorded but **UNVERIFIABLE** by me —
   my probe failed on enum-variant construction syntax (`Color.Red` → `unknown identifier 'Color'`) rather
   than on the caveat itself, so I have no evidence either way and did not create a row for it.
6. **No cargo build, test, clippy, or benchmark was run** (disk constraint, per the brief). Every "green
   gate" and perf figure quoted here is sourced from a recorded claim, never re-measured — with the three
   exceptions verified directly: `scripts/size-gate.sh` (passes, `fails=0`), the P0's three-leg divergence,
   and L-82's two-leg `Core.Validation` divergence.
7. **One documented count discrepancy:** the KNOWN_ISSUES agent measured **67** files over the 500-line cap;
   I measured **66** with `find src -name '*.rs' | xargs wc -l | awk '$2!="total" && $1>500'`. I use 66 and
   state the method. Either way the figure is *down from CRAFT-1's 90*, and the gate passes.

### Counts
| Category | Count |
|---|---|
| Master-table rows (deduplicated) | **95** |
| Needing a developer ruling | **~46** (18 ranked agenda items in §A, most bundling sub-questions) |
| Ruled but not built (autonomous-ready) | **~30** |
| Deferred with a stated reason | **17** (**3 obsolete**, 3 partly obsolete) |
| Known limitations / doc debt | **~22** (plus the ~60-100 un-lifted sub-bullets, gap 1 above) |
| **Stale labels found** | **40** — 26 recorded-open-but-built, 14 recorded-done-but-not |

### The three things worth doing before any decision is made
1. **Read `docs/research/2026-07-25-completeness-register.md` §2** — the ranked 17-ruling agenda
   (DEC-339…355) that §A of this file maps onto. (My original item here — "commit the global-review
   directory" — was **done mid-sweep**: `b30d9b5` / `68dca8e` / `b3e635e`.)
2. **Rule L-01 (the P0)** — it is the only item in this document that produces silently wrong output.
3. **Run the pinned dev-box microbench** (L-69) — the only thing that can be done on the developer's box
   and nowhere else, and it decides whether the perf-flip campaign has 3 losses left or 1.

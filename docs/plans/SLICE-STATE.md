# SLICE-STATE (live cursor — updated as work progresses; read FIRST after any compaction)

## ▶ CURRENT CURSOR (2026-09-07) — **scout is the forcing function. Plan: `docs/plans/2026-09-07-scout-forcing-function.plan.md`**

Developer directive, 2026-09-07: explore `/stack/projects/scout` and *"use it to
test/validate/certify/review/audit/modify/enrich/improve phorj in a way that i will be able to do a
phorj version of it"*, then — *"include everything! the phorj version must beat php in everything
absolutely! every lifted or transpiled thing must!"* Twelve rulings taken interactively, recorded as
**DEC-504…508** and in that plan's Decisions Log. The four that change how work is done here:

- **scout is READ-ONLY** (DEC-508). No source edits, no docblock additions to raise liftability, no
  bug fixes. It is a frozen yardstick — 274 PHP files / 80 035 lines, PHP 8.5, zero composer runtime
  deps, 1 272 tests, live against eight real sources — and a measurement must never have to ask
  whether the thing being measured moved. The 2026-09-02 16:40 **no-scout-port ruling STANDS**; the
  developer builds the phorj scout himself later. What is validated is **four** surfaces: lift,
  transpile, LSP, speed.
- **DEC-507 — the ABSOLUTE perf bar, with escalation.** Every lifted or transpiled artifact is
  benched; any ratio ≤ 1.0× must be FLIPPED before its slice closes; a loss that cannot be flipped
  **stops the lane and is escalated**, never logged as an OWED and passed. This **supersedes the
  2026-07-10 "MATCHES-not-beats php on 20-yr-tuned string/array/collection" refinement** for this
  campaign. **The baseline is dockerised `php:8.5-cli` with JIT ON**, core-pinned and interleaved —
  the on-box gate oracle is `PHP 8.5.9 (cli) (ZTS DEBUG GCOV)`, correct for byte-identity and
  **invalid for any perf claim**. Two PHPs, two jobs.
- **Readiness steps 13 and 14 are HOISTED** ahead of 10/11/12: HTML5 parse + selectors, then
  `Core.Net` + `Core.Mime` + read-only `Core.Imap`. They are scout's only two hard blockers.
- **Two language slices ruled:** DEC-505 `<=>` + lexicographic tuple/list ordering (both halves are
  missing today, and one line of scout blocks the depth target on it); DEC-504 structural
  named-field tuples (73 keyed `array{…}` sites — the largest lift wall). Both QUEUED in
  UNIFIED-SPEC § "Ordering, `<=>`, and named-field tuples".

**Measured starting state.** `phg lift <dir> -o <out>` writes `LIFT-REPORT.md` naming every refusal —
**this is the durable, re-runnable census** and it replaces the lost scratchpad `raw/scout-needs.md`
the readiness plan calls its yardstick. Today: **54 of 123 files lift, 69 refuse**. The histogram is
**first-error-per-file**, so fixing one wall exposes the next (`yield`, in 23 files, does not appear
in it at all) — re-census after every lifter change; never treat the 69 as a worklist.

**Lane order:** L0 docs → L1 lifter mechanicals (enum `self`, block closures, positional
shapes → tuples, a NAMED by-ref diagnostic per DEC-506) → **L2 DEC-505 → L4 DEC-504 → L3 the depth
oracle** → L5 HTML5 → L6 Net/Mime/Imap → L7 breadth census loop → L8 perf twins.

> **REORDERED 2026-09-08 (plan §15, Claude-level, overrulable): L3 comes AFTER L4, not before.**
> `Classification::toArray()` IS the four-leg contract the depth oracle diffs against, and it refuses
> on the keyed `array{…}` shape that L4 builds — so L3 could never have run first. L2 is still the
> next build (smaller, ruled, and `Classification.php:48` needs `<=>` too), it just does not on its
> own unblock L3. **L0/L1a/L1b/L1c are DONE** (`e42e9a9a`, `9bd0a43e`, `f9e26192`, `78317867`);
> re-census at `78317867` holds at **57/123** with the depth cluster's walls moved — `Tenure` now
> LIFTS, `TenureClassifier` is a named by-ref refusal (hand-port, 1 site), `Text.php` fell through to
> `?? throw`. Two new banked questions in the plan's *Needs input*: **Q-0908-1 `?? throw`**
> (throw-as-expression; phorj's `throw` is `Stmt`-only) and **Q-0908-2 spread `...`** (phorj has none
> at all). Full detail: `docs/plans/2026-09-07-scout-forcing-function.plan.md` §1 re-census.
>
> **DEC-512 RULED 2026-09-08, before any L2 code was written:** ordering is a TUPLE capability;
> `List<T>` is NOT orderable. DEC-505 ruled lexicographic element-wise ordering AND a transpile leg
> emitting PHP's own `<=>`, and for lists those clauses are mutually exclusive because **PHP's array
> `<=>` is COUNT-FIRST** (`[9] <=> [1,1,1]` is `-1`, not `+1`). Tuples are immune — static arity ⇒
> equal lengths ⇒ the two orderings provably coincide — so tuples order with no divergence on any
> input, `List<T>` ordering is a checker error naming the ruling, and `phg lift` maps a positional
> array literal in an ordering-operand position to a tuple (DEC-166-safe: a literal's arity is
> syntactically present). NaN pinned alongside: PHP yields `1` for every NaN comparison, so
> `compare_ord`'s `Ok(None)` projects to `1` at all three call sites. **This is what L2 builds.**
>
> **L2 IS BUILT — `eba09d4e` + `ab959542`, 2026-09-08.** `<=>` (`Op::Cmp`, equality tier; LEFT-associative where
> PHP is non-associative — parenthesized on the PHP leg, banked as Q-0908-4) and
> tuple `< > <= >=` on all three legs + lift + LSP + editors; `List<T>` refused as `E-ORDER-LIST`.
> **Narrower than the ruling in one named way, amended into DEC-512 and the spec in the same batch:**
> `decimal` is EXCLUDED from both new surfaces (`E-ORDER-DECIMAL`, pending **Q-0908-3**), because
> `9007199254740993.00 <=> 9007199254740992.00` is `0` in PHP and `1` natively — bare `d1 < d2`
> already carries that divergence and keeps it (`KNOWN_ISSUES.md`), and the new surfaces refuse
> rather than inherit it.
>
> **DEC-507 bench: a CONFIRMED LOSS, OWED and ESCALATED.** `bench/micro/spaceshipsort` (scout's
> comparator, 1200 rows, output-identical) measures **0.36–0.37× vs docker php:8.5+JIT** across three
> pinned interleaved runs. Attributed, not guessed: a scalar `<=>` costs what an integer subtraction
> costs (60 vs 56 ms), so `Op::Cmp` is fine — ~65% of the time is MATERIALIZING TWO TUPLES per
> comparison. Recorded OWED per DEC-365 (never re-baselined) **in this prose, NOT in
> `bench/micro-baseline.json._owed`** — a new micro reaches that list only via `--emit`, which
> DEC-365 forbids here, so `microbench-gate` prints a loud non-blocking `not in baseline (new)` note
> every run but does NOT protect the loss from deepening — which is why it is being FIXED rather than
> carried: **RULED as DEC-513, lane L2b** (fuse the comparison in the compiler). Rejected there: JIT
> whitelisting, and carrying it to L8. Re-census after L2 holds at **57/123** —
> `Classification.php`'s wall moved 48 → 68, i.e. off `<=>` and onto the keyed shape L4 builds.
> **L2b is CLOSED** (DEC-513, ruled + built + pushed 2026-09-08 — `d4365564`, docs `14355d83`,
> attribution corrected `1968de08`). Structural goal MET, perf bar MISSED, and the residual loss was
> ESCALATED per DEC-507 and RULED by the developer: **carry the OWED, proceed to L4. L2c — a `CmpSeq`
> specialized for checker-known scalar element types — is NOT authorized**; reopening this bench needs
> a new lane and a new ruling.
>
> **L4 BUILT (2026-09-08)** — DEC-504 named-field tuples, the 73-site lift wall, and the other half of
> what L3 (the depth oracle) is blocked on. Three commits: the core language slice, the lift leg +
> refusals, and format/LSP/editors. **NEXT: L3** (the depth oracle), which L4 unblocks.
>
> **What L4 shipped.** `(bp: int, source: string)` as a type and `(bp: 3, source: "x")` as a literal,
> with `t.bp` resolving to a POSITION at check time and rewritten to `t[<i>]` before erasure — so no
> backend learns the fields had names. Labels ride in the `Ty::Tuple` payload, which makes all four of
> the developer's Q1–Q4 rulings fall out of `PartialEq` with no new code. Lift: a KEYED `array{…}`
> shape lifts to the named tuple its own keys describe, and `$row['bp']` against such a parameter
> lifts to `row.bp` — without that second half a lifted body would not check against its own lifted
> signature. Refusals by name: assignment, unknown field, field on a positional tuple, duplicate
> label, `t?.bp`, half-keyed shapes, and keys that cannot be phorj field names.
>
> **The Invariant-7 trap was REAL and had THREE consumers, not one** — the register row predicted the
> VM's `CTy::List`; it reaches further. A tuple erases to a list, whose operand type is ONE
> homogeneous element taken from position 0, so every later field inherited position 0's type in:
> the VM compiler (`CTy::Tuple`), the TRANSPILER (`OpKind::Tuple` — where it did not merely
> de-specialize but picked the wrong PHP OPERATOR, emitting `$t[1] . 1` and printing `31` where both
> native legs print `4`), and INFERRED locals (`var t = (…)` has no annotation, so both consumers fell
> back to the already-erased literal). The third needed `ty_to_ast_type`'s tuple arm fixed: it mapped
> EVERY `Ty::Tuple` to `Type::Erased`, one arm feeding pipe params, destructure binders and foreach
> binders alike.
>
> **Two defects found by probing the refusal surface, both regressions against the positional form:**
> `(bp: int, source: string)?` was a PARSE ERROR while `(int, string)?` parsed (the named type
> returned early, skipping the trailing-`?` loop); and `t?.bp` typed as a non-optional `int` where
> `b?.v` on a class correctly yields `int?`, because the rewrite drops the `safe` flag — now refused
> as `E-TUPLE-SAFE-FIELD`.
>
> **`phg format` silently STRIPPED tuple labels** — found by writing the Invariant-9 example, which
> the pre-commit hook formats. It rewrote `(bp: int, source: string)` to `(int, string)`, turning
> every `t.bp` into `E-TUPLE-POSITIONAL-FIELD`, and `(only: 7)` to `(7)`, an int in parens. Fixed in
> both printers and guarded by a test asserting the formatted program still RUNS THE SAME.
>
> ⚠ **PERF: an OWED verdict, and it is NOT the feature's — RE-MEASURED through the harness.**
> The first numbers here were one unpinned `phg run` per side (K=1) on a box whose own harness
> reports a 284% spread on this bench; they are withdrawn. Re-measured with `scripts/microbench.sh`
> at **K=7, interleaved and core-pinned** [Verified 2026-09-08]:
>
> | bench | VM ns | php+JIT ns | ratio |
> |---|---|---|---|
> | `namedtuplefield` (named tuple / PHP assoc array) | 1 840 265 519 | 467 968 425 | **0.25× LOSS** |
> | `nestedlist` (its ERASED form / PHP list array) | 1 832 042 889 | 57 388 686 | **0.03× LOSS** |
> | `listindex` (one level) | 11 307 087 | 16 582 928 | 1.47× WIN |
> | `intadd` (pure arith) | 4 246 042 | 5 682 775 | 1.34× WIN |
>
> **The feature costs ZERO**: the two VM legs are 1840 vs 1832 ms — 0.4% apart, same checksum —
> and `bench/micro/nestedlist` now ships as a PERMANENT paired control so the claim stays checkable
> rather than resting on one recollection. **The loss is phorj's two-level list element read**:
> 370 ns/iter against 4.3 ns/iter one level deep and 1.4 ns/iter for pure arithmetic. It is NOT a
> deep copy — cost is flat across inner widths 2→128 (285/310/306/281 ms) — and the JIT does engage
> yet does not close it (7× from `--no-jit` here against 59× for `listindex`).
>
> ⚠ The 0.25× row FLATTERS phorj: its PHP leg pays a string-key hash (468 ms) that the phorj leg
> does not, while the honest positional comparison is the 0.03× row. Recorded per DEC-365
> (NO-HIDDEN-LOSS): both benches ship, NEITHER is in `micro-baseline.json` (so both are UNPROTECTED
> by the ratchet), `--emit` was NOT run and `_owed` was NOT hand-edited — it is a derived field.
> **This is a DEC-507 escalation and is the developer's call**, not a
> self-decided carry.
>
> ⚠ **LIFT CEILING — counted at last, and it changes the claim** [Verified 2026-09-08, read-only over
> `/stack/projects/scout` @ f7a4345]. The 1,324 on record was every `$x['k']` read in the corpus,
> never the liftable subset. Keyed `array{…}` shapes sit at 111 sites in 43 files; the REWRITABLE
> reads are **140 across 16 files** — 55 direct on a keyed-shape `@var` local, 85 through a `foreach`
> binder over a keyed collection. **The shipped rewrite covers 0 of them**: `set_tuple_fields` is
> seeded only from PARAMS (`src/lift/lifter/decls/declarations.rs:16,206`) and scout has zero
> `$p['k']` reads on a keyed-shape param — its shapes are `list<array{…}>` that get ITERATED, and it
> is the binder that is indexed. L4's delivered value is the WALL (a keyed shape now has a phorj type,
> so the file lifts at all); the read-rewrite reaches nothing in this corpus until the seed is
> extended to `@var` locals and foreach binders. QUEUED, not done, not claimed.
>
> ⚠ **LSP (Invariant 17's 100% rule) — completion was shipped, the rest was never checked.** Probing
> the running server found two defects, both now fixed with tests and three landed mutations:
> hovering a tuple local showed the declaration text cut at its first comma — `(source: string` for a
> named tuple, `(int` for DEC-288's positional one, an unbalanced type presented as the reader's own —
> and hovering `t.bp` answered `null`, or, where a local happened to share the field's name, answered
> with THAT local's type and jumped go-to-definition to it. The cut is now depth-aware and the field
> resolves against its RECEIVER. Go-to-definition on a tuple field now answers nothing rather than
> jumping somewhere wrong; jumping to the label inside the written type needs spans
> `receiver_tuple_fields` does not carry, and is named here as an uncertified follow-up.
>
> A first version of that bench read one loop-INVARIANT tuple and reported 0.24× too — but for the
> wrong reason: PHP's tracing JIT constant-folded the body to `acc + 11`. Kept here because the number
> was RIGHT and the attribution WRONG, the same shape of error as the DEC-513 residue mis-attribution
> two lanes ago.
>
> **What L2b was:** FUSE the comparison in the compiler.
> `Op::CmpSeq(n, SeqOrd)` pops the `2n` elements of the two tuples and compares them IN PLACE on the
> operand stack, so a both-sides-literal `(a, b) <=> (c, d)` materializes no tuple at all. The
> in-place part is the optimization, not an implementation detail: popping the operands into a `Vec`
> would trade two `Rc<Vec<Value>>` for one `Vec` and give the win straight back. `value::compare_seq`
> and `value::project_three_way` are extracted so the generic and fused paths share ONE lexicographic
> loop and ONE `None -> 1` projection (Invariant 4); `vm::cmp_seq::project_bool` is likewise the
> single `None -> false` for the four ordering kinds. Interpreter, transpile and lift legs unchanged
> — which is what makes the differential harness the equivalence proof: the interpreter never fuses,
> so `phg run` (VM, fused) versus `--tree-walker` (unfused) IS the two lowerings compared against
> each other, on every globbed example.
>
> **SCOPE RULING (Claude-level, 2026-09-08 — recorded because the alternative was weighed, not
> overlooked): the fusion keys on the LITERAL tuple shape on both sides, never on the checked tuple
> type.** The allocation DEC-513 removes happens where a tuple is BUILT, not where it is compared:
> in `var lo = (a, b); lo < hi` both tuples are already on the heap before the comparison runs, so
> fusing there would remove a dispatch and not the ~65% materialization. Widening to the checked type
> is a DIFFERENT optimization (scalar replacement at the binding), not a wider version of this one,
> and it is out of this lane. The narrow form also needs no AST field and no checker threading.
>
> **The Invariant-11 before-number is `ratio=0.373` (loss)** — `spaceshipsort` measured by
> `microbench-gate`'s own protocol at settled 1-min load 2.33, not the manual 0.36–0.37× runs (the
> two agree). Acceptance for the lane is BOTH: materialization eliminated (a `disassemble` of the
> comparator shows `CmpSeq` and no `MakeList`) AND a re-measured ratio under the same pinned,
> interleaved, dockerised protocol. Note the ~35% that fusion does NOT touch is the re-entrant
> `sortWith` → VM callback per comparison, so a ratio near 1.0× rather than comfortably above it is
> the expected best case; if it lands at or below 1.0× that is a DEC-365 OWED recorded honestly, not
> a licence to widen scope mid-lane.
>
> ⚠ **The `sortWith`-callback sentence in the paragraph above was REFUTED by the measurement** — kept
> as the written acceptance criterion, not as a fact. The scalar comparator pays the same callback
> and is fast, so it cancels out; see the MEASURED OUTCOME below for where the residue actually is.
>
> **MEASURED OUTCOME (2026-09-08, A/B on the same quiet box — core 7 at 98.98% idle for two
> consecutive samples, `MICROBENCH_RUNS=15`, core-pinned both sides, interleaved, vs docker
> `php:8.5-cli`+JIT). Both legs proven by disassembly, not by which build was checked out:**
>
> | leg | comparator bytecode | VM ns | php+JIT ns | ratio | spread v/p |
> |---|---|---|---|---|---|
> | A — fusion OFF | `MakeList(2)` `MakeList(2)` `Cmp` | 124,599,687 | 46,964,085 | **0.38x** | 23%/21% |
> | B — fusion ON | `CmpSeq(2, Spaceship)`, no `MakeList` | 100,307,950 | 46,490,679 | **0.46x** | 8%/18% |
>
> PHP's own time matches within 1% across the two legs, which is what makes them comparable; the
> A-leg's 0.38x also cross-validates the recorded `0.373` gate baseline. **VM time 124.6 -> 100.3 ms:
> a measured 19.5% improvement (1.24x).**
>
> **The structural goal is MET and the bar is STILL MISSED — recorded as a DEC-365 OWED, not a flip.**
> No tuple is materialized any more (the A/B disassembly is the proof, from freshly built binaries).
> But 0.46x means phorj still takes ~2.2x php's time on this shape.
>
> **The DEC-513 ruling's own prediction did NOT hold, and WHERE the residue actually sits matters
> more than the number.** It expected "the ~65% construction cost leaves every literal-tuple
> comparison, landing near the scalar `<=>` number". Fusion removed materialization ENTIRELY and
> bought 24.3 ms (124.6 -> 100.3), against the ~109 ms tuple-vs-scalar gap that same attribution
> reported (169 ms / 60 ms) — so construction was roughly a FIFTH of the gap, not ~65%.
>
> **The residue is NOT the `List.sortWith` -> VM callback.** The scalar comparator is re-entered
> through the same callback on every comparison and is fast, so the callback cancels out of a
> tuple-vs-scalar comparison and cannot explain the difference between them. What is left is what
> `Op::CmpSeq` still does that scalar `Op::Cmp` does not: per-element `compare_ord` dispatch through
> the `match (a, b)` ladder, the `Option<Ordering>` threaded per element, and the `SeqOrd`
> re-projection. That is COMPILER/VM-reachable — a `CmpSeq` specialized for checker-known scalar
> element types, or an unrolled arity-2 exec — and is NOT L8 runtime territory. **Do not re-derive
> the 65% figure from the register, and do not aim the next attempt at the callback.**
>
> **THEN L4** (DEC-504 named-field tuples), which unblocks L3.

**The depth oracle is FOUR legs, not three.** `tests/differential.rs` proves interpreter ≡ VM ≡
transpiled-PHP over `examples/**/*.phg`; it never touches scout's ORIGINAL PHP. scout built the
contract for the fourth leg itself — `Rent/Core/Classification::toArray()` is documented *"stable
structure for the cross-language differential test"* and yields
`{tenure, confidence_bp, outcome, reasons[]}`. The eleven-property spec the port must satisfy is
scout's `docs/PHORJ-REQUIREMENTS.md` § "The classifier's cross-implementation contract".

## ▶ PREVIOUS CURSOR (2026-09-04 → 05) — **THE BIG WAVE**

Developer directive, 2026-09-04: *"a very big wave … perf/lsp/editors/lift/transpile/vision … a
continuous/autonomous very long session"*. Plan: `docs/plans/2026-09-04-big-wave.plan.md` (six
lanes, each landed as its own green commit; its Lanes E/F are POINTERS at readiness steps 19 and 3).
Landed so far: **Lane D** `58ed82d0` (the helper-bucket header ratcheted three ways against the
registry — it had drifted 173-vs-187 for four months under a comment claiming it could not);
**Lane C** `71fc9377` (every `lift_from` registration proven to FIRE end to end, `array_slice` →
`List.slice` claimed after a 7/7 three-leg oracle check; both of the lane's starting premises were
phantom gaps and are recorded as such). **Lane A** `39ec01a7` (LSP signature help — the last capability Invariant 17 names by name — plus
both editors, VS Code 0.6.0); **Lane B-1** `3a44a264` (the surface ratchet had been measuring the
WRONG things — it scanned the `phg explain` catalog as emit sites, saw only one of three emit forms,
and counted LSP providers from test strings; made honest: 258/311, providers 9); **Lane B-2**
`51e2efed` (the 53 truly-unasserted codes fixtured → **311/311 — Invariant 17's 100% rule for
diagnostics is CLOSED**); **Lane F** `13e913a0` (§4.20 parity recompute: ≈71 / 59 / 72); **Lane E**
`b15ff3b1` (DEC-431 B — the VM's quadratic `s = s + x` made amortised O(1) by a `Concat`/`SetLocal`
lookahead, 425 ms → 18 ms on `strappend --no-jit`; no new `Op`). **OWED at this close:** the full
pre-push gate (`nextest --all-features`, both clippies, doc-tests, `validate-infra`) — three hooked
commits were stopped externally under a 22/8 load and the developer ruled `--no-verify` with targeted
`-j2` verification (big-wave plan Decisions Log, 2026-09-05 00:48); every lane's own tests, sabotage
and three-leg checks ran green in completed jobs. Run the gate on a quiet box before the next push;
time zones (step 10) remain the next readiness step after the wave.

**Wave 2 (2026-09-05, same directive, plan `docs/plans/2026-09-05-big-wave-2.plan.md`) — the
developer's question *"is phorj 100% capable of implementing a project like scout / invoiceninja /
twes-in?"* was answered NO (parity ≈71%, readiness 9/19) and turned into **Lane R**: lift REAL scout
modules, fix what is mechanical, bank what is a ruling, stand up a real macro-benchmark twin. Landed
in this wave: **Lane T** (namespaced `Debug.dump` of enums — the `__phorj_debug_enums` table keyed by
FQN; `examples/project/reflectdump/` measures it on three legs), **Lane L** (LIFT-ECHO-INT fixed in
the lifter: a non-string echo argument becomes ONE interpolation, a bare variable and a string
literal stay as written), **Lane A** (`workspace/symbol` + `foldingRange`, providers 9 → 11, both
editors + VS Code 0.7.0, ratchet floor re-frozen and its summary now says MET at 311/311), the
Invariant-13 splits the size gate demanded (`vm/concat.rs`, `lsp/{signature,rpc}.rs`,
`transpile/helper_buckets/`, the lift test modules), and **Lane R v1** — the two parser refusals
that blocked 5/5 scout modules (`readonly class`, PHP 8.3 typed class constants) now lift; then
**R-2** `4cab4ed7` (arrow closures → lambdas), **R-3** `b5145855` (array append → `List.append`,
primitive casts → `as`, root-qualified names → implicit `use`), **R-4** (docblock generics type the
bare `array` — the wall behind all five modules; named arguments in every argument list), **R-5**
`691ea21c` (interfaces, parameter defaults, `@`-suppression dropped, root-qualified parents,
`1_000`), **R-6** `be09639b` (an empty literal takes its type from a `@var` or the declared return),
**R-7** `acf0ba00` (`self` → the enclosing class exactly, a property default → a field
initializer, root-qualified `instanceof`), and **R-8** `4cc00c6f` (the static-factory idiom: a
static method is never `open`, `new self(…)` and `self::` name the enclosing class, `static` refused
loudly in all three positions). Each step was re-measured on the five modules before the
next was chosen, and on the whole tree at each landing: **12/120 → 42/120 → 54/121 files lift**,
12 of them checking clean standalone (the rest reference siblings and check only as a project).

The Invariant-9 example for R-7 (`de8f0276`) found two lifter bugs by being RUN rather than read: a
root-qualified name pointing into the file's OWN namespace became an import of itself, and every
top-level statement got a fresh already-declared set, so a file-scope reassignment re-declared its
variable — a shape almost every PHP script contains. Both fixed and sabotage-proven; the remaining
known gap is `§LIFT-DISCARD` (a call written for effect lifts fine, then fails `phg check`).

**Lane F landed too — DEC-475**: every `Http.ServeConfig` field is nullable (`null` = unset), the
effective defaults moved to the consumption sites, and `E-SERVE-CONFIG-RANGE` validates
`port`/`workers`/`timeout`/`maxBodySize` before the socket binds. That closes
KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE: `timeout: 0` ("no timeout") is expressible for the first
time, and a CLI flag overriding a hand-written default is announced instead of falling silent.
UNIFIED-SPEC's D4 is amended in the same change, as the ruling requires.

R-8 was chosen by censusing the check errors of every file that lifts, and the same census is now
the map for what follows: `E-UNKNOWN-TYPE` 36 + `E-UNKNOWN-IDENT` 22 + `E-MODULE-NOT-FOUND` 14 are
ONE wall (a file referencing a sibling that does not lift yet) and fall only to raising the lift
count; of the 67 files that still refuse, the bare `array` inference wall (12) and `array{…}` shapes
(14) are the whole ballgame, and BOTH are banked developer questions rather than autonomous work.
`E-UNUSED-VALUE` does not appear at all — `§LIFT-DISCARD` is real but is not what blocks scout. **Deferred by ruling**
(plan Decisions Log 11:10): `clone($x, [...])` → `with`, `list()`, `: never`, first-class
callables, `...$spread` (no phorj form), `?? throw` (no throw expression), and `yield` (a LANGUAGE
gap — DEC-479 generators are RULED, build QUEUED; 22 scout files). Two banked questions for the developer in the plan's *Needs input*
(Q-W2-1 `Color.Green` cross-package, Q-W2-2 `Acme\Color.Green` rendering). **The OWED gate has since RUN** (2026-09-05 12:37, on `be09639b`) and was
RED on 2 of 9 steps — the repo format sweep (8 non-canonical `.phg`) and BOTH clippy tiers. Neither
is reachable from the fast pre-commit tier, which is exactly the debt `--no-verify` under load
creates. Fixed at the root in `7f441a09` (`phg lift` now writes the formatter's canonical bytes, so
the pair gate and the format sweep stop demanding different text), after which the full ten-step
gate was green on the frozen commit and wave-1 + wave-2 were PUSHED (`39529d95..bad2ac87`).

## ▶ CURRENT CURSOR (2026-08-29) — **S3.5 SHIPPED. DEC-331 SLICE 3 IS CLOSED.**

**DEC-331 D7 built: `phg serve` terminates TLS**, feature-gated `http-server-tls`. HTTPS enables iff
BOTH `cert` and `key` are set on the registered `ServeConfig` — no `--tls` flag, no switch;
`tlsMinVersion` (`"1.2"` default, `"1.3"`) is the floor. **All five of Slice 3 are now built**
(D1/D4/D5/D6/D7).

**It added NO crate.** rustls 0.23 has no client/server feature split, so the outbound HTTP client's
existing feature set already compiled `ServerConfig`/`ServerConnection`/`StreamOwned`. PEM decoding is
hand-rolled (`src/serve/pem.rs`, ~85 lines) rather than admitting a fifteenth dependency. If someone
later proposes a PEM crate: the decoder is tested to yield FEWER blocks on malformed input, never a
wrong one, which is the only property `tls::build` relies on.

**Six things that will bite whoever touches this next:**
- **A lone `cert` is an ERROR, not plain HTTP.** D7's surface says "iff BOTH are set", and the literal
  reading is a silent downgrade to clear text on a port the operator believes is encrypted. The
  refusal is the ruling (`E-SERVE-TLS-INCOMPLETE`); do not "fix" it back toward the spec text.
- **`TlsServer` is an UNINHABITED enum without the feature** — not a struct that is never built. That
  is what makes "a non-TLS build cannot serve a TLS-configured program in the clear" a fact about the
  types; `Conn::accept` discharges the branch with `match *never {}`. Turning it into a struct would
  silently delete the guarantee while every test still passed.
- **Config errors outrank build errors**, pinned by a test: a lone `cert` on a feature-off build
  reports `-INCOMPLETE`, not `-DISABLED`, because the config is wrong however phg was compiled.
- **The stream is wrapped only AFTER blocking mode + timeouts are set on the raw `TcpStream`.** rustls
  fails outright on a non-blocking socket, and running the handshake through those same timeouts is
  what bounds a TLS-level slowloris. Both accept paths already did this; do not hoist the wrap.
- **The handshake happens in the WORKER, never the accept loop.** `StreamOwned` drives it on first
  read, so a stalled client cannot serialize `accept()` and starve the pool.
- **TLS is read directly from the config, NOT through `settings::resolve`.** That function is the
  flag-vs-config PRECEDENCE rule and D7 rules there is no TLS flag — one source, no precedence. It
  would also give `ServeSettings` (which derives `PartialEq, Eq`) a field whose type has neither.

`src/serve/transport.rs` fell 635 → 455 lines and **left `scripts/size-baseline.txt`** (the ratchet
tightens): the wire framing moved to `src/serve/framing.rs`, which is where it belonged — every
function there is generic over `Read` or pure over `&[u8]`.

**Deferred BY RULING, not oversight** (KNOWN_ISSUES §SERVE-TLS): HTTP→HTTPS redirect, HSTS,
certificate hot-reload, mTLS. Cert paths resolve against the process cwd, not the site-mode app root;
passphrase-protected keys are not supported.

### ⊳ CURSOR 2026-09-02 (evening) — panel round 3 in-repo, the readiness wave is the next body of work

**Read first:** `docs/plans/2026-09-02-php-parity-readiness.plan.md` (the delta against three real PHP
apps + cross-language + PHP ecosystem, the ruled ORDER, the questions queue) and
`docs/plans/2026-08-31-post-slice3-consolidation.plan.md` § "Panel round 3" (35 findings transcribed
in-repo, disposition per finding) + its Decisions Log (the 2026-09-02 rulings).

**Gate: CLOSED (DEC-490, 2026-09-03) on the round-4 fixes** — the readiness wave is the active body of
work; OWED at this close: the parity-% recompute (M-gap-matrix §4). History: panel round 3 ran on frozen `6a18f71a` over `0c982019..6a18f71a`: correctness 12
(1×P0 `default_fills` collision reproduced on 3 legs), completeness 16 (3×P1), safety 7. DEC-481
(fix-then-verify) ruled the closing rule: every P0/P1 fixed, freeze, ONE round. **Round 4 ran on
`bfafdd23`** (correctness 8, safety 8, completeness 13 — see the consolidation plan § "Panel round
4"): its P1s are fixed in the commits after it; whether they close the gate or a round 5 is owed is
the developer's call, asked at the end of step 1.

**ORDER (ruled 17:05):** (1) harness trust — panel disposition (C6 `default_fills` P0 FIXED `53df9ef1`+`1e62f74a`; the three ungated PHP-emit paths C7/C8/F1 FIXED `06e9e975`; the LSP/check test-mode path DONE (DEC-486); the differential floor K4 DONE; DEC-459 prelude isolation BUILT; C10/F4 malformed Content-Length FIXED; the regex cluster C1–C5/C11 BUILT with REGEX-B (DEC-461, C4 deferred); the docs pass DONE (K2/K3/K5/K6/K9/K10/K14/K15, MASTER-PLAN §0.07 mirror rows); next: FREEZE and run the ONE panel round of DEC-481 over `0c982019..HEAD`); (2) readiness wave in
leverage order — **charset BUILT 2026-09-04** (DEC-494/495 — both legs hand-rolled from one table, no crate, no ini extension); `String.foldAccents` BUILT too (step 6b, DEC-496), so DEC-468 is fully closed; `Time.sleep` BUILT (DEC-487, ladder DEC-497) and `Runtime.onShutdown` + `isShuttingDown` BUILT (DEC-204/498, step 9); next = time zones (DEC-466 + DEC-499 — the PHP leg is RULED: emit the pinned table, `Zone.of` takes a literal; the crate-vs-generate half is still open, measurements in the readiness plan's Needs research); (3) DEC-333 perf. **Register rows: DEC-460 … DEC-503** (`C-decisions.md`, the
2026-09-02 readiness rulings — 486 LSP/check test mode, 487 `Time.sleep`, 488 the Q22 split, 489 the small stdlib rows); MASTER-PLAN's §0 cursor is stale relative to this block until step 1's
docs pass — this block + the two plan files' Decisions Logs are the record meanwhile (Invariant 19
pointer, not a fork).

**The "next" block at the bottom of the 2026-09-02 cursor below is SUPERSEDED by this one** where
they differ (its `phg test` line predates the CD-31 fix; see panel K3).

### ⊳ ADDENDUM 2026-09-02 — the milestone panel RAN, and the gate is still OPEN

The 3-lens milestone panel that S3.5's plan §5 declared due was run 2026-08-31 against the FROZEN
commit `cf6875db` `feat(serve): HTTPS that refuses rather than falls back — inbound TLS (S3.5, DEC-331
D7)`, over the range `0c982019..cf6875db`. That discharges the obligation to RUN it. It does not close
the milestone.

- **security + safety-promises: CLEAN** — every promise checked against diff, source and EXECUTED
  tests (10/10 default-feature TLS refusals; 21/21 with `http-server-tls`, a real handshake included;
  no committed secrets; the `unsafe` island untouched).
- **completeness + blast-radius: FINDINGS — 8** (2×P1, 2×P2, 4×P3).
- **correctness + regression: FINDINGS — 3** (1×P1 — a live Invariant-1 spine break — and 2×P2).

**Gate status: OPEN, clean counter at zero.** DEC-268 wants two consecutive fully-clean rounds; this
was round one, with findings. Slice 3 must NOT be reported as panel-certified until the developer's
chosen closing procedure completes — and that choice is theirs, because DEC-268's two-clean rule and
the 2026-08-19 economize ruling (one panel per milestone; a second is the waste the rule exists to
prevent) genuinely conflict here. Do not resolve that alone.

The P1 spine break is FIXED (2026-09-02): `Core.Native.Http.registerServe` bypassed
`E-TRANSPILE-SERVE` and emitted PHP that fatalled at runtime. The doc findings are fixed too. Range
disclosure: the panel read only `0c982019..cf6875db`, so S3.2 Parts A/B were reviewed at HEAD state
but their diffs were never panel-read.

**G-8 remains OWED** and was re-confirmed unmeasurable on 2026-09-02: the pre-push microbench-gate
found 1-min load 10.59, waited 90s, still 4.36 against its 2.5 ceiling, and SKIPPED rather than
reporting a number. That is NO-HIDDEN-LOSS (DEC-365) working — never `--emit` a re-baseline to green
it. It needs a quiet box, gated on per-core `mpstat` idle rather than load-avg.

Full plan, ordering and the queued adjudications:
`docs/plans/2026-08-31-post-slice3-consolidation.plan.md`.

### ⊳ CURSOR UPDATE 2026-09-02 — post-consolidation state

**Slice 3's milestone gate is still OPEN.** A 3-lens panel ran against frozen `d182cd45` and returned
**24 findings** across the three lenses (1×P0, 5×P1, rest P2/P3). The P0 — a `Core.Native.Http` ladder
bypass reachable with NO import, via a leaked prelude alias — is FIXED and verified end-to-end. The
false claims it exposed are corrected in place. **The remaining findings are NOT yet worked**, and the
gate must not be reported as closed until they are.

**CLOSED since the last cursor:** KNOWN_ISSUES §TEST-RAW-CHECKER is **FIXED** — `phg test` shares the
front end (injected-prelude types resolve), the item-level desugars descend into `Item::Test` (CD-31),
and since DEC-486 `phg check` / `check --json` / the LSP check a document that declares a `test` item in
test mode, so `check ≡ LSP ≡ test` holds; `run`/`transpile`/`build` stay strict.

**RULED 2026-09-02 — build state per item** (Invariant 19: a ruled-but-unbuilt item belongs in the cursor,
not only the register):
- **DEC-457** — generic `#[Config]` providers key on the REIFIED type (`Map<string,string>` vs
  `Map<string,int>` become distinct injection keys).
- **DEC-458** — the `Core.Database` PHP twin is a `__phorj_db_stmt` wrapper
  `[PDOStatement, sql, params[], nextIndex]`. Unblocks case-1 step 2; step 3 (the `decimal` mapping)
  still needs its own ruling.
- **DEC-459** — BUILT 2026-09-02 (alias isolation at injection; F6 closed with it). Was: its own slice;
  adjacent to §span-collision. It is also the structural cure for the P0 above, whose current fix is a
  containment arm.

### ⊳ CURSOR UPDATE 2026-09-02 (later) — the panel's `Item::Test` finding was the tip of CD-31

Working the panel's *"`resolve_variant_imports`/`desugar_router` skip `Item::Test`"* finding widened
into a class. DEC-356 made `Expr`/`Stmt`/`Pattern` walks exhaustive and its ratchet lists the six
extracted `*_walk.rs` files — so the identical defect survived one level up, in the ITEM walks of
their parent files. **Five live defects, all verified end-to-end against the release binary with a
class control proving the asymmetry** (`CD-31` + addendum carry the table):

| shape | before |
|---|---|
| `html"…"` in a trait method | check clean → both backends `unreachable!`, exit 101 |
| `html"…"` in a **field initializer** | check clean → both backends `unreachable!`, exit 101 |
| UFCS in a trait method | check clean → backend `unknown field` |
| `inject<T>()` in a trait method | check clean → `unreachable!("inject() not expanded")`, exit 101 |
| generic method in a trait | **INVARIANT 1 BROKEN** — natives print `7`, PHP dies `TypeError: must be of type U`, exit 255 |

**The root cause in one line:** a trait's members are a full `Vec<ClassMember>` whose bodies EXECUTE
(they flatten into the using class), and every item walk that omitted `Item::Trait` was skipping
executable code while reading as though it were skipping a declaration.

`item_leaves!()` now joins the three macros in `src/ast/leaves.rs` — **`Import` and `TypeAlias` only**,
because `Interface` and `Enum` both carry `Expr` (`Param.default`, `Attribute.args`,
`variants[].backing_value`). Nine item walks carry explicit arms, and the DEC-356 ratchet was widened
to cover the eight parent files — it immediately caught two more `_ => {}` collection loops.

**Two behaviours left UNCHANGED on purpose, now visible as named arms** (Invariant 15 — a
user-visible change is the developer's call): a `#[Route]` static declared in a trait still does not
register, and `resolve_variant_imports` still does not collect `TypeAlias` names into its collision
set. Both are open questions, recorded in CD-31.

**What this did NOT close** (stated so it is never read as covered): no item-level pass walks param
defaults or attribute arguments — not for `Function` or `Class` either — so a rewrite needed inside
`function f(int n = <expr>)` or `#[Attr(<expr>)]` is missed uniformly. That is DEC-356 FOLLOW-UP B's
territory (one shared total visitor), not a drive-by widening.

**Still owed from the panel, NOT started:** the LSP non-test path, the playground's ungated PHP-emit
path, and the differential's missing floor assertion.

**Next: DEC-331 is done — the queue is open.** Two items are carried OWED and are the natural
candidates: the **G-8 microbench ratchet** (skipped since S3.4; needs a quiet box) and
**KNOWN_ISSUES §TEST-RAW-CHECKER** — FIXED (see the cursor above; DEC-486 closed the last gap).

---

## CURSOR (2026-08-28) — **S3.4 SHIPPED. The wrong verb now says so.**

**DEC-331 D6 built: `E-NO-ENTRY-FOR-ROLE`, symmetric.** `phg run` on a program whose only entry is
`kind: EntryKind.Web` — and `phg serve` on a `kind: EntryKind.Cli` one — no longer reports a bare
absence. It names the role that is missing, the role that IS declared, and the verb that would have
worked; on an interactive terminal it then offers to run that verb, defaulting to NO.

**`src/cli/role_mismatch.rs` is the whole rule, and it is pure.** TTY-ness and the answer are the
caller's, the way `serve::settings::resolve` takes its `cores` — so the ruling is exercised by the
suite rather than by a human at a prompt.

**Four things that will bite whoever touches this next:**
- **The guard is at each RUN VERB, not at a chokepoint, and that is deliberate.** The obvious shared
  steps — `parse_checked`, `check_and_expand{,_reified}` — are also `check`'s, `transpile`'s and
  `benchmark`'s, where a web-only program is perfectly legal. `pipeline::run_guard` is called from the
  nine run paths; `prepare_serve` carries the Web half.
- **It runs BEFORE the check.** A program that is both role-mismatched and type-broken reports the
  mismatch: the verb is wrong regardless, and the user was not trying to run this program at all.
  Pinned by `the_role_mismatch_is_reported_before_type_errors`.
- **`detect` fires only when the OTHER role is present.** A program with neither role is a library and
  keeps `no entry point` / `E-SERVE-NO-HANDLER`; a reserved kind (`Desktop`…) declares no active role
  and keeps `E-ENTRY-KIND-RESERVED`. Both pinned — the second only because `entry_declared_role` is
  `Active`-only, which a later widening could silently undo.
- **The prompt shows exactly what it runs**, so accepting goes through `cli::serve_with_defaults` →
  the SHARED `serve_preamble`. A hand-assembled preamble at the switch site would silently inherit
  `phg run`'s `Dev` profile and serve stack traces from a command typed as `serve`.
- **The serve→run direction is fixed by ORDERING, and it has to be.** `serve_preamble` disables stdin
  process-wide and `src/native/input.rs` has NO inverse, so a switch taken after it would run a CLI
  program with stdin dead. `serve_cli` therefore runs the role guard BEFORE any serve setup;
  `prepare_serve` keeps its own guard as the invariant for other callers, which is why that one never
  fires on this path. Do not "simplify" by deleting either.

`main.rs` fell from 622 to 496 lines and **left `scripts/size-baseline.txt` entirely** (the ratchet
tightens): the 140-line `serve` argv branch moved to `src/cli/serve_cli.rs`, which is where the
switch's sibling wiring belongs anyway.

**[Certified by execution both ways on a real pty]** — `n`, bare Enter, `y`→serve (which bound the
program's own `ServeConfig` port, not 8080), and `y`→run. Non-TTY never reads stdin. Sabotage-verified
twice: no-op'ing `run_guard` and deleting the `prepare_serve` guard each turn the suite red; restore
verified byte-for-byte by checksum.

**Next: S3.5** — inbound TLS via rustls, feature-gated `http-server-tls`. It is the last of DEC-331
Slice 3.

---

## CURSOR (2026-08-23) — **S3.2 Part C SHIPPED. `ServeConfig` finally binds the socket.** ⚠ SUPERSEDED by the cursor above

**DEC-455.14 — developer-ruled: the CLI flag wins, but LOUDLY.** Until this slice the registered
config was INERT: `serve_register::config()` carried `#[expect(dead_code)]` and had no caller, so
`Http.serve(new ServeConfig(port: 3000), h)` still bound 8080.

**The rule.** The config is the DEFAULT source for the four settings the loop binds — `host`+`port`,
`workers`, `timeout`. A flag that was PASSED and whose value DIFFERS wins, after one
`W-SERVE-CONFIG-OVERRIDDEN` line per field on stderr. A flag that merely RESTATES the config prints
nothing.

**Three things that will bite whoever touches this next:**
- **Ordering is load-bearing.** The config can only be read AFTER `web_*_factory` — its startup
  validation run is what executes the `Web` entry and populates the global — and that is still before
  any socket binds. Reading it earlier always sees `None`, i.e. the config silently never applies.
- **Provenance is approximated by VALUE.** A constructed object carries none, so a field is "set" iff
  it differs from D4's class default (`settings::class_defaults`, pinned against the prelude SOURCE).
  `new ServeConfig(timeout: 0)` therefore cannot express *no timeout*; `--timeout 0` can.
  KNOWN_ISSUES §SERVE-CONFIG-PROVENANCE — the real fix is a nullable D4 field set, its own Invariant
  15 question.
- **Do NOT read the config unconditionally.** D4 declares `timeout = 0` while `phg serve` defaults to
  30s, so that would have silently disabled the B4 idle-socket guard for every existing server.
  `an_all_default_config_is_indistinguishable_from_no_config` is the pin.

**Scope is those four fields and no more:** `cert`/`key`/`tlsMinVersion` await D7 (inbound TLS is
unbuilt — `rustls` is linked only by the outbound http-client), `maxBodySize` belongs to the wire
parser, `serverName` has no consumer. Wiring a field whose reader does not exist is a config that
still does nothing.

Resolution is a PURE function (`src/serve/settings.rs`, `cores` injected) — 9 tests, red-first
against a stub reproducing the ignore-the-config behaviour, sabotage-verified twice.
**[Verified end-to-end on a real socket both ways.]** `src/cli/serve_pipeline.rs` was split out
because the wiring pushed grandfathered `pipeline.rs` past its size-baseline row.

**Next: S3.4.** *(Done — 2026-08-28, see the current cursor.)*

---

---

## Older cursors

Everything before the cursors above lives in
[`docs/archive/plans/SLICE-STATE-ARCHIVE.md`](../archive/plans/SLICE-STATE-ARCHIVE.md) — about two
dozen dated cursor, session and handoff blocks, moved there 2026-09-02 so this file is the live
cursor and nothing else. Nothing was deleted. The Json-ADT JIT build plan that used to sit among them
is live design and was promoted to [`json-adt-jit.plan.md`](json-adt-jit.plan.md).

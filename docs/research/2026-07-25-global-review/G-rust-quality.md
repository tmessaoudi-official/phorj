# G — Rust source quality audit (phorj)

**Scope:** Rust source quality, naming, file naming/structure, documentation — per the developer's
2026-07-25 ask. **Read-only**: no repo file was modified; no `cargo build` was run.
**Baseline policy honoured:** no finding proposes editing `scripts/size-baseline.txt`.
**Adjudication (project Invariant 15):** every finding below carries findings + options + ONE
recommendation. Nothing here is a ruling.

---

## Sampling method (honest coverage statement)

The tree is **566 `.rs` files / 154,817 lines** [Verified: `find src -name '*.rs' | wc -l` = 566;
`find src -name '*.rs' -exec cat {} + | wc -l` = 154,817]. Exhaustive review was not attempted.
Coverage was obtained in four tiers:

| Tier | Method | Coverage |
|---|---|---|
| **Whole-corpus mechanical** | Scripted census over all 566 files: line-count distribution, `//!` presence per module root, `#[allow(...)]` enumeration, `unsafe` occurrence, `unwrap/expect/panic/todo/unimplemented/unreachable/let _ =` counts, wildcard-arm detection in every AST walker, resolution of **every** `src/*.rs` and `docs/*.md` path cited inside a comment against the filesystem, word-boundary counts for ~55 candidate naming spellings | **100% of files** |
| **Targeted deep read** | The 12 largest files + the 3 Invariant-3 match sites + every checker sugar pass header + `cli/pipeline.rs` + `chunk/`, `value/`, `vm/`, `native/mod.rs`, `limits.rs`, `phstr.rs`, `green/`, `serve/`, `tokenizer/`, `scripts/*.sh` | ~55 files read in full or in the cited region |
| **Doc cross-check** | `CLAUDE.md`, `docs/ARCHITECTURE.md`, `docs/INVARIANTS.md`, `docs/adr/0001`, `docs/plans/SLICE-STATE.md`, `scripts/size-gate.sh`, `scripts/size-baseline.txt` read in full and diffed against the real tree | full |
| **Parallel specialist sweeps** | 3 read-only agents (naming / documentation / duplication), each returning `file:line` evidence; **every P0/P1 claim they returned was independently re-verified by me** before inclusion (the re-checks are noted inline; one agent claim was corrected — see G23 note) | — |

**Deliberately NOT covered** (state so you can weight the verdict): the bodies of `src/ext/*` (15.2k
lines) beyond spot checks, `src/lsp/` (2.7k) beyond its `mod.rs`, `src/pm/` (2.0k) beyond its
`let _ =`/`unwrap` census, `src/format/` (2.7k) beyond `doc.rs` + `printer/`, and the *semantic
correctness* of `src/jit/` codegen (21.2k lines — structure and naming were audited, arithmetic
lowering was not).

---

## Dimension 1 — File size / structure vs Invariant 13

### Actual current distribution [Verified: scripted `wc -l` over all 566 files]

| Threshold | Count | Share |
|---|---|---|
| Total `.rs` files in `src/` | 566 | — |
| **> 300 (soft cap)** | **184** | 32.5% |
| **> 500 (hard cap)** | **66** | 11.7% |
| > 1000 | 8 | 1.4% |
| Grandfathered rows in `scripts/size-baseline.txt` | 78 | — |

**The gate is currently GREEN and correctly so** [Verified: I re-implemented `size-gate.sh`'s logic
and found **zero** files over 500 that are absent from the baseline — i.e. zero new hard-cap
breaches, and zero grandfathered file has grown past its ceiling]. The recent M-Decomp work
(SLICE-STATE.md "✅ INV-13 DEBT CLEANUP — DONE (2026-07-25)", 13 files → ~90) is real: the
verified slack shows `checker/calls/methods.rs` 973→252, `checker/collect/interfaces.rs` 760→169,
`transpile/mod.rs` 758→198, `loader/mod.rs` 655→198.

### G1 — `scripts/size-gate.sh` instructs an action the developer has forbidden — P2
[Verified: read `scripts/size-gate.sh:16-17` and the baseline-vs-actual diff]

`scripts/size-gate.sh:16-17` says:
> `# When a grandfathered file is split below 500, drop its row from scripts/size-baseline.txt so the`
> `# ratchet tightens (the gate WARNs when a baseline row is now comfortably under, as a reminder).`

But `docs/plans/SLICE-STATE.md:5-6` records the developer's ruling: *"real M-Decomp (NOT baseline
edits — 'don't cheat')"*, and the standing instruction for this audit is that baseline edits are
forbidden. So the script's own remediation instruction is now unfollowable, and it emits **12
permanent `note` lines** every run [Verified: 5 rows point at files that no longer exist
(`src/cli/explain.rs`, `src/cli/tests.rs`, `src/loader/tests.rs`, `src/parser/items/decls.rs`,
`src/parser/items/types.rs`, `src/parser/tests/items.rs`), 7 point at files now ≤ 500].

The functional consequence is not cosmetic: **`src/ast/class_hierarchy.rs` is grandfathered at 467
— below the 500 hard cap** [Verified: baseline row `467	src/ast/class_hierarchy.rs`, actual 467].
A file whose baseline is under the hard cap gets a *tighter* ceiling than a non-baselined file, and
`size-gate.sh:45-52` never applies the 300 soft-cap WARN to baselined files — so that file is
silently exempt from the soft-cap reminder it should be receiving.

- **Option A:** reword the comment to match the ruling ("stale rows are expected; do not edit this
  file — the notes are informational") and leave the data alone.
- **Option B:** make the note-emission conditional on an env flag so the routine output is quiet.
- **Option C:** leave as-is and accept 12 lines of noise per gate run.
- **RECOMMENDED: A** — it is a comment-only change, costs nothing, and removes a documented
  instruction that contradicts a standing ruling. The forbidden action stays forbidden; only the
  script stops asking for it.

### G2 — Top 10 offenders, with a concrete cohesion seam for each — P2
[Verified: read each file's item outline]

| # | File | Lines | Nature | Proposed cohesion split (seam, not line count) |
|---|---|---|---|---|
| 1 | `src/checker/desugar_db.rs` | **3139** | 83-line `//!` + one giant DEC-208 pass | `desugar_db/mod.rs` (the `Db` struct + `ritem/rfn/rmember/rexpr/rstmt` total rewriter) · `naming.rs` (`Naming`/`NamingFind`/`NamingMode`/`naming_suffix`/`snake_case`/`scan_naming_facts` — DEC-258 tier logic, self-contained) · `layout.rs` (`FieldKind`/`ClassKind`/`is_promoted`/`accessor_for`/`scalar_label`/`class_helper_name` — the T-from-ctor resolution) · `helpers.rs` (`HelperSpec` + helper-function synthesis) · `dispatch.rs` (the tier-2 runtime `stmt.naming` branch emitter) · `validate.rs` (`validate_class` + the `E-DB-*` diagnostics) |
| 2 | `src/jit/analyze/mod.rs` | 2476 | kind-flow analysis, 8 distinct concerns | `abi.rs` (`abi_param_kinds`/`is_dynable`/`is_list_kind`/`field_read_kind`/`join_unknown_bottom`) · `prov.rs` (`Prov`/`unboxed_proven_param_kinds`/`entry_prefix_const_inits`) · `ranges.rs` (`range_proven_ops`) · `stack_model.rs` (`ub_push`/`ub_pop`/`LeaderStates`) · `graph.rs` (`UbGraphInfo` + its 4 methods) · `ownership.rs` (`single_use_params`/`movable_dying_elem`/`owned_this_taken_fields`/`mark_this_read_fields`) · `accumulator.rs` (`accumulator_site`/`accumulator_chain`) · `mod.rs` keeps `unboxed_analyze` + `UbAnalysis`/`UbDiscovery` |
| 3 | `src/jit/handles/mod.rs` | 2000 | bit-layout consts + arena + slow-path helpers | `tags.rs` (the ~22 `UB_TAG_*`/`UB_*` consts + `ub_is_untagged` — ~130 lines of pure, well-documented bit layout, zero coupling) · `ctx.rs` (`UbCtx` + `new`/`reset_for_run`/`const_compile_handles`/`alloc`/`alloc_json`/`materialize`/`release`) · `slots.rs` (`alloc_slot`/`alloc_slot_bytes`/`canon1_of`/`str_bytes`/`try_append_in_place`) · `acc.rs` (`AccRec` + `acc_grow_to`/`acc_push`/`acc_take_record`) · `rt_helpers/` grouped by family (the `rt_u_*` fns) |
| 4 | `src/jit/emit_unboxed/mod.rs` | 1658 | **two functions total**: `ub_ref` (22 lines) + `build_body_unboxed` (~1590) | The seam is inside the one function. `setup.rs` (analysis + entry block + dual-space `Variable` declaration + sticky/fault-exit wiring) · `control_flow.rs` (Jump/JumpIfFalse/loop-header/block-seal arms) · `frame_slots.rs` (GetLocal/SetLocal/SetIndexLocal/SetPathLocal arms) · `mod.rs` keeps only the dispatch loop. The `Ec` Copy-context already exists (`mod.rs:5-8` documents it as replacing captured closures) so the mechanism to cross the file boundary is already built |
| 5 | `src/jit/tests/verticals.rs` | 1411 | test file | Split per vertical under test (`verticals/{concat,index,map,set,hof}.rs`) — the sealed `#[cfg(test)] #[path] mod` pattern the project already uses |
| 6 | `src/transpile/runtime_php.rs` | 1370 | **one function**: `emit_runtime_helpers` (~1360 lines of PHP template strings) | `runtime_php/mod.rs` keeps only the `uses_*` gating dispatch (the `HelperGates` sub-struct from the 2026-07-25 split already isolates the flags); one file per helper family: `arith.rs`, `compare.rs`, `string.rs`, `collections.rs`, `decimal.rs`, `json.rs` — each exporting `&'static str` templates. **This file's own doc says the templates "mirror the Rust value kernels byte-for-byte" — see G20 for the parity consequence.** |
| 7 | `src/cli/preludes.rs` | 1245 | ~500 lines of **phorj source embedded as Rust string literals** + a 440-line `CORE_MODULES` table | See **G3** — this is not a plain split, it's a structural fix with a testing payoff |
| 8 | `src/vm/exec.rs` | 1053 | **one method**: `exec_op`, the wildcard-free `Op` match | **DO NOT SPLIT.** This is the Invariant-13-sanctioned "genuinely-cohesive exhaustive-match unit" and `docs/ARCHITECTURE.md:61-63` explicitly rules it stays whole. Credited in the attestations. |
| 9 | `src/jit/emit_unboxed/verticals.rs` | 944 | the handle-op inline fast paths | Already partially split (`verticals_map.rs`, `verticals_set.rs`, `verticals_hof.rs` exist). Continue the same seam: move the remaining concat/index/scan arms into `verticals_concat.rs` / `verticals_index.rs` |
| 10 | `src/parser/stmts.rs` | 935 | 26 `parse_*` methods on `Parser` | `stmts/loops.rs` (`parse_for`/`parse_for_tuple_destructure`/`parse_foreach`/`for_header_is_classic`/`parse_cfor_rest`/`parse_for_clause_stmt`/`parse_while`/`parse_do_while` — lines 414-837, ~420 lines, one cohesive family) · `stmts/destructure.rs` (`parse_list_destructure`/`parse_tuple_destructure_inferred`/`try_explicit_tuple_destructure`/`parse_struct_destructure`/`finish_destructure` — lines 141-276) · `stmts/decls.rs` (`parse_var_*` family + `try_var_decl_header`/`try_typed_binding`) · `mod.rs` keeps `parse_stmt` dispatch + `parse_block`/`parse_return`/`parse_if`/`parse_try`/`parse_throw`/`parse_discard` |
| 11 | `src/cli/pipeline.rs` | 899 | the front-end chokepoints + 35 public entry points | `pipeline/chokepoints.rs` (`check_and_expand` family — Invariants 5+6, the load-bearing part) · `pipeline/cmd.rs` (the `cmd_*` string-entry family) · `pipeline/program.rs` (the `*_program` / `*_program_exit` family) · `pipeline/disasm.rs` (`annotate` + `disasm_program`) · `pipeline/gates.rs` (`foreign_runtime_gate`, `reject_native_only_transpile`). **See G18 for the naming problem this file's entry-point table has.** |

- **Option A:** burn these down in the standing ratchet order (largest first), one file per commit.
- **Option B:** prioritise by *change frequency* rather than size — split `cli/pipeline.rs`,
  `parser/stmts.rs`, and `checker/desugar_db.rs` first because they are the files a language
  feature actually touches, and defer the JIT whales (which one person touches in long sessions).
- **RECOMMENDED: B.** The purpose of Invariant 13 is *"easy to understand or extend"*, and the
  extension surface is the front-end, not the JIT internals. `src/cli/pipeline.rs` at 899 lines is
  read by every feature; `src/jit/handles/mod.rs` at 2000 is read during JIT work only.

### G3 — ~500 lines of phorj source live as Rust string literals, outside every phorj-level gate — P1
[Verified: read `src/cli/preludes.rs:14-554`; `grep -rn include_str src/` returns only
`src/bundle/manifest.rs:18`; read `tests/format.rs:117-127`]

`src/cli/preludes.rs` embeds the entire `Core.*` virtual-module surface as `const *_PRELUDE: &str`
— `JSON_PRELUDE`, `ROUNDING_MODE_PRELUDE`, `OPTION_PRELUDE`, `RESULT_PRELUDE`, `INPUT_PRELUDE`,
`HTTP_RESPOND_BRIDGE`, `FS_PRELUDE`, `SECRET_PRELUDE`, `DEQUE_PRELUDE`, `PRIORITY_QUEUE_PRELUDE`,
`ITERATOR_PRELUDE`, `TIME_PRELUDE` (~500 lines of real phorj declarations, several as raw strings
spanning 50-180 lines each).

The gate consequence is the finding. `tests/format.rs:117-122`:
```rust
fn every_repo_phg_formats_idempotently_and_safely() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_phg(&root.join("examples"), &mut files);
    collect_phg(&root.join("selftest"), &mut files);
```
The sweep globs **only `examples/` and `selftest/`**. So the prelude sources — which every program
that writes `import Core.Time;` compiles against — are **the only phorj code in the repo that is
never format-checked, never `phg check`-ed as a file, and can never carry a real span in a
diagnostic**. They also cannot be read by the LSP, cannot be lifted, and are invisible to Invariant
17's "always-current surfaces" discipline.

**Concrete fix:** move each prelude to `src/cli/preludes/<name>.phg` and `include_str!` it. That
(a) drops ~500 lines out of the file, taking it from 1245 to ~700 and the `CORE_MODULES` table into
its own `preludes/registry.rs` gets it under 300; (b) puts the prelude corpus inside the formatter
idempotency sweep by adding one `collect_phg(&root.join("src/cli/preludes"), …)` line; (c) gives
the sources syntax highlighting and editor support in the project's own editors.

- **Option A:** full `include_str!` extraction + extend the fmt sweep (above).
- **Option B:** extract to `.phg` files but do **not** extend the fmt sweep (preludes may
  deliberately use a non-canonical layout; extending the sweep could churn them).
- **Option C:** leave embedded; add a dedicated test that runs `phg check` over each prelude string.
- **RECOMMENDED: A.** The preludes are the stdlib's public surface; they deserve at least the
  scrutiny an `examples/` file gets. If the sweep flags them, that is information, not churn.
  (Option C is the cheap partial — it buys the check but not the formatter or the editor support.)

### G4 — `docs/ARCHITECTURE.md`'s M-Decomp table is stale in 8 of 8 rows — P1
[Verified: `ls src/<module>/*.rs` for each row vs `docs/ARCHITECTURE.md:65-79`]

| Module | ARCHITECTURE.md claims | Actual |
|---|---|---|
| `checker/` | 15 cluster files | **31** top-level files + `calls/`, `collect/`, `expr/`, `stmt/`, `program/`, `desugar_di/`, `rewrite_pipe/` as **directories**. ~19 files (`desugar_config`, `desugar_router`, `desugar_variadics`, `collapse_injected`, `enforce_injected`, `erase_tuples`, `function_imports`, `inline_parent_ctor`, `intrinsic_imports`, `overloads`, `plumbing`, `qualify_variants`, `reflect`, `resolutions`, `resolve_variant_imports`, `rewrite_fills`, `rewrite_foreach`, `rewrite_invoke_tostring`, `rewrite_new`, `rewrite_ufcs`) are absent from the doc |
| `parser/` | `exprs stmts items types patterns` | `mod patterns stmts types` + `exprs/` and `items/` are **directories** |
| `ast/` | `walk` · `classes` | `class_hierarchy class_layout entry exprs stmts types_core walk` — **`ast/classes.rs` does not exist** [Verified: `ls` → No such file] |
| `loader/` | `resolve` · `fs` | 9 files (`assemble discovery entry fs import_hygiene imports resolve unit visibility`) |
| `compiler/` | `program stmt expr matches` | `ctors cty emit matches program variants` + `stmt/`, `expr/` dirs; **`stack_effect` is in `emit.rs`, not `mod.rs`** |
| `transpile/` | `program types stmt expr call matches` | **25** files |
| `interpreter/` | `stmt expr call construct` | 10 files (`coop engine kernels variants` added) |
| `vm/` | `exec` · `closure` | + `coop` |

And `docs/ARCHITECTURE.md:78-79`:
> `\`tokenizer/\` (621 lines, one cohesive scanner) and \`chunk.rs\` (shared \`Op\`/\`validate\` contract) are deliberately left single.`

Both halves are false: `src/tokenizer/` is **5 files / 1988 lines** and `src/chunk/` is **4 files**
(`mod.rs op.rs validate.rs tests.rs`) [Verified: `ls`; `wc -l src/tokenizer/*.rs` → 1988 total].

- **Option A:** regenerate the table from the tree and add a CI check that every module named in
  the table exists and every `src/*/` directory appears in it.
- **Option B:** replace the per-file table with a per-module *responsibility* paragraph and stop
  enumerating files at all — the file list is what rots; the seam is what matters.
- **RECOMMENDED: B**, with A's existence check applied to the module-level rows only. An enumerated
  file list in a doc will rot again the next time M-Decomp runs; a seam description will not. This
  is also the cheapest thing that makes the doc trustworthy again.

### G5 — 31% of the codebase is absent from the "one-page map of the codebase" — P1
[Verified: `grep -c "jit\|lsp\|lift\|\bpm\b\|bundle" docs/ARCHITECTURE.md` → **0**; module line counts by `find`]

`docs/ARCHITECTURE.md:1` calls itself "A one-page map of the codebase". These top-level modules
have **zero mentions** anywhere in it:

| Absent module | Lines | What it is |
|---|---|---|
| `src/jit/` | **21,251** | the 4th backend (Cranelift), a **default** feature, and the sole `unsafe` island |
| `src/lift/` | 5,450 | the PHP→Phorj lifter — named in CLAUDE.md's own project description |
| `src/lsp/` | 2,658 | the language server — named in CLAUDE.md, and Invariant 17 makes it co-equal with `phg check` |
| `src/format/` | 2,698 | the formatter — named in CLAUDE.md |
| `src/pm/` | 2,036 | the package manager (`phg vendor`, Invariant 10's only network command) |
| `src/bundle/` | 1,393 | the `phg build` artifact layer — Invariant 1's "third surface" per `docs/INVARIANTS.md:18` |
| `src/debug.rs` + `src/dap.rs` | ~700 | the debugger — named in CLAUDE.md |
| `src/dump.rs`, `src/inspect.rs`, `src/json.rs`, `src/php_names.rs`, `src/phstr.rs`, `src/profile.rs` | — | — |
| **Total absent** | **≈48,700** | **31% of 154,817 lines** |

The `jit/` omission is the sharp one: `docs/ARCHITECTURE.md:26` draws a three-backend pipeline
diagram, while `src/jit/mod.rs:63` says *"The JIT is a 4th backend intimately coupled to
`Op`/`Value`/chunk (invariants #3/#4/#6)"*. The architecture doc and the JIT's own module doc
disagree about how many backends exist.

- **Option A:** add rows for all 14 to the module table and a 4th leg to the pipeline diagram.
- **Option B:** restructure into two sections — "the spine" (tokenizer→…→backends, incl. JIT) and
  "the tooling ring" (lsp, format, lift, pm, bundle, debug/dap, inspect, dump) — since those are
  genuinely different kinds of module and a flat table hides that.
- **RECOMMENDED: B.** A flat 30-row table is not a "one-page map". The spine/ring split is also the
  honest description of the coupling: spine modules must respect Invariants 1-8; ring modules must
  respect Invariant 17. A newcomer needs to know which set they are in before anything else.

---

## Dimension 2 — Naming quality

### G6 — 13 different verbs for one operation, nested 17 deep in one expression — P1 (highest-value naming finding)
[Verified: read `src/cli/pipeline.rs:180-215` in full]

`src/cli/pipeline.rs:180-215` is a **single nested call expression, 17 levels deep**, applying 17
sugar-removal passes that all have the identical shape `Program → Program`. The verbs used:

`resolve_` · `qualify_` · `erase_` · `materialize_` · `lower_` · `inline_` · `rename_` ·
`rewrite_` · `unwrap_` · `inject_` · `expand_` · `desugar_` · `apply_`

Two problems compound:
1. **No verb carries information.** From a name alone you cannot tell what a pass does or when it
   runs. `erase_generics`, `expand_aliases`, `resolve_html`, `unwrap_new`, `rewrite_ufcs`, and
   `apply_default_fills` are the same kind of operation under six verbs.
2. **The nesting is inside-out with interleaved comments**, so the reading order is
   comment→outer-call→…→innermost. `src/cli/pipeline.rs:192` reads
   `crate::checker::erase_tuples(crate::checker::materialize_tuple_binds(` — two passes, one line,
   and the DEC comment explaining them sits *above* both.

- **Option A:** settle on exactly two verbs — `desugar_*` (pre-check, fallible) and `lower_*`
  (post-check, infallible) — and rename all 17.
- **Option B:** keep the names; replace the 17-deep nest with a flat `let`-chain or a
  `[fn; N]`-style pass list so the order is readable top-to-bottom.
- **Option C:** both.
- **RECOMMENDED: C, but B first.** B is a pure-mechanical, zero-semantic-risk change that fixes the
  worst half of the readability problem in one commit and makes the pass order auditable (which
  matters: Invariant 5 depends on pass *ordering*, and the DEC comments at `:180`, `:186`, `:190`,
  `:194`, `:206` are all order constraints that are currently invisible in the control flow). A is
  larger and touches 17 public names across the crate, so it wants its own commit — but note that
  a flat list makes A's diff trivially reviewable afterwards.

### G7 — the `desugar_*` / `rewrite_*` filename convention is real but violated twice, and 7 of 8 files export a mismatched fn name — P2
[Verified: read all 12 pass headers + every `pub fn` + all `cli/pipeline.rs` call sites]

The intended axis is **pipeline phase**, and it is a good one:
- `desugar_*` = pre-check, returns `Result<Program, Vec<Diagnostic>>` (can diagnose)
- `rewrite_*` = post-check, infallible `Program → Program` (consumes checker side-tables)

Two hard violations:

| file:line | current | problem | proposed |
|---|---|---|---|
| `src/checker/desugar_variadics.rs:12` | `desugar_variadic_params` | Prefixed `desugar_` but is a **post-check** pass — called at `cli/pipeline.rs:208` inside the post-check nest, infallible, and its own doc at `desugar_variadics.rs:5` says *"a program reaching this post-check pass"* | `checker/lower_variadics.rs` / `lower_variadic_params` |
| `src/checker/rewrite_pipe/` + `mod.rs:53` | dir `rewrite_pipe/`, fn `lower_pipes` | Prefixed `rewrite_` but runs **FIRST, pre-check** (`cli/pipeline.rs:64`, `:260`, `:569` — three call sites each commenting "pipes lower FIRST"). The same directory also holds `materialize_pipe_params`, which *is* post-check — one directory straddling both phases under the wrong prefix | split `checker/desugar_pipe/` (pre-check) + fold `materialize_pipe_params` into the post-check set |

And the prefix↔function mismatch: of the **8** `checker/rewrite_*.rs` files, only **1**
(`rewrite_ufcs.rs`) exports a function whose name matches its filename. `grep rewrite_generics`
does not find the function that file exists to hold (it is `erase_generics`).

- **Option A:** fix both phase violations + rename each file's public fn to match its filename.
- **Option B:** fix only the two phase violations (the actively-misleading half) and leave the
  prefix/fn mismatch.
- **RECOMMENDED: A**, folded into G6's option A so it is one rename commit, not two. The
  greppability loss is the real cost here: a newcomer who reads Invariant 5 and greps for the pass
  named in a filename finds nothing.

### G8 — `rt` denotes three unrelated concepts; one of those has four spellings — P2
[Verified: read all six bodies]

| file:line | name | meaning | proposed |
|---|---|---|---|
| `src/interpreter/mod.rs:66` | `fn rt<T>(msg) -> R<T>` | **runtime fault** constructor — ~80 call sites across `interpreter/`, two letters, **no `///`** | `runtime_fault` |
| `src/jit/boxed.rs:82-288` (13 fns) | `rt_push_int`, `rt_arith`, `rt_cmp`, … | **runtime bridge** (`extern "C"` trampolines) | `jitrt_*` |
| `src/checker/rewrite_alias.rs:23` | `fn rt(ty, a, depth)` | **rewrite type** | `rewrite_type` |
| `src/checker/collapse_injected.rs:38` | `fn rt(ty)` | rewrite type | `rewrite_type` |
| `src/checker/rewrite_generics.rs:44` | `fn rty(ty, params)` | rewrite type — **same concept, 2nd spelling** | `rewrite_type` |
| `src/checker/desugar_db.rs:1556` | `fn retype(&mut self, t)` | rewrite type — **3rd spelling** | `rewrite_type` |

The last four are structurally the same walk (`match ty { Type::Named { name, args, span } => …
args.iter().map(recurse) … }`), which is also a **duplication** finding — see G21.

- **Option A:** rename all six per the table.
- **Option B:** rename only the `rewrite_type` family (4 sites, mechanical, also enables the G21
  de-duplication) and leave `interpreter::rt` and `jit::rt_*` alone as established local idiom.
- **RECOMMENDED: B** — then reconsider `interpreter::rt` separately, since 80 call sites is a large
  diff for a name that is at least *consistent* within its module. Its missing `///` is the cheaper
  half of that fix and should happen regardless.

### G9 — `vm/exec.rs:9` — two untyped `usize` indices in the hottest 1044-line function, one actively misleading — P2
[Verified: read `src/vm/exec.rs:9,16,30,176,188,740` and `src/vm/mod.rs:113`]

```rust
pub(super) fn exec_op(&mut self, op: &Op, fr: usize, func: usize) -> Result<Flow, String> {
```
`fr` indexes `self.frames`; `func` indexes `self.program.functions`. Neither name says "index", and
`func` is actively wrong because `chunk::Function` is a real type (`src/chunk/mod.rs:156`) — so
`func: usize` reads as a function *value*. The same field name is on the struct
(`src/vm/mod.rs:113` `Frame { func: usize }`), so `self.frames[fr].func` is "frame index →
function index" with neither word present.

House convention is `_idx` (`idx` 530 occurrences vs `index` 557 vs `slot` 728 — all three live,
`_idx` is the established *suffix*).

- **Option A:** rename to `frame_idx` / `func_idx` at the signature and `Frame { func_idx }`.
- **Option B:** introduce newtypes `FrameIdx(usize)` / `FuncIdx(usize)` so the compiler prevents
  the confusion rather than the reader.
- **RECOMMENDED: A.** B is more correct in principle but this is the hot loop and Invariant 11
  forbids a perf change without a measured before/after — a newtype wrapper *should* be zero-cost
  but proving it costs a `phg benchmark` cycle. A is free and captures most of the value.

### G10 — five competing conventions for the recursive-walker helper family, several in 180-230-line functions — P2
[Verified: read the cited signatures and bodies]

| convention | count | sites |
|---|---|---|
| `r`-prefix (`rexpr`/`rstmt`/`rblock`/`ritem`) — majority | ~30 | `rewrite_ufcs.rs:66,295,417`, `rewrite_html.rs:18,215,338`, `resolve_variant_imports.rs:165,214,434,558`, `desugar_db.rs:2626,2648,2706,2759`, … |
| `q`-prefix (`qe`/`qs`/`qp`/`qblock`) | 4 | `qualify_variants.rs:95,101,126,205` |
| `walk_*` | 8 | `enforce_injected.rs:130-320`, `rewrite_foreach.rs:117,132` |
| `visit_*_mut` | 2 | `rewrite_pipe/walk.rs:11,26` |
| `rt`/`rty`/`retype` | 4 | see G8 |

Worst individual cases: `rewrite_ufcs.rs:66` `rexpr` is **229 lines**; `qualify_variants.rs:205`
`qe` spans **~180 lines** (`:205` to EOF at `:387`) — both far past any "short closure" exemption.
And `walk_*` vs `visit_*_mut` looks like a read-only/mutating split but isn't:
`rewrite_foreach.rs:117,132` uses `walk_*` for a **mutating** walk over `&mut [Stmt]`.

Compounding it, the side-table each pass consumes is named six different single letters:
`u: &Map` (`rewrite_ufcs.rs:66`), `h: &Map` (`rewrite_html.rs:18`), `m: &VarMap`
(`resolve_variant_imports.rs:214`), `a: &Aliases` (`rewrite_alias.rs:23`), `r: &[Route]`
(`desugar_router.rs:236`), `inv`/`ts` (`rewrite_invoke_tostring.rs:40`).

- **Option A:** standardise on `visit_expr`/`visit_stmt`/`visit_block`/`visit_item` (+ `_mut`), and
  name each side-table param after itself (`ufcs`, `html`, `variants`, `aliases`, `routes`).
- **Option B:** standardise on the existing majority `rewrite_expr`/`rewrite_stmt`/… since these
  are rewriters, not visitors, and reserve `visit_*` for the read-only walkers.
- **RECOMMENDED: B.** It matches the majority convention already in the tree (~30 sites keep their
  semantics, only the spelling expands from `rexpr` to `rewrite_expr`), and it preserves a real
  distinction — `enforce_injected.rs`'s walkers genuinely only read. It also makes the G16
  exhaustiveness work easier to review, because the walkers become greppable as one family.

### G11 — remaining vocabulary drift, with whole-corpus counts — P3
[Verified: word-boundary counts over all 566 files; every cited line read]

| Concept | Spellings + counts | Sites of the minority | Proposed |
|---|---|---|---|
| source span | **`span` 224** vs `sp` 34 | `rewrite_pipe/mod.rs:53,137`, `rewrite_pipe/materialize.rs:41`, `desugar_router.rs:127,167` | `span` throughout. *(Good news: `at`/`loc`/`pos`/`line_col` as span params = **0** — this axis is otherwise clean)* |
| package | **`package` 1033** vs `pkg` 76 | `loader/imports.rs:49,64,100,103,170,229,239`, `loader/assemble.rs:58`, `loader/import_hygiene.rs:14,90` | `package` / `package_path`. Mixed *within one line* at `loader/assemble.rs:78`: `mangle(&prog.package, name)` keyed by `(pkg.clone(), …)` |
| validation verb | **`check_` 38** vs `validate_` 7 vs `verify_` vs `ensure_` | `checker/throws.rs:137,151`, `checker/collect/functions.rs:127,280,421`, `checker/collect/types_decls.rs:10`, `pm/ops.rs:84`, `jit/range_acc.rs:370`, `bundle/cross.rs:78` | `check_` for diagnostic emission, `ensure_` for fallible side-effecting setup; retire `validate_`/`verify_` |
| fault text | `FaultMsg` / `fn message()` / doc says "body" / bound as `body` | `chunk/mod.rs:48,73`; `vm/exec.rs:7`; `vm/mod.rs:464` binds it `body`; `diagnostic.rs:62` names the analogue `message` | align on `message` |
| "builtin" | `native` 865, `builtin` 56, `intrinsic` 57, `stdlib` 37 — **each a genuinely distinct concept**, correctly used | one snag: `native/mod.rs:1` glosses "native (**built-in**) function registry", equating native≡builtin, while `loader/imports.rs:291` `is_builtin_type_leaf` means a *Phorj* built-in type and `native/mod.rs:409` means a *PHP* builtin — three senses in one crate | drop "(built-in)" from `native/mod.rs:1`; rename `is_builtin_type_leaf` → `is_primitive_type_leaf`; reserve `builtin` for PHP only |

- **RECOMMENDED:** fix `sp`→`span` and `pkg`→`package` (pure mechanical, 110 sites, zero risk) and
  the three-senses-of-builtin gloss; defer the `validate_`→`check_` sweep until it can ride along
  with G7's rename commit.

### G12 — misleading names — P2
[Verified: read all five]

| file:line | name | problem | proposed |
|---|---|---|---|
| `src/checker/desugar_db.rs:1048` | `fn validate_class(&mut self, …) -> bool` | Does **two** jobs — emits diagnostics into `self` *and* returns a validity bool. A caller cannot tell whether ignoring the bool is safe | split `check_class(&mut self)` + `is_valid_class(&self) -> bool` |
| `src/value/types.rs:277` | `fn get_field(&self, name) -> Option<Value>` | `get_*` implies a cheap borrow; this does a layout lookup **plus a `.clone()` out of a `RefCell`** (`:279-280`). Honestly documented at `:274`, but the name fights the doc | `read_field` or `field_cloned` |
| `src/value/types.rs:286` | `fn set_field(&self, …) -> bool` | `set_*` taking `&self` and mutating through `RefCell` — semantically required for handle values and documented at `:283-285`, but the signature alone reads as a no-op; also returns an ignorable bool | keep `&self`; change to `-> Result<(), UnknownField>` so the ignorable-bool ambiguity disappears |
| `src/checker/desugar_variadics.rs:12` | `desugar_variadic_params` | Name asserts a pipeline phase it does not occupy (G7) | `lower_variadic_params` |
| `src/jit/range_acc/verify.rs` | `fn verify_with_g(…, h: usize, e: usize, g: i64)` | 3 single letters among 10 params; only `g` is documented, and `g` is baked into the function name | `check_at_trip_bound(header_ip, exit_ip, trip_bound)` |

- **RECOMMENDED:** `validate_class` first (P2 → it is the one where a caller can be *wrong*, not
  merely confused), then `get_field`/`set_field` as a pair, then the rest opportunistically.

### G13 — Rust API guidelines: near-clean, and that is a real result — P3 (attestation with one arguable item)
[Verified: exhaustive regex over all 566 files]

| check | result |
|---|---|
| `into_*` that borrows (`fn into_\w+\(&`) | **0** |
| `as_*` that allocates (`fn as_\w+.*-> (String\|Vec<\|Cow)`) | **0** |
| `new` that can fail (`fn new\(.*\) -> (Result\|Option)`) | **0** |
| `is_*`/`has_*` returning non-bool | **0** — every one returns `bool` |
| `_mut` that doesn't mutate | 2 hits, both genuine |
| `to_*` consuming `self` | 1 hit — `profile.rs:62 to_flag_bit(self)`, and `Profile` is `Copy`, so correct |

One arguable item: `src/ext/registry.rs:278` `pub fn disabled() -> impl Iterator<Item = &'static
Extension>` is the only iterator-returning fn not named `iter*`.
- **RECOMMENDED:** rename to `iter_disabled`. Trivial, and it is the last item in an otherwise
  perfect sweep.

---

## Dimension 3 — Documentation clarity

### G14 — `docs/INVARIANTS.md` §1 is corrupted by a global find-replace AND is now semantically wrong — P0 (highest-severity finding in this audit)
[Verified: read `docs/INVARIANTS.md:19-27`; `grep -n "fn built_binary\|fn cross_musl" tests/build.rs`; read `src/main.rs:16-32`]

A global text replacement of `runvm` → `the VM leg` was applied across `docs/` and it **rewrote
identifiers**. `docs/INVARIANTS.md:19-21,27` now reads:

> `  \`cli::cmd_the VM leg\` at startup (the self-detect hook in \`src/main.rs\`), so its output MUST equal`
> `  \`phg run <file>\`. **Enforced by** \`tests/build.rs::built_binary_matches_the VM leg\`. The startup`
> `  hook must keep dispatching through \`cmd_the VM leg\` (never \`cmd_run\`) and must not transform the source`
> `    \`cross_musl_binary_matches_the VM leg\` (native exec) …`

Three separate failures, escalating:

1. **The enforcement pointers are unresolvable.** The real test names are
   `built_binary_matches_vm` (`tests/build.rs:204`) and `cross_musl_binary_matches_vm`
   (`tests/build.rs:140`). Grepping for what the doc says finds nothing.
2. **The identifier `cmd_runvm` no longer exists at all.** `src/cli/pipeline.rs` exports `cmd_run`
   and `cmd_treewalk`; there is no `cmd_runvm`.
3. **The invariant now states the opposite of what the code does.** De-corrupted, the text reads
   *"must keep dispatching through `cmd_runvm` (**never `cmd_run`**)"*. But `src/main.rs:32` is
   literally `match cli::cmd_run_exit(&src) {`. Per CLAUDE.md Invariant 1, `run` **is** the VM
   engine now — so the code is right and the invariant is a lie. This is in the file CLAUDE.md
   instructs Claude to *"read before touching backends"*, and its warning clause forbids exactly
   what the correct code does.

**Concrete fix** for `docs/INVARIANTS.md:19-27`: `cli::cmd_the VM leg` → `cli::cmd_run_exit`;
`built_binary_matches_the VM leg` → `built_binary_matches_vm`;
`cross_musl_binary_matches_the VM leg` → `cross_musl_binary_matches_vm`; and re-express the warning
clause in post-rename terms — the intent was *"go through the VM engine, not the tree-walker"*,
which today reads **"never `cmd_treewalk`"**.

Same corruption, other files [Verified: `grep -rn "the VM leg"`]:
`docs/ARCHITECTURE.md:89` (`cmd_the VM leg`), `docs/adr/0001-no-shared-run-vm-ir.md:13`
(`cmd_the VM leg`), `docs/plans/MASTER-PLAN.md:1246` (`run+the VM leg`), `:1683`,
`docs/plans/SLICE-STATE.md:2116`, `KNOWN_ISSUES.md:2275`.

- **Option A:** fix all 7 sites and re-express the semantically-stale clause.
- **Option B:** fix the 4 in `INVARIANTS.md` + `ARCHITECTURE.md` (the read-before-work docs) now;
  treat MASTER-PLAN/SLICE-STATE/KNOWN_ISSUES as append-only logs and leave them.
- **RECOMMENDED: A**, with the ADR handled per G15. This is the one finding in the report that is
  unambiguously a correctness hazard for a future session and not a matter of taste.

### G15 — `docs/adr/0001` states a fact that is false, in a record declared immutable — P1
[Verified: `grep -rnE '^\s*(pub )?trait ' src/ | grep -v test` → 6 traits]

`docs/adr/0001-no-shared-run-vm-ir.md:13`:
> `(\`cmd_run\` / \`cmd_the VM leg\` / \`cmd_transpile\`); \`grep 'trait ' src/\` returns zero.`

`docs/ARCHITECTURE.md:89-90` makes the *same* claim with a different wrong number:
> `finds 4 traits (\`Transport\`, \`DebugFrontend\`, \`Suspend\`, \`Task\`), none a backend abstraction`

Actual count is **6**: `src/green/exec.rs:41 Suspend`, `src/green/exec.rs:65 Task`,
`src/value/db.rs:23 DbObject`, `src/serve/handlers.rs:129 Transport`, `src/debug.rs:62
DebugFrontend`, `src/ext/database/natives/driver.rs:17 DriverConn`. So: the ADR says 0,
ARCHITECTURE says 4, reality is 6 — three numbers, one grep.

The bite is procedural: `docs/ARCHITECTURE.md:97` declares ADRs **"immutable once Accepted"**, and
`:106` says *"a reversal supersedes the ADR rather than editing it"*. The ADR's *decision* is still
sound (no `Backend` trait — none of the 6 is a backend abstraction). Only a factual aside is wrong.

- **Option A:** treat the corrupted identifier + the stale count as **errata**, not a reversal —
  amend in place with a dated `> Errata (2026-07-25):` note under the Context section. Immutability
  protects the *verdict*, not typos.
- **Option B:** write ADR-000N superseding 0001 solely to fix a count. (Heavy; loses the point of
  the immutability rule.)
- **Option C:** fix only `ARCHITECTURE.md`'s number, leave the ADR untouched.
- **RECOMMENDED: A**, and fix ARCHITECTURE.md's "4 traits" to "6 traits, none a backend
  abstraction" in the same change. The immutability rule exists so decisions aren't quietly
  rewritten; an errata note is visible and preserves the original text. **This is a governance
  question about `docs/adr/README.md`'s own rules — the developer's call, not mine.**

### G16 — 18 of 27 spec/plan paths cited in Rust comments point at deleted files — P1
[Verified: extracted every distinct `docs/**.md` path from `src/**/*.rs` comments (27 unique), tested each with `[ -f ]`, and checked each miss against `docs/specs/archive/`]

| Outcome | Count |
|---|---|
| Resolves | 6 |
| **Recoverable — moved to `docs/specs/archive/`** | **3** (`2026-06-27-dependency-policy.md`, `2026-06-28-public-surface-file-rule-design.md`, `2026-06-28-secret-type-design.md`) |
| **Truly gone — not in `archive/` either** | **18** |

The 18: `docs/plans/{2026-06-25-overnight-design-forks-review.plan.md, di-attributes.plan.md,
perf-wave.plan.md}` and `docs/specs/{2026-06-15-m2-bytecode-vm-design.md,
2026-06-16-m2-p5-object-model-design.md, 2026-06-18-m3-namespace-system-design.md,
2026-06-18-m5-project-model-design.md, 2026-06-25-core-reflect-design.md,
2026-06-25-process-io-quarantine-seam-design.md, 2026-06-26-core-json-design.md,
2026-06-26-m4-casting-conversion-design.md, 2026-06-27-class-entry-points-design.md,
2026-06-27-m4-stdlib-charter.md, 2026-06-28-core-regex-design.md, 2026-06-28-lsp-design.md,
2026-06-28-m-time-design.md, 2026-06-28-m6-w3-serve-concurrency-design.md,
2026-06-29-m6-w4-green-threads-design.md}`. [Verified: `ls docs/plans/` → only `MASTER-PLAN.md`
and `SLICE-STATE.md`]

This is not cosmetic, because of *what those comments say*. `src/green/mod.rs:4`:
> `//! The architecture (**developer-locked**, \`docs/specs/2026-06-29-m6-w4-green-threads-design.md\`):`

The lock is unverifiable — the locked document is gone. `src/native/mod.rs:11` cites the namespace
design as "the load-bearing target of `import Core.Output;`" — gone. `src/lsp/mod.rs:1`,
`src/vm/mod.rs:1`, `src/chunk/mod.rs:2`, `src/serve/mod.rs:16` — all gone.

**This is a measurable violation of the project's own Invariant 19**, which mandates *"every
roadmap item, decision, and slice-status lives in exactly ONE canonical place and **everything else
points to it**"*. 18 pointers point at nothing. The content was presumably consolidated into
`docs/specs/UNIFIED-SPEC.md` (which CLAUDE.md names as canonical), so the pointers have a valid
target — they just weren't re-pointed when the consolidation happened.

- **Option A:** re-point all 21 misses (18 → `docs/specs/UNIFIED-SPEC.md §<section>`, 3 →
  `docs/specs/archive/…`) **and** add a CI check that every `docs/…md` and `src/…rs` path appearing
  in a comment resolves. The check is ~15 lines of bash and is the only thing that stops this
  recurring.
- **Option B:** re-point only the ~6 that make a *normative* claim ("developer-locked",
  "load-bearing", "per the spec") and delete the rest of the citations as noise.
- **RECOMMENDED: A.** The CI check is the actual fix; without it this recurs at the next
  consolidation, and G4/G5/G17 are the same disease. Note the check also catches G4's
  `ast/classes.rs` and every finding in G17 automatically — one gate, five findings closed.

### G17 — ~30 comment references to `foo.rs` paths that M-Decomp turned into `foo/mod.rs` — P2
[Verified: each path tested with `ls`; each citation read]

| Dead path | Real path | Sites |
|---|---|---|
| `src/vm.rs` | `src/vm/exec.rs` | `chunk/validate.rs:42`, `interpreter/expr.rs:205`, `value/core_impl.rs:181` |
| `src/compiler.rs` | `src/compiler/emit.rs:75` | `chunk/validate.rs:43` |
| `value.rs` | `src/value/{arith,core_impl,types}.rs` | `value/types.rs:174,186`, `green/mod.rs:7`, `limits.rs:42`, `jit/emit_unboxed/mod.rs:54`, `jit/boxed.rs:50` — **6 sites, and each is asserting the Invariant-4 single-sourcing claim** |
| `value.rs`/`parser.rs`/`checker.rs` | `…/mod.rs` | `limits.rs:3-4` — **three dead paths in one sentence** |
| `src/ext/database/natives.rs` | `…/natives/mod.rs` | `value/db.rs:4,20`, `value/types.rs:187`, `ext/database/prelude.rs:53` |
| `src/parser/exprs.rs` | `…/exprs/mod.rs` | `format/printer/atoms.rs:148`, `lift/printer/exprs.rs:355` (identical duplicated line) |
| `src/lift/printer.rs` | `…/printer/mod.rs` | `format/printer/mod.rs:2` — and this is the *seam-defining* sentence of that module doc |
| `transpile/program.rs` | `src/transpile/program_emit.rs` | `native/option.rs:112`, `native/result.rs:154`, `ext/json/natives.rs:7`, `ext/regex/natives.rs:286` |
| `compile.rs` (in `src/jit/`) | `src/jit/compile/mod.rs` | 6 sites; worst is `jit/handles/symbols.rs:1-3`, whose whole module doc is about keeping "in lockstep with … the `declare` list in `compile.rs`" — a file it misnames twice |
| `exec.rs` (as a `src/jit/` sibling) | `src/vm/exec.rs` | `jit/boxed.rs` ×8 (lines 93, 107, 158, 180, …) — a reader looks for a sibling and finds nothing |

- **RECOMMENDED:** fold into G16 option A — one mechanical sweep, one CI check covering both
  `src/…rs` and `docs/…md` citation targets.

### G18 — `src/chunk/op.rs:1-2` — the comment that *is* Invariant 3's enforcement mechanism sends you to two nonexistent paths — P1
[Verified: read `src/chunk/op.rs:1-2`; located all three matches]

```
//! The `Op` set — every variant extends THREE exhaustive matches in the same commit:
//! `vm::exec_op`, `BytecodeProgram::validate`, `compiler::stack_effect` (Invariant 3).
```
- `BytecodeProgram::validate` — **correct** (`src/chunk/validate.rs:21`).
- `vm::exec_op` — wrong: it is a `pub(super)` **method on `Vm`** in `vm::exec` (`src/vm/exec.rs:9`).
- `compiler::stack_effect` — wrong: a `pub(in crate::compiler)` **method on `Compiler`** in
  `compiler::emit` (`src/compiler/emit.rs:75`).

Severity is elevated because this comment is the *only* thing telling a newcomer where to go, and
CLAUDE.md Invariant 3 + `docs/INVARIANTS.md:62-64` repeat the same two stale paths (`src/vm.rs`,
`src/compiler.rs`, `src/chunk.rs`).

- **RECOMMENDED:** fix the three doc sites (`chunk/op.rs:1-2`, `docs/INVARIANTS.md:62-64`,
  CLAUDE.md Invariant 3) to `src/vm/exec.rs:9` / `src/chunk/validate.rs:21` /
  `src/compiler/emit.rs:75` — and note that the G16 CI check would have caught two of the three.

### G19 — `src/green/` documents itself as unshipped while shipping — P1
[Verified: read `src/green/mod.rs:1-30`, `src/cli/pipeline.rs:367-369`, `ls src/green/`]

`src/green/mod.rs:13`:
> `//! This module currently contains only the kernel; the executor wiring lands in the next build steps.`

Contradicted **two lines later in the same file** by `pub mod exec;` (`:15`) and by `pub mod coro;`
(`:22`, commented "Native coroutine bridge (S4.3 step 3b-2a) — corosensei↔run_loop glue"), and by
`src/cli/pipeline.rs:367-369`: *"S4.3 cutover: a program that uses `spawn` runs on the cooperative
green-thread driver (real task interleaving)"*.

Two related staleness sites:
- `src/green/spike.rs:8-9` — *"it is a feasibility probe, **deleted once** the real executor (step
  3b) lands."* Step 3b landed (`src/green/coro.rs:1` says "step 3b-2a"); `spike.rs` is still
  present and still `mod spike;` at `src/green/mod.rs:26`.
- `src/serve/mod.rs:14-16` — *"This supersedes the old 'green-threads' plan (which would have been
  single-core + needs unstable/unsafe std machinery)"*. Both halves false: green threads shipped,
  and the "needs unsafe" claim is refuted by `src/green/spike.rs:6` (*"No `unsafe` in our crate"*).
  A reader trusting `serve/mod.rs` concludes green threads don't exist and can't be done safely.

- **Option A:** update the three docs to reflect shipped state and delete `spike.rs` per its own
  stated contract.
- **Option B:** update the docs but keep `spike.rs` as a regression test that deep suspension still
  works without `unsafe` — and rewrite its header to say so, since that is a legitimate reason to
  keep a "spike".
- **RECOMMENDED: B.** The spike encodes a real invariant (`#![deny(unsafe_code)]` compatibility of
  deep suspension) that nothing else tests. Keeping it is right; describing it as pending deletion
  is what's wrong.

### G20 — the `#![forbid(unsafe_code)]` comment cluster: 5 sites, 3 lying — P2
[Verified: `grep -rn "forbid(unsafe_code)\|forbidden" src/`; read `src/lib.rs:1-10`, `src/jit/mod.rs:74-80`]

Ground truth: **there is no `#![forbid(unsafe_code)]` anywhere in the tree.** `src/lib.rs:10` and
`src/main.rs:5` are `#![deny(unsafe_code)]`, and `src/jit/mod.rs:74-80` correctly documents the
relaxation and carries the scoped `#![allow(unsafe_code)]` island.

| file:line | text | severity |
|---|---|---|
| `src/green/spike.rs:2` | `works under phorj's \`#![forbid(unsafe_code)]\`` | **lying** — and `src/lib.rs:8-9` says the wording there deliberately avoids the literal token *because a CI grep matches it*; this comment is exactly that false positive |
| `src/serve/handlers.rs:10` | `so phorj's code stays \`#![forbid(unsafe_code)]\`` | **lying** |
| `src/bundle/mod.rs:3` | `\`unsafe\` is **forbidden crate-wide** (lib.rs)` | **lying twice** — denies not forbids, and it is not crate-wide (`src/jit/mod.rs:80` is an audited island) |
| `src/phstr.rs:13` | `this crate forbids \`unsafe\` outside \`src/jit/\`` | imprecise only — semantics correct (the CI `unsafe-island` gate makes it a build failure), verb wrong |
| `src/native/process_tests.rs:104` | `The crate forbids \`unsafe\`` | imprecise only |

*(Correction to an intermediate claim in my own sweep: `src/serve/mod.rs` does **not** contain this
string — only `serve/handlers.rs:10` does. [Verified: read `serve/mod.rs:1-16`])*

- **RECOMMENDED:** fix the 3 lying sites to "`deny(unsafe_code)`, with the audited `src/jit/` island"
  and add `forbid(unsafe_code)` to the CI `unsafe-island` gate's grep so the phrase cannot
  reappear. `phstr.rs`/`process_tests.rs` are fine as prose.

### G21 — module-doc and public-item coverage: strong, with two specific gaps — P2
[Verified: scripted `head`/`grep` over all 424 non-test files and all 88 module roots]

**Module-level `//!`: 407 of 424 non-test files (96%)** carry one. That is excellent and should not
be re-litigated. But the 17 misses are **systematic, not random**:

- **7 are the compile-time-sugar passes** — `checker/{collapse_injected, enforce_injected,
  rewrite_alias, rewrite_generics, rewrite_html, rewrite_invoke_tostring, rewrite_ufcs}.rs`. These
  are the *most subtle* part of the pipeline (Invariant 5's core, the thing a newcomer most needs
  explained) and they are the undocumented ones.
- **8 are `native/{bytes, file, html, list, map, math, set, text}.rs`** — the stdlib surface.
- **`src/lib.rs` has zero `//!` lines** [Verified: `grep -c '^//!' src/lib.rs` → 0]. It opens with
  nine `//` lines about `unsafe`, then `#![deny(unsafe_code)]`, then 36 bare `pub mod`
  declarations. **`cargo doc` renders the `phorj` crate landing page empty.** For a language
  implementation this is the single highest-leverage missing doc in the project.

**Quality — the good ones are genuinely good.** `src/limits.rs:1-11` is the gold standard: it
states *why* centralisation ("symmetry as policy"), the invariant upheld ("keep adversarial-but-
bounded input faulting *cleanly* rather than overflowing the native stack (SIGABRT)"), and why the
limits are reachable at all (the 256 MB worker). `src/serve/mod.rs:1-16` defines the module as a
*seam* ("the ONE place sockets + wall-clock non-determinism live, kept deliberately OUTSIDE the
byte-identity spine"). `src/phstr.rs:1-20` states a hard invariant plus its proof obligation.
`src/jit/mod.rs:68-79` ("## The unsafe island") is the best rationale doc in the crate.

**Quality — the vacuous ones describe the refactor, not the module:**
`src/compiler/stmt/mod.rs:1`, `src/checker/stmt/mod.rs:1`, `src/compiler/expr/mod.rs:1`,
`src/parser/exprs/mod.rs:1`, `src/parser/items/mod.rs:1` — all of the form
`//! \`impl Compiler\` — stmt cluster, split by statement family.` A reader learns nothing `ls`
wouldn't tell them. Plus **12 near-identical `ext/*/mod.rs` headers** of the form `//! The \`hash\`
extension (DEC-273 wave 2): natives + tests colocated per AMENDMENT 2.` — they say which *wave*
shipped them, never what they do. `src/ext/database/mod.rs:1-3` breaks the template to say
"multi-driver SQL natives", proving the template is a choice.

**Public `///` coverage: 602 public items crate-wide, ~55 genuinely undocumented (≈91%)**. Clean
sweeps worth naming: `src/diagnostic.rs`, `src/limits.rs`, `src/native/mod.rs`, and **all** of
`src/loader/` have zero undocumented public items. The misses are concentrated in the *most central
types*, presumably because they felt obvious:

| file:line | item | note |
|---|---|---|
| `src/value/types.rs:118` | `pub enum Value` | the single most important type in the runtime; its *variants* are lavishly documented (the `Decimal` variant gets 6 lines at `:120-125`), only the enum is bare |
| `src/ast/types_core.rs:6` | `pub enum Type` (+ `MatchArm`, `UnaryOp`, `BinaryOp`) | — |
| `src/checker/mod.rs:382` | `pub struct Checker` | its *private* fields `funcs`/`sealed` have multi-line `///`; the struct does not. `pub fn check` also bare |
| `src/token.rs:6` | `pub struct Span` (+ `Token`, `TokenKind`, `CommentKind`) | all four public items bare, despite the module `//!` specifically calling `Span` "the single source of source-position truth" |
| `src/cli/pipeline.rs:48,363,407,662` | `check_and_expand`, `cmd_treewalk`, `cmd_run`, `transpile_program` | the CLI's four primary entry points, all bare — while the *private* helper `render_all` at `:43` has a `///`, and `check_and_expand_reified` at `:53` has a 4-line doc that links to `[check_and_expand]`, i.e. **a documented function links to an undocumented one** |
| `src/phstr.rs:45` | `pub enum PhStr` | module doc excellent, type bare |
| `src/vm/mod.rs:89` | `pub struct Vm` | — |

*(False positives I ruled out so they aren't chased: `value/arith.rs:222,225` are covered by a group
`///` at `:218`; `chunk/mod.rs:48 FaultMsg` **does** have a `///` at `:42-44` separated by an
intervening `//` note; `value/types.rs:336-348` are trivial accessors next to a documented `iter`.)*

- **Option A:** a `//!` on `src/lib.rs` + `///` on the 7 central types/entry points above + `//!`
  on the 7 sugar passes. ~15 items, high leverage.
- **Option B:** all of A plus a `#![warn(missing_docs)]` on the crate root to prevent regrowth.
- **RECOMMENDED: A now, B as a separate decision.** B is attractive but `[lints] warnings = "deny"`
  means `missing_docs` would immediately fail the build on ~55 items — so B is a
  "fix-all-55-first" commitment, not a one-liner. Worth doing, worth scheduling deliberately.

---

## Dimension 4 — "No shortcuts unless earned" (Anti-bandaid gate)

### Census [Verified: scripted counts over the 424 non-test-named files, then hand-classified]

| Pattern | Raw count | Genuine production sites |
|---|---|---|
| `todo!` / `unimplemented!` | **0** | **0** |
| `panic!` | 18 | **0** — all 18 are inside in-file `#[cfg(test)] mod` blocks [Verified: read `value/mod.rs:215-230` (a test helper `assert_dec`), `checker/qualify_variants.rs:344-370`, `native/log/mod.rs:347`, `ext/session/natives.rs:314-363`] |
| `unwrap()` | 217 | ⚠ **CORRECTED to 26** by the certification pass — this row said **≈20** from a census that excluded whole *files* containing `cfg(test)`, which drops production code in those files. Correct method: exclude test files and `cfg(test)` **blocks**, keep production code beside them → **26 production sites**. Five were therefore never read: `src/pm/resolve.rs:93`, `src/pm/manifest.rs:196`, `src/bundle/sha256.rs:29`, `:30`, `src/native/random.rs:147` (`draw.try_into().unwrap()`, in the RNG path). The 566-files / 154,817-lines figures are exact. |
| `unreachable!` | 80 | mostly guarded by a proven invariant, e.g. `vm/exec.rs:893` `_ => unreachable!("receiver kind changed within one op")` with the preceding comment *"`layout_ptr` above already proved this is an `Instance`"* |
| `let _ =` | 88 | see G23 |
| `#[allow(...)]` | **65** | see G22 |
| `unsafe` outside `src/jit/` | **0 code sites** | see attestation A1 |

**This is a genuinely clean bill on the loudest patterns.** Zero `todo!`, zero production `panic!`,
and 26 production `unwrap()` in 155k lines is exceptional discipline. The findings below are about
*justification density*, not volume.

### G22 — the 65 `#[allow(...)]` opt-outs: 35 are one clippy lint, 17 are retained dead code — P2
[Verified: full enumeration via `grep -rn "#!\?\[allow(" src/`]

**Full list, grouped.** Every one is an explicit opt-out of `[lints] warnings = "deny"`.

**(a) `clippy::too_many_arguments` — 35 sites.** This is not 35 independent decisions; it is one
design signal repeated 35 times.
- `src/jit/emit_unboxed/`: `mod.rs:64`, `scalar.rs:48`, `verticals.rs:636,843`,
  `verticals_map.rs:179`, `verticals_hof.rs:32,223,328`, `index_lists.rs:10,92`,
  `objects.rs:93,185,281`, `call_plumbing.rs:68,164,186,276,336` — **18 sites**, 14 annotated
  `// emit plumbing`
- `src/jit/`: `handles/mod.rs:1001` (`// fixed extern "C" shape: 6 part registers + masks` — the
  one genuinely *earned* instance: an ABI shape you cannot change), `boxed.rs:399`,
  `range_acc.rs:369,512` (`// analysis plumbing`)
- `src/checker/`: `calls/dispatch_intersection.rs:13`, `calls/methods.rs:10`,
  `calls/overloads.rs:297,530`, `calls/dispatch_named.rs:13`, `overloads.rs:144,442`
- `src/compiler/`: `emit.rs:9`, `ctors.rs:11,124`
- `src/interpreter/engine.rs:210`, `src/lsp/completion/mod.rs:39`

**(b) `clippy::dead_code` — 17 sites, all in `src/jit/`.** `handles/mod.rs:183,186,194,373`,
`handles/json_ext.rs:25,32`, `handles/helper_refs.rs:97,99,101,103`,
`analyze/kinds.rs:153,157,160,167,176`, `analyze/mod.rs:539`. The comments are honest about why:
four say `// read by the 5b Json emit arms; unused until they land`, one says
`// DEC-333: constructed by the refinement peephole in the next increment`. **This is code retained
for work that has not landed** — a real (if small) form of shortcut: the compiler is being told to
stop reporting that a slice is incomplete.

**(c) Justified singletons — 13 sites, and these are the model.** `value/core_impl.rs:82`
`#[allow(clippy::float_cmp)] // intentional: language-level float equality`;
`interpreter/kernels.rs:145` same lint `// intentional: literal float patterns match exactly`;
`json.rs:62 cast_possible_truncation`; `native/input.rs:70 option_option`;
`ext/database/natives/mysql.rs:212 cast_precision_loss`; `checker/resolutions.rs:9`,
`cli/pipeline.rs:56,307,326`, `ast/class_hierarchy.rs:282` `type_complexity`;
`jit/mod.rs:80 #![allow(unsafe_code)]` (the audited island, extensively documented at `:74-79`).

The `too_many_arguments` cluster is the finding. `src/jit/emit_unboxed/mod.rs:5-8` already documents
the fix pattern that solved this once:
> `Shared emit state crosses file boundaries via the Copy [\`Ec\`] context (which replaced the old captured closures).`

So the codebase already invented `Ec` for exactly this problem — 18 of the 35 sites are in the
module that has `Ec` and still pass 8+ loose arguments alongside it.

- **Option A:** widen `Ec` (or add a sibling `EmitArgs` struct) to absorb the loose parameters in
  the 18 `emit_unboxed` sites, deleting those `#[allow]`s. Same pattern for the 7 `checker/calls/`
  sites and the 2 `jit/range_acc.rs` "analysis plumbing" sites.
- **Option B:** accept the cluster as inherent to codegen plumbing and instead move the lint to a
  crate-level `#![allow(clippy::too_many_arguments)]` with one honest paragraph of justification —
  35 scattered opt-outs pretending to be individual decisions is less honest than one explicit one.
- **Option C:** leave as-is.
- **RECOMMENDED: A for `emit_unboxed` only** (where `Ec` already exists and the win is real), then
  **B for the remainder** if A doesn't naturally clear them. A blanket `#![allow]` is normally worse
  than local ones — but not when the local ones are 35 copies of the same non-decision. The
  `dead_code` cluster (b) is separate: recommend a tracking note in `SLICE-STATE.md` naming the 17
  sites so "unused until 5b lands" has a place it will actually be checked.

### G23 — 88 `let _ =` with essentially no inline justification; two are real bandaids and two hide dead code — P2
[Verified: full census + read every non-`remove_dir_all` site]

**The bulk is legitimate.** ~40 of the 88 are `let _ = std::fs::remove_dir_all(…)` /
`remove_file(…)` best-effort temp-directory cleanup in `src/pm/{vendor,resolve,ops,registry,fetch}.rs`
and `src/bundle/cross.rs` — a correct idiom for cleanup where failure is genuinely non-fatal.
**None of them carries a one-line comment saying so**, which is the pattern-level finding: the
project's Anti-bandaid gate requires the failure mode to be stated, and 40 identical undocumented
discards is where a real one will eventually hide.

Four specific sites:

| file:line | code | assessment |
|---|---|---|
| `src/dap.rs:50` | `let _ = write!(self.out, "Content-Length: {}\r\n\r\n{}", body.len(), body);` | **bandaid.** Silently swallows a write failure on the debug-adapter transport. If the DAP socket dies the debugger no-ops instead of reporting. No documented failure mode. Fix: propagate, or log once and set a `disconnected` flag |
| `src/dap.rs:51` | `let _ = self.out.flush();` | same |
| `src/checker/qualify_variants.rs:385` | `let _ = table;` | **`#[allow(dead_code)]` in disguise** — suppresses an unused-variable warning, invisible to the G22 allow-list audit |
| `src/bundle/macho.rs:125` | `let _ = sect_hdr_at;` | same |

Correctly documented counter-example, for contrast: `src/checker/calls/variants.rs:339`
`let _ = self.check_expr(call); // surface nested errors` — the comment explains that errors are
recorded in `self` rather than returned. That is the standard the other 87 should meet.

- **Option A:** fix the two `dap.rs` sites properly; convert the two disguised-dead-code sites to
  explicit `#[allow(dead_code)]` with a reason (so they show up in the allow audit); add a one-line
  reason comment to the `pm/` cleanup idiom **once per file**, not per site.
- **Option B:** only fix `dap.rs` (the sole behavioural bandaid) and leave the rest.
- **RECOMMENDED: A.** The `dap.rs` fix is the behavioural one; the two disguised sites matter
  disproportionately because they *evade the audit surface* the developer explicitly asked to see
  in full — an opt-out that doesn't appear in the opt-out list is the worst kind.

### G24 — 26 production `unwrap()` (⚠ this section originally said ~20 — CORRECTED by the certification pass; see the census row above), ~0 with a justification comment; 1 is a cross-module invariant with no test — P3
[Verified for 21 of 26 sites — ⚠ CORRECTED: the original census excluded whole files containing `cfg(test)`, so **5 production sites were never read** (`pm/resolve.rs:93`, `pm/manifest.rs:196`, `bundle/sha256.rs:29`, `:30`, `native/random.rs:147`). This stamp does NOT cover those five.]

Almost all are provably safe: `min()/max()` on a non-empty fixed array
(`jit/range_acc.rs:108,280,305`), `from_utf8` on an ASCII-only scanner slice
(`tokenizer/{scan.rs:139,174, ident.rs:10, strings.rs:551}`), `to_digit(16)` after
`is_ascii_hexdigit()`, `get_mut` on a key just inserted (`checker/collect/types_decls.rs:222,744`,
`checker/collect/interfaces.rs:122`), `next()` after a length check
(`ast/class_hierarchy.rs:263`, `parser/items/types/members.rs:122`), `as_ref().unwrap()` guarded by
an `all(|e| e.key.is_some())` two lines above (`lift/lifter/exprs.rs:179`).

Three observations:

1. **The justification discipline is inconsistent within 15 lines.** `src/tokenizer/ident.rs:10`
   `std::str::from_utf8(&self.src[start..self.pos]).unwrap()` has no comment; `ident.rs:23-25`,
   the very next function, documents the identical invariant beautifully:
   *"The source is always valid UTF-8 (it came from `&str`), so a char boundary is guaranteed at
   `self.pos`."* One of those two is the house standard; the other should adopt it.
2. **`src/compiler/stmt/core.rs:79-80` is a guard/unwrap double-lookup:**
   ```rust
   Expr::Member { object, name, .. } if self.static_slot(object, name).is_some() => {
       let idx = self.static_slot(object, name).unwrap();
   ```
   The method is called twice; clippy's `unnecessary_unwrap` does not fire because it is a call, not
   a binding. Fix: hoist to `if let Some(idx) = self.static_slot(object, name)` (or a `let`-chain).
   Same shape at `:90-91` with `hook_set_method`.
3. **`src/transpile/classes.rs:48` `self.emit_type(e.backing_type.as_ref().unwrap())`** depends on
   the checker having rejected every backed enum lacking a backing type. That is a real
   cross-module invariant with no comment naming it and no test pinning it.

- **RECOMMENDED:** fix (2) (mechanical, removes a redundant lookup in the compiler), add the
  one-line invariant comment to (3), and adopt `ident.rs:23-25`'s wording for the four tokenizer
  `from_utf8` sites. Leave the rest — they are correct, and 26 justified unwraps in 155k lines is
  ⚠ **(CORRECTED: read as 26, not 20 — and the 5 sites listed in the corrected census row above were NOT part of
  the "read every remaining site" set, so this "they are correct" assurance does not yet cover them.)**
  not a debt.

### G25 — `scripts/microbench.sh:144,146` discards the PHP leg's stderr — P3
[Verified: read `scripts/microbench.sh:28,135-155`; read `scripts/microbench-gate.sh:96-115`]

```bash
pline="$(taskset -c "$CPU" "$LOCAL_PHP" $OPCACHE_ARG $JIT_FLAGS "$MICRO/$name.php" 2>/dev/null)"
```
**Being fair: this is largely defended.** `scripts/microbench.sh:28` is `set -eEuo pipefail`, so a
non-zero php exit aborts the script rather than producing a bogus number; and
`scripts/microbench-gate.sh:98` blocks on `identical != true`, which would catch a
silently-empty PHP leg. So the WIN-OR-FLAG integrity of Invariant 18 holds.

What remains is diagnosability: when the PHP leg does fail, the operator gets an abort with **no
message**, because the explanation went to `/dev/null`. Fix: `2>"$errfile"` and `cat` it on
failure.

- **RECOMMENDED:** capture rather than discard. Low priority; the gate is sound.

---

## Dimension 5 — Duplication / single-sourcing vs Invariant 4

**Method:** extracted every 8-90-char lowercase string literal from
`src/{interpreter,vm,jit,transpile,native,value,chunk,ext}` (tests excluded), grouped by literal,
kept those appearing in >1 backend tree, then read every hit site. **I independently re-verified
every P0/P1 claim below by reading the cited lines myself** — the verifications are noted inline.

### G26 — `tests/differential.rs::classify` re-types the 12 canonical fault bodies as its own literals, so an *unclassified* duplicated fault can drift invisibly — P1 (this sets the risk scale for everything below)
[Verified: read `tests/differential.rs:126-236`]

`classify` hard-codes the classified fault bodies — `"integer overflow"`, `"division by zero"`,
`"modulo by zero"`, `"stack overflow"`, `"list index out of range"`, `"force-unwrap of null"`,
`"range too large"`, `"decimal division is not exact"`, `"recv from empty channel"`, `"join on an
incomplete task"`, `"no case of enum"`, `"no field"` — instead of referencing `value::FAULT_*` /
`FaultMsg`. Anything **not** in that list falls to `FaultKind::Other(full_string_incl_prefix)`, and
the VM prepends `at N:` while the interpreter does not — so **an unclassified fault body can never
be asserted equal by `agree_err`**.

And `agree_err_php` (`:206-236`) only asserts a **non-zero exit**, never message text — so every
PHP-side fault literal is entirely ungated.

Consequence: for a duplicated-and-unclassified fault literal, drift is not merely untested, it is
*invisible*. That is why G27-G31 below are ranked by classification status, not just site count.

- **Option A:** have `classify` reference `value::FAULT_*` / `FaultMsg::…::message()` instead of
  re-typing the strings. Converts the whole Tier-B class from "invisible" to "gated" in one change.
- **Option B:** extend `agree_err_php` to compare a normalised fault body too (accepting that some
  PHP messages are deliberately PHP-native — see G30).
- **RECOMMENDED: A.** It is a test-file-only change, it makes a rename of a canonical constant
  automatically propagate to the oracle, and it is the prerequisite that makes fixing G27-G29
  verifiable rather than hopeful. B is a bigger question entangled with Invariant 14's ladder.

### G27 — a canonical constant EXISTS and a backend re-inlines it anyway — 5 clusters, direct Invariant-4 breach — P1

**(a) `"force-unwrap of null"` — the constant exists; two of three legs bypass it.** RISK: HIGH
[Verified: read all four sites]
- Canonical: `src/chunk/mod.rs:69` → `FaultMsg::ForceUnwrapNull => "force-unwrap of null".to_string()`
- `src/interpreter/expr.rs:226` → `rt("force-unwrap of null")` — re-inlined
- `src/transpile/expr.rs:353` → `"({v} ?? throw new \\RuntimeException(\"force-unwrap of null\"))"` — re-inlined
- The VM correctly routes through `Op::Fault(FaultMsg::ForceUnwrapNull)`.

`src/transpile/call.rs:12-39` proves the correct pattern is available *and already used* for
`panic`/`todo`/`unreachable`/`assert` — `expr.rs:353` simply didn't use it.

**(b) `"non-exhaustive match at runtime"` — ALREADY DRIFTED in the PHP leg.** RISK: HIGH
[Verified: read all three]
- Canonical: `src/chunk/mod.rs:68` `FaultMsg::NonExhaustiveMatch`
- `src/interpreter/expr.rs:516` → `rt("non-exhaustive match at runtime")` — re-inlined literal
- `src/transpile/matches.rs:287` → `"{else_kw}{{ throw new \\UnhandledMatchError(); }}"` —
  **no message at all.** A different user-visible text from both Rust backends.

This is a **realized divergence**, not a latent one. It is masked only because `agree_err_php`
doesn't compare text (G26).

**(c) `"stack overflow"` — 11 literal sites, no canonical constant, and the code says so.** RISK: HIGH
[Verified: read all 11, including the admission]
`src/vm/closure.rs:35,70`; `src/vm/exec.rs:494,570,861,947,977`;
`src/interpreter/{construct.rs:175, call.rs:337, engine.rs:228}`; and
`src/jit/boxed.rs:53` `pub(super) const FAULT_STACK_OVERFLOW: &str = "stack overflow";`
with this comment immediately above it (`src/jit/boxed.rs:49-52`):
> `/// The VM's clean deep-recursion fault. The string is a bare literal in \`vm::exec\`/\`vm::closure\`/the`
> `/// interpreter (**not yet single-sourced in \`value.rs\` like the arithmetic faults**), so it is duplicated`
> `/// here — but the tests assert the JIT fault against the VM oracle's rendering, not this literal, so`
> `/// any VM-side drift is caught.`

The code is honest about the gap and explains why *its own* copy is safe. The 10 other copies are
not covered by that reasoning. Fix: `pub const FAULT_STACK_OVERFLOW` in `src/value/arith.rs` next to
`FAULT_INT_OVERFLOW` (`:10`), delete the `jit/boxed.rs` copy, point the 10 sites at it.

**(d) `"list index out of range"` + `"expected int index, found {}"` — the list-*read* bounds
decision is re-implemented in Rust in both backends.** RISK: HIGH
[Verified: read `value/collections.rs:93,126,131`, `vm/exec.rs:242,245-249,266,294`, `interpreter/expr.rs:202,206-208`]

`src/value/collections.rs` exports `map_index`, `list_set`, `map_set`, `set_nested` — but **no
`list_index` read kernel**. So the read path is the one collection op both backends re-express:
```rust
// src/vm/exec.rs:245-249
let i = usize::try_from(idx).ok().filter(|i| *i < xs.len())
    .ok_or_else(|| "list index out of range".to_string())?;
```
```rust
// src/interpreter/expr.rs:206-208
match usize::try_from(i).ok().filter(|i| *i < list.len()) {
    Some(i) => …, None => rt("list index out of range") }
```
This is textbook Invariant-4 *"re-implements the decision logic in Rust"* — the expression **and**
the literal are duplicated. Adding `value::list_index` and reusing it kills 9 literal sites across
(d) and (e).

**(e) `"cannot index-assign {}"` (3 sites)** — `value/collections.rs:154` (kernel) +
`vm/exec.rs:276,302` for the two ops that don't route through `set_nested`. RISK: HIGH [Verified]

### G28 — no canonical constant anywhere; duplicated across ≥2 backends AND unclassified by `classify` (drift is invisible) — P1

Ranked by site count. All [Verified: read every cited line].

| Literal | Sites | Backends | Note |
|---|---|---|---|
| ``"ambiguous overloaded call to `{name}`"`` | **6** | vm + interp | `vm/exec.rs:589,915`; `interpreter/call.rs:270,438,570,653` |
| ``"no overload of `{name}` matches the argument types"`` | **5** | vm + interp | `vm/exec.rs:592,919`; `interpreter/call.rs:273,574,657`. **`:919` uses the positional `{}` form** — same rendered text, different source shape, so a grep-based rename misses one |
| `"cannot call {} as a function"` | 4 | vm + interp | `vm/closure.rs:23`; `vm/exec.rs:988`; `interpreter/call.rs:50,80` |
| ``"enum `{}` variant `{}` has no backing value"`` | 2 | vm + interp | `vm/exec.rs:650`; `interpreter/expr.rs:183`. **The sharp one** — its *sibling* fault (`Enum.from` miss) **IS** single-sourced via `value::enum_from_miss` (`value/core_impl.rs:192-199`, called from `vm/exec.rs:675` + `interpreter/variants.rs:45`). The kernel pattern exists in the same feature; this one was left as twin literals. `classify` matches `"no case of enum"`, which this text does **not** contain → `Other` → untestable |
| `"cannot index {}"` | 2 | vm + interp | `vm/exec.rs:252`; `interpreter/expr.rs:217` |
| `"cannot interpolate {} into a string"` | 2 | vm + interp | `vm/exec.rs:209`; `interpreter/expr.rs:419`. Both sit immediately after a shared `v.as_display()` — the *kernel* is shared, only the fault body is twinned |
| ``"no field `{}` on `{}`"`` | 3 | vm + interp | `vm/exec.rs:761,775`; `interpreter/stmt.rs:309`. Partly covered by `classify`'s `"no field"` substring arm, so lower risk |
| `"cannot negate {}"` / `"cannot apply ! to {}"` / `"expected bool, found {}"` | 2 each | **vm + jit** | `vm/exec.rs:145,149,192` vs `jit/boxed.rs:176,191,254`. These three are Rust-level re-implementations of the *decision logic*, **not** the acceptable "emits machine code for the same semantics" category — their doc comments say "mirrors `exec.rs`", i.e. hand-kept copies |
| `"invalid map key: {}"` / `"invalid set element: {}"` | 7 + 4 | kernel + native | `value/collections.rs:44,78,102,145` vs `native/map.rs:39,69,97,111` and `native/set.rs:18,75,90`. The natives re-do `HKey::from_value(..).ok_or_else(..)` inline rather than calling `build_map`/`build_set`/`map_set` |
| `"recv from empty channel"` / `"join on an incomplete task"` | 2 each | vm + interp | `vm/exec.rs:405,426`; `interpreter/call.rs:485,506`. **Lower risk** — both ARE in `classify` (`FaultKind::Concurrency`), so a one-sided drift is caught; and the PHP leg is ladder-excluded by design (Invariant 14) |
| `"map key not found"` | 2 | **inside the kernel module itself** | `value/collections.rs:82` (in `map_index`) + `:150` (in `set_nested`) — `set_nested`'s nested-map arm hand-rolls the lookup instead of reusing `map_index`. No backend re-inlines it, so LOW-MED |

- **Option A:** promote each to a `pub const FAULT_*` in `src/value/` (or a `FaultMsg` variant where
  it's a plain fault) and point every site at it.
- **Option B:** fix only the ones `classify` currently misses — i.e. exactly the rows above with
  "untestable"/"unclassified" notes — since those are where drift is *invisible* rather than merely
  untested, and leave the classified ones (concurrency, `no field`) alone.
- **RECOMMENDED: B, after G26-A.** Do G26-A first (make `classify` reference the constants), because
  it changes which rows are risky: once `classify` derives from the constants, promoting a literal
  to a constant automatically gates it. Doing B before A means hand-maintaining two lists again.

### G29 — the PHP leg re-expresses the value kernels with nothing structural holding them together — P1
[Verified: read `src/transpile/runtime_php.rs:1-11,199,228,244,249,370,374,388,527,581,596,803,808`; `src/value/arith.rs:5-32`]

`src/transpile/runtime_php.rs:1-2` states the coupling:
> `//! PHP transpiler — the once-per-file \`__phorj_*\` runtime helper templates … **mirroring the Rust value kernels byte-for-byte**.`

`docs/INVARIANTS.md:38` states the invariant: kernels *"live **once**, in `src/value.rs`. Both
backends call them."* But there are **four** backends, and the PHP one cannot *call* a Rust kernel —
it re-expresses it, in 1,370 lines of PHP embedded in a Rust string literal.

Concretely: `'integer overflow'` is written **5 times** as a PHP literal
(`runtime_php.rs:527,581,596,803,808`) while `src/value/arith.rs:10` holds
`pub const FAULT_INT_OVERFLOW: &str = "integer overflow";`. Same shape for `'decimal overflow'`
(`:199`), `'decimal division by zero'` (`:244,374`), `'decimal modulo by zero'` (`:228`),
`'decimal division is not exact'` (`:249`), `'decimal scale out of range'` (`:370,388`) — each has a
`pub const` in `src/value/arith.rs:16-32` that the emitter could interpolate instead of retyping.

`src/jit/emit_unboxed/mod.rs:54` is the same shape from the JIT side: *"CONDITIONS mirror the
`value.rs` int kernels EXACTLY"* — a comment, not a constraint (and it cites a path that no longer
exists, per G17).

The only thing keeping Rust and PHP in agreement is `tests/differential.rs` running the
`examples/**/*.phg` corpus — i.e. **coverage-based, not structural** — and per G26 the PHP leg's
messages aren't compared at all.

- **Option A:** interpolate the existing `value::FAULT_*` consts into the PHP templates
  (`format!("… throw new \\OverflowException('{FAULT_INT_OVERFLOW}');")`). Small, mechanical,
  removes 12+ retyped literals and makes a const rename propagate to PHP automatically.
- **Option B:** generate the PHP helper bodies from one declarative kernel table (one row = Rust
  impl + PHP template + fault string) so a missing PHP side is a compile error.
- **Option C:** accept it as inherent and instead add a differential example per kernel fault path
  so "no example exercises it" stops being possible.
- **RECOMMENDED: A now, C alongside it, B as a QUEUED spec question.** A is achievable today and is
  strictly better than the status quo. C closes the coverage hole. B is the structurally right
  answer and is exactly the user-invisible-but-load-bearing design choice Invariant 15 reserves for
  the developer — and Invariant 16's "byte-identity-is-a-tool" clause says the trade must be
  surfaced and ruled, not self-decided.

### G30 — three fault texts have ALREADY drifted Rust↔PHP, ungated — P1
[Verified: read every pair]

**(a) `"Modulo by zero"` (capital M).** `src/transpile/runtime_php.rs:26`:
```
if ($b == 0) { throw new \DivisionByZeroError("Modulo by zero"); }
```
vs `src/value/arith.rs:7` `FAULT_MOD_ZERO: &str = "modulo by zero"`. `classify` matches the
lowercase form, so the PHP text would classify as `Other`.

**Fairness note:** capital-`M` `"Modulo by zero"` is **PHP's own native message** for `%` by zero,
so this looks like a deliberate choice to match what a PHP developer expects from PHP. That makes it
a *legitimate Invariant-14 ladder question* ("faithful idiomatic PHP" vs "identical failure
behaviour"), not simply a bug — but I could find no record of it being ruled
[Unverified: `grep "Modulo by zero" KNOWN_ISSUES.md docs/` found nothing].

**(b) `String.format` diagnostics — 8 message pairs, none identical.** Examples:
| Rust | PHP |
|---|---|
| `native/text_format.rs:98` ``"String.format: dangling `%` at the end of the format string"`` | `runtime_php.rs:733` `'String.format: dangling %'` |
| `text_format.rs:55` ``"String.format: positional index must be >= 1 (`%0$` is invalid)"`` | `runtime_php.rs:721` `'String.format: positional index must be >= 1'` |
| `text_format.rs:269` `"String.format: the format string needs at least {} value(s)"` | `runtime_php.rs:738` `'String.format: not enough values'` |
| `text_format.rs:106` | `runtime_php.rs:759`/`:777` |
| `text_format.rs:439`, `:445` | `runtime_php.rs:785`, `:786` |
The PHP re-implementation of the format *engine* is legitimately backend-specific; the *messages*
are not.

**(c) Index-OOB text collapses two faults into one.** `runtime_php.rs:558`:
`throw new \OutOfRangeException('index or key not found: ' . …)` — one PHP message covering **both**
`"list index out of range"` and `"map key not found"`, matching neither. This is deliberate (PHP
represents List and Map as one array type — see the DEC-255 comment at `:546-552`), but it means
`FaultKind::IndexOob` can never be established from the PHP leg, and it is **not recorded in
`KNOWN_ISSUES.md`** [Verified: grepped for `index or key not found` → no hit].

**(d) Native fault text duplicated verbatim native↔PHP with no shared constant:**
`"Conversion.truncate: float is out of int range"` (`native/convert.rs:42` vs
`runtime_php.rs:431,438` — **twice** on the PHP side); `"Conversion.round: …"`
(`native/convert.rs:53` vs `runtime_php.rs:450,457`); `Math.clamp` (`native/math.rs:168` vs
`runtime_php.rs:541` — byte-equal today, two independent literals).

- **Option A:** align (b) and (d) on shared constants; ladder-surface (a) and (c) as PENDING design
  questions per Invariant 14/15 and record whichever way they're ruled in `KNOWN_ISSUES.md` +
  the decision register.
- **Option B:** align everything including (a)/(c), accepting a less PHP-idiomatic message.
- **RECOMMENDED: A.** (b) and (d) are unambiguous bugs — nobody chose to word the same error two
  ways. (a) and (c) are genuine ladder trades where "faithful idiomatic PHP" competes with
  "identical failure behaviour", and Invariant 15 says those are the developer's to rule. The one
  thing that is *not* optional either way: (c) is an undisclosed byte-identity carve-out, and
  Invariant 14 requires every exclusion to be "a tracked, tested, register-recorded artifact."

### G31 — the primitive type-test table is written FIVE times across four modules, held together only by comments — P1 (worst *structural* duplication)
[Verified: read all five sites in full]

| Site | Form |
|---|---|
| `src/interpreter/expr.rs:94-99` | `match type_name { "int" => matches!(v, Value::Int(_)), … }` (`Expr::InstanceOf`) |
| `src/interpreter/kernels.rs:211-216` | identical `match` (`match_pattern` / `Pattern::Type`) |
| `src/vm/exec.rs:841-846` | identical `match` (`Op::IsInstance`) |
| `src/transpile/expr.rs:210-214` | `"int" => format!("is_int({v})"), …` |
| `src/transpile/matches.rs:336-340` | same, plus an `OpKind` column |

All five enumerate exactly `"int" | "float" | "string" | "bool" | "null"` and nothing else, and
**every one carries a comment asserting agreement with the others** — e.g.
`src/interpreter/expr.rs:92-93` *"so all three backends agree byte-for-byte"*,
`src/interpreter/kernels.rs:209-210` *"the oracle for the VM's `Op::IsInstance` primitive arm and
PHP's `is_int()`/`is_float()`/`is_string()`/`is_bool()`/`is_null()`"*. Five hand-kept copies held
together by five comments.

Adding `decimal` or `bytes` as a type-pattern requires editing all five. Miss one and **the VM
accepts what the interpreter rejects** (or PHP silently differs) — with **no compile error**. This is
structurally identical to the `Op`-variant three-match rule, *without* exhaustiveness forcing it.
There is no `value::is_primitive_type(name, &Value) -> Option<bool>` kernel.

**Same shape, lower rank:** the class/interface `instanceof` test —
`matches!(v, Value::Instance(inst) if inst.class == name || class_implements.get(&*inst.class).is_some_and(|i| i.contains(name)))`
— appears at `interpreter/expr.rs:100-105`, `interpreter/kernels.rs:217-221` (type pattern),
`interpreter/kernels.rs:238-242` (struct pattern), and `vm/exec.rs:847-853`. Three copies inside the
interpreter alone. And the shapes have already diverged: the VM uses `ifaces.contains(name)` while
the interpreter uses `ifaces.iter().any(|i| i == type_name)` — **equivalent today** [Verified: read
both], but two different expressions of one rule.

- **Option A:** one `value::primitive_type_test(name: &str, v: &Value) -> Option<bool>` kernel;
  interpreter + VM call it, and the two transpile sites derive their `is_*` name from the same
  table (a `&[(&str, &str)]` name→PHP-fn map) so all four read from one source.
- **Option B:** make the primitive set a real enum (`PrimTy::{Int,Float,Str,Bool,Null}`) resolved
  once in the checker and carried in the AST/`Op`, so the backends match on an enum — which makes
  the match **exhaustive** and a new primitive a compile error in all four places. This gets the
  Invariant-3 guarantee for the primitive set.
- **Option C:** leave duplicated; add a single test asserting all five agree for every
  (name, Value-variant) pair.
- **RECOMMENDED: B, with C as the immediate stopgap.** B is the only option that makes the fan-out
  *mechanically* enforced rather than test-enforced, and it is the same technique the project
  already trusts for `Op`. It touches the AST/`Op` set, so it is Invariant-3/7 territory and wants
  a proper slice. C is ~40 lines of test and can ship today, closing the hole while B is specced.
  A is the middle path but leaves the transpile sites deriving from a second table.

### Dimension 5 — what is CLEAN (checked, no finding — recorded so it isn't re-ploughed)
[Verified: grepped whole tree + read the cited dispatch sites]

- **Core arithmetic is exemplary.** **Zero** `checked_add`/`checked_sub`/`checked_mul`/`checked_div`/
  `checked_rem`/`overflowing_*` anywhere outside `src/value/`, in any backend.
  `src/vm/exec.rs:28-63` and `src/interpreter/kernels.rs:22-56` both dispatch into
  `value::int_*`/`float_*`/`decimal_*`, including the `#[UncheckedOverflow]` wrapping variants.
- **`compare_ord` is single-sourced.** `interpreter/kernels.rs:125`, `vm/mod.rs:611`,
  `jit/boxed.rs:223` all call `value::compare_ord`; only the op→bool projection is backend-local,
  which is correct (the op enums genuinely differ). No `partial_cmp` on `Value` outside
  `src/phstr.rs:317` (a `PhStr` impl, legitimate).
- **Equality is single-sourced** on `Value::eq_val` — 20+ call sites, zero re-implementations.
- **`FaultMsg` is used correctly for the fault intrinsics** by all three legs (`chunk/mod.rs:66`
  canonical; `interpreter/call.rs:183-194`; `transpile/call.rs:12-39`; `vm/exec.rs`). This is
  exactly the pattern G27(a)/(b) should follow — the mechanism is built and working.
- **The JIT's native re-expressions are the acceptable kind.** `jit/emit_unboxed/scalar.rs:363-378`
  (`Math.abs`) guards `n == i64::MIN` → `ec.fault_if(b, is_min, 5)` and lets the VM redo render the
  canonical text; `jit/emit_unboxed/verticals_hof.rs:110-117` (`List.sumBy`) uses `sadd_overflow` +
  code-5 redo. Neither emits a fault string. That is "emits machine code for the same semantics",
  correctly plumbed. **The violations are confined to `jit/boxed.rs`** (G27c, G28) — which is Rust,
  not codegen.
- **The native registry is the strongest single-sourcing in the codebase.** `NativeFn`
  (`src/native/mod.rs:54-80`) carries `module`, `name`, `params`, `ret`, `eval`, `php`, `lift_from`,
  `pure` in **one row** — checker signature, runtime body, PHP emission and lifter inverse
  co-registered. There is **no** second arity/signature table in `src/checker/`. The
  `("Core.Math", "abs")`-style pairs in `transpile/call.rs:273-381`, `checker/calls/core.rs:102,476`,
  `checker/calls/args.rs:159`, `jit/analyze/natives.rs:59,430,471` are *identity lookups keyed off
  the registry row* (each guarded by `nf.module == … && nf.name == …`), not duplicated data.
  **This corrects the "PARTIAL" note in G31's table** — the native fan-out is better than I first
  scored it; only the `uses_*` transpile gate sits outside the row.
- **`Value::as_display` is the single float/bool renderer** (Invariant 4 / EV-6);
  `transpile/runtime_php.rs:42-52` (`__phorj_str`) is a documented PHP mirror with an explicit
  comment on why `(string)$float` won't do.
- **`src/vm/coop.rs` vs `src/interpreter/coop.rs` are NOT duplicates** — both drive the shared
  `green::exec::run_loop` over the shared `green::sched::Scheduler`; only per-backend task
  construction differs. The one shared literal (`"done {got}"` at `vm/coop.rs:150` /
  `interpreter/coop.rs:240`) is doc-test `.phg` source, harmless.

### G32 — the "rewrite a `Type` recursively" walk is written four times — P2
[Verified: read all four bodies — `checker/rewrite_alias.rs:23-40`, `checker/collapse_injected.rs:38-52`, `checker/rewrite_generics.rs:44-58`, `checker/desugar_db.rs:1556`]

Four functions (`rt`, `rt`, `rty`, `retype` — the four spellings from G8) implement the structurally
identical walk: `match ty { Type::Named { name, args, span } => … args.iter().map(recurse) … }`. They
differ only in the substitution decision at the `Named` leaf.

- **Option A:** extract one `ast::rewrite_type(ty, &mut impl FnMut(&Type) -> Option<Type>)` into
  `src/ast/walk.rs` (which already exists as the home for AST walk primitives) and have all four
  pass a closure.
- **Option B:** leave duplicated — each pass's leaf logic is small and the walks may legitimately
  diverge.
- **RECOMMENDED: A.** Same argument as Invariant 4's: four copies of a recursive type walk mean a new
  `Type` variant must be added in four places with nothing enforcing it — the G34 hazard in
  miniature. One extraction closes it permanently, and it also resolves G8's four-spellings problem
  as a side effect.

---

## Dimension 6 — Extensibility friction

### Invariant 3 verification: PASSES, and cleanly [Verified: read all three matches]

| Site claimed | Actual location | Wildcard-free? |
|---|---|---|
| `vm::exec_op` (CLAUDE.md says `src/vm/exec.rs`) | `src/vm/exec.rs:9`, `match *op {` at `:14` | **YES** — the three `_ =>` in the file (`:847`, `:893`, `:986`) are all in *nested* inner matches, not the top-level `Op` match [Verified: read each with surrounding context and indentation] |
| `BytecodeProgram::validate` (CLAUDE.md says `src/chunk.rs`) | **`src/chunk/validate.rs:21`** | **YES** — zero `_ =>` in the file |
| `compiler::stack_effect` (CLAUDE.md says `src/compiler/mod.rs`) | **`src/compiler/emit.rs:75`** | **YES** — zero `_ =>` in the file |

**No invariant breach.** All three are genuinely exhaustive. Two of the three *paths* named in
CLAUDE.md Invariant 3 are stale — see G18.

### G33 — Invariant 3 says "three places", but there is a fourth `Op` consumer — P2
[Verified: `src/jit/mod.rs:63`, `src/jit/analyze/mod.rs:155,235,389,439,704,728,763`, `src/jit/collect_unboxed.rs:78`]

`src/jit/mod.rs:63` says it plainly:
> `//! The JIT is a 4th backend intimately coupled to \`Op\`/\`Value\`/chunk (invariants #3/#4/#6)`

`src/jit/` matches on `Op` in at least 8 places, all deliberately wildcarded for
**soundness-by-conservatism** — e.g. `src/jit/analyze/mod.rs:439`:
```rust
_ => break, // unmodeled op: stop (later slots stay unproven — sound)
```
plus `JitError::Unsupported` bail paths throughout (`:1103`, `:1113`, `:1158`, …), documented at
`:502` as *"`Unsupported` (VM fallback), never a miscompile"*.

**This design is correct and should be credited** — a new `Op` cannot miscompile in the JIT, it
simply won't be optimised. But Invariant 3 as written tells a newcomer "three exhaustive matches"
and nothing more; they never learn that their new op silently falls off the JIT fast path. In a
project whose Invariant 18 is WIN-OR-FLAG, a silent de-optimisation is exactly the class of thing
that should be visible.

- **Option A:** extend Invariant 3 to "three exhaustive matches + a 4th, deliberately-wildcarded
  JIT consumer that bails to the VM (sound by construction — but a new op is un-JITted until you
  add it)".
- **Option B:** also make the de-optimisation *loud* — a debug-only counter or a
  `phg disassemble --jit-coverage` that lists ops the JIT declines.
- **RECOMMENDED: A now, B as a QUEUED perf-tooling item.** A is a two-line doc fix that removes a
  real surprise. B is genuinely useful for Invariant 18 work but is a feature, not a fix.

### G34 — the *real* extensibility hazard: 17 named catch-alls over `Expr`/`Item`, so a new AST variant is SILENTLY skipped by 12 passes — P1 (the highest-value structural finding in this audit)
[Verified: `grep -rn "^\s*\(leaf\|other\|rest\|e\|x\) => \(leaf\|other\|rest\|e\|x\),$"` + read every site]

`Expr` has **37 variants** (`src/ast/exprs.rs:44`); `Stmt` has 15 (`src/ast/stmts.rs`). Adding an
`Op` variant is mechanically enforced by three wildcard-free matches. Adding an **`Expr`** variant
is enforced by **nothing**.

There are **13 hand-rolled total AST rewriters** in `src/checker/` alone, and 17 of them use a
*named* catch-all arm — `leaf => leaf` or `other => other` — which compiles cleanly and silently
returns a new variant unmodified:

| file:line | arm |
|---|---|
| `src/checker/desugar_db.rs:2644` | `other => other,` |
| `src/checker/desugar_db.rs:2967` | `leaf => leaf,` |
| `src/checker/desugar_di/walker.rs:202` | `other => other,` |
| `src/checker/desugar_di/walker.rs:607` | `leaf => leaf,` |
| `src/checker/desugar_di/mod.rs:358` | `other => other,` |
| `src/checker/desugar_router.rs:78` | `other => other,` |
| `src/checker/desugar_router.rs:406` | `leaf => leaf,` |
| `src/checker/rewrite_generics.rs:644` | `other => other,` |
| `src/checker/rewrite_invoke_tostring.rs:477` | `other => other,` |
| `src/checker/rewrite_ufcs.rs:291` | `leaf => leaf,` |
| `src/checker/rewrite_ufcs.rs:460` | `other => other,` |
| `src/checker/rewrite_html.rs:211` | `leaf => leaf,` |
| `src/checker/rewrite_html.rs:376` | `other => other,` |
| `src/checker/resolve_variant_imports.rs:153` | `other => other,` |
| `src/checker/resolve_variant_imports.rs:210` | `leaf => leaf,` |
| `src/checker/resolve_variant_imports.rs:430` | `leaf => leaf,` |
| `src/checker/overloads.rs:585` | `other => other,` |

Plus bare `_ => {}` on `Expr`/`Item` matches at `src/checker/rewrite_pipe/walk.rs:230` and `:32`,
`src/checker/desugar_router.rs:119`, `src/checker/enforce_injected.rs:41`,
`src/checker/resolve_variant_imports.rs:101`, and `src/interpreter/stmt.rs:250`.

**The sharpest instance, and the proof this is not theoretical.** `src/ast/walk.rs:748` closes
`collect_pattern_bindings` — the function that feeds `free_vars`, i.e. closure capture — with
`_ => {}`. Three lines above it, at `:741-743`, is this comment:
> `// A struct pattern (\`Point { x, y }\`, S5.2) binds via each field's sub-pattern (recurse —`
> `// a nested struct or rename binds too). **Missing this would drop struct-bound names from**`
> `// \`free_vars\`, **miscompiling a lambda that captures one** (the guard-recursion lesson).`

And `src/ast/walk.rs:15-16` states the contract:
> `// **Note:** over-reporting is acceptable … **Under-reporting (missing a real capture) is a correctness bug.**`

So: this exact bug already happened once (`Pattern::Struct` was missing), the fix was to add an arm
while **leaving the `_ => {}` in place**, and the next `Pattern`/`Expr` variant will reproduce it
identically. The parenthetical "(the guard-recursion lesson)" says it has happened at least twice.

`src/checker/desugar_db.rs:67-69` even states the invariant this violates:
> `//! INVARIANT — keep the rewriter TOTAL (matching \`desugar_di\`): \`ritem\`/\`rfn\`/\`rmember\`/\`rexpr\`/\`rstmt\``
> `//! recurse EVERY expression-bearing position … **A new expression-bearing AST node → add its arm here.**`

The file asserts totality and then closes `rexpr` at `:2967` with `leaf => leaf`. Nothing enforces
the invariant it declares.

**Options:**
- **Option A — remove every catch-all** from the AST rewriters/walkers, enumerating all 37 `Expr`
  variants explicitly (grouping true leaves into one `Expr::Int(..) | Expr::Float(..) | … => …`
  arm, which is still exhaustive and still breaks the build on a new variant). Cost: ~19 files,
  large mechanical diff, and the leaf-group arms need re-listing each time a leaf is added.
- **Option B — one shared total visitor.** Put a single exhaustive `visit_expr_mut` /
  `visit_pattern` in `src/ast/walk.rs` (which already exists for this purpose) and have all 13
  passes drive it with a closure. One exhaustive match crate-wide instead of 19. Cost: real
  refactor; some passes need pre/post-order control (`rewrite_pipe/walk.rs` already threads a
  `pre: bool`, so the mechanism is proven).
- **Option C — a dummy-variant CI smoke check.** `docs/ARCHITECTURE.md:62-63` claims the three `Op`
  matches are *"verified by a dummy-variant smoke check"*; I could not find such a check
  [Unverified: `grep -rn "dummy.variant\|dummy_variant"` over `src/`, `tests/`, `scripts/` found
  nothing — either it lives somewhere I didn't look, or the claim is stale]. Whatever mechanism the
  `Op` set has (or should have), point it at `Expr`/`Stmt`/`Pattern` too: add a variant under
  `#[cfg(feature = "exhaustiveness-check")]`, confirm the build fails, and gate it in CI.
- **Option D — scope it to the one proven-dangerous site.** Remove only `src/ast/walk.rs:748`'s
  wildcard, since `free_vars` under-reporting is a documented *correctness* bug and everything else
  is (probably) a mis-lowering that a test would catch.

- **RECOMMENDED: D immediately, then C, then B as a QUEUED spec.** D is a single file, closes a
  documented correctness bug class that has already fired twice, and can ship today. C is the
  cheapest thing that makes the whole class *visible* (and it also resolves whether
  ARCHITECTURE.md's dummy-variant claim is real). B is the right long-term answer but it is a
  multi-day refactor of the pipeline's most delicate code and it changes user-visible lowering
  order risk — squarely Invariant 15 territory. **A is the option I'd argue against**: 19 files of
  enumerated leaf-groups is the *appearance* of exhaustiveness that regrows into catch-alls the
  first time someone adds a leaf.

### G35 — other "remember to touch N places" hazards, ranked by whether they are mechanically caught — P2

| Hazard | Places | Mechanically enforced? |
|---|---|---|
| New `Op` variant | 3 exhaustive matches (+1 sound JIT bail) | **YES** — verified wildcard-free (and see G33) |
| New diagnostic code | code site + `phg explain` entry | **YES** — `src/cli/tests/explain_coverage.rs` + `src/cli/tests/explain_ratchet.rs`, and the counts line up: **305 distinct `E-`/`W-` codes in `src/` vs 306 registered in `cli/explain*`** [Verified: `grep -rhoE '"E-[A-Z0-9-]+"' src/ \| sort -u \| wc -l`]. **This is the best-engineered registry in the project and the model the others should copy.** |
| New `Expr`/`Stmt`/`Pattern` variant | ~19 walkers + 4 backends | **NO** — see G34 |
| New `Type` variant | 4 duplicated type walks | **NO** — see G32 |
| New native | `native/<mod>.rs` body + `native/<mod>_registry.rs` row + `transpile/call.rs` `uses_*` gate + `transpile/runtime_php.rs` template | **PARTIAL.** `src/native/mod.rs:1-8` documents the single-sourcing intent well ("One entry single-sources all four facets … so the four backends cannot drift"), but the transpile side is separate: `Text.trim` appears at `src/native/text_registry.rs:97` **and** `src/transpile/call.rs:233` (`"trim" => self.gates.uses_text_trim = true`). A native added without its `uses_*` gate transpiles to a call with no helper emitted. [Unverified whether a test catches this — I did not locate one] |
| New `Core.*` virtual module | `CORE_MODULES` row + prelude const + `core_module_bound_names` + `unavailable_gated_modules` | **PARTIAL** — one table (`src/cli/preludes.rs:605`), which is good, but see G3 |

- **RECOMMENDED:** apply the `explain_coverage`/`explain_ratchet` pattern to the native↔`uses_*`
  pairing — a test that walks the native registry and asserts every native with a PHP helper has a
  gate. That is the same shape as the diagnostic-code ratchet, in a codebase that has already
  proven the pattern works.

---

## Top 10 ranked by (impact ÷ effort)

Dimension 5's findings landed after the first draft and displace several earlier entries — the table
below is the final ranking across all 35 findings.

| # | Finding | Sev | Impact | Effort | Why here |
|---|---|---|---|---|---|
| 1 | **G14** — `docs/INVARIANTS.md` §1 is corrupted by a `runvm`→"the VM leg" find-replace **and** semantically inverted: it says *"never `cmd_run`"* while `src/main.rs:32` calls `cmd_run_exit`; both enforcement pointers are unresolvable | **P0** | high | ~10 min | The one finding that will actively mislead a future session, in the file CLAUDE.md orders read before backend work |
| 2 | **G27(a)+(b)** — a canonical `FaultMsg` exists and two backends re-inline it; `"non-exhaustive match at runtime"` has **already drifted** (PHP throws `UnhandledMatchError()` with no message) | **P1** | high | ~30 min | A realized byte-identity divergence with the fix mechanism already built and used elsewhere in the same file (`transpile/call.rs:12-39`) |
| 3 | **G34-D** — remove the `_ => {}` at `src/ast/walk.rs:748` | **P1** | high | ~20 min | Closes a documented correctness-bug class ("under-reporting a capture is a correctness bug") that the comment three lines above says has already fired twice |
| 4 | **G18** — fix the 3 stale paths in the comment that *is* Invariant 3's enforcement (`chunk/op.rs:1-2`, `INVARIANTS.md:62-64`, CLAUDE.md Inv 3) | **P1** | high | ~10 min | The only signpost for the project's most-enforced invariant points at two nonexistent modules |
| 5 | **G26** — make `tests/differential.rs::classify` reference `value::FAULT_*`/`FaultMsg` instead of re-typing all 12 fault bodies | **P1** | high | ~30 min | Prerequisite for G28/G29/G30 being *verifiable*: today an unclassified duplicated fault can drift **invisibly**, and a canonical-constant rename does not reach the oracle |
| 6 | **G16-A** — CI check that every `docs/…md` + `src/…rs` path in a comment resolves; then sweep | **P1** | high | ~1 h | 18 dead spec pointers (incl. `green/mod.rs:4`'s "developer-locked" spec) + ~30 dead source paths; one gate closes G4/G16/G17/G18 permanently and prevents recurrence |
| 7 | **G31-C then B** — the primitive type-test table is written **5×** across 4 modules, held together by 5 cross-referencing comments; adding `decimal`/`bytes` as a type-pattern silently diverges the VM from the interpreter with no compile error | **P1** | high | C ~40 lines / B a slice | Worst *structural* duplication found; C ships today, B (a `PrimTy` enum) buys the Invariant-3 exhaustiveness guarantee for the primitive set |
| 8 | **G19** — `src/green/mod.rs:13` says the executor hasn't shipped, two lines above `pub mod exec;`; `serve/mod.rs:14-16` says green threads were superseded and need `unsafe` (both false) | **P1** | med-high | ~15 min | Three docs describe a shipped subsystem as absent/impossible |
| 9 | **G3** — move the ~500 lines of embedded prelude phorj to `.phg` + `include_str!` | **P1** | high | ~2 h | The stdlib's public surface is the only phorj in the repo outside the formatter sweep and `phg check`; also takes `preludes.rs` 1245→<300 |
| 10 | **G5** — 31% of the codebase (incl. `src/jit/`, 21k lines, the 4th backend) is absent from the "one-page map of the codebase" | **P1** | high | ~1-2 h | A newcomer's first document omits the second-largest module and contradicts `jit/mod.rs:63` on the backend count |

*Just below the line, and why:* **G6-B** (flatten the 17-deep pass expression in `cli/pipeline.rs`)
would be #11 and is a strong candidate to promote — zero semantic risk, and pass *order* is an
Invariant-5 constraint currently invisible in the control flow. **G27(c)+(d)** (`"stack overflow"`
×11, `value::list_index` missing) is mechanical and kills ~20 literal sites, but wants G26 first so
the fix is verifiable. **G29/G30** (PHP-leg kernel literals, 3 already-drifted texts) are
high-impact but partly entangled with Invariant-14 ladder rulings that are the developer's, not
mine. **G21-A** (`//!` on `src/lib.rs` — currently zero, so rustdoc's landing page is empty) is
cheap and would be #11 on effort alone. **G22** (35 × `too_many_arguments`) is deliberately low:
its best fix is arguably one honest crate-level allow, which is a judgement call. **G7/G8/G10/G11**
(the rename family) have high total impact but should land as one coordinated commit after G6-B,
not piecemeal. **G34-B** (one shared total AST visitor) is the correct long-term answer and is
deliberately ranked *out* — it is a multi-day refactor of the most delicate code in the project and
is squarely Invariant-15 developer territory.

## Positive attestations (genuinely well-built — do not re-litigate)

**A1 — The `unsafe` island claim holds exactly as documented.** [Verified: `grep -rn unsafe src/ | grep -v '^src/jit/'` → **25 hits, every one inside a comment or a doc string; zero `unsafe` code**] `src/lib.rs:10` and `src/main.rs:5` carry `#![deny(unsafe_code)]`; the only `#![allow(unsafe_code)]` in the tree is `src/jit/mod.rs:80`, and `src/jit/mod.rs:74-79` explains the `deny`-not-`forbid` choice and names the CI `unsafe-island` gate that machine-enforces it. `src/lib.rs:6-7` even explains that the surrounding prose avoids the literal token so the CI grep doesn't false-positive. This is exemplary — the policy, its rationale, its scope, and its enforcement are all written down in the right places.

**A2 — Invariant 3 is genuinely, verifiably upheld.** All three coupled `Op` matches are wildcard-free ([Verified: zero top-level `_ =>` in `src/vm/exec.rs`'s `match *op`, zero in `src/chunk/validate.rs`, zero in `src/compiler/emit.rs`]). `src/compiler/emit.rs:73-74` even states *why*: *"kept exhaustive so a new op can't silently skew the height."* And the `src/jit/` wildcards are deliberately sound-by-conservatism (`analyze/mod.rs:439` `_ => break, // unmodeled op: stop (later slots stay unproven — sound)`), which is the right engineering choice for a bail-to-VM optimiser.

**A3 — `src/vm/exec.rs` at 1053 lines is correct, not debt.** One method, one exhaustive match, explicitly sanctioned by Invariant 13's "genuinely-cohesive exhaustive-match units" clause and by `docs/ARCHITECTURE.md:61-63`. Splitting it would be a regression. Same for `src/chunk/` staying a small, focused module.

**A4 — Zero `todo!`, zero `unimplemented!`, zero production `panic!`.** [Verified: all 18 `panic!` are inside in-file `#[cfg(test)]` blocks] For a 155k-line language implementation with four backends, a JIT, an LSP, a package manager, and a debugger, that is a remarkable number.

**A5 — The diagnostic-code registry is the best-engineered fan-out in the project.** 305 distinct codes in `src/` vs 306 registered in `cli/explain*`, with `src/cli/tests/explain_coverage.rs` and `explain_ratchet.rs` enforcing it. This is the pattern G34/G35 should copy; it is proof the project can build mechanical enforcement when it decides to.

**A6 — Module-doc discipline is 96%, and the good docs are genuinely excellent.** [Verified: 407/424 non-test files carry `//!`] `src/limits.rs:1-11` (states the *reason* for centralisation, the invariant, the measured numbers per constant, and why the limits are reachable at all), `src/serve/mod.rs:1-16` (defines the module as a determinism *seam*, not a feature), `src/phstr.rs:1-20` (states a hard invariant plus its proof obligation), and `src/jit/mod.rs:68-79` (the `## The unsafe island` section) are better than most production Rust. `src/native/mod.rs:1-12`'s four-facet single-sourcing explanation is the clearest statement of intent in the crate.

**A7 — Rust API guidelines: an exhaustive sweep over all 566 files found essentially zero violations.** Zero `into_*` that borrows, zero `as_*` that allocates, zero fallible `new`, zero non-bool `is_*`/`has_*`, both `_mut` genuine, the single `to_*(self)` correct because the type is `Copy`. One arguable item (`ext/registry.rs:278`). That is a cleaner bill than most mature crates.

**A8 — The M-Decomp work is real and the size gate is honestly green.** [Verified: re-implemented `size-gate.sh`'s ratchet logic independently — **zero** new hard-cap breaches, **zero** grandfathered file grown past its ceiling] `checker/calls/methods.rs` 973→252, `collect/interfaces.rs` 760→169, `transpile/mod.rs` 758→198, `loader/mod.rs` 655→198. And `SLICE-STATE.md:5-6` records that this was done by real decomposition with the baseline **untouched** — the developer refused the cheap fix, and the evidence confirms it.

**A9 — The size gate itself is well-designed.** A ratchet (freeze existing debt, forbid growth, forbid new breach, WARN on soft cap) is the right mechanism for exactly this problem, and its header comment states the semantics precisely enough that I could re-derive its behaviour and independently confirm the green result.

**A10 — Shell hygiene is uniform.** All 6 scripts (`microbench.sh`, `microbench-gate.sh`, `perf-gate.sh`, `size-gate.sh`, `git-hooks/pre-commit`, `git-hooks/pre-push`) carry `set -eEuo pipefail` or `set -euo pipefail`. [Verified: `grep -m1 -o "set -[a-zEuo]*"` per file] And the gate design is thoughtful: `microbench-gate.sh:61-65` documents a *measured* load-sensitivity finding (ratios 1.1-1.7 at load <2 vs 0.2-0.5 at load ~7, no code change) and explicitly routes the load-immune regression check to `perf-gate.sh` instead. That is Rule-14 root-cause discipline applied to a flaky gate rather than a retry loop bolted on.

**A11 — Where justification comments exist, they are excellent.** `src/tokenizer/ident.rs:23-25` (why `from_utf8` cannot fail), `src/vm/exec.rs:892` (why the `unreachable!` is unreachable — *"`layout_ptr` above already proved this is an `Instance`"*), `src/value/core_impl.rs:82` (`float_cmp` "intentional: language-level float equality"), `src/jit/handles/mod.rs:1001` (`too_many_arguments` "fixed extern \"C\" shape"), `src/checker/calls/variants.rs:339` (`let _ =` "surface nested errors"). The problem in Dimension 4 is *density*, never quality — the house standard is high, it just isn't applied everywhere.

**A12 — Core arithmetic single-sourcing is exemplary and machine-checkable.** [Verified: grepped the whole tree] **Zero** `checked_add`/`checked_sub`/`checked_mul`/`checked_div`/`checked_rem`/`overflowing_*` anywhere outside `src/value/`, in any of the four backends. `src/vm/exec.rs:28-63` and `src/interpreter/kernels.rs:22-56` both dispatch into `value::int_*`/`float_*`/`decimal_*`, including the `#[UncheckedOverflow]` wrapping variants. Likewise `compare_ord` (three call sites, zero re-implementations, no stray `partial_cmp` on `Value`) and `Value::eq_val` (20+ call sites, zero re-implementations). Invariant 4's core promise is kept, precisely. The Dimension-5 findings are all at the *periphery* — fault strings and collection bounds — never the arithmetic itself.

**A13 — The native registry is the strongest single-sourcing in the codebase.** [Verified: read `src/native/mod.rs:54-80` + every `("Core.X", "y")` pair site] `NativeFn` carries `module`, `name`, `params`, `ret`, `eval`, `php`, `lift_from`, `pure` in **one row** — the checker's signature, the shared runtime body, the PHP emission, and the lifter inverse all co-registered, exactly as `src/native/mod.rs:1-8` promises ("One entry single-sources all four facets … so the four backends cannot drift"). There is **no** second arity/signature table in `src/checker/`; the `("Core.Math", "abs")`-style pairs scattered across `transpile/call.rs`, `checker/calls/`, and `jit/analyze/natives.rs` are all *identity lookups keyed off the registry row*, each guarded by `nf.module == … && nf.name == …`. Only the `uses_*` transpile gate sits outside the row (G35).

**A14 — The JIT's fault handling is correctly architected.** [Verified: read `jit/emit_unboxed/scalar.rs:363-378`, `verticals_hof.rs:110-117`, `jit/analyze/mod.rs:502`] The unboxed codegen never emits a fault *string* — it speculates, sets a sticky flag, and hands off to the VM redo path to render the canonical text (`ec.fault_if(b, is_min, 5)` / `sadd_overflow` + code-5). That is the "emits machine code for the same semantics" category, correctly plumbed, and it means a JIT/VM message divergence is structurally impossible on those paths. The Dimension-5 JIT violations are confined to `src/jit/boxed.rs`, which is ordinary Rust — and even there, `boxed.rs:49-52` **documents its own duplication and explains why its copy is safe**. Honest self-documentation of a known gap is the behaviour you want from a codebase, not something to penalise.

**A15 — `src/vm/coop.rs` and `src/interpreter/coop.rs` are not the duplication they look like.** [Verified: read both] Both drive the *shared* `green::exec::run_loop` over the *shared* `green::sched::Scheduler`; only per-backend task construction differs. `src/green/sched.rs:11` states the design intent — *"the `interp ≡ VM` spine … pure data + logic, unit-tested in isolation"* — and the code matches it. Byte-identical task interleaving across backends is achieved structurally, not by parallel maintenance.

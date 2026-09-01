# Agent I — Incompleteness + Missing-Enforcement Sweep (fresh pass, 2026-07-25)

> Fulfils the **fresh-sweep half** of `docs/archive/specs/2026-07-24-visibility-model.md` **DV-5**
> ("global completeness sweep is its OWN research pass … Reuse the rich existing audits + a fresh
> `/gaps` sweep, synthesized into ONE ranked completeness register").
>
> Repo state: `master` @ `25053be`, working tree clean (`git status --porcelain` empty).
> Binary: `target/release/phg` 1.0.0-nightly.0, built 2026-07-25 21:03 — **current with HEAD**
> (unlike the 2026-07-01 H-audit, whose binary was one commit behind).
> Probe corpus: 46 `.phg` programs + 3 multi-file projects under
> `/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-gaps/`.

---

## Method + honest coverage statement

**Prior art read first** (as instructed, to avoid re-reporting): `docs/research/full-audit/raw/H-enforcement.md`
(262 lines, the direct predecessor — read in full), `docs/research/2026-07-16-full-reopen-audit.md`
(section map + D2/D3/D4 findings), `docs/INVARIANTS.md` (full), `CLAUDE.md` (all 19 invariants),
`FEATURES.md` (full), plus targeted reads of `KNOWN_ISSUES.md`, `docs/plans/MASTER-PLAN.md`,
`docs/plans/SLICE-STATE.md`, and the 11 `docs/specs/2026-07-2*.md` status headers.

**What I did:**
- **Behavioural probing** (the primary method): 46 hand-written probes exercised against the current
  release binary across `check`, `check --json`, `run`, `run --tree-walker`, `transpile`,
  `disassemble`, `format`, `explain` — including 3 synthetic multi-file projects to reach loader-mode
  rules. Every finding below that says [Verified] carries a transcript captured in this session.
- **Re-verification of every H-audit recommendation** (its §7 table, 12 rows) against the current
  binary — this is the "still-open vs now-fixed" table below, and it is where most of the value sits:
  **6 of 12 are now fixed**, which the register does not yet reflect for all of them.
- **Static sweeps** for enforcement gates: Invariant 3 (three exhaustive matches), Invariant 4 (value
  kernels), Invariant 6 (reified operands on all vm-compile paths), Invariant 10 (determinism),
  Invariant 12 (naming), `TODO`/`FIXME`/`todo!()`/`unimplemented!()`, `panic!`/`unwrap` outside tests.
- **Three delegated read-only sub-sweeps**, whose raw findings I re-verified before adopting:
  (a) Invariant-10 HashMap-determinism sweep — 366 HashMap/HashSet occurrences across 78 non-test
  files, all 28 `.keys()`/`.values()` sites and ~50 `.join(` render sites traced;
  (b) Invariant-9 examples coverage — bidirectional, 48 `Core.*` modules vs 266 `.phg` files vs 257
  README rows;
  (c) diagnostic-code registry — `phg explain` executed on **all 313 registered + all 307 raised**
  codes (620 invocations).
  I independently re-ran the two most load-bearing claims from (a) and (c) — the `disassemble`
  non-determinism and the `--json` parse-error bug — before including them.

**Coverage I did NOT achieve (stated plainly):**
- **No `cargo test` / `cargo build`** (caller-imposed: ~6 GB free). So: I cannot report whether the
  existing test suite is green, cannot confirm `every_emitted_diagnostic_code_has_an_explanation`
  passes, and every claim about *test* behaviour below is from reading test source, graded [Inferred].
- **No PHP-leg verification.** No `php` on PATH in this session, so I could not close the third leg
  of the byte-identity spine for any finding. Where a PHP-leg divergence is plausible I say so and
  grade it [Unverified] — notably I8 (property-hook divergence) is verified `run` vs `--tree-walker`
  only, not against real PHP.
- **Native-function-level coverage** of the stdlib is an upper bound on "used" / exact on "unused"
  (sub-sweep (b)'s caveat: a `.fn(` receiver-form match can over-credit a native if a user class
  shares a method name).
- **Not swept:** `src/jit/**` and `src/vm/**` internals for determinism (their maps are keyed
  lookups producing no rendered lists — [Inferred], not proven).
- **Out of scope per the caller** and deliberately untouched: package/`package Main` enforcement,
  LSP completion + editor grammars, database module, wildcard imports, loop syntax,
  UFCS-vs-qualified idiom, the `class main` example, `Core.Input` streaming, filesystem locking,
  cloning, Rust code quality/naming/file-size, docs consistency/SSOT divergence, and the
  already-found nested-block variable-shadowing P0. Two findings below (I13, I17) brush the docs
  boundary; I flag the overlap inline rather than drop an Invariant-9 result.

---

## Already-known findings: still-open vs now-fixed

Source audit for all rows: `docs/research/full-audit/raw/H-enforcement.md` (2026-07-01/03) unless noted.

| # | Known finding | Source | Status now | Evidence |
|---|---|---|---|---|
| 1 | **P0** private/protected **static field** read+write from outside accepted; `run`/VM≠PHP | H §2.1, §7 | ✅ **FIXED** | `k1-privstatic.phg` → rc=1, `` `s` is a private field of `A` `` **[E-FIELD-VISIBILITY]** + hint. FEATURES.md:95 already records "Static-field visibility (G4) confirmed enforced". [Verified] |
| 2 | **P1** static method callable via instance (`a.m()`) — the caller's named **G5** | H §2.2, §7 | ✅ **FIXED** | `k2-staticviainstance.phg` → rc=1, `` `m` is a static method of `A` — call it as `A.m(…)`, not through an instance `` **[E-STATIC-VIA-INSTANCE]**. A conformance golden now exists (`conformance/diagnostics/static-method-via-instance`). [Verified] |
| 3 | **P2** `E-ALIAS-CYCLE` detected but diagnostic **uncoded**; unused cycle passes clean | H §1, §7 | ✅ **FIXED (both halves)** | `p2-alias.phg` → `type alias cycle: B → A` **[E-ALIAS-CYCLE]** + hint; `p2b-unusedalias.phg` (cycle declared, never used) **also** rc=1 ⇒ resolution is now eager. [Verified] |
| 4 | **P3** `E-OVERLOAD-SELECT-CONFLICT` registered in `explain`, never raised | H §1, §7 | ✅ **FIXED** | `phg explain E-OVERLOAD-SELECT-CONFLICT` → `unknown diagnostic code`; entry removed. `E-OVERLOAD-SELECT-UNKNOWN` survives and *is* raised. [Verified] |
| 5 | **P1** package-**decl** casing arm CLI-unreachable (project mode accepts `package acme;`) | H §1, §2.3, §7 | ✅ **FIXED** | project `proj-case/` with `src/acme/util.phg` declaring `package acme;`, **reached** via `import acme.Util;` → rc=1, `` src/acme/util.phg: package segment `acme` must be PascalCase `` **[E-PKG-CASE]**. Raise sites now also in the loader (`src/loader/fs.rs:104`). [Verified] |
| 6 | **P1** unknown import silently accepted (loose + project) — H's **M2** | H §2.3, §6, §7 | ⚠️ **PARTIALLY FIXED — now loud but WRONG** | `k3-unkimport.phg` (`import Core.Bogus;`) → rc=1, so no longer silent. But the code is **`E-UNUSED-IMPORT`** with the message *"nothing in this file references `Bogus` — remove the import, or use it"*, which misdiagnoses "does not exist" as "unused". → escalated as **I2**. [Verified] |
| 7 | **P1** reserved **`Core.` root** unenforced on every CLI path | H §2.3, §7 | 🔴 **STILL OPEN — and now with worse diagnostics** | project `proj-res/` with `src/Core/Thing.phg` declaring `package Core;`, **reached** via `import Core.Thing;` → **no `E-RESERVED-PACKAGE`**; instead `unknown function Thing` + a bogus `E-NEW-ON-NONCONSTRUCT` "call a function without `new`". Same for `package Core.Output;` (`proj-core/`). → **I3**. [Verified] |
| 8 | **P1** lambda assignment to a by-value captured `mutable` local compiles; **the write silently vanishes** — H's **M1** | H §6, §7 | 🔴 **STILL OPEN** | `m1-capture.phg`: `mutable int x = 1; var f = function(): void { x = 5; }; f();` then print `x` → **`1`**, rc=0, `check` clean. PHP forces `use (&$x)` to opt into sharing, so the copy is *visible* there and invisible here. → **I14**. [Verified] |
| 9 | **P2** no `W-UNUSED-*` family (unused local / param / silent shadowing) — H's **M3/M4** | H §6, §7 | 🔴 **STILL OPEN** | `m3-unused.phg`: unused local `int dead = 9;` **and** unused param `int ignored` → `OK (type-checks clean)`, rc=0, zero warnings. Sharpened by contrast with row 6: an unused **import** is a *hard error*. → **I15**. [Verified] |
| 10 | **P2** catch of a never-thrown type accepted silently — H's **M5** | H §6, §7 | 🔴 **STILL OPEN** | `m5b-catch.phg`: `try { print } catch (BoomError e) { … }` where the block throws nothing → `OK (type-checks clean)`. [Verified] |
| 11 | **P2** self-referential property hook (`get => this.p`) compiles, stack-overflows — H's **M6** | H §6, §7 | 🔴 **STILL OPEN — and the two backends now disagree** | `m6b-hook.phg` compiles clean; faults at runtime on both engines. **New**: the two engines' failure output diverges structurally → **I8**. [Verified] |
| 12 | **P2** `Math.sqrt(-1.0)` → NaN while `0.0/0.0` faults (half-enforced no-NaN story) — H's **M7** | H §5, §6, §7 | 🔴 **STILL OPEN** | `nan1.phg` → prints `NaN` then `false` (`n == n`); `nan2.phg` (`a/a`, `a = 0.0`) → `runtime error at 9: division by zero`. [Verified] |
| 13 | **P1** VM fault line = 1 inside `"{…}"` interpolation; trace frames skewed (W0-5 → fix W5-13) | H §5; `docs/INVARIANTS.md:90-96`; `KNOWN_ISSUES.md:2039-2051` | 🔴 **STILL OPEN (both halves, incl. the widened checker half)** | `pos1.phg`/`pos6.phg`: a **static checker** error inside interpolation reports `at 1:1` / `at 1:5` and underlines **`package Main;`** — the same error outside interpolation is located exactly (`pos4.phg` → `at 5:11`). Already widened-in-scope by `MASTER-PLAN.md:1259` (UA-0.14) and disclosed at `KNOWN_ISSUES.md:2042`. Not re-reported as new; see I9 for the one **un**disclosed consequence. [Verified] |
| 14 | **P1** `->` return-type syntax still parses (retirement sequence UA-1.5) | `MASTER-PLAN.md:1276`, `:1556` | 🔴 **STILL OPEN** | `s1-arrow.phg`: `function f() -> int` **and** `function main() -> void` → `OK (type-checks clean)`; `phg format` then silently rewrites both to `: T`. Parser accept-site `src/parser/types.rs:113` (`self.eat(FatArrow) \|\| self.eat(Arrow)`), lambda site `src/parser/exprs/primary.rs:167`. 85 `package Main;`-bearing Rust test strings still use it. [Verified] |
| 15 | **P3** `var f = a.m` reports "no field m" even for a public method | H §2.1, §7 | ⚪ **NOT RE-PROBED** — outside my remaining budget; treat H's status as current. [Unverified] |

**Net:** 6 fixed, 8 confirmed still-open, 1 partially fixed (→ escalated), 1 unverified.

---

## Area 1 — Missing enforcement (rules the project *states* but does not *gate*)

### I1 · `phg disassemble` output is non-deterministic — direct Invariant-10 breach, and the code's own doc comment claims the opposite · **P1** · [Verified]

`compiler::program` assigns overload **set ids** by iterating `overload_order: HashMap<String, Vec<usize>>`
(`src/compiler/program.rs:151`) and `method_order: HashMap<(String,String), OverloadSet>` (`:465`).
Those ids are printed by `phg disassemble` as `CallOverload(sid, argc)` (`src/cli/pipeline.rs:841-848`).

Reproducer — `d1-overload.phg` (3 two-arm overload sets `alpha`/`beta`/`gamma`), 12 consecutive runs:

```
$ for i in $(seq 1 12); do phg disassemble d1-overload.phg | grep -o "CallOverload([0-9]*" | tr '\n' ' '; echo; done | sort -u
CallOverload(0 CallOverload(1 CallOverload(2
CallOverload(0 CallOverload(2 CallOverload(1
CallOverload(1 CallOverload(2 CallOverload(0
CallOverload(2 CallOverload(0 CallOverload(1
CallOverload(2 CallOverload(1 CallOverload(0
```

**5 distinct outputs from 12 runs of the same input.** `disasm_program`'s own doc comment
(`src/cli/pipeline.rs:806-808`) states *"the method table is sorted (HashMap iteration order is
non-deterministic — invariant #8) so the output is stable across runs"* — the table is sorted; the
**ids** are not, so the claim is false.

**Blast radius is bounded (checked, and this is the good news):** program output and transpiled PHP
are **stable** — 20 `phg run` invocations all printed `ai bs gi`; 12 `phg transpile` invocations all
hashed to `424dbf155dea9e7f4d82467a25a6e64d`. So the byte-identity spine (Invariant 1) is **not**
broken; only the inspection surface is.

*Recommended fix:* iterate `overload_order` / `method_order` through a sorted key view (or make them
`BTreeMap`) before assigning set ids. Then delete or keep the doc comment honestly. A golden
`disassemble` snapshot test on a ≥2-overload-set program would ratchet it.

### I2 · A nonexistent module or member import is reported as `E-UNUSED-IMPORT` — the diagnostic misdiagnoses the error class · **P1** · [Verified]

The H-audit's M2 ("unknown import silent") was closed by making imports loud, but with the wrong
code. Current behaviour, single-file mode:

```
$ phg check i2-bogusmod.phg        # import Core.NoSuchModule;
… unused import `Core.NoSuchModule` — nothing in this file references `NoSuchModule`
  (remove the import, or use it) [E-UNUSED-IMPORT]     rc=1
$ phg check i1-bogusmember.phg     # import Core.Output.nosuchThing;
… unused import `Core.Output.nosuchThing` — nothing in this file references `nosuchThing`
  (remove the import, or use it) [E-UNUSED-IMPORT]     rc=1
```

The message's remedy — *"or use it"* — is unachievable, and following it makes things worse:

```
$ phg check i3-usedbogus.phg       # import Core.NoSuchModule; + NoSuchModule.doIt()
type error at 1:1: unknown identifier `NoSuchModule`   [E-UNKNOWN-IDENT]
```

So the user is never told the module does not exist. `E-MODULE-NOT-FOUND` exists and is documented
(`src/cli/explain/imports_casts.rs:95`, "an import does not resolve to any package on disk") but only
fires from the loader for *project* paths (`src/loader/entry.rs:120`); `E-IMPORT-UNKNOWN` exists
(`src/checker/intrinsic_imports.rs:91,338`, `src/checker/program/imports.rs:76`) but covers enum-variant
and intrinsic member imports, not the `Core.*` root. FEATURES.md:94 advertises `E-IMPORT-UNKNOWN`
("member not exported") as an enforced hard error — true for the wildcard path, not for this one.

*Recommended fix:* resolve the import path first; unresolvable → `E-MODULE-NOT-FOUND` (module) or
`E-IMPORT-UNKNOWN` (member, with a did-you-mean over the module's real exports). Reserve
`E-UNUSED-IMPORT` for imports that **did** resolve. Ordering matters: existence before usage.

### I3 · The reserved `Core.` root is still unenforced, and now produces two *actively wrong* errors · **P1** · [Verified]

H §2.3's P1 is unfixed (see table row 7) and the failure mode has degraded from "silently dead" to
"misleading". Project `proj-res/` — `src/Core/Thing.phg` declaring `package Core;` with a
`public class Thing`, imported and used from `src/Main/main.phg`:

```
type error at 9:11: unknown function `Thing`
type error at 9:11: `new` is only for constructing a class or enum variant   [E-NEW-ON-NONCONSTRUCT]
  hint: call a function without `new`; `new` precedes a class/variant construction
```

The user imported a real class from a real file on disk and is told (a) it is an unknown *function*
and (b) to stop using `new` — advice that cannot possibly help. `E-RESERVED-PACKAGE` is raised at
`src/checker/program/walk.rs:114` and `src/loader/fs.rs:97` but neither site sees a per-file
`package Core…;` decl in project mode, because the loader flat-merges before `check()` (H's root-cause
analysis at `src/checker/program.rs:97-126` still holds).

**Additionally, its `explain` text is stale and teaches three wrong things** [Verified]:

```
$ phg explain E-RESERVED-PACKAGE
E-RESERVED-PACKAGE — a user file claimed a `core` package root.
The `core.` root is reserved for the standard library (`Core.Console`, `Core.Math`,
`Core.File`, …), like a built-in type name. Root your own packages elsewhere, e.g.
`package app;` or `package app.util;`.
```

1. lowercase `core` / `` `core.` `` — the reserved root is `Core.` (`E-PKG-CASE` requires PascalCase);
2. **`Core.Console` does not exist** — 0 hits in `src/native/registry_modules.rs`, `src/cli/preludes.rs`,
   `FEATURES.md`, `examples/README.md`; the module is `Core.Output`;
3. its own suggested remedy is now illegal — `package app;` → `E-PKG-CASE: package segment 'app' must
   be PascalCase` [Verified: `x1-lowerpkg.phg`].

*Recommended fix:* run the reserved-root + decl-casing checks **per file in the loader, before the
flat merge** (where `E-PKG-PATH` already lives — H's own recommendation, still the right shape), and
rewrite the explain entry against current reality.

### I4 · No source-level gate enforces Invariant 6, and the cooperative-VM test harness violates it · **P2** · [Verified]

Invariant 6 ("Reified operands thread ALL vm-compile paths … a miss hides a VM≠tree-walker divergence
off the differential's CLI path") has **no automated gate** — nothing forbids a new
`compiler::compile(` call site. I found exactly one live violation:

- `src/vm/coop.rs:123-124` — the cooperative-VM test harness does
  `parse_checked_program(src)` then `crate::compiler::compile(&prog)`, i.e. **plain `compile`, no
  reified side-table**. Its doc comment (`:120-121`) claims *"Mirrors `cmd_run`'s pipeline"* — false:
  production `cmd_run` (`src/cli/pipeline.rs:411-413`) does `check_and_expand_reified` +
  `compile_with`. `src/interpreter/coop.rs:211` is the interpreter twin (correct there — the
  interpreter needs no reified table).
- `parse_checked_program` exists **only** for this bypass (it is the sole non-definition caller);
  the correct twin `parse_checked_program_reified` is right beside it at `src/cli/pipeline.rs:327`.

Production is correct, so this is a **test-fidelity** hole, not a live bug — but it is precisely the
blind spot the invariant names: the concurrency-path tests structurally cannot catch a
reified-operand divergence. `tests/cli.rs:546` gates that CLI commands accept a reified program,
which does not cover non-CLI compile entry points.

*Recommended fix:* one-line swap to `parse_checked_program_reified` + `compile_with` in
`src/vm/coop.rs`; then either delete `parse_checked_program` or add a CI grep gate (the
`unsafe-island` job in `.github/workflows/ci.yml:89` is the working precedent for a source-shape gate).

### I5 · The DEC-252 `check` ≡ LSP drift guard is boolean-only over 4 hardcoded cases · **P2** · [Verified: read `src/cli/pipeline.rs:854-897`]

Invariant 17 states *"`phg check` ≡ LSP diagnostics (same pipeline, **never diverge**)"*. The guard
(`front_end_diagnostics_agrees_with_check`) compares `has_error(prog)` — a **boolean** — over exactly
**4** inline source strings. The two paths may therefore emit different **codes, messages, positions,
hints, counts, or severities** and the gate stays green; only a flipped error/no-error verdict fails
it. `src/cli/pipeline.rs:254-256` acknowledges the fragility in prose ("**STANDING RULE:** this
mirrors `check_and_expand_reified`'s pass sequence exactly; any change to that sequence must be
reflected here"), which is a comment, not a gate.

*Recommended fix:* compare the full normalized diagnostic vectors (code, line, col, message, hint) and
drive the case list from an existing corpus — `conformance/diagnostics/` (9 cases) or the probe corpus
— rather than 4 literals.

### I6 · Two more Invariant-10 determinism breaches in emitted diagnostics · **P2** · [Verified: source read]

Beyond I1 and I7, the HashMap sweep found two multi-diagnostic ordering hazards (I did not build a
reproducer for either — graded [Inferred] on observability, [Verified] on the code shape):

- `src/checker/collect/types_decls.rs:731` — iterates `hooks: HashMap<String, HookInfo>` (declared
  `:356`) emitting one `E-HOOK-DUP` per colliding hook. Two colliding hooks in one class ⇒ the two
  diagnostic **lines swap between runs**; `cli::pipeline::render_all` (`:43-45`) and
  `diagnostic::diagnostics_json` (`:274`) both render in emission order and never sort.
- `src/checker/resolve.rs:158-186` — `E-INTERSECT-SIG` names the first clashing method found while
  iterating `ClassInfo.methods: HashMap<String, Vec<FnSig>>`; with two clashing names the error names
  whichever the hash seed surfaced (`sig_conflict.is_none()` keeps only the first). The interface leg
  is safe — `iface_flat_methods` sorts (`src/checker/collect/inherit.rs:320-324`).

*Recommended fix:* sort before render / before first-wins selection. **The invariant is otherwise well
kept** — 15+ compliant sort-before-render sites verified, several citing Invariant 10 by name
(`src/checker/matches.rs:147,214`, `src/checker/calls/variants.rs:25`, `src/transpile/stmt.rs:48,60`,
`src/lsp/{mod.rs:161,references.rs:26,completion/mod.rs:245,catalog.rs:35}`,
`src/ext/session/natives.rs:196`, `src/checker/desugar_di/mod.rs:70-92`), and `BTreeMap`/`BTreeSet`
is the house default in `transpile/`, `ast/`, `native/mod.rs`, `ext/registry.rs`.

### I7 · The flagship "did you mean" hint is non-deterministic · **P1** · [Verified]

`Checker::nearest_name` (`src/checker/plumbing.rs:165`, candidates built `:144-156`) does
`.map(|c| (levenshtein(name, c), c)).filter(d <= 2).min_by_key(|(d, _)| *d)` over a `Vec` built by
`extend(scope.keys())` / `extend(self.funcs.keys())` / `extend(info.fields.keys())` — all `HashMap`
keys. `min_by_key` returns the **first** minimum in iteration order, so with ≥2 candidates at equal
edit distance the hint varies per process (`RandomState` is per-process).

Reproducer — `d2-hint.phg` (locals `car`, `cot`, `cut` in scope; typo `cat`), 20 consecutive runs:

```
$ for i in $(seq 1 20); do phg check d2-hint.phg | grep -o "did you mean \`[a-z]*\`"; done | sort | uniq -c
      5 did you mean `car`
      7 did you mean `cot`
      8 did you mean `cut`
```

This is the surface FEATURES.md:87 advertises as a headline ✅ ("Sharp diagnostics: caret-underlined
span, **did-you-mean hints**, stable codes"), it flows into `phg check --json` and therefore into LSP
hover/quick-fix text, and **no test would catch it**: no assertion anywhere in `tests/`, `src/`, or
`conformance/` pins a `nearest_name` hint with ambiguous candidates [Verified: grep].

*Recommended fix:* `candidates.sort()` before `min_by_key`, or tie-break lexicographically. Two lines.
Highest impact-per-effort finding in this report.

### I8 · `run` ≡ `run --tree-walker` failure behaviour diverges for a self-referential property hook — outside the documented interpolation exception · **P1** · [Verified: `run` vs `--tree-walker`; PHP leg Unverified]

Invariant 1 requires *"identical stdout AND identical failure behaviour, for every program"*;
`docs/INVARIANTS.md:90` names the interpolation fault-line skew as **"the one exception"**. This is a
second one, and it is not in interpolation.

`m6b-hook.phg` — `class C { constructor(public mutable int raw) {} int p { get => this.p; } }`, read
`c.p` from a plain statement at line 17:

```
$ phg run m6b-hook.phg               ;# rc=1          $ phg run --tree-walker m6b-hook.phg   ;# rc=1
runtime error at 9: stack overflow                     runtime error at 17: stack overflow
    get => this.p;                                       int v = c.p;
stack trace (most recent call first):                  stack trace (most recent call first):
  → C::p$get           line 9                            → main               line 17
    C::p$get           line 9   (×~4096 frames)
```

Different fault line (**9 vs 17**), different underlined source line, and a **4099-line vs 4-line**
stack trace with entirely different frames. Exit code and the `FaultKind` body (`"stack overflow"`)
agree — which is exactly why the differential harness cannot see it: `agree_err` classifies on the
fault **body substring** (`docs/INVARIANTS.md:12-15`).

**Isolated to hooks** [Verified]: the same shape via a plain recursive function (`r1-recfn.phg`,
`function deep(int n): int { return deep(n + 1); }`) is **byte-identical** on both engines —
`diff` of the two full outputs reports no difference. So this is not the generic
deep-recursion/interpolation skew; the tree-walker's hook-getter call appears not to push a
trace frame, hitting `MAX_CALL_DEPTH` while still attributed to the caller.

*Recommended fix:* push a frame for hook getter/setter invocation in the interpreter (mirroring the
VM's `C::p$get`), then add a differential case. Ladder note: this also strengthens H's M6 (row 11) —
a `W-HOOK-SELF-REFERENCE` warning on syntactic self-reference would make the whole class unreachable.

### I9 · Invariant-12 naming enforcement is genuinely complete — verified, no finding · [Verified]

Reported as a positive because the caller asked me to probe it and a stated-but-unenforced naming
rule would have been the highest-value finding class. It is **fully enforced**, in every position I
could construct (14 probes, all rc=1 with a code and a corrected-name hint):

| Position | Rule | Code | Probe |
|---|---|---|---|
| free function | camelCase | `E-NAME-CASE` | `function BadName()` → hint `badName` |
| function (snake) | camelCase | `E-NAME-CASE` | `snake_case_fn` → hint `snakeCaseFn` |
| method / field / **static field** | camelCase | `E-NAME-CASE` | `Bad_Method`/`Bad_Field`/`Bad_Static` |
| parameter | camelCase | `E-NAME-CASE` | `function f(int Bad_Param)` |
| local | camelCase | `E-NAME-CASE` | `int Bad_Local = 1;` |
| class | PascalCase | `E-TYPE-CASE` | `class badname` → hint `Badname` |
| enum / **enum variant** | PascalCase | `E-TYPE-CASE` | `enum lower_enum`, `E { lower_variant }` |
| interface / trait | PascalCase | `E-TYPE-CASE` | `interface some_iface`, `trait bad_trait` |
| class `const` | SCREAMING_SNAKE | `E-CONST-CASE` | `const int bad_const` → hint `BAD_CONST` |
| package segment / import segment / alias | PascalCase | `E-PKG-CASE` | `package acme;` (project, reached) |

**One un-probed sub-rule:** CLAUDE.md Invariant 12 says *"type-params PascalCase"*. I could not
construct a probe — `new Box<int>(1)` is a **parse error** (turbofish on constructors is unsupported,
see I11), so I could not reach a lowercase-type-param declaration that is also used. Grade
[Unverified] for that one cell; every other cell is [Verified].

**Invariants 3 and 4 also check out** [Verified: source read]: `src/chunk/validate.rs:41` documents
the closed `_ => None` gap; the only `_ =>` arms in `src/vm/exec.rs` (`:847`, `:893`, `:986`) are on
`Value`/receiver-kind matches, not on `Op`; `stack_effect` lives at `src/compiler/emit.rs:75`. Value
kernels are single-sourced in `src/value/` with no re-inlined `checked_*` in a backend.

---

## Area 2 — Incomplete features / unfulfilled promises

### I10 · The source is exceptionally clean of stub markers — verified, no finding · [Verified]

Stated as a positive so the register can retire this suspicion. Across 424 non-test `.rs` files:
`todo!()` **0**, `unimplemented!()` **0**. `TODO`/`FIXME`/`XXX`/`HACK` = **4 total**, of which 3 are
false positives (`\uXXXX` in `src/diagnostic.rs:253`, `src/json.rs:173`,
`src/ext/json/parser/mod.rs:193`). **The single genuine one:** `src/lift/lifter/exprs.rs:343` — *"The
lift draft never synthesizes defaults (PHP promoted defaults are a lift TODO)"* — a real Invariant-17
lift gap, P3, already narrow and disclosed in place. `panic!` outside `#[cfg(test)]` blocks: **0**
reachable from user input (all 18 raw hits are inside inline test modules). EV-7's "never panics on
input" discipline is upheld structurally.

### I11 · Turbofish on a constructor is a raw parse error, not the documented `E-TURBOFISH-NON-GENERIC` · **P2** · [Verified]

`KNOWN_ISSUES.md:1493-1497` states that explicit type arguments on *"a **constructor** / enum-variant
construction …"* **"are rejected (`E-TURBOFISH-NON-GENERIC`)"**. Actual behaviour:

```
$ phg check g4-ufcs-import.phg      # var s = new Stack<int>();
parse error at 13:20: expected '(' — `new` must be followed by a constructor call,
e.g. `new Counter()`, found Lt
```

No code, no mention of turbofish, and the suggested shape (`new Counter()`) does not tell the user how
to supply `T`. Documented-vs-actual divergence in a deferral the docs claim is *cleanly* rejected.

**Mitigation exists and works** [Verified]: contextual inference covers the uninferrable-`T` case —
`Stack<int> s = new Stack();` binds `T = int` and then type-checks members correctly
(`g5-ctxinfer.phg`: `s.push("oops")` → `` `push` argument 1 expects `int`, found `string` ``). So this
is a diagnostic-quality finding, not a capability hole.

*Recommended fix:* accept `new C<...>(...)` in the parser and reject it in the checker with
`E-TURBOFISH-NON-GENERIC` + a hint pointing at the contextual form (`Stack<int> s = new Stack();`).

### I12 · `FEATURES.md` — the advertised capability matrix — omits shipped language features · **P2** · [Verified]

`README.md` routes users to `FEATURES.md` as "what works **today**". Two fully-shipped, exampled,
DEC-ruled features are absent from it:

| Feature | Shipped | Proof | In `FEATURES.md`? |
|---|---|---|---|
| **Named arguments** (DEC-297) | ✅ | `examples/guide/named-args.phg`, `examples/README.md:107`; probe `e1-named.phg` → clean | **0 hits** |
| **Variadic parameters** (DEC-298) | ✅ | `examples/guide/variadics.phg`, `examples/README.md:108`; probe `v1-variadic.phg` → `check` clean, VM `6`, tree-walker `6`, transpiles | **0 hits** |
| `#[Invoke]` callable instances | ✅ (slice 1, `docs/archive/specs/2026-07-23-invoke-tostring.md` §8) | probe `f3-invoke.phg` → `E-NOT-CALLABLE` + *"add an `#[Invoke]` method to make instances callable"* | **0 hits** |

`grep -c -i` on `FEATURES.md`: `named arg` = 0, `variadic` = 0, `invoke` = 0; on `examples/README.md`:
2, 2, 3. Correctly-omitted-because-not-built (verified via probe, so the matrix is right to be silent):
`any`/`object` top types (`f2-any.phg` → `E-UNKNOWN-TYPE` ×2), labeled `break`/`continue`
(`f1-labeled.phg` → parse error), user-type index access (`f4-arrayaccess.phg` → `type Bag cannot be
indexed`) — all three have RULED-but-QUEUED specs dated 2026-07-23.

*Note:* this brushes the excluded "docs consistency" scope. I report it because it is a *capability*
claim gap found by probing, not a cross-doc wording drift — the fix is 3 FEATURES.md rows.

### I13 · Invariant 9 — one shipped feature with no example, plus 11 stale README rows and 3 README overclaims · **P2** · [Verified]

Bidirectional inventory: **Side A** = 48 user-importable `Core.*` modules (23 prelude,
`src/cli/preludes.rs:605` `CORE_MODULES`; 23 registry, `src/native/registry_modules.rs`; 2 checker
intrinsics, `src/checker/intrinsic_imports.rs:45` — `Core.Assert`, `Core.Abort`) + ~95 `FEATURES.md` ✅
rows. **Side B** = 266 `.phg` files, 257 `examples/README.md` index rows, 46 real `Core.X` modules
imported. Delta = 2 modules unimported.

- **Genuine Invariant-9 violation (1):** **shebang / extensionless executable entries (DEC-336)** —
  `FEATURES.md:96` claims ✅ with `chmod +x bin/console && ./bin/console migrate`. Shipped at
  `src/tokenizer/mod.rs:152` + `src/loader/discovery.rs:54`; covered by `tests/cli.rs` only.
  `grep -rl 'env phg' examples/` → empty; no executable or extensionless file under `examples/`; no
  README row. The feature landed without its example.
- **`Core.Test` (8 natives) never imported by any example** — **not** a violation: disclosed at
  `examples/README.md:203`, which routes to `selftest/` because `test` blocks aren't byte-identity
  programs. Compliant in spirit; the letter is waived by an explicit README entry.
- **11 README rows point at a directory that does not exist:** `db/{basic,typed,streaming,mysql,nested,transactions,transaction-closure,writes,naming,mapping,postgres}.phg`.
  On disk the directory is `examples/database/`; `examples/db` does not exist. Line 232 of the same
  file has it right in prose (`phg run examples/database/postgres.phg`) while the row labels say `db/`.
- **3 README overclaims:** `millisecondsOfDay`, `ofEpochDay`, `ofEpochMilliseconds` each appear in a
  README row (`guide/time.phg`, `guide/dates.phg`, `guide/datetimes.phg`) with **zero** occurrences
  anywhere in `examples/**/*.phg` — the README asserts a demonstration that does not exist.
- **Directories absent from the top-level index:** `examples/lift/`, `examples/random/`
  (each has its own walkthrough README, unlinked); `examples/debug/` has a README and no `.phg`.
- **Thinnest per-module coverage** (exact on the "unused" direction): `Core.Conversion` 8/20 used
  (12 unused incl. the whole `asBool`/`asInt`/`asFloat` family); **`Core.UriModule` 15/36** — the
  sharpest, because `FEATURES.md:109` names the `raw*` getters and strict withers explicitly in its ✅
  row and cites `examples/guide/uri.phg` as the proof, yet `rawScheme rawUserInfo rawUsername
  rawPassword rawHost rawPath rawQuery rawFragment`, `scheme userInfo username password fragment`,
  and `withScheme withUserInfo withHost withPath withQuery withFragment` are never called;
  `Core.Time` 36/42; `Core.FileSystemModule` 13/18. `Core.List` 44/44, `Core.Math` 36/36,
  `Core.Map` 14/14, `Core.Set` 12/12, `Core.Validation` 14/14 are complete.

*Recommended fix:* add `examples/cli/shebang/` (a `bin/console` + README row); `sed` the 11 `db/` row
labels to `database/`; either add the 3 missing Time calls to their cited examples or drop the claims;
extend `guide/uri.phg` to cover the `raw*`/wither families the ✅ row promises.

---

## Area 3 — "Better than PHP" goal-blockers (Invariant 16 / META-7)

### I14 · The most-hit diagnostics carry no `E-` code, so `phg explain` is unreachable for them by construction · **P1** · [Verified]

The registered ↔ raised loop is **airtight and ratcheted** — 313 registered codes, 307 raised,
`phg explain` executed on all 620, **0 failures in both directions**, guarded by
`src/cli/tests/explain_ratchet.rs:48`. The gap is the **third axis nobody guards**:
`Diagnostic.code` is `Option`, defaults to `None` (`src/diagnostic.rs:66,92`), and **nothing ratchets
against `None`**.

Probed via the authoritative `check --json` `code` field:

```
type mismatch        code=null  | expected `string`, found `int`
return mismatch      code=null  | expected `int`, found `string`
arity                code=null  | `add` expects 2 argument(s), found 1
unknown method       code=null  | type `string` has no method `nope`
unknown function     code=null  | unknown function `nosuchfn`
if-condition type    code=null  | `if` condition must be `bool`, found `int`
bad `+` operands     code=null  | `+` concatenates two `string`s or adds two numbers …
non-exhaustive match code=null  | non-exhaustive match: missing B
user-type indexing   code=null  | type `Bag` cannot be indexed          (my probe f4)
```

Uncoded emission-site inventory (non-test `src/`; counts [Inferred] from a regex+window scan, the
shape [Verified]): **parser ~226 of 236** (`src/parser/mod.rs:142,157` — only 10 sites attach a code),
**tokenizer 31 of 33**, **checker 107 sites / 90 distinct messages** (`src/checker/plumbing.rs:89`,
the uncoded `self.err` vs 342 `err_coded`), **VM 45** (`src/vm/exec.rs:145,149,153,192,208,…`),
**native stdlib 291**, **compiler 13** (`src/compiler/cty.rs:136,141,181,…`), **pm 31**, **lift 29**,
**bundle 8**, **serve 5**, **all runtime faults** (`Diagnostic::runtime`/`runtime_at_line`,
`src/diagnostic.rs:198,203`, 30 call sites, never `with_code`).

**A missing semicolon — the most common error in any language — has no `phg explain` path.** Against
PHP this is parity, not a win: PHP's `syntax error, unexpected …` is equally uncodeable. The project's
own doctrine makes diagnostics a headline claim, so parity here is a goal-blocker.

Also **alive but invisible to the ratchet**: `E-TRANSPILE-UNCHECKED`, `E-TRANSPILE-UNICODE`,
`E-CONCURRENCY-NO-PHP` are raised as message **prefixes**, not `code` fields
(`src/transpile/functions.rs:29`, `src/transpile/call.rs:360`, `src/transpile/expr.rs:548`) — the
rendered diagnostic has no `[E-…]` bracket line, so the user has no token to feed `explain`.

**And the golden corpus is blind here:** `conformance/diagnostics/` holds **9** cases, and **all 9
assert a `[CODE]` line** — so `tests/diagnostics.rs` never pins a single uncoded diagnostic.

*Recommended fix:* add a `Diagnostic.code == None` ratchet with a shrinking allowlist — the exact
analogue of the existing `explain_ratchet`, which is the proven pattern in this repo. Then code the
top tier first: type mismatch, arity, unknown method/field, non-exhaustive match, parse `expected X`.

### I15 · `phg check --json` emits plain text for parse, lex, and runtime errors — the machine-readable contract breaks on the most common error class · **P1** · [Verified]

`FEATURES.md:85` claims *"`phg check --json` emits machine-readable diagnostics
(stage/severity/message/line/col/code/hint) for editors/LSP"*. It holds for type errors and fails for
syntax errors:

```
$ phg check --json j1-typeerr.phg          # type error → valid JSON
[{"stage":"type","severity":"error","message":"expected `string`, found `int`",
  "line":7,"col":25,"code":null,"hint":null}]

$ phg check --json j2-parseerr.phg         # parse error → NOT JSON
…/j2-parseerr.phg: parse error at 7:38: expected ';' after variable declaration, found Ident("Output")
function main(): void { string s = 1 Output.printLine(s); }
                                     ^
$ python3 -c "import json; json.load(open('j2.out'))"
json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)
```

Same for lex errors and for `phg run --json` on a runtime fault. Any LSP or CI consumer parsing stdout
crashes on the single most common failure mode. This is a **correctness bug in a documented interface**,
independent of I14's code question, and it is the one place where a missing code and a broken contract
compound: the consumer can neither parse the output nor look the error up.

*Recommended fix:* route parse/lex/runtime diagnostics through the same `diagnostics_json` renderer
(`src/diagnostic.rs:274`) when `--json` is set. Add a test that `json.load`s the output of a parse
error.

### I16 · Ergonomics PHP has that phorj lacks, ranked by how likely a PHP migrant hits them, with no migration hint in the diagnostic · **P2** · [Verified]

Every row probed against the current binary. "Documented?" = disclosed in `KNOWN_ISSUES.md`,
`FEATURES.md`, or `UNIFIED-SPEC.md`'s rejected list.

| PHP construct | phorj today | Documented? | Hint points to the phorj way? |
|---|---|---|---|
| `switch (x) { case … }` | parse error: *"expected ';' after statement, found LBrace"* | ✅ rejected, `UNIFIED-SPEC.md:1125` (C-style fall-through) | ❌ **no mention of `match`** |
| ternary `c ? a : b` | parse error: *"expected ';' after variable declaration, found Int(1)"* | ⚠️ only as *"ternary stays deferred-not-rejected"*, `MASTER-PLAN.md:1694` | ❌ no mention of expression-`if` (`var x = if (c) { 1 } else { 2 }`) |
| **spread / argument unpacking** `f(...$args)` | parse error: *"expected an expression, found DotDotDot"* | ❌ **undocumented anywhere** — 0 hits in `KNOWN_ISSUES.md`/`FEATURES.md`; only `MASTER-PLAN.md:1647` W4-1 lists it as unbuilt alongside named-args/variadics (both of which **shipped**) | ❌ |
| labeled `break outer;` | parse error: *"expected ';' after statement, found Colon"* | ✅ RULED-but-QUEUED spec `docs/specs/2026-07-23-labeled-break-continue.md` | ❌ |
| `$obj['k']` on a user type | *"type `Bag` cannot be indexed"* (**uncoded**, see I14) | ✅ RULED-but-QUEUED spec `docs/specs/2026-07-23-array-access.md` | ❌ |

**Two shapes of finding here.** (a) The spread row is a genuine **undisclosed** gap: its two W4-1
siblings shipped, so a reader of FEATURES.md/KNOWN_ISSUES has no way to learn spread did not.
(b) The other four are correctly-deferred capabilities whose **diagnostics are the goal-blocker**: a
PHP migrant typing `switch` or `?:` gets a token-level parse error that never names the phorj
replacement, in a project whose stated doctrine is familiarity-first and beat-PHP-on-diagnostics.
This is the cheapest available "better than PHP" win in the whole report — a parse-time keyword
recognizer for `switch`/`?:`/`...`/`label:` emitting `E-USE-MATCH` / `E-USE-IF-EXPR` /
`E-SPREAD-UNSUPPORTED` / `E-LABEL-UNSUPPORTED` with the phorj form in the hint.

*Also confirmed present and good* (so the register does not chase them): named arguments, variadics,
`i++`, `do…while`, `|>` with `%` placeholder, `??`/`?.`/`!`, expression-`if`, `foreach (m as k => v)`,
UFCS receiver form.

---

## Area 4 — Safety / soundness holes

### I17 · Statically-knowable faults are deferred to runtime — parity with PHP where a win is available · **P2** · [Verified]

Four programs whose fault is decidable at compile time from literals alone. All four `check` **clean**
(rc=0) and fault at runtime (rc=1):

| Probe | Program | `check` | Runtime |
|---|---|---|---|
| `z1-divzero.phg` | `int x = 10 / 0;` | `OK (type-checks clean)` | `runtime error at 7: division by zero` |
| `z4-modzero.phg` | `int x = 10 % 0;` | `OK` | `runtime error at 7: modulo by zero` |
| `z3-litoverflow.phg` | `int x = 9223372036854775807 + 1;` | `OK` | `runtime error at 7: integer overflow` |
| `z2-oob.phg` | `var xs = [1,2,3]; int v = xs[7];` | `OK` | `runtime error at 7: list index out of range` |

The faults themselves are **exemplary** — clean, positioned, with a stack trace, never a panic (EV-7
holds). But Rust rejects `1/0` at compile time (`unconditional_panic`), and phorj's whole pitch is
moving PHP's runtime failures to compile time — three of its own brag-list items (checked arithmetic,
bounds-checked indexing, non-exhaustive match) are exactly this move. Here it stops one step short and
lands at **PHP parity** (PHP 8 also throws `DivisionByZeroError` at runtime).

Scope caveat, honestly: `xs[7]` needs const-propagation through a `var` binding, so it is materially
harder than the three literal-operand cases. The literal-divisor and literal-overflow cases are cheap.

*Recommended fix:* a constant-folding pre-check over literal-operand arithmetic emitting
`E-CONST-DIV-ZERO` / `E-CONST-OVERFLOW` (Invariant 15 applies — this is a user-visible language
decision, so it is a **question for the developer**, not a ruling: rejecting `10/0` at compile time is
a behaviour change, and a deliberate `1/0` in a test fixture would stop compiling).

### I18 · Static-vs-instance discipline is now complete — G5's sibling class is closed · [Verified]

The caller asked for siblings of G5. I probed the full matrix; **every cell is enforced**, so the class
is closed rather than partially fixed:

| Angle | Result |
|---|---|
| instance field via class name (`A.x`) | ENFORCED — `A has no static field x` |
| instance method via class name (`A.m()`) | ENFORCED (per H §2.2, unchanged) |
| static field via instance (`a.s`) | ENFORCED — `type A has no field s` |
| **static method via instance (`a.m()`)** — **G5** | ✅ **NOW ENFORCED** — `E-STATIC-VIA-INSTANCE` + hint `write A.m(…)` |
| private/protected **static field** from outside | ✅ **NOW ENFORCED** — `E-FIELD-VISIBILITY` |
| private/protected static **method** / class `const` | ENFORCED — `E-METHOD-VISIBILITY` / `E-CONST-VISIBILITY` |
| `this` in a static method | ENFORCED — `E-STATIC-THIS` |

The remaining soundness gaps in this area are the **capture** (I19) and **hook** (I8) classes, not the
static/instance one.

### I19 · Lambda write to a by-value capture is silently discarded — a wrong-answer footgun, worse than PHP · **P1** · [Verified]

H's M1, still open (table row 8), promoted here because it is the only finding in this report where
**the program produces a wrong answer with no error at all** on any surface:

```
mutable int x = 1;
var f = function(): void { x = 5; };
f();
Output.printLine("{x}");        # prints 1 — the write vanished
```

`check` clean, both backends print `1`, rc=0. PHP is **better** here: capture is explicit, so
`function() { $x = 5; }` cannot even see `$x` without `use ($x)`, and sharing requires the visible
`use (&$x)`. Phorj's capture is implicit, which makes the copy invisible and the footgun silent.

*Recommended fix:* H's own recommendation stands — **ERROR on assignment to a by-value capture**
(`E-CAPTURE-ASSIGN`), with a hint naming the functional alternative (return the value / use a field on
a shared handle). Invariant 15 note: this is user-visible, so it needs a developer ruling; the failing
program above is the minimal current-syntax case to embed in the question.

### I20 · Unused-binding asymmetry: unused **import** is a hard error, unused **local/param** is entirely silent · **P2** · [Verified]

H's M3/M4, still open (table row 9), sharpened by a contrast that did not exist when H ran. DEC-282
made import hygiene **maximal** — `E-IMPORT-MAIN`, `E-DUP-IMPORT`, `E-UNUSED-IMPORT` are all *hard
errors* (`FEATURES.md:93`, *"all three were silently accepted before"*), and I2 shows how aggressively
`E-UNUSED-IMPORT` fires. Meanwhile:

```
function h(int used, int ignored): int { return used; }   # `ignored` — silent
… int dead = 9;                                          # `dead` — silent
→ OK (type-checks clean)   rc=0   zero warnings
```

There is **no `W-UNUSED-*` channel at all** for bindings. The inconsistency is the finding: a project
that fails the build over an unused *import* cannot coherently ignore an unused *local*, and the
warning tier is where dead code actually accumulates.

### I21 · Resource-safety classes checked and clear — no finding · [Verified]

Recorded so the register can retire them. **File I/O has no handle class:** `Core.File` exposes
`append copy delete exists read rename size write` (`src/native/file.rs`) — whole-file operations
only, no `open`/`close`, so the file-descriptor-leak class does not exist by construction.
**Database connections** do expose `close` (`src/ext/database/natives/registry.rs:270`) and the
examples use it (`examples/database/{transactions,transaction-closure,postgres,mysql}.phg`); Rust
`Drop` covers a forgotten one. Not probed (no live DSN): whether forgetting `close()` warns — likely
not, and PHP does not either, so at worst parity. **Integer/index bounds:** `BytecodeProgram::validate`
turns would-be out-of-range panics into clean errors before the VM runs an op
(`docs/INVARIANTS.md:81`, `src/chunk/validate.rs:41`), and every fault probe (I17) exited 1 with a
clean `Diagnostic`, never a panic.

---

## NEW findings only (ranked)

Excludes everything in the still-open table (rows 7–14), which are re-verifications, not new.

| # | Finding | P | Grade |
|---|---|---|---|
| **I7** | "did you mean" hint is **non-deterministic** — 3 different hints in 20 runs; flows into `check --json`/LSP; no test could catch it | **P1** | [Verified] |
| **I15** | `check --json` emits **plain text** for parse/lex/runtime errors → consumer `JSONDecodeError`; breaks a documented interface | **P1** | [Verified] |
| **I8** | `run` ≠ `run --tree-walker` failure output for a self-referential property hook (line 9 vs 17, 4099 vs 4 trace lines) — a **second** exception to Invariant 1's "one exception"; hook-specific (plain recursion is byte-identical) | **P1** | [Verified] |
| **I1** | `phg disassemble` non-deterministic (5 outputs / 12 runs); the code's own doc comment claims stability. Program output + transpiled PHP **are** stable | **P1** | [Verified] |
| **I14** | No ratchet on `Diagnostic.code == None`: type mismatch, arity, unknown method, non-exhaustive match, **every parse/lex error**, **every runtime fault** are uncoded ⇒ `phg explain` unreachable. Conformance corpus (9 cases) asserts a code in all 9, so it is blind | **P1** | [Verified] |
| **I2** | Nonexistent import reported as `E-UNUSED-IMPORT` ("or use it" — unachievable); using it then gives `unknown identifier` | **P1** | [Verified] |
| **I3** | Reserved `Core.` root: still unenforced **and** now yields two actively wrong errors; `phg explain E-RESERVED-PACKAGE` teaches a lowercase root, the nonexistent `Core.Console`, and an `E-PKG-CASE`-illegal remedy | **P1** | [Verified] |
| **I16** | Spread `f(...$args)` **undisclosed** (its two W4-1 siblings shipped); `switch`/`?:`/label/index rejections carry no migration hint to the phorj form | **P2** | [Verified] |
| **I4** | Invariant 6 has no gate; `src/vm/coop.rs:123` bypasses reified operands and its doc comment falsely claims it mirrors `cmd_run` | **P2** | [Verified] |
| **I5** | DEC-252 `check` ≡ LSP guard is boolean-only over 4 hardcoded cases — codes/messages/positions/counts may diverge freely | **P2** | [Verified] |
| **I13** | Invariant 9: shebang entries (DEC-336) shipped with no example/README row; 11 README rows point at the nonexistent `examples/db/`; 3 README-claimed Time natives absent from the corpus; `Core.UriModule` 15/36 despite its ✅ row naming the uncovered families | **P2** | [Verified] |
| **I12** | `FEATURES.md` omits shipped named-args (DEC-297), variadics (DEC-298), `#[Invoke]` | **P2** | [Verified] |
| **I17** | `10/0`, `10 % 0`, literal `i64` overflow, literal-index OOB all pass `check` — PHP parity where a compile-time win is available | **P2** | [Verified] |
| **I6** | Two more Invariant-10 hazards: `E-HOOK-DUP` emission order (`types_decls.rs:731`), `E-INTERSECT-SIG` first-wins over a HashMap (`resolve.rs:158`) | **P2** | [Inferred] |
| **I11** | Constructor turbofish is a raw parse error, not the documented `E-TURBOFISH-NON-GENERIC` (`KNOWN_ISSUES.md:1493`) | **P2** | [Verified] |
| **I20** | Unused import = hard error; unused local/param = totally silent. No `W-UNUSED-*` channel | **P2** | [Verified] |
| — | 6 stale `explain` entries never raised: `E-MULTIPLE-MAIN` (superseded by `E-DUPLICATE-ENTRY-KIND`), `E-DB-NAMING-NOT-CONST`, `E-DECIMAL-DIV`, `E-MODULE-UNAVAILABLE`, `E-TRANSPILE-FS`, `E-VENDOR-MISSING` | P3 | [Verified] |
| — | 32/313 explain entries contain no imperative fix verb (incl. `E-UNKNOWN-IDENT`, the likeliest beginner code); 92/313 contain no code example | P3 | [Inferred: heuristic regex] |
| — | `src/lift/lifter/exprs.rs:343` — PHP promoted-parameter defaults are never synthesized by the lifter (Invariant 17 gap) | P3 | [Verified] |
| — | `src/cli/module_catalog.rs:12` builds import completion from `CORE_MODULES ∪ registry()`, omitting `Core.Assert`/`Core.Abort` (importable, `intrinsic_imports.rs:45`). *Overlaps the LSP-completion agent's scope* | P3 | [Verified] |
| — | `examples/interop/withdecls/phorj.toml` still exists and still activates legacy project mode (`src/loader/entry.rs:16`) while `FEATURES.md:72` states manifest-less. *Overlaps the SSOT agent's scope* | P3 | [Verified] |

**Positives worth pinning so they never regress silently:** Invariant 12 naming fully enforced in 14
positions (I9); `todo!()`/`unimplemented!()` = 0 and 1 real TODO in 424 files (I10); Invariants 3 & 4
intact (I9); static/instance discipline now complete (I18); no file-handle leak class (I21); the
registered↔raised code loop airtight and ratcheted (I14's good half); 15+ verified sort-before-render
determinism sites (I6's good half); `Core.List` 44/44, `Core.Math` 36/36, `Core.Map` 14/14,
`Core.Set` 12/12, `Core.Validation` 14/14 example coverage (I13's good half).

---

## Top 10 by (impact ÷ effort)

| Rank | # | Fix | Effort | Why it ranks here |
|---|---|---|---|---|
| 1 | **I7** | `candidates.sort()` before `min_by_key` in `src/checker/plumbing.rs:165` | **2 lines** | Removes user-visible non-determinism from the flagship diagnostic surface; nothing currently guards it |
| 2 | **I1** | Sorted key view (or `BTreeMap`) for `overload_order`/`method_order` in `src/compiler/program.rs:151,465` | **~5 lines** | Closes a hard-reproducer Invariant-10 breach and makes an existing false doc comment true |
| 3 | **I4** | Swap `src/vm/coop.rs:123` to `parse_checked_program_reified` + `compile_with`; delete `parse_checked_program` | **1-line swap** | Closes the only live Invariant-6 bypass; deleting the function makes regression impossible |
| 4 | **I2** | Resolve import existence **before** the usage check; emit `E-MODULE-NOT-FOUND`/`E-IMPORT-UNKNOWN` | **S** | Turns an actively misleading error into a correct one on a first-contact surface; codes + explain entries already exist |
| 5 | **I15** | Route parse/lex/runtime diagnostics through `diagnostics_json` under `--json` | **S** | Fixes a broken documented interface; one `json.load` test locks it |
| 6 | **I16(a)** | Parse-time recognizers for `switch` / `?:` / `...` / `label:` → coded errors naming the phorj form; disclose spread in KNOWN_ISSUES | **S-M** | Cheapest available "better than PHP" win; converts 5 cryptic token errors into migration guidance |
| 7 | **I3** | Run reserved-root + decl-casing per file in the loader before the flat merge; rewrite the stale explain entry | **M** | Enforces a documented guarantee; removes two wrong errors; the explain rewrite alone is minutes |
| 8 | **I14** | `Diagnostic.code == None` ratchet with a shrinking allowlist, then code the top tier | **M** (ratchet S) | The ratchet is S and makes the whole M-sized backlog visible in CI; `explain_ratchet` is the proven in-repo pattern |
| 9 | **I8** | Push a trace frame for hook getter/setter in the interpreter + a differential case | **M** | Closes a second exception to the project's central correctness contract, invisible to `agree_err` by construction |
| 10 | **I12 + I13** | 3 `FEATURES.md` rows; `sed` 11 `db/`→`database/` README labels; add the shebang example | **S** | Pure-mechanical Invariant-9/docs debt with no design risk |

**Needs a developer ruling before any build (Invariant 15 — surfaced, not decided):** **I19**
(error on assignment to a by-value capture — the minimal failing program is in I19's body),
**I17** (compile-time rejection of literal `10/0` / literal overflow — a behaviour change that would
break a deliberate fixture), **I20** (introducing a `W-UNUSED-*` warning tier, and whether it is
warning or error to match the import tier), and **I16**'s ternary row (`?:` is recorded as
*deferred-not-rejected*, so its diagnostic wording depends on whether it will eventually ship).

— End of report. Probe corpus preserved at
`/tmp/claude-0/-home-user-phorj/4519ba2a-7bcc-54d2-80b5-d8fbd68ed10d/scratchpad/probe-gaps/`.

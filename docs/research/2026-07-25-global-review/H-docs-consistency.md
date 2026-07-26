# H — Documentation consistency audit (phorj, 2026-07-25)

Auditor lens: doc-vs-reality contradictions, Invariant-19 SSOT divergence, intra-document
contradictions, stale/orphaned docs, and "holding us back" blockers.

---

## Sampling method (honest coverage)

**Read fully** (small top-level docs): `CLAUDE.md` (189 L), `docs/INVARIANTS.md` (208 L),
`ROADMAP.md` (29 L), `docs/DEPRECATION.md` (43 L), `docs/GA-CHECKLIST.md` (53 L),
`docs/adr/README.md` (33 L), `docs/adr/0005-offline-only-vendor.md` (35 L),
`docs/research/2026-07-25-plans-divergence-audit.md` (155 L),
`docs/research/2026-07-25-currency-audit.md` (79 L). Partial reads: `README.md` (lines 25–60,
100–120), `docs/MILESTONES.md` (1–60 + all headings), `FEATURES.md` (all headings + grep of every
`never|always|enforced|guaranteed|byte-identical` line), `STABILITY.md` (1–25),
`CONTRIBUTING.md` (1–50), `examples/lift/README.md` (78–92), `KNOWN_ISSUES.md` (150–180).

**Targeted greps only** (per the brief — these are too large to read): `docs/plans/MASTER-PLAN.md`
(2421 L), `docs/plans/SLICE-STATE.md` (2308 L), `docs/research/full-audit/raw/C-decisions.md`
(3376 L), `docs/specs/UNIFIED-SPEC.md` (1696 L — read §"External dependency policy" 871–915 and
the v0.1 decision table 150–175 in full), `CHANGELOG.md` (4235 L).

**Behaviour testing** — the highest-value axis. I ran `target/release/phg` **~25 times** against
specific doc claims (never `cargo`): the README quickstart commands verbatim; `->` return form;
`phg vendor`; `phg install/add/update/remove`; `phg run --no-jit`; `phg explain` on 5 codes;
12 randomly-sampled `examples/**/*.phg`; a 200k-segment member chain; a `with {}` transpile→lift
round trip. I also grepped `src/` to verify every symbol/path/flag a doc names.

**NOT covered** (declare honestly): I did not read MASTER-PLAN, SLICE-STATE, C-decisions, or
CHANGELOG end-to-end, so I cannot claim completeness on percentage-ledger or DEC-row divergence —
that axis was delegated to a parallel agent (results in the companion section at the end; where that
agent's output had not landed at write time, the gap is stated). I did not run the test suite, so
"the gate is green" is unverified by me. `docs/research/roadmap-completeness/raw/*` (23 files) and
`docs/research/2026-07-03-unification-audit/raw/*` (10 files) were **not** audited — they are dated
historical research, low drift-risk, and out of budget.

**Reconciliation with today's three audits** — done, see §6. I deliberately did not re-report the
`2026-07-25-plans-divergence-audit.md` findings as new; I re-verified each one's current status.

---

## 1. Doc claims contradicted by actual behaviour (highest value)

### H1 — README.md's flagship program and BOTH quickstart commands do not run [P0]

`README.md:36` (the hero snippet, the first Phorj code any reader sees) and the two copy-paste
CLI commands:

> `README.md:36` — `function main(): void {`
> `README.md:111` — ``$ echo 'package Main; import Core.Output; function main(): void { Output.printLine("{1 + 2}"); }' | phg run -``
> `README.md:114` — ``$ phg run -e 'package Main; import Core.Output; function main(): void { Output.printLine("inline!"); }'``

Actual behaviour — I ran `README.md:111` and `README.md:114` **verbatim**:

```
compile error: no entry point: running needs an `#[Entry(kind: EntryKind.Cli)]` function (DEC-331).
A library or web file still type-checks and transpiles — use `phg check` / `phg transpile`
exit=1
```

Both fail. The working form additionally needs two imports the README never mentions
(`import Core.Runtime.Entry;` + `import Core.Runtime.EntryKind;` — a bare `#[Entry]` raises
`E-INJECTED-TYPE-BARE`). Verified working minimum:

```phg
package Main;
import Core.Output;
import Core.Runtime.Entry;
import Core.Runtime.EntryKind;
#[Entry(kind: EntryKind.Cli)]
function main(): void { Output.printLine("ok"); }
```

**Grade:** [Verified: ran both README commands verbatim; captured exit 1 + the DEC-331 message, then
ran the corrected form to exit 0.]
**Severity: P0** — the project's front page. Every first-time reader's first action fails. This is
also an Invariant-17 ("always-current surfaces") breach: DEC-331 shipped the breaking change and the
README was not updated in the same change.
**Correction:** update `README.md:29–41` (hero snippet), `:111`, `:114` to include the `#[Entry(kind:
EntryKind.Cli)]` attribute and the two `Core.Runtime` imports. Add a one-line note that `main` is no
longer implicitly the entry point (DEC-331). **Then grep the rest of the repo for the same pattern** —
any `.md` showing `function main(): void` without `#[Entry]` is equally broken.

---

### H2 — `docs/INVARIANTS.md` §6 promises "never SIGABRT/panic"; I reproduced a SIGABRT [P1]

> `docs/INVARIANTS.md:74-75` — "## 6. No crash on input (EV-7) / Malformed or adversarial `.phg`
> must exit 1 with a clean `Diagnostic`, **never** SIGABRT/panic."
> `docs/INVARIANTS.md:76-77` — "The whole pipeline runs on a 256 MB worker thread
> (`cli::on_deep_stack`) so the *explicit* depth limits — not Rust's ambient stack — bound recursion."

vs

> `KNOWN_ISSUES.md:160-164` — "A deeply nested left-associative expression — a member chain
> `a.a.a.…` (~50–100k+ segments) … overflows the native stack and aborts (`SIGABRT`, `thread 'main'
> has overflowed its stack`) during `phg check`/`run`."

Reproduced (200 000 segments, 400 KB source):

```
$ ./target/release/phg check /tmp/deep2.phg
Aborted (core dumped)
exit=134
thread 'main' (28153) has overflowed its stack
fatal runtime error: stack overflow, aborting
```

Two distinct defects, not one:
1. **The absolute claim is false** and INVARIANTS.md carries **no carve-out** — contrast §7
   (`:89`), which *does* disclose its exception as a bolded "**Known limitation (fault-line
   skew…)**". §6 was not given the same treatment when the SIGABRT was discovered (2026-07-25,
   today).
2. **The stated enforcement mechanism is wrong for this path.** §6 says "the whole pipeline runs on
   a 256 MB worker thread". The abort message says **`thread 'main'`** — so
   `enforce_injected_discipline` (`src/cli/pipeline.rs` → `src/checker/enforce_injected.rs`
   `walk_expr`) runs *off* `cli::on_deep_stack`. A reader trusting §6 would conclude this class of
   bug is structurally impossible.

**Grade:** [Verified: reproduced the abort, exit 134, captured the `thread 'main'` message; read both
doc sides.]
**Severity: P1** — a load-bearing correctness invariant stated as absolute is measurably false, and
its stated mechanism does not hold. Both sides already exist in the repo (KNOWN_ISSUES documents it
honestly); only INVARIANTS.md is out of sync.
**Correction:** add to `docs/INVARIANTS.md` §6 a disclosed-exception paragraph in the §7 style,
pointing at `KNOWN_ISSUES.md` §"STACKDEPTH-deep-member-chain", and correct `:76-77` to say the
*main checker* runs on the deep-stack worker while `enforce_injected` and attribute-arg walkers do
not (that is the actual gap).

---

### H3 — `CLAUDE.md:9` says the core has **four** external deps; there are **14** [P1]

> `CLAUDE.md:8-10` — "Phorj is a statically-typed, PHP-inspired language implemented in Rust
> (edition 2021; core is std-only with **four** vetted, feature-gated exceptions — `argon2`,
> `regex`, `ctrlc`, `corosensei` — per `docs/specs/UNIFIED-SPEC.md` §"External dependency policy")"

Actual `Cargo.toml` optional dependencies (all 14, with line numbers):

| crate | Cargo.toml | gated by | **default-on?** |
|---|---|---|---|
| `argon2` | :130 | `cryptography` | **yes** |
| `unicode-segmentation` | :133 | `unicode` (DEC-256) | **yes** |
| `rustls` | :134 | `http-client` | no |
| `webpki-roots` | :135 | `http-client` | no |
| `corosensei` | :144 | `green` | **yes** |
| `cranelift` | :181 | `jit` | **yes** |
| `cranelift-jit` | :182 | `jit` | **yes** |
| `cranelift-module` | (:182 blk) | `jit` | **yes** |
| `regex` | :189 | `regex` | **yes** |
| `ctrlc` | :197 | `signals` | **yes** |
| `rusqlite` | (`database`) | `database` | **yes** |
| `postgres` | (:106 blk) | `database-postgres` | no |
| `mysql` | (:110 blk) | `database-mysql` | no |
| `lettre` | (:116 blk) | `mail` | no |

`Cargo.toml:50-55` — `default = ["cryptography", "regex", "signals", "green", "jit", "database",
"unicode", "ini", "csv", "encoding", "json", "uri", "path", "hash", "decimal", "test", "debug",
"session"]`. So **9 crates ship by default**, not 4 — and 5 of those 9 (`unicode-segmentation`,
`cranelift` ×3, `rusqlite`) are not named anywhere in CLAUDE.md.

The spec CLAUDE.md cites as its authority *also* disagrees with it, and *also* is stale:

> `docs/specs/UNIFIED-SPEC.md:873-877` — "**Status: ADOPTED 2026-06-27; AMENDED 2026-07-03 (SQL
> driver + TLS domains) and 2026-07-06 (native codegen / JIT — domain #7).** This policy is why
> "zero external dependencies" claims in older docs are **false and must not be repeated**: Phorj's
> *core stays `std`-only*, but **four vetted, feature-gated crates ship by default**, and three more
> domains are approved."

UNIFIED-SPEC enumerates **seven** admitted domains (`:884-903`: crypto, regex, signals, stackful
coroutines, SQL, TLS, native codegen) — plus the later `unicode-segmentation` / DEC-256 admission
referenced at `:104` ("icu4x DEC-271"). Its own "four … ship by default" sentence predates the
**2026-07-09 jit-as-default ruling** recorded at `CLAUDE.md:25` and `Cargo.toml:44-49`.

**Grade:** [Verified: enumerated every optional dep from `Cargo.toml` with line numbers; read the
`default` list; read UNIFIED-SPEC §"External dependency policy" in full.]
**Severity: P1** — this is the *opening paragraph of the file every session reads first*, and its
own cited authority contradicts it. The irony is direct: UNIFIED-SPEC:875 warns that understated
dependency claims "are **false and must not be repeated**", and CLAUDE.md repeats one.

**Proposed exact replacement for `CLAUDE.md:8-10`:**

> Phorj is a statically-typed, PHP-inspired language implemented in Rust (edition 2021). The **core**
> (lexer, parser, checker, interpreter, VM, transpiler, loader, bundler) is **`std`-only**; every
> external crate is feature-gated and confined to one of the **seven approved domains** of
> `docs/specs/UNIFIED-SPEC.md` §"External dependency policy" (crypto, regex, OS signals, stackful
> coroutines, SQL, TLS, native codegen) plus the DEC-256 Unicode-segmentation admission. **14
> optional crates are declared** (`Cargo.toml` `[dependencies]`), of which **9 ship by default**:
> `argon2`, `regex`, `ctrlc`, `corosensei`, `cranelift`/`cranelift-jit`/`cranelift-module` (`jit` is
> default since 2026-07-09), `rusqlite`, `unicode-segmentation`. The rest (`rustls`, `webpki-roots`,
> `postgres`, `mysql`, `lettre`) are opt-in. **Never state a dependency count without recounting
> `Cargo.toml` first.**

Fix `UNIFIED-SPEC.md:875` in the same change (its "four … by default" is stale post-jit-default), or
CLAUDE.md will just re-drift to it.

---

### H4 — `CLAUDE.md` invariant 10 names a **retired** command as the only network command [P1]

> `CLAUDE.md:114-116` — "10. **Determinism.** `run`/`check`/`transpile` never touch the network
> (`phg vendor` is the only network command); examples use only deterministic inputs…"

vs the binary:

```
$ ./target/release/phg vendor
error: phg vendor is retired (DEC-282): use `phg add <Publisher/Name>`, `phg install`,
`phg update`, or `phg remove <Publisher/Name>` (DEC-316).
```

and the surface docs:

> `FEATURES.md:98` — "Offline `vendor/<Publisher>/<Name>/` dependency resolution (folder = package);
> compiler never touches the network | ✅ | DEC-282; fetching = a future package-manager extension
> (**`phg vendor` retired**)"
> `FEATURES.md:72` — "…offline `vendor/` dependency resolution (DEC-282, **manifest-less** — no
> `phorj.toml`/`[require]`/`phg vendor`…)"

The *actual* network commands are now `phg install` / `phg add` / `phg update` (`src/pm/ops.rs:19`
— "Resolve `phorj.json` → **fetch** + materialize `vendor/` → write `phorj.lock`";
`src/pm/vendor.rs:3`). So invariant 10's parenthetical is wrong on both halves: the named command
does not exist, and the commands that *do* touch the network are unnamed.

**Grade:** [Verified: ran `phg vendor`; read `FEATURES.md:72,98`; grepped `src/pm/`.]
**Severity: P1** — an invariant that a session is meant to enforce cannot be enforced against a
non-existent command, and the real network surface is undocumented in the rule file.
**Correction:** `CLAUDE.md:114` → "`run`/`check`/`transpile`/`build` never touch the network; the
package-manager commands `phg add`/`install`/`update` are the only network-touching commands
(DEC-282/DEC-316 — `phg vendor` is retired)."

---

### H5 — `CLAUDE.md` invariant 14 cites a CLI flag that does not exist [P1]

> `CLAUDE.md:136-137` — "(First application: concurrency — hard error + explicit
> **`--sequential-concurrency`** opt-in with warning.)"

`grep -rn 'sequential.concurrency|sequential_concurrency'` over the whole repo returns **only .md
files** — `CLAUDE.md:137`, `docs/plans/MASTER-PLAN.md:726`, `MASTER-PLAN.md:2382`,
`docs/research/full-audit/raw/C-decisions.md:948`. **Zero hits in `src/`.**

The complete flag set actually parsed by the CLI: `--` `--addr` `--address` `--all` `--check`
`--dap` `--dev` `--docs` `--dump-on-fault` `--help` `--json` `--no-jit` `--php` `--sign`
`--target` `--timeout` `--tree-walker` `--version` `--vs-php` `--workers`. No
`--sequential-concurrency`.

The *other* half of the same ladder rung **is** shipped and correct — `src/transpile/expr.rs:548`
raises `"E-CONCURRENCY-NO-PHP: green-thread concurrency (`spawn` / channels) cannot be transpiled to
PHP"`, and `phg explain E-CONCURRENCY-NO-PHP` resolves. So this is a half-built ladder rung
documented as whole. `MASTER-PLAN.md:2382` compounds it by presenting both halves together:
"hard error (**shipped code**: `E-CONCURRENCY-NO-PHP`) + `--sequential-concurrency` opt-in w/
warning" — the "shipped code" qualifier attaches only to the first half, which reads as covering
both.

**Grade:** [Verified: grepped the whole repo; enumerated the real flag set from `src/main.rs` +
`src/cli/`; confirmed `E-CONCURRENCY-NO-PHP` exists at `src/transpile/expr.rs:548`.]
**Severity: P1** — Invariant 14 is the LADDER RULE, which explicitly requires "Every exclusion is a
**tracked, tested, register-recorded artifact**". Its own first application cites an untracked,
unbuilt, untested flag. That undermines the rule's credibility as a template.
**Correction:** either (a) reword `CLAUDE.md:137` to "(First application: concurrency — hard error
`E-CONCURRENCY-NO-PHP` on transpile + differential quarantine; the ruled `--sequential-concurrency`
opt-in is **QUEUED, not built**)", or (b) build the flag. **This is a design/scope question →
developer's call (Invariant 15); I do not rule on it.** Whichever way, the ruled-but-unbuilt state
must appear in SLICE-STATE/MASTER-PLAN as QUEUED per Invariant 19.

---

### H6 — Four shipped CLI commands are absent from `phg --help` [P1]

`phg --help` lists exactly 16 commands: `run check parse tokenize transpile lift disassemble
benchmark build serve lsp debug test format extensions explain`. It does **not** list
`install`, `add`, `update`, `remove`.

All four dispatch:

```
$ phg install  -> error: no phorj.json in /home/user/phorj: No such file or directory (os error 2)
$ phg add      -> error: usage: phg add <Publisher/Name>[@version] [--git <url> --ref <tag>] [--path <dir>]
$ phg update   -> error: no phorj.json in /home/user/phorj: No such file or directory (os error 2)
$ phg remove   -> error: usage: phg remove <Publisher/Name>
```

They are documented in `examples/package-manager/README.md:54,65-69` and `examples/README.md:234,311`
— but not in the binary's own help, which is the primary discovery surface. `phg vendor`'s own error
message *tells* users about them, so the binary knows they exist.

**Grade:** [Verified: ran `--help` and all four commands; grepped every `.md` for their
documentation.]
**Severity: P1** — undocumented public interface. Breaches CLAUDE.md Rule 6's Docs dimension
("Every changed public interface is documented — show the updated help text") and Invariant 17.
**Correction:** add a `package manager:` block to the `commands:` section of `phg --help` (in
`src/main.rs`/`src/cli/` help text) listing `add`, `install`, `update`, `remove`.

---

### H7 — `CLAUDE.md` invariants 3 and 4 point at **three files that do not exist** [P1]

> `CLAUDE.md:89-91` — "3. **A new `Op` variant extends three exhaustive matches in the same
> commit:** `vm::exec_op` (`src/vm/exec.rs`), `BytecodeProgram::validate` (**`src/chunk.rs`**),
> `compiler::stack_effect` (**`src/compiler/mod.rs`**)."
> `CLAUDE.md:92-93` — "4. **Value kernels are single-sourced** in **`src/value.rs`** (checked
> int/float arithmetic, `compare_ord`, canonical fault strings)."

Filesystem check:

| cited path | exists? | actual location |
|---|---|---|
| `src/vm/exec.rs` | **yes** | `exec_op` at `src/vm/exec.rs:9` ✓ |
| `src/chunk.rs` | **NO** | `BytecodeProgram::validate` is at `src/chunk/validate.rs:21` |
| `src/compiler/mod.rs` | exists, **wrong file** | `stack_effect` is at `src/compiler/emit.rs:75` |
| `src/value.rs` | **NO** | `src/value/` dir; `FAULT_DIV_ZERO` at `src/value/arith.rs:5` |

`docs/INVARIANTS.md` has the *same* defect one generation further back — `:63` cites
`vm::Vm::exec_op` (`src/vm.rs`) and `compiler::Compiler::stack_effect` (`src/compiler.rs`), **both
non-existent**; `:38` cites `src/value.rs`; `:65` cites `src/chunk.rs`.

**Grade:** [Verified: `ls`/`test -e` on all 8 cited paths; `grep -rn` located each symbol's real
file.]
**Severity: P1** — invariants 3, 7, and 8 are labelled **MUST-CHECK**: they are the exact rules a
session is told to verify before touching the `Op` set. A session following the pointer finds
nothing and may conclude the invariant no longer applies. Note the *substance* of invariant 3 still
holds — I verified all three matches are wildcard-free (the only `_ =>` arms in
`src/vm/exec.rs:847,893,986` are inner matches, and `src/chunk/validate.rs:41` mentions `_ => None`
only in a comment explaining it was **removed**). So this is purely a citation-rot bug, which makes
it cheap to fix and inexcusable to leave.
**Correction:** `CLAUDE.md:90` → `src/chunk/validate.rs`; `:91` → `src/compiler/emit.rs`; `:92` →
`src/value/` (kernels in `src/value/arith.rs`). Same three in `docs/INVARIANTS.md:38,63,65`.
Root-cause fix: the 2026-07-16 M-Decomp splits (and today's `25053be` "M-Decomp 13 oversized files")
move files without updating the docs that cite them. **Recommend a CI/pre-push guard that extracts
every backticked `src/...` / `tests/...` / `scripts/...` path from `*.md` and fails if it does not
exist on disk.** That single check would have caught H7, H8, and most of §4.

---

### H8 — `docs/INVARIANTS.md` §1 contains a botched find-and-replace: four corrupted symbol names [P2]

A `runvm` → "the VM leg" substitution was applied to *code identifiers*, producing text that names
nothing:

> `docs/INVARIANTS.md:19` — "runs its **embedded source** through `cli::cmd_the VM leg` at startup"
> `docs/INVARIANTS.md:20` — "**Enforced by** `tests/build.rs::built_binary_matches_the VM leg`."
> `docs/INVARIANTS.md:21` — "The startup hook must keep dispatching through `cmd_the VM leg` (never `cmd_run`)"
> `docs/INVARIANTS.md:27` — "gated by `cross_musl_binary_matches_the VM leg` (native exec)"

Real names: `tests/build.rs:204` `fn built_binary_matches_vm()`, `tests/build.rs:140`
`fn cross_musl_binary_matches_vm()`. There is no `cmd_the VM leg`; `src/cli/pipeline.rs` has
`cmd_run` (:407), `cmd_run_exit` (:439), `cmd_treewalk` (:363).

`:21` is worse than cosmetic — it is **inverted**. It instructs "must keep dispatching through
`cmd_the VM leg` (**never `cmd_run`**)", but `src/main.rs:32` correctly does
`match cli::cmd_run_exit(&src)`. Post-`runvm`-retirement, `cmd_run` *is* the VM path, so the
invariant now forbids the correct implementation. A session "fixing" main.rs to comply would break
the build surface.

Same corruption elsewhere: `docs/ARCHITECTURE.md:89` ("`cmd_run`/`cmd_the VM leg`/`cmd_transpile`"),
`docs/adr/0001-no-shared-run-vm-ir.md:13` (same), `docs/plans/SLICE-STATE.md:2116`,
`docs/plans/MASTER-PLAN.md:1246,1683`, `KNOWN_ISSUES.md:2275`.

Also `docs/INVARIANTS.md:9` — "The tree-walking interpreter (`phg run`) and the bytecode VM
(`phg run`) must produce identical stdout" — both legs labelled `phg run`, making the sentence
self-referential nonsense. Correct: tree-walker is `phg run --tree-walker` (per
`CLAUDE.md:81-82`).

**Grade:** [Verified: grepped `"the VM leg"` repo-wide (9 hits in 6 files); listed the real
`fn` names in `tests/build.rs` and the real `cmd_*` in `src/cli/`; read `src/main.rs:32`.]
**Severity: P2 for the cosmetic hits, P1 for `:21`** (actively instructs the wrong thing).
**Correction:** in all 9 locations, replace "the VM leg" with the correct identifier (`vm` inside a
test name, `cmd_run` / `cmd_run_exit` for the dispatch). Rewrite `:21` to "must keep dispatching
through `cmd_run_exit` (never a source-transforming path)". Fix `:9` to
"`phg run --tree-walker`". **This class of damage is what a mechanical doc-reference check
(see H7) prevents.**

---

### H9 — Invariant 17's transpile↔lift symmetry is broken on phorj's *own* transpiler output [P1]

> `CLAUDE.md:150-153` — "17. **Always-current surfaces** … **transpile AND lift updated in the same
> change** as every language/stdlib feature (a feature that runs but doesn't transpile/lift, or vice
> versa, **is not done**)"
> (restated verbatim at `docs/INVARIANTS.md:203-205`.)

Measured round trip on `p with { y = 9 }` (record-update, a shipped feature):

```
$ phg run /tmp/cw2.phg
1,9                                        # runs ✓

$ phg transpile /tmp/cw2.phg | grep clone
    $b = clone($a, ['y' => 9]);            # transpiles ✓

$ phg transpile /tmp/cw2.phg > cw2.php && phg lift cw2.php
lift parse error: `clone` is Tier-2/Tier-3, found Ident("clone") (line 7)
exit=1                                     # lift REFUSES ✗
```

The lifter cannot read the transpiler's own emission. Both refusal points confirmed in source:
`src/lift/parser/exprs.rs:303-304` (`"clone" | "print" | "yield" | … => Err(self.err(&format!("`{word}` is Tier-2/Tier-3")))`)
and `src/lift/printer/exprs.rs:217-224` (`Expr::Bytes | Expr::Lambda | Expr::CloneWith |
Expr::Inject | Expr::TaggedTemplate | Expr::Html => Err("printer:
bytes/lambda/clone-with/inject/html/tagged-template are outside the lift subset")`).

So **six** constructs are outside the lift subset: bytes, lambdas, `with {}`, inject, tagged
templates, and html — several of which are marked ✅ in FEATURES.md.

**This is the "impossible compliance" class the brief asked me to look for explicitly.** The
lifter is *by design* a tiered translator — `examples/lift/README.md:80` "## What lift refuses
(loudly — **the Tier-2 frontier**)", and `docs/plans/MASTER-PLAN.md:1660` queues
"**W4-7 · Lift Tier-2/3 depth** + playground PHP input (L)". Deepening it is a *future roadmap
item*. Yet Invariant 17 is written unconditionally, with **no tier carve-out** — unlike
Invariant 1, which explicitly discloses "The ONE disclosed exception: concurrency
(see rule 14…)", and unlike Invariant 14, which gives non-mappable features a formal escape
(`E-TRANSPILE-<FEATURE>` + quarantine + disclosure). **The lift direction has no equivalent
escape mechanism.** As literally written, Invariant 17 cannot be satisfied by any feature outside
the Tier-1 lift subset, which means it is either routinely violated in silence or routinely
ignored — both corrosive.

Secondary: `examples/lift/README.md:80-86` enumerates what lift refuses (array type annotations,
key/value `foreach`, backed enums, default params, untyped params, elvis, assignment-as-expression,
non-literal match arm) but **omits `clone`** — `grep -i clone examples/lift/README.md` returns
nothing. So even the lift doc does not disclose the refusal that hits phorj's own output.

**Grade:** [Verified: ran the full run→transpile→lift round trip and captured all three outputs;
read both refusal sites in `src/lift/`; read `examples/lift/README.md:80-86`; grepped MASTER-PLAN
for the queued W4-7 item.]
**Severity: P1** — a delivery invariant that cannot be complied with is worse than no invariant.
**Correction (recommendation only — the substantive choice is the developer's, Invariant 15):**
add an explicit lift-tier carve-out to `CLAUDE.md:150-153` and `docs/INVARIANTS.md:203-205`,
mirroring Invariant 1's disclosure style — e.g. "…**transpile** updated in the same change as every
feature; **lift** updated in the same change *when the construct is inside the Tier-1 lift subset*.
A construct outside it is recorded as a tracked lift-tier exclusion (the lift analogue of
Invariant 14's `E-TRANSPILE-*`) in `examples/lift/README.md` §refuses + the register, and is queued
under W4-7 — never left silently unlifted." Then add `clone`/`with {}` (and bytes, lambdas, inject,
tagged templates, html) to `examples/lift/README.md`'s refusal list. The alternative — build lift
support now — is a scope decision I am forbidden from making.

---

### H10 — `CLAUDE.md`'s pre-commit description omits a gate the hook actually runs [P2]

> `CLAUDE.md:32-34` — "**pre-commit** runs the fast Rust-only tier (`fmt` + `nextest --features
> jit`, EXCLUDING the two heavy sweeps …) — ~12s vs ~126s."

The actual hook runs **four** steps, not two (`scripts/git-hooks/pre-commit`):

```
:30  echo "[pre-commit] cargo fmt --check"                  # documented
:38  echo "[pre-commit] phg format --check examples selftest (.phg canonical form)"
:39  cargo run --quiet -- format --check examples selftest  # NOT documented
:46  cargo nextest run --features jit -E "$_GS_FAST_EXCLUDE" --status-level fail  # documented
:48  cargo test --doc --features jit --quiet                # NOT documented
```

The `.phg` canonical-format gate is explicitly **developer-ruled 2026-07-20** (hook comment
`:33-37`) and is *not* the same thing as `cargo fmt` — it gates the language's own sources. A session
that follows CLAUDE.md's description, runs `cargo fmt --check` + `nextest`, sees green, and commits
will be **blocked by its own hook** for an unformatted `.phg`.

**Grade:** [Verified: read `scripts/git-hooks/pre-commit` in full; compared step-for-step against
`CLAUDE.md:32-38`.]
**Severity: P2** — costs a wasted commit cycle, no correctness risk.
**Correction:** `CLAUDE.md:33` → "(`cargo fmt --check` + **`phg format --check examples selftest`**
(the `.phg` canonical-form gate, developer-ruled 2026-07-20) + `nextest` + `cargo test --doc`,
EXCLUDING the two heavy sweeps)".

---

### H11 — `docs/INVARIANTS.md:114` says pre-commit runs clippy; the hook moved clippy to pre-push [P2]

> `docs/INVARIANTS.md:114` — "The tracked `scripts/git-hooks/pre-commit` runs `fmt --check` +
> **`clippy -Dwarnings`** + `test`."

vs the hook's own header:

> `scripts/git-hooks/pre-commit:17` — "**`clippy` also moved to PRE-PUSH** (a lint is zero
> correctness-risk and batches cleanly). See scripts/git-hooks/pre-push."

and `CLAUDE.md:35-36`, which agrees with the hook ("**pre-push** runs the FULL suite … + `clippy`").
There is no `clippy` invocation anywhere in the pre-commit script.

**Grade:** [Verified: read the hook end-to-end — zero `clippy` calls; read all three doc sides.]
**Severity: P2.** **Correction:** `docs/INVARIANTS.md:114` → "…runs `fmt --check` + the `.phg`
format gate + the fast test tier; `clippy` + the two heavy sweeps + the PHP oracle run at
**pre-push** (2026-07-08 speed split)."

Bonus, same hook: `scripts/git-hooks/pre-commit:18` says "`--features jit` gates the JIT backend
(**not a default feature**)", contradicting `CLAUDE.md:25` ("**`jit` is a DEFAULT feature**
(developer-ruled 2026-07-09)") and `Cargo.toml:50-55`. Stale hook comment, P3.

---

### H12 — Three mutually inconsistent definitions of "green" [P2]

Three files each define the quality gate, and no two agree:

> **A** `CLAUDE.md:24-25` — "**Green means ALL of:** `cargo test --workspace` + `cargo clippy
> --all-targets` + `cargo fmt --check` + `cargo build --release`, clean."
> **B** `CLAUDE.md:39-45` — "**Full correctness gate — ALL-FEATURES (developer-ruled 2026-07-16)** …
> `PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features` + `cargo clippy --all-targets
> --all-features` + `cargo clippy --all-targets --no-default-features` + … **`--all-features` is
> mandatory**"
> **C** `docs/INVARIANTS.md:115-116` — "**Green means:** `cargo test` + `cargo clippy
> --all-targets` + `cargo fmt --check` + `cargo build --release`, all clean."
> **D** `CONTRIBUTING.md:26-29` — "`cargo test` / `cargo clippy --all-targets` / `cargo fmt
> --check`" (no `build --release` at all)

B's justification is explicit and evidence-backed: "the non-default features (`http-client`, `mail`,
`database-postgres`, `database-mysql`) are otherwise NEVER compiled/linted/tested by the gate — the
`--features jit`-only gate **hid real clippy lints** in those files (DEC-264 build)." So B is the
ruled definition, and A, C, D are all the exact gate B was ruled to replace — sitting 15 lines
above B in the same file (A), and in two other files.

This matters operationally: `CLAUDE.md:70` grants autonomous commit authority "when the quality gate
**above** is green". "Above" is ambiguous between A and B, and A is the weaker, superseded one — so
the ambiguity resolves in the unsafe direction.

**Grade:** [Verified: read all four definitions; read B's DEC-264 rationale.]
**Severity: P2** (P1 for the commit-authority ambiguity).
**Correction:** delete definition A (`CLAUDE.md:24-25`) or reduce it to "the fast local loop — NOT
the gate; see *Full correctness gate* below". Replace `docs/INVARIANTS.md:115-116` and
`CONTRIBUTING.md:26-29` with a pointer to the single definition rather than a restated copy — the
same anti-duplication discipline Invariant 19 mandates for plans. Make `CLAUDE.md:70` cite the gate
by name ("when the **Full correctness gate** is green").

---

### H13 — `#![forbid(unsafe_code)]` vs `#![deny(unsafe_code)]` — three docs claim `forbid` [P3]

Actual: `src/lib.rs:10` and `src/main.rs:5` both `#![deny(unsafe_code)]`. `docs/INVARIANTS.md:110`
and `CLAUDE.md:30` correctly explain the relaxation ("relaxed from `forbid` for the JIT: its
finalize→transmute→fn-ptr `unsafe` is the sole first-party island").

Stale `forbid` claims:
> `CONTRIBUTING.md:36` — "There is no `unsafe` in this crate — **`#![forbid(unsafe_code)]`** is set
> crate-wide and must stay."
> `Cargo.toml:141-142` (corosensei rationale) — "which this crate confines outside phorj's
> **`#![forbid(unsafe_code)]`**"
> `docs/plans/MASTER-PLAN.md:1246` — "both blocked by **`forbid(unsafe_code)`** + no-new-deps"

`CONTRIBUTING.md:36` is the worst: it is the contributor-facing statement, it asserts "There is no
`unsafe` in this crate" (false — `src/jit/` has an audited island), and it says the attribute "must
stay" (it was deliberately changed).

**Grade:** [Verified: `grep -n 'deny(unsafe_code)\|forbid(unsafe_code)' src/lib.rs src/main.rs`
returns `deny` for both; read all four doc sides.]
**Severity: P3** for Cargo.toml/MASTER-PLAN (comments), **P2** for `CONTRIBUTING.md:36` (public,
and factually asserts no `unsafe` exists).
**Correction:** `CONTRIBUTING.md:36` → "`#![deny(unsafe_code)]` is set on both crate roots. The
JIT's audited `unsafe` (`src/jit/`, behind the CI `unsafe-island` gate) is the sole first-party
island; no other `unsafe` is admitted."

---

### H14 — `CONTRIBUTING.md` repeats the four-dep claim and understates the gate [P2]

> `CONTRIBUTING.md:15` — "`cargo build`  # cargo fetches **the four vetted deps** (argon2, regex,
> ctrlc, corosensei)"

Same defect as H3, in the contributor-facing file. A contributor running `cargo build` today fetches
9 crates (H3's table), including the three Cranelift crates — a materially different build (size,
time, license surface). `THIRD-PARTY-NOTICES.md` should be cross-checked against the real 14 in the
same pass.

`CONTRIBUTING.md:26-29`'s gate additionally omits `cargo build --release`, `--workspace`, and
`--all-features` (see H12).

**Grade:** [Verified: read `CONTRIBUTING.md:1-50`; cross-referenced `Cargo.toml` default list.]
**Severity: P2.** **Correction:** `:15` → "# fetches the default-feature dependency set (9 crates —
see `Cargo.toml` `[dependencies]` and `docs/specs/UNIFIED-SPEC.md` §External dependency policy)".
Point `:26-29` at the single gate definition per H12.

---

### H15 — `UNIFIED-SPEC.md:162` retires the `->` return form; the parser still accepts it [P2]

> `docs/specs/UNIFIED-SPEC.md:162` — "| Return annotation | `-> float` in samples | **superseded**:
> canonical `: T`; **`->` retired** (W2-4, parser-reject pending) |"

Verified accepted — a program using `function main() -> void` runs to completion:

```
$ phg run /tmp/t1.phg      # function main() -> void { … }
arrow-return-still-parses
exit=0
```

Same under `--tree-walker`. So `->` is *live syntax*, not retired.

Two mitigating facts I must state for fairness: (a) the cell **does** say "parser-reject pending",
so the doc is not wholly dishonest — but "**retired**" in bold and "pending" in the same cell are
contradictory, and a reader scanning the bolded verdict gets the wrong answer; (b) the residual
corpus is **comments, not code** — of 81 `) -> ` occurrences across 34 `.phg` files, the ones I
sampled in `examples/` are all prose (`examples/web/server.phg:5` "calls ONE entry per request —
`respond(bytes) -> bytes`"; `examples/web/README.md:53` same). I found **no** `.phg` file using
`->` as a live return annotation outside my own test file. So the corpus is cleaner than the raw
count suggests; the defect is the doc's verdict word plus the un-built parser rejection.

**Grade:** [Verified: ran an `->` program on both backends to exit 0; counted 81 occurrences /
34 files and hand-inspected the `examples/` hits to confirm they are comments.]
**Severity: P2.** **Correction:** change the cell's verdict from "**retired**" to "**ruled retired;
parser rejection QUEUED (W2-4) — `->` still parses today**", and record the W2-4 parser-reject as a
QUEUED item in SLICE-STATE per Invariant 19. Also sweep the ~81 comment occurrences to `: T` so the
corpus stops teaching the retired form.

---

## 2. Invariant-19 SSOT divergence

### H16 — `README.md`'s status table is a fourth roadmap surface, and it contradicts MILESTONES [P1]

`ROADMAP.md` was deliberately stripped of per-item status — and says so:

> `ROADMAP.md:3-5` — "It intentionally carries **no per-item status** — that lives in the single
> sources of truth below, so nothing here can drift out of date (**this file previously accreted
> stale milestone markers**; those now live only where they stay current)"

`README.md` did not get the same treatment and has drifted exactly as predicted:

> `README.md:49-53` —
> "| **M2.5** | `phg build` → standalone native executables | 🔨 in progress … |
>  | **M3+** | Language enrichment, ecosystem, tooling | **🔲 planned** — see ROADMAP.md & VISION.md |"

vs

> `docs/MILESTONES.md:117` — "## M3 — Language enrichment — **🔨 IN PROGRESS**"

Direct contradiction: `🔲 planned` vs `🔨 IN PROGRESS`. And "M3+" is far past planned — MILESTONES
lists M5, M6, M7 complete or core-complete (`:263`, `:274`, `:299`), the register is at DEC-338,
and `Cargo.toml:7` is `version = "1.0.0-nightly.0"`. A newcomer reading README concludes the project
is at M2.5 with everything after it unstarted.

**Grade:** [Verified: read `README.md:47-56`, `ROADMAP.md:1-16`, and every `## M*` heading in
`docs/MILESTONES.md`; read `Cargo.toml:7`.]
**Severity: P1** — highest-traffic doc, and a textbook Invariant-19 "no divergent artifact"
violation: milestone status now lives in README **and** MILESTONES with different content.
**Correction:** apply the ROADMAP.md remedy to `README.md:45-56` — delete the per-milestone table
and replace with a two-line pointer to `docs/MILESTONES.md` (delivered) and
`docs/plans/MASTER-PLAN.md` (forward). Keep only stable prose ("pre-1.0, single-developer").

---

### H17 — `docs/GA-CHECKLIST.md` is a **third competing percentage SSOT**, and self-admittedly stale [P1]

`CLAUDE.md:184-185` names exactly one parity-percentage home:

> "the parity % model in `docs/research/full-audit/raw/M-gap-matrix.md` §4 (recompute at every
> milestone close)"

But `docs/GA-CHECKLIST.md` claims the same authority for itself, in stronger terms:

> `docs/GA-CHECKLIST.md:4-5` — "This file is **the real denominator**: GA% is computed from the
> weighted table below, not estimated. Update the per-rock status as work lands; recompute the
> total. **Supersedes any vibe-% in chat.**"
> `:21` — "| | **GA total** | 100% | | **≈ 57%** | |"
> `:23` — "**Honest GA ≈ 57%**"

Meanwhile the most recent SSOT reconciliation commit (`44ffe21`, today) reads: *"mark stale
§11/finishing-wave % SUPERSEDED by §4.11 (**68/69**)"*. So the repo simultaneously carries ≈57%
(GA-CHECKLIST) and 68/69% (M-gap-matrix §4.11) with **no cross-reference between them**.
GA-CHECKLIST is not mentioned in `CLAUDE.md` "Where things live" at all — it is an orphan authority.

The file also **admits its own staleness twice** and has not been touched since:

> `:16` — "| 2 | **Daily-use tooling** | 20% | **70%** *(stale — see 2026-07-03 log line)* |"
> `:50-52` — "**2026-07-03: correction (unification audit B3-5)** — rock 2's "Missing: an LSP"
> premise was stale … Rock 2's 70% (and therefore **the ≈57% total) is a stale lower bound pending a
> re-score**; no new number invented here."

That is **22 days** of known-stale headline. And the deeper problem is that its *blockers* are
delivered — see H18.

**Grade:** [Verified: read GA-CHECKLIST in full; read `CLAUDE.md:184-185`; read commit `44ffe21`'s
message.] The exact §11 / §4.11 reconciliation state is [Unverified by me] — delegated to the
parallel SSOT agent; see §6.
**Severity: P1** — a direct Invariant-19 violation ("every roadmap item … lives in exactly ONE
canonical place"), aggravated by the file claiming supersession authority it was never granted.
**Correction:** the developer must pick one home (Invariant 15 — I do not rule). Two coherent
options: (a) **fold** GA-CHECKLIST's weighted-rock model into `M-gap-matrix.md` §4 and reduce
GA-CHECKLIST to a pointer stub; or (b) keep it as *the GA-shippability* denominator (a genuinely
different question from *PHP-parity %*) but then (i) add it to `CLAUDE.md` "Where things live",
(ii) state explicitly at `:4` that it measures shippability **not** parity and that parity % lives
in M-gap-matrix §4, and (iii) re-score it (H18). Either way the un-cross-referenced 57%-vs-69%
coexistence must end.

---

### H18 — GA-CHECKLIST's stated GA blockers are **already delivered** — the score is holding us back for no reason [P1] *(class 5)*

Rock 3 is the single largest claimed gap (20% weight at 15% done = **17 points of headroom**, and
`:36` calls it "Literally what '1.0' means"). Its stated blockers:

> `docs/GA-CHECKLIST.md:17` — "| 3 | **Stability & conformance** | 20% | **15%** | 3.0 | **Missing:**
> frozen language surface, a **conformance test corpus** asserting the spec, a written **semver/BC +
> deprecation policy**. Bar: surface frozen + conformance suite green + BC policy published. |"

All three exist on disk today:

| stated as Missing | actual |
|---|---|
| "a **conformance test corpus** asserting the spec" | `conformance/` — **64 `.phg` files** across 8 domains (`collections ddd diagnostics errors lang stdlib types web`) + `conformance/README.md`, plus `tests/conformance.rs` |
| "a written **semver/BC** … policy" | `SEMVER.md` (58 L), `STABILITY.md` (96 L — three tiers, per-construct stable/experimental lists) |
| "… + **deprecation policy**" | `docs/DEPRECATION.md` (43 L — full lifecycle, `W-DEPRECATED` wired at `native::deprecation_of`, `phg explain W-DEPRECATED` resolves) |

I confirmed the mechanism is real, not just prose: `phg explain W-DEPRECATED` →
"W-DEPRECATED — a deprecated stdlib symbol is used (lint)." And `STABILITY.md:8` already
cross-references the corpus as a live gate: "Every stable construct is exercised by the
[conformance corpus](conformance/), so a regression … **fails CI**."

Rock 2's remaining premise is stale too: `:16` lists "`interp/VM/check/transpile/build/benchmark/
disassemble/explain/lift/**vendor**/serve` ✓" — `vendor` is retired (H4), and the four shipped PM
commands (H6) are uncounted.

Only rock 5 (Documentation, 40%) survives scrutiny: I searched and found **no** language reference,
tutorial, stdlib reference, or PHP-migration guide (`docs/*.md` = ARCHITECTURE, DEPRECATION,
EXTENSIONS, EXTENSIONS-AUTHORING, GA-CHECKLIST, HISTORY, INVARIANTS, MILESTONES only). So rock 5's
number is [Inferred: plausible — its four named gaps genuinely do not exist on disk].

**Grade:** [Verified: `ls conformance/` + `find conformance -name '*.phg' | wc -l` = 64; `wc -l` on
all three policy docs; ran `phg explain W-DEPRECATED`; read `STABILITY.md:1-25`; `find` for
reference/tutorial/migration docs returned nothing.]
**Severity: P1 — this is the clearest "holding us back without a justified reason" finding in the
audit.** Rock 3's 15% is anchored to three deliverables that shipped. Whatever the re-scored number
is, the *published* GA figure is understated by a margin large enough to distort roadmap priority:
`:32-37` orders the critical path around rock 3 as an open 17-point gap, which may no longer be
where the leverage is.
**Correction:** re-score rock 3 against evidence (surface-frozen is the one bar plausibly still
open; corpus + BC policy + deprecation policy are delivered), re-score rock 2 (drop `vendor`, add
`add`/`install`/`update`/`remove`), recompute the total, and append a dated burn-down line — the
file's own closing instruction (`:53`) requires exactly this. **The re-scored numbers are the
developer's call (Invariant 15); I am reporting that the current inputs are factually stale, not
what the output should be.**

---

## 3. Internal contradictions inside one document

### H19 — see H12 (two conflicting "green" definitions 15 lines apart inside `CLAUDE.md`) [P2]

`CLAUDE.md:24-25` (definition A) and `CLAUDE.md:39-45` (definition B, the ruled one) sit in the same
section under the same heading, and B was explicitly ruled to *replace* A's `--features jit`-only
shape ("the `--features jit`-only gate hid real clippy lints", `:45`). Recorded here as a
class-3 instance; correction under H12.

### H20 — `UNIFIED-SPEC.md`'s dependency policy contradicts its own domain enumeration [P2]

> `docs/specs/UNIFIED-SPEC.md:875-876` — "Phorj's *core stays `std`-only*, but **four** vetted,
> feature-gated crates ship **by default**, and three more domains are approved."

Its own list immediately below (`:884-903`) enumerates **seven** domains — crypto, regex, OS
signals, stackful coroutines, embedded SQL + drivers, TLS, native codegen — and `:104` records a
further Unicode admission ("*(2026-06-27, amended 2026-07-03, icu4x DEC-271)*"). Of those, **SQL
(`rusqlite`) and native codegen (`cranelift` ×3) and Unicode (`unicode-segmentation`) are all
default-on** per `Cargo.toml:50-55`, so "four ship by default" is wrong by five crates. The
`:873-874` status line even dates the JIT amendment to 2026-07-06 — three days *before* the
2026-07-09 jit-as-default ruling that broke the count, and the sentence was never revisited.

**Grade:** [Verified: read `:871-915` in full; cross-checked against `Cargo.toml:50-55` and the
optional-dep list of H3.]
**Severity: P2** (P1 by blast radius — it is the upstream source CLAUDE.md's H3 error copies from).
**Correction:** `:875-876` → "…but **nine** vetted, feature-gated crates ship **by default**
(`argon2`, `regex`, `ctrlc`, `corosensei`, `cranelift`/`-jit`/`-module`, `rusqlite`,
`unicode-segmentation`) across the **seven** approved domains below, and five more are opt-in
(`rustls`, `webpki-roots`, `postgres`, `mysql`, `lettre`)." **Fix this in the same change as H3** —
otherwise CLAUDE.md re-drifts to its cited authority.

### H21 — Invariant 12's naming-SSOT attribution [P2 — partially delegated]

`CLAUDE.md:119-122` attributes six specific rules to `docs/specs/UNIFIED-SPEC.md`
§"Naming overhaul": PascalCase packages/types/type-params, `Core.` reserved, camelCase
functions/natives, keyword `function` (never `fn`), return types `: T`, mandatory `new`, explicit
`this.field`. A prior agent reported the section contains **no package↔folder rule** despite
Invariant 12 pointing there.

I can corroborate one half independently: the package↔folder casing rule **does** exist in the
repo, but in `FEATURES.md`, not the cited SSOT —

> `FEATURES.md:74` — "| Identifier casing (**enforced**) | ✅ | camelCase functions/methods/params/
> vars (`E-NAME-CASE`), PascalCase classes/enums/variants/type aliases (`E-TYPE-CASE`), **PascalCase
> package/folder + import segments + `as` aliases (`E-PKG-CASE`, 1:1 to PHP namespaces)**;
> front-end-only — never affects the generated PHP |"

and the diagnostic is live: `phg explain E-PKG-CASE` → "E-PKG-CASE — a package or import segment is
not PascalCase." So the rule is real and enforced; the question is only whether the *declared SSOT*
contains it. **Grade: [Verified: the rule exists and is enforced — read `FEATURES.md:74`, ran `phg
explain E-PKG-CASE`. Whether §"Naming overhaul" itself omits it is [Unverified by me] — delegated;
see §6.]**
**Severity: P2** if confirmed — Invariant 12 declares a single naming SSOT, so a rule living only in
FEATURES.md means the SSOT is not one.
**Correction (if confirmed):** add the package↔folder / `E-PKG-CASE` rule to UNIFIED-SPEC
§"Naming overhaul" and have `FEATURES.md:74` point to it rather than restate it.

---

## 4. Stale / orphaned docs

### H22 — `ADR-0005` is still `Status: Accepted` but its decision has been **reversed** [P1]

> `docs/adr/0005-offline-only-vendor.md:3` — "- **Status:** Accepted (2026-06-19)"
> `:18-22` — "**`phg vendor` is the only network-touching command**: clone → checkout the pinned rev
> → copy the dependency's source into `vendor/<vendor>/<package>/` → content-hash → write
> `phorj.lock`. … A required dependency that isn't vendored is a hard error (**`E-VENDOR-MISSING`**)."

Every load-bearing element of that paragraph is now false:

| ADR-0005 claim | reality |
|---|---|
| "`phg vendor` is the only network-touching command" | `phg vendor` → "**is retired** (DEC-282) … use `phg add`, `phg install`, `phg update`, `phg remove` (DEC-316)" |
| `E-VENDOR-MISSING` | `grep -rn 'E-VENDOR-MISSING' src/` → **zero hits** |
| the `vendor` command's workflow | now `phg install` (`src/pm/ops.rs:19`) |
| `vendor/<vendor>/<package>/` path shape | now `vendor/<Publisher>/<Name>/` (`FEATURES.md:98`, PascalCase per `E-PKG-CASE`) |
| implied `phorj.toml [require]` manifest | `FEATURES.md:72` — "DEC-282, **manifest-less** — no `phorj.toml`/`[require]`"; the manifest is now `phorj.json` (`src/pm/ops.rs:19`, `src/loader/entry.rs:16`) |

And the ADR index still presents it as live, with a link to a spec that no longer exists:

> `docs/adr/README.md:25` — "| [0005](0005-offline-only-vendor.md) | Vendoring is offline-only —
> determinism over convenience | m5-project-model-design M5-10 |"

This breaks the ADR system's **own** stated protocol:

> `docs/adr/README.md:9-10` — "**immutable once Accepted** (a decision is changed by adding a new
> ADR that *supersedes* it, never by editing the old one)"
> `docs/adr/README.md:31-33` — "To reverse a decision, add a new ADR with `Status: Accepted` that
> notes "Supersedes ADR-NNNN", and **set the old one's status to `Superseded by ADR-MMMM`** (the
> only edit ever made to an accepted ADR)."

DEC-282 and DEC-316 reversed ADR-0005's decision. No ADR-0006 exists (`ls docs/adr/` → 0001–0005 +
README). So the reversal was recorded in the DEC register and never propagated to the ADR layer that
`docs/adr/README.md:8` declares "**canonical for the decision + its consequences**".

**Grade:** [Verified: read ADR-0005 and adr/README in full; ran `phg vendor`; grepped
`E-VENDOR-MISSING` (zero) and `phorj.json`/`phorj.lock` in `src/pm/`; read `FEATURES.md:72,98`;
`ls docs/adr/`.]
**Severity: P1** — the ADR layer claims canonical authority for load-bearing decisions and is
serving a reversed one as Accepted. This is the most structurally serious stale-doc finding: it is
not drift inside a living doc, it is a *canonical record* that is wrong.
**Correction:** author **ADR-0006** ("Manifest-less `phorj.json` package management supersedes
offline-only vendoring — DEC-282/DEC-316") capturing the new decision + consequences; set
`docs/adr/0005-offline-only-vendor.md:3` to `Status: Superseded by ADR-0006 (2026-07-xx)`; add the
0006 row to `docs/adr/README.md:25`'s index. **Also audit ADR-0001–0004 the same way** — ADR-0001 is
already known to carry the H8 "the VM leg" corruption at `:13`, which means it has been
mechanically edited despite the immutability rule.

### H23 — `FEATURES.md:98` calls the package manager a "future" extension; it shipped [P2]

> `FEATURES.md:98` — "DEC-282; **fetching = a future package-manager extension** (`phg vendor` retired)"

vs `phg add`/`install`/`update`/`remove` all dispatching today (H6), `src/pm/{ops,resolve,vendor}.rs`
present, and `examples/package-manager/README.md` shipping a runnable demo whose `:54` reads
"`$ phg install   # resolve require -> fetch -> vendor/ -> write phorj.lock`".

**Grade:** [Verified: ran all four commands; `ls src/pm/`; read `examples/package-manager/README.md`.]
**Severity: P2.** **Correction:** `FEATURES.md:98` → "fetching ships as the package manager (`phg
add`/`install`/`update`/`remove`, DEC-316); `run`/`check`/`transpile`/`build` still never touch the
network." Add a FEATURES row for the PM itself — it is a shipped surface with no row.

### H24 — `docs/MILESTONES.md` presents the retired `runvm` verb in the present tense [P2]

`CLAUDE.md:81-82` is emphatic: "(**there is NO `runvm` command** — the VM is `run`'s default engine,
the tree-walker its `--tree-walker` oracle)". MILESTONES uses it as live syntax:

> `docs/MILESTONES.md:47-48` — "+ **`phg runvm`** (`src/cli.rs`) + the **differential harness**
> (`tests/differential.rs`): **`runvm` stdout is byte-identical to `run`**"
> `:57` — "**`runvm` now covers the full M1 surface**"
> `:70-71` — "every `examples/*.phg` … **produce identical stdout under `phg runvm` and `phg run`**,
> gated by `tests/differential.rs`"

`:70-71` sits under a "### Success criteria — met" heading in present tense, so it reads as a live
statement of how the gate works. (The earlier plans-divergence audit flagged the same verb at
`:226`; that instance is now fixed — these are additional, unflagged ones.)

MILESTONES also cites the pre-M-Decomp file layout throughout (`:18` `src/{lexer,parser,checker,
interpreter}.rs`, `:20` `src/transpile.rs`, `:38` `src/chunk.rs`/`src/vm.rs`, `:41` `src/compiler.rs`)
— all now directories. In a *historical* ledger that is defensible; in the present-tense success
criteria it is not.

**Grade:** [Verified: read `docs/MILESTONES.md:1-80` and all `## M*` headings; read `CLAUDE.md:81-82`;
confirmed `src/vm.rs`/`src/compiler.rs`/`src/chunk.rs`/`src/value.rs` do not exist.]
**Severity: P2.** **Correction:** MILESTONES already has the right pattern for this at `:7-13` — a
blockquote explaining that historical links are dead-by-design. Extend it one sentence: "Commands
and file paths in per-milestone sections are **as-of-that-milestone** (`phg runvm` was retired —
today's equivalent is `phg run` / `phg run --tree-walker`; single-file modules named here are now
directories)." Then fix `:70-71` specifically, since it is phrased as a live criterion.

### H25 — `examples/lift/README.md`'s refusal list omits `clone` [P2]

`examples/lift/README.md:80` — "## What lift refuses (loudly — the Tier-2 frontier)" — lists eight
refusals (`:82-85`: array type annotation, key/value `foreach`, backed enums and enum methods,
default parameter values, untyped parameters, elvis `?:`, assignment-as-sub-expression, non-literal
match arm) and promises "**Each is a clear `lift …` message naming what to do by hand**".

`grep -i clone examples/lift/README.md` → **nothing**. Yet `clone` is the refusal a user is *most*
likely to hit, because phorj's own transpiler emits it (H9). Also absent: bytes, lambdas, `inject`,
tagged templates, html — the other five `src/lift/printer/exprs.rs:217-224` rejections.

**Grade:** [Verified: read `:78-92`; grepped for `clone`; read the six-variant reject arm in source.]
**Severity: P2.** **Correction:** add `clone` (PHP `clone` / phorj `with {}`), bytes, lambdas/arrow
fns, `inject`, tagged templates, and html to the refusal list, and note explicitly that a
transpiled-phorj PHP file containing `clone(...)` is not currently round-trippable.

### H26 — Dangling `src/` path references [P1 — see H7/H8; scope partially delegated]

Confirmed dangling by direct `test -e`: `src/vm.rs`, `src/compiler.rs`, `src/chunk.rs`,
`src/value.rs` — cited from `CLAUDE.md:90,92`, `docs/INVARIANTS.md:38,63,65`,
`docs/MILESTONES.md:38,41`. Confirmed existing: `src/vm/exec.rs`, `src/compiler/mod.rs`,
`src/limits.rs`, `src/checker/common.rs`. The exhaustive repo-wide markdown-reference sweep was
delegated; see §6.

---

## 5. "Holding us back" blockers

### H27 — H18 is the headline instance: GA is scored against three delivered blockers [P1]
See H18. Recorded here as the class-5 primary.

### H28 — Invariant 17 is unsatisfiable as written [P1]
See H9. This is the "two rules that cannot both be satisfied" case the brief asked me to hunt
explicitly: Invariant 17 demands lift parity for *every* feature; the lifter is a documented tiered
subset whose deepening is queued as MASTER-PLAN **W4-7**. Compliance is impossible today, and unlike
Invariant 14 there is no formal exclusion mechanism for the lift direction. Recorded here as the
class-5 secondary.

### H29 — W5-13's stated blocker still holds (a *correctly* justified deferral) [P3 — no action]

I checked this one expecting a stale reason and found a sound one, so I record it as a
counter-example that the deferral discipline does work when applied:

> `docs/INVARIANTS.md:94-96` — "Pinned by the `#[ignore]`d
> `interpolation_fault_line_matches_between_backends` gate in `tests/differential.rs`; the fix needs
> VM debug symbols (scope IP ranges) and is scheduled **W5-13**."
> `tests/differential.rs:260` — `#[ignore = "W5-13: VM reports line 1 for faults inside string
> interpolation (H §5); un-ignore when VM debug symbols land"]`
> `tests/differential.rs:258` — "is `#[ignore]`d; **un-ignore it when W5-13 lands** and it must go
> green."

Doc, test attribute, and inline comment all agree; the stated blocker (VM debug symbols) is a real
unbuilt capability; the pin is a live test, not prose. **Grade: [Verified: read all three sides.]**
**No action.** This is the pattern H2 and H9 should follow.

---

## 6. Reconciliation with today's three audits (coordinator addendum)

### H30 — `2026-07-25-plans-divergence-audit.md`: **all nine findings are now FIXED** [informational]

I re-verified each rather than re-reporting. Per-finding current status:

| # | Finding | Status now | Evidence |
|---|---|---|---|
| **H1** | Q-A/Q-B cluster missing from the register | **FIXED** | `C-decisions.md:3241-3256` now carries the cluster — ":3241 Canonical detail lives in the two frozen specs…; recorded here per Inv 19"; ":3254 **Q-B — visibility model completeness (RULED DV-1..DV-5; DV-1/2/3 + follow-up DONE+certified…)**". Max DEC advanced 335 → **338**. |
| **H2** | SLICE-STATE "Pushed" cursor stale | **FIXED** | now `SLICE-STATE.md:97` — "**Pushed:** `origin/master @ dee608e` — all of Q-A + Q-B (DV-1/2/3 + ctor-promoted-param…". Tree clean, `HEAD == origin/master == 25053be`. *(Note: the cursor now names `dee608e` while HEAD is `25053be` — 4 commits have landed since. See H31.)* |
| **H3** | MASTER-PLAN:148 listed a DONE follow-up as open | **FIXED** | `MASTER-PLAN.md:147-150` now reads "**✅ DONE 2026-07-25:** DV-1+DV-2 … DV-3 … `internal` on ctor-promoted params **✅ DONE+certified**". |
| **M1** | MILESTONES "Visibility modifiers" stale vs Q-B | **FIXED** | `MILESTONES.md:230` now appends "**Q-B update (✅ 2026-07-25, DEC-268-certified — `docs/specs/2026-07-24-visibility-model.md`)**". |
| **M2** | FEATURES.md had no Q-B row | **FIXED** | `FEATURES.md:95` — "| Visibility model completeness (Q-B) | ✅ | package HIERARCHY … member `internal` added …". |
| **M3** | CHANGELOG missing Q-A/Q-B/LSP fix | **FIXED** | `CHANGELOG.md:39` "### Added — Q-A wildcard & group imports (2026-07-25, DEC-268-certified)"; `:46` "### Added — Q-B visibility model completeness". |
| **M4** | stale "← CURRENT: S3.1 in flight" marker | **FIXED** | `grep '← CURRENT'` → **zero hits**; `SLICE-STATE.md:173` now reads "(S3.1 is DONE; **this note's original "S3.1 in flight" was stale**.)" |
| **L1** | wildcard spec step-7 example path wrong | **FIXED** | `2026-07-24-wildcard-imports.md:131` now "shipped as the project `examples/project/wildcard-imports/`". |
| **L2** | visibility spec DV-3 carve-out contradicted its follow-up | **FIXED** | `2026-07-24-visibility-model.md:124-126` now "**v1 carve-out (SUPERSEDED 2026-07-25 — see the ctor-promoted-param follow-up below, now DONE; `E-INTERNAL-PROMOTION` was removed)**". |

**Grade:** [Verified: nine targeted greps/`sed -n` reads, one per finding, each quoted above.]
**Assessment: the SSOT-repair loop demonstrably works.** Nine doc findings raised and closed within
one day is strong evidence the Invariant-19 machinery is healthy for *tonight's* work. Every finding
in §1–§5 of this report is older drift the nightly loop does not reach — which is the actionable
insight: the loop audits **the current slice**, not the standing corpus.

### H31 — The prior audit's own H2 fix is already one generation stale [P3]

`SLICE-STATE.md:97` says `Pushed: origin/master @ dee608e`, but `git rev-parse --short HEAD` =
`origin/master` = **`25053be`**, four commits later (`44ffe21`, `cbfdc1c`, `179474e`, `50da104`,
`25053be`). Not a divergence in substance (tree is clean, nothing unpushed), but the cursor names a
stale SHA — the exact defect the prior audit's H2 fixed, recurring within hours. **Grade: [Verified:
`git rev-parse --short HEAD` and `origin/master` both `25053be`; `git status -sb` → `##
master...origin/master` with no ahead/behind; read `SLICE-STATE.md:97`.] Severity: P3.**
**Correction:** structural, not textual — a hardcoded SHA in a living cursor file will always rot.
Recommend the Pushed line state a *condition* ("tree clean, nothing unpushed as of <date>") rather
than a SHA, or be regenerated by the pre-push hook.

### H32 — `2026-07-25-currency-audit.md`'s "No GAPs" headline overreaches its own scope [P2]

**Verdict: the scope legitimately excluded the two counter-examples; the HEADLINE does not carry the
scope qualifier.** Both halves of that verdict matter.

*Scope is explicitly narrow* — stated twice:
> `:4-5` — "Verifies that **the features shipped tonight** are reflected across ALL surfaces —
> lifter (PHP→Phorj), transpiler (Phorj→PHP), formatter, and LSP"
> and the body has exactly three sections: `:16` "Feature 1 — Q-A wildcard/group imports",
> `:32` "Feature 2 — Q-B visibility: member `internal`", `:46` "Feature 3 — Top-level `internal`…".

Neither `with {}`/`CloneWith` nor DB/PDO is one of tonight's three features, so **they were
correctly out of scope**. The audit did not miss them; it never claimed them.

*The headline does not say so:*
> `:10` — "**RESULT: No GAPs. Every surface is UP-TO-DATE or N-A with a sound, stated reason.**"

Read alone — and it is the bolded, standalone headline — "**Every** surface is UP-TO-DATE" asserts a
repo-wide Invariant-17 clean bill. That is falsified by H9 (`with {}` runs + transpiles, lift
refuses the transpiler's own output). Anyone citing `:10` as "Invariant 17 is clean" would be wrong,
and the audit gives them no scope guard at the point of citation.

On the second counter-example I must **correct the premise**: *"no PDO lifting at all"* is **not** an
Invariant-17 gap. `grep -rln 'PDO' src/transpile/` returns **nothing** either — DB does not
transpile at all; `src/cli/explain/transpile_di.rs:88-89` confirms "**E-TRANSPILE-DB** — a program
importing `Core.DatabaseModule` cannot be transpiled to PHP", and `FEATURES.md` lists DB as
native-only. So transpile-absent + lift-absent is **symmetric**, and symmetric absence is exactly
what Invariant 14's LADDER rung 2 prescribes. **No finding.** (Contrast `with {}`: transpile
present, lift absent — asymmetric, hence H9.)

**Grade:** [Verified: read the currency audit in full; ran the `with {}` round trip (H9);
`grep -rn PDO src/lift/` and `src/transpile/` both empty; read `explain/transpile_di.rs:88-89`.]
**Severity: P2** — an overclaiming headline is a documentation-integrity defect in its own right,
because audits are cited later by their conclusions, not their scope sections.
**Correction:** `:10` → "**RESULT (scope: the three features shipped 2026-07-25): No GAPs** — every
surface is UP-TO-DATE or N-A with a sound, stated reason **for these three features**. This audit
does NOT certify Invariant 17 repo-wide; see `KNOWN_ISSUES.md` for standing lift-tier exclusions."
Add a `## Out of scope` section naming the standing exclusions (Tier-2/3 lift frontier, the
`E-TRANSPILE-*` native-only set). **Generalise it:** any audit whose scope is one slice should carry
the scope inside its RESULT line, not only in its method paragraph.

### H33 — Three same-day audit files in `docs/research/` with no index [P3]

`docs/research/2026-07-25-{plans-divergence,currency,lsp-completion}-audit.md` are all dated today;
`docs/research/` has **no README or index**, and none of the three is referenced from MASTER-PLAN,
SLICE-STATE, or the register. Their findings (H30: all nine fixed) live only inside the audit files,
so a fresh context cannot tell a closed finding from an open one without re-verifying all nine — as
I just did. Not an Invariant-19 violation (an audit records findings, not roadmap/decision/status),
but it is the seam where audit findings *become* untracked status.
**Grade:** [Verified: `ls docs/research/`; grepped MASTER-PLAN/SLICE-STATE/C-decisions for the three
filenames — no hits.] **Severity: P3.**
**Correction:** add a `docs/research/README.md` index with one line per audit (date, scope, verdict,
**open/closed**), and require that any audit finding accepted as work-to-do is mirrored into
SLICE-STATE/MASTER-PLAN per Invariant 19 — an audit file is a findings record, never a task list.
*(This report is subject to its own recommendation: it is a findings record. Nothing in it should be
treated as roadmap state until the developer triages it into SLICE-STATE.)*

---

## 7. Delegated deep-dives (two parallel read-only agents; findings independently evidence-backed)

Two agents covered the axes I could not read exhaustively. Their coverage statements are reproduced
so the sampling honesty carries through. Where a finding duplicates mine I say so and keep the
stronger version.

**Agent A — SSOT divergence** (built the complete repo-wide `DEC-[0-9]+` set: **303 distinct ids,
max DEC-338; the register contains 290**; cross-tabulated status keywords for all 303 across 11
files; manually read both sides for 25 ids; read `ROADMAP.md` in full + targeted MASTER-PLAN
§0/§11/§13, M-gap-matrix §4.11/§4.12, SLICE-STATE. **Not covered:** CHANGELOG body, per-row
verification of the 824-row gap matrix.)

**Agent B — spec staleness + dangling refs** (all 11 dated spec status lines read; §"Naming
overhaul" read line-by-line; path+link extraction over **all 175** non-`target` `.md` files; all
intra-file anchors in UNIFIED-SPEC verified; archive listing diffed against its README. **Sampled:**
8 of ~25 DECs in UNIFIED-SPEC's ⚠ batches; anchors outside UNIFIED-SPEC unverified;
`docs/research/**` excluded.)

### H34 — **P0 (supersedes my H31): both canonical cursors cite commit hashes that no longer exist in the branch**

> `docs/plans/MASTER-PLAN.md:30` — "_(UPDATE 2026-07-25: subsequent slices through DEC-337 are now
> PUSHED to `origin/master @ 6e0c58a` …)_"
> `docs/plans/SLICE-STATE.md:97` — "**Pushed:** `origin/master @ dee608e` — all of Q-A + Q-B …"

Git reality: `HEAD == origin/master == 25053be`; `git merge-base --is-ancestor 6e0c58a HEAD` → **no**,
same for `dee608e`. **Both hashes are orphaned** — an amend/rebase rewrote them (two commits in
current history carry the same subjects: `88cc0de` ≡ `dee608e`, `11ca804` ≡ `6e0c58a`), and neither
canonical home was re-pointed. Both cursors are **11 commits behind**, and they name **different**
tips as current.

**Grade:** [Verified: `git rev-parse`, `git merge-base --is-ancestor` on both hashes, read both doc
lines.] **Severity: P0** — SLICE-STATE *is* the live cursor per Invariant 19; a fresh context
resuming cannot resolve either SHA. This is strictly worse than my H31 (I saw only "stale SHA"; the
SHAs are in fact unresolvable). **Correction:** set both to `origin/master @ 25053be`, note the
rewrite. Structurally: record `origin/master` + a subject line rather than a bare short hash — a
hardcoded SHA in a living cursor cannot survive a rebase (this is the second recurrence today).

### H35 — **P1: `2026-07-24-wildcard-imports.md` is titled "NOT YET BUILT" while its own §BUILD STATUS says shipped + certified**

> `:1` — "# SPEC (RULED — BUILD-READY, **NOT YET BUILT**) — Wildcard & group imports"
> `:3` — "Status: **RULED 2026-07-24 … BUILD-READY, NOT BUILT.**"  `:7` — "Mirrored as QUEUED…"

vs, in the **same file**:

> `:215` — "## BUILD STATUS (autonomous, 2026-07-25) / Steps 0-1 (parser) ✅ f8c5224 · step 2 ✅
> 6bf9c3b · step 3 ✅ 30bc060 · steps 5-6 ✅ 084fe77"
> `:227` — "## ✅ Q-A DONE (2026-07-25 — DEC-268 certified)"

Independently confirmed built: `src/parser/items/decls/imports.rs:100,122,143` implement
`E-WILDCARD-ALIAS`/`E-WILDCARD-STDLIB-ROOT`; `examples/project/wildcard-imports/` exists and
`phg run …/src/main.phg` → "area: 12 / true / paint: green". Also stale: `:232`
"## Backends / invariants checklist (**for the eventual build**)".

**Grade:** [Verified: read `:1,:3,:7,:215,:227,:232`; grepped the parser implementations; ran the
example.] **Severity: P1** — the file's title is the first thing read and says the opposite of its
own body. Note the sibling `2026-07-24-visibility-model.md:1` gets it right
("# SPEC (RULED — BUILT, 2026-07-25)"), so this is a one-file omission, not a missing convention.
**Correction:** retitle `:1`/`:3` to BUILT + DEC-268-CERTIFIED 2026-07-25; drop `:7`'s "QUEUED";
relabel `:232` "(satisfied)".

### H36 — **P1: `2026-07-23-eval-position.md` declares a V1 build item with zero implementation, and contradicts itself**

> `:3` — "Status: **RULED … `Core.Sandbox` BUILDS IN V1**"; `:47` "**RULED … BUILDS IN V1** with
> exactly this scope"; `:62` "**P2 → Core.Sandbox v1 BUILDS**"

vs `grep -rn 'Sandbox\|SANDBOX' src/ tests/ conformance/ examples/` → **zero hits** (neither
`Core.Sandbox` nor the ruled ladder code `E-TRANSPILE-SANDBOX`), and **no Sandbox slice** in
MASTER-PLAN or SLICE-STATE. The file also self-contradicts: `:5` "the one open avenue is a sandboxed
typed sub-interpreter, **gated on a concrete use case**" and `:25` "the §3 sandboxed sub-interpreter,
**IF ruled in**".

**Grade:** [Verified: read `:3,:5,:25,:47,:62`; zero-hit grep across four trees.] **Severity: P1** —
Invariant 19 requires a ruled-but-unbuilt spec to be recorded as QUEUED so a fresh context resumes
from repo state; this one is invisible to both plan files. **Correction:** restate `:3` as
"RULED-TO-BUILD, NOT BUILT (no slice yet)", reconcile `:5`/`:25`, and file the slice in
MASTER-PLAN + SLICE-STATE as QUEUED. *(Whether Sandbox should build at all is the developer's call —
Invariant 15.)*

### H37 — **P1: `2026-07-23-entry-kinds-serve-tls.md` says BUILD-READY; one of its two breaking changes has shipped**

> `:3` — "Status: **SPEC RULED (dev, 2026-07-23) — BUILD-READY.** … Contains the cluster's TWO
> breaking changes (D5: `respond(bytes)` retired; §6 P1: bare `#[Entry]` now requires `kind:`)."

§6 P1 **is** shipped: `phg check` on a bare `#[Entry]` → "`#[Entry]` requires a `kind:` … 
[E-ENTRY-KIND-REQUIRED]"; raised at `src/checker/program/entry_points.rs:55,68,121` with a
regression test at `src/checker/tests/entry_point.rs:143`. The file's own body knows this (`:40`
records the 2026-07-25 DEC-337 update) — only the status line was not updated. The rest is genuinely
unbuilt: `Http.ServeConfig` → `E-UNKNOWN-TYPE`; `respond` is **not** retired
(`src/serve/handlers.rs:27` `pub const SERVE_ENTRY: &str = "respond";`); no inbound TLS in
`src/serve/`.

**Grade:** [Verified: ran the bare-`#[Entry]` and `Http.ServeConfig` probes; read the checker
sites + `serve/handlers.rs:27`.] **Severity: P1** — this is the causal root of my **H1**: the
breaking change shipped, and neither its own spec status line nor the README was updated in the same
change (Invariant 17). **Correction:** `:3` → "PARTIALLY BUILT: D1/§6-P1 SHIPPED 2026-07-25
(DEC-337); D4 (`Http.ServeConfig`), D5 (retire `respond`), D6/D7 NOT BUILT", plus a `## BUILD STATUS`
section matching the convention the invoke/rich-request/visibility specs already use.

### H38 — **P1: 6 of the 8 rules Invariant 12 attributes to §"Naming overhaul" are absent from it** *(upgrades my H21; prior agent's report confirmed and widened)*

The cited section is `docs/specs/UNIFIED-SPEC.md:273-338`. Its structure: `:275` status,
`:279 ### Policy (locked)` — **6 numbered rules, all about abbreviations** — `:291` change list,
`:329` codemod safety. Verdict per Invariant-12 rule:

| `CLAUDE.md:119-122` claims | in §273-338? | evidence |
|---|---|---|
| keyword `function`, never `fn` | ✅ **present** | `:300-301` "Lambda **`fn` → `function`**" |
| packages/types PascalCase | ⚠ **aside only** | `:293-294` "user classes are PascalCase" — the word occurs **once**, parenthetically; the locked-policy block has no casing rule |
| type-params PascalCase | ❌ absent | zero occurrences |
| `package Main;` reserved | ❌ absent | lives at `:524` (§Import roots and PSR-4 mapping) |
| `Core.` reserved | ❌ absent | lives at `:707` (§Standard library charter) |
| functions/natives camelCase | ❌ absent as a rule | zero occurrences of "camelCase" in-section; stated at `:708` |
| return types `: T` | ❌ absent | lives at `:22` |
| mandatory `new` | ❌ absent | lives at `:22`, `:1194` |
| explicit `this.field` | ❌ absent | **exists in no spec section at all** |

Six of the eight actually live in `UNIFIED-SPEC.md:20-23` — a **caveat about historical code
samples**, not a normative section: "Canonical current syntax: `function` (never `fn`), `: T` return
annotations, … mandatory `new`, `Core.` stdlib root, PascalCase packages/types, camelCase
functions." And `:1194` back-references "Invariant 12" for mandatory `new` — a **circular
citation** (CLAUDE.md points at the spec; the spec points back).

**Confirms and widens the prior agent's package↔folder finding:** §"Naming overhaul" has zero
mention of folders. The package↔folder relationship lives at `:482` §"Import roots and PSR-4
mapping", whose ruling is in fact the *decoupling* of the two — `:521-522` "**Emitted PHP namespace
is always the namespace path, never the folder**".

All eight rules are nonetheless **enforced by the binary** (`fn(int x) => x` → parse error;
`A a = A();` does not construct; `E-PKG-CASE` per my H21), so this is purely a citation defect —
but a costly one: an agent that obeys `CLAUDE.md:121` and reads only the cited section learns two of
eight rules.

**Grade:** [Verified: §273-338 read line-by-line; each absent rule located elsewhere by grep with
line numbers; `fn` and no-`new` rejections probed on the binary.] **Severity: P1.**
**Correction (preferred):** add a `### Canonical surface (binding)` subsection under
`## Naming overhaul` stating all eight explicitly — promoting `:20-23` out of the historical-samples
caveat, folding in `Core.<Pascal>` (`:707`), `package Main;` (`:524`), and giving `this.field` its
first home. Then `CLAUDE.md:121` becomes true and `:1194`'s circular reference can be de-circularized.
Alternative: make `CLAUDE.md:121` a multi-target pointer — cheaper, but leaves the SSOT non-singular,
which Invariant 19's spirit disfavours.

### H39 — **P1: the MASTER-PLAN roadmap SSOT presents shipped work as an open build queue (three instances)**

**(a) Tier-1 security queue, all six shipped, none ✅-stamped.** `MASTER-PLAN.md:423` "**Tier 1 —
HIGH correctness/security (do first):**" lists DEC-263, 264, 270, 265, 251, 252, 255 — **none**
carries the `✅` that Tier-2 items 7/8/11/13/14/15/16 all carry. Register says all closed:
`C-decisions.md:1778` "DEC-263 — **SHIPPED** (2026-07-16, Tier-1 build)", `:1798` DEC-264 SHIPPED,
`:1845` DEC-270 SHIPPED, `:1866` DEC-265 SHIPPED, `:1905` DEC-251 COMPLETE, `:1927` DEC-252 SHIPPED,
`:1965` DEC-255 RULED. No "Tier 1 CLOSED" annotation exists anywhere in MASTER-PLAN.
**These are the *security* items** — the roadmap tells a resumer to "do first" six things that
shipped 9 days earlier.

**(b) Same-file contradiction on DEC-256/242/258/243.** `MASTER-PLAN.md:30` — "DEC-256/242/258/243
all **COMMITTED + panel-certified**" vs `:487` item 17 "**DEC-256** W4-4 Unicode FULL" and `:488`
item 18 "**DEC-243** … **DEC-242** … **DEC-258**" — both un-✅'d. Corroborated shipped by
`C-decisions.md:2179` "**DEC-258 BUILT (2026-07-17)**".

**(c) ≥7 UNIFIED-SPEC ⚠ "build-pending" items are shipped.** `UNIFIED-SPEC.md:28` "**⚠ PENDING
SURFACE CHANGES — 2026-07-13 … (RULED, build-pending)**" and `:47` (the 2026-07-16 batch). Agent B
probed the binary on 8 and found 7 built: DEC-207 `A::f()` → prints `7`; DEC-214
`new List<int>()` → `0`; DEC-209 `default` arm → `z`; DEC-211 `class Box<T: Shown>` binds clean;
DEC-248 typed `foreach` green + `for-in` already retired ("parse error … found In"); DEC-249 method
default params type-check clean; DEC-253 `(int|string)? x = 1;` clean. Control case still correctly
pending: DEC-254 `ref` params → parse error. The `✅ … SHIPPED` convention already exists in the same
block (DEC-239, DEC-257) and was simply not applied.

**Grade:** [Verified: both sides read with line numbers for (a) and (b); 8 binary probes for (c),
each cross-checked with a `src/` grep.] Agent B is explicit that the ~17 unprobed DECs in those
batches are **[Unverified]**, so 7 is a **floor**. **Severity: P1** — Invariant 19 makes MASTER-PLAN
the roadmap SSOT; a queue that lists delivered work as "do first" actively misdirects a fresh
context, and the ⚠ blocks overstate the pending language surface by ≥7 items.
**Correction:** mechanical ✅-stamping in all three places, plus — per the ⚠ batch's own G-5 rule
("prose is rewritten in the same change that implements it") — folding the 7 shipped items' prose
into their sections.

### H40 — **P1: 13 DEC ids are referenced repo-wide with no register row, and DEC-188…193 are register-forked into MASTER-PLAN**

Complete set arithmetic: 303 distinct ids referenced across `*.md`; register holds 290. **Missing
entirely from `C-decisions.md`: DEC-185, 187, 188, 189, 190, 192, 193, 194, 195, 196, 198, 199, 303.**
Several are load-bearing, not incidental:

> `examples/format/README.md:1` — "# `phg format` — the width-canonical formatter (**DEC-187**)"
> — a doc *titled* after an unregistered ruling
> `examples/README.md:102` (DEC-194 user attributes), `:135-136` (DEC-196 intrinsic imports),
> `:99` (DEC-199 `String.format`); `SLICE-STATE.md:1340` + `M-gap-matrix.md:818` (DEC-303 `String.chunk`)

The **forked register**: `MASTER-PLAN.md:2175` — "### 13.1.1 · 2026-07-04 design-seed adjudications
(RULED interactively — NEXT-SESSION build queue, **DEC-188…193**)" … "**None built yet — this is the
design record + build queue.**" — six full register-shaped ruling blocks (`:2181`, `:2187`, `:2194`,
`:2220`, `:2229`) with *Alternatives* and *Rationale*. `C-decisions.md` has zero rows for them.

Sharpest instance — **DEC-190's ruling exists in exactly one place in the entire repo**:
> `MASTER-PLAN.md:2194` — "- **DEC-190 — Core is extensible: all Core CLASSES `open`, all Core
> methods overridable.**"

`grep -rniE 'all Core (classes|methods)'` returns only that line. No corroboration in FEATURES.md,
UNIFIED-SPEC.md, or CHANGELOG — and it sits in tension with the documented *user* model,
`FEATURES.md:66` "final-by-default (a class/method must be `open` to extend/override)". A
language-surface ruling with a single un-registered home is one bad edit from being lost.

Register numbering also has unreferenced reserved gaps (17–19, 38–46, 71–79, 108–109, 115–119,
136–139, 157–159 — harmless) plus the 13 real holes above.

**Grade:** [Verified: complete `DEC-[0-9]+` set built from all `*.md` and differenced against the
register; both sides read for the cited instances; single-hit grep for DEC-190.]
**Severity: P1** — a two-directional Invariant-19 breach: rulings with no canonical home, and
register-shaped content living in the roadmap doc.
**Correction:** back-fill register rows for the 13 (185/187/194/195/196/198/199/303 as SHIPPED with
commit refs; 188–193 moved **verbatim** out of §13.1.1 as QUEUED), then replace §13.1.1's bodies with
pointers. Root cause is H45.

### H41 — **P1: three superseded rulings are presented as live, all in MASTER-PLAN's most-read sections**

**(a) DEC-201 — retracted language rule stated as "locked" in §0.**
> `MASTER-PLAN.md:32` (§0 CURSOR, "Locked rulings") — "empty literals = contextual typing +
> List.empty/Map.empty (**DEC-201**)" — no supersession marker.
> `C-decisions.md:384` — "**DEC-201** … *(**SUPERSEDED by DEC-214**, 2026-07-13 — empty collections
> now use `new List<T>()`/`new Map<K,V>()`; `[]`/`{}` contextual typing and `List.empty`/`Map.empty`
> **removed**)*"

MASTER-PLAN contradicts itself 1054 lines later — `:1086` "`List.empty`/`Map.empty` were **never
built** and are not planned" and `:298` "No `List.empty`/`Map.empty` factory ever existed." Other
files get it right (`UNIFIED-SPEC.md:34-35`, `KNOWN_ISSUES.md:510`). §0 is the first thing a resuming
context reads.

**(b) DEC-200 — "PENDING adjudication" in two files, closed in the register.**
> `MASTER-PLAN.md:2249` — "**DEC-200** … (**PENDING adjudication**, surfaced 2026-07-06). **Not yet
> ruled** — surface to the developer via AskUserQuestion before building (§15)."
> `KNOWN_ISSUES.md:675-679` — "a **PENDING adjudication question** … **Until ruled, avoid** naming a
> top-level `class`/`enum` after a PHP builtin class or a non-guarded reserved keyword"
> `C-decisions.md:389` — "**DEC-202 (closes DEC-200)** … *(**SHIPPED 2026-07-13**:
> `is_php_builtin_class_name` in checker/common.rs …)*"

Both groups DEC-200 named are closed (the keyword subset by `SLICE-STATE.md:1465`). MASTER-PLAN
even contradicts itself at `:32` ("DEC-202, **closes DEC-200**"). **And this was already caught and
never fixed** — `docs/research/2026-07-16-full-reopen-audit.md:440` flagged exactly this 9 days ago.
The live consequence: `KNOWN_ISSUES.md` still instructs the developer to avoid a naming pattern the
compiler already guards.

**(c) DEC-191 — retired ruling carries a SHIPPED stamp and no retirement note.** The only full
DEC-191 ruling in the repo is `MASTER-PLAN.md:2201` (role inferred from signature), stamped
"**SHIPPED 2026-07-17 fable**" at `:2215`. But the inference is retired:
`C-decisions.md:2982` "**BREAKING #2: bare `#[Entry]` = `E-ENTRY-KIND-REQUIRED`, DEC-191 inference
RETIRED**"; `2026-07-23-entry-kinds-serve-tls.md:79` same; and MASTER-PLAN itself knows it at `:105`.

**Grade:** [Verified: both sides read with line numbers for all three; the self-contradictions
located in the same file.] **Severity: P1** — (b) is the worst because it emits *active wrong
guidance*, and it survived a prior audit that named it.
**Correction:** `:32` → "empty collections = `new List<T>()`/`new Map<K,V>()` (DEC-214, supersedes
DEC-201)"; `:2249` → "CLOSED by DEC-202 (shipped) + the keyword-set fix", and **delete**
`KNOWN_ISSUES.md:675-679`'s "Until ruled, avoid…" paragraph; annotate `:2201` "(role inference
SUPERSEDED 2026-07-25 by DEC-331 D1 / DEC-337 — kind is now explicit)".

### H42 — **P2: the percentage ledger — `44ffe21` only partly delivered, and §11.3 now projects a future below the present** *(resolves my H17's open axis)*

Agent A inventoried **every** parity/vision/floor figure in the repo. The good news first, and it is
substantial: **no file outside `MASTER-PLAN.md` and `M-gap-matrix.md` quotes a stale parity number as
current** (checked README, FEATURES, VISION, STABILITY, MILESTONES, HISTORY, KNOWN_ISSUES,
UNIFIED-SPEC, ROADMAP), and `M-gap-matrix.md:897-899` explicitly reconciles §4.12's simple-model 44%
against §4.11's weighted 49% to **prevent a double-count**. That discipline is real.

Three residual defects:

**(a) `44ffe21`'s claim is *partly* true.** It added exactly two SUPERSEDED banners (`:844` and
`:1841` — both verified real). It did **not** reach:
> `MASTER-PLAN.md:1799-1805` (§11.4) — "Parity = … ≈ **60%**" / "Vision … ≈ **62%**. Floor ≈ **41%**" — unmarked
> `MASTER-PLAN.md:1776` (§11.2) — "Raw row-parity floor moves to ≈**39%**" — unmarked

Aggravating: §11's heading order is **11.1 → 11.2 → 11.4 → 11.5 → 11.3**, so the one banner (at the
end of §11.5) sits *after* §11.4 and *immediately before* §11.3, making its scope genuinely
ambiguous.

**(b) §11.3's projection is overtaken by reality.**
> `MASTER-PLAN.md:1851` — "| baseline (2026-07-10 … §11.4) | … | **≈60%** | **≈62%** |"
> `MASTER-PLAN.md:1854` — "| W3 | DB + HTTP + sessions + FS + url … | **≈65–66%** | ≈69% |"
> vs `M-gap-matrix.md:828` — "**PHP-parity = 0.35×83.3 + 0.40×49.1 + 0.25×75.0 = … ≈ 68%**", `:831`
> "**Vision … ≈ 69%**"

The roadmap's *after-W3* forecast is **already passed on parity and already met on vision**. A
resumer planning W4–W6 from §11.3 mis-sizes every remaining wave.

**(c) The "owed §1.2 re-tally" is DONE in two files and STILL OWED in two — including inside
M-gap-matrix itself.**
> DONE: `M-gap-matrix.md:841` "### 4.12 FULL §1.2 per-row re-tally … (**the owed 631-row re-pass**)"
> → "27.5% → ≈ **44.1%**"; `SLICE-STATE.md:1243` "✅ §4.12 full §1.2 re-tally".
> OWED: `M-gap-matrix.md:614` "**full re-tally still owed**"; `MASTER-PLAN.md:30` "⚠ A full per-row
> §1.2 re-tally is **still owed**".

`M-gap-matrix.md:613` also still marks §4.11 "⟶ CURRENT" though §4.12 and §4.13 follow it.

**(d) `ROADMAP.md` routes readers to the stale ledger.** `ROADMAP.md:7-8` calls MASTER-PLAN the
"**live percentage ledger**", but MASTER-PLAN disclaims itself at `:1844-1845` — "**Treat
`M-gap-matrix §4.11` (and §0) as the parity-% SSOT**". So the one otherwise-exemplary pointer doc
(my positive attestation #1) has a single wrong pointer.

**Grade:** [Verified: full percentage inventory with file:line for ~22 distinct figures;
`git show 44ffe21 -- docs/plans/MASTER-PLAN.md` confirmed exactly two added banners.]
**Severity: P2** (the figures are recoverable; the SSOT pointer is not misleading in *direction*, only
in *target*). **Correction:** banner §11.2/§11.3/§11.4; re-base §11.3 on the ≈68/69/53 anchor or mark
it SUPERSEDED pending the re-projection §11.5 already calls owed; point `M-gap-matrix.md:613-614` at
§4.12 as latest and strike the "still owed" clauses; `ROADMAP.md:7` → point the percentage ledger at
`M-gap-matrix.md §4` with MASTER-PLAN §0 as the mirror. **This settles the 57%-vs-69% question in my
H17: the ≈57% is GA-CHECKLIST's separate shippability model, not a third parity figure — but it is
still an unindexed authority, so H17's correction stands unchanged.**

### H43 — **P2: five more spec/register status divergences (all one-line fixes)**

| # | Live-but-wrong side | Correct side | Note |
|---|---|---|---|
| a | `UNIFIED-SPEC.md:1297` — "**Pending amendment: DEC-265** … SMTP **will** REQUIRE TLS when credentials are set"; `:1309` "TLS — see the DEC-265 amendment above" | `C-decisions.md:1866` — "**DEC-265 — SHIPPED (2026-07-16, Tier-1 build)**: SMTP require-TLS when credentials are set" | **security posture** described as not-yet-in-force |
| b | `UNIFIED-SPEC.md:1295` "**DEC-249 now queued**"; `:1277` "**when DEC-249 method defaults land**" | `MASTER-PLAN.md:461` "13. ✅ **DEC-249** … **SHIPPED 2026-07-16**"; corroborated `C-decisions.md:2090,2179` | |
| c | `UNIFIED-SPEC.md:105` "*(RULED DEC-273; **migration pending**)*"; `:1003` "physical migration **PENDING**" | `C-decisions.md:2294,2344,2412` — waves 1/2/3 **BUILT** 2026-07-17, certified `:2335,:2404,:2421`; `MASTER-PLAN.md:30` agrees | 16 extensions migrated |
| d | `C-decisions.md:908,916` — "**DEC-223 RULED build-pending**" … "build handed to Fable" (the register's **only** two DEC-223 lines; no BUILT stamp exists) | `UNIFIED-SPEC.md:110,1291` "**SHIPPED (2026-07-15)**"; `MASTER-PLAN.md:638`; `examples/mail/README.md:1`; `2026-07-16-full-reopen-audit.md:385` | **the register is the only file that thinks `Core.Mail` is unbuilt** — inverted from the usual direction |
| e | `SLICE-STATE.md:1474` — "⚠ **replaceCallback CORE = DEC-295 PENDING — BUILD-READY DESIGN LOCKED**" + ~15 lines of build instructions | `C-decisions.md:215` — "✅ **BUILT (2026-07-18)**" | mitigated: `:1467` has a "(historical detail below)" divider and `:1465,:1426` record it COMPLETE — P3 |

**Grade:** [Verified: both sides read with line numbers for all five; (d) confirmed by a negative
grep — `Core.Mail` + built/shipped over the register returns nothing.] **Severity: P2** (e: P3).
**Correction:** one-line status flips each; (d) needs a `✅ BUILT 2026-07-15` closing note on the
DEC-223 row **and** a fix to the `:908` section heading.

### H44 — **P2: `KNOWN_ISSUES.md` institutionalizes 17 known-stale rows instead of correcting them**

> `KNOWN_ISSUES.md:45-49` — "**⚠ 2026-07-16 FULL REOPEN AUDIT — this file was fully re-verdicted.**
> Every row was reopened; **17 rows are STALE (superseded by later shipped work)** … **Individual
> stale rows below are corrected as their build slices land; until then, cross-check against the
> audit report.**"

This is a standing instruction to consult a **fourth** document
(`docs/research/2026-07-16-full-reopen-audit.md`, itself carrying 217 distinct DEC ids) to know
whether any row in a 2323-line file is true. That converts every row into "unverified" — the exact
divergence Invariant 19 forbids. **H41(b) is the concrete casualty**: DEC-200's stale row has
survived 9 days under this policy while emitting active wrong guidance.

A second, mechanically-caused corruption in the same file:
> `KNOWN_ISSUES.md:64-70` — "2. **interp/VM labels stale/inverted.** … **`phg run` = the bytecode
> VM** … there is **no `phg run` subcommand**. Docs still name a literal `phg run` … 
> (`phg run -e`), … (`phg run --dump-on-fault`)."

It defines `phg run` and denies its existence in the same sentence, then lists **correct**
invocations as instances to sweep. Cause identified: commit `f69b746` (DEC-330, retire the `runvm`
name) blanket-replaced `runvm`→`run` in prose *whose subject was the word `runvm`* — the original
read "there is **no `phg runvm` subcommand** … (`phg runvm -e`)". The item is now unactionable and, if
obeyed, would break working commands. **Same root cause as my H8** (the "the VM leg" corruption) — one
find-and-replace campaign, two damaged files.

*Reported for honesty (negative result):* Agent A checked whether DEC-330's sweep left live `runvm`
references and found **none** — `CLAUDE.md:81` and `SLICE-STATE.md:827` are correct *negations*, and
the 166 CHANGELOG / 17 MILESTONES / 29 register hits are all inside DEC-330's explicit
record-exemption. **No finding there.**

**Grade:** [Verified: read `:45-49` and `:64-70`; `git show f69b746` confirmed the pre-sweep text;
`grep -c runvm` across the governing docs.] **Severity: P2.**
**Correction:** resolve the 17 rows against the audit's §D2 and **delete the standing cross-check
clause**; delete or restore-verbatim-under-a-historical-marker punch-list item 2. Also
`KNOWN_ISSUES.md:3` — the top P0 heading "## 🔴 P0 (2026-07-19) — the example byte-identity GLOB is a
NO-OP" is un-✅'d although `:30` says "**Status: ✅ FIXED (2026-07-19, `a355c342`)**" (201 SKIP → 139
RUN) — a TOC-scan hazard, P3.

### H45 — **P3 (root cause of H40/H41): `MASTER-PLAN.md` §13 is a second decision register by construction**

`MASTER-PLAN.md:1890` "## 13. DECISIONS LOG — 2026-07-03 unification-audit rulings (all
developer-ruled, final)", plus §13.1 (`:1949`), §13.1.1 (`:2175`), §13.2 (`:2284`), and
"Appendix B — 2026-07-02 RULINGS LEDGER (**authoritative** for the wave-0..6 roadmap)" (`:2367`).
MASTER-PLAN carries **122 distinct DEC ids** — second only to the register's 290 among non-research
files — and H40 (DEC-188–193, DEC-190), H41(a) (DEC-201), H41(b) (DEC-200), H41(c) (DEC-191) are
**all** instances of rulings whose only or most-current text lives here rather than in the register.
Appendix B even claims the word "authoritative", which the register is supposed to own.

**Grade:** [Verified: structural — headings read, 122-id count from the full DEC set.]
**Severity: P3 standalone** (a reader does not hit it directly) **but it is the generator of four P1
findings**, so fixing it is the highest-leverage single action in §2/§7.
**Correction:** banner §13 + Appendix B as *frozen historical records* ("canonical rulings live in
`C-decisions.md`"), and migrate the DEC-188…193 / 190 / 191 / 200 / 201 bodies out per H40/H41.

### H46 — **P2: 60+ dangling `src/` path references, concentrated in four files; one dangling anchor cited 3×** *(completes my H26)*

Agent B extracted every backticked repo-rooted path and every `](path)` link from **all 175**
non-`target` `.md` files and `test -e`'d each. Headline: **markdown links are perfect — 0 dangling
across every file.** Backticked source paths are not.

**Confirmed dangling, by file** (all [Verified]):
- **`docs/INVARIANTS.md`** — `:62` `src/vm.rs`, `:63` `src/compiler.rs`, `:64` `src/chunk.rs`,
  `:38` `src/value.rs`, `:39` `interpreter.rs`/`vm.rs`, `:174` `src/jit/tests.rs` (actual:
  `src/jit/tests/` — 16 files; the `ovf_spec_*` guards are plausibly in
  `range_and_overflow.rs` [Inferred: from filename, symbol not grepped]).
- **`CLAUDE.md`** — `:90` `src/chunk.rs`, `:92` `src/value.rs` (my H7; Agent B independently
  confirms and adds that `src/value/arith.rs` is the file the invariant is *about*).
- **`docs/MILESTONES.md` — 30 dangling targets**, the worst file after CHANGELOG: 7 pre-decomposition
  source paths (`:20` `src/transpile.rs`, `:31` `src/chunk.rs`+`src/vm.rs`, `:32,:39`
  `src/compiler.rs`, `:35` `src/cli.rs`, `:90` `src/bundle.rs`, `:277,:287` `src/serve.rs`) and 23
  retired plan/spec files. Notably `:272` `2026-06-18-m5-project-model-design.md` and `:297`
  `2026-06-18-m6-web-design.md` are **not in `archive/` either** — deleted, not moved.
- **`docs/plans/MASTER-PLAN.md` — 11**: `:51` `src/ext/db/` (→ `src/ext/database/`), `:237`
  `tests/verticals/`, `:325` `src/native/db.rs`, `:494` `src/devtools/`+`src/package/` (→ `src/pm/`),
  `:590` `docs/archive/`, `:1936` `src/manifest.rs`, `:2117`/`:2121` `src/fmt/`+`src/fmt/printer.rs`
  (→ `src/format/`), `:2151` `examples/bench/`+`examples/fmt/`.
- **`docs/plans/SLICE-STATE.md` — 6**: `:1016` `src/lsp/refs.rs`, `:1084` `src/lsp/completion.rs`,
  `:1440` `bench/micro/deepjson`, `:1690` `src/ext/crypto/`+`src/ext/db/`, `:1691` `examples/db/`.
- **`docs/specs/UNIFIED-SPEC.md` — 4**: `:1191`+`:1377` `docs/plans/web-spine.plan.md`, `:1326`
  `docs/plans/di-attributes.plan.md`, `:1349` `src/checker/desugar_di.rs`.
- **`docs/specs/2026-07-24-wildcard-imports.md` — 8** pre-decomposition paths in its build plan
  (`:165,166,209,210,211`) — ironically the same spec whose `:224` reports *performing* an
  Invariant-13 decomposition without updating its own touch-list.
- **`examples/**/README.md` — 5 genuine**: `examples/README.md:211` + `examples/web/README.md:43`
  `tests/crypto.rs` (no such file), `examples/README.md:220`/`:232`
  `src/ext/database/{mysql,postgres}.rs` (→ `.../natives/`), `examples/format/README.md:45`
  `tests/fmt.rs` (→ `tests/format.rs`), `examples/web/README.md:55,110` `src/serve.rs`.

**Dangling anchor, cited 3× [Verified]:** `#coredb--the-enhanced-pdo-database-primitive-dec-208` at
`UNIFIED-SPEC.md:109` (its own table of contents), `UNIFIED-SPEC.md:1186`, and the archive README's
**only** pointer for the DB spec. The real heading is `UNIFIED-SPEC.md:1236 ## Core.DatabaseModule —
…` → `#coredatabasemodule--the-enhanced-pdo-database-primitive-dec-208`. `grep -c 'Core\.Db'
UNIFIED-SPEC.md` = **0**: the module was renamed `Core.Db` → `Core.DatabaseModule` and the heading
followed, but the three anchors kept the old slug. All other UNIFIED-SPEC anchors verify clean
(including `CLAUDE.md:10`'s §"External dependency policy" → `:871` and `CLAUDE.md:122`'s §"Naming
overhaul" → `:273`).

**Deliberately not counted as defects:** `CHANGELOG.md`'s 55 dangling targets — an immutable
historical record where a path correct at commit time is *expected* to rot; rewriting it is worse
than leaving it. `KNOWN_ISSUES.md`'s 11 **are** worth a sweep (it is a live doc): `src/jit/analyze.rs`
(`:101,271`), `src/jit/handles.rs` (`:103,312`), `src/ext/database/natives.rs` (`:103,703,717`),
`src/cli/explain.rs` (`:103`), `src/fmt/{doc,printer,printer/expr}.rs` (`:1982,2000,2003`).
Agent B also correctly triaged out several **false positives** — `examples/package-manager/README.md:5`
`src/main.phg` and `examples/project/function-imports/README.md:4-5` resolve relative to their own
example dir; `examples/project/withdeps/README.md:38` `src/Acme/Strutil/` is *deliberately*
non-existent prose illustrating a shadow that does not ship; `2026-07-22-transpile-into-project.md:66`
`src/_phorj/runtime.php` is an output path in the *user's* app (feature confirmed at
`src/cli/build_php.rs:71`). `docs/adr/0001:14` `src/ir.rs` is arguably intentional given the ADR is
titled "no shared IR". **Do not "fix" these.**

**Grade:** [Verified: exhaustive extraction + `test -e` over 175 files; anchor slugs recomputed with
the GitHub algorithm; false positives hand-triaged.] **Severity: P1** for the INVARIANTS/CLAUDE.md
invariant paths (H7), **P2** for the rest.

**Clean files (positive result):** `README.md`, `CONTRIBUTING.md`, `FEATURES.md`,
`docs/ARCHITECTURE.md`, `docs/EXTENSIONS.md`, `docs/EXTENSIONS-AUTHORING.md`, `docs/GA-CHECKLIST.md`,
`docs/DEPRECATION.md`, `docs/HISTORY.md`, `SEMVER.md`, `STABILITY.md`, `SECURITY.md`, `ROADMAP.md`,
`VISION.md`, `conformance/README.md`, `selftest/README.md`, `editors/**/README.md`,
`playground/README.md`, `docs/adr/0002-0005` — **zero dangling paths and zero dangling links**. Four
of the ten high-traffic files are entirely clean; the rot is concentrated in `docs/INVARIANTS.md`,
`docs/MILESTONES.md`, `docs/plans/`, and the spec files.

### H47 — **P2: archive hygiene — 0 of 20 archived specs carries a successor pointer**

`docs/specs/archive/README.md`'s table correctly lists **all 20** files on disk (verified row-for-row
— no missing, no phantom). But:

> `grep -l 'UNIFIED-SPEC' docs/specs/archive/*.md | grep -v README | wc -l` → **0**

All 20 originals still open with their **original** status line and no supersession banner. Read in
isolation — exactly how a grep-driven agent finds them — they read as current:
> `archive/2026-06-27-dependency-policy.md:3` — "Status: **adopted** 2026-06-27 (developer)."
> `archive/2026-06-29-m4-stdlib-charter.md:3` — "Status: **adopted** (2026-06-29). This is the
> **governing policy** for every `Core.*` … module."
> `archive/2026-07-03-unified-import-and-injected-type-discipline.md:3` — "Status: **ADOPTED** 2026-07-03"
> `archive/2026-06-19-core-html-design.md:2` — "Status: ✅ **Waves 1 … 3 all shipped** … the design is
> fully realized."
> `archive/2026-06-15-phorj-language-design.md:4` — "Status: Design frozen — ready for implementation planning"

The archive README's own warning "**do not treat them as current**" is visible only to someone who
opens the README, which **nothing in the 20 files links to**. This is materially relevant to my H3:
`archive/2026-06-27-dependency-policy.md` is the source document for the four-dep claim and still
reads "adopted".

Minor, same file: `archive/README.md:4` — "developer ruled to fold **all eighteen** into one
document" — there are **20** (the table itself flags two later additions with "*(folded
2026-07-16)*"). P3.

**Grade:** [Verified: 20-row table diffed against `ls`; zero-hit grep for successor pointers; five
status lines quoted.] **Severity: P2** (P3 for the count).
**Correction:** prepend a one-line banner to each of the 20 — "⚠ **ARCHIVED** (folded YYYY-MM-DD).
**SUPERSEDED BY** `docs/specs/UNIFIED-SPEC.md` §"<Section>" — do not treat as current." The per-file
target section is already tabulated in the README, so this is mechanical.

### H48 — Cross-cutting root cause identified: **M-Decomp moves files; no doc sweep follows** [P1 systemic]

Agent B's conclusion, which I independently reached from H7/H8: **12 of its 16 findings share one
cause.** CLAUDE.md Invariant 13 (soft cap 300 / hard cap 500, "split-as-you-go is the DEFAULT")
turned ~15 single-file modules into directories — `src/{vm,compiler,chunk,value,transpile,cli,
bundle,serve,interpreter,manifest}.rs` → `.../` — and **every citation silently broke**. Today's HEAD
(`25053be`, "refactor: M-Decomp 13 oversized files under the 300-line cap") did it again. The
secondary cause is the naming overhaul's own CLI renames (`fmt`→`format`, `bench`→`benchmark`), which
renamed `tests/fmt.rs`, `src/fmt/`, `examples/{bench,fmt}/` without a docs pass — the exact failure
`UNIFIED-SPEC.md:308` predicted: "The old names are **dead** — docs teaching them are wrong, per the
2026-07-03 audit B3-3."

**This is the single highest-leverage fix in the whole report.** A markdown reference checker
(backticked repo-rooted paths + `](links)` + intra-file anchors, with `CHANGELOG.md` allow-listed as
historical) would have caught **H7, H8, H26, H46, H47** and, going forward, prevents the entire class.
Agent B measured the scan at **well under a second over 175 files** — cheap enough for `pre-commit`,
and it pairs with the existing Rule 6 "Docs" evidence dimension. **Recommend adding it to
`scripts/git-hooks/pre-push`** (pre-commit is speed-critical per CLAUDE.md:32-34; pre-push already
carries the heavy tier).

---

## Top 10 by impact

| # | Finding | Sev | One-line why |
|---|---|---|---|
| 1 | **H1** README hero + both quickstart commands fail (DEC-331 `#[Entry]`) | **P0** | Front page; every newcomer's first command exits 1. Ran both verbatim. Root cause is H37. |
| 2 | **H34** Both canonical cursors cite commit SHAs **not in the branch** (`6e0c58a`, `dee608e` orphaned by a rebase); 11 commits behind; they name different tips | **P0** | SLICE-STATE *is* the live cursor (Inv 19) — a fresh context cannot resume. Second recurrence today. |
| 3 | **H2** INVARIANTS §6 "never SIGABRT" — reproduced exit 134; the stated 256 MB-worker mechanism doesn't cover the path | **P1** | Load-bearing correctness invariant measurably false, no disclosed carve-out (contrast §7, which does disclose). |
| 4 | **H48** M-Decomp moves files, no doc sweep follows → **60+ dangling `src/` refs** incl. CLAUDE.md invariants 3–4 and 30 in MILESTONES (H7/H8/H26/H46/H47) | **P1** | One systemic cause behind five findings. A sub-second markdown ref-checker in `pre-push` closes the whole class — highest-leverage single fix. |
| 5 | **H18** GA-CHECKLIST scores rock 3 at 15% against three blockers that **all shipped** (conformance corpus, SEMVER/STABILITY, DEPRECATION) | **P1** | 17 pts of headroom on stale premises → misdirects the critical path. The clearest "holding us back". |
| 6 | **H39/H40/H41/H45** MASTER-PLAN presents shipped security work as "do first"; 13 DEC ids have no register row; DEC-190's only home in the repo is one line; 3 superseded rulings live in §0 | **P1** | Four Inv-19 breaches with one generator (§13 is a second register). H41(b) emits *active wrong guidance* and survived a prior audit that named it. |
| 7 | **H3/H20** "four vetted deps" — actually **14 declared / 9 default-on**; the cited spec is stale too | **P1** | Opening paragraph of the file every session reads; UNIFIED-SPEC:875 itself warns against understated dep claims. |
| 8 | **H9/H28** Invariant 17 unsatisfiable: `with {}` transpiles to `clone(...)`, which `phg lift` **refuses** | **P1** | Impossible-compliance rule; the lift direction has no `E-TRANSPILE-*`-style exclusion mechanism. |
| 9 | **H4/H5/H6** Invariant 10 names retired `phg vendor`; Invariant 14 names a non-existent `--sequential-concurrency`; 4 shipped PM commands absent from `--help` | **P1** | Rules unenforceable against non-existent surfaces; shipped public interface undocumented. |
| 10 | **H22** ADR-0005 `Accepted` but its decision was reversed (DEC-282/316); no ADR-0006 | **P1** | A *canonical* record serving a reversed decision, breaking the ADR system's own supersession rule. |

*Just below the cut, and each a one-line fix:* **H35** (wildcard spec titled "NOT YET BUILT" above its
own "✅ DONE + certified"), **H38** (6 of 8 Invariant-12 rules absent from the section it names as
naming SSOT), **H16** (README status table contradicts MILESTONES), **H36/H37** (specs whose status
lines misstate their build state), **H43** (five register↔spec status flips, incl. a **security**
posture — DEC-265 SMTP require-TLS — described as not-yet-in-force), **H44** (KNOWN_ISSUES
institutionalizes 17 known-stale rows), **H42** (§11.3 projects a future parity *below* current
parity).

---

## Positive attestations (docs that are genuinely accurate and well-maintained)

Stated so the report is not read as uniformly negative — these held up under active testing:

1. **`ROADMAP.md`** — the model the rest of the repo should copy. `:3-5` explicitly refuses to carry
   per-item status *and explains why* ("this file previously accreted stale milestone markers; those
   now live only where they stay current"). Pure pointer, structurally undriftable. [Verified: read
   all 29 lines; every one of its four pointer targets exists.]
2. **`phg explain`** — the best-maintained surface I tested. All five codes I sampled resolved with
   accurate text, including **retirement tracking**: `phg explain E-TRANSPILE-FS` → "**RETIRED**
   (DEC-313, 2026-07-22): `Core.FileSystemModule` now transpiles." A diagnostic catalogue that
   records its own retirements is unusual discipline. Also correct: `E-PKG-CASE`,
   `E-CONCURRENCY-NO-PHP`, `W-DEPRECATED`, `E-INJECTED-TYPE-BARE`. [Verified: ran all five.]
3. **The example corpus** — 266 `.phg` files; I ran 12 spread across `guide/`, `database/`, `fs/`,
   and all 12 exited 0 with sensible output (`concurrency.phg` → "9 squared = 81";
   `crypto-mac.phg` → a real HMAC; `datetimes.phg` → an ISO timestamp). Invariant 9's
   examples-ship-with-features rule is being honoured in practice, and the corpus survived the
   breaking DEC-331 entry-point change. [Verified: 12 runs, all exit 0.]
4. **`KNOWN_ISSUES.md`** — exemplary honesty, and the *source* of H2 rather than its victim. Its
   STACKDEPTH entry (`:158-172`) states severity, a `[Verified: reproduced + traced]` grade, the
   exact recursion site (`src/checker/enforce_injected.rs` `walk_expr`), why `MAX_EXPR_DEPTH` is
   bypassed, and an explicit "**NOT a regression**" analysis of the DEC-337 relationship. INVARIANTS
   §6 should simply link to it.
5. **`docs/DEPRECATION.md`** — 43 lines, self-consistent, and volunteers its own weakness: "The
   table is **empty in the shipping build** today — the mechanism is in place ahead of the first real
   deprecation (a `#[cfg(test)]` sample exercises the lint end-to-end)." Verified live via
   `phg explain W-DEPRECATED`. A doc that discloses "mechanism ready, no contents yet" is more
   trustworthy than one that implies coverage.
6. **The W5-13 deferral chain** (H29) — `docs/INVARIANTS.md:94-96` ↔ `tests/differential.rs:258,260`
   agree exactly, and the deferral is pinned by a live `#[ignore]`d test rather than prose. The
   correct pattern for every disclosed exception.
7. **The 2026-07-25 SSOT-repair loop** (H30) — nine findings raised and **all nine closed** within
   one day, verified independently. `C-decisions.md` advanced 335 → 338 with the Q-A/Q-B cluster
   properly registered. The Invariant-19 machinery works well on the current slice; the gap is that
   it never sweeps the standing corpus (which is where §1–§5 of this report lives).
8. **`STABILITY.md` / `SEMVER.md` / `conformance/`** — a real three-tier stability model,
   per-construct stable/experimental lists, and a 64-file conformance corpus wired as a CI gate
   (`STABILITY.md:8`). Notably these are the *very* deliverables `GA-CHECKLIST.md:17` still lists as
   "Missing" (H18) — the work landed; only the scorecard didn't notice.
9. **Markdown link hygiene is perfect** — `0` dangling `[text](path)` links across **all 175** `.md`
   files [Verified: exhaustive extraction + `test -e`]. Every defect in H46 is a *backticked source
   path* or an anchor slug, never a link. Whatever discipline produces the links is working; it simply
   was never extended to inline code citations.
10. **Nineteen files are entirely reference-clean** — `README.md`, `CONTRIBUTING.md`, `FEATURES.md`,
   `docs/ARCHITECTURE.md`, `docs/EXTENSIONS{,-AUTHORING}.md`, `docs/GA-CHECKLIST.md`,
   `docs/DEPRECATION.md`, `docs/HISTORY.md`, `SEMVER.md`, `STABILITY.md`, `SECURITY.md`,
   `ROADMAP.md`, `VISION.md`, `conformance/README.md`, `selftest/README.md`, `editors/**/README.md`,
   `playground/README.md`, `docs/adr/0002`–`0005`. Four of the ten high-traffic files I was asked to
   prioritise are clean; the rot is *concentrated*, not diffuse — which makes it tractable.
11. **`M-gap-matrix.md:897-899` actively prevents a metric double-count** — when §4.12's full 631-row
   re-tally produced a *simple-model* 44.1% against §4.11's *weighted* 49.1%, the doc explicitly
   reconciled them ("headline PHP-parity ≈68% … **UNCHANGED** by this re-tally"; "simple 44% <
   weighted 49%") instead of quietly adopting the more flattering number. That is unusual statistical
   honesty in a self-scored project, and it is why H42 is only P2 — the *arithmetic* is sound; only
   some *banners* are missing.
12. **`docs/specs/UNIFIED-SPEC.md:1087`** correctly disclaims its own obsolete figures ("the 'GA ~72% ·
   Global ~58%' figures are obsolete; the live model is M-gap-matrix §4") and points at the *section*
   rather than pinning a sub-revision — the pattern `ROADMAP.md:7` should copy (H42d).
13. **The `✅ SHIPPED <date>` convention already exists and works** where applied — MASTER-PLAN's
   Tier-2 queue items 7/8/11/13/14/15/16 and UNIFIED-SPEC's DEC-239/DEC-257 rows all carry it
   correctly. H39 is therefore not a missing convention but an unapplied one, which makes it a
   mechanical fix rather than a design question.
14. **The `2026-07-24-visibility-model.md` spec is the model status-header** —
   `:1` "# SPEC (RULED — **BUILT**, 2026-07-25)" with a matching `## BUILD STATUS` section and an
   explicit "(SUPERSEDED 2026-07-25 — see the follow-up below)" annotation on its own retracted
   carve-out. Its sibling wildcard spec (H35) diverges from this good pattern; the pattern itself is
   sound and should simply be enforced.

---

## The single worst documentation-integrity problem

Not any one finding: **status is recorded in more places than are kept in sync, and the sync loop only
sweeps the current slice.** The evidence is that the 2026-07-25 nightly loop closed **9 of 9**
Invariant-19 findings within a day (H30 — genuinely excellent), while the *standing* corpus
accumulated: 2 P0s, ~14 P1s, 60+ dangling references, 13 unregistered DEC ids, a reversed ADR still
marked Accepted, and a shipped-security queue still labelled "do first". Invariant 19 is being
enforced *forward* and not *backward*.

The mechanical remedy is narrow and cheap: **(1)** a markdown reference checker in `pre-push`
(H48 — closes ~60 findings and prevents the class); **(2)** a DEC-set consistency check — every
`DEC-\d+` referenced in any `.md` must have exactly one register row (H40); **(3)** cursors record
`origin/master` + subject, never a bare short SHA (H34). The judgement calls that remain — the
GA re-score (H18), Invariant 17's lift carve-out (H9), whether `--sequential-concurrency` and
`Core.Sandbox` get built (H5/H36) — are **all developer rulings under Invariant 15, and this report
deliberately does not make them.**

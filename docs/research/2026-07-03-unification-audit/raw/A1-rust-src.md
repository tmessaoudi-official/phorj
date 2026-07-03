# A1 — Rust `src/` Structural Audit (unification audit, 2026-07-03)

> HEAD `0691228`, clean tree [Verified: `git rev-parse` + `git status`]. Method: fresh grep/read
> sweeps over `src/` (220 `.rs` files, 84,227 total lines) + independent re-verification of every
> Rust-relevant seed finding from `full-audit/raw/A-craftsmanship.md`, `full-audit/raw/H-enforcement.md`,
> and `2026-07-03-corpus-audit.md` against current lines. Read-only — no code modified.
>
> **Certification disclosure:** this agent's tool set has no `advisor()`; per the Phase 3C/6C
> subagent carve-out, the three-lens check (completeness / adversarial / blast-radius) was run
> **self-graded**. Mitigations: variant coverage computed by set-difference (`comm`), not eyeball;
> every wildcard arm context-read; every seed claim re-checked against current line content.

## Headline

`src/` is structurally clean: **zero stubs, zero TODO/FIXME markers, zero dead-code allows, all
hard invariants (Op coupling, unsafe-forbid, kernel single-sourcing, dependency policy) hold at
HEAD.** Every P0/P1 from the prior audits that touched `src/` is now FIXED and verified in code.
What remains open is a small set of already-known P2/P3 polish items (VM hot-path allocations,
DAP/LSP framing robustness, two untested pure-logic modules) plus a tracked-but-unenforced
file-size rule (12 files over the 1000-line hard cap, `scripts/size-gate.sh` not yet built — W1-6),
and a handful of stale doc-comments referencing dead diagnostic codes.

---

## 1. Markers & stubs — CLEAN

| Check | Result | Evidence |
|---|---|---|
| `TODO`/`FIXME`/`XXX` | **0 real** — the only 3 grep hits are doc comments about `\uXXXX` JSON escapes (`src/native/json.rs:379`, `src/diagnostic.rs:253`, `src/json.rs:173`) | [Verified: grep + read of each hit] |
| `todo!()` / `unimplemented!()` | **0** in all of `src/` | [Verified: grep, zero hits] |
| Bare `unreachable!()` (no justification msg) | **0** — the four bare sites A-ERR-1 flagged (`compiler/mod.rs:808,829`, `compiler/expr.rs:524,541`) are gone; every remaining `unreachable!` carries a message | [Verified: `grep -rn 'unreachable!()' src/` → zero hits] |
| `panic!` outside test code | **0** — only 3 sites total outside `*tests.rs` files (`value.rs:1654,1948`, `manifest.rs:585`) and all three sit inside inline `#[cfg(test)]` modules (test mod starts value.rs:1457 / manifest.rs:398) | [Verified: grep + awk cfg(test)-position check] |
| `#[allow(dead_code)]` / `#[allow(unused…)]` | **0** in `src/` | [Verified: grep, zero hits] |

## 2. Op-variant match coupling (INVARIANTS #5 / CLAUDE.md rule 3) — HOLDS

- `Op` enum: **73 variants** (`src/chunk.rs:82–331`) [Verified: awk extraction of the enum body].
- All 73 appear in each of the three match sites; set-difference (`comm -23`) against each site's
  `Op::` mentions is **empty** for all three [Verified: scripted extraction + comm]:
  - `vm::Vm::exec_op` — `src/vm/exec.rs:9`
  - `BytecodeProgram::validate` — `src/chunk.rs:487`
  - `Compiler::stack_effect` — `src/compiler/mod.rs:488` (function ends before :566)
- **Wildcard-free confirmed.** The `_ =>` arms found in those files are all in *nested non-Op*
  matches: `vm/exec.rs:759` matches `ClosureData` (inside `CallValue`), `chunk.rs:36` matches
  `Value` in `ConstKey::of`, and every `compiler/mod.rs` wildcard (322, 331, 402, 636–860) lies
  outside the 488–565 `stack_effect` span, in the `CTy`-inference helpers matching `Expr`
  [Verified: context reads of each wildcard site + line-range check].

## 3. `#![forbid(unsafe_code)]` — HOLDS (with one note)

- `src/lib.rs:3` and `src/main.rs:3` both carry it [Verified: read]. This satisfies INVARIANTS #10's
  "both crate roots" (the phorj lib + bin roots).
- **Note (P3, informational):** `playground/src/lib.rs` (the separate workspace member) has **no**
  `forbid(unsafe_code)` attribute [Verified: grep, zero hits]. Not an invariant violation as written
  (INVARIANTS #10 names the phorj crate roots), and `#[wasm_bindgen]`-generated code may not compile
  under `forbid` — but if the playground is meant to inherit the guarantee, that's currently only by
  convention, not by attribute. Flag for the developer to rule on. — [Verified: grep; the
  wasm-bindgen-compatibility rationale is [Speculative]]

## 4. Dependency policy — COMPLIANT

- Core `phorj` crate: exactly the four vetted, feature-gated exceptions — `argon2` (crypto),
  `regex`, `ctrlc` (signals), `corosensei` (green, non-wasm32 target-gated); each documented inline
  with its policy rationale; `default = ["crypto","regex","signals","green"]` [Verified: full
  `Cargo.toml` read].
- `rusqlite`/`rustls`: **not present** in `Cargo.toml`/`Cargo.lock` — consistent with
  approved-but-not-yet-shipped status [Verified: grep, zero hits].
- Playground member: `phorj` (path, default-features off) + `serde_json` + wasm32-only
  `wasm-bindgen` [Verified: `playground/Cargo.toml` read].
- **P3 (stale comment):** `Cargo.toml:83–85` says "wasm-bindgen (the **only external crate** in the
  project) lives solely in that member" — false since `serde_json` is also a playground dependency,
  and the core crate itself has four. Same claim family as corpus-audit B1. Also: neither
  `serde_json` nor `wasm-bindgen` is mentioned in `docs/specs/2026-06-27-dependency-policy.md`
  [Verified: grep, zero hits] — the playground member's deps are policy-undocumented.

## 5. File-size anti-regrowth (CLAUDE.md rule 13 / MASTER-PLAN G-6) — RULE ADOPTED, GATE NOT BUILT

The ratified rule (MASTER-PLAN.md:117–120): soft cap **800 production lines** (inline `#[cfg(test)]`
excluded), hard review trigger **1000 with a tracked exemption**, enforced by `scripts/size-gate.sh`
in CI (**W1-6**). At HEAD:

- **`scripts/size-gate.sh` does not exist** (`scripts/` contains only `git-hooks/` + `perf-gate.sh`)
  [Verified: ls]. W1-6 is a tracked planned item (MASTER-PLAN.md:358–359), not silent drift — but
  until it lands the rule is unenforced and there is **no exemption register**.
- **12 files over the 1000 hard cap by production lines** (lines before the first `#[cfg(test)]`;
  method note: assumes trailing test mods, which is this codebase's convention) [Verified: scripted
  awk count]:

| Production lines | File |
|---|---|
| 2746 | `src/transpile/program.rs` |
| 2114 | `src/checker/calls.rs` |
| 2034 | `src/checker/collect.rs` |
| 1474 | `src/cli/mod.rs` |
| 1470 | `src/fmt/printer.rs` |
| 1456 | `src/value.rs` |
| 1345 | `src/cli/explain.rs` (pure code→text table; A-DES-1 called it exempt-by-nature) |
| 1312 | `src/lift/parser.rs` |
| 1222 | `src/lexer/mod.rs` |
| 1170 | `src/checker/expr.rs` |
| 1075 | `src/compiler/expr.rs` |
| 1051 | `src/compiler/mod.rs` |

  (`src/interpreter/mod.rs` sits exactly at 1000 — at the trigger, not over.) Between the soft and
  hard caps: `checker/program.rs` 977, `ast/classes.rs` 968, `parser/items.rs` 962, `ast/mod.rs` 926,
  `checker/stmt.rs` 916, `transpile/mod.rs` 911, and ~8 more.
- **Severity: P2** — the whale files predate the 2026-07-02 ratification and the adopted "23-split
  decomposition" covers them; the actionable gap is *ship W1-6* (gate + exemption list), otherwise
  the anti-regrowth rule cannot bite.

## 6. Value-kernel single-sourcing (INVARIANTS #3/#4) — HOLDS

- Zero `checked_add/sub/mul/div/rem/neg` call sites in `src/vm/`, `src/interpreter/`,
  `src/transpile/` [Verified: grep, zero hits] — all checked arithmetic flows through the
  `value.rs` kernels.
- Corroborates A-DES-2 (kernels defined once at `value.rs`; backends call them). No drift found.

## 7. Seed-finding re-verification (what moved since 2026-07-02)

### 7a. FIXED at HEAD (verified in current code — do not re-report)

| Seed | Was | Now |
|---|---|---|
| **A-CI-1 (P0)** stale absolute `core.hooksPath` | gate void | `core.hooksPath = scripts/git-hooks` (relative, rename-proof) [Verified: `git config --show-origin`] |
| **A-ERR-1 (P3)** 4 bare `unreachable!()` | no messages | zero bare sites remain [Verified: grep] |
| **A-SEC-3 (P3)** `u64 as usize` truncation | `container.rs:74` | now `usize::try_from(...).ok()?` with an explanatory comment (`src/bundle/container.rs:74–77`) [Verified: read] |
| **H P0** private/protected static-field visibility hole | unenforced | `E-FIELD-VISIBILITY` gated on read (`checker/calls.rs:1630,1795`) + write (`checker/assign.rs:226`), with tests (`checker/tests/visibility.rs:19`) [Verified: grep+read] — W0-2 |
| **H P1** reserved-`Core`/pkg-case dead in project mode | loader never checked | per-file gates in `src/loader/fs.rs:85–103` (`E-RESERVED-PACKAGE`, `E-PKG-CASE`) [Verified: read] — W0-4 |
| **H P2** `E-ALIAS-CYCLE` uncoded + lazy | uncoded | raised at collect time, deduped, coded (`src/checker/collect.rs:154–219`) [Verified: read] — W0-4 |
| **Corpus C4** `E-FIELD-INIT` "verify not a prefix" | suspected dead | it WAS a prefix — `E-FIELD-INIT-FORWARD-REF`/`E-FIELD-INIT-TYPE` both live in explain (`cli/explain.rs:1071,1087`) [Verified: grep] |

### 7b. STILL OPEN at HEAD (re-verified against current lines)

| # | Sev | Finding | Location | Grade |
|---|---|---|---|---|
| 1 | P2 | DAP transport write errors silently swallowed (`let _ = write!` + `let _ = flush`): broken client pipe ⇒ all subsequent responses lost while session continues | `src/dap.rs:50–51` | [Verified: read] |
| 2 | P2 | Malformed `Content-Length` handled non-loudly in both framers: DAP maps it to `None` ⇒ session silently ends as if EOF (`src/dap.rs:90,93`); LSP maps it to "empty body, ignored upstream" ⇒ the unconsumed real body is then parsed as the next message's headers — stream desync (`src/lsp/mod.rs:663–669`) | `src/dap.rs:90`, `src/lsp/mod.rs:663–669` | [Verified: read of both framers] |
| 3 | P2 | `Op::CallMethod` allocates two fresh `String`s (`names[name_idx].clone()` + `inst.class.clone()`) per method call — no inline cache, unlike `GetField` | `src/vm/exec.rs:665–676` | [Verified: read] |
| 4 | P2 | `Op::CallValue` deep-clones the whole `ClosureData` (incl. captures `Vec`) per closure invocation (`cd.as_ref().clone()`) — paid per element by higher-order stdlib | `src/vm/exec.rs:757–759` | [Verified: read] |
| 5 | P2 | Interpreter static/const member read builds a `(String,String)` clone key per access (twice on the miss path) | `src/interpreter/expr.rs:119–123` | [Verified: read] |
| 6 | P2 | Zero direct tests for two pure-logic modules: `src/dispatch.rs` (overload selection) and `src/json.rs` (DAP/LSP JSON framing) — both `#[test]`-count 0; exercised only end-to-end | `src/dispatch.rs`, `src/json.rs` | [Verified: `grep -c '#[test]'` = 0/0] |
| 7 | P2 | File-size rule adopted but unenforced: 12 files > 1000 production lines, no `size-gate.sh`, no exemption register (§5 above; tracked as W1-6) | see §5 | [Verified: ls + scripted count] |
| 8 | P3 | Overload candidate signature vectors rebuilt (`k.clone()).collect()`) on every overloaded call | `src/vm/exec.rs:453,683` | [Verified: grep+read] |
| 9 | P3 | Bytes literal deep-copied + fresh `Rc` per evaluation in the interpreter (`Value::Bytes(Rc::new(b.clone()))`) — a `b"…"` in a loop clones per iteration; VM const pool unaffected | `src/interpreter/expr.rs:36` | [Verified: read] |
| 10 | P3 | Two production `Result::expect` on scheduler invariants (internal-invariant panics, not user-input-reachable; the only prod panic paths between a scheduler regression and a user crash) | `src/interpreter/coop.rs:103`, `src/green/exec.rs:131` | [Verified: grep+read; `green/exec.rs:170` is test-mock, excluded] |
| 11 | P3 | `E-OVERLOAD-SELECT-CONFLICT` registered in explain but never raised anywhere in `src/` (its own explain text says "raised once the inferable sinks land") | `src/cli/explain.rs:645` (only occurrence) | [Verified: grep — no raise site] |

### 7c. NEW findings (this sweep)

| # | Sev | Finding | Location | Grade |
|---|---|---|---|---|
| 12 | P3 | `E-IMPORT-BUILTIN` and `E-IMPORT-SHADOW` have explain entries but **no literal raise site** outside `cli/explain.rs` — post-S0 re-homing of the old `E-TYPE-IMPORT-*` pair appears to have kept the registry entries but (at least as quoted strings) dropped the raise sites. Needs a runtime probe (`import int;`) to confirm dead vs dynamically-constructed | `src/cli/explain.rs:1311,1322`; raise sites: none found | [Verified: grep for the quoted strings outside explain.rs → zero] / conclusion [Inferred] |
| 13 | P3 | Stale `src/` doc-comments referencing **removed** syntax/codes: `src/loader/mod.rs:617` ("An `import type` naming one of these is `E-TYPE-IMPORT-BUILTIN`" — `import type` was removed in S0; code renamed), `src/loader/resolve.rs:590` (references `E-TYPE-IMPORT-SHADOW`), `src/value.rs:596` (claims "the checker rejects a non-literal static initializer (`E-STATIC-INIT-CONST`)" — that code exists nowhere in `src/`; the literal-initializer rejections in `checker/collect.rs:1243,1692` carry no such code) | as listed | [Verified: grep + reads] |
| 14 | P3 | `Cargo.toml:83–85` workspace comment claims wasm-bindgen is "the only external crate in the project" — stale (playground also depends on `serde_json`; core has 4 vetted deps); same false-claim family as corpus-audit B1 | `Cargo.toml:83–85` | [Verified: read] |
| 15 | P3 | `playground/src/lib.rs` has no `#![forbid(unsafe_code)]` (letter-of-invariant compliant — INVARIANTS #10 names the phorj crate roots — but the guarantee doesn't extend to the workspace member by attribute) | `playground/src/lib.rs:1` | [Verified: grep] |

## 8. Clean-check register (explicitly verified, not just absence-of-finding)

1. Stub markers: 0 (§1). 2. Op coupling: 73/73/73/73, wildcard-free (§2). 3. `forbid(unsafe_code)`
on both phorj crate roots (§3). 4. Dependencies exactly per policy; rusqlite/rustls not yet added
(§4). 5. Value kernels single-sourced — no `checked_*` in any backend (§6). 6. No
`#[allow(dead_code)]`/`#[allow(unused)]` anywhere in `src/` (§1). 7. All prior-audit P0/P1s
touching `src/` verified fixed in current code (§7a).

## Summary

| Severity | Count | Items |
|---|---|---|
| P0 | 0 | — |
| P1 | 0 | — |
| P2 | 7 | #1–#7 (DAP write-swallow; framing robustness; 3 VM/interp hot-path allocation patterns; 2 untested pure-logic modules; size-gate unshipped) |
| P3 | 8 | #8–#15 (2 minor perf; 2 prod expects; 3 dead/stale diagnostic-code references; 1 stale Cargo.toml claim; playground forbid note) |

All P2s and most P3s were already known from the 2026-07-02 audits and are re-confirmed still open;
genuinely new this sweep: #12–#15 (all P3, all doc/registry hygiene). No invariant violation found.

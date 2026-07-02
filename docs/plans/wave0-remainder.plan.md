# Wave-0 Remainder Plan

> Autonomous session 2026-07-03. Completes the RULED §12 Wave-0 tail + records XML (W4-10) as a
> PENDING adjudication artifact (§15 — user-visible surface is the developer's, not ruled alone).
> SSOT = `docs/plans/MASTER-PLAN.md`. Gate = full PHP-oracle `cargo test --workspace` + clippy + fmt + build.

## Decisions Log
- [2026-07-03] AGREED (framework, advisor-confirmed): this session builds the **Wave-0 remainder**
  (W0-6b, W0-9, W0-10 — all RULED §12, mechanical, no adjudication) autonomously; **XML (W4-10) is
  NOT built** — it is `DESIGN-NEEDED` (user-visible surface), so it is recorded as a PENDING
  adjudication question surfaced at session close. Rationale: top-down wave order (0→6) + §15.
- [2026-07-03] VERIFIED divergences from stale roadmap markers (Rule 11): W0-6b(d) codemods already
  deleted; examples/README "71 KB" = bytes not lines (289 long lines, real 72 KB monolith);
  CI supply-chain pins (A-CI-4/5/6) need external data — verify or record, never guess.

## Formal Plan

### W0-10 — P2 hardening batch (code half first: local + testable)
- A-SEC-3: `src/bundle/container.rs:74` — `u64 as usize` → `usize::try_from(...).ok()?` (32-bit truncation).
- A-ERR-1: 4 bare `unreachable!()` → justification messages (`compiler/mod.rs:808,829`, `compiler/expr.rs:524,541`).
- A-ERR-3: DAP write errors — track `dead: bool`, end session on write failure (`src/dap.rs`).
- A-ERR-4: malformed `Content-Length` in DAP + LSP framing → protocol error/close, never silent desync.
- A-ED-2: commit VS Code extension `package-lock.json`.
- A-SEC-7: document `W-SECRET` one-hop limitation in its explain text.
- A-TEST-1: unit tests — BOTH halves: `native/json.rs` + `dispatch.rs`/`select_overload` ambiguity/no-match edges.
- CI half (A-CI-4 wasm-pack pin / A-CI-5 zig checksum + action SHA pin / A-CI-6 reorder): verify
  external data (WebFetch) or record as precise PENDING; never guess a pin/SHA that could break CI.

### W0-6b — front door
- (b) CLI-verb rename `bench/fmt/disasm/lex → benchmark/format/disassemble/tokenize` across the 12
  enumerated files ONLY (README CLI table+refs, CONTRIBUTING, examples/bench, examples/README,
  examples/cli, editors/*/README ×3, docs/GA-CHECKLIST, docs/MILESTONES). Context-shift guard: do NOT
  touch pipeline-stage words ("lex → parse"), "on lex error", or the `docs/specs`/`docs/research` record layer.
- (c) CI doc-snippet check: extract ```phorj fences from README + doc READMEs, `phg check` each, fail-closed.
- (d) codemod deletion — DONE (verified absent).

### W0-9 — housekeeping
- (a) delete 2 dangling worktree-agent branches.
- (b) `dist/` stale binaries — local `rm`, note in commit.
- (c) KNOWN_ISSUES.md prune (1133 lines) — move resolved → CHANGELOG.
- (d) examples/README restructure (72 KB) → thin root index + per-dir READMEs; fix index gaps.

### XML (W4-10) — RECORD ONLY, do not build
- Write the design proposal + minimal failing program + option previews; surface as closing AskUserQuestion.

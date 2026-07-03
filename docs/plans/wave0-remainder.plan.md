# Wave-0 Remainder Plan

> Autonomous session 2026-07-03. Completes the RULED §12 Wave-0 tail + records XML (W4-10) as a
> PENDING adjudication artifact (§15 — user-visible surface is the developer's, not ruled alone).
> SSOT = `docs/plans/MASTER-PLAN.md`. Gate = full PHP-oracle `cargo test --workspace` + clippy + fmt + build.

## Decisions Log
- [2026-07-03] AGREED (developer, interactive — SUPERSEDES this session's primary focus):
  **Injected `Core.Http` names become qualification-required** (Option 2). Concretely:
  (1) default usage must be QUALIFIED: `Http.Router`, `Http.Request`, `Http.Response`, and the
      attribute `#[Http.Route(...)]` — bare `Router`/`Route`/etc. after a plain `import Core.Http`
      becomes an error (mirror of `E-INJECTED-VARIANT-BARE`).
  (2) NEW targeted **member-import** form `import Core.Http.Router;` (three-level) brings the leaf
      `Router` into bare scope — so `Router rt` works ONLY when the member is explicitly imported.
  (3) Parity consequence flagged by developer: bare `Json` type name would then be the inconsistent
      one — apply the same treatment for consistency (investigate scope).
  (4) Developer directive: **inspect ALL code + ALL compiler/interpreter code** for affected sites.
  Rationale: "everything should be imported" — nothing usable in the wind; explicit is the rule.
- [2026-07-03] AGREED (developer): **Functions/natives are NOT bare-importable** (challenge accepted).
  They stay module-qualified (`String.trim(s)`) or UFCS (`s.trim()`) — always traceable. Only TYPES
  (class/enum/interface/trait) are bare-importable. Preserves "nothing in the wind" + DEC-087 method-first.
- [2026-07-03] FINDING: Phorj already has the member-import — `import type Pkg.Path.Type` binds a bare
  type name for class/enum/interface/trait across USER packages (18 sites; `shapes/`, `mixins/`). Gaps:
  (1) Core injected types excluded; (2) `type` keyword vs the bare `import Core.Http.Router` the dev typed;
  (3) plain `import` binds a call-qualifier, `import type` binds a bare type — different purposes today.
- [2026-07-03] AGREED (developer — FULL MODEL LOCKED). The import/namespace redesign:
  1. **Unify `import`; DROP `import type` entirely (no back-compat, migrate all 18 sites).** The resolver
     classifies each import by resolving its path: → module ⇒ bind call-qualifier; → type
     (class/enum/interface/trait) ⇒ bind bare name; → neither ⇒ error.
  2. **Injected Core types get import discipline** (the bug found): qualified-by-leaf DEFAULT —
     `Http.Router/Request/Response`, `#[Http.Route]`, `Time.Duration/Date/Instant`, `Decimal.RoundingMode`;
     bare ONLY via member-import (`import Core.Http.Router;`); new `E-INJECTED-TYPE-BARE` (mirror of
     `E-INJECTED-VARIANT-BARE`). Requires NEW qualified-type resolution `Qualifier.Type` in type position.
  3. **Single-type modules (Json/Regex/Secret) compliant as-is** (leaf==type); variants stay `Json.Object`.
  4. **Functions NOT bare-importable** (qualified/UFCS only). **No associated functions** (`MyClass.fn(x)`
     is NOT a free-fn namespace; use `x.fn()` UFCS or a static method).

  ### Implementation slices (each gates green + commits independently)
  - **S0 — Unify import ✅ SHIPPED `11a6c71`**: parser dropped `type` keyword; loader classifies
    module-vs-type by path (`is_type_import_path`; Core skipped in `build_type_imports`);
    `E-TYPE-IMPORT-*` → `E-IMPORT-*`; UNKNOWN preserved via known-package heuristic; 18 sites + 4 tests
    migrated. Full oracle gate green. Vestigial `type_only` field + stale `import type` prose comments
    pending S2 cleanup.
  - **S1 — Qualified type refs**: parser + AST + checker resolve `Qualifier.Type` in type position
    (`Http.Router` as an annotation) → the injected type; transpiler erases qualifier (PHP stays bare).
  - **S2 — Injected-type discipline**: `E-INJECTED-TYPE-BARE`; member-import for Core injected types;
    `#[Http.Route]` qualified attribute (parser dotted attr name + desugar_router match); migrate the
    injected preludes' user-facing surface + ~40 .phg (examples/conformance) + docs to qualified/member form.
- [2026-07-03] AGREED (framework, advisor-confirmed): this session builds the **Wave-0 remainder**
  (W0-6b, W0-9, W0-10 — all RULED §12, mechanical, no adjudication) autonomously; **XML (W4-10) is
  NOT built** — it is `DESIGN-NEEDED` (user-visible surface), so it is recorded as a PENDING
  adjudication question surfaced at session close. Rationale: top-down wave order (0→6) + §15.
- [2026-07-03] VERIFIED divergences from stale roadmap markers (Rule 11): W0-6b(d) codemods already
  deleted; examples/README "71 KB" = bytes not lines (289 long lines, real 72 KB monolith);
  CI supply-chain pins (A-CI-4/5/6) need external data — verify or record, never guess.

## Dependency-policy amendment (AGREED 2026-07-03, developer)
- **APPROVED:** admit `rusqlite` (SQLite) + `rustls` (TLS) as new vetted domains — native-only,
  feature-gated (`db`/`tls`, off in WASM playground), spine-quarantined (corosensei/ctrlc shape),
  `#![forbid(unsafe_code)]` intact in phorj's own code. Unblocks native `Core.Db` + HTTPS client.
  Ships pure zero-dep P0s first (`Core.Sql`, `Core.Url`). Requires editing `docs/specs/2026-06-27-dependency-policy.md`.
- **DB engine scope RULED (developer, 2026-07-03):** a multi-driver **SQL DBAL** (data-access layer,
  PDO/Doctrine-DBAL analog) — **SQLite** (P1, rusqlite, embedded) + **Postgres** (`postgres` sync crate
  → PDO_PGSQL) + **MySQL/MariaDB** (`mysql` sync crate → PDO_MYSQL; one driver both). ALL sync drivers
  (no tokio — async runtimes stay policy-rejected). **Oracle DEFERRED** (closed-source Instant Client
  violates policy clause 2). **MongoDB ACCEPTED as a SEPARATE LADDER item** (non-SQL/no-PDO → native-only
  `E-TRANSPILE-MONGO`, no PHP leg; async-driver problem) — its own future XL design, NOT part of the SQL DBAL.
- **The three parallel designs = the "three things":** (1) SQL DBAL [W3-1], (2) HTTP client [W3-2],
  (3) **Unicode-correct strings [W4-4]** (codepoints default; case-folding is the LADDER-quarantine
  landmine; ~12/35 Core.String natives change). All 3 drafts in `docs/research/wave3-4-drafts/`.

## Delivery order (AGREED 2026-07-03, developer — Option 1 ROI-first)
Ledger basis (§10): W3 web-spine 59%→65%, W4 bridge 65%→71% are the +12 parity points.
1. Import redesign **S1 → S2** (finish in-flight; fixes the `Route`-in-the-wind bug).
2. **W3-4** CSPRNG/HMAC/KDF (M) + **W3-5** sprintf/`Core.Fmt` (M) — high value-per-effort.
3. **W3-1** DB access (XL) + **W3-2** HTTP client (XL) — the heart of "real apps in Phorj".
4. **W3-3/6/8** — finish the web spine.
5. **W4-4** Unicode-correct strings (XL, PHP-is-wrong correctness fix).
6. **W4-6** stdlib blitz (L) + **W4-5** date/time (L).
7. rest of W4 → W2 polish → W5 beyond-PHP → W6 GA; W0/W1 hygiene folded in opportunistically.
Also queued (developer, 2026-07-03): **playground example expansion** (curated breadth beyond
examples/guide/ — see challenge re: conformance-are-tests + WASM-run safety).

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

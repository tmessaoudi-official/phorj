# dependency refresh (2026-11-19) Plan

> Developer directive, 2026-11-19 (mid-session, during GENERIC-PAYLOAD-OPERAND): *"Check for all the toolchain and any
> version/pin/package updates/upgrades !"*. Precedent: `docs/plans/2026-09-29-dependency-refresh.plan.md` (same
> directive; ruled shape: a compatible lock refresh + floor raises in one commit, each semver-breaking bump alone, one
> gate on the frozen tree, cargo-audit reinstalled, edition 2024 excluded). Inventory below is from a read-only survey
> agent (its commands listed in the session; re-checked at apply time — a version is a claim until `cargo` resolves it).
> Defects-first (north star rule 1): the unrecorded advisories are SECURITY defects, so this batch runs right after
> GENERIC-PAYLOAD-OPERAND lands, ahead of the next feature-free defect.

## Decisions Log
- [2026-11-19 15:59] AGREED: rusqlite -> 0.43 latest (developer, AskUserQuestion 2026-11-19): closes RUSTSEC-2026-0459; 0.43 rejects empty / comment-only SQL, a user-visible Core.Sql change, to be pinned by a three-leg test and disclosed.
- [2026-11-19 15:59] AGREED: fancy-regex -> 0.20.2 regardless (developer: 'Aim for Option 3 ! and tell me what are the risks !'): bump with pin tests of the new behaviour; risks reported to the developer. Pre-bump probe: (?=(a+))a\1$ on 'aaa' is false on all three legs under 0.19 (already PCRE-like there).
- [2026-11-19 15:59] AGREED: semver-breaking bumps in this batch (developer, multi-select): Rust 1.98.1 -> 1.100.0, cranelift 0.136 -> 0.137, vscode-languageclient 11 + php-wasm 0.2; each its own commit. zig 0.17 not selected (re-asked with the PHP floor question).
- [2026-11-19 16:01] AGREED: PHP floor stays 8.5 'for now' (developer, AskUserQuestion 2026-11-19, option 1): local oracle php-8.5.13 (phpbrew build, after the code commits); CI gate on 8.5 AND 8.6 GA (both gating); non-gating nightly canary on 8.7-dev (relabel the stale '8.6-dev' job). Facts: 8.6.0 + 8.5.13 released 2026-11-19 (php.net JSON); local php-master = 8.7.0-dev.
- [2026-11-19 16:02] AGREED: zig 0.16 -> 0.17.1 and drop mlugg/setup-zig@v2 (node20) for a direct sha-checked install, as ci.yml does (developer, AskUserQuestion 2026-11-19).

## Inventory (survey 2026-11-19)
### Security (RustSec via OSV over all 308 locked crates; `cargo-audit` not installed)
| crate@locked | advisory | fixed in | compiled in | in KNOWN_ISSUES |
|---|---|---|---|---|
| rustls 0.23.45 | RUSTSEC-2026-0346/0347/0348/0349 | 0.23.46 | http-client, http-server-tls, mail | no |
| rustls-webpki 0.103.15 | RUSTSEC-2026-0382 | 0.103.16 | same | no |
| webpki-roots 1.0.9 | RUSTSEC-2026-0381 | 1.0.10 | same | no |
| lettre 0.11.23 | RUSTSEC-2026-0560…0565 | 0.11.24 | mail | no |
| libsqlite3-sys 0.38.2 | RUSTSEC-2026-0459 (bundled SQLite) | 0.39 = rusqlite 0.41+ (semver-incompatible) | DEFAULT `database` | no |
| ctutils 0.4.2 | RUSTSEC-2026-0385 | 0.4.4 | DEFAULT `cryptography` (argon2 → blake2 → digest) | no |
| tokio 1.53.1 | RUSTSEC-2026-0386 | 1.53.3 | database-postgres | no |
| rustix 1.1.5 | RUSTSEC-2026-0486 | 1.1.6 | no host path | no |
| pin-utils 0.1.0 | RUSTSEC-2026-0441 (unmaintained) | — | no (async-std) | no |
- Yanked in `Cargo.lock`: async-executor 1.14.0, yoke-derive 0.8.3 (the lock refresh replaces both).
- `editors/vscode/package-lock.json`: brace-expansion 5.0.9 (HIGH, fixed 5.0.12, in range); minimatch 10.2.6 (moderate, fixed only in 11.0.1, blocked by vscode-languageclient's `^10.2.6`).
- PHP oracle: local php-8.5.12; 8.5.13 (security) released 2026-11-19. `scripts/toolchain.env` globs `php-8.5.*`, so an install needs no repo edit.
- CI: `mlugg/setup-zig@v2` (`.github/workflows/stub-registry.yml`) runs on the deprecated node20 runtime; no node24 release upstream.
### Lock-compatible (one commit)
`cargo update`: 95 packages, none declaring an MSRV above 1.98.1. Floors to raise so a fresh resolve cannot go back:
rustls 0.23.46, webpki-roots 1.0.10+, lettre 0.11.24 (mysql 28.0.4 optional). Tools: cargo-nextest 0.9.149 → 0.9.152
(`.claude/cloud-setup.sh`, sha256 with it), wasm-pack v0.15.0 → v0.15.1 (`.github/workflows/playground.yml`, sha256
with it), vendored CodeMirror patch/minor rebuild (`playground/web/vendor/README.md`), VS Code lockfile in-range refresh.
### Semver-incompatible / decisions
| item | now → latest | note |
|---|---|---|
| Rust toolchain | 1.98.1 → 1.100.0 | `warnings = "deny"` + clippy deny: new lints may need fixes |
| cranelift ×3 | 0.136.1 → 0.137.2 (MSRV 1.97) | JIT re-verification; `src/jit/compile/mod.rs` cites 0.136.1 |
| rusqlite | 0.40.2 → 0.41+ (0.43.0 latest) | the only fix for RUSTSEC-2026-0459; 0.43 rejects empty / comment-only SQL (user-visible) |
| fancy-regex | 0.19.2 → 0.20.2 | 0.20.1 makes positive lookaheads atomic (PCRE-like): regex behaviour change |
| zig | 0.16.0 → 0.17.1 | cargo-zigbuild compatibility; setup-zig node20 |
| vscode-languageclient | 10.1.2 → 11.0.0 | major; minimatch advisory needs 11.x transitively (blocked either way) |
| php-wasm | 0.1.0 → 0.2.0 | playground PHP pane; bundled PHP unknown |
| edition 2024 | — | excluded (ruled 2026-10-06, DEC-521 plan) |
### Found during the survey (2026-11-19, release binary at `b396ce96`)
- **Live regex divergence under fancy-regex 0.19** (legs disagree: a P0-shaped defect the 0.20 bump fixes):
  `Regex.compileBacktracking("^(a(?i)b)c$")` on `"aBC"` and `("^(?=x|(x))\\1x$")` on `"xx"` print `true` on the VM and
  the tree-walker, `false` on PHP (both cases are in fancy-regex's 0.20.1 changelog). `(?=(a+))a\\1$` on `"aaa"` is
  already `false` on all three legs. A three-leg test goes in red BEFORE the bump.
- **New tool pins** (sha256 of the release asset, downloaded 2026-11-19): cargo-nextest 0.9.152
  `d039b43c3e0958441b6ad0a39c8884d018b3d2e8fb9928f209074b1100b5e17a` (no upstream `.sha256` file: self-computed over
  TLS); wasm-pack v0.15.1 `410101009739b9d01c7d5affaaad6e9c9a24b4cba6791a4805db8e89b9bf7af2`; zig 0.17.1
  `3581863e3cbcfd0fce0efc682ce7e9e93da96668a977791d356a26d2a7e9b17a` (equals ziglang.org `index.json`, released 2026-11-02).
- **PHP**: 8.6.0 and 8.5.13 released 2026-11-19 (php.net JSON); local builds are 8.4.27, 8.5.12, 8.6.0RC7 and php-master
  (8.7.0-dev). CI's non-gating canary is still labelled `8.6-dev`.
### Doc drift found
`docs/specs/UNIFIED-SPEC.md` says argon2 "0.5.x" (Cargo.toml has 0.6); a `Cargo.toml` comment says "14 crates" (15 declared).
Prose to update on bumps: UNIFIED-SPEC dependency rows (rusqlite, fancy-regex), `src/jit/compile/mod.rs` (cranelift).

## Formal Plan
1. **Commit A `chore(deps)`**: `cargo update`; floors rustls 0.23.46, webpki-roots 1.0.10, lettre 0.11.24; nextest and
   wasm-pack pins (sha256 re-derived from the release assets); VS Code lockfile in-range refresh; advisories recorded in
   KNOWN_ISSUES; argon2 / crate-count doc drift fixed. Certified by `cargo check` + targeted suites, then the gate.
2. **Commit B** per approved semver-incompatible bump, each alone, in this order: fancy-regex 0.20.2 (red three-leg test
   of the two divergences first); rusqlite 0.43 (empty / comment-only SQL pinned on the legs it reaches, UNIFIED-SPEC row);
   Rust 1.100.0 (new deny lints fixed); cranelift 0.137 (JIT suites, `src/jit/compile/mod.rs` citation);
   vscode-languageclient 11 + php-wasm 0.2; zig 0.17.1 with `mlugg/setup-zig` replaced by a sha-checked direct install;
   CI PHP legs (8.5 and 8.6 gating, 8.7-dev nightly) and the local 8.5.13 oracle build.
3. **cargo-audit** reinstalled locally, run on the final lockfile; every remaining advisory disclosed.
4. **Gate once on the frozen tree**, then one push and CI watch.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Compatible refresh, floors, tool pins, advisories recorded | M | done | `38f312a4` gate green (nextest 3781/3781, clippy 0/0, release); VS Code lockfile is gitignored (fresh resolve already gets brace-expansion 5.0.13); KNOWN_ISSUES advisory list owed to step 3 (cargo-audit) | Cargo.toml Cargo.lock KNOWN_ISSUES.md |
| 2 | Approved semver-incompatible bumps, one commit each | M | doing | fancy-regex 0.20.2: red pin first, 375/375 regex+corpus suites, three-leg probe agrees, panic gone; REGEX-DUP-GROUP-NAME queued (pre-existing), `4abc1f49`; rusqlite 0.43: database + differential suites 450/450, empty-SQL message pinned on both backends (0.40 already failed it, message only) | Cargo.toml Cargo.lock |
| 3 | cargo-audit on the final lockfile | S | todo | - | KNOWN_ISSUES.md |
| 4 | Gate, push, CI | S | todo | - | - |
<!-- /progress-block -->
### Blocked
### Needs input
- none: every bump in this plan was answered 2026-11-19 (Decisions Log).
### Needs research
### Fragile
### Known issues

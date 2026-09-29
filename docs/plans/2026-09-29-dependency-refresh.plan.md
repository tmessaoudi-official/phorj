# dependency refresh (2026-09-29) Plan

> Developer directive, 2026-09-29, in answer to the resume chooser: *"Okay for option 1 ! but can you
> update all deps/versions first"* — i.e. refresh every dependency/version pin BEFORE row 4l-b3 of
> `2026-09-07-scout-forcing-function.plan.md`. A follow-on to `2026-09-13-dependency-upgrade.plan.md`
> (DEC-521); that plan's edition-2024 row (11) stays there and is NOT part of this refresh.

## Decisions Log
<!-- Stamped with `bash ~/.claude/bin/project-state.sh --append-decision` — never hand-typed. -->
- [2026-09-29 11:55] AGREED: Developer 2026-09-29 (resume chooser): update all deps/versions BEFORE row 4l-b3; the refresh runs first, 4l-b3 follows.
- [2026-09-29 11:56] ASSUMED (review): Edition 2024 is NOT part of this refresh — it is a language and formatting migration (199 files reformatted, 7 split for the size gate), already a separate todo row (11) in 2026-09-13-dependency-upgrade.plan.md — because it is not a dependency version. Alternatives: fold it in (a much larger, riskier batch).
- [2026-09-29 11:56] ASSUMED (review): Two commits (compatible lock refresh; cranelift 0.135 to 0.136 alone), one full gate on the frozen final tree, one push — because a cranelift regression must bisect cleanly, following the 2026-09-13 precedent.
- [2026-09-29 11:56] ASSUMED (review): Reinstall cargo-audit locally (not a phorj dependency, not in CI) and run it on the final lockfile — the 2026-09-13 ruling, re-applied because the stack wipe removed the binary.

## Inventory (verified 2026-09-29)
| Surface | Now | Latest | Source |
|---|---|---|---|
| Rust toolchain / `rust-version` | 1.98.1 | 1.98.1 | static.rust-lang.org stable manifest |
| `Cargo.lock` compatible bumps | see `cargo update --dry-run` | about 50 packages | cargo |
| `cranelift`, `cranelift-jit`, `cranelift-module` | 0.135.2 | 0.136.1 (0.x minor, semver-breaking) | crates.io |
| `mysql` floor | 28.0.2 | 28.0.3 | crates.io |
| `webpki-roots` floor | 1.0.8 | 1.0.9 | crates.io |
| every other direct crate | latest | — | crates.io |
| GitHub Actions (9), zig 0.16.0, wasm-pack 0.15.0, nextest 0.9.146, cargo-zigbuild 0.23.4 | latest | — | git ls-remote, crates.io, ziglang.org |
| `vscode-languageclient` | 10.1.1 (range ^10.1.1) | 10.1.2, engines still ^1.91.0 | npm |
| CodeMirror 6.0.2 / php-wasm 0.1.0 / esbuild 0.28.2 | latest | — | npm |
| PHP oracle | php-8.5.11 (bcmath present), resolved by `scripts/toolchain.env` | — | probe |
| `cargo-audit` | NOT installed (the stack wipe removed it) | 0.22.2 | crates.io |

## Formal Plan
Two commits, then ONE full gate on the frozen final tree, then one push.
1. **Commit A `chore(deps)`** — `cargo update`; `mysql` floor 28.0.3, `webpki-roots` floor 1.0.9; VS Code client 10.1.2 in `editors/vscode`. Certified by: `cargo check`, the regex corpus (`tests/lift_preg.rs` and the engine tests) under `PHORJ_REQUIRE_PHP=1`.
2. **Commit B `chore(deps): cranelift 0.136.1`** — the three `cranelift*` crates; compile first and work from the errors; JIT suite + three-leg differential; NO perf claim.
3. **Stale-version sweep** — phorj `CLAUDE.md` (oracle patch version, bare-`php` note), docs and CI's PHP canary, checked against what is actually installed / released.
4. **cargo-audit** — install locally, run on the final lockfile, disclose any advisory in KNOWN_ISSUES.
5. **Gate, once, on the frozen tree** — `PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features`, clippy `--all-features` and `--no-default-features`, `fmt --check`, `cargo check --no-default-features`, `size-gate.sh`, release build; run detached (`setsid`), never two cargo runs at once. Then the certification schedule's panel (commit B touches `src/jit/`), then plain `git push` and `/ci-watch` on the pinned SHA.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | Inventory and oracle probe (php-8.5.11 resolves, bcmath present) | S | done | - | docs/plans/2026-09-29-dependency-refresh.plan.md |
| 2 | Commit A - lock refresh, mysql and webpki-roots floors, VS Code client 10.1.2 | M | done | 26dfd3f5 | Cargo.toml Cargo.lock editors/vscode/package.json editors/vscode/package-lock.json |
| 3 | Commit B - cranelift 0.136.1 x3, JIT suite and three-leg differential | L | done | 655282f1 | Cargo.toml Cargo.lock src/jit/** |
| 4 | Stale-version sweep - CLAUDE.md oracle note, docs, CI PHP canary (still valid: no PHP 8.6 release) | S | done | - | CLAUDE.md docs/** .github/workflows/ci.yml |
| 5 | cargo-audit on the final lockfile (lru advisory cleared, 2 remain) | S | done | - | KNOWN_ISSUES.md |
| 6 | Full gate on the frozen tree, panel, push, ci-watch | M | todo | - | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues

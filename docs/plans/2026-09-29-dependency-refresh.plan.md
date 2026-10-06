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
- [2026-09-29 14:50] ASSUMED (review): The playground strict_types fix rides in this batch instead of its own push — because the developer reported it as broken and one full pre-push gate covers both. Alternatives: push the two playground commits alone (they sit above the dep commits, so the push would carry the batch anyway).
- [2026-10-06 15:17] ASSUMED (review): Audit F5: row 6 marked done with evidence ed1501a5 (`fix(playground,docs): panel round 1 on the unpushed batch`; push + green CI run 36575185132), the 'panel round 2' half recorded as never having run as a separate step rather than claimed; 655282f1's three-leg overclaim corrected by a dated Known-issues note, history not rewritten. Alternatives: split row 6 into a done push row and a todo panel row; leave todo until the developer rules.

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
| CodeMirror meta-package 6.0.2 / php-wasm 0.1.0 / esbuild 0.28.2 | latest | — | npm |
| vendored CodeMirror bundle's TRANSITIVE tree (`playground/web/vendor/README.md`) | state 6.7.4, view 6.43.11, commands 6.11.0, `@lezer/common` 1.5.2, `@lezer/highlight` 1.2.3, style-mod 4.1.3 | 6.7.6, 6.43.13, 6.11.1, 1.5.3, 1.2.5, 4.1.4 — missed by the first inventory (panel round 1), rebuilt | npm |
| `rustls` floor | 0.23 (locked 0.23.44 before the refresh) | 0.23.45 fixes RUSTSEC-2026-0285 (panel round 1), floor raised | crates.io, RustSec |
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
| 2 | Commit A — `chore(deps): compatible lock refresh (47 version changes and one removal)`, mysql and webpki-roots floors, VS Code client floor ^10.1.2 (manifest only; the gitignored lock and node_modules were installed in the round-1 fix) | M | done | 26dfd3f5 | Cargo.toml Cargo.lock editors/vscode/package.json |
| 3 | Commit B — `chore(deps): cranelift, cranelift-jit, cranelift-module 0.135 to 0.136.1`; the pre-commit tier ran TWO legs (interpreter and VM, `PHORJ_SKIP_PHP=1`), the PHP leg belongs to the pre-push gate | L | done | 655282f1 | Cargo.toml Cargo.lock |
| 4 | Stale-version sweep — `docs(deps): 2026-09-29 refresh bookkeeping`, CLAUDE.md oracle note, CI PHP canary still valid (no PHP 8.6 release) | S | done | 49e5f16f | CLAUDE.md KNOWN_ISSUES.md docs/plans/SLICE-STATE.md |
| 5 | cargo-audit on the final lockfile — `docs(deps): 2026-09-29 refresh bookkeeping`, lru advisory cleared, 2 remain | S | done | 49e5f16f | KNOWN_ISSUES.md |
| 7 | Panel round 1 fixes (3 lenses, 14 findings, frozen at `docs(deps): 2026-09-29 refresh bookkeeping`): rustls floor 0.23.45 + RUSTSEC-2026-0285 recorded, vendored CodeMirror tree rebuilt (before/after screenshots), VS Code client installed, CHANGELOG + DEC-556 + MASTER-PLAN rows, stale php notes, PIC/GOT disclosure, corrected certification claims | M | done | 2b2cd4ea | Cargo.toml KNOWN_ISSUES.md CHANGELOG.md playground/web/vendor/* docs/plans/* scripts/* src/jit/compile/mod.rs |
| 8 | Playground PHP pane fix (rode into this batch at the developer's report) — `fix(playground): run transpiled PHP through pib_run without php-wasm's ?> prefix`: php-run.js, Node test, CI step, KNOWN_ISSUES PLAYGROUND-PHP-WASM | M | done | 4bc26130 | playground/web/php-run.js playground/web/main.js playground/tests/php-run.test.mjs .github/workflows/playground.yml |
| 6 | Full gate on the frozen tree, push, ci-watch (pushed with ed1501a5 `fix(playground,docs): panel round 1 on the unpushed batch`; CI run 36575185132 green: gate incl. PHP oracle, jit, cross-build, perf-gate, unsafe-island; only the non-gating 8.6-dev oracle failed). No panel round 2 is recorded (see Known issues, 2026-10-06) | M | done | ed1501a5 | - |
<!-- /progress-block -->
### Blocked
### Needs input
### Needs research
### Fragile
### Known issues
- **2026-10-06 correction (audit F5).** Commit B's message (655282f1) says "Certified by the pre-commit fast tier (JIT suite + the three-leg differential); the PHP-oracle differential and the full all-features gate run once on the frozen final tree before the push". The same sentence defers the PHP oracle, so "three-leg" contradicts it: the fast-tier certification was TWO legs: the pre-commit tier sets `PHORJ_SKIP_PHP=1`, so it ran the interpreter and the VM only. The commit message is left as written (history is not rewritten); this note is the correction. The third leg is certified by CI, not by the commit. CI run 36575185132 on ed1501a5 (the batch's push) is green in `gate (fmt + clippy + test + PHP oracle)`, `jit`, `cross-build`, `perf-gate` and `unsafe-island` [Verified 2026-10-06: `gh run view 36575185132`]. That is what the bullets below meant by "row 6", so their "NOT yet run" is superseded. Row 6 was left `todo` after that push and is now `done`. **No "panel round 2" ran as a separate, recorded step.** `git log --grep='round 2'` finds no commit for this batch. The corrections the last bullet attributes to "the panel round 2 fix commit" (`microbench.sh`, `backend-parity-reviewer.md`) landed in ed1501a5, which is titled "panel round 1 on the unpushed batch". The four earlier `ASSUMED (review)` entries (11:56 ×3, 14:50) remain unratified; the fifth (2026-10-06 15:17) is this correction's own.
- **cranelift-jit 0.136 changes the x86_64 JIT codegen model** (panel round 1, correctness F1 and completeness F8). `JITBuilder::new` / `with_flags` (`src/jit/compile/mod.rs:52`, `:212`) now force `is_pic=true`: every `Linkage::Import` helper call loads its address through a per-blob GOT entry instead of an absolute address. Commit B's message said "no Cranelift API change we use"; that sentence was carried from the 2026-09-13 plan and NOT verified — it is wrong. Not a proven miscompile (the helpers compute the same values; the JIT unit tests route every boxed operation through an `rt_*` helper, so they exercise the GOT path), but it is to be certified only by the pre-push gate's JIT suite and differential (NOT yet run on this batch — row 6), never by a measurement. NO perf claim; a quiet-box before/after stays OWED.
- **What commit A and B actually ran.** The pre-commit tier is `--features jit` with `PHORJ_SKIP_PHP=1`: 3518 tests, interpreter and VM legs only. The PHP leg, the `--all-features` build and the `PHORJ_REQUIRE_PHP=1` regex corpus (`tests/lift_preg.rs`, the engine tests) run ONLY in the pre-push gate, which has NOT yet run on this batch (row 6). No regex crate moved in commit A.
- **`wasm-bindgen` 0.2.129 / `js-sys` / `web-sys` 0.3.106 are wasm32-only.** A native `cargo check` never compiles them, so `cargo check -p phorj-playground --target wasm32-unknown-unknown` was run in the round-1 fix: it finished clean (1 min 8 s, exit 0) [Verified 2026-09-29]. The wasm-pack build and the live Pages deploy remain CI's.
- **The playground pane shows "execution crashed" locally** (before and after the CodeMirror rebuild, identical): `playground/web/pkg/` (wasm-pack output) is absent on this box, so only the editor mount was verifiable here.
- **microbench comparator.** `docker image inspect php:8.5-cli` finds no image on this box; the pre-push microbench gate compares only against `_baseline_php` (recorded on PHP 8.5.10, `sha256:9ebdf4c2…`) and will pull the moved tag. Pre-existing gate behaviour, not a pin to bump.
- **Floors were raised only where a fix or feature needed it** (`mysql`, `webpki-roots`, `rustls` — the last for RUSTSEC-2026-0285); the other direct crates, `cranelift*` included, keep their minor-level requirement (`"0.136"`) while the lock holds the latest patch. Commit A's message says floors follow the lock; that is true of those three, not of all.
- **`microbench.sh:27` and `backend-parity-reviewer.md`** carried stale php version notes; both corrected in the panel round 2 fix commit.

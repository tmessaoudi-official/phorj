# dependency upgrade (2026-09-13) Plan

> Developer directive, 2026-09-13: *"upgrade all deps in phorj — rust and everything … check all
> deps/versions … don't miss anything"*, after a `/stack` hard-restart moved the machine to Rust
> 1.98.1 and PHP 8.5.10. Runs BEFORE L4b lands (row 5c of
> `2026-09-07-scout-forcing-function.plan.md`), which stays uncommitted in the tree meanwhile.

## Decisions Log
<!-- Ruled interactively 13:52–14:18 in one session; 14:18 is the `date` read after the last batch. -->
- [2026-09-13 14:18] AGREED: `fancy-regex` 0.11 → 0.19.2, with tests pinning each observable behaviour change, every pin checked against PHP `preg_*` on the 8.5 oracle; a divergence from PHP is recorded in KNOWN_ISSUES, never hidden.
- [2026-09-13 14:18] AGREED: scope = Rust 1.97.1 → 1.98.1, every compatible `Cargo.lock` bump, `argon2` 0.5 → 0.6 (guarded by a hash produced by today's 0.5.3 build), `mysql` 28.0.2, CI wasm-pack 0.15.0, `vscode-languageclient` 10.1.1 + `engines.vscode` ^1.91, playground CodeMirror 6.0.2 rebuild with before/after screenshots, `php-wasm` pinned to 0.1.0.
- [2026-09-13 14:18] AGREED: order = the upgrade first, ONE full all-features gate on the combined tree, the upgrade and L4b as separate commits, one push.
- [2026-09-13 14:18] AGREED: `cranelift` ×3 0.134.3 → 0.135.2 NOW, in its own commit, gated by the JIT suite + the three-leg differential; no perf claim until a quiet-box benchmark.
- [2026-09-13 14:18] AGREED: install `cargo-audit` locally (not a phorj dependency, not in CI), run it on the upgraded lockfile, report every advisory; an unfixed one goes to KNOWN_ISSUES.
- [2026-09-13 14:18] AGREED: CI runners (`*-latest`) and the 9 actions stay on floating major tags — already the latest majors; only the wasm-pack install changes.
- [2026-09-13 14:49] AGREED: Rust edition 2021 → **2024, formatting included** (`style_edition` follows the edition), as its own slice AFTER the upgrade and L4b commits. The 5 `set_var`/`remove_var` sites under `#![deny(unsafe_code)]` (`src/bundle/cross.rs`, `src/bundle/manifest.rs` test fns) are refactored to take the value, never `#[allow(unsafe_code)]` — `src/jit/` stays the sole unsafe island; the 8 in `tests/registry.rs` get `unsafe {}`; files the reformat would push past the size-gate ratchet are split first. Evidence behind the ruling: `-W rust-2024-compatibility` on rustc 1.97.1 flagged 55 phorj sites — 13 env writes, 36 drop-order (35 trait-object `Value` temporaries, 0 lock/`RefCell` guards, 1 real destructor at `src/serve/transport.rs:343` on the shutdown path), 5 macro `expr` fragments, 1 test pattern.

## Inventory (verified 2026-09-13)
| Surface | Before | Latest | Source |
|---|---|---|---|
| Rust toolchain / `rust-version` | 1.97.1 (playground metadata 1.74) | 1.98.1 | static.rust-lang.org stable manifest |
| `argon2` | 0.5.3 | 0.6.0 (`std` feature removed, `password-hash` 0.6) | crates.io + CHANGELOG |
| `cranelift`, `cranelift-jit`, `cranelift-module` | 0.134.3 | 0.135.2 | crates.io |
| `fancy-regex` | 0.11.0 | 0.19.2 | crates.io + CHANGELOG |
| `mysql` | 28.0.0 | 28.0.2 | crates.io |
| `rustls`, `wasm-bindgen`, ~60 transitive | patch/minor | `cargo update --dry-run` | cargo |
| corosensei, ctrlc, lettre, postgres, regex, rusqlite, unicode-segmentation, webpki-roots, serde_json | latest | — | crates.io |
| PHP oracle / docker `php:8.5-cli` / 8.4 | 8.5.10 / digest `9ebdf4c2…` / 8.4.25 | same (8.6 unreleased) | php.net, Docker Hub |
| cargo-nextest / cargo-zigbuild / zig | 0.9.144 / 0.23.4 / 0.16.0 | same | crates.io, ziglang.org |
| GitHub Actions (9) + runners | floating majors | latest majors | `git ls-remote` |
| wasm-pack (CI) | 0.13.1 (hardcoded in the rustwasm installer) | 0.15.0 (`wasm-bindgen/wasm-pack`) | crates.io + release asset HEAD 200 |
| `vscode-languageclient` / `engines.vscode` | 10.1.0 / ^1.75 (already wrong: 10.1.0 needs ^1.91) | 10.1.1 / ^1.91 | npm |
| CodeMirror bundle / php-wasm | 6.0.1 / unpinned | 6.0.2 / 0.1.0 | npm |
| Duplicate majors (`rand`, `getrandom`, `windows-sys`, `hashbrown`) | held by upstream crates | — | `cargo tree -d` |

## Formal Plan
**Three commits, as ruled** (upgrade · cranelift · L4b). Steps 2–4 and 6–8 below are work items inside
commit A, not commits of their own. `Cargo.lock` cannot be split by hunk here, so the cranelift bump
starts only after commit A lands. Pre-commit tests the working tree, so commits A and B are not
individually certified; the full gate runs once, on the final tree, before the push.

1. **Fixture first:** a hash produced by the current `argon2` 0.5.3 build, pinned in `src/ext/cryptography/tests.rs` before the bump.
2. **Toolchain** — `rust-toolchain.toml`, `Cargo.toml` + `playground/Cargo.toml` `rust-version`, `cargo update`, `mysql` 28.0.2; fix any new lint the 1.98 clippy raises.
3. **`argon2` 0.6** — drop the removed `std` feature; `PasswordHash` moves to `password_hash::phc`, and `hash_password(pw)` now draws its own salt (`getrandom` default feature). Defaults are unchanged (`m=19456,t=2,p=1`, read from `params.rs` at the `argon2-v0.6.0` tag). Both the PHP-made and the 0.5.3-made hash must verify.
4. **`fancy-regex` 0.19.2** — API adaptation for the call sites in `engine.rs` (`captures_iter`/`find_iter`/`captures`/`find`/`is_match`/`capture_names`) + behaviour-pin tests checked against PHP.
5. **Commit A** `chore(deps)` (steps 1–4, 6–8), then **commit B** `chore(deps): cranelift 0.135.2` — `src/jit/`; JIT suite + differential. The wasmtime 48.0.0 release notes (`release-48.0.0` branch) list no Cranelift API change; compile first and work from the errors.
6. **CI** — wasm-pack 0.15.0 pinned download in `.github/workflows/playground.yml`.
7. **VS Code** — `editors/vscode/package.json` + lock.
8. **Playground** — `playground/web/vendor/codemirror.js` rebuild, recording the resolved `@codemirror/*` versions in `vendor/README.md`; `php-wasm@0.1.0` (`cdn.jsdelivr.net/npm/php-wasm@0.1.0/PhpWeb.mjs` answers 200, and `0.1.0` is the `latest` dist-tag); before/after screenshots.
9. **cargo-audit** on the final lockfile; advisories reported, unfixed ones to KNOWN_ISSUES.
10. **Stale-version sweep** over docs, `CLAUDE.md`, `README.md`, `FEATURES.md`, `KNOWN_ISSUES.md`, `src`, `Cargo.toml`; every hit accounted for.
11. **Gate** (once, final tree): `PHORJ_REQUIRE_PHP=1 cargo nextest run --workspace --all-features`, clippy `--all-features` and `--no-default-features`, `fmt --check`, `size-gate.sh`, `doc-guards.sh`, release build; then **commit C** (L4b), then one push with SSH keepalives.

12. **Edition 2024 slice (row 11, after the push).** Measured on a scratch copy of the tree
    (edition set to 2024, playground `rust-version` raised — 1.74 is refused by edition 2024, which
    needs ≥1.85): `cargo fmt --all` reformats **199 files, +1393/−1074**, and `size-gate.sh` then
    FAILS on **7** files, which are split by cohesion BEFORE the reformat lands:
    `src/value/mod.rs` (668→672), `src/parser/tests/exprs.rs` (559→567), `src/native/list_tests.rs`
    (740→742), `src/checker/tests/unions.rs` (680→686), `src/checker/tests/generics.rs` (510→512)
    grow past their grandfathered baselines; `src/ext/http_client/tests.rs` (501) and
    `src/checker/tests/calls.rs` (503) become new hard-cap breaches. Then: env-write refactor (5
    under the deny) + `unsafe {}` (8 in `tests/registry.rs`), `cargo fix --edition` for the 6
    mechanical sites, the 36 drop-order sites reviewed against the full gate + differential, and
    `CLAUDE.md`'s "edition 2021" line.

**Test-count baseline (reconciled 2026-09-13):** 3225 `#[test]` functions in tracked `.rs` files (src 2582 · tests 630 · playground 13) = 3221 listed by `cargo nextest list --workspace --all-features` + 2 `#[ignore]` timing benches (`src/jit/tests/boxed.rs:312`, `src/jit/tests/unboxed_flow.rs:476`) + 2 `cfg(not(feature))` tests absent under all-features (`src/ext/database/natives/tests_more.rs:184`, `tests/serve_tls.rs:163`). Name-by-name multiset compare, zero unexplained.

## Status
<!-- progress-block v1 -->
| # | Step | Size | State | Evidence | Files |
|---|------|------|-------|----------|-------|
| 1 | argon2 0.5.3 hash fixture pinned before the bump (7/7 crypto tests on 0.5.3, then green on 0.6) — `chore(deps): DEC-521` | S | done | 40d17417 | src/ext/cryptography/tests.rs |
| 2 | Toolchain 1.98.1 + lockfile (87 packages) + mysql 28.0.2 — check build clean — `chore(deps): DEC-521` | M | done | 40d17417 | rust-toolchain.toml Cargo.toml Cargo.lock playground/Cargo.toml |
| 3 | argon2 0.6.0 (`phc::PasswordHash`, crate-drawn salt) — 19/19 crypto+regex tests | M | done | 40d17417 | Cargo.toml src/ext/cryptography/* |
| 4 | fancy-regex 0.19.2 (compiled unchanged); 23-pattern probe vs PHP 8.5.10/PCRE2 10.44: 19 agree (11 pinned in `tests/differential.rs`), 2 NEW divergences `\O` + `(?~…)` now refused on every leg (`reject.rs` + PHP twin); pre-commit caught `^(?=a)(a|a)+$` resolving natively while PCRE faults — disclosed PHP-leg limitation widened, pinned | M | done | 40d17417 | Cargo.toml src/ext/regex/* |
| 5 | cranelift 0.135.2 — `chore(deps): DEC-521 — cranelift` — no source change; JIT suite + three-leg differential 400/400; no perf claim (quiet-box before/after OWED) | L | done | 424feff0 | Cargo.toml src/jit/* |
| 6 | CI wasm-pack 0.15.0, sha256-verified pinned asset — `chore(deps): DEC-521` | S | done | 40d17417 | .github/workflows/playground.yml |
| 7 | VS Code client ^10.1.1 + engines ^1.91.0 (lockfile is untracked) — `chore(deps): DEC-521` | S | done | 40d17417 | editors/vscode/package.json editors/vscode/package-lock.json |
| 8 | Playground CodeMirror 6.0.2 + php-wasm@0.1.0 — editor mounts identically before/after (1 editor, 13 lines, 255 chars; screenshots in session scratch) — `chore(deps): DEC-521` | M | done | 40d17417 | playground/web/* |
| 9 | cargo-audit: 1 vuln + 2 warnings left, all unfixable upstream, disclosed as KNOWN_ISSUES DEP-ADVISORIES — `chore(deps): DEC-521` | S | done | 40d17417 | KNOWN_ISSUES.md |
| 10 | Full gate + commits + push — commits `40d17417` `424feff0` `df227f0a` landed; the pre-push hook is the full gate | M | doing | - | - |
| 11 | Edition 2024 slice (language + formatting), after the push | L | todo | - | Cargo.toml playground/Cargo.toml src/bundle/cross.rs src/bundle/manifest.rs tests/registry.rs CLAUDE.md |
<!-- /progress-block -->
### Blocked
### Needs input
- Rust edition 2021 → 2024 (asked 2026-09-13, answer pending).
### Needs research
### Fragile
- `fancy-regex` 0.19.2 was published on 2026-09-13, the same day as this upgrade.
### Known issues

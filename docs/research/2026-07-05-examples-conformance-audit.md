# examples/ + conformance/ audit — 2026-07-05

Five parallel audits over the 245-file corpus (187 `examples/`, 58 `conformance/`). Raw per-agent
reports were in scratch; this is the consolidated, dispositioned synthesis. Feeds the next big session.
Disposition tags: **[FIX]** = just edit/format · **[COMPILER]** = enforce/change in checker/registry ·
**[DESIGN]** = developer ruling (Invariant 15) · **[NEXT]** = build-out for the next session.

## 1. Import & intrinsic discipline
- **Intrinsic set = `panic`/`todo`/`unreachable`/`assert`** (`src/checker/common.rs:8-12`, `is_intrinsic_name`) — bare by design, reserved, no import. A separate `Test.assert` native is the unit-test API.
- **Nothing genuinely "in the wind."** Every bare call is an intrinsic, a same-package user function (bare-legal), or an interop `declare`d foreign builtin (`examples/interop/*.d.phg`, PHP-target-only).
- **[DESIGN, already adjudicated] `assert` etc. behind `import Core;`** — `UNIFIED-SPEC.md:295-301` **W2-6** already plans this breaking change. The developer's instinct (bare reads inconsistent) matches the roadmap. Decision: do W2-6 now, or keep tracked.
- **Leaky `Core.Http` prelude** — `examples/web/core-http.phg:40` uses `String.replace` with no `import Core.String` yet compiles: the injected Http prelude (`src/cli/mod.rs:549-552`) begins `import Core.Bytes/String/List/Regex;` and those merge into the *user* import map (bounded to Http; 6 other preludes probed clean). `native/mod.rs:441` shows it's meant to be prevented → genuine hole. **[FIX]** add `import Core.String;` to the example · **[COMPILER]** scope prelude imports to the prelude.
- `->` return syntax: none stray (all inside string literals). Bare-`string` UFCS misuse: none.

## 2. Naming — commands (dead verbs `fmt`/`bench`/`disasm`)
Real verbs are full-name-only. README/FEATURES/examples-README already correct (prior B3-3 fix). Live residue:
- **[FIX] `examples/fmt/` → `examples/format/`** (created this session; its README/showcase already say `format`, only the dir is wrong). Update refs in `examples/README.md`, `MASTER-PLAN`, `CHANGELOG`, `FEATURES`.
- **[FIX] `examples/bench/` (+`bench/manual/`) → `examples/benchmark/`** (bigger blast radius; command is `phg benchmark`).
- **[FIX] `src/cli/bench.rs:339`** output string `"phg bench — median of…"` → `phg benchmark` + its two tests (`src/cli/tests.rs:329,549`). (user-facing output)
- **[FIX]** `examples/bench/README.md`, `workload.phg` comments, `bench/manual/*` — `phg bench`/`phg disasm` → full names.
- **[FIX, low-pri] internal doc-comments** in ~15 `src/**` files say `phg fmt`/`phg bench` (dev-facing rustdoc). Module `src/fmt/` + file `fmt_cmd.rs` names are fine; only the `phg fmt` prose is wrong.
- **[LEAVE] historical**: `docs/specs/archive/`, `docs/research/`, `CHANGELOG.md`, dated `GA-CHECKLIST.md` lines (`phg fmt` was the real name then).

## 3. Naming — stdlib method casing (Invariant 12: camelCase)
- **[COMPILER, breaking] `String.uppercase` → `upperCase`** (`src/native/text.rs:436`; ~13 sites: registry + fault + PHP emit `strtoupper` + tests + examples + docs).
- **[COMPILER, breaking] `String.lowercase` → `lowerCase`** (sibling; ~6 sites; PHP emit `strtolower`).
- **[DESIGN] `String.substring`** — Java/JS/PHP treat it one-word; recommend KEEP.
- **[COMPILER, test] strengthen `charter_function_names_are_lowercamel`** (`src/native/tests.rs`) — it only checks first-char-lowercase, NOT multi-word-all-lowercase, so `uppercase`/`lowercase` passed. HTML tag helpers (`blockquote`/`tbody`) are a documented KEEP exception.

## 4. Naming — module-name doc drift
- **[FIX] `STABILITY.md`** names modules `Core.Console/Text/Convert/Validate/Reflect/Env` but the code/examples use `Core.Output/String/Conversion/Validation/Reflection/Environment`. Reconcile the doc to the real (SSOT) names.

## 5. Playground portability (WASM)
The browser runs the interpreter directly (no `on_deep_stack` worker) and the playground crate builds
`phorj` with **`default-features = false`** (to keep `argon2` out) — which ALSO drops `regex`/`signals`/`green`.
`gen_examples.py` → `examples.js` is **Python (regenerable this session; no wasm-pack)**; it excludes
`{project,interop,lift}` + multi-file + `SYSCALL_IMPORTS`.
- **[FIXED 2026-07-05] `gen_examples.py` was missing `Core.Regex`** in its exclusion set, so `regex.phg`
  leaked into the playground and faulted (regex feature off in WASM). Added `Core.Regex`. **CORRECTION:**
  an earlier draft of this audit claimed the set also mis-spelled `Cryptography` (that the module was
  `Core.Crypto`) — that was WRONG (a substring-match error). The module IS `Core.Cryptography`
  (`src/native/crypto.rs:61`, consistent with the code's long-form convention `Conversion`/`Validation`/
  `Reflection`/`Environment`), so `web/password-verify.phg` was ALREADY correctly excluded; the only real
  bug was the missing `Core.Regex`. Also added `bench` to `SKIP_DIRS` (excludes `workload.phg` — see below).
- **`examples/guide/regex.phg`** — regex feature off in WASM. **[DECISION]** exclude from playground now (generator, ships this session, coverage loss) **vs [NEXT]** enable `regex` in `playground/Cargo.toml` (regex compiles to WASM; bigger bundle; needs a rebuild).
- **`examples/web/password-verify.phg`** — argon2 deliberately out of WASM. **[FIX]** exclude (generator).
- **`examples/guide/concurrency.phg`** — `green`/corosensei can't target wasm32; likely sequential-fallback — confirm or exclude.
- **`examples/bench/workload.phg`** — the ONLY deep-recursion breaker (`allocateChain(1000)`; all else ≤ depth 30). **[FIX] reduce depth ~1000→~120** (native bench stays valid) **+ [FIX frontend] catch `RangeError: Maximum call stack size exceeded`** (co-equal; depth-120 can't be browser-verified this session).
- **[FIX] stale comment** `playground/Cargo.toml` — "every other native is unaffected" is false (regex dropped too).

## 6. Coverage (100%-of-forge check)
- **Examples ≈ 100%** of shipped surface (28/28 constructs; ~all 28 stdlib modules). Only operator-overloading is unshipped.
- **Conformance ≈ 50%** of its `STABILITY.md`-scoped stable surface. **[NEXT] ~11 golden programs**: Convert, **Hash** (RFC KATs), Encoding, Url, Validate, Csv, Path, Ini, Option/Result combinators, deeper Json; then goldens for stable constructs (lambdas, pipe, default params, sealed, must-use, totality).
- **[NEXT/FIX] test-runner** has no `test "…" {}` + `Core.Test` example despite `FEATURES.md ✅`.
- Correctly-absent (do NOT add goldens): Random/File/Process/Environment/Crypto/Secret/Concurrency/faults/LSP.

## 7. SSOT hygiene
- **[FIX] `docs/plans/wave0-remainder.plan.md`** — marked SUPERSEDED (points to MASTER-PLAN), content in git history (`0691228`). `git rm` it — MASTER-PLAN is the sole plan SSOT.

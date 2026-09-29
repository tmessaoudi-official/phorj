# Phorj Playground

A free, zero-backend, browser playground for Phorj. Edit code on the left; on the right you get
**Phorj** (the bytecode VM — what `phg run` executes; no JIT tier exists on wasm, the CLI adds it
natively), the **transpiled PHP source**, and that PHP **executed in-browser** (php-wasm 0.1.0: PHP 8.4.1, 32-bit ints) —
with a badge comparing the two legs' output (and a diff banner when they differ; a handful of examples do —
KNOWN_ISSUES `PLAYGROUND-PHP-WASM`). Everything runs client-side; nothing is sent to a server. (The tree-walking interpreter
remains the correctness oracle in `tests/differential.rs`; the UI no longer shows it separately.)

The **⬆ Lift PHP** button runs the inverse direction: it treats the editor contents as PHP and lifts
them to a Phorj draft (the same engine as `phg lift`), opening the result with a `// lifted (verify)`
banner — a best-effort, review-required scaffold for the Tier-1 PHP subset.

It is auto-deployed to GitHub Pages on every push to `master`, so the live site always runs the latest
`phg`.

> **Single-file only.** The sidebar auto-loads every *single-file* example from `examples/` (see
> `web/gen_examples.py`). Multi-file examples — `examples/package-manager/`, `examples/project/*`,
> `examples/interop/*` — cannot run in the single-file wasm sandbox (no multi-file/vendored virtual
> FS yet), so they live in the repo only; clone and run them with `phg run`. (Making them runnable
> here is a tracked future slice — a virtual multi-file FS in wasm.)

## How it works

- The Phorj pipeline is compiled to WebAssembly. The core `phorj` crate is unchanged (its
  14 admitted dependencies are feature-gated; the wasm build compiles the std-only core, and its
  roots are `#![deny(unsafe_code)]` with the audited JIT island compiled out); this `playground/`
  crate is a **separate workspace member** (itself `#![forbid(unsafe_code)]`) adding only
  `wasm-bindgen` (wasm32-only) and `serde_json`.
- The wasm runs in a **Web Worker** with a per-call timeout — a runaway program terminates the worker
  instead of freezing the tab (wasm is single-threaded and non-interruptible).
- The wrapper functions bypass the CLI's 256 MB `std::thread` worker (`on_deep_stack`, unavailable on
  wasm) and call the public pipeline directly: `parse → check → compile+VM / transpile` (the
  interpreter export still exists for the oracle, the UI just doesn't call it).
- The transpiled PHP is executed by [`php-wasm`](https://github.com/seanmorris/php-wasm) (0.1.0, PHP 8.4.1 —
  BELOW Phorj's 8.5 transpile floor), loaded lazily from a CDN on first run.

The wrapper *logic* lives in plain `*_json(&str) -> String` functions in `src/lib.rs` and is unit-tested
on the native target (`cargo test -p phorj-playground`); only the thin `#[wasm_bindgen]` exports are
wasm-gated. Byte-identity itself stays gated by `tests/differential.rs` — the playground only *surfaces*
agreement.

## Build & run locally

Prerequisites: the pinned Rust toolchain (`rust-toolchain.toml`), `wasm-pack`, and Python 3.

```bash
# from the repo root
rustup target add wasm32-unknown-unknown                 # once
cargo install wasm-pack                                  # once (or use the official installer)

# bump the wasm stack (MAX_CALL_DEPTH is 4096 — see src/limits.rs)
RUSTFLAGS="-C link-arg=-zstack-size=33554432" \
  wasm-pack build playground --target web --release --out-dir web/pkg

python3 playground/web/gen_examples.py                   # regenerate examples.js

# serve the static dir (any static server works); a module worker needs http://, not file://
python3 -m http.server -d playground/web 8000
# open http://localhost:8000
```

## Deploy

GitHub Pages, via `.github/workflows/playground.yml`:

1. In the repo: **Settings → Pages → Build and deployment → Source = GitHub Actions** (one-time).
2. Push to `master` (or run the `playground` workflow manually via *Actions → playground → Run
   workflow*). The workflow builds the wasm, runs the php-wasm test, regenerates `examples.js`, assembles `dist/`, and deploys.
3. The site is published at `https://<owner>.github.io/phorj/`.

## Tests

```bash
cargo test -p phorj-playground          # native unit tests of the wrapper logic
node playground/tests/php-run.test.mjs  # the php-wasm run path (real php-wasm 0.1.0; needs network unless PHP_WASM_DIR is set)
```

## Known limitations (v1)

- **php-wasm**: `web/php-run.js` runs the emitted PHP through `pib_run` directly, because php-wasm's own `run()` prefixes `?>` and PHP then rejects the leading `declare(strict_types=1)`. Covered by `node playground/tests/php-run.test.mjs` (`PHG=<phg binary>` is optional; installs `php-wasm@0.1.0` into a temp dir unless `PHP_WASM_DIR` is set); runs in `playground.yml` before deploy (without a `phg` binary it runs 8 checks on the hand-written emit shape; with one, a 9th checks that shape against the real transpiler).
- **The PHP pane on the real page was never exercised locally** (`web/pkg/` is a wasm-pack output and is absent on the dev
  box): the run path is tested standalone (Node `PhpNode` and Chromium `PhpWeb`), so the first real run of the fixed pane is the
  Pages deploy. The CI step checks the hand-written emit shape; the transpiler's own opening lines are pinned Rust-side
  (`src/transpile/tests.rs`), so a change there is caught by `cargo test`, not by this step.
- php-wasm is pinned to `0.1.0` in `web/main.js` (`PHP_WASM_URL`).
- **Very deep recursion** can hit the wasm engine's call-stack limit before Phorj's `MAX_CALL_DEPTH`
  guard; it surfaces as an "execution crashed/timed out" message rather than a clean fault.
- Single-snippet `package Main;` programs only — no multi-file projects, vendored deps, `phg build`,
  or real `Core.File` I/O. Filesystem guide examples are excluded from the picker.

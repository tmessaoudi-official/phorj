#!/usr/bin/env bash
# wasm-check.sh — compile for `wasm32-unknown-unknown`, the target the LOCAL gate never touched.
#
# Why this exists. `INJECTED_SPAN_BASE` was `1 << 32`, which is a compile ERROR on wasm32 (`usize` is 32
# bits there, so the shift overflows during const-eval). Every local gate step — `cargo test`, both
# `clippy` passes, `cargo build --release`, `cargo check --no-default-features` — builds for the 64-bit
# host and passed green. The ONLY wasm32 compile in the project was the `playground` GitHub workflow, so
# the break shipped and stayed red for six consecutive runs while every local signal said fine.
#
# A gate that cannot see a whole target is not a gate for that target. This closes it: the same class of
# bug (a 64-bit-only constant, a pointer-width assumption, a dependency that will not build for wasm)
# now fails in the pre-push lane instead of on GitHub.
#
# Scope: `cargo check`, not a full `wasm-pack build`. Checking is what catches type/const errors, and it
# needs no `wasm-pack`, no node, and no network — so it runs anywhere the wasm32 target is installed.
# `--no-default-features` is REQUIRED for the library: `jit` is a default feature and cranelift cannot
# target wasm32.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
  echo "[wasm-check] SKIP — the wasm32-unknown-unknown target is not installed."
  echo "[wasm-check]   install it with: rustup target add wasm32-unknown-unknown"
  echo "[wasm-check]   SKIPPING IS LOUD ON PURPOSE: without this the playground build is unverified"
  echo "[wasm-check]   locally, which is exactly how the INJECTED_SPAN_BASE overflow reached CI."
  exit 0
fi

# The library first: it is where the pointer-width assumptions live.
echo "[wasm-check] cargo check --target wasm32-unknown-unknown --no-default-features (lib)"
cargo check --quiet --target wasm32-unknown-unknown --no-default-features

# Then the playground crate itself, in release — byte-for-byte the configuration the workflow builds.
echo "[wasm-check] cargo check -p phorj-playground --target wasm32-unknown-unknown --release"
cargo check --quiet -p phorj-playground --target wasm32-unknown-unknown --release

echo "[wasm-check] OK — the playground's target compiles."

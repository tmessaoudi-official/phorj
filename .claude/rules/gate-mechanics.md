---
paths:
  - "scripts/git-hooks/**"
  - "Cargo.toml"
  - ".config/nextest.toml"
---

# Gate mechanics — what each git hook runs, and why the test tier is fast

Moved unchanged out of `CLAUDE.md` § "Toolchain & quality gate" on 2026-11-14 (`/rules-split`, plan
`docs/plans/2026-11-14-ground-prep.plan.md`). `CLAUDE.md` keeps the green definition, the full correctness gate
command and a one-line index entry. A new lesson about the hooks, their timings or the build profile belongs HERE,
not in `CLAUDE.md`.

- **Tiered git hooks** (speed, 2026-07-08 — `scripts/git-hooks/{pre-commit,pre-push}`): **pre-commit**
  runs the fast Rust-only tier (`fmt` + `nextest --features jit`, EXCLUDING the two heavy sweeps
  `every_repo_phg_formats_idempotently_and_safely` + `shipped_manual_example_runs_on_both_backends`) —
  ~12s vs ~126s (pre-commit also runs `phg format --check examples selftest` + doc-tests;
  pre-push also runs `fmt --check`, the size-gate, `validate-infra.sh` (DEC-388.4), doc-tests and
  `cargo build --release` — the hooks are the SSOT of their own steps). **pre-push** runs the FULL
  suite (those two included) + `clippy` (`--no-default-features`
  AND `--all-features`) + the PHP-oracle spine check + `microbench-gate`. Test-speed rests on
  `Cargo.toml [profile.dev]` (deps opt-2, workspace opt-1, **deps `debug = false`** — that last one cut
  `target/debug` from 24 GB to 7.4 GB, which is what keeps `target/` warm instead of being cleared for
  disk; phorj's own debuginfo is untouched); `cargo-nextest` is the parallel runner (fallback:
  `cargo test`) and IS installed — a warm full-suite cycle is ~43 s.

# Contributing to Phorj

Thanks for your interest in Phorj! This guide covers how to set up, the quality bar every change
must clear, and the correctness invariants that keep the project sound. By participating you agree to
the [Code of Conduct](CODE_OF_CONDUCT.md).

Phorj is pre-1.0 and developed by a single maintainer; the surface is still moving. Before investing
in a large change, please open an issue to discuss it first (see [SUPPORT.md](SUPPORT.md)).

## Development setup

You need **Docker** (with the compose plugin), `bash`, `git` and coreutils (`scripts/test-docker.sh` pins the set).
Nothing else: the whole toolchain (the pinned
Rust from `rust-toolchain.toml`, PHP 8.5 with bcmath as the byte-identity oracle, cargo-nextest, `llvm-objcopy`,
zig and cargo-zigbuild for `phg build --target`, wasm-pack and node for the playground) lives in one image,
`docker/Dockerfile`, which is also the one place a tool version is written. `scripts/dk` runs any command in it,
from anywhere in the repo, as you (the files it writes are yours) and with the repo at its own path:

```sh
git clone https://github.com/tmessaoudi-official/phorj
cd phorj
git config core.hooksPath scripts/git-hooks   # the pre-commit and pre-push gates (they run in the image too)
scripts/dk cargo build                        # the first run builds the image (several minutes, once)
scripts/dk cargo test                         # run the full suite
scripts/dk                                    # a shell inside the toolchain
```

The binaries land in `target/` as usual. On a Linux host with glibc 2.39 or newer they also run on the host, so
editors can launch `target/release/phg`; elsewhere point the editor at a native build.

## The quality gate (must be green before every commit)

```sh
scripts/dk cargo test                   # all tests pass
scripts/dk cargo clippy --all-targets   # zero warnings — warnings are DENIED in the manifest
scripts/dk cargo fmt --check            # formatting matches rustfmt
```

A pre-commit hook (`scripts/git-hooks/pre-commit`) enforces this locally. GitHub Actions runs the
same gate, in the same image, on every push and PR (`.github/workflows/ci.yml`), additionally setting
`PHORJ_REQUIRE_PHP=1` so the PHP oracle in `tests/differential.rs` *fails* — never silently
skips — if transpiled PHP diverges from the interpreter/VM. A second CI job exercises
`phg build --target` cross-compilation parity. A change is not done until all three checks are
clean. First-party `unsafe` is confined to the audited JIT island (`src/jit/`, CI-gated) —
`#![deny(unsafe_code)]` is set on both crate roots and must stay; no new `unsafe` outside that
island in `src/`. (The `tests/` integration crates sit outside the deny roots and may carry the
rare `unsafe` std requires — e.g. edition-2024 `env::set_var` — keep them unsafe-free otherwise.)

## Test-driven by default

Write the failing test **before** the implementation, watch it fail, then make it pass. Every new or
changed behavior ships with a test that exercises it. Tests must be *run* (not just compiled) before a
change is considered complete.

- Unit tests live in `#[cfg(test)] mod tests` next to the code.
- Integration tests live in `tests/` (`cli.rs`, `build.rs`, `differential.rs`, the `*_integration.rs`
  suites).
- Golden diagnostics: `PHORJ_BLESS=1 scripts/dk cargo test --test diagnostics` (re)generates the `.expected`
  files — review the diff before committing a blessed change.

## Correctness invariants (read before touching backends)

These are non-negotiable. The full list is in [`docs/INVARIANTS.md`](docs/INVARIANTS.md); the
load-bearing ones:

1. **Backend parity is the spine.** `phg run` (interpreter) and `phg run` (VM) must produce
   **byte-identical** output. The interpreter is the reference semantics; the VM matches it. Enforced
   by `tests/differential.rs`, which globs `examples/**/*.phg`. A standalone built binary is a third
   surface on the same spine and must match the VM.
2. **Adding an `Op` touches three exhaustive matches in the same commit:** `vm.rs::exec_op`,
   `chunk.rs::BytecodeProgram::validate`, and `compiler.rs::stack_effect`. Miss one and the build
   won't compile (by design).
3. **Value kernels are single-sourced.** Arithmetic and comparison live once in `value.rs`
   (`int_*`/`float_*`/`compare_ord`); both backends call them — never re-inline `checked_*`/
   `partial_cmp` in a backend, or the two will drift.
4. **Never panic on input (EV-7).** Lexer/parser/checker and the object-file section readers must
   reject adversarial or malformed input cleanly (a diagnostic or `None`), never a panic/SIGABRT. All
   offset arithmetic in the readers uses `checked_add`/`checked_mul`.
5. **Run a perf number before/after a perf change.** `phg benchmark <file>` measures both backends
   (median-of-N, output-identity gated).

## Architecture orientation

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) is a one-page module map. In short: `lexer` → `parser`
→ `checker` → (`interpreter` | `compiler`+`vm` | `transpile`). Design rationale and decision logs live
in `docs/specs/` (frozen designs) and `docs/plans/` (per-milestone plans) — these *are* the ADRs;
extend the relevant log rather than adding a separate `adr/` tree.

## Commit & PR conventions

- **Conventional-style prefixes:** `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`,
  optionally scoped (`feat(bundle):`, `feat(cli):`).
- **Small, green, self-contained commits.** Don't commit a broken build or red tests.
- One logical change per PR; fill in the [pull request template](.github/PULL_REQUEST_TEMPLATE.md).
- Update docs (`CHANGELOG.md`, the relevant `docs/` files, `examples/README.md`) in the same change
  when you alter a public interface or add an example.

## Adding an example

Drop a `.phg` file under `examples/` (or `examples/guide/` / `examples/realworld/`). It is
automatically byte-identity-gated by `tests/differential.rs` the moment it lands. Add a row to
`examples/README.md`.

## Reporting bugs & requesting features

Use the [issue templates](.github/ISSUE_TEMPLATE). For security issues, **do not** open a public
issue — follow [SECURITY.md](SECURITY.md).

## License of contributions

Contributions are dual-licensed under MIT OR Apache-2.0, matching the project (see
[README](README.md#license)). By submitting a contribution you agree to this.

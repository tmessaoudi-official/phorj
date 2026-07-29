# Third-Party Notices

## Runtime dependencies: a small, feature-gated, all-permissive set

Phorj's **core is `std`-only**: built with `--no-default-features` it links **zero** external crates.
The default build enables nine narrowly-scoped, **feature-gated** dependencies (of 14 admitted), for
capabilities `std` cannot provide safely from phorj's own `#![deny(unsafe_code)]` code (the
audited first-party `unsafe` island — the JIT's fn-ptr call plus its `extern "C"` trampolines,
~48 sites — is confined to `src/jit/`) — crypto, a ReDoS-safe
regex engine, OS-signal handling, stackful coroutines, native JIT codegen, embedded SQL, and Unicode
segmentation (the full policy and clause-by-clause justification live in
`docs/specs/UNIFIED-SPEC.md#external-dependency-policy`). All are permissively licensed, compatible
with Phorj's own license; each can be switched off at build time.

| Crate | Feature | Default? | Domain | License |
|---|---|---|---|---|
| [`argon2`](https://github.com/RustCrypto/password-hashes) (RustCrypto) | `cryptography` | yes | Argon2id password hashing | MIT OR Apache-2.0 |
| [`regex`](https://github.com/rust-lang/regex) (rust-lang) | `regex` | yes | ReDoS-safe regex engine | MIT OR Apache-2.0 |
| [`ctrlc`](https://github.com/Detegr/rust-ctrlc) | `signals` | yes | SIGINT/SIGTERM for `phg serve` | MIT OR Apache-2.0 |
| [`corosensei`](https://github.com/Amanieu/corosensei) | `green` (non-wasm only) | yes | stackful coroutines for green threads | MIT OR Apache-2.0 |
| [`cranelift`](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift), `cranelift-jit`, `cranelift-module` (Bytecode Alliance) | `jit` (non-wasm only) | yes | native-codegen JIT backend | Apache-2.0 WITH LLVM-exception |
| [`rusqlite`](https://github.com/rusqlite/rusqlite) (+ bundled SQLite) | `database` | yes | embedded SQL (`Core.DatabaseModule`) | MIT (SQLite: public domain) |
| [`unicode-segmentation`](https://github.com/unicode-rs/unicode-segmentation) (unicode-rs) | `unicode` | yes | UAX #29 grapheme clusters | MIT OR Apache-2.0 |
| [`rustls`](https://github.com/rustls/rustls) | `http-client` | no | TLS for `Core.HttpClient` | Apache-2.0 OR ISC OR MIT |
| [`webpki-roots`](https://github.com/rustls/webpki-roots) | `http-client` | no | Mozilla trust anchors | CDLA-Permissive-2.0 |
| [`postgres`](https://github.com/sfackler/rust-postgres) | `database-postgres` | no | Postgres driver (sync API) | MIT OR Apache-2.0 |
| [`mysql`](https://github.com/blackbeam/rust-mysql-simple) | `database-mysql` | no | MySQL/MariaDB driver (sync API) | MIT OR Apache-2.0 |
| [`lettre`](https://github.com/lettre/lettre) | `mail` | no | SMTP/MIME mailer (`Core.Mail`) | MIT |

Their transitive dependencies are likewise permissively licensed — MIT/Apache-2.0-family, ISC
(`rustls-webpki`, `untrusted`; `ring` is `Apache-2.0 AND ISC`), Unicode-3.0 (the `icu_*` family via
`lettre`→`idna`; `unicode-ident` is `(MIT OR Apache-2.0) AND Unicode-3.0`), 0BSD, Unlicense OR MIT,
Zlib, BSD-3-Clause, BSL-1.0-as-an-option (`whoami`), Apache-2.0 WITH LLVM-exception; verified from
the lockfile, no copyleft anywhere in the tree. The notable
ones: argon2 pulls the RustCrypto core crates (`password-hash`/`blake2`/`base64ct`); regex pulls
`regex-automata`/`regex-syntax`/`aho-corasick`/`memchr`; ctrlc pulls `nix`; the cranelift JIT pulls
the `cranelift-*` family plus `regalloc2`/`target-lexicon`/`memmap2`/`region`; `rusqlite` pulls
`libsqlite3-sys`, which **compiles the bundled SQLite C source into the binary** (no system
libsqlite3 — SQLite itself is public domain). Among the non-default features, `postgres` pulls
**tokio** transitively (the crate's internal blocking wrapper — the phorj-facing API stays sync), and
`rustls` pulls `ring`. The **WASM playground** (`phorj-playground`) builds with the features off
(`corosensei` and the cranelift crates are additionally non-wasm-gated), so the in-browser build
stays minimal.

Keeping the dependency set this small is a deliberate design constraint (see [VISION.md](VISION.md)):
the language stays buildable in seconds, auditable in full, and low on supply-chain surface.

## Build- and distribution-time tooling (not linked, not distributed)

Some optional workflows shell out to external tools. These are **invoked as separate processes** at
build time — none of their code is linked into Phorj or into produced binaries, so their licenses do
not propagate to Phorj's output. They are only required for the workflows noted:

| Tool | Used for | License |
|---|---|---|
| Rust toolchain (`cargo`, `rustc`) | building Phorj | MIT OR Apache-2.0 |
| `llvm-objcopy` (LLVM) | embedding the program section in `phg build` | Apache-2.0 WITH LLVM-exception |
| [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) | cross-target builds (`build --target`/`--all`) | MIT |
| [`zig`](https://ziglang.org) | the C/linker driver for cross builds | MIT |
| `php` (optional) | round-trip-testing the transpiler output | PHP License |

If you build only the host target with the Rust toolchain, you need none of the cross-build tools.

## Phorj's own license

Phorj is dual-licensed MIT OR Apache-2.0 — see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).

# Security Policy

## Supported versions

Phorj is pre-1.0 and developed by a single maintainer. Only the latest tagged release and the
`master` branch receive fixes.

| Version | Supported |
|---|---|
| latest stable (`v*`) release / `master` | ✅ |
| `nightly` (rolling prerelease from `master`) | ✅ — fixed by the next master push |
| older tags | ❌ |

(Channels: `nightly` = rolling prerelease rebuilt on every master push; `stable` = `v*` tags — see
[`SEMVER.md`](SEMVER.md) §Release channels. No LTS pre-1.0.)

## Reporting a vulnerability

**Please do not open a public issue for security vulnerabilities.**

Report privately through GitHub's **[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)**
on the repository (Security → Report a vulnerability), or contact the maintainer through their GitHub
profile: **[@tmessaoudi-official](https://github.com/tmessaoudi-official)**.

Please include: a description of the issue, steps to reproduce (a minimal `.phg` program or input is
ideal), the affected version/commit, and the impact you observed. You can expect an initial
acknowledgement within a reasonable time; fixes are coordinated before public disclosure.

## Threat model & hardening notes

Phorj is a language toolchain, so the relevant attack surface is **untrusted input**:

- **Untrusted source programs.** The lexer, parser, and type-checker must reject malformed or
  adversarial programs cleanly (a diagnostic + non-zero exit), never with a panic, infinite loop, or
  unbounded memory growth. Recursion and nesting are explicitly depth-limited (`src/limits.rs`) on a
  fixed-size worker stack so pathological nesting faults cleanly.
- **Untrusted binaries (`phg build`).** The hand-rolled ELF / PE / Mach-O section readers used to
  detect an embedded program parse attacker-controlled object files. They perform **minimal section
  lookup with checked arithmetic on every offset** — malformed or hostile headers return `None`, never
  an overflow panic or out-of-bounds read (invariant **EV-7**). `#![deny(unsafe_code)]` is set on
  both crate roots; the audited first-party `unsafe` island is confined to `src/jit/` (the
  finalize→fn-ptr call plus the `extern "C"` trampolines' raw-pointer dereferences — ~48 audited
  sites) behind a scoped `#![allow]` and a CI `unsafe-island` gate.
- **A small, vetted dependency surface.** The core is std-first; the default build links 9 vetted,
  feature-gated crates (crypto, regex, signals, coroutines, JIT codegen, SQLite, Unicode
  segmentation) out of 14 admitted — each can be compiled out (`--no-default-features` links
  zero). See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) and
  `docs/specs/UNIFIED-SPEC.md#external-dependency-policy`.
- **Package manager (supply chain).** `phg add` / `install` / `update` / `remove` (DEC-316) and
  `phg build --target` (which downloads a cross-compile binary stub on cache miss,
  sha256-verified against a manifest baked into the binary before it is published to the cache)
  are the only commands that touch the network, and only on explicit invocation (`phg vendor` is
  retired — DEC-282 — and errors). Dependency names are validated at `phorj.json`-parse time
  (strict PascalCase alphanumeric `Publisher/Name` segments, `Core` reserved) so a name cannot
  traverse or escape the `vendor/` tree. Git fetching shells out to the host `git` binary; stub
  and registry fetching shell out to `curl` (`PHORJ_GIT` and `PHORJ_CURL` override which binaries
  are run, and `PHORJ_STUB_REGISTRY` redirects where stubs are fetched from — treat all three as
  security-sensitive). A dependency's `git` URL and `ref` are passed to `git clone`/`git checkout`
  as given (`https://…`, `file://…`, and local paths are all supported), so treat a third-party
  `phorj.json` with the same care as any build manifest you did not write. `run`/`check`/
  `transpile` never fetch — they resolve offline from the committed `vendor/`.
- **`phg serve` (HTTP runtime).** The server runs a **bounded OS-thread worker pool** (`--workers N`,
  default = number of CPU cores; `--workers 1` restores the single-threaded path). Each worker owns
  its own `Rc` value heap — values never cross threads — and handles one connection at a time. It is
  resilient — a per-connection read
  or send error, a request fault (→ 500), or a slow/idle client (bounded by `--timeout`, default 30s)
  never ends the server; only a persistently failing listener does. **Bind `127.0.0.1` (the default)
  on untrusted networks** and keep `--timeout` set. Note: the request body is capped (8 MiB) but the
  `Core.File` natives a handler may call do **no path sandboxing** — a served program that opens
  caller-influenced paths can read/write any file the server process can. Treat a `phg serve` program
  as you would any unsandboxed web app.

If you find input that causes a panic, crash, hang, or unbounded resource use, that is a bug we want
to hear about — please report it as above.

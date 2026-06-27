# Phorge external-dependency policy

> Status: **adopted** 2026-06-27 (developer). Phorge has been `std`-only / zero-dependency since
> inception. This document defines the **single, narrow exception** under which an external crate may
> be admitted, and records the first one.

## The rule

Phorge's core (lexer, parser, checker, interpreter, VM, transpiler, loader, bundler) **remains
`std`-only**. The build admits an external crate **only** when ALL of these hold:

1. **Domain is security-critical primitive crypto** (password hashing, AEAD, signatures, constant-time
   comparison) — a domain where the universal engineering rule is *"never roll your own."* No other
   domain qualifies. Convenience, performance, or parsing crates do **not**.
2. **The crate is independently audited / widely-vetted** (e.g. the RustCrypto org, ring) with an
   active maintenance record. A from-scratch or unaudited crypto implementation is **never** admitted —
   that would be *more* dangerous than the dependency.
3. **There is no `std`-only path that is both secure and Phorge-native.** Delegating the capability to
   the PHP transpile target is **not** an acceptable substitute: the transpile/lift bridges exist only
   to migrate from PHP and to test Phorge against a reference — Phorge's own runtime (the Rust
   interpreter/VM) must implement every feature natively. A feature that runs *only* after transpiling
   to PHP is a delegation and is disallowed.
4. **It is feature-gated** so the WASM playground (which must stay tiny + browser-safe) can build
   without it.

If a candidate fails any clause, the feature is deferred — it does not justify a dependency.

## Why crypto is the one exception

Rolling your own password hash / AEAD is the canonical security anti-pattern. `std` ships no crypto.
So a *secure, native* implementation has exactly one responsible source: a vetted crate. Clauses 1–3
make this a principled exception, not a slippery slope — the bar is "audited crypto primitive that
cannot be done safely in `std` and must not be delegated to PHP."

## Admitted dependencies

| Crate | Version | Domain | Used by | Feature gate | Justification |
|-------|---------|--------|---------|--------------|---------------|
| `argon2` (RustCrypto) | 0.5.x | Argon2id password hashing | `Core.Crypto` | `crypto` (default; off for `phorge-playground`) | OWASP #1 password KDF; audited; no `std` equivalent; must run on the Rust backends (not PHP-delegated). Emits standard PHC strings → interoperates with PHP `password_verify`. |

Transitive: `password-hash`, `base64ct`, `rand_core`/`getrandom` (salt entropy) — pulled by `argon2`,
same audit umbrella.

## Process to admit the next one

A new crate requires: (1) an entry in the table above with the clause-by-clause justification, (2) a
note in `CHANGELOG.md`, (3) feature-gating verified against the playground build. Anything outside the
crypto domain requires revisiting this policy itself, not just adding a row.

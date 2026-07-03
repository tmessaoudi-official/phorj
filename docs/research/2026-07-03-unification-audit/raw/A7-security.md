# A7 — Security Audit (application-level)

> Auditor: batch-2 agent A7 · Date: 2026-07-03 · HEAD: `0691228` (clean tree)
> Scope: anything that makes the language/runtime/tooling insecure — crypto misuse, injection,
> unsafe defaults, path traversal, DoS vectors, secret handling. NOT a memory-safety audit
> (`#![forbid(unsafe_code)]` confirmed by a sibling agent).
> Files read in full: `src/native/{crypto,hash,random,process,file,path,regex}.rs`,
> `src/limits.rs`, `src/vendor.rs` (guard section), `src/transpile/mod.rs` (escape family),
> `src/transpile/expr.rs` (emit_string), `src/transpile/program.rs` (regex helpers, reflect table),
> `Cargo.toml`, `Cargo.lock`; targeted greps across `src/`.

## Verdict summary

**0 High · 2 Medium · 5 Low (P3) · 2 Informational.** No exploitable-by-an-external-attacker
vulnerability found. The codebase shows a deliberately strong security posture (constant-time
compare, OWASP-default Argon2id, CSPRNG that fails loud, hardened `phg vendor`, bounded HTTP
server, documented recursion limits). The findings below are silent-weakening bugs, cross-backend
security asymmetries, and documentation gaps — not remote-attack surface.

---

## MEDIUM findings

### M-1. `Hash.pbkdf2` silently truncates the iteration count (`i64 as u32`)

- **Where**: `src/native/hash.rs:516-518` — guard is `*iters > 0` (i64), then `*iters as u32`.
- **What**: an iteration count above `u32::MAX` wraps to its low 32 bits. `pbkdf2(pw, salt,
  4294967296, 32)` (= 2^32) truncates to **0**, and `pbkdf2_sha256`'s inner loop `for _ in
  1..iterations` then runs zero extra rounds → the derived key is effectively **1 iteration**.
  Any value in `(u32::MAX, i64::MAX]` silently weakens the KDF instead of faulting.
- **Second-order**: the PHP emission (`hash_pbkdf2('sha256', …)` at hash.rs:600) receives the
  *untruncated* i64 → the transpiled program derives a different key than the native backends.
  That is a **byte-identity-spine violation** (Invariant 1) on top of the crypto weakening —
  PHP either computes with the full count or errors, while native computes with the wrapped one.
- **Exploitable?** Requires the Phorj program itself to pass an absurd-but-legal count, so not an
  external-attacker vector; it is a silent-crypto-weakening + parity bug. Fix is one guard:
  reject `iters > u32::MAX` (or accept and use `u64` internally).
- **Severity**: Medium. **Grade**: [Verified: read hash.rs:514-526 and the `1..iterations` loop
  at hash.rs:453; the `as u32` wrap and the PHP emission divergence are both visible in source].

### M-2. ReDoS asymmetry: natively-linear patterns can catastrophically backtrack on the PHP leg

- **Where**: `src/native/regex.rs` (native, `regex` crate) vs `src/transpile/program.rs:998-1050`
  (`__phorj_regex_*` helpers → `preg_*`/PCRE).
- **What**: the module doc (regex.rs:4-7) claims the crate's restricted feature set "is exactly
  the *regular* subset PHP `preg_*` matches identically". Feature-set parity ≠ complexity parity:
  a pattern like `(a+)+$` or `(a|a)*b` contains no backrefs/lookaround, so `Regex.compile`
  accepts it and the native engines match in linear time — but PCRE executes it by backtracking,
  which is exponential on a non-matching subject. On the transpiled leg this yields either a
  multi-second/minute stall or (past `pcre.backtrack_limit`, default 1M) `preg_match` returning
  `false` → a **semantic divergence** (native says "no match" after linear scan; PHP says "no
  match" because it *gave up*, or hangs). So: native runtime is ReDoS-immune (as designed), the
  transpiled artifact is not — a program that is safe under `phg run` can be DoS-able once
  deployed as PHP with attacker-controlled subjects.
- **Mitigating context**: "transpile is a bridge, not a runtime" (standing rule) — but the bridge
  is exactly the deployment path where untrusted subjects appear. Deserves at minimum a
  disclosure line in KNOWN_ISSUES (the existing KNOWN_ISSUES regex note covers only the
  `\d\w\s` Unicode/ASCII edge, per the program.rs:1001-1003 comment).
- **Severity**: Medium (disclosure/parity gap; real DoS only on the PHP leg with untrusted
  input). **Grade**: [Inferred: read both engines' invocation paths; PCRE backtracking behavior
  on nested quantifiers is standard documented behavior — not executed here].

---

## LOW findings (P3 / defense-in-depth)

### L-1. `constant_time_eq` is hand-rolled with no optimization barrier

- **Where**: `src/native/hash.rs:468-477`.
- **What**: the classic `diff |= a[i] ^ b[i]` loop. This is the accepted idiom and LLVM does not
  currently short-circuit it, but nothing *guarantees* constant time across compiler versions
  (no `black_box`, no `subtle`). Notably `subtle` is **already in the dependency tree**
  (Cargo.lock:368, pulled by `argon2`/`password-hash`) — `Hash.equals` is only compiled under
  the same `crypto`-adjacent build in practice, so using `subtle::ConstantTimeEq` behind the
  existing feature gate would cost nothing new policy-wise. The length-mismatch early return is
  intentional PHP `hash_equals` parity (length leak is documented in the doc comment) — fine.
- **Severity**: Low (defense-in-depth). **Grade**: [Verified: read the function; `subtle` presence
  verified in Cargo.lock].

### L-2. Hand-rolled SHA-256 now underpins keyed crypto (HMAC/HKDF/PBKDF2)

- **Where**: `src/native/hash.rs:297-464`.
- **What**: the original charter (hash.rs:5-8) scoped the hand-rolled digests as "checksums, not
  a MAC facility… deliberately not hand-rolled". W3-4 then built HMAC/HKDF/PBKDF2 **on top of the
  hand-rolled SHA-256** — keyed, secret-bearing primitives — which is in tension with the
  dependency policy's own "never roll your own" clause that admitted `argon2`. Mitigations are
  real: RFC known-answer tests + PHP-oracle byte-identity pin correctness, and SHA-256/HMAC have
  no data-dependent branches or table lookups (inherently constant-time on these inputs). This is
  a policy-consistency observation, not a defect found in the implementation (the HMAC key
  handling — hash-then-zero-pad at hash.rs:398-404 — is RFC-2104-correct).
- **Severity**: Low / policy note. **Grade**: [Verified: read the implementations; KAT coverage
  asserted by the module comment and `hash_tests.rs` existence — tests not re-executed].

### L-3. `Core.File`/`Core.Process.Environment` ambient authority is undocumented

- **Where**: `src/native/file.rs` (all natives), `src/native/process.rs:50-68`.
- **What**: `File.read/write/append/delete/rename/copy` pass paths straight to `std::fs` — no
  sanitization, no root confinement, full PHP-`fopen`-without-`open_basedir` trust model. That is
  a defensible by-design choice for a PHP-parity language, **but no documented statement of the
  trust model exists**: grep for `trust model|unrestricted|sandbox|sandboxing` across
  FEATURES.md, KNOWN_ISSUES.md, and `docs/specs/*.md` finds no filesystem-related hit. An
  undocumented capability is a footgun — e.g. anyone embedding the interpreter (playground-style)
  or running third-party `.phg` must currently discover by reading source that a Phorj program
  can read/delete any file the process can. One paragraph in FEATURES.md ("Phorj programs run
  with the full authority of the invoking user; there is no sandbox") closes this. The WASM
  playground already mitigates by excluding `Core.File`/`Process` importers (per project memory).
- **Severity**: Low (doc gap, not a code bug). **Grade**: [Verified: read file.rs in full; the
  negative doc claim verified by grep across FEATURES.md/KNOWN_ISSUES.md/docs/specs — no hits].

### L-4. Wrong escaper for single-quoted PHP context (latent, currently unexploitable)

- **Where**: `src/transpile/program.rs:1380` and `:1392` — `format!("'{}'", php_escape(n))`.
- **What**: `php_escape` (transpile/mod.rs:835-839) escapes `\ " $` for **double**-quoted PHP
  strings but does **not** escape `'`. Used inside single quotes: a value containing `'` would
  terminate the string → PHP code injection; a value containing `"` or `$` would gain a spurious
  backslash (PHP single quotes only process `\\` and `\'`). **Currently safe**: the values are
  class/member identifiers from `ClassTables`, and the lexer restricts identifiers to a
  quote-free charset — so there is no input that reaches the flaw today. It is a
  wrong-tool-for-context landmine: any future table keyed by user-influenced strings (or an
  identifier-charset widening) turns it into injection.
- **Severity**: Low (latent; no current path). **Grade**: [Verified: read php_escape and both call
  sites; identifier-charset constraint inferred from the lexer's role — the *absence* of a
  current attack path is Inferred, the escaper mismatch itself is Verified].

### L-5. Unbounded allocations reachable from program input (self-DoS class)

- `Random.secureBytes(n)`: `vec![0u8; n]` for any non-negative i64 `n`
  (src/native/random.rs:128-135) — a single call can request exabytes → abort/OOM.
- `Hash.pbkdf2` iterations (up to u32::MAX after M-1's wrap) and `nonneg_len` lengths — unbounded
  CPU/memory, program-authored.
- Regex pattern cache (src/native/regex.rs:23-29) — thread-local `HashMap` with no eviction; a
  program compiling unbounded distinct patterns grows it forever.
- **Context**: all are self-inflicted by the running program, which can equally `while(true)`
  allocate lists — the language has no resource-quota model, and `src/limits.rs` deliberately
  scopes its guards to *crash-class* (stack-overflow) inputs only. Listed for completeness, not
  as vulnerabilities. **Severity**: Low/informational. **Grade**: [Verified: read all three sites].

---

## INFORMATIONAL

### I-1. Deterministic default-seeded PRNG is predictable — by design, and honestly separated

`Core.Random.nextInt/nextFloat/intBetween` are xorshift64 with a **fixed default seed** (GOLDEN,
random.rs:35) — every unseeded program replays the same stream, and `intBetween` uses plain `%`
(modulo bias, random.rs:105). Both are fine *because* the module doc declares determinism as the
contract and W3-4 added the clearly-named `secureBytes`/`secureInt` CSPRNG pair. Residual risk is
user misuse (`intBetween` for a token); naming + docs make that a user error, not a library flaw.
[Verified: read random.rs in full]

### I-2. `zmij` in Cargo.lock is serde_json's float formatter (playground closure)

`zmij 1.0.21` (Cargo.lock:542) is depended on by `serde_json` (lock line 364, inside the
serde_json block) — the ryu-successor float writer. serde_json enters only via `wasm-bindgen`
in the `playground` workspace member; the core `phorj` crate does not link it. Not a policy
violation. [Verified: read Cargo.lock blocks]

---

## POSITIVE ATTESTATIONS (things checked that are RIGHT)

| # | Claim | Evidence |
|---|-------|----------|
| P1 | `Hash.equals` is constant-time for equal lengths; length-leak is intentional PHP `hash_equals` parity | [Verified: hash.rs:466-477 + registry entry 565-573] |
| P2 | `Cryptography.hashPassword` = Argon2id, fresh `OsRng` salt, and the crate defaults are the OWASP-recommended m=19456 KiB / t=2 / p=1 | [Verified: crypto.rs:24-36; params read from the actual registry source `/stack/tools/cargo/registry/src/…/argon2-0.5.3/src/params.rs:42-88`] |
| P3 | `verifyPassword` uses the library's constant-time verify; malformed hash → `false`, never a fault; the plaintext password is never echoed in any error path | [Verified: crypto.rs:41-56 — the only `format!` interpolates the argon2 error, not the password] |
| P4 | `Random.secureBytes/secureInt` are CSPRNG-backed (`/dev/urandom` read directly), **fail loud** on any entropy failure (never fall back to xorshift), and `secureInt` uses rejection sampling (no modulo bias, correct at the full i64 domain) | [Verified: random.rs:118-155 — traced the i128 arithmetic at the min=i64::MIN/max=i64::MAX extreme] |
| P5 | **No shell is ever involved in command execution.** `Core.Process` has NO exec capability at all (argv/env read-only). Every `Command::new` in the tooling (vendor, bench, bundle/cross, bundle/elf) uses the arg-vector API; zero `sh -c` sites | [Verified: `grep -rn "Command::new\|sh -c" src/` — 9 hits, all arg-vector; process.rs read in full] |
| P6 | `phg vendor` (the only network command) is hardened against git argument/transport injection: rejects leading `-`, case-insensitive `ext::`/`file::` remote-helpers, plus `--` separator and `protocol.ext.allow=never` at the call site; GIT_* env scrubbed; tested (vendor.rs:338-340) | [Verified: vendor.rs:139-198] |
| P7 | Transpiled-PHP string emission is injection-safe: `php_escape`/`php_escape_interp`/`php_escape_bytes` escape `\ " $` in every double-quoted context; interpolation holes are `$`-rooted, brace-checked chains; a Phorj literal `';system('…');//` cannot escape its PHP string | [Verified: transpile/mod.rs:835-884, expr.rs:480-530] |
| P8 | PCRE delimiter wrapping cannot be broken out of: `__phorj_regex_delim` picks a delimiter absent from the pattern (6 candidates) or escapes `~` in the fallback; a dangling-`\` pattern is rejected earlier at `Regex.compile` by the regex crate | [Verified: program.rs:1004-1013 + regex.rs:34-46] |
| P9 | `Core.Regex` really is the `regex` crate (linear-time, ReDoS-immune natively); default builder keeps the crate's 10 MB compiled-size cap, bounding repetition-blowup at compile | [Verified crate identity: regex.rs:38 `::regex::Regex::new`; size-limit value is the crate default — Inferred, not read from crate source] |
| P10 | `phg serve` has a deliberate DoS posture: 8 MiB request cap (serve.rs:564), per-connection read+write timeouts (306-307), keep-alive capped at 100 req/conn (609), accept-error loop cap (70), bounded worker pool, default bind `127.0.0.1:8080`, and an explicit "bind 127.0.0.1 on untrusted networks" warning in both the banner and help text | [Verified: serve.rs greps + main.rs:302-305, cli/mod.rs:209] |
| P11 | Crash-class input is centrally bounded and test-locked: MAX_CALL_DEPTH 4096 (both backends share one limit → fault parity), MAX_NEST_DEPTH 512, MAX_EXPR_DEPTH 10k | [Verified: limits.rs read in full incl. the lock test] |
| P12 | Runtime fault messages echo **type names**, not values (`invalid map key: {type_name}`) — no secret-bearing value reflection found on the sampled fault surfaces | [Verified: value.rs:323-424 sample; sampled, not exhaustive across all 30+ native modules] |
| P13 | Dependency inventory at HEAD: exactly the 4 vetted deps (`argon2`, `regex`, `ctrlc`, `corosensei`) + `wasm-bindgen` confined to the playground member; **rusqlite/rustls are NOT yet in the tree** (the 2026-07-03 dep-amendment is approved but unimplemented). All ~60 lock entries map to those closures (RustCrypto chain, regex-automata chain, nix/windows-sys, wasm-bindgen/serde_json/zmij). No typosquat-shaped or bad-reputation name found | [Verified: Cargo.toml read in full; Cargo.lock names enumerated. **Caveat**: `cargo-audit` is NOT installed (`which cargo-audit` → not found) and no network available for the RustSec DB → known-CVE status of the pinned versions is **Unverified**] |

## Recommended actions (ranked)

1. **M-1**: add `iters <= u32::MAX` guard to `Hash.pbkdf2` (one-line fix + one test + one
   differential case with a large count).
2. **M-2**: add a KNOWN_ISSUES disclosure (or a LADDER-style disclosure paragraph) that transpiled
   `preg_*` is backtracking and not ReDoS-immune, unlike the native engines.
3. **L-3**: one FEATURES.md paragraph stating the no-sandbox/full-user-authority trust model.
4. **L-4**: rename `php_escape` → `php_escape_dq` or add a `php_escape_sq` and switch the two
   single-quoted call sites (mechanical, removes the landmine).
5. **L-1**: consider `subtle::ConstantTimeEq` for `Hash.equals` under the existing crypto feature.
6. **P13 caveat**: install `cargo-audit` (or run it in CI) so the vetted-4 pins are checked
   against RustSec on every gate.

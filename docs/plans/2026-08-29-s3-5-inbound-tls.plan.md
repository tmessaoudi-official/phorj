# PLAN — S3.5: inbound TLS for `phg serve` (DEC-331 D7, build slice 3 of 3, item 5 of 5)

> Closes DEC-331 Slice 3. Spec: `docs/specs/2026-07-23-entry-kinds-serve-tls.md` §1/§2 D7, §5, §6 P2.
> Status at authoring: D7 **RULED, NOT BUILT** — `http-server-tls` does not exist, `rustls` is linked
> only by the outbound http-client.

## Decisions Log

- [2026-08-29 01:20] AGREED: build S3.5 next (developer, `AskUserQuestion`) — the last item of Slice 3.
- [2026-08-29 01:30] AGREED: **no new crate.** `rustls` 0.23.43 has no client/server feature split;
  `ServerConfig`, `ServerConnection`, `StreamOwned` and `version::{TLS12,TLS13}` are all reachable
  with the existing `["ring","std","tls12","logging"]` set, and `rustls-pki-types` is already in
  `Cargo.lock` as a transitive. So `http-server-tls = ["dep:rustls"]` and nothing else. This is what
  makes S3.5 a feature gate rather than a dependency-policy adjudication.
  [Verified: rustls-0.23.43 `Cargo.toml:56-90`, `lib.rs:566,663,672`; `Cargo.lock:2067,2082`.]
- [2026-08-29 01:35] AGREED: **TLS resolution stays OUT of `serve::settings::resolve`.** That
  function is the flag-vs-config PRECEDENCE rule, and D7 rules there is no `--tls` flag — so there is
  no precedence to resolve. TLS is read from the config directly in `prepare_serve`.
- [2026-08-29 01:35] AGREED: **feature-off is an uninhabited type.** `#[cfg(not(feature =
  "http-server-tls"))] pub enum TlsServer {}` makes `Option<TlsServer>` provably `None`, so the
  compiler — not a comment — guarantees no build without the feature can serve plaintext while the
  program asked for TLS.
- [2026-08-29 01:40] AGREED: the example is a **project** (`examples/web/serve-tls/`), mirroring
  `examples/web/core-http/`. A single-file `examples/web/serve_tls.phg` would be COLLECTED and RUN by
  the byte-identity corpus glob, where a Web-only entry now fails `E-NO-ENTRY-FOR-ROLE` (S3.4) — the
  corpus gate would break. `collect_phg` returns early on any dir containing `src/`.
  [Verified: `tests/differential.rs:1929-1932`; corpus run shows every `examples/web/*.phg` RUNNING.]
- [2026-08-29 01:40] AGREED: the handshake test **generates a throwaway self-signed cert with the
  `openssl` CLI at test time**, skip-loud when absent. No private key is committed — which also keeps
  the repo clear of secret-scanner / push-protection trips.

## 1. What ships

`Http.serve` gains TLS termination. A registered `ServeConfig` with **both** `cert` and `key` set
makes `phg serve` bind HTTPS; the banner says `https://`. `tlsMinVersion` (`"1.2"` | `"1.3"`, default
`"1.2"`) is the floor. Terminating TLS only.

**Three refusals, none of them silent** — the whole point of the slice, because every failure mode
here degrades to *plaintext on a port the operator believes is encrypted*:

| Condition | Code |
|---|---|
| exactly one of `cert`/`key` set | `E-SERVE-TLS-INCOMPLETE` |
| `tlsMinVersion` not in {`"1.2"`,`"1.3"`} | `E-SERVE-TLS-MIN-VERSION` |
| cert+key set, binary built without `http-server-tls` | `E-SERVE-TLS-DISABLED` |

Plus the I/O-shaped failures (unreadable cert path, malformed PEM, key/cert mismatch) which surface
as `E-SERVE-TLS-CERT` naming the path and the underlying error.

**Ordering is ruled and pinned:** `requested()` runs before `build()`, so a lone `cert` on a
feature-OFF build reports `E-SERVE-TLS-INCOMPLETE`, not `-DISABLED`. The config is broken regardless
of how the binary was compiled, and reporting the build first would send the reader to rebuild a
binary that still would not serve.

## 2. Files

| File | Change |
|---|---|
| `Cargo.toml` | `http-server-tls = ["dep:rustls"]`; correct the `rustls` comment (it says "admitted EXPLICITLY to gate this client" — now two consumers) |
| `src/serve/tls.rs` | **NEW.** The pure rule (`requested`), the codes, PEM decoding, the rustls `ServerConfig` build, `TlsServer`, `Conn` |
| `src/serve/framing.rs` | **NEW.** Carve-out from `transport.rs`: `read_http_request`, `request_wants_keepalive`, `response_keeps_alive`, `head_value`, `token_list_has`, `parse_content_length`, `find_subslice` + their tests |
| `src/serve/transport.rs` | Thread `Option<&TlsServer>` through `serve_tcp` / `serve_pool_with` / `worker_loop` / `TcpTransport`; wrap the accepted stream in `Conn`; `https://` banner. **Must SHRINK** past its 635 ratchet — the carve-out is what buys that |
| `src/serve/mod.rs` | `mod tls; mod framing;` + re-exports |
| `src/cli/serve_pipeline.rs` | `prepare_serve` returns the `Option<TlsServer>` third element; `serve_program` threads it |
| `src/cli/explain/…` | three (four with `-CERT`) new explain rows |
| `scripts/surface-baseline.txt` | codes_total / codes_asserted +4 |
| `scripts/size-baseline.txt` | `transport.rs` row re-emitted at its new, lower count |
| `examples/web/serve-tls/` | **NEW project**: `serve.phg` (Web entry, cert/key) + `src/main.phg` (Cli) |
| `examples/web/README.md` | the walkthrough: `openssl req -x509 …`, `phg serve`, `curl --cacert`, the three refusal texts |
| `tests/serve_tls.rs` | **NEW.** Rule tests, refusal tests, ordering pin, and the openssl-gated handshake + floor tests |

**Not wired, stated rather than absorbed:** `maxBodySize` (belongs to the wire parser) and
`serverName` (no consumer) stay unread — spec §5's test list covers the whole D-cluster, not D7.
**`cert`/`key` paths resolve relative to the process cwd**, not the site-mode app root; stated here
because a site-mode user could reasonably expect otherwise. Deferred per the spec, into
KNOWN_ISSUES: HTTP→HTTPS redirect, HSTS, cert hot-reload, mTLS.

## 3. Correctness notes that cost me something to establish

- **Timeouts and blocking mode must be set on the raw `TcpStream` BEFORE it is wrapped.** Both accept
  paths already do `set_nonblocking(false)` + `set_read_timeout`/`set_write_timeout` on the accepted
  stream (`transport.rs:97-99` pool, `:100-104` single-threaded). Handing a non-blocking socket to
  rustls fails the handshake immediately. Keeping that order also means the read timeout bounds the
  **handshake**, not just the request — which is what stops a TLS-level slowloris.
- **The handshake happens lazily inside the worker, never in the accept loop.** `StreamOwned` drives
  it on first read, so a slow client cannot serialize `accept()`. A failed handshake surfaces as a
  read error and lands in the existing "dropping connection (read error)" arm — no new path.
- **`TlsServer` must not live in `ServeSettings`**, which derives `PartialEq, Eq`; `rustls::ServerConfig`
  has neither.
- **`ServerConfig::builder_with_protocol_versions` `.unwrap()`s internally.** Use
  `builder_with_provider(ring::default_provider()).with_protocol_versions(…)` and handle the
  `Result` — it cannot fire with ring+tls12, but a handled Result costs nothing and the repo's
  no-crash posture is worth more than the two saved lines.
- **`serve_pool` keeps its signature; only `serve_pool_with` gains the parameter.** That mirrors the
  existing shutdown-flag split and holds the test fan-out to one call site out of nine.

## 4. Verification

Test-first, in `tests/serve_tls.rs`: the rule tests go red against a stub `requested` that returns
`None`, before any rustls code exists.

- **Certified by execution, feature-off** (the default gate): rule tests, all three refusals, the
  incomplete-before-disabled ordering pin.
- **Certified by execution, `--features http-server-tls`**: a real client handshake against a real
  bound listener with an openssl-generated self-signed cert; a client trusting nothing uses rustls's
  `danger::ServerCertVerifier`. **Plus the negative floor test** — a client capped at TLS 1.2 against
  `tlsMinVersion: "1.3"` must FAIL. That is the test that proves the floor is *wired*, not merely
  parsed; without it, `tlsMinVersion` could be dropped on the floor and every positive test stays green.
- **Sabotage**: (a) make `requested` return `None` when only one of cert/key is set → the incomplete
  test must go red; (b) drop the floor from the version list → the negative floor test must go red.
  Restores checksum-verified.
- **The completion report counts non-skips** for the openssl-gated tests. A skipped TLS suite
  reported as green is precisely the example-glob-noop failure, and it is not going to happen twice.

**Perf (Invariant 18):** `php -S` has no TLS, so there is **no PHP equivalent to bench** — recorded as
no-equivalent per DEC-371, not as an OWED verdict. The plain-HTTP path gains one enum discriminant
per I/O op; no claim is made about it without a measurement.

## 5. Milestone

S3.5 closes DEC-331 Slice 3, which is a **milestone boundary** under the economize ruling: the full
three-lens panel is due at the end, on the frozen commit — and it is the natural place to resolve or
re-record the still-OWED G-8 microbench verdict.

---

## 6. What shipped (filled in at completion)

**Code.** `src/serve/tls.rs` (the rule, the refusals, `TlsServer`, `Conn`), `src/serve/pem.rs` +
`pem_tests.rs` (the decoder), `src/serve/framing.rs` (the Invariant-13 carve-out),
`src/cli/explain/serve_tls.rs` (four explain rows). Threading in `src/serve/transport.rs`,
`src/serve/mod.rs`, `src/cli/serve_pipeline.rs`, `src/cli/help.rs`.

**Two things went differently from the plan, both worth recording:**

* **`src/serve/tls.rs` came out at 364 lines**, past Invariant 13's 300-line soft cap, so the PEM
  decoder was split into `src/serve/pem.rs` in the same pass rather than left for later. Encoding and
  TLS policy change for entirely different reasons — it is a cohesion cut, not a line-count one.
* **The base64 decoder shipped with two unreachable guards** (`pad > 2`, `bits >= 8`) that a
  self-review removed: `pad` is 0/1/2 by construction and `bits < 8` is a loop invariant. Defensive
  code with no reachable failure mode is what the anti-bandaid gate exists to catch, and it reads to
  a later reader as though those cases were observed.

## 7. Found in passing

**The first handshake-tier run was a false green, caught by counting non-skips rather than by the
verdict.** `cargo nextest -E 'test(serve_tls) or test(pem)'` filters on test NAME, not binary — so of
the four `handshake::` tests only `a_malformed_pem_is_refused_rather_than_ignored` matched (on the
substring "pem"), and the run reported `9 passed` while the actual handshake, the version floor and
the missing-cert path had never executed. Reporting that as evidence would have been exactly the
example-glob-noop failure repeated. The re-run drops the filter and selects binaries
(`--lib --test serve_tls`).

Lesson, already this repo's rule and now paid for twice: **a green verdict is not evidence a gate
ran.** Count the non-skips and name the tests you expected to see.


## 8. End-to-end evidence (the 6C finding that mattered)

`advisor()` found, correctly, that **the shipped wiring had never carried a byte of TLS traffic.**
`tests/serve_tls.rs::handshake` binds its OWN `TcpListener` and drives `rustls::StreamOwned` directly,
so it proves the RULE and the rustls configuration — and touches none of `serve_tcp`,
`serve_pool_with`, `worker_loop`, `TcpTransport::recv` or `Conn::accept`. Every line changed in
`transport.rs` was covered by nothing. Same shape as the 2026-08-21 panel that read a diff three ways
and never asked whether the tool worked.

Closed by running the README's walkthrough verbatim against a `--features http-server-tls` release
binary, on **both** accept paths (both were modified):

| Path | Banner | `curl --cacert … https://localhost:8443/hello` | plaintext client |
|---|---|---|---|
| pool (default, 8 workers) | `phg serve: listening on https://127.0.0.1:8443` | `served over TLS: /hello`, exit 0 | rejected (`Received HTTP/0.9 when not allowed`) |
| single-threaded (`--workers 1`) | same + `W-SERVE-CONFIG-OVERRIDDEN (8 → 1)` | `served over TLS: /hello`, exit 0 | rejected |

The banner matches the line the README quotes, character for character. The throwaway cert was
generated into the gitignored `certs/` and deleted afterwards.

**Also corrected as part of that finding:** the CHANGELOG, the DEC-455.16 row and the commit message
all claimed the two ordering facts were "tested rather than commented". They are not — no test drives
the worker path with TLS. All three now say they are exercised end-to-end instead. A completion
artifact that overclaims its own verification is the exact failure this repo's certification section
names.

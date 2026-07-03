# W3-2 · HTTP Client — Design Doc (`Core.Http` client + `Core.Url`)

> Status: **DESIGN — not yet implemented.** Research doc for the developer's adjudication.
> Scope: MASTER-PLAN §Wave 3 W3-2 (all 13 FN-CURL rows; folds DEF-030 / URL breadth / four-lane Q3).
> Grounding: read `docs/plans/MASTER-PLAN.md` W3-2, `docs/specs/2026-06-27-dependency-policy.md`,
> `docs/INVARIANTS.md`, DEC-007/008/009 (`C-decisions.md`), `src/cli/mod.rs` HTTP_PRELUDE,
> `src/serve.rs` (std-net transport), `tests/differential.rs` `uses_impure_native` seam.

---

## 0. TL;DR — the one decision that gates everything

Phorj can build a **plaintext HTTP/1.1** client entirely in `std` (the server side already frames
HTTP/1.1 over `std::net::TcpStream` — see `serve.rs`). It **cannot** do **HTTPS** without a TLS crate:
`std` has TCP but **no TLS**, and hand-rolling TLS is the canonical "never roll your own crypto"
disaster. The dependency policy **rejects general HTTP crates today** and TLS sits *outside* the four
enumerated exception domains — so **admitting a TLS crate requires a policy revisit, i.e. an explicit
developer ruling** (the roadmap already anticipates this: W3-2 DEPS = *"rustls dep authorization,
DEC-009"*). This is the #1 adjudication and mirrors the W3-1 DB-driver dilemma. **Nearly every real
API is HTTPS**, so the client's value is downstream of this ruling. Only the **`Core.Url` parser (Tier
A, pure, zero-dep)** is genuinely unblocked and shippable on its own merits.

---

## 1. API surface — `Core.Http` client + `Core.Url`

### 1.1 Value types — reuse `Response`, add `HttpRequest` (do NOT reuse server `Request`)

The injected server `Request` (`src/cli/mod.rs` HTTP_PRELUDE) is **path/attrs-shaped**
(`method, path, body, headerLines, attrs`) — built around a *routed server request* with `param()`
lookups. A client request is **URL-shaped** (absolute URL, host resolution, query string). Forcing the
server type to carry a client's URL harms both. So:

- **Reuse `Response`** verbatim (`status, body, headerLines` + `text()`/`reason()`/`serialize()`).
  It already fits a client response 1:1. The client adds a **wire-parse mirror** (see §1.4).
- **New immutable builder `HttpRequest`** (client-shaped), mirroring the Router's chainable-immutable
  style already proven in HTTP_PRELUDE:

```phorj
class HttpRequest {
  constructor(public string method, public string url,
              private List<string> headerLines, public bytes body,
              private int timeoutMs) {}
  static function of(string method, string url): HttpRequest { ... }   // fresh builder
  function header(string name, string value): HttpRequest { ... }      // chainable, immutable
  function body(bytes b): HttpRequest { ... }
  function timeout(int ms): HttpRequest { ... }
  function serialize(): bytes { ... }   // CLIENT-side wire form (mirror of Response.serialize)
}
```

### 1.2 Natives — `Core.Http` (all `pure: false` ⇒ auto-quarantined, see §4)

| Native | Signature | Notes |
|---|---|---|
| `Http.get`  | `(string url): Response` | convenience |
| `Http.post` | `(string url, bytes body): Response` | convenience |
| `Http.send` | `(HttpRequest req): Response` | the general entry (put/patch/delete/head via builder method) |

All impure (network) ⇒ `NativeFn::pure = false`. This single flag is what quarantines them from the
differential (§4) — **no harness edit needed** (the `uses_impure_native` seam derives the impure set
from the flag, `tests/differential.rs:1027`).

### 1.3 Middleware (folds four-lane **Q3** — the HTTP-client callable pattern)

Reuse the **exact** compose pattern already shipped for the server Router:
`(HttpRequest, next) -> Response`, folded outermost-first via a `Http.compose`-style helper mirroring
`Router.compose`. This gives retry/auth/logging middleware with zero new concepts — the symmetry with
the server side is the recommendation for Q3 (§6-Q3).

### 1.4 The framing is a MIRROR of the server, not literal reuse

Server has `Request.parse` (wire→value) + `Response.serialize` (value→wire). The client needs the
**opposite pair**: `HttpRequest.serialize` (value→wire) + `Response.parse(bytes) -> Response?`
(wire→value). Same proven std-net machinery, but **new code**. And unlike the server (which always
emits `Content-Length`), a client parsing *arbitrary* servers must also handle **chunked
transfer-encoding, redirects, and read timeouts** — real work the server never faced. Do not
overclaim "just reuse serve.rs framing."

### 1.5 `Core.Url` — Tier A, PURE, zero-dep (folds DEF-030 + LI-D7 URL breadth)

The **only** genuinely-unblocked deliverable. Spec-compliant parser that leapfrogs PHP's
non-conformant `parse_url` (the G-url plan):

```phorj
class Url {
  public string scheme; public string host; public int? port;
  public string path;   public string query; public string? fragment;
  static function parse(string s): Url?           // RFC 3986 / WHATWG
  static function buildQuery(Map<string,string> m): string   // = PHP http_build_query
}
```

Pure ⇒ **byte-identity-gated** (run ≡ runvm ≡ PHP) with WHATWG/RFC test vectors. Ships independently
of the client and the TLS ruling.

---

## 2. PHP transpile mapping (Tier B — quarantined, NOT byte-identity)

Native-first is non-negotiable (policy clause 3 + invariant 14): the client runs on the **Rust
backends**; PHP is a migration/test *bridge only*, never the runtime. The mapping still exists and is
fixture-tested, but is **excluded from the byte-identity oracle** because network I/O is
non-deterministic.

| Phorj | PHP idiom (recommended: `curl_*`) |
|---|---|
| `Http.get(url)` | `curl_init` → `curl_setopt(CURLOPT_RETURNTRANSFER, true)` → `curl_exec` → `curl_getinfo` (status) → `curl_close` |
| `Http.post(url, body)` | + `CURLOPT_POST` / `CURLOPT_POSTFIELDS` |
| `HttpRequest.header()` | `CURLOPT_HTTPHEADER` array |
| `.timeout(ms)` | `CURLOPT_TIMEOUT_MS` |
| `Url.buildQuery` | `http_build_query` (exact) |

**Why `curl_*` over stream contexts:** the M-gap is literally the 13 **FN-CURL** rows; `curl_*` gives
full header/status/timeout control and is the idiomatic port target. Streams
(`file_get_contents` + `stream_context_create`) is a fallback for the trivial GET only.

`Core.Url.parse` / `buildQuery` are **pure** ⇒ their PHP mapping (`http_build_query`, hand-emitted
parse) **stays in the byte-identity spine** — only the network natives are Tier B.

---

## 3. Dependency stance — THE HARD FORK (mirror of the DB dilemma)

### 3.1 What `std` gives and what it doesn't

- **HTTP/1.1 wire framing over `std::net::TcpStream`** — ✅ genuinely std-doable, **no dep**. The
  policy itself says HTTP *parsing* is "done in std today"; `serve.rs` proves it server-side. This
  half is airtight.
- **HTTPS / TLS** — ❌ **impossible in `std`.** `std` has TCP, no TLS. Hand-rolling a TLS stack is the
  strongest imaginable "never roll your own" case (worse than hand-rolling a hash).

### 3.2 The three logical paths for HTTPS — only one is admissible, and it needs a ruling

1. **Authorize a TLS crate (`rustls`), feature-gated.** — TLS fits the policy's *principle*
   (std-impossible + strongest "never roll your own"), **but it is NOT one of the four *enumerated*
   domains** (crypto=hash/AEAD/signature/const-time, regex, signals, coroutines). rustls is a
   *protocol* built on crypto primitives, not a listed primitive. Per the policy's own *"Process to
   admit the next one"*: anything outside the four domains **"requires revisiting this policy itself,
   not just adding a row."** So this is a **developer policy-revisit / adjudication**, not a
   pre-decided "the crypto clause covers it." The roadmap already frames it exactly this way
   (*"rustls feature-fork, dep authorization required, DEC-009"*). If admitted: gate behind `tls`/
   `https` (off for `phorj-playground`, like every other optional dep).
2. **Transpile-only HTTPS (delegate to PHP `curl`).** — **FORBIDDEN.** Policy clause 3: a feature that
   runs *only* after transpiling to PHP is a delegation and is disallowed. Invariant 14 (LADDER):
   native-absent / silent downgrade is forbidden.
3. **Defer HTTPS.** — Ship plaintext-only. But plaintext-only is a **footgun** (real APIs are HTTPS);
   its value is contingent on P2 arriving.

**Explicitly rejected:** general HTTP client crates (`reqwest`, `hyper`, `ureq`, `curl`) — these are
the exact "general-purpose / HTTP" crates clause 1 rejects. We admit (if anything) a **TLS transport
primitive**, never an HTTP framework.

### 3.3 Recommendation (see §6-Q1)

Authorize `rustls` as a **feature-gated fifth "transport-security" domain** via a policy revisit —
because the alternative is a toy client. Framed as the developer's ruling, recommended YES.

---

## 4. Determinism / Transport model (Tier-B quarantine + fixtures)

### 4.1 Quarantine mechanism (already exists — reuse it, no new machinery)

`tests/differential.rs::uses_impure_native` derives the impure module set from `NativeFn::pure` and
**skips** any example importing an impure module (line 1127). Marking every `Core.Http` network native
`pure: false` ⇒ examples importing `Core.Http` client natives are auto-excluded from the byte-identity
oracle. This is the DEC-007 Determinism Partition: **Tier A** (Url parser — pure, byte-identity) vs
**Tier B** (client — impure, quarantined). Same seam as `Core.File` mutation ops and `Core.Process`.

### 4.2 Transport seam for deterministic tests (A-TEST-6)

- **Real transport:** blocking `std::net::TcpStream` (+ rustls stream when `tls` on) — mirror of
  `serve.rs`'s `TcpListener` transport.
- **Test fixtures:** MASTER-PLAN ACCEPTANCE mandates *"fixture-tested against a local `phg serve`
  instance (loopback only, A-TEST-6)."* Spin a loopback `phg serve` on an ephemeral 127.0.0.1 port
  serving **committed deterministic responses**, point the client at it → deterministic round-trip.
  No recorded-cassette layer needed (we own both ends). Optionally a `Transport` trait so a unit test
  can inject a fixed-bytes transport (mirrors serve.rs's transport abstraction) for
  chunked/redirect/timeout edge cases without a live socket.

### 4.3 Determinism invariant (invariant 10)

`run`/`check`/`transpile` never touch the network — only *executing* an `Http.*` native does, and only
at runtime. Any user-facing list derived from headers (a `HashMap`) is sorted before rendering.
`phg vendor` remains the *only* network command at compile/vendor time; the client is a **runtime**
capability, so it does not widen the compile-time network surface.

---

## 5. Phasing (ordered by what's actually unblocked)

| Phase | Deliverable | Dep | Gate |
|---|---|---|---|
| **P0** | `Core.Url` parser + `buildQuery` (folds DEF-030) | none | **Tier A, byte-identity + RFC/WHATWG vectors.** Ship independently — unblocked today. |
| **P1** | `Core.Http` **plaintext** client (builder, get/post/send, `Response.parse` mirror, middleware) over std TcpStream | none | Tier B, loopback-fixture-tested. **Value contingent on P2** (see note). |
| **P2** | **HTTPS** via authorized TLS crate behind `tls` gate | **rustls (BLOCKED on §6-Q1 ruling)** | Tier B. Until authorized, `https://` URLs → hard **`E-HTTP-NO-TLS`** (LADDER-honest, never silent plaintext). |
| **P3** | Redirect-follow, chunked decode, read timeouts, connection pooling on green threads (`corosensei` already admitted) | none new | Tier B. |

> **Phasing caveat (advisor):** P1 and P2 are **not** independent increments. Plaintext-only is a
> footgun if HTTPS never lands. Recommendation: resolve §6-Q1 **before** committing to P1; if the
> ruling is "no TLS," reconsider whether the client ships at all vs. Url-parser-only. P0 is safe
> regardless.

---

## 6. Open questions for the developer (invariant 15 — recommended-first, with why)

Each ships to the developer with a minimal current-syntax failing program embedded and after-state in
per-option previews. Summarized here:

**Q1 (THE fork) — Admit a TLS crate (`rustls`) via a policy revisit, for HTTPS?**
- *Failing program:* `import Core.Http; Http.get("https://example.com");` → today: no such native /
  no TLS.
- **Recommended: YES — feature-gated `tls` (off for playground), framed as a fifth "transport-security"
  domain in a policy revisit.** Why: HTTPS is std-impossible and the strongest "never roll your own"
  case; a plaintext-only client is toy-grade (real APIs are HTTPS); transpile-only is forbidden
  (clause 3 + invariant 14). The roadmap already anticipates rustls-pending-authorization (DEC-009).
  This is a **developer ruling**, not auto-covered by the existing crypto clause.
- *Alt:* defer HTTPS (ship Url + plaintext only); or reject entirely (client deferred).

**Q2 — Request value shape: reuse server `Request`, or new `HttpRequest`?**
- **Recommended: NEW `HttpRequest`, REUSE `Response`.** Why: server `Request` is path/attrs-shaped;
  forcing it to carry a URL harms both sides. `Response` fits a client response 1:1.

**Q3 (folds four-lane Q3) — HTTP-client callable/middleware pattern?**
- **Recommended: same `(req, next) -> Response` compose as the server Router.** Why: proven,
  symmetric, zero new concepts.

**Q4 — PHP transpile idiom: `curl_*` or stream contexts?**
- **Recommended: `curl_*`.** Why: the M-gap is literally the 13 FN-CURL rows; full header/status/
  timeout control. Streams only as the trivial-GET fallback.

**Q5 — Behaviour when `https://` requested but TLS unauthorized/unbuilt?**
- **Recommended: hard `E-HTTP-NO-TLS`.** Why: LADDER invariant 14 forbids silent downgrade to
  plaintext; the error is explicit and honest.

---

## 7. Acceptance / tests

- **`Core.Url` (Tier A):** byte-identity across run/runvm/**real PHP 8.5** for a full URL matrix; RFC
  3986 + WHATWG conformance vectors as unit tests; `buildQuery` golden vs `http_build_query`.
- **`Core.Http` (Tier B):** `tests/http.rs` — start loopback `phg serve` on ephemeral 127.0.0.1,
  assert get/post/headers/status/body round-trip; `Transport`-trait unit tests for
  chunked-decode / redirect-follow / timeout using injected fixture bytes (no live socket).
- **Quarantine proof:** an `examples/web/*.phg` importing `Core.Http` client natives must be **skipped**
  by `all_examples_match_between_backends` (asserts the `pure:false` seam fires); example uses the
  `pure: false` convention with a walkthrough README (invariant 9 — faults/network captured in README,
  not as gated runnable output).
- **Transpile golden:** `Http.*` → `curl_*` emitted PHP compiles and (in a network-enabled canary,
  non-gating) round-trips; the byte-identity oracle never sees it.
- **Full correctness gate green** before "done":
  `PHORJ_PHP=/stack/tools/phpbrew/php/php-8.5.7/bin/php PHORJ_REQUIRE_PHP=1 cargo test --workspace`
  + clippy + fmt + release build. Report `target/release/phg`.
- **Deps:** if Q1 = YES, add the `rustls` row to the dependency policy table with clause-by-clause
  justification + CHANGELOG note + playground-build feature-gate verification (policy §"Process to
  admit the next one").

---

## 8. Risks / MUST-CHECK

- **Chunked / redirects / timeouts are NEW** (server never framed them) — the largest hidden scope in
  P1/P3, not covered by reusing serve.rs.
- **`HttpRequest.serialize` + `Response.parse` are mirror-new** — client-side wire code, single-source
  any framing constants shared with the server prelude rather than duplicating.
- **`Core.Url` must be spec-compliant** (WHATWG/RFC), *not* a `parse_url` clone — the whole point of
  DEF-030 is to leapfrog PHP's non-conformance; a naive port re-imports the bug.
- **Playground build** must stay dep-free — `tls`/`https` feature OFF for `phorj-playground` (verify).
- **LADDER (invariant 14):** every HTTPS-absent path is a hard error, never a silent plaintext or
  PHP-delegated fallback.

---

STATUS: Designed — not yet implemented. Q1 (TLS/rustls policy revisit) is the blocking developer
adjudication; P0 (`Core.Url`) is shippable regardless.

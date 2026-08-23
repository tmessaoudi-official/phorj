# Web examples — the M6 HTTP story

Phorj's web model is **a `(Request) => Response` handler at the value level** (PSR-7/15 shaped). The
request/response types and the parse/route/serialize logic are **pure Phorj**, byte-identity-gated
on both backends like every other example. Two thin, untranspiled runtimes carry those bytes over
a real socket — one native, one PHP — and both call the *same* handler.

**Since DEC-331 D5, a program registers its handler by calling `Http.serve(cfg, handler)` from a
`(): void` `Web` entry.** The pre-D5 forms — a named `respond(bytes): bytes` entry, and a
`handle(Request): Response` entry that a synthesized bridge wrapped — are both RETIRED: `respond`
went with S3.3c, and S3.3d narrowed the checker so a `(Request): Response` entry no longer
type-checks at all (`E-ENTRY-SIG`, with a migration hint). `phg explain E-SERVE-NO-HANDLER` carries
a before/after.

**Why three of these are directories.** `phg transpile` refuses any file that calls `Http.serve`
(`E-TRANSPILE-SERVE`, Invariant 14 tier 2 — PHP is served BY a web server rather than being one).
A file that registers therefore has no PHP leg. Splitting each servable app into `src/` (the logic,
driven by a CLI entry and gated through real PHP by the project harness) plus a sibling `serve.phg`
(the `Web` entry, gated by `tests/serve.rs`) is what keeps the *logic* byte-identity-gated while the
untranspilable registration sits in its own compilation unit. Neither differential glob collects a
`serve.phg`: `collect_phg` skips any directory containing `src/`, and the project harness only ever
picks a file named `main.phg`.

| File | What it is |
|---|---|
| `handler.phg` | **W1** — the handler model built BY HAND: `Request`/`Response` classes, `parseRequest(bytes) -> Request?`, `serializeResponse(Response) -> bytes`, and an ordinary `handle(Request) -> Response`. Bodies are `bytes`; headers are raw `List<string>` lines behind `req.header(name)`. No socket, and deliberately NOT servable — this file exists to show the wire format, and `Http.serve` takes `Core.Http`'s types, not these. Same shape as `rich_request.phg`: a CLI entry driving an ordinary handler. |
| `core-http/` | **`Core.Http`** — the same model promoted to the STDLIB: `import Core.Http;` gives `Request`/`Response` with `Request.parse`, `req.headers.get`, `Response.text`, `resp.serialize()`. `src/main.phg` drives it over canned requests (PHP-gated); `serve.phg` serves it. |
| `json-api/` | **`Core.Json` + `Core.Http`** — POST a JSON array, get `{"count": N, "sum": S}` back; a non-array body is a 400 whose body is itself JSON. `src/` holds the endpoint, `serve.phg` serves it. |
| `router.phg` | **W2** — a static exact-match router: a `List<Route>` table + linear `(method, path)` scan → a `Handler` enum tag → exhaustive `match` dispatch. Pure Phorj, no new language feature. |
| `server/` | **W4** — the full served app: `Core.Http` parse/serialize + W2 routing, over three files. `src/Acme/Routing/` is the route table and dispatch (this example's actual contribution); `src/main.phg` drives the whole pipe over canned wire requests and keeps its PHP leg; `serve.phg` is what `phg serve` runs. `server.php` is the `php -S` front-controller that calls the SAME transpiled code. |
| `response-builders.phg` | **DEC-220 S2** — the `Response` builders (the browser-bound sink of the 3-sink output system): `Response.html/json/text` constructors + immutable chainable `.status(n)`/`.withHeader(k,v)`/`.withCookie(k,v)`. Headers-before-body is structural (Response is a value), so PHP's "headers already sent" is impossible. Pure value construction ⇒ byte-identical on both backends and real PHP. |
| `password-verify.phg` | **`Core.Cryptography`** — verify a password against a committed Argon2id PHC hash. Deterministic ⇒ byte-identity-gated; the non-deterministic `hashPassword` is documented below. |

## `Core.Cryptography` — password hashing (Argon2id)

Secure password hashing follows the one inviolable rule — **never roll your own crypto**. Phorj
implements it natively on the Rust backends via the audited RustCrypto **`argon2`** crate (the sole
external dependency, admitted under `docs/specs/UNIFIED-SPEC.md#external-dependency-policy`); the transpile
bridge emits PHP's `password_hash`/`password_verify` as a *peer* target. Both speak the standard PHC
string (`$argon2id$…`), so **a hash made by either backend verifies in the other**.

```phorj
package Main;
import Core.Output;
import Core.Cryptography;

function main(): void {
    // hashPassword uses a fresh random salt → a different string every call (this is correct).
    string hash = Cryptography.hashPassword("correct horse battery staple");
    Output.printLine(hash); // e.g. $argon2id$v=19$m=...$.../...

    // Verify is deterministic for a fixed (password, hash) pair.
    Output.printLine("{Cryptography.verifyPassword(\"correct horse battery staple\", hash)}"); // true
    Output.printLine("{Cryptography.verifyPassword(\"wrong\", hash)}");                        // false
}
```

- **`Cryptography.hashPassword(password: string) -> string`** — Argon2id over a random salt; returns the PHC
  string. **Non-deterministic** (random salt) ⇒ it is *quarantined* from the byte-identity oracle and
  has no runnable gated example (its output differs every run by design); it is covered by
  `tests/crypto.rs` instead.
- **`Cryptography.verifyPassword(password: string, hash: string) -> bool`** — constant-time verify; a
  malformed hash is `false`, never a fault. Deterministic ⇒ `password-verify.phg` gates it 3-way.
- **Salt is internal.** You don't manage a salt (unlike a raw KDF) — Argon2id embeds it in the PHC
  string, and `verifyPassword` reads it back. Rotate cost params by re-hashing on next login.

## Run it natively — `phg serve`

```
phg serve examples/web/server/serve.phg
```

`serve.phg` has a `(): void` `Web` entry whose body calls `Http.serve(cfg, handler)` with a
`(Request) => Response` closure. `serve` REGISTERS and returns — the runtime drives the accept loop,
frames each HTTP/1.1 request, calls the registered handler, and writes the bytes back. All HTTP
logic — parsing, routing, dispatch — is pure Phorj; the runtime (`src/serve/`) is the thinnest
possible glue. A malformed request 400s and a handler fault degrades to a 500, both inside the
runtime, which is why neither appears in the example.

If nothing registers, `phg serve` refuses at startup rather than serving an empty app — see
`phg explain E-SERVE-NO-HANDLER`.

**Backend:** the registered handler runs on the **bytecode VM by default** — byte-identical to the interpreter
(`Vm::run_entry` ≡ `call_named`, asserted in `tests/serve.rs`) and materially faster per request
(measured ~2.3× lower end-to-end latency than the tree-walker on a representative handler; the pure
handler-compute gain is larger — the fixed socket round-trip is in both numbers). `--tree-walker`
selects the interpreter oracle instead. Each worker compiles its own copy of the program once at startup (the compiled
program holds `Rc` state and can't cross threads); the compile cost is amortised over every request
that worker serves.

**HTTP/1.1 keep-alive (M6 W4 / S4.1):** with a `--timeout` set, a connection is reused for multiple
requests (every response carries `Content-Length`, so it is self-delimiting) until the client sends
`Connection: close`, the per-connection cap (100) is reached, or the idle read-timeout fires. The
timeout is the idle-socket guard: **without `--timeout`, keep-alive is off** and each connection
serves one request then closes (so an idle client can never pin the single-threaded server or a pool
worker). Both the single-threaded path and the `--workers N` pool keep connections alive.

**Graceful shutdown (M6 W4 / S4.2):** `Ctrl-C` (SIGINT) or SIGTERM stops the server accepting new
connections, lets in-flight requests finish, joins the worker pool, and exits `0` — no request is cut
mid-flight. (A second `Ctrl-C` while draining hard-kills.) This needs the `signals` build feature
(on by default; off only for the WASM playground, which has no sockets).

```console
$ phg serve examples/web/server/serve.phg --address 127.0.0.1:8080
phg serve: listening on http://127.0.0.1:8080

$ curl -i http://127.0.0.1:8080/
HTTP/1.1 200 OK
Content-Length: 18
Content-Type: text/plain

Phorj web — home

$ curl -s http://127.0.0.1:8080/greet -H 'Host: phorj.dev'
Hello phorj.dev
$ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/missing
404
```

**Concurrency (`--workers N`):** the server runs a bounded OS-thread pool by default (one request per
worker, each with its own `Rc` value heap — values never cross threads, and the immutable program is
shared via `Arc`); `--workers 1` keeps the single-threaded path. The same *unchanged*
`(Request) => Response` contract holds on every worker.

`server/src/main.phg` exercises the same pipe on canned `b"…"` requests, so the LOGIC stays
byte-identical on both backends and through real PHP. The socket path — and `serve.phg` itself —
are covered by `tests/serve.rs`, deliberately **outside** the byte-identity spine (the determinism
quarantine): `every_example_serve_phg_registers_serves_and_is_transpile_quarantined` loads every
shipped `serve.phg`, drives one real request through the serve loop, and asserts `phg transpile`
refuses it with `E-TRANSPILE-SERVE`.

## Run it on PHP — `php -S`

The same program transpiles to idiomatic PHP. `server.php` is a hand-written front-controller (the
superglobal↔wire adapter is runtime glue, not transpiled — exactly like `src/serve/` on the native
side) that rebuilds the raw request and calls the transpiled `respond(bytes): bytes`. Generate the
application next to it (dropping the demo `main()` bootstrap), then start PHP's built-in server:

```console
$ cd examples/web/server
$ phg transpile src/main.phg | sed '/^ *\\Main\\main();$/d' > web_app.php
$ php -S 127.0.0.1:8080 server.php

$ curl -s http://127.0.0.1:8080/greet -H 'Host: phorj.dev'
Hello phorj.dev
```

`web_app.php` is a generated artifact — regenerate it from `src/main.phg`; it is gitignored.

Note the `sed` matches the bootstrap LINE, not the last line. In a project the bootstrap
(`\Main\main();`) is emitted inside the trailing global `namespace { }` block AHEAD of the runtime
helpers, so it is not the final line and the older `sed '$d'` recipe deleted a helper's closing brace
instead (DEC-455.12).

## Why this shape

- **One value contract, two engines.** `handle(Request) -> Response` is the portable unit. `phg
  serve` and `php -S` are interchangeable hosts for it; the byte path is identical because the
  Phorj backends are byte-identical and PHP round-trips the same logic.
- **Determinism stays intact.** Everything testable (parse, route, serialize) is pure Phorj, gated
  on both backends. The non-deterministic socket is one quarantined module checked over an in-memory
  transport — it never touches `tests/differential.rs`.

## Deferred

Path parameters (`/users/{id}`) and middleware/closure routes are shown by `route-constraints.phg`,
`router-attrs.phg` and `middleware.phg`; the `server/` example deliberately keeps the exact-match
core so the routing story stays readable.

**Historical note, kept because the shape of the change is instructive.** This section used to
describe a future in which a standard `Core.Http` module would let `phg serve` run a bare
`handle(Request) -> Response` directly, making each app's hand-written `respond(bytes) -> bytes` glue
disappear. That future arrived and then moved on. `Core.Http` shipped the types; a synthesized
`respond` bridge did make a bare `handle` servable; and DEC-331 D5 retired BOTH — the bridge, because
a synthesized item that misreads the program is a whole class of bug, and the magic entry NAME,
because `#[Entry(kind:)]` had already abolished name-means-something resolution everywhere else.
What replaced them is explicit: `Http.serve(cfg, handler)`, called from a `(): void` `Web` entry,
with the handler as an ordinary closure value.

# SPEC — Rich Request v1, incl. file uploads (DEC-331 D8, build slice 2 of 3)

> Status: **SPEC RULED (dev, 2026-07-23). SLICE 2 BUILT + 3-leg byte-identity-green (2026-07-24);
> the LAZY mode + its ServeConfig switch ship with slice 3 — see §8 BUILD STATUS.** Elaborates
> D8a (eager+lazy switch), D8b (first-wins + getAll), D8c (files in v1), D8d (the six defaults).

## 1. Surface

```phg
package Main;
import Core.Http;
import Core.Json;
import Core.Output;
import Core.Runtime.Entry;

function handler(Request req): Response {
    string q      = req.query.get("page") ?? "1";          // FIRST value (D8b)
    List<string> tags = req.query.getAll("tag");            // all values
    string ua     = req.headers.get("user-agent") ?? "";    // CASE-INSENSITIVE (D8d)
    Json? payload = req.body.json();                         // Core.Json ADT, no mixed (D8d)
    UploadedFile? avatar = req.files.get("avatar");          // multipart in v1 (D8c)
    if (var f = avatar) {
        Output.printLine("upload {f.name} ({f.size} bytes, {f.contentType})");
        Bytes data = f.bytes();
    }
    string traceId = req.attributes.get("traceId") ?? "";    // string->string bag (D8d)
    return Response.text("ok");
}
```

## 2. The type (locked shape)

`Request` (native-backed class, REPLACES the thin `Core.Http.Request` — examples/web/*
migrate in the same change, D8d):

| member | type | notes |
|---|---|---|
| `method` | `string` | uppercase |
| `path` | `string` | decoded path, no query |
| `query` | `ParamBag` | first-wins `.get`, `.getAll` |
| `headers` | `HeaderBag` | case-insensitive keys |
| `cookies` | `ParamBag` | |
| `form` | `ParamBag` | urlencoded + multipart fields |
| `files` | `FileBag` | `get(k): UploadedFile?`, `getAll(k)` |
| `body` | `Body` | `.bytes(): Bytes`, `.text(): string`, `.json(): Json?` — **built as `RequestBody`** (a bare `Body` would capture user classes; the FS-taxonomy/DEC-202 precedent) |
| `attributes` | `AttrBag` | `string -> string`, middleware scratch **+ route params (PSR-7 convention; Router.handle writes them)** |

Uniform bag API (D8d): `.get(k): string?` / `.get(k, default): string` — **built as
`getOrDefault(k, fallback)`: phorj's `E-OVERLOAD-RETURN` rule forbids return-type-differing
overloads (the `Core.Map.getOrDefault` precedent)** — / `.has(k): bool` /
`.all(): Map<string, List<string>>`. Query/form values are always `string` — the caller
coerces. `UploadedFile { name, size: int, contentType: string, bytes(): Bytes }` with
temp-spill above a threshold and `ServeConfig.maxBodySize` enforcement (D8c).

## 3. Eager vs lazy parsing (D8a, locked)

`Http.ServeConfig.requestParsing = RequestParsing.Eager (default) | RequestParsing.Lazy` —
IDENTICAL handler API; only WHEN parsing happens changes. Soundness: one request = one worker
thread = one heap; the Request never crosses threads, so lazy memoization (the `LazyJson`
precedent: parse-on-first-access, cache in a `OnceCell`-style slot, observationally immutable)
is safe. Eager mode 400s malformed input before the handler runs; lazy surfaces bad input at
the access point (`None` / canonical fault). `RequestParsing` is a stdlib enum.

## 4. Backends (Invariant 17)

- **Interp/VM**: `Request` construction happens native-side in the serve loop (both engines
  call the same native builder — byte-identity by construction).
- **Transpile — Ladder**: `serve` is already native-only (`E-TRANSPILE-SERVE`, D7a); Request
  therefore only exists behind serve → NO new transpile surface. The TYPE still transpiles
  (class shape) for code that merely mentions it; userland construction is via the RULED
  `Request.fake(...)` test builder (§7 P2) — the only non-serve constructor.
- **Lift**: PHP superglobal reads (`$_GET['k']`, `$_POST`, `$_FILES`, `getallheaders()`) lift
  to the corresponding bag calls where the lifter already recognizes them; unrecognized
  patterns keep the existing lift behavior (no regression; incremental mapping table).

## 5. Faults

Malformed multipart / body-too-large: eager → automatic `400` response (never reaches the
handler); lazy → canonical faults at access (`"request body exceeds maxBodySize"`,
`"malformed multipart body"` — exact strings fixed at build). `.json()` on non-JSON = `null`
(mirrors `Json.parse`).

## 6. Examples & tests (Inv 9)

`examples/web/rich_request.phg` (echo server exercising every bag) + README row; unit tests
per bag (first-wins, getAll, case-insensitive headers, default overloads); multipart fixture
tests (small inline + spill threshold + oversize rejection); eager-vs-lazy parity test (same
program, both modes, identical output); migration of existing `examples/web/*`.

## 7. RULED (dev, 2026-07-23)

- **P1 → 256 KiB spill threshold** (in-memory below, temp-file above; cap = `maxBodySize`).
  **PLUS a NEW dev directive → DEC-334**: enumerate and research the FULL php.ini-equivalent
  runtime-knob catalog (every convertible setting, default values, project `phorj.json` vs
  global config) — a dedicated multi-round research/design campaign with the dev; this spill
  threshold and the ServeConfig fields become rows in that catalog.
- **P2 → `Request.fake(...)` ships in v1** (handler unit tests without a socket).
- **P3 → mutable `req.attributes.set(k, v)`** — the ONE documented mutable bag.

## 8. BUILD STATUS (2026-07-24 — autonomous slice, DEC-268 panel-certified plan)

**SLICE 2 BUILT + 3-leg byte-identity-green** (`phg run` ≡ `--tree-walker` ≡ transpiled PHP on
php-8.5.8): the full §2 surface — bags (first-wins + `getAll`, case-insensitive `HeaderBag` vs
case-SENSITIVE ParamBag keys, cookies split on the FIRST `=`), `files` (multipart w/ per-part
256 KiB temp-spill behind deterministic int HANDLES — the path never enters a phorj value, Inv 10),
memoized `body.json()` via the always-registered `Core.Native.Http.jsonParse` (feature `json` off →
flag-naming fault, never a vanished method), eager `Request.parse` (malformed/oversize → null → the
untouched respond bridge's 400), `Request.fake` + `withHeader`/`withCookie`/`withBody` withers that
REBUILD from the ORIGINAL raw target/header lines/body through the same parse (decode is never
round-tripped; CR/LF in header names/values FAULTS — DEC-242 bar). Route params → `attributes`
(PSR-7; `param()` delegates; `Router.handle` now MUTATES its argument — recorded deviation from the
old `withParams` copy). Gates: `examples/web/rich_request.phg` (differential),
`conformance/web/rich-request-bags.phg` (3-leg golden incl. the mutation-observability case), CRLF
`agree_err_php` fault-parity, native unit tests (decode edges, multipart small/over-cap/malformed/
filename-vs-name, stash thresholds, canonical-string pins).

**Build deviations (recorded, dev to review):** `Body`→`RequestBody` + `get(k, default)`→
`getOrDefault` (both forced by language rules — see §2 annotations); `Request.parse` stays public
until slice 3 retires `respond` (the bridge + examples need it); a multipart content-type with an
EMPTY body parses to empty form/files (the fake/wither builder passes through that state; no body ≠
malformed body); multipart part cap 1024 (over-cap = malformed, deliberately); body cap
`DEFAULT_MAX_BODY_SIZE` = 8 MiB == the transport frame cap, so it is INERT under serve in slice 2
(reachable via fake/parse only; slice 3 reconciles frame-vs-body semantics — KNOWN_ISSUES).

**DEFERRED to slice 3 (with `ServeConfig`):** the `RequestParsing.Eager|Lazy` switch (§3) + the §6
eager-vs-lazy parity test; the §5 canonical fault strings ship NOW as single-sourced consts
(`src/native/http.rs`, test-pinned) but become runtime-reachable only in lazy mode. **DEFERRED
(recorded):** superglobal lift mappings (§4) — the lifter recognizes no superglobals today, so
"where already recognized" is vacuously satisfied; a faithful lift needs an ambient→parameter
transform design. **PERF (WIN-OR-FLAG):** `bench/micro/queryparse` lands with the slice and is a
HARD-FLAGGED ~8x loss vs an idiomatic PHP full-request parse — queued in the dev-re-ruled
flip-all-losses campaign (next after this slice), candidate fix = a nativized/JIT-vertical parse.

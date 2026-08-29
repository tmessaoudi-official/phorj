# SPEC — `#[Entry(kind:)]` + `Http.ServeConfig` + serve{} + inbound TLS + retire `respond` (DEC-331 D1/D4/D5/D6/D7, build slice 3 of 3)

> Status: **SPEC RULED (dev, 2026-07-23) — BUILD-READY.** The riskiest slice (D10a: built last).
> Contains the cluster's TWO breaking changes (D5: `respond(bytes)` retired; §6 P1: bare
> `#[Entry]` now requires `kind:`).

## BUILD STATUS (added 2026-07-28, consistency audit — this spec previously had no build-status section)

- **S3.1 ✅ SHIPPED** — `#[Entry(kind:)]` + `E-ENTRY-KIND-REQUIRED` (full `--all-features` gate
  green incl. PHP byte-identity; recorded in SLICE-STATE, never mirrored here until now).
- **S3.2 ⚠ PARTIAL (2026-08-06/07) — two of its three ruled pieces.** Part A: `Http.ServeConfig` +
  `Http.RequestParsing` with D4's field set and default VALUES, in `src/cli/serve_config_prelude.rs`
  (one deviation: D4 §2 writes `tlsMinVersion?="1.2"` and the class declares it NON-optional
  `public string tlsMinVersion = "1.2"` — tracked, not silently absorbed). Part B: **the `#[Config]`
  injection now accepts N typed parameters** — the pass had a hard one-parameter limit and rejected
  multi-parameter config entries outright.
  **⚠ READ THIS BEFORE ASSUMING §1 WORKS: it does NOT.** Part B is necessary but NOT sufficient for
  §1's `function web(Http.ServeConfig cfg, AppSettings app)`, because `entry_role`
  (`src/ast/entry.rs:167-169`) defines a `Web` entry as EXACTLY `(Request): Response` — so a `Web`
  entry can never carry config parameters, and §1 verbatim still fails `E-ENTRY-SIG`. [Verified
  2026-08-07 by running `phg check` on §1's program.] The second gate is unbuilt and belongs with
  S3.3, where `Http.serve(cfg, handler)` gives the `Web` role a shape that can accept config. What
  Part B DOES deliver today is a multi-parameter **`Cli`** entry taking config types.
  **✅ SUPERSEDED 2026-08-22 — that second gate is BUILT (S3.3b).** And the diagnosis above is one
  step short: config parameters never reach `entry_role` at all, because `desugar_config` is a
  PRE-check (`src/cli/pipeline.rs:130`) and erases them — so a config-carrying `Web` entry arrived
  ZERO-ARG and failed `E-ENTRY-SIG` only because `(): void` read as `Cli`. The gate is now
  `ast::entry_shape_matches(f, declared)` (*"is this shape legal FOR the declared role?"*), and a
  config-carrying `Web` entry checks clean on both the CLI and LSP paths. **Still true, and the
  reason this row is not simply deleted: §1 VERBATIM does not check yet** — its body calls
  `Http.serve`, which lands with S3.3a. Injection is in DECLARATION order (observable: `examples/guide/config.phg` prints
  from each provider), and every unresolved parameter gets its own `E-CONFIG-MISSING`. A GENERIC parameter
  type is ACCEPTED — it keys on the bare head, so `Map<string, string>` resolves; a briefly-added filter
  that rejected generics deleted a working surface and was reverted, and the resulting sharp edge (two
  `Map<…>` providers colliding under one key) is **DEC-455.4, pending a ruling**.
  Part A also exposed and fixed a pre-existing P0 (DEC-452:
  a QUALIFIED constructor dropped defaults and named args, panicking the VM on the shipped `Http.Cookie`).
  **✅ BUILT 2026-08-23 (S3.2 Part C, DEC-455.14) for the SERVE surface: the CLI flag wins LOUDLY** —
  the registered `ServeConfig` is the default source for `host`+`port`/`workers`/`timeout`, a passed
  flag whose value differs overrides it with a `W-SERVE-CONFIG-OVERRIDDEN` line. The env and
  `phorj.json` tiers of the chain below remain unbuilt, and for a `Cli` entry they still carry the
  Invariant 1 parity problem DEC-455.2 records (the serve path does not: it has no PHP leg).
  **Originally recorded as NOT built from S3.2: the precedence chain** (CLI flag > env > `#[Config]` >
  `phorj.json` > attribute default) — its env/CLI tiers are RUNTIME reads inside a spine DEC-318 keeps
  pure, so for a `Cli` entry the PHP leg would have to read the same sources or Invariant 1 breaks, and an
  env-reading example is not a deterministic input (Invariant 10). That parity story needs a ruling.
- **D5 — BUILT** (S3.3a–e, 2026-08-22/23): `Http.serve(cfg, handler)` is the only way to register a
  handler; `respond`/`handle` and `SERVE_ENTRY` are deleted.
- **D6 — BUILT** (S3.4, 2026-08-28): `E-NO-ENTRY-FOR-ROLE`, symmetric both directions, with the
  TTY-guarded switch prompt and the non-TTY error. `src/cli/role_mismatch.rs`.
- **D7 — ✅ BUILT (S3.5, 2026-08-29)**: `http-server-tls` exists (`Cargo.toml`), `src/serve/tls.rs`
  is the rule and `src/serve/pem.rs` the decoder. HTTPS auto-enables iff both `cert` and `key` are
  set; `tlsMinVersion` is the floor. **With one deviation from the surface text, deliberate and
  ruled:** D7's "iff BOTH are set" is NOT read as "a lone `cert` means plain HTTP" — that is
  `E-SERVE-TLS-INCOMPLETE`. The literal reading is a silent downgrade to clear text on a port the
  operator believes is encrypted, which `src/cli/serve_config_prelude.rs` had already flagged in
  prose as "a security footgun of exactly the shape DEC-363 was written about".
  Deferred as ruled: HTTP→HTTPS redirect, HSTS, cert hot-reload, mTLS (KNOWN_ISSUES §SERVE-TLS).

  > **This slice closes DEC-331 Slice 3 entirely** (D1/D4/D5/D6/D7 all built). The 2026-07-25
  > verification line the previous version of this block replaced read *"D5/D6/D7 … `respond` is
  > still the live `SERVE_ENTRY`, `E-NO-ENTRY-FOR-ROLE` has 0 src hits"*. All three are now false —
  > which is the intended end state, not drift.

## 1. Surface

```phg
package Main;
import Core.Http;
import Core.Config;
import Core.Runtime.Entry;

class AppSettings {
    string greeting;
    function construct(string greeting) { this.greeting = greeting; }
}

#[Config]
function serveConfig(): Http.ServeConfig {
    return new Http.ServeConfig(host: "0.0.0.0", port: 8443,
                                cert: "certs/site.pem", key: "certs/site.key");
}

#[Config]
function appSettings(): AppSettings { return new AppSettings("hello"); }

#[Entry(kind: EntryKind.Web)]
function web(Http.ServeConfig cfg, AppSettings app): void {
    Http.serve(cfg, function(Request req): Response {
        return Response.text("{app.greeting} {req.path}");
    });
}

#[Entry(kind: EntryKind.Cli)]
function tool(): void { /* the same program can also ship a CLI role */ }
```

> **DEC-337 (2026-07-25):** the kind is an INJECTED enum variant `Core.Runtime.EntryKind`, reached
> QUALIFIED and import-gated — `import Core.Runtime.EntryKind;` then `kind: EntryKind.Cli`. A bare
> `kind: Cli` is `E-INJECTED-VARIANT-BARE` (nothing in the wind, like `Option.Some`); an unimported
> `EntryKind.Cli` is `E-UNIMPORTED`; the fully-qualified `Core.Runtime.EntryKind.Cli` is self-gating.
> Reserved kinds are real variants (`E-ENTRY-KIND-RESERVED`). Compile-time-only marker (Inv 5).

## 2. Rulings elaborated (all locked)

- **D1 — roles & config**: `#[Entry(kind: Type)]`, active `Cli`/`Web`, reserved (recognized,
  unbuilt: parse + clear "reserved kind" error) `Desktop`/`Mobile`/`Worker`/`Embedded`. Config
  arrives as TYPED ENTRY PARAMETERS (DEC-318 injection) — the parameter type IS the config
  declaration; `#[Entry]`/`#[Config]` work on class static methods too; config values are class
  instances. Precedence (highest wins): CLI flag > env var > `#[Config]` provider >
  `phorj.json` static block > attribute inline default.
- **D4 — `Http.ServeConfig`** (stdlib class, the runtime's contract): `host="127.0.0.1"`,
  `port=8080`, `workers=<cores>`, `timeout=0` (secs, 0=none), `cert?`, `key?`, `serverName?`,
  `maxBodySize=8_388_608`, `tlsMinVersion?="1.2"`, plus `requestParsing=Eager` (Rich-Request
  spec D8a). App settings are a SEPARATE injected parameter — never mixed into ServeConfig.
- **D5 — one handler model (BREAKING)**: typed `(Request): Response` is THE web handler — as the
  CLOSURE passed to `Http.serve(cfg, handler)`, not as the entry itself; the `Web` entry is a `(): void`
  closure FACTORY. **✅ BUILT: `Http.serve` in S3.3a; `respond(bytes): bytes` RETIRED in S3.3c**
  (2026-08-22), together with the `handle(Request): Response` entry the `Core.Http` bridge used to wrap
  — Q1, developer-ruled. An unservable program is refused at startup with `E-SERVE-NO-HANDLER`.
  ⚠ `examples/web/*`, `playground/web/examples.js` and site-mode `index.phg` did NOT migrate "in this
  same slice": they ride with S3.3d, deliberately, so that removing the bridge does not fail
  `phg check` on the shipped corpus in the same commit. They still check/run/transpile; `phg serve`
  refuses them until then. Immutable `Response` makes "headers already sent" structurally
  impossible. Static-file site mode (public/, MIME/ETag/traversal guards, DEC-282) unchanged.
- **D6 — role mismatch UX**: `phg run` on a Web-only program (or `phg serve` on Cli-only) →
  `E-NO-ENTRY-FOR-ROLE` naming the mismatch + the right command, THEN a TTY-guarded
  interactive "Did you mean `phg serve <file>`? [y/N]" (runs it on `y`); non-TTY (CI/pipe):
  error + suggestion, exit non-zero, never block on stdin.
- **D7 — inbound TLS**: native-only (`E-TRANSPILE-SERVE` inherited, Ladder tier 2 — loud
  refusal, no silent PHP-built-in-server downgrade). HTTPS auto-enables iff BOTH `cert` and
  `key` are set (no `--tls` flag). Floor via `tlsMinVersion` (default 1.2). Deferred to a later
  slice + KNOWN_ISSUES: HTTP→HTTPS redirect, HSTS, cert hot-reload, mTLS. v1 = terminating TLS
  only, via **rustls** (RULED §6 P2: feature-gated `http-server-tls`).

## 3. Checker / CLI rules

1. Multiple entries allowed iff kinds differ; two entries of the SAME kind =
   `E-DUPLICATE-ENTRY-KIND`. `kind:` values type-check against the reserved-name set.
2. Entry params must each resolve to exactly one `#[Config]` provider (or a
   precedence-chain source) by TYPE — ambiguity/missing = compile error naming the type.
3. `phg run` selects the `Cli` entry; `phg serve` the `Web` entry (D6 mismatch UX otherwise).
4. **RULED (§6 P1): bare `#[Entry]` = `E-ENTRY-KIND-REQUIRED`** — DEC-191 inference retired.

## 4. Backends (Invariant 17)

Roles/config are host-side (CLI + serve loop): interp ≡ VM by construction. Transpile: `Cli`
entries transpile as today; a call to `Http.serve` hits `E-TRANSPILE-SERVE` (BUILT in S3.3a,
`src/transpile/call.rs`). **Corrected 2026-08-22:** this line previously said `Web` *entries* hit it
and called that "already the rule" — both halves were false. The refusal is keyed on the CALL, and
was verified against the corpus: `examples/web/core-http.phg` and `examples/web/handler.phg` are
`Web` entries that transpile clean today, so an entry-kind key would have broken the five shipped
`examples/web/*` and Invariant 1's corpus enforcement with them (**superseded 2026-08-23, DEC-455.12:**
neither file is a `Web` entry any more — `core-http` became a project whose `Web` entry lives in a
sibling `serve.phg`, and `handler.phg` dropped its `Web` attribute entirely. The reasoning stands and
its conclusion is now structural: the CALL key is what lets `src/main.phg` keep its PHP leg while the
registering file does not);
`#[Config]` providers transpile as plain functions (DEC-318 machinery shipped). Lift:
unchanged (PHP has no entry-role concept; lifted code keeps the inferred entry).

## 5. Examples & tests (Inv 9)

`examples/web/serve_config.phg` (the §1 shape, HTTP), `examples/web/serve_tls.phg`
(cert/key walkthrough README — TLS needs local certs, so README-driven per the faults-cant-run
rule), migrated `examples/web/*` (D5); tests: precedence-chain resolution, duplicate-kind
error, role-mismatch UX (TTY + non-TTY legs), TLS handshake smoke (self-signed fixture),
`maxBodySize` enforcement, reserved-kind error.

## 6. RULED (dev, 2026-07-23)

- **P1 → HARD-REQUIRE `kind:` NOW** (dev chose the clean end-state over the deprecation
  path): bare `#[Entry]` = compile error `E-ENTRY-KIND-REQUIRED`; DEC-191 signature inference
  is RETIRED. This is the slice's SECOND breaking change (alongside D5's respond retirement)
  — all shipped examples migrate in the same slice.
- **P2 → feature-gated `http-server-tls`** (off by default; rustls added as a vetted
  exception row in UNIFIED-SPEC §external-deps same-change; the all-features gate covers it).
- **P3 → symmetric auto-correct** (both `run`→`serve` and `serve`→`run` directions).

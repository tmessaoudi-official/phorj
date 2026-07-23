# SPEC — `#[Entry(kind:)]` + `Http.ServeConfig` + serve{} + inbound TLS + retire `respond` (DEC-331 D1/D4/D5/D6/D7, build slice 3 of 3)

> Status: **SPEC FROZEN, awaiting dev ruling (D10b).** The riskiest slice (D10a: built last).
> Contains the cluster's ONE breaking change (D5: `respond(bytes)` retired).

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

#[Entry(kind: Web)]
function web(Http.ServeConfig cfg, AppSettings app): void {
    Http.serve(cfg, function(Request req): Response {
        return Response.text("{app.greeting} {req.path}");
    });
}

#[Entry(kind: Cli)]
function tool(): void { /* the same program can also ship a CLI role */ }
```

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
- **D5 — one handler model (BREAKING)**: typed `(Request): Response` is THE web handler;
  `respond(bytes): bytes` is RETIRED — its docs, `examples/web/*`, and site-mode `index.phg`
  migrate in this same slice. Immutable `Response` makes "headers already sent" structurally
  impossible. Static-file site mode (public/, MIME/ETag/traversal guards, DEC-282) unchanged.
- **D6 — role mismatch UX**: `phg run` on a Web-only program (or `phg serve` on Cli-only) →
  `E-NO-ENTRY-FOR-ROLE` naming the mismatch + the right command, THEN a TTY-guarded
  interactive "Did you mean `phg serve <file>`? [y/N]" (runs it on `y`); non-TTY (CI/pipe):
  error + suggestion, exit non-zero, never block on stdin.
- **D7 — inbound TLS**: native-only (`E-TRANSPILE-SERVE` inherited, Ladder tier 2 — loud
  refusal, no silent PHP-built-in-server downgrade). HTTPS auto-enables iff BOTH `cert` and
  `key` are set (no `--tls` flag). Floor via `tlsMinVersion` (default 1.2). Deferred to a later
  slice + KNOWN_ISSUES: HTTP→HTTPS redirect, HSTS, cert hot-reload, mTLS. v1 = terminating TLS
  only, via **rustls** (feature-gated `http-server-tls`? see P2 — external-dependency policy).

## 3. Checker / CLI rules

1. Multiple entries allowed iff kinds differ; two entries of the SAME kind =
   `E-DUPLICATE-ENTRY-KIND`. `kind:` values type-check against the reserved-name set.
2. Entry params must each resolve to exactly one `#[Config]` provider (or a
   precedence-chain source) by TYPE — ambiguity/missing = compile error naming the type.
3. `phg run` selects the `Cli` entry; `phg serve` the `Web` entry (D6 mismatch UX otherwise).
4. **OPEN (D1 flagged): signature inference (DEC-191) when `kind:` is omitted** — see P1.

## 4. Backends (Invariant 17)

Roles/config are host-side (CLI + serve loop): interp ≡ VM by construction. Transpile: `Cli`
entries transpile as today; `Web` entries hit `E-TRANSPILE-SERVE` (already the rule);
`#[Config]` providers transpile as plain functions (DEC-318 machinery shipped). Lift:
unchanged (PHP has no entry-role concept; lifted code keeps the inferred entry).

## 5. Examples & tests (Inv 9)

`examples/web/serve_config.phg` (the §1 shape, HTTP), `examples/web/serve_tls.phg`
(cert/key walkthrough README — TLS needs local certs, so README-driven per the faults-cant-run
rule), migrated `examples/web/*` (D5); tests: precedence-chain resolution, duplicate-kind
error, role-mismatch UX (TTY + non-TTY legs), TLS handshake smoke (self-signed fixture),
`maxBodySize` enforcement, reserved-kind error.

## 6. PENDING for dev

- **P1** (flagged in D1): when `kind:` is omitted — keep DEC-191 signature inference as a
  fallback (recommended: yes for one release with a deprecation note steering to explicit
  `kind:`) or hard-require `kind:` now?
- **P2**: rustls lands as a NEW vetted exception to the std-only policy (UNIFIED-SPEC
  §external-deps currently lists argon2/regex/ctrlc/corosensei) — feature-gated
  `http-server-tls` (recommended, mirrors `http-client`) or bundled into the default `jit`-like
  feature set?
- **P3**: `E-NO-ENTRY-FOR-ROLE` auto-correct — also offered for `phg serve`→`phg run`
  direction (recommended: yes, symmetric)?

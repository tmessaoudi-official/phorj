//! The `Core.Http` serve-configuration fragment — DEC-331 D4 (`Http.ServeConfig`) plus the
//! Rich-Request D8a `RequestParsing` switch, built in slice S3.2.
//!
//! Split into its own file per Invariant 13: `http_prelude.rs` is at 297 lines against the 300 soft
//! cap, so this fragment starts life beside it rather than pushing it over. It is injected as a third
//! `srcs` entry of the `Core.Http` virtual module, so `import Core.Http;` reaches
//! `Http.ServeConfig` / `Http.RequestParsing` exactly as the spec's §1 surface writes them.
//!
//! WHY THIS IS ORDINARY PHORJ SOURCE, not a native: `ServeConfig` is a value, and D1 makes it arrive
//! as a TYPED ENTRY PARAMETER through the DEC-318 `#[Config]` injection — which is a pure pre-check
//! desugar (Invariant 5). Keeping the class in phorj means all three legs see the same class and the
//! same defaults by construction, so the config surface stays inside the byte-identity spine for
//! `Cli` entries. The CLASS itself is legal everywhere — a `Cli` entry may take a `ServeConfig` and
//! print it. The Invariant-14 tier-2 refusal `E-TRANSPILE-SERVE` is **BUILT** as of S3.3a
//! (`src/transpile/call.rs`), and it is keyed on a `Http.serve` CALL — NOT on the `Web` entry kind,
//! which two earlier drafts of this comment and the specs both assumed. That distinction is load
//! bearing here: a `Web` entry constructing a `ServeConfig` with `cert:` set still type-checks clean
//! AND transpiles with exit 0, exactly as verified on 2026-08-06, because it never calls
//! `Http.serve`. Keying the refusal on the entry kind would have broken that shape and the five
//! shipped `examples/web/*` with it.
//! [Verified 2026-08-06 on the exact shape below — a promoted constructor with defaults, a `string?`
//! defaulting to `null`, an `8_388_608` underscore literal and a zero-payload enum default: `run` ≡
//! `run --tree-walker` ≡ `run --no-jit` ≡ transpiled php-8.5.8, all four byte-identical. The PHP leg
//! emits the promoted ctor as `__construct(public string $host, …)` and normalizes named arguments to
//! positional with defaults filled (DEC-297's `normalize_named_args`, erased before backends).]
//!
//! NO VALIDATION HERE, deliberately. D4 specifies the FIELD SET and the DEFAULTS; it does not specify
//! constructor guards, and the values are not consumed until `Http.serve` lands in S3.3. Two checks
//! belong there rather than here, because that is where the decision is actually made:
//!   * `cert`/`key` one-without-the-other — **RESOLVED in S3.5 the way this note predicted**, in
//!     `src/serve/tls.rs`: a lone `cert` is `E-SERVE-TLS-INCOMPLETE`, not a quiet fall back to plain
//!     HTTP. The spec's literal "iff BOTH are set" would have made it the footgun DEC-363 was written
//!     about, so the refusal is the ruling. `tlsMinVersion` ∈ {"1.2","1.3"} is checked there too
//!     (`E-SERVE-TLS-MIN-VERSION`), and only when TLS is actually requested — the field has a
//!     non-null class default, so validating it unconditionally would let a typo in an unused field
//!     refuse a plain-HTTP server.
//!   * `port` / `maxBodySize` / `timeout` ranges — **RESOLVED by DEC-475** in `serve::settings`'s
//!     `validate`, which runs before the socket binds (`E-SERVE-CONFIG-RANGE`). It could not be
//!     written until the field set became nullable: while the class default was itself a value,
//!     refusing a nonsense number meant refusing the default too.
//!
//! A guard added here would also have to raise through a native (the `HeaderSafety.reject` →
//! `NativeHttp.headerFault` route), which is a larger surface than this fragment needs.

/// `Http.ServeConfig` + `Http.RequestParsing` — the web runtime's configuration contract (D4).
///
/// `workers = 0` means AUTO (one per core), resolved by the serve loop in S3.3. The default is a
/// literal `0` rather than the core count because D4 writes it `workers=<cores>`, and baking a
/// machine-dependent number into a class default would make the value differ between the two native
/// legs and the PHP leg — Invariant 10 (determinism) and Invariant 1 both. The sentinel keeps the
/// class a pure deterministic value and moves the machine query to the one place that needs it, which
/// is the same shape D4 already uses for `timeout = 0` meaning "no timeout".
pub(crate) const SERVE_CONFIG_PRELUDE: &str = r#"
// Rich-Request D8a — when the request body is parsed. `Eager` 400s malformed input before the
// handler runs; `Lazy` defers and surfaces bad input at first access. Eager is the default because
// failing at the edge beats failing mid-handler.
enum RequestParsing { Eager, Lazy }

// DEC-331 D4 — the canonical serve configuration. Immutable value; a promoted constructor with
// defaults, so every field is optional at the call site and named arguments select what you set:
//
//   new Http.ServeConfig(host: "0.0.0.0", port: 8443, cert: "certs/site.pem", key: "certs/site.key")
//
// App settings are a SEPARATE injected entry parameter and are never mixed in here (D4).
//
// DEC-475: every field is NULLABLE and defaults to `null`, which means UNSET. A field's effective
// value is applied where it is consumed, so `null` and "written by hand at the same value the
// runtime would have chosen" are different things — which is what makes `timeout: 0` ("no timeout")
// expressible at all, and what lets `phg serve` say truthfully which fields a CLI flag overrode.
// The effective defaults are named beside each field and asserted against this source by
// `serve::settings`' own tests, so the two cannot drift.
class ServeConfig {
  constructor(
    // Effective default "127.0.0.1".
    public string? host = null,
    // Effective default 8080. Must be 1..=65535 when set.
    public int? port = null,
    // Effective default: one worker per core. 0 = AUTO (that same per-core count) written
    // explicitly; negative is refused.
    public int? workers = null,
    // Seconds; effective default 30. `0` means NO TIMEOUT and is now expressible; negative is
    // refused.
    public int? timeout = null,
    // HTTPS auto-enables iff BOTH cert and key are set (D7) — no separate `--tls` flag.
    public string? cert = null,
    public string? key = null,
    public string? serverName = null,
    // Effective default 8 MiB, single-sourced with the wire-parsing limit in `Core.Native.Http`
    // (Invariant 4). Must be >= 1 when set.
    public int? maxBodySize = null,
    // Effective default "1.2".
    public string? tlsMinVersion = null,
    // Effective default `Eager`.
    public RequestParsing? requestParsing = null
  ) {}
}
"#;

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
//! `Cli` entries. (`Web` entries hit `E-TRANSPILE-SERVE` under the Invariant-14 ladder, but the
//! CLASS itself is legal everywhere — a `Cli` entry may take a `ServeConfig` and print it.)
//! [Verified 2026-08-06 on the exact shape below — a promoted constructor with defaults, a `string?`
//! defaulting to `null`, an `8_388_608` underscore literal and a zero-payload enum default: `run` ≡
//! `run --tree-walker` ≡ `run --no-jit` ≡ transpiled php-8.5.8, all four byte-identical. The PHP leg
//! emits the promoted ctor as `__construct(public string $host, …)` and normalizes named arguments to
//! positional with defaults filled (DEC-297's `normalize_named_args`, erased before backends).]
//!
//! NO VALIDATION HERE, deliberately. D4 specifies the FIELD SET and the DEFAULTS; it does not specify
//! constructor guards, and the values are not consumed until `Http.serve` lands in S3.3. Two checks
//! belong there rather than here, because that is where the decision is actually made:
//!   * `cert`/`key` one-without-the-other. D7 says HTTPS auto-enables "iff BOTH are set", so a lone
//!     `cert` would silently serve plain HTTP — a security footgun of exactly the shape DEC-363 was
//!     written about. Rejecting it is the reading this repo's posture implies, but the spec is
//!     genuinely ambiguous, so it is surfaced for a ruling with S3.3 rather than decided here.
//!   * `port` / `maxBodySize` / `timeout` ranges, and `tlsMinVersion` ∈ {"1.2","1.3"}.
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
class ServeConfig {
  constructor(
    public string host = "127.0.0.1",
    public int port = 8080,
    // 0 = AUTO (one worker per core), resolved by the serve loop — see this fragment's Rust doc.
    public int workers = 0,
    // Seconds; 0 = no timeout.
    public int timeout = 0,
    // HTTPS auto-enables iff BOTH cert and key are set (D7) — no separate `--tls` flag.
    public string? cert = null,
    public string? key = null,
    public string? serverName = null,
    // 8 MiB. Single-sourced with the wire-parsing limit in `Core.Native.Http` (Invariant 4).
    public int maxBodySize = 8_388_608,
    public string tlsMinVersion = "1.2",
    public RequestParsing requestParsing = new RequestParsing.Eager()
  ) {}
}
"#;

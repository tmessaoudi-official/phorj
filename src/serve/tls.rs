//! DEC-331 D7 (S3.5) — inbound TLS for `phg serve`. Terminating TLS only, via `rustls`.
//!
//! **Every mistake in this file has the same shape, and it is why the code is arranged the way it
//! is.** A TLS misconfiguration that is handled sloppily does not produce an outage: it produces a
//! server that binds the requested port, answers every request, and speaks *plaintext*. Nothing in
//! the response says so; the operator's `curl` succeeds; the mistake is discovered by someone else.
//! So the module is split into a PURE rule that decides what was asked for, and a fallible build that
//! either produces a TLS server or refuses — with no path between them that yields a plain socket
//! while the config asked for an encrypted one.
//!
//! Three consequences worth stating, because each looks like an over-reaction on its own:
//!
//!   * **A lone `cert` is an ERROR, not plain HTTP.** D7's surface says HTTPS auto-enables "iff BOTH
//!     are set", which read literally makes a half-configured server fall back to clear text. That
//!     reading is rejected here, and `src/cli/serve_config_prelude.rs` already anticipated it in
//!     prose ("a lone `cert` would silently serve plain HTTP — a security footgun"). Someone who
//!     wrote `cert:` believes the port is encrypted.
//!   * **On a build without the `http-server-tls` feature, [`TlsServer`] is an UNINHABITED enum.**
//!     Not a struct that is never constructed — a type with no values at all. `Option<TlsServer>` is
//!     then provably `None`, so it is the compiler, not a comment or a test, that guarantees no such
//!     build can serve plaintext on a port the program asked to encrypt. [`Conn::accept`] discharges
//!     the impossible branch with `match *never {}`.
//!   * **`tlsMinVersion` is validated only when TLS is actually requested.** The field carries a
//!     non-null class default, so EVERY `ServeConfig` has one; validating it unconditionally would
//!     let a typo in an unused field refuse a plain-HTTP server that never wanted TLS.
//!
//! **Not here, deliberately** (spec §1, deferred to a later slice + KNOWN_ISSUES): HTTP→HTTPS
//! redirect, HSTS, certificate hot-reload, and mTLS. `cert`/`key` paths resolve against the process
//! working directory, not a site-mode app root.
#[cfg(feature = "http-server-tls")]
use super::pem::pem_blocks;
use super::ServeCfg;
use std::path::PathBuf;

/// Exactly one of `cert`/`key` was set. The config cannot be honoured and must not degrade.
pub const E_SERVE_TLS_INCOMPLETE: &str = "E-SERVE-TLS-INCOMPLETE";
/// `tlsMinVersion` was not one of the two ruled values.
pub const E_SERVE_TLS_MIN_VERSION: &str = "E-SERVE-TLS-MIN-VERSION";
/// TLS was requested but this binary was built without the `http-server-tls` feature.
pub const E_SERVE_TLS_DISABLED: &str = "E-SERVE-TLS-DISABLED";
/// The certificate or key could not be read, decoded, or accepted by rustls.
pub const E_SERVE_TLS_CERT: &str = "E-SERVE-TLS-CERT";

/// The ruled floor. Two values, because D7 rules two — TLS 1.0/1.1 are deprecated and rustls does not
/// implement them, so accepting `"1.1"` could only ever mean silently serving something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinVersion {
    Tls12,
    Tls13,
}

impl MinVersion {
    /// Parse D4's `tlsMinVersion` string. Deliberately strict: no `"TLSv1.2"`, no whitespace
    /// tolerance, no case folding. A floor is a security control, and guessing at what an operator
    /// meant is how a control ends up lower than they think.
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "1.2" => Ok(Self::Tls12),
            "1.3" => Ok(Self::Tls13),
            other => Err(format!(
                "{E_SERVE_TLS_MIN_VERSION}: ServeConfig.tlsMinVersion is {other:?}; \
                 the supported floors are \"1.2\" and \"1.3\""
            )),
        }
    }
}

/// A validated request for TLS: both halves present, floor understood. Says nothing about whether
/// the files exist or this binary can honour it — that is [`build`]'s job, and keeping the two apart
/// is what makes the ordering rule (config errors before build errors) structural.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsRequest {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub min_version: MinVersion,
}

/// The pure rule: what did this `ServeConfig` ask for?
///
/// `Ok(None)` = plain HTTP, the default and overwhelmingly common answer. `Ok(Some(_))` = a coherent
/// TLS request. `Err` = a config that cannot be honoured as written, which is never downgraded.
///
/// # Errors
/// [`E_SERVE_TLS_INCOMPLETE`] when exactly one of `cert`/`key` is set; [`E_SERVE_TLS_MIN_VERSION`]
/// when TLS is requested with an unruled `tlsMinVersion`.
pub fn requested(cfg: &ServeCfg) -> Result<Option<TlsRequest>, String> {
    match (cfg.cert.as_deref(), cfg.key.as_deref()) {
        (None, None) => Ok(None),
        (Some(cert), Some(key)) => Ok(Some(TlsRequest {
            cert: PathBuf::from(cert),
            key: PathBuf::from(key),
            min_version: MinVersion::parse(
                cfg.tls_min_version
                    .as_deref()
                    .unwrap_or(super::settings::DEFAULT_TLS_MIN_VERSION),
            )?,
        })),
        (Some(_), None) => Err(incomplete("cert", "key")),
        (None, Some(_)) => Err(incomplete("key", "cert")),
    }
}

/// Both halves of the message name the MISSING field, not the present one: the reader already knows
/// what they typed. The second sentence exists because the tempting "fix" is to delete the field
/// that IS set, which silently returns the server to plain HTTP — the outcome this refusal prevents.
fn incomplete(present: &str, missing: &str) -> String {
    format!(
        "{E_SERVE_TLS_INCOMPLETE}: ServeConfig sets {present} but not {missing}, so HTTPS cannot be \
         enabled. Set both to serve HTTPS; removing {present} serves plain HTTP on this port."
    )
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The built server — present only when the feature is.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// A ready TLS server configuration.
///
/// Without the `http-server-tls` feature this is an **uninhabited** enum: there is no value of this
/// type, so no refactor can produce one, and `Option<TlsServer>` is `None` by construction. That is
/// the enforcement mechanism for "a build that cannot do TLS never serves a TLS-configured program
/// in the clear" — a type-level fact rather than a runtime check that could be bypassed.
#[cfg(feature = "http-server-tls")]
#[derive(Clone)]
pub struct TlsServer(std::sync::Arc<rustls::ServerConfig>);

/// Hand-written rather than derived, and it prints NOTHING about the configuration. A
/// `ServerConfig` holds the resolved private key; a derived `Debug` would put key material into any
/// log line, panic message or test failure that formatted a `TlsServer`.
#[cfg(feature = "http-server-tls")]
impl std::fmt::Debug for TlsServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TlsServer(<rustls config, contents withheld>)")
    }
}

#[cfg(feature = "http-server-tls")]
impl TlsServer {
    /// The shared rustls configuration, cloned per connection (`Arc`, so this is a refcount bump).
    #[must_use]
    pub fn config(&self) -> std::sync::Arc<rustls::ServerConfig> {
        std::sync::Arc::clone(&self.0)
    }
}

#[cfg(not(feature = "http-server-tls"))]
#[derive(Debug)]
pub enum TlsServer {}

/// Turn a validated request into a server configuration.
///
/// # Errors
/// [`E_SERVE_TLS_DISABLED`] on a build without the feature; [`E_SERVE_TLS_CERT`] when the cert or key
/// cannot be read, decoded, or accepted.
#[cfg(not(feature = "http-server-tls"))]
pub fn build(req: &TlsRequest) -> Result<TlsServer, String> {
    // The message names the cert so the reader can tell WHICH server refused, and names the feature
    // so the fix is mechanical. It deliberately does not offer "or remove cert/key to serve HTTP":
    // the operator asked for encryption, and a refusal that suggests turning it off is an invitation.
    Err(format!(
        "{E_SERVE_TLS_DISABLED}: ServeConfig requests HTTPS (cert {}) but this phg was built without \
         inbound TLS. Rebuild with `--features http-server-tls`.",
        req.cert.display()
    ))
}

#[cfg(feature = "http-server-tls")]
pub fn build(req: &TlsRequest) -> Result<TlsServer, String> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    let cert_pem = read(&req.cert, "certificate")?;
    let key_pem = read(&req.key, "private key")?;

    let certs: Vec<CertificateDer<'static>> = pem_blocks(&cert_pem)
        .into_iter()
        .filter(|(label, _)| label == "CERTIFICATE")
        .map(|(_, der)| CertificateDer::from(der))
        .collect();
    // An EMPTY chain is the failure this check exists for: a decoder that silently yields nothing on
    // unparseable input would hand rustls a server with no identity, which fails at HANDSHAKE time on
    // every request instead of at startup — a server that binds successfully and can never answer.
    if certs.is_empty() {
        return Err(format!(
            "{E_SERVE_TLS_CERT}: {} contains no CERTIFICATE block",
            req.cert.display()
        ));
    }

    let key = pem_blocks(&key_pem)
        .into_iter()
        .find_map(|(label, der)| match label.as_str() {
            "PRIVATE KEY" => Some(PrivateKeyDer::Pkcs8(der.into())),
            "RSA PRIVATE KEY" => Some(PrivateKeyDer::Pkcs1(der.into())),
            "EC PRIVATE KEY" => Some(PrivateKeyDer::Sec1(der.into())),
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "{E_SERVE_TLS_CERT}: {} contains no PRIVATE KEY, RSA PRIVATE KEY or EC PRIVATE KEY \
                 block",
                req.key.display()
            )
        })?;

    let versions: &[&'static rustls::SupportedProtocolVersion] = match req.min_version {
        MinVersion::Tls12 => &[&rustls::version::TLS12, &rustls::version::TLS13],
        MinVersion::Tls13 => &[&rustls::version::TLS13],
    };
    // `builder_with_provider` rather than `builder_with_protocol_versions`: the latter `.unwrap()`s
    // internally on an incompatible provider/version pair. It cannot fire with ring + tls12, but a
    // handled `Result` costs two lines and this repo does not ship a panic path it can avoid.
    let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(versions)
    .map_err(|e| format!("{E_SERVE_TLS_CERT}: TLS version floor not supported: {e}"))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|e| {
        format!(
            "{E_SERVE_TLS_CERT}: {} and {} were not accepted as a certificate/key pair: {e}",
            req.cert.display(),
            req.key.display()
        )
    })?;
    Ok(TlsServer(std::sync::Arc::new(config)))
}

#[cfg(feature = "http-server-tls")]
fn read(path: &std::path::Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| {
        format!(
            "{E_SERVE_TLS_CERT}: cannot read the {what} {}: {e}",
            path.display()
        )
    })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The connection wrapper the transport reads and writes through.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// One accepted connection, plain or TLS-wrapped.
///
/// The variant is decided once, at accept time, and every later read/write goes through this type —
/// so there is exactly one place where a plain socket can be produced, and it is the place that has
/// already been told whether TLS was requested.
pub(super) enum Conn {
    Plain(std::net::TcpStream),
    #[cfg(feature = "http-server-tls")]
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>>),
}

impl Conn {
    /// Wrap an accepted stream. **The caller must already have set blocking mode and the read/write
    /// timeouts on the raw `TcpStream`** — both accept paths do. That order is not incidental: rustls
    /// fails immediately on a non-blocking socket, and because the handshake runs through these same
    /// timeouts, the read timeout is also what bounds a client that opens a connection and then
    /// stalls mid-handshake.
    ///
    /// The handshake itself is **not** performed here. `StreamOwned` drives it on first read, inside
    /// the worker — so a slow or hostile client cannot serialize the accept loop, and a failed
    /// handshake surfaces as an ordinary read error on a path that already drops the connection.
    pub(super) fn accept(
        stream: std::net::TcpStream,
        tls: Option<&TlsServer>,
    ) -> std::io::Result<Self> {
        match tls {
            None => Ok(Self::Plain(stream)),
            #[cfg(feature = "http-server-tls")]
            Some(server) => {
                let conn = rustls::ServerConnection::new(server.config())
                    .map_err(|e| std::io::Error::other(format!("tls: {e}")))?;
                Ok(Self::Tls(Box::new(rustls::StreamOwned::new(conn, stream))))
            }
            // No feature ⇒ `TlsServer` is uninhabited ⇒ this branch has no inhabitants to handle.
            // The empty match is the compiler agreeing, and it is what makes "a non-TLS build cannot
            // serve a TLS-configured program in the clear" a fact about the types rather than a
            // promise in a comment.
            #[cfg(not(feature = "http-server-tls"))]
            Some(never) => match *never {},
        }
    }
}

impl std::io::Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            #[cfg(feature = "http-server-tls")]
            Self::Tls(s) => s.read(buf),
        }
    }
}

impl std::io::Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(buf),
            #[cfg(feature = "http-server-tls")]
            Self::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            #[cfg(feature = "http-server-tls")]
            Self::Tls(s) => s.flush(),
        }
    }
}

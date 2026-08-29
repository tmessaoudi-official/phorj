//! DEC-331 D7 (S3.5) — inbound TLS for `phg serve`.
//!
//! **What this file is really guarding.** Every failure mode in TLS configuration degrades the same
//! way if it is handled sloppily: the server comes up on the port the operator asked for, answers
//! requests, and speaks *plaintext*. Nothing in the response says otherwise. So the tests below are
//! weighted toward the refusals, not the happy path — a green handshake proves TLS can work, while
//! the refusals prove that a half-configured server cannot quietly not-work.
//!
//! Two tiers:
//!   * **Always** — the pure rule and the three refusals. These run on the DEFAULT feature set, which
//!     is the point: the `E-SERVE-TLS-DISABLED` path only exists in a build without the feature.
//!   * **`--features http-server-tls`** — a real handshake against a real listener, and the negative
//!     floor test. Both need a certificate, so they generate a throwaway self-signed one with the
//!     `openssl` CLI and skip LOUDLY when it is absent (the repo's DB/mail convention). No private
//!     key is committed to this repo.
use phorj::serve::tls;
use phorj::serve::ServeCfg;

/// A `ServeCfg` at D4's declared defaults — the "nothing configured" baseline every case perturbs.
fn cfg() -> ServeCfg {
    phorj::serve::class_defaults()
}

#[test]
fn no_cert_and_no_key_is_plain_http() {
    // The overwhelmingly common case, and the one that must stay free: a config that says nothing
    // about TLS asks for none. `None` here is what keeps `phg serve` a loopback dev server by default.
    assert!(
        tls::requested(&cfg())
            .expect("a config with no TLS fields is valid")
            .is_none(),
        "a default ServeConfig must not request TLS"
    );
}

#[test]
fn both_cert_and_key_requests_tls() {
    let mut c = cfg();
    c.cert = Some("certs/site.pem".to_string());
    c.key = Some("certs/site.key".to_string());
    let req = tls::requested(&c)
        .expect("both halves set is valid")
        .expect("TLS is requested");
    assert_eq!(req.cert.to_string_lossy(), "certs/site.pem");
    assert_eq!(req.key.to_string_lossy(), "certs/site.key");
    // D4's default floor, carried through rather than re-defaulted at the use site.
    assert_eq!(req.min_version, tls::MinVersion::Tls12);
}

#[test]
fn a_lone_cert_is_refused_rather_than_serving_plaintext() {
    // D7 says HTTPS auto-enables "iff BOTH are set". Read literally that makes a lone `cert` mean
    // plain HTTP — which is the exact footgun `src/cli/serve_config_prelude.rs` already names in
    // prose ("a lone `cert` would silently serve plain HTTP"). An operator who wrote `cert:` believes
    // the port is encrypted. Refuse instead.
    let mut c = cfg();
    c.cert = Some("certs/site.pem".to_string());
    let err = tls::requested(&c).expect_err("a half-configured TLS setup must not be accepted");
    assert!(err.starts_with(tls::E_SERVE_TLS_INCOMPLETE), "{err}");
    assert!(
        err.contains("key"),
        "it must name the half that is MISSING: {err}"
    );
}

#[test]
fn a_lone_key_is_refused_symmetrically() {
    let mut c = cfg();
    c.key = Some("certs/site.key".to_string());
    let err = tls::requested(&c).expect_err("the other half alone is equally incomplete");
    assert!(err.starts_with(tls::E_SERVE_TLS_INCOMPLETE), "{err}");
    assert!(
        err.contains("cert"),
        "it must name the half that is MISSING: {err}"
    );
}

#[test]
fn the_floor_accepts_exactly_the_two_ruled_versions() {
    for (raw, want) in [
        ("1.2", tls::MinVersion::Tls12),
        ("1.3", tls::MinVersion::Tls13),
    ] {
        let mut c = cfg();
        c.cert = Some("c.pem".to_string());
        c.key = Some("k.pem".to_string());
        c.tls_min_version = raw.to_string();
        let req = tls::requested(&c)
            .expect("a ruled version")
            .expect("TLS requested");
        assert_eq!(req.min_version, want, "tlsMinVersion {raw:?}");
    }
}

#[test]
fn an_unruled_floor_is_refused_and_names_what_is_allowed() {
    // `"1.1"` is the case that matters: it is a REAL TLS version, just a deprecated one. Silently
    // clamping it up to 1.2 would be a policy decision made on the operator's behalf; silently
    // treating it as 1.0 would be worse. Refuse and say what is allowed.
    for raw in ["1.1", "1.0", "TLSv1.3", "", "1.4", "13"] {
        let mut c = cfg();
        c.cert = Some("c.pem".to_string());
        c.key = Some("k.pem".to_string());
        c.tls_min_version = raw.to_string();
        let err = tls::requested(&c).expect_err("{raw} is not a ruled floor");
        assert!(
            err.starts_with(tls::E_SERVE_TLS_MIN_VERSION),
            "{raw:?}: {err}"
        );
        assert!(err.contains("1.2") && err.contains("1.3"), "{raw:?}: {err}");
    }
}

#[test]
fn the_floor_is_not_validated_when_no_tls_is_requested() {
    // A program that never set cert/key has no TLS posture to get wrong, so a nonsense
    // `tlsMinVersion` must not fail its plain-HTTP server. Ruled this way because the field carries a
    // non-null class default: EVERY config has a `tlsMinVersion`, so validating it unconditionally
    // would make the field's mere existence able to refuse a server that does not use TLS.
    let mut c = cfg();
    c.tls_min_version = "nonsense".to_string();
    assert!(tls::requested(&c)
        .expect("no TLS requested → the floor is irrelevant")
        .is_none());
}

#[test]
fn an_incomplete_config_is_reported_before_the_missing_feature() {
    // ORDERING PIN. On a build without `http-server-tls`, a lone `cert` has TWO things wrong with it.
    // The config one is reported, because it is true regardless of how the binary was compiled —
    // reporting the build first would send the reader off to rebuild a binary that still would not
    // serve. `requested()` runs before `build()`, which is what gives this for free; the test is what
    // keeps a later refactor from inverting them.
    let mut c = cfg();
    c.cert = Some("certs/site.pem".to_string());
    let err = tls::requested(&c).expect_err("still incomplete");
    assert!(err.starts_with(tls::E_SERVE_TLS_INCOMPLETE), "{err}");
    assert!(
        !err.contains(tls::E_SERVE_TLS_DISABLED),
        "the build is not the user's problem here: {err}"
    );
}

/// The feature-OFF half of the contract. This test is the reason `TlsServer` is an UNINHABITED enum
/// when the feature is absent: there is then no value of the type at all, so no code path — however
/// it is later refactored — can produce a TLS server in a build that cannot do TLS. The refusal is a
/// type-system fact that this test merely observes.
#[cfg(not(feature = "http-server-tls"))]
#[test]
fn a_tls_config_on_a_build_without_the_feature_is_refused_loudly() {
    let mut c = cfg();
    c.cert = Some("certs/site.pem".to_string());
    c.key = Some("certs/site.key".to_string());
    let req = tls::requested(&c)
        .expect("the config itself is valid")
        .expect("TLS requested");
    let err = tls::build(&req).expect_err("this build cannot terminate TLS");
    assert!(err.starts_with(tls::E_SERVE_TLS_DISABLED), "{err}");
    assert!(
        err.contains("http-server-tls"),
        "it must name the feature that would fix it: {err}"
    );
    // The refusal must never read as advice to drop the cert and carry on in the clear.
    assert!(
        !err.to_lowercase().contains("plain http"),
        "the fix is to build with TLS, not to stop asking for it: {err}"
    );
}

#[test]
fn the_four_codes_keep_their_published_spelling() {
    // The other tests assert through the CONSTS, which is refactor-safe but blind to the one change
    // that actually reaches a user: renaming a const AND its value together. These codes are a
    // published interface — `phg explain E-SERVE-TLS-INCOMPLETE` is in the README, the help text and
    // the changelog — so the literal spelling is pinned here exactly once.
    assert_eq!(tls::E_SERVE_TLS_INCOMPLETE, "E-SERVE-TLS-INCOMPLETE");
    assert_eq!(tls::E_SERVE_TLS_MIN_VERSION, "E-SERVE-TLS-MIN-VERSION");
    assert_eq!(tls::E_SERVE_TLS_DISABLED, "E-SERVE-TLS-DISABLED");
    assert_eq!(tls::E_SERVE_TLS_CERT, "E-SERVE-TLS-CERT");

    // And each is explainable — `phg explain <code>` is where every diagnostic sends the reader.
    for code in [
        "E-SERVE-TLS-INCOMPLETE",
        "E-SERVE-TLS-MIN-VERSION",
        "E-SERVE-TLS-DISABLED",
        "E-SERVE-TLS-CERT",
    ] {
        let text = phorj::cli::explain_text(code)
            .unwrap_or_else(|| panic!("{code} has no `phg explain` entry"));
        assert!(
            text.starts_with(code),
            "the explanation must lead with the code: {text}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Handshake tier — `--features http-server-tls` only.
// ─────────────────────────────────────────────────────────────────────────────────────────────────
#[cfg(feature = "http-server-tls")]
mod handshake {
    use super::cfg;
    use phorj::serve::tls;
    use std::io::{Read, Write};
    use std::path::PathBuf;

    /// Generate a throwaway self-signed cert+key for `127.0.0.1` into a fresh temp dir. Returns
    /// `None` (after a LOUD skip line) when `openssl` is unavailable — the DB/mail convention. The
    /// key never leaves the temp dir and is never committed; a checked-in private key would trip
    /// secret scanners and push protection for no gain.
    fn self_signed() -> Option<(PathBuf, PathBuf, PathBuf)> {
        let dir = std::env::temp_dir().join(format!(
            "phorj-tls-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).expect("create the fixture dir");
        let cert = dir.join("cert.pem");
        let key = dir.join("key.pem");
        let out = std::process::Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
            ])
            .arg("-keyout")
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .output();
        match out {
            Ok(o) if o.status.success() => Some((dir, cert, key)),
            Ok(o) => {
                eprintln!(
                    "SKIP serve_tls handshake: openssl failed to generate a fixture: {}",
                    String::from_utf8_lossy(&o.stderr)
                );
                None
            }
            Err(e) => {
                eprintln!("SKIP serve_tls handshake: openssl not found ({e}) — install it to run this tier");
                None
            }
        }
    }

    /// A client verifier that trusts anything. Confined to this test module, and it is why: the
    /// fixture cert is self-signed, so a real verifier would reject it for exactly the right reason.
    /// The property under test is that a TLS session is established AT ALL with the floor honoured —
    /// not that a chain validates.
    #[derive(Debug)]
    struct TrustAnything(rustls::crypto::CryptoProvider);

    impl rustls::client::danger::ServerCertVerifier for TrustAnything {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }

    fn client_config(
        versions: &[&'static rustls::SupportedProtocolVersion],
    ) -> rustls::ClientConfig {
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let mut c = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(versions)
            .expect("the ring provider supports both ruled versions")
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(TrustAnything(
                (*provider).clone(),
            )))
            .with_no_client_auth();
        c.enable_sni = false;
        c
    }

    /// Bind a listener, accept ONE connection, and drive the server side of a TLS session over it,
    /// echoing a fixed body. Returns the bound address and the thread handle.
    fn serve_once(
        server: std::sync::Arc<rustls::ServerConfig>,
    ) -> (
        std::net::SocketAddr,
        std::thread::JoinHandle<Result<Vec<u8>, String>>,
    ) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
        let addr = listener.local_addr().expect("local_addr");
        let handle = std::thread::spawn(move || {
            let (sock, _) = listener.accept().map_err(|e| format!("accept: {e}"))?;
            let conn =
                rustls::ServerConnection::new(server).map_err(|e| format!("server conn: {e}"))?;
            let mut tls = rustls::StreamOwned::new(conn, sock);
            let mut buf = [0u8; 64];
            // The read is what drives the handshake to completion; a rejected client fails HERE.
            let n = tls.read(&mut buf).map_err(|e| format!("read: {e}"))?;
            tls.write_all(b"pong").map_err(|e| format!("write: {e}"))?;
            tls.flush().map_err(|e| format!("flush: {e}"))?;
            Ok(buf[..n].to_vec())
        });
        (addr, handle)
    }

    #[test]
    fn a_client_completes_a_handshake_and_exchanges_bytes() {
        let Some((dir, cert, key)) = self_signed() else {
            return;
        };
        let mut c = cfg();
        c.cert = Some(cert.to_string_lossy().into_owned());
        c.key = Some(key.to_string_lossy().into_owned());
        let req = tls::requested(&c).expect("valid").expect("TLS requested");
        let server = tls::build(&req).expect("the fixture cert and key build a server config");

        let (addr, handle) = serve_once(server.config());
        let name = rustls::pki_types::ServerName::try_from("localhost").expect("a valid DNS name");
        let conn = rustls::ClientConnection::new(
            std::sync::Arc::new(client_config(rustls::DEFAULT_VERSIONS)),
            name,
        )
        .expect("client conn");
        let sock = std::net::TcpStream::connect(addr).expect("connect");
        let mut client = rustls::StreamOwned::new(conn, sock);
        client
            .write_all(b"ping")
            .expect("the handshake completes and the write lands");
        client.flush().expect("flush");
        let mut back = [0u8; 4];
        client
            .read_exact(&mut back)
            .expect("the server answers over TLS");

        assert_eq!(&back, b"pong", "the round trip must survive encryption");
        assert_eq!(handle.join().expect("server thread"), Ok(b"ping".to_vec()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The test that proves the floor is WIRED, not merely parsed.** Every positive test above stays
    /// green if `min_version` is parsed and then dropped on the way to the `ServerConfig`. This one
    /// does not: a client that can only speak TLS 1.2, against a server whose floor is 1.3, must fail
    /// to establish a session at all.
    #[test]
    fn a_client_below_the_floor_is_rejected() {
        let Some((dir, cert, key)) = self_signed() else {
            return;
        };
        let mut c = cfg();
        c.cert = Some(cert.to_string_lossy().into_owned());
        c.key = Some(key.to_string_lossy().into_owned());
        c.tls_min_version = "1.3".to_string();
        let req = tls::requested(&c).expect("valid").expect("TLS requested");
        assert_eq!(req.min_version, tls::MinVersion::Tls13);
        let server = tls::build(&req).expect("builds");

        let (addr, handle) = serve_once(server.config());
        let name = rustls::pki_types::ServerName::try_from("localhost").expect("a valid DNS name");
        let conn = rustls::ClientConnection::new(
            std::sync::Arc::new(client_config(&[&rustls::version::TLS12])),
            name,
        )
        .expect("client conn");
        let sock = std::net::TcpStream::connect(addr).expect("connect");
        let mut client = rustls::StreamOwned::new(conn, sock);
        let mut back = [0u8; 4];
        // Where the failure lands is not pinned — rustls may reject at the write that drives the
        // handshake, or at the read of the server's alert. Either is correct; what must NOT happen is
        // that all three succeed, which is the case where the floor was silently ignored.
        let read_outcome = client
            .write_all(b"ping")
            .and_then(|()| client.flush())
            .and_then(|()| client.read_exact(&mut back));
        assert!(
            read_outcome.is_err(),
            "a TLS-1.2-only client must NOT be able to talk to a 1.3-floor server"
        );
        assert!(
            handle.join().expect("server thread").is_err(),
            "and the server side must have failed the session too, not answered in the clear"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_cert_file_names_the_path_it_could_not_read() {
        let mut c = cfg();
        c.cert = Some("/nonexistent/phorj-test/cert.pem".to_string());
        c.key = Some("/nonexistent/phorj-test/key.pem".to_string());
        let req = tls::requested(&c)
            .expect("the config shape is valid")
            .expect("TLS requested");
        let err = tls::build(&req).expect_err("there is no such file");
        assert!(err.starts_with(tls::E_SERVE_TLS_CERT), "{err}");
        assert!(err.contains("/nonexistent/phorj-test/cert.pem"), "{err}");
    }

    #[test]
    fn a_malformed_pem_is_refused_rather_than_ignored() {
        let Some((dir, cert, key)) = self_signed() else {
            return;
        };
        // A cert file that is not PEM at all. The danger being guarded is a decoder that returns an
        // EMPTY certificate list for unparseable input and lets rustls build a server with no
        // identity — which fails later, at handshake time, on every request instead of at startup.
        std::fs::write(&cert, b"this is not a certificate\n").expect("overwrite the fixture");
        let mut c = cfg();
        c.cert = Some(cert.to_string_lossy().into_owned());
        c.key = Some(key.to_string_lossy().into_owned());
        let req = tls::requested(&c).expect("valid").expect("TLS requested");
        let err = tls::build(&req).expect_err("a file with no PEM block cannot be a certificate");
        assert!(err.starts_with(tls::E_SERVE_TLS_CERT), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

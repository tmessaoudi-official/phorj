//! `phg explain` — the inbound-TLS codes (DEC-331 D7, S3.5).
//!
//! Their own module rather than rows in `names_types.rs`, which is already past Invariant 13's
//! 300-line soft cap; these four are a cohesive family (one feature, one failure domain).
//!
//! Every explanation here has the same job beyond describing the code: say why the answer is a
//! refusal instead of a fallback. Each of these conditions has an obvious "helpful" behaviour — serve
//! plain HTTP, clamp the version up, carry on without TLS — and each of them ends with a server the
//! operator believes is encrypted and is not.

pub(super) fn text(code: &str) -> Option<&'static str> {
    Some(match code {
        "E-SERVE-TLS-INCOMPLETE" => {
            "E-SERVE-TLS-INCOMPLETE — `ServeConfig` set one half of the TLS pair, not both.\n\n\
             HTTPS needs a certificate AND its private key. This config named one of them, so the\n\
             server cannot be started as configured.\n\n\
             \x20   // refused\n\
             \x20   new ServeConfig(port: 8443, cert: \"certs/site.pem\")\n\n\
             \x20   // serves HTTPS\n\
             \x20   new ServeConfig(port: 8443, cert: \"certs/site.pem\", key: \"certs/site.key\")\n\n\
             WHY THIS IS AN ERROR RATHER THAN PLAIN HTTP. D7's surface says HTTPS auto-enables\n\
             \"iff BOTH are set\", and the literal reading of that would be to fall back to clear\n\
             text here. Phorj refuses instead: someone who wrote `cert:` believes the port is\n\
             encrypted, and a server that quietly is not looks identical to one that is — the\n\
             mistake surfaces when someone else finds credentials in a packet capture.\n\n\
             If you genuinely want plain HTTP on this port, remove the field you did set. That is a\n\
             deliberate edit, which is the point.\n"
        }
        "E-SERVE-TLS-MIN-VERSION" => {
            "E-SERVE-TLS-MIN-VERSION — `ServeConfig.tlsMinVersion` is not a supported floor.\n\n\
             The accepted values are exactly \"1.2\" (the default) and \"1.3\". The check is strict:\n\
             \"TLSv1.3\", \"1.30\" and stray whitespace are all rejected rather than interpreted.\n\n\
             \x20   new ServeConfig(cert: c, key: k, tlsMinVersion: \"1.3\")   // TLS 1.3 only\n\
             \x20   new ServeConfig(cert: c, key: k)                        // floor 1.2, the default\n\n\
             WHY \"1.1\" IS REFUSED RATHER THAN RAISED. TLS 1.0 and 1.1 are deprecated and phorj's TLS\n\
             stack does not implement them, so honouring `\"1.1\"` could only mean serving something\n\
             other than what was asked for. Silently raising the floor to 1.2 would be a security\n\
             decision made on your behalf; a floor is a control, and guessing at intent is how a\n\
             control ends up somewhere other than where its author believes it is.\n\n\
             This field is validated ONLY when TLS is actually requested — a plain-HTTP server is\n\
             never refused over a value it does not use.\n"
        }
        "E-SERVE-TLS-DISABLED" => {
            "E-SERVE-TLS-DISABLED — this `phg` was built without inbound TLS.\n\n\
             The program's `ServeConfig` asks for HTTPS, but `phg serve`'s TLS support is behind the\n\
             non-default `http-server-tls` feature and this binary does not carry it.\n\n\
             \x20   $ cargo build --release --features http-server-tls\n\n\
             WHY THE SERVER DOES NOT JUST START WITHOUT TLS. Because that is the one outcome nobody\n\
             asked for: the port binds, requests are answered, and the encryption the config\n\
             requested is absent with nothing in the response to say so. The refusal is enforced by\n\
             the type system rather than a runtime check — without the feature the internal\n\
             `TlsServer` type is uninhabited, so no code path can produce one.\n\n\
             TLS is off by default because `phg serve`'s default posture is a loopback development\n\
             server, and linking a TLS stack a server never uses is attack surface for nothing.\n\n\
             If a config error is ALSO present (a lone `cert`, say), you will see that one first:\n\
             the config is wrong however this binary was compiled, and rebuilding would not fix it.\n"
        }
        "E-SERVE-TLS-CERT" => {
            "E-SERVE-TLS-CERT — the certificate or private key could not be loaded.\n\n\
             Covers every I/O-shaped failure of the pair: the path does not exist or is unreadable,\n\
             the file contains no PEM block of the expected kind, the base64 is corrupt, or the two\n\
             are not a matching certificate/key pair. The message names the path and the underlying\n\
             cause.\n\n\
             Paths resolve against the process working directory — the one you ran `phg serve` from,\n\
             NOT the application root in site mode. A relative `certs/site.pem` therefore follows\n\
             your shell, not your project layout.\n\n\
             Accepted key encodings: PKCS#8 (`BEGIN PRIVATE KEY`), PKCS#1 (`BEGIN RSA PRIVATE KEY`)\n\
             and SEC1 (`BEGIN EC PRIVATE KEY`). A key still wrapped in a passphrase is not supported;\n\
             decrypt it first.\n\n\
             WHY THIS STOPS STARTUP. A server built with no usable identity binds its port perfectly\n\
             well and then fails every TLS handshake — which reads as a network problem, is reported\n\
             by clients rather than by the server, and can persist for a long time. Failing here\n\
             makes it a startup error you see immediately, on the terminal that started it.\n\n\
             To generate a throwaway certificate for local development:\n\n\
             \x20   openssl req -x509 -newkey rsa:2048 -nodes -days 365 \\\n\
             \x20     -subj /CN=localhost -keyout certs/site.key -out certs/site.pem\n"
        }
        _ => return None,
    })
}

//! `Core.HttpClient` unit tests (sibling file per Invariant 13): URL parsing, redirect resolution,
//! chunked decoding, and full request/response round-trips against an in-process
//! `std::net::TcpListener` fixture server — deterministic, no external network, no TLS (the https
//! path is covered by the opt-in live test in `tests/http_client.rs`).

use super::*;

// ── URL parsing ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_url_covers_the_shapes() {
    let u = parse_url("http://example.com").unwrap();
    assert_eq!((u.https, u.port, u.target.as_str()), (false, 80, "/"));
    let u = parse_url("https://example.com/a/b?q=1").unwrap();
    assert_eq!(
        (u.https, u.port, u.target.as_str()),
        (true, 443, "/a/b?q=1")
    );
    let u = parse_url("http://h:8080?x").unwrap();
    assert_eq!((u.port, u.target.as_str()), (8080, "/?x"));
    let u = parse_url("http://[::1]:9999/z").unwrap();
    assert_eq!((u.host.as_str(), u.port), ("::1", 9999));
    // Rejections: scheme, userinfo (credential smuggling), empty host, junk port.
    assert!(parse_url("ftp://x/").is_err());
    assert!(parse_url("http://user:pw@host/")
        .unwrap_err()
        .contains("userinfo"));
    assert!(parse_url("http:///x").is_err());
    assert!(parse_url("http://h:99999/").is_err());
}

#[test]
fn resolve_location_absolute_rooted_and_relative() {
    let cur = parse_url("http://h:81/a/b/c?q").unwrap();
    assert_eq!(
        resolve_location(&cur, "https://other/x").unwrap().host,
        "other"
    );
    assert_eq!(resolve_location(&cur, "/root").unwrap().target, "/root");
    assert_eq!(resolve_location(&cur, "sib").unwrap().target, "/a/b/sib");
}

// ── Chunked decoding ─────────────────────────────────────────────────────────────────────────────────

#[test]
fn decode_chunked_reassembles_and_rejects_junk() {
    let body = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
    assert_eq!(decode_chunked(body).unwrap(), b"Wikipedia");
    // A chunk-size extension is tolerated; garbage sizes are not.
    let ext = b"4;name=v\r\nWiki\r\n0\r\n\r\n";
    assert_eq!(decode_chunked(ext).unwrap(), b"Wiki");
    assert!(decode_chunked(b"zz\r\nx\r\n0\r\n\r\n").is_err());
    assert!(decode_chunked(b"5\r\nab\r\n").is_err());
}

// ── Fixture-server round-trips ───────────────────────────────────────────────────────────────────────

/// Spawn a one-shot (or N-shot) fixture server returning canned raw responses; returns its port.
fn fixture(responses: Vec<Vec<u8>>) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for resp in responses {
            let (mut sock, _) = match listener.accept() {
                Ok(s) => s,
                Err(_) => return,
            };
            // Drain the request head (fixture servers don't parse bodies beyond the head).
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf);
            let _ = sock.write_all(&resp);
        }
    });
    port
}

#[test]
fn get_with_content_length_round_trips() {
    let port = fixture(vec![
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
    ]);
    let r = run_request(
        "GET",
        &format!("http://127.0.0.1:{port}/x"),
        &[],
        &[],
        5000,
        5,
        false,
    )
    .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"hello");
    assert_eq!(
        r.headers
            .iter()
            .find(|(n, _)| n == "content-type")
            .map(|(_, v)| v.as_str()),
        Some("text/plain")
    );
}

#[test]
fn chunked_response_round_trips() {
    let port = fixture(vec![
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n"
            .to_vec(),
    ]);
    let r = run_request(
        "GET",
        &format!("http://127.0.0.1:{port}/c"),
        &[],
        &[],
        5000,
        5,
        false,
    )
    .unwrap();
    assert_eq!(r.body, b"Wikipedia");
}

#[test]
fn redirect_is_followed_and_capped() {
    let port2 = fixture(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ndest".to_vec()
    ]);
    let port1 = fixture(vec![format!(
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port2}/final\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes()]);
    let r = run_request(
        "GET",
        &format!("http://127.0.0.1:{port1}/a"),
        &[],
        &[],
        5000,
        5,
        false,
    )
    .unwrap();
    assert_eq!((r.status, r.body.as_slice()), (200, b"dest".as_slice()));
    // Cap: a 0-redirect budget on a redirecting URL is the typed TooManyRedirects error.
    let port3 = fixture(vec![
        b"HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n".to_vec(),
    ]);
    let e = run_request(
        "GET",
        &format!("http://127.0.0.1:{port3}/a"),
        &[],
        &[],
        5000,
        0,
        false,
    )
    .unwrap_err();
    assert!(e.contains("<<TooManyRedirects>>"), "{e}");
}

#[test]
fn post_body_and_303_downgrade_to_get() {
    // The 303 target answers 200 only to a GET with an empty body (the fixture can't assert, but a
    // POST would carry a body the fixture ignores — the visible contract is the status flow).
    let port2 = fixture(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec()
    ]);
    let port1 = fixture(vec![format!(
        "HTTP/1.1 303 See Other\r\nLocation: http://127.0.0.1:{port2}/done\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes()]);
    let r = run_request(
        "POST",
        &format!("http://127.0.0.1:{port1}/submit"),
        &[("content-type".into(), "application/json".into())],
        b"{\"a\":1}",
        5000,
        5,
        false,
    )
    .unwrap();
    assert_eq!(r.status, 200);
}

#[test]
fn timeout_is_typed() {
    // A listener that accepts but never responds → read timeout → <<Timeout>>.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (_sock, _) = listener.accept().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
    let e = run_request(
        "GET",
        &format!("http://127.0.0.1:{port}/"),
        &[],
        &[],
        60,
        0,
        false,
    )
    .unwrap_err();
    assert!(e.contains("<<Timeout>>"), "{e}");
}

// ── DEC-264: credential stripping on cross-origin redirects ────────────────────────────────────────

#[test]
fn same_origin_compares_scheme_host_port() {
    let a = parse_url("http://h:80/a").unwrap();
    assert!(same_origin(&a, &parse_url("http://h/b?q").unwrap())); // default port 80, path differs
    assert!(same_origin(&a, &parse_url("http://H:80/x").unwrap())); // host case-insensitive
    assert!(!same_origin(&a, &parse_url("http://h:81/a").unwrap())); // port differs
    assert!(!same_origin(&a, &parse_url("http://other/a").unwrap())); // host differs
    assert!(!same_origin(&a, &parse_url("https://h/a").unwrap())); // scheme differs (downgrade/upgrade)
                                                                   // Scheme term isolated from the port term: SAME host AND SAME explicit port, only scheme differs
                                                                   // (a plaintext https→http downgrade to the identical host:port) — must still be cross-origin so the
                                                                   // credential strip fires. Without this, the default-port asymmetry (80 vs 443) would mask the bug.
    assert!(!same_origin(
        &parse_url("https://h:443/a").unwrap(),
        &parse_url("http://h:443/a").unwrap()
    ));
}

#[test]
fn credential_headers_stripped_cross_origin_kept_same_origin() {
    let hdrs = vec![
        ("Authorization".to_string(), "Bearer sekret".to_string()),
        ("Cookie".to_string(), "sid=abc".to_string()),
        ("Proxy-Authorization".to_string(), "Basic zzz".to_string()),
        ("WWW-Authenticate".to_string(), "Bearer".to_string()),
        ("X-Trace".to_string(), "keep-me".to_string()),
        ("Accept".to_string(), "application/json".to_string()),
    ];
    let a = parse_url("https://api.example.com/v1").unwrap();

    // Same origin (only the path changes) — every header survives.
    let same = headers_for_hop(&a, &parse_url("https://api.example.com/v2").unwrap(), &hdrs);
    assert_eq!(same.len(), hdrs.len());

    // Cross origin — the four credential headers drop, the non-sensitive ones stay.
    let cross = headers_for_hop(&a, &parse_url("https://evil.example.net/x").unwrap(), &hdrs);
    let names: Vec<&str> = cross.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["X-Trace", "Accept"],
        "only non-credential headers survive"
    );

    // https→http downgrade (same host/port-family) also strips (scheme change = cross origin).
    let down = headers_for_hop(&a, &parse_url("http://api.example.com/v1").unwrap(), &hdrs);
    assert!(!down
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("authorization")));
}

#[test]
fn run_request_strips_credentials_on_cross_origin_redirect_e2e() {
    use std::sync::{Arc, Mutex};
    // A fixture that RECORDS the request head it received (so we can assert which headers arrived).
    fn recording_fixture(resp: Vec<u8>) -> (u16, Arc<Mutex<String>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(String::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                // Read until the full request HEAD arrives (a single `read` can return a partial head
                // under TCP segmentation — reading once made this test flaky).
                let mut acc = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    match sock.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            acc.extend_from_slice(&buf[..n]);
                            if acc.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                *seen2.lock().unwrap() = String::from_utf8_lossy(&acc).to_string();
                let _ = sock.write_all(&resp);
            }
        });
        (port, seen)
    }

    // Destination on a DIFFERENT origin (different port) answers 200.
    let (dest_port, dest_seen) =
        recording_fixture(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec());
    // Source 302-redirects cross-origin to the destination.
    let (src_port, src_seen) = recording_fixture(
        format!(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{dest_port}/final\r\nContent-Length: 0\r\n\r\n"
        )
        .into_bytes(),
    );

    let r = run_request(
        "GET",
        &format!("http://127.0.0.1:{src_port}/start"),
        &[
            (
                "Authorization".to_string(),
                "Bearer sekret-token".to_string(),
            ),
            ("X-Trace".to_string(), "keep-me".to_string()),
        ],
        &[],
        5000,
        5,
        false,
    )
    .unwrap();
    assert_eq!((r.status, r.body.as_slice()), (200, b"ok".as_slice()));

    let src_head = src_seen.lock().unwrap().clone();
    let dest_head = dest_seen.lock().unwrap().clone();
    // First hop (same origin as the request) carried the Authorization (header name case is preserved
    // as written, so compare case-insensitively).
    assert!(
        src_head
            .to_ascii_lowercase()
            .contains("authorization: bearer sekret-token"),
        "src: {src_head}"
    );
    // Cross-origin hop: Authorization STRIPPED, no plaintext token leaked, X-Trace preserved.
    assert!(
        !dest_head.to_ascii_lowercase().contains("authorization"),
        "dest leaked auth: {dest_head}"
    );
    assert!(
        !dest_head.contains("sekret-token"),
        "dest leaked token: {dest_head}"
    );
    assert!(
        dest_head.to_ascii_lowercase().contains("x-trace: keep-me"),
        "dest dropped non-credential header: {dest_head}"
    );
}

// ── DEC-270: SSRF guard (block private/link-local/metadata, allow loopback) ─────────────────────────

#[test]
fn is_blocked_ip_blocks_private_link_local_metadata_allows_loopback_and_public() {
    use std::net::IpAddr;
    let b = |s: &str| is_blocked_ip(s.parse::<IpAddr>().unwrap());
    // BLOCKED — the SSRF-exfiltration targets:
    assert!(b("169.254.169.254"), "cloud metadata endpoint"); // the headline threat
    assert!(b("169.254.1.1"), "link-local");
    assert!(b("10.0.0.5"), "RFC1918 10/8");
    assert!(b("172.16.5.4"), "RFC1918 172.16/12");
    assert!(b("192.168.1.1"), "RFC1918 192.168/16");
    assert!(b("0.0.0.0"), "unspecified");
    assert!(b("255.255.255.255"), "broadcast");
    assert!(b("fc00::1"), "IPv6 ULA");
    assert!(b("fe80::1"), "IPv6 link-local");
    assert!(b("::"), "IPv6 unspecified");
    assert!(
        b("::ffff:169.254.169.254"),
        "IPv4-mapped metadata must not slip through"
    );
    assert!(b("::ffff:10.0.0.1"), "IPv4-mapped private");
    assert!(b("100.64.0.1"), "CGNAT/shared 100.64/10");
    assert!(b("100.100.100.200"), "Alibaba metadata (in 100.64/10)");
    assert!(
        b("192.0.0.192"),
        "IETF-assignments 192.0.0.0/24 metadata target"
    );
    assert!(b("2002:a9fe:a9fe::"), "6to4 embedding 169.254.169.254");
    assert!(b("2002:0a00:0005::"), "6to4 embedding 10.0.0.5");
    assert!(
        b("64:ff9b::a9fe:a9fe"),
        "NAT64 embedding 169.254.169.254 (DNS64 bypass)"
    );
    assert!(b("64:ff9b::0a00:0001"), "NAT64 embedding 10.0.0.1");
    assert!(
        b("::a9fe:a9fe"),
        "IPv4-compatible embedding 169.254.169.254"
    );
    // ALLOWED — loopback (legitimate: sidecars, dev) and genuine public addresses:
    assert!(!b("127.0.0.1"), "loopback allowed");
    assert!(!b("127.5.5.5"), "loopback /8 allowed");
    assert!(!b("::1"), "IPv6 loopback allowed");
    assert!(!b("::ffff:127.0.0.1"), "IPv4-mapped loopback allowed");
    assert!(!b("93.184.216.34"), "public (example.com) allowed");
    assert!(!b("8.8.8.8"), "public allowed");
    assert!(
        !b("2606:2800:220:1:248:1893:25c8:1946"),
        "public IPv6 allowed"
    );
    // OVER-BLOCK guards (pin the widened bounds so a future "simplify to a blanket block" regresses):
    assert!(!b("100.0.0.1"), "100.0/… is public, below CGNAT 100.64/10");
    assert!(
        !b("100.128.0.1"),
        "100.128/… is public, above CGNAT 100.64/10"
    );
    assert!(
        !b("192.0.2.1"),
        "192.0.2/24 TEST-NET is not the blocked 192.0.0/24"
    );
    assert!(
        !b("2002:5db8:d822::"),
        "6to4 embedding PUBLIC 93.184.216.34 must stay allowed (not a blanket 2002::/16 block)"
    );
}

#[test]
fn run_request_blocks_metadata_ip_by_default_and_opt_in_bypasses() {
    // Default (allow_private=false): a metadata/link-local host is refused BEFORE any connect — instant
    // typed <<BlockedAddress>>, no hang. (Port 1 would never accept; we never reach it.)
    let e = run_request(
        "GET",
        "http://169.254.169.254:1/latest/meta-data/",
        &[],
        &[],
        200,
        0,
        false,
    )
    .unwrap_err();
    assert!(
        e.contains("<<BlockedAddress>>"),
        "expected BlockedAddress, got: {e}"
    );
    assert!(
        e.contains("169.254.169.254"),
        "error names the refused address: {e}"
    );
    // A private LAN host is likewise blocked by default.
    let e2 = run_request("GET", "http://10.0.0.5:1/", &[], &[], 200, 0, false).unwrap_err();
    assert!(e2.contains("<<BlockedAddress>>"), "{e2}");
    // Opt-in (allow_private=true) bypasses the guard: it PROCEEDS to connect (and fails with a
    // connection/timeout error against the dead port) — the point is it is NOT <<BlockedAddress>>.
    let e3 = run_request("GET", "http://10.0.0.5:1/", &[], &[], 200, 0, true).unwrap_err();
    assert!(
        !e3.contains("<<BlockedAddress>>"),
        "opt-in must bypass the SSRF guard: {e3}"
    );
}

#[test]
fn header_injection_is_rejected_at_the_gate() {
    let mut out = String::new();
    let r = hc_request(
        &[
            Value::Str("GET".into()),
            Value::Str("http://127.0.0.1:1/".into()),
            Value::List(Rc::new(vec![Value::Str("x-evil".into())])),
            Value::List(Rc::new(vec![Value::Str("a\r\nHost: evil".into())])),
            Value::Null,
            Value::Int(100),
            Value::Int(0),
            Value::Bool(false),
        ],
        &mut out,
    )
    .unwrap();
    let s = format!("{r:?}");
    assert!(s.contains("forbidden character"), "{s}");
}

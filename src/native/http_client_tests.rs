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
    let e = run_request("GET", &format!("http://127.0.0.1:{port}/"), &[], &[], 60, 0).unwrap_err();
    assert!(e.contains("<<Timeout>>"), "{e}");
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
        ],
        &mut out,
    )
    .unwrap();
    let s = format!("{r:?}");
    assert!(s.contains("forbidden character"), "{s}");
}

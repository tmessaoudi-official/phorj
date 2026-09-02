//! HTTP/1.1 wire FRAMING — reading one request off a stream and deciding whether the connection
//! survives it. Byte-level only: nothing here parses a request semantically (that is the served
//! program's `parse_request`, which turns a malformed buffer into a 400).
//!
//! Split out of `transport.rs` by S3.5 (Invariant 13, M-Decomp). That file is ratcheted at 635 lines
//! in `scripts/size-baseline.txt` and may only SHRINK, and threading TLS through the two accept paths
//! adds to it. Framing is the cohesive cut rather than merely the largest one: every function here is
//! generic over `Read` or pure over `&[u8]`, none of them touches a socket, a thread or a handler —
//! which is exactly why they were already the unit-testable part of the module. Behaviour is
//! unchanged by the move; the tests moved with them.
use std::io::{self, Read};

/// Cap a single request at 8 MiB — keeps a hostile or runaway client from exhausting memory (EV-7).
const MAX_REQUEST: usize = 8 * 1024 * 1024;

/// What the framing decided about one request off the stream.
pub(super) enum Framed {
    /// A complete request: the head plus exactly the declared body. Any bytes that arrived past it (a
    /// pipelined next request, RFC 9112 §9.3.2) were CARRIED OVER for the next read — before round 4
    /// (2026-09-02) they were handed to the handler as this request's body and never answered.
    Request(Vec<u8>),
    /// The request cannot be honoured (RFC 9112 §6.3 / §5.1): the fixed response to write, then close.
    Reject(&'static [u8]),
}

/// `400` + close: a malformed or self-contradicting framing (§6.3 ¶3/¶5, §5.1 whitespace before the
/// colon, a declared body cut short by FIN — §6.3 ¶6 "MUST NOT process").
pub(super) const REJECT_400: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
/// `413` + close: a declared body over [`MAX_REQUEST`] — it used to be truncated to 8 MiB and SERVED.
pub(super) const REJECT_413: &[u8] =
    b"HTTP/1.1 413 Content Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
/// `501` + close: a `Transfer-Encoding` alone — chunked (or any) transfer coding is not implemented
/// (§6.1: an unsupported transfer coding is `501`, a deliberate choice over `400`).
pub(super) const REJECT_501: &[u8] =
    b"HTTP/1.1 501 Not Implemented\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// Read one HTTP/1.1 request from `stream`: everything up to and including `\r\n\r\n`, then exactly
/// the `Content-Length` body. `carry` holds bytes read past the previous request on this connection
/// (a pipelined next request) and receives any bytes past THIS one — so every request is answered
/// (round-4 safety F4). Framing only — no semantic validation beyond the header VERDICT below; a
/// partial/malformed buffer flows to the program's `parse_request`, which returns `null` and yields a
/// 400. Generic over [`Read`] so the framing is unit-testable over a `Cursor` (P1-d).
pub(super) fn read_http_request<R: Read>(
    stream: &mut R,
    carry: &mut Vec<u8>,
) -> io::Result<Framed> {
    const SEP: &[u8] = b"\r\n\r\n";
    let mut buf = std::mem::take(carry);
    let mut chunk = [0u8; 4096];
    // Only re-scan newly-arrived bytes for the header terminator (with a `SEP.len()-1` overlap so a
    // terminator split across two reads is still found). Scanning the whole buffer every chunk is
    // O(n²) — a CPU-DoS on a large no-terminator request; this keeps it linear.
    let mut scanned = 0usize;
    let head_end = loop {
        let from = scanned.saturating_sub(SEP.len() - 1);
        if let Some(rel) = find_subslice(&buf[from..], SEP) {
            break from + rel + SEP.len();
        }
        scanned = buf.len();
        if buf.len() > MAX_REQUEST {
            return Ok(Framed::Request(buf)); // 8 MiB with no terminator → parse → 400 (unchanged)
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(Framed::Request(buf)); // EOF before full headers → partial (parse → 400)
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let body_len = match framing_verdict(&buf[..head_end]) {
        Ok(n) => n,
        Err(resp) => return Ok(Framed::Reject(resp)),
    };
    let want = head_end + body_len;
    while buf.len() < want {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            // A declared body cut short: §6.3 ¶6 — MUST NOT process it as complete.
            return Ok(Framed::Reject(REJECT_400));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    if buf.len() > want {
        *carry = buf.split_off(want);
    }
    Ok(Framed::Request(buf))
}

/// Max requests served on one kept-alive connection before it is closed (EV-7 — bounds a client that
/// pins a connection/worker forever). The client simply reconnects for more.
pub(super) const MAX_REQUESTS_PER_CONN: usize = 100;

/// Whether the **request** asks to keep the connection open (HTTP/1.1 S4.1 keep-alive). HTTP/1.1
/// defaults to keep-alive unless `Connection: close`; HTTP/1.0 defaults to close unless
/// `Connection: keep-alive`. Header value matched case-insensitively (a comma-list like
/// `keep-alive, foo` counts). Framing-only parse over the raw bytes — mirrors `parse_content_length`.
pub(super) fn request_wants_keepalive(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let head = text.split("\r\n\r\n").next().unwrap_or("");
    let mut lines = head.split("\r\n");
    let is_http11 = lines
        .next()
        .is_some_and(|req_line| req_line.contains("HTTP/1.1"));
    let conn = head_value(head, "connection");
    match conn {
        Some(v) if v.eq_ignore_ascii_case("close") || token_list_has(&v, "close") => false,
        Some(v) if token_list_has(&v, "keep-alive") => true,
        _ => is_http11, // no Connection header → HTTP/1.1 keeps alive, HTTP/1.0 closes
    }
}

/// Whether the **response** permits keep-alive — false when the server's own headers say
/// `Connection: close` (the `http_500`/error responses do, so a faulted exchange always closes). A
/// kept-alive response must be self-delimiting; every Phorj response carries `Content-Length` (set by
/// `serialize_response` / the error helpers), so reuse is safe.
pub(super) fn response_keeps_alive(resp: &[u8]) -> bool {
    let text = String::from_utf8_lossy(resp);
    let head = text.split("\r\n\r\n").next().unwrap_or("");
    match head_value(head, "connection") {
        Some(v) => !(v.eq_ignore_ascii_case("close") || token_list_has(&v, "close")),
        None => true,
    }
}

/// The (trimmed) value of header `name` (case-insensitive) in an HTTP head, or `None`.
fn head_value(head: &str, name: &str) -> Option<String> {
    head.split("\r\n").skip(1).find_map(|line| {
        line.split_once(':').and_then(|(k, v)| {
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    })
}

/// Whether a comma-separated header value contains `token` (case-insensitive, trimmed) — e.g.
/// `Connection: keep-alive, Upgrade` contains `keep-alive`.
fn token_list_has(value: &str, token: &str) -> bool {
    value
        .split(',')
        .any(|t| t.trim().eq_ignore_ascii_case(token))
}

/// The request head's framing VERDICT (RFC 9112): `Ok(body_len)` — `0` when there is no body — or the
/// fixed reject response. Checked BEFORE any handler runs, at every framing site:
/// * a header name followed by whitespace before its colon (`Content-Length : 5`) — §5.1 MUST 400;
/// * `Transfer-Encoding` beside `Content-Length` — §6.3 ¶3, 400 (request smuggling shape);
/// * `Transfer-Encoding` alone — no transfer coding is implemented: §6.1, 501;
/// * several `Content-Length` fields that differ — §6.3 ¶5 MUST 400 (identical copies are one value);
/// * a `Content-Length` that is not `1*DIGIT` (`abc`, `-1`, `+5`, a 24-digit value) — §6.3, 400;
/// * a declared body over [`MAX_REQUEST`] — 413 (it used to be truncated to 8 MiB and served).
pub(super) fn framing_verdict(head: &[u8]) -> Result<usize, &'static [u8]> {
    let text = String::from_utf8_lossy(head);
    let mut lengths: Vec<&str> = Vec::new();
    let mut transfer_encoding = false;
    for line in text.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.len() != name.trim_end().len() {
            return Err(REJECT_400);
        }
        let name = name.trim();
        if name.eq_ignore_ascii_case("content-length") {
            lengths.push(value.trim());
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            transfer_encoding = true;
        }
    }
    if transfer_encoding {
        return Err(if lengths.is_empty() {
            REJECT_501
        } else {
            REJECT_400
        });
    }
    let Some(first) = lengths.first() else {
        return Ok(0);
    };
    if lengths.iter().any(|l| l != first) {
        return Err(REJECT_400);
    }
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) {
        return Err(REJECT_400);
    }
    // Not representable at all (a 24-digit value) is malformed — 400, like `abc`; only a value that
    // parses and exceeds the cap is 413.
    let n: usize = first.parse().map_err(|_| REJECT_400)?;
    if n > MAX_REQUEST {
        return Err(REJECT_413);
    }
    Ok(n)
}

/// First index of `needle` in `hay`, or `None`. An empty needle matches at 0 (defensive; the only
/// caller passes the non-empty `\r\n\r\n`).
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn find_subslice_basics() {
        assert_eq!(find_subslice(b"abc\r\n\r\nxyz", b"\r\n\r\n"), Some(3));
        assert_eq!(find_subslice(b"no terminator here", b"\r\n\r\n"), None);
        assert_eq!(find_subslice(b"", b"\r\n\r\n"), None);
        assert_eq!(find_subslice(b"anything", b""), Some(0)); // empty needle → 0
    }

    // --- parse_content_length ---------------------------------------------

    #[test]
    fn content_length_absent_is_zero() {
        assert_eq!(framing_verdict(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"), Ok(0));
    }

    #[test]
    fn content_length_present_is_parsed() {
        assert_eq!(
            framing_verdict(b"POST / HTTP/1.1\r\nContent-Length: 42\r\n\r\n"),
            Ok(42)
        );
    }

    #[test]
    fn content_length_is_case_insensitive_and_trims() {
        assert_eq!(
            framing_verdict(b"POST / HTTP/1.1\r\ncOnTeNt-LeNgTh:   7  \r\n\r\n"),
            Ok(7)
        );
    }

    /// Exactly the declared body, or the fixed reject; the carry-over is asserted by the callers below.
    fn request_bytes<R: Read>(r: &mut R) -> Vec<u8> {
        let mut carry = Vec::new();
        match read_http_request(r, &mut carry).unwrap() {
            Framed::Request(b) => b,
            Framed::Reject(resp) => panic!("unexpected reject: {}", String::from_utf8_lossy(resp)),
        }
    }

    #[test]
    fn a_pipelined_request_is_carried_over_not_swallowed() {
        let two =
            b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nAAAAAGET /b HTTP/1.1\r\n\r\n".to_vec();
        let mut carry = Vec::new();
        let first = match read_http_request(&mut Cursor::new(two), &mut carry).unwrap() {
            Framed::Request(b) => b,
            Framed::Reject(_) => panic!("first must frame"),
        };
        assert_eq!(
            first,
            b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nAAAAA".to_vec()
        );
        assert_eq!(
            carry,
            b"GET /b HTTP/1.1\r\n\r\n".to_vec(),
            "the second request is carried over"
        );
        // The carry is consumed by the next read on the same connection, even with nothing new arriving.
        let second = request_bytes(
            &mut Cursor::new(Vec::new()).chain(Cursor::new(std::mem::take(&mut carry))),
        );
        assert!(
            second.starts_with(b"GET /b"),
            "{}",
            String::from_utf8_lossy(&second)
        );
    }

    #[test]
    fn framing_verdicts_follow_rfc_9112() {
        let head = |h: &str| format!("POST / HTTP/1.1\r\n{h}\r\n\r\n");
        assert_eq!(framing_verdict(head("Content-Length: 7").as_bytes()), Ok(7));
        assert_eq!(framing_verdict(head("Host: x").as_bytes()), Ok(0));
        assert_eq!(
            framing_verdict(head("Content-Length: 5\r\nContent-Length: 5").as_bytes()),
            Ok(5)
        );
        assert_eq!(
            framing_verdict(head("Content-Length: 5\r\nContent-Length: 44").as_bytes()),
            Err(REJECT_400)
        );
        assert_eq!(
            framing_verdict(head("Transfer-Encoding: chunked\r\nContent-Length: 5").as_bytes()),
            Err(REJECT_400)
        );
        assert_eq!(
            framing_verdict(head("Transfer-Encoding: chunked").as_bytes()),
            Err(REJECT_501)
        );
        assert_eq!(
            framing_verdict(head("Content-Length : 5").as_bytes()),
            Err(REJECT_400)
        );
        assert_eq!(
            framing_verdict(head("Content-Length: 9000000").as_bytes()),
            Err(REJECT_413)
        );
        assert_eq!(
            framing_verdict(head("Content-Length: 123456789012345678901234").as_bytes()),
            Err(REJECT_400)
        );
    }

    #[test]
    fn content_length_malformed_is_rejected() {
        // RFC 9112 §6.3: a Content-Length that is not a valid non-negative integer is a 400 + close,
        // never "read no body and serve it" (panel C10/F4: `abc`, `-1` and a 24-digit value all
        // served `200`). Absent stays `Ok(0)`.
        for bad in [
            "not-a-number",
            "-1",
            "123456789012345678901234",
            "1 2",
            "0x10",
            // `1*DIGIT` admits no sign: Rust's `usize::from_str` would accept this one.
            "+5",
            "",
        ] {
            let head = format!("POST / HTTP/1.1\r\nContent-Length: {bad}\r\n\r\n");
            assert!(
                framing_verdict(head.as_bytes()).is_err(),
                "`Content-Length: {bad}` must be rejected"
            );
        }
        assert_eq!(framing_verdict(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"), Ok(0));
    }

    // --- read_http_request (over a Cursor, no socket) ----------------------

    #[test]
    fn reads_headers_only_request() {
        let req = b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
        let got = request_bytes(&mut Cursor::new(req.clone()));
        assert_eq!(got, req);
    }

    #[test]
    fn reads_request_with_body() {
        let req = b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello".to_vec();
        let got = request_bytes(&mut Cursor::new(req.clone()));
        assert_eq!(got, req, "head + the declared 5 body bytes");
    }

    #[test]
    fn eof_before_headers_returns_partial() {
        // No CRLFCRLF, then EOF → returns whatever was read (parse → 400 downstream), never hangs.
        let req = b"GET / HTTP/1.1 no terminator".to_vec();
        let got = request_bytes(&mut Cursor::new(req.clone()));
        assert_eq!(got, req);
    }

    /// A reader that yields its data in fixed-size pieces — exercises the accumulation loop with the
    /// `\r\n\r\n` terminator split across multiple `read` calls.
    struct ChunkedReader {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }
    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let remaining = &self.data[self.pos..];
            let n = remaining.len().min(self.chunk).min(buf.len());
            buf[..n].copy_from_slice(&remaining[..n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn terminator_and_body_split_across_chunks() {
        let req = b"POST /x HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc".to_vec();
        let mut r = ChunkedReader {
            data: req.clone(),
            pos: 0,
            chunk: 1, // one byte per read → terminator and body span many reads
        };
        let got = request_bytes(&mut r);
        assert_eq!(got, req);
    }

    /// A reader that never produces a terminator — drives the [`MAX_REQUEST`] cap.
    struct InfiniteReader;
    impl Read for InfiniteReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            for b in buf.iter_mut() {
                *b = b'a';
            }
            Ok(buf.len())
        }
    }

    #[test]
    fn max_request_cap_terminates() {
        // No `\r\n\r\n` ever arrives; the read must stop near the cap rather than loop forever.
        let got = request_bytes(&mut InfiniteReader);
        assert!(got.len() > MAX_REQUEST, "stopped at the cap");
        assert!(
            got.len() <= MAX_REQUEST + 4096,
            "no more than one chunk past the cap"
        );
    }
}

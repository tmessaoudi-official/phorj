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

/// Read one HTTP/1.1 request from `stream`: everything up to and including `\r\n\r\n`, then the
/// `Content-Length` body (0 if absent). Capped at [`MAX_REQUEST`]. Framing only — no semantic
/// validation; a partial/malformed buffer flows to the program's `parse_request`, which returns
/// `null` and yields a 400. Generic over [`Read`] so the framing is unit-testable over a `Cursor`
/// (P1-d) without binding a socket.
pub(super) fn read_http_request<R: Read>(stream: &mut R) -> io::Result<Vec<u8>> {
    const SEP: &[u8] = b"\r\n\r\n";
    let mut buf = Vec::new();
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
            return Ok(buf);
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(buf); // EOF before full headers → partial (parse → 400)
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let want = head_end
        .saturating_add(parse_content_length(&buf[..head_end]))
        .min(MAX_REQUEST);
    while buf.len() < want {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
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

/// Parse the `Content-Length` header from a request head (0 if absent or unparseable).
fn parse_content_length(head: &[u8]) -> usize {
    let text = String::from_utf8_lossy(head);
    for line in text.split("\r\n") {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse().unwrap_or(0);
            }
        }
    }
    0
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
        assert_eq!(
            parse_content_length(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"),
            0
        );
    }

    #[test]
    fn content_length_present_is_parsed() {
        assert_eq!(
            parse_content_length(b"POST / HTTP/1.1\r\nContent-Length: 42\r\n\r\n"),
            42
        );
    }

    #[test]
    fn content_length_is_case_insensitive_and_trims() {
        assert_eq!(
            parse_content_length(b"POST / HTTP/1.1\r\ncOnTeNt-LeNgTh:   7  \r\n\r\n"),
            7
        );
    }

    #[test]
    fn content_length_malformed_is_zero() {
        // Non-numeric value parses to 0 (framing reads no body; the program's parser handles it).
        assert_eq!(
            parse_content_length(b"POST / HTTP/1.1\r\nContent-Length: not-a-number\r\n\r\n"),
            0
        );
    }

    // --- read_http_request (over a Cursor, no socket) ----------------------

    #[test]
    fn reads_headers_only_request() {
        let req = b"GET / HTTP/1.1\r\nHost: x\r\n\r\n".to_vec();
        let got = read_http_request(&mut Cursor::new(req.clone())).unwrap();
        assert_eq!(got, req);
    }

    #[test]
    fn reads_request_with_body() {
        let req = b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello".to_vec();
        let got = read_http_request(&mut Cursor::new(req.clone())).unwrap();
        assert_eq!(got, req, "head + the declared 5 body bytes");
    }

    #[test]
    fn eof_before_headers_returns_partial() {
        // No CRLFCRLF, then EOF → returns whatever was read (parse → 400 downstream), never hangs.
        let req = b"GET / HTTP/1.1 no terminator".to_vec();
        let got = read_http_request(&mut Cursor::new(req.clone())).unwrap();
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
        let got = read_http_request(&mut r).unwrap();
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
        let got = read_http_request(&mut InfiniteReader).unwrap();
        assert!(got.len() > MAX_REQUEST, "stopped at the cap");
        assert!(
            got.len() <= MAX_REQUEST + 4096,
            "no more than one chunk past the cap"
        );
    }
}

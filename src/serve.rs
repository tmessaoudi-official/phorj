//! M6 W3 — HTTP serve runtime. The ONE place sockets + wall-clock non-determinism live, kept
//! deliberately OUTSIDE the byte-identity spine: `tests/differential.rs` never imports this module —
//! its conformance is covered by `tests/serve.rs` over a deterministic in-memory [`Transport`].
//!
//! The portable unit stays `handle(Request) -> Response` (W1) *inside* the served program; the
//! runtime only shuttles raw bytes to a single Phorge entry **`respond(bytes) -> bytes`** ([`SERVE_ENTRY`])
//! and writes the result back. HTTP/1.1, `Connection: close`, one request per accepted connection.
//!
//! Single-threaded by FORCE: the `Rc`-shared heap (P5a) makes `Value` non-`Send`, so a thread pool
//! is impossible; real concurrency arrives with M6 green-threads under this unchanged contract.
use crate::ast::Program;
use crate::interpreter::call_named;
use crate::value::Value;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;

/// The default Phorge entry the runtime calls per request: `respond(bytes) -> bytes`.
pub const SERVE_ENTRY: &str = "respond";

/// Seam between the serve loop and the world. [`TcpTransport`] is the real socket; `tests/serve.rs`
/// swaps an in-memory transport (the env-update HTTP-fixture-seam pattern) so the loop needs no port
/// and stays deterministic.
pub trait Transport {
    /// Block for the next raw request, or `Ok(None)` when the source is exhausted (shutdown).
    fn recv(&mut self) -> io::Result<Option<Vec<u8>>>;
    /// Write the raw response for the request just `recv`'d, then end that exchange.
    fn send(&mut self, response: &[u8]) -> io::Result<()>;
}

/// Serve requests from `transport`, routing each raw buffer through the program's
/// `respond(bytes) -> bytes`. A fault on one request degrades to a 500 (logged to stderr); the loop
/// continues. Returns when the transport reports exhaustion.
pub fn serve<T: Transport>(program: &Program, transport: &mut T) -> io::Result<()> {
    while let Some(raw) = transport.recv()? {
        let response = respond_once(program, &raw);
        transport.send(&response)?;
    }
    Ok(())
}

/// Invoke `respond(bytes) -> bytes` once. Any captured stdout (a handler calling `console.println`)
/// is treated as a server log line and written to stderr, keeping the HTTP response body clean.
/// A non-`bytes` return or a runtime fault degrades to a 500 — never a panic (EV-7).
fn respond_once(program: &Program, raw: &[u8]) -> Vec<u8> {
    let arg = Value::Bytes(Rc::new(raw.to_vec()));
    match call_named(program, SERVE_ENTRY, vec![arg]) {
        Ok((Value::Bytes(b), out)) => {
            if !out.is_empty() {
                eprint!("{out}");
            }
            b.as_ref().clone()
        }
        Ok((other, _)) => {
            eprintln!(
                "serve: `{SERVE_ENTRY}` returned {}, expected bytes",
                other.type_name()
            );
            http_500()
        }
        Err(e) => {
            eprintln!("serve: request failed: {e}");
            http_500()
        }
    }
}

/// A minimal, well-formed `500 Internal Server Error` response (`Connection: close`).
fn http_500() -> Vec<u8> {
    let body = b"internal server error";
    let head = format!(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: text/plain\r\n\r\n",
        body.len()
    );
    head.into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
}

/// Production transport: a single-threaded `TcpListener`, one request per accepted connection
/// (`Connection: close`). `recv` *frames* the request (reads up to `\r\n\r\n`, then `Content-Length`
/// bytes) — framing only; the program's `parse_request` does the semantic parse.
pub struct TcpTransport {
    listener: TcpListener,
    current: Option<TcpStream>,
}

impl TcpTransport {
    /// Bind a listener (e.g. `"127.0.0.1:8080"`, or `":0"`-style `"127.0.0.1:0"` for an ephemeral port).
    pub fn bind(addr: &str) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr)?,
            current: None,
        })
    }
    /// The actually-bound address (useful when binding to port 0).
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }
}

impl Transport for TcpTransport {
    fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        let (mut stream, _peer) = self.listener.accept()?;
        let raw = read_http_request(&mut stream)?;
        self.current = Some(stream);
        Ok(Some(raw))
    }
    fn send(&mut self, response: &[u8]) -> io::Result<()> {
        if let Some(mut stream) = self.current.take() {
            stream.write_all(response)?;
            stream.flush()?;
        }
        Ok(()) // dropping the stream closes the connection (Connection: close)
    }
}

/// Bind `addr` and serve until killed — the blocking accept-loop `phg serve` calls (W4).
pub fn serve_tcp(program: &Program, addr: &str) -> io::Result<()> {
    let mut t = TcpTransport::bind(addr)?;
    eprintln!("phg serve: listening on http://{}", t.local_addr()?);
    serve(program, &mut t)
}

/// Cap a single request at 8 MiB — keeps a hostile or runaway client from exhausting memory (EV-7).
const MAX_REQUEST: usize = 8 * 1024 * 1024;

/// Read one HTTP/1.1 request from `stream`: everything up to and including `\r\n\r\n`, then the
/// `Content-Length` body (0 if absent). Capped at [`MAX_REQUEST`]. Framing only — no semantic
/// validation; a partial/malformed buffer flows to the program's `parse_request`, which returns
/// `null` and yields a 400.
fn read_http_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
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

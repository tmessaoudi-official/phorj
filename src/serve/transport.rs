//! Serve — TCP transport, worker pool, HTTP request framing, keep-alive.

use super::*;
// Wire framing lives in the sibling `framing` module (Invariant 13 carve-out, S3.5): same functions,
// same behaviour, moved so this file could take TLS without growing past its ratchet.
use super::framing::{
    read_http_request, request_wants_keepalive, response_keeps_alive, Framed, MAX_REQUESTS_PER_CONN,
};

/// Production transport: a single-threaded `TcpListener`, one request per accepted connection
/// (`Connection: close`). `recv` *frames* the request (reads up to `\r\n\r\n`, then `Content-Length`
/// bytes) — framing only; the program's `parse_request` does the semantic parse.
pub struct TcpTransport {
    listener: TcpListener,
    current: Option<super::tls::Conn>,
    /// Bytes read past the previous request on the kept-alive socket (a pipelined next request —
    /// round-4 safety F4); consumed by the next `recv`, cleared when the connection drops.
    carry: Vec<u8>,
    /// S3.5: the TLS server configuration, when the program's `ServeConfig` asked for HTTPS. `None`
    /// = plain HTTP. Set once before the loop starts, exactly like the timeout and shutdown flag.
    tls: Option<super::tls::TlsServer>,
    /// Per-connection read/write timeout (slowloris guard, GA blocker B4). `None` = no timeout.
    timeout: Option<Duration>,
    /// S4.1 keep-alive: whether the request just `recv`'d asked to keep the connection open (decided in
    /// `recv`, consumed in `send` together with the response's own `Connection` header).
    req_wants_keepalive: bool,
    /// Requests already served on the currently-kept-alive socket (capped at [`MAX_REQUESTS_PER_CONN`]).
    served_on_current: usize,
    /// S4.2 graceful shutdown: when set (by the signal handler), `recv` stops accepting and returns
    /// `Ok(None)`, which the `serve` loop treats as clean exhaustion. `None` ⇒ never shuts down (the
    /// pre-S4.2 blocking-accept behaviour). A single-threaded server has ≤1 in-flight request (already
    /// sent before the next `recv`), so "drain" is automatic.
    shutdown: Option<Arc<AtomicBool>>,
}

impl TcpTransport {
    /// Bind a listener (e.g. `"127.0.0.1:8080"`, or `":0"`-style `"127.0.0.1:0"` for an ephemeral port).
    pub fn bind(addr: &str) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr)?,
            tls: None,
            current: None,
            carry: Vec::new(),
            timeout: None,
            req_wants_keepalive: false,
            served_on_current: 0,
            shutdown: None,
        })
    }
    /// Set the per-connection read/write timeout (GA blocker B4 — bounds a slow/idle client on the
    /// single-threaded server). `None` disables it (a slow client may then hold a connection
    /// indefinitely — only appropriate for trusted/loopback use).
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }
    /// Set the graceful-shutdown flag (S4.2). When it flips, `recv` stops accepting and returns
    /// `Ok(None)` (clean exhaustion). When `None` (the default), the server accepts forever.
    /// S3.5: terminate TLS on every accepted connection. Must be set before serving begins.
    pub fn set_tls(&mut self, tls: Option<super::tls::TlsServer>) {
        self.tls = tls;
    }

    pub fn set_shutdown(&mut self, shutdown: Arc<AtomicBool>) {
        self.shutdown = Some(shutdown);
    }
    /// The actually-bound address (useful when binding to port 0).
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }
}

impl Transport for TcpTransport {
    fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        // S4.1: first try the kept-alive socket from the previous exchange (if `send` kept it). A
        // subsequent request reuses the connection; EOF/timeout on it just drops it and we accept a new
        // one — so an idle keep-alive client can never wedge the single-threaded server (it is reaped by
        // the read timeout, which is why keep-alive is only kept when a timeout is configured).
        if let Some(mut stream) = self.current.take() {
            match read_http_request(&mut stream, &mut self.carry) {
                // RFC 9112 §6.3 / §5.1 (panel C10/F4, round-4 F5): a request whose framing cannot be
                // honoured gets the fixed reject and the connection is CLOSED — its boundary is
                // unknowable, so it cannot be reused.
                Ok(Framed::Reject(resp)) => {
                    if let Err(e) = stream.write_all(resp).and_then(|()| stream.flush()) {
                        eprintln!(
                            "serve: could not send the framing reject (connection dropped): {e}"
                        );
                    }
                    self.carry.clear();
                }
                Ok(Framed::Request(raw)) if !raw.is_empty() => {
                    self.req_wants_keepalive = request_wants_keepalive(&raw);
                    self.current = Some(stream);
                    return Ok(Some(raw));
                }
                // Empty (client closed) or a read error (idle timeout / reset) → this connection is
                // done; fall through to accept a fresh one.
                _ => self.carry.clear(),
            }
        }
        // S4.2: when a shutdown flag is present, poll-accept (non-blocking listener + sleep) so the loop
        // can notice the flag and return `Ok(None)` for a clean shutdown — std has no accept-timeout. The
        // listener stays non-blocking only while a flag is set; accepted streams are restored to blocking
        // so their reads use the normal timeout path. With no flag, accept blocks exactly as pre-S4.2.
        let polling = self.shutdown.is_some();
        let _ = self.listener.set_nonblocking(polling);
        // Accept connections until one yields a request. An `accept()` error propagates to the serve
        // loop's circuit breaker (it decides if the listener is unrecoverable). A per-connection read
        // error — a read timeout from a slow/idle client (B4), or a reset mid-headers — is logged and
        // the *next* connection is accepted, so one bad client cannot wedge the single-threaded
        // server (B3 + B4 together).
        loop {
            if let Some(flag) = &self.shutdown {
                if flag.load(Ordering::SeqCst) {
                    return Ok(None); // graceful shutdown — the serve loop exits cleanly
                }
            }
            let (stream, _peer) = match self.listener.accept() {
                Ok(pair) => pair,
                Err(e) if polling && e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL_INTERVAL);
                    continue;
                }
                Err(e) => return Err(e),
            };
            let _ = stream.set_nonblocking(false); // blocking reads (timeout-bounded) for this conn
            if let Some(t) = self.timeout {
                // Best-effort: a platform that rejects the timeout must not crash the server.
                let _ = stream.set_read_timeout(Some(t));
                let _ = stream.set_write_timeout(Some(t));
            }
            // S3.5: wrap AFTER blocking mode and the timeouts are set on the raw socket — rustls
            // fails outright on a non-blocking one, and running the handshake through these same
            // timeouts is what bounds a client that connects and then stalls mid-handshake.
            let mut stream = match super::tls::Conn::accept(stream, self.tls.as_ref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("serve: dropping connection (tls setup): {e}");
                    continue;
                }
            };
            self.carry.clear();
            match read_http_request(&mut stream, &mut self.carry) {
                // RFC 9112 §6.3 / §5.1: the fixed reject + close, then accept the next connection.
                Ok(Framed::Reject(resp)) => {
                    if let Err(e) = stream.write_all(resp).and_then(|()| stream.flush()) {
                        eprintln!(
                            "serve: could not send the framing reject (connection dropped): {e}"
                        );
                    }
                    self.carry.clear();
                }
                Ok(Framed::Request(raw)) => {
                    self.req_wants_keepalive = request_wants_keepalive(&raw);
                    self.served_on_current = 0;
                    self.current = Some(stream);
                    return Ok(Some(raw));
                }
                Err(e) => {
                    eprintln!("serve: dropping connection (read error): {e}");
                    // loop: accept the next connection
                }
            }
        }
    }
    fn send(&mut self, response: &[u8]) -> io::Result<()> {
        if let Some(mut stream) = self.current.take() {
            stream.write_all(response)?;
            stream.flush()?;
            // S4.1: keep the socket for the next request only when a timeout is configured (so an idle
            // client is reaped, never wedging the single-threaded server), the request and response both
            // permit it, and we are under the per-connection cap. Otherwise the stream drops here →
            // `Connection: close` (verbatim pre-S4.1 behaviour when keep-alive does not apply).
            self.served_on_current += 1;
            let keep = self.timeout.is_some()
                && self.served_on_current < MAX_REQUESTS_PER_CONN
                && self.req_wants_keepalive
                && response_keeps_alive(response);
            if keep {
                self.current = Some(stream);
            } else {
                self.carry.clear(); // the socket is gone; so is anything pipelined on it
            }
        }
        Ok(())
    }
}

/// Bind `addr` and serve until killed — the blocking accept-loop `phg serve` calls (W4/W3). `timeout`
/// is the per-connection read/write timeout (GA blocker B4); `None` disables it. `workers` is the
/// request concurrency: `<= 1` keeps the single-threaded path (verbatim pre-W3 behaviour); `> 1`
/// runs an OS-thread pool, one request per worker thread, each with its own `Rc` `Value` heap
/// (`ast::Program` is `Send + Sync` and values never cross threads — M6 W3 design).
pub fn serve_tcp(
    factory: HandlerFactory,
    addr: &str,
    timeout: Option<Duration>,
    profile: crate::profile::Profile,
    workers: usize,
    tls: Option<super::tls::TlsServer>,
) -> io::Result<()> {
    // M-DX S0: the build profile is the source of truth; serve's fault pages are a Dev-only
    // side-channel (they leak traces/source). Derive the leaf `dev` bool here at the CLI boundary.
    let dev = profile.is_dev();
    // S4.2: SIGINT/SIGTERM → graceful shutdown (drain in-flight, exit 0). Installed once for either path.
    let shutdown = install_shutdown_handler();
    let scheme = scheme(tls.as_ref());
    if workers <= 1 {
        let mut t = TcpTransport::bind(addr)?;
        t.set_timeout(timeout);
        t.set_tls(tls);
        t.set_shutdown(Arc::clone(&shutdown));
        eprintln!("phg serve: listening on {scheme}://{}", t.local_addr()?);
        serve_banner(timeout, dev, 1);
        return serve(&factory, &mut t, dev);
    }
    serve_tcp_pool(factory, addr, timeout, dev, workers, shutdown, tls)
}

/// The startup banner (bind/timeout/workers + the untrusted-network note).
fn serve_banner(timeout: Option<Duration>, dev: bool, workers: usize) {
    if dev {
        eprintln!(
            "phg serve: --dev — rich HTML error pages on fault (DEV ONLY, leaks traces/source)"
        );
    }
    let conc = if workers <= 1 {
        "single-threaded".to_string()
    } else {
        format!("{workers} workers")
    };
    match timeout {
        Some(d) => eprintln!(
            "phg serve: per-connection timeout {}s; HTTP/1.1 keep-alive; {conc} — bind 127.0.0.1 on untrusted networks",
            d.as_secs()
        ),
        None => eprintln!(
            "phg serve: no connection timeout (pass --timeout to enable keep-alive); {conc} — bind 127.0.0.1 on untrusted networks"
        ),
    }
}

/// The W3 concurrent server: a fixed pool of `workers` threads, each handling one request at a time
/// with its own heap. The main thread `accept()`s and hands each `TcpStream` to the pool over a
/// **bounded** channel (capacity = `workers`) — when every worker is busy and the queue is full,
/// `accept` blocks, giving natural backpressure (no unbounded spawn, no dropped connection). The
/// immutable program is shared via `Arc` (`Program: Send + Sync`); a worker panic is caught so one bad
/// request never kills a worker.
fn serve_tcp_pool(
    factory: HandlerFactory,
    addr: &str,
    timeout: Option<Duration>,
    dev: bool,
    workers: usize,
    shutdown: Arc<AtomicBool>,
    tls: Option<super::tls::TlsServer>,
) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!(
        "phg serve: listening on {}://{}",
        scheme(tls.as_ref()),
        listener.local_addr()?
    );
    serve_banner(timeout, dev, workers);
    serve_pool_with(
        listener,
        factory,
        timeout,
        dev,
        workers,
        Some(shutdown),
        tls,
    )
}

/// The URL scheme the bind line announces. Derived from whether a TLS server was actually BUILT, not
/// from whether the config mentioned a certificate — so the banner cannot claim `https` for a server
/// that is about to answer in the clear.
fn scheme(tls: Option<&super::tls::TlsServer>) -> &'static str {
    if tls.is_some() {
        "https"
    } else {
        "http"
    }
}

/// The pool accept-loop over an already-bound `listener` — the testable seam (a test binds
/// `127.0.0.1:0`, reads `local_addr`, then drives this with real concurrent clients). `workers >= 1`.
/// Runs until killed (no shutdown flag); for the graceful-shutdown path use [`serve_pool_with`].
pub fn serve_pool(
    listener: TcpListener,
    factory: HandlerFactory,
    timeout: Option<Duration>,
    dev: bool,
    workers: usize,
) -> io::Result<()> {
    serve_pool_with(listener, factory, timeout, dev, workers, None, None)
}

/// [`serve_pool`] plus the S4.2 graceful-shutdown flag. When the flag flips, the accept loop stops,
/// the work channel is dropped (so each worker finishes its in-flight connection then exits as
/// `recv` errors), and every worker is **joined** before returning — a clean drain, no abrupt cut.
/// With `shutdown = None` the loop runs forever (blocking accept, verbatim pre-S4.2). When a flag is
/// present the listener is non-blocking and the loop polls it every [`ACCEPT_POLL_INTERVAL`].
pub fn serve_pool_with(
    listener: TcpListener,
    factory: HandlerFactory,
    timeout: Option<Duration>,
    dev: bool,
    workers: usize,
    shutdown: Option<Arc<AtomicBool>>,
    tls: Option<super::tls::TlsServer>,
) -> io::Result<()> {
    // The factory is `Send + Sync`; share it across workers, each of which calls it once to build its
    // own (non-`Send`) per-thread handler — the VM handler compiles its own `Rc`-bearing program there.
    let factory = Arc::new(factory);
    // S3.5: the rustls config is `Send + Sync` behind an `Arc`, so every worker terminates TLS with
    // the same certificate. The RAW `TcpStream` still crosses the channel — the handshake happens in
    // the worker, never in the accept loop, so a client that stalls mid-handshake cannot serialize
    // `accept()` and starve every other connection.
    let tls = tls.map(Arc::new);
    let (tx, rx) = sync_channel::<TcpStream>(workers);
    let rx = Arc::new(Mutex::new(rx));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let factory = Arc::clone(&factory);
        let rx = Arc::clone(&rx);
        let tls = tls.clone();
        handles.push(std::thread::spawn(move || {
            worker_loop(&factory, &rx, timeout, dev, tls.as_deref());
        }));
    }

    let polling = shutdown.is_some();
    let _ = listener.set_nonblocking(polling);
    let mut consecutive_errors = 0usize;
    let result = loop {
        if let Some(flag) = &shutdown {
            if flag.load(Ordering::SeqCst) {
                break Ok(()); // graceful shutdown → drain + join below
            }
        }
        match listener.accept() {
            Ok((stream, _peer)) => {
                consecutive_errors = 0;
                let _ = stream.set_nonblocking(false); // workers do blocking, timeout-bounded reads
                                                       // Blocks when the bounded queue is full → backpressure. Errors only if every worker
                                                       // has gone (all receivers dropped) — then the pool is dead and we stop.
                if tx.send(stream).is_err() {
                    break Ok(());
                }
            }
            Err(e) if polling && e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(e) => {
                consecutive_errors += 1;
                eprintln!("serve: accept error (skipped): {e}");
                if consecutive_errors >= MAX_CONSECUTIVE_TRANSPORT_ERRORS {
                    eprintln!(
                        "serve: {consecutive_errors} consecutive accept errors — shutting down"
                    );
                    break Err(e);
                }
            }
        }
    };
    // Drain: dropping the sender closes the channel; each worker finishes the connection it is on, then
    // its next `recv` errors and it returns. Join them so in-flight requests complete before we exit.
    drop(tx);
    for h in handles {
        let _ = h.join();
    }
    result
}

/// One pool worker: pull a connection, frame+handle+write it with this thread's own heap, repeat.
/// `respond_once` already degrades a fault to a 500 (never panics, EV-7); the `catch_unwind` is a
/// belt-and-suspenders guard so an unexpected interpreter panic (e.g. a stack-depth edge) recovers
/// the worker instead of silently shrinking the pool.
fn worker_loop(
    factory: &HandlerFactory,
    rx: &Mutex<std::sync::mpsc::Receiver<TcpStream>>,
    timeout: Option<Duration>,
    dev: bool,
    tls: Option<&super::tls::TlsServer>,
) {
    // Build this worker's own handler once (its own compiled program for the VM backend), reused for
    // every connection + request this thread handles — the whole point of compiling per worker.
    let mut handler = factory();
    loop {
        // Hold the lock only to dequeue; release it before handling so workers run concurrently.
        let stream = {
            let guard = rx.lock().unwrap_or_else(|e| e.into_inner());
            guard.recv()
        };
        let Ok(stream) = stream else {
            return; // channel closed → the server is shutting down
        };
        if let Some(t) = timeout {
            let _ = stream.set_read_timeout(Some(t));
            let _ = stream.set_write_timeout(Some(t));
        }
        // S3.5: wrap AFTER the timeouts land on the raw socket (see `Conn::accept`). A TLS setup
        // failure drops this connection only — the worker stays in the pool for the next one.
        let mut stream = match super::tls::Conn::accept(stream, tls) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("serve: dropping connection (tls setup): {e}");
                continue;
            }
        };
        // S4.1: serve multiple requests on this socket when keep-alive applies. Keep-alive is only
        // entered when a timeout is configured, so an idle client is reaped by the read timeout and can
        // never pin a worker (with no timeout this serves exactly one request, verbatim pre-S4.1).
        let keepalive = timeout.is_some();
        let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut served = 0usize;
            // Bytes past the previous request on this socket (pipelining) — each loop turn sees
            // exactly ONE request, so the keep-alive and empty-buffer checks below stay per-request.
            let mut carry: Vec<u8> = Vec::new();
            loop {
                match read_http_request(&mut stream, &mut carry) {
                    // Empty buffer = the client closed (EOF before any bytes) — only meaningful on a
                    // kept-alive socket; on a fresh one it flows to `parse_request` → 400 (served == 0).
                    Ok(Framed::Request(raw)) if served > 0 && raw.is_empty() => break,
                    // RFC 9112 §6.3 / §5.1: the fixed reject + close, never a handler call.
                    Ok(Framed::Reject(resp)) => {
                        if let Err(e) = stream.write_all(resp).and_then(|()| stream.flush()) {
                            eprintln!("serve: could not send the framing reject (connection dropped): {e}");
                        }
                        break;
                    }
                    Ok(Framed::Request(raw)) => {
                        let response = respond_once(&mut handler, &raw, dev);
                        if let Err(e) = stream.write_all(&response).and_then(|()| stream.flush()) {
                            eprintln!("serve: send failed (connection dropped): {e}");
                            break;
                        }
                        served += 1;
                        if !(keepalive
                            && served < MAX_REQUESTS_PER_CONN
                            && request_wants_keepalive(&raw)
                            && response_keeps_alive(&response))
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        // A read error after ≥1 request is the expected idle keep-alive timeout (not
                        // worth logging); on the first read it is a genuine dropped/slow connection.
                        if served == 0 {
                            eprintln!("serve: dropping connection (read error): {e}");
                        }
                        break;
                    }
                }
            }
        }));
        if handled.is_err() {
            eprintln!("serve: worker recovered from a panic on one request");
        }
        // `stream` drops here → connection closes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_error_page_escapes_and_includes_frames_and_request() {
        let diag =
            crate::diagnostic::Diagnostic::runtime_at_line("boom <script>", 3).with_frames(vec![
                crate::diagnostic::Frame {
                    function: "web".into(),
                    file: None,
                    line: 3,
                    col: 0,
                },
            ]);
        let page = dev_error_page(&diag, b"GET /x?<a> HTTP/1.1\r\nHost: a\r\n\r\nBODY");
        let s = String::from_utf8(page).unwrap();
        assert!(s.contains("500 Internal Server Error"), "{s}");
        assert!(s.contains("text/html"), "{s}");
        assert!(s.contains("&lt;script&gt;"), "message must be escaped: {s}");
        assert!(!s.contains("<script>"), "no raw script tag: {s}");
        assert!(s.contains("web"), "frame shown: {s}");
        assert!(
            s.contains("/x?&lt;a&gt;"),
            "request line shown + escaped: {s}"
        );
        assert!(
            !s.contains("BODY"),
            "request body is not included (head only): {s}"
        );
    }
}

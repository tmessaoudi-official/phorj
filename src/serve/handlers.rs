//! Serve — request handlers: interpreter/VM handler factories, the serve loop core,
//! response shaping, dev error pages.

use super::*;

/// Install the graceful-shutdown signal handler (S4.2) and return the flag it flips. With the
/// `signals` feature, SIGINT (Ctrl-C) and SIGTERM set the flag; the accept loops then stop taking new
/// connections, drain in-flight work, and exit cleanly. Without the feature (the WASM playground), the
/// flag is never set and the server runs until killed — verbatim pre-S4.2.
///
/// DEC-487: the registration itself moved to [`crate::shutdown`], because `ctrlc::set_handler` may be
/// called only ONCE per process and `Time.sleep` is now a second interested party. Both observe the
/// same flag, so one Ctrl-C wakes a sleeping worker AND stops the accept loop.
#[must_use]
pub fn install_shutdown_handler() -> Arc<AtomicBool> {
    crate::shutdown::flag()
}

/// The checker's reified-operand side-table (`expr span → Ty`), threaded into [`compile_with`] so the
/// VM specializes arithmetic operands exactly as the byte-identical `phg run` path does (Invariant 6).
pub type Reified = std::collections::HashMap<usize, crate::types::Ty>;

/// A per-thread request handler: given the raw request bytes, invoke the served program's
/// registered web handler, returning its value + captured stdout (or a runtime fault). It is
/// **not** `Send` — the VM handler owns an `Rc`-bearing compiled [`BytecodeProgram`], and values never
/// cross threads — so exactly one is built **per worker thread** (never shared).
pub type Handler = Box<dyn FnMut(&[u8]) -> Result<(Value, String), Diagnostic>>;

/// A `Send + Sync` factory the CLI (or a test) supplies; each worker — and the single-threaded loop —
/// calls it once to build its own [`Handler`]. The VM factory does the per-thread `compile_with`
/// **inside** the produced handler, so no `Rc`-bearing state ever crosses a thread boundary — only the
/// factory itself (which captures the `Send + Sync` checked [`Program`] + [`Reified`] table) does. This
/// is why serve compiles once per worker rather than sharing one bytecode program: a `BytecodeProgram`
/// holds `Rc` class layouts and is not `Send`.
pub type HandlerFactory = Box<dyn Fn() -> Handler + Send + Sync>;

/// Seam between the serve loop and the world. [`TcpTransport`] is the real socket; `tests/serve.rs`
/// swaps an in-memory transport (the env-update HTTP-fixture-seam pattern) so the loop needs no port
/// and stays deterministic.
pub trait Transport {
    /// Block for the next raw request, or `Ok(None)` when the source is exhausted (shutdown).
    fn recv(&mut self) -> io::Result<Option<Vec<u8>>>;
    /// Write the raw response for the request just `recv`'d, then end that exchange.
    fn send(&mut self, response: &[u8]) -> io::Result<()>;
}

/// If the transport reports this many consecutive errors with **no** successful request in between,
/// the listener is treated as unrecoverable and the loop ends. Transient per-connection failures
/// (client resets, slow-client read timeouts) are logged and skipped far below this bound, so one
/// hostile or broken client can never take the server down — GA blocker B3.
pub(super) const MAX_CONSECUTIVE_TRANSPORT_ERRORS: usize = 64;

/// Serve requests from `transport`, routing each raw buffer through the program's
/// registered web handler. **Resilient by design (GA blockers B3/B4):** a fault on one request
/// degrades to a 500, a `send` failure (client reset / broken pipe) is logged and skipped, and a
/// `recv` error (e.g. a transient `accept()`) is logged and retried — only `MAX_CONSECUTIVE_…` recv
/// errors in a row with no progress ends the loop. Returns `Ok` when the transport reports
/// exhaustion (`recv` → `Ok(None)`).
pub fn serve<T: Transport>(
    factory: &HandlerFactory,
    transport: &mut T,
    dev: bool,
) -> io::Result<()> {
    // Single-threaded loop: build this loop's one handler once, reuse it for every request.
    let mut handler = factory();
    let mut consecutive_errors = 0usize;
    loop {
        match transport.recv() {
            Ok(Some(raw)) => {
                consecutive_errors = 0;
                let response = respond_once(&mut handler, &raw, dev);
                if let Err(e) = transport.send(&response) {
                    // One client's broken pipe / reset must not end the server.
                    eprintln!("serve: send failed (connection dropped): {e}");
                }
            }
            Ok(None) => return Ok(()), // transport exhausted → graceful shutdown
            Err(e) => {
                consecutive_errors += 1;
                eprintln!("serve: connection error (skipped): {e}");
                if consecutive_errors >= MAX_CONSECUTIVE_TRANSPORT_ERRORS {
                    eprintln!(
                        "serve: {consecutive_errors} consecutive transport errors — listener \
                         appears unrecoverable, shutting down"
                    );
                    return Err(e);
                }
            }
        }
    }
}

/// Invoke the registered handler once. Any captured stdout (a handler calling `Output.printLine`)
/// goes to the server's real STDOUT — `Output.*` is ALWAYS stdout (DEC-220 removed the old
/// serve-only Output→stderr "log" redirect; leveled server logging is now `Core.Log` → stderr, and
/// the browser body comes from the returned `Response`). The stdout write's flush error is swallowed
/// (a closed/redirected stdout is an ambient condition, not a program fault — same resilience the
/// `send failed` path above uses; mirrors `Core.Log`'s swallowed stderr write). A non-`bytes` return
/// or a runtime fault degrades to a 500 — never a panic (EV-7).
pub(super) fn respond_once(handler: &mut Handler, raw: &[u8], dev: bool) -> Vec<u8> {
    // DEC-282 site mode: an exact static-file match under the docroot short-circuits the program
    // (one intercept point covers the single-thread, pool, and keep-alive paths alike). Unset
    // outside `phg serve <DIR>` — zero cost for handler-mode serves and the in-memory test
    // transport.
    if let Some(root) = super::docroot() {
        if let Some(resp) = super::static_files::try_static(root, raw) {
            return resp;
        }
    }
    match handler(raw) {
        Ok((Value::Bytes(b), out)) => {
            if !out.is_empty() {
                print!("{out}");
                let _ = io::stdout().flush();
            }
            b.as_ref().clone()
        }
        Ok((other, _)) => {
            eprintln!(
                "serve: the web handler returned {}, expected bytes",
                other.type_name()
            );
            http_500()
        }
        Err(e) => {
            eprintln!("serve: request failed: {e}");
            // Dev mode renders a rich HTML error page (the trace + request context). Production never
            // leaks a trace/source — a bare generic 500 (a security rule, error-handling slice 1).
            if dev {
                dev_error_page(&e, raw)
            } else {
                http_500()
            }
        }
    }
}

/// HTML-escape `s` with the same 5-char table as `Core.Html` (PHP `htmlspecialchars(_, ENT_QUOTES)`),
/// so every value interpolated into the dev error page is XSS-safe by construction.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            other => out.push(other),
        }
    }
    out
}

/// A development-only HTML `500` page for an uncaught handler fault: the fault message, its call
/// stack, and the request's start-line + headers. **Runtime glue** — outside the byte-identity value
/// contract; only reached when `phg serve --dev` is set. Every interpolated value is escaped.
pub(super) fn dev_error_page(diag: &crate::diagnostic::Diagnostic, raw: &[u8]) -> Vec<u8> {
    // The request head (start-line + headers) is everything up to the CRLFCRLF body boundary.
    let head = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(raw, |i| &raw[..i]);
    let req = String::from_utf8_lossy(head);
    let mut frames = String::new();
    for (i, f) in diag.frames.iter().enumerate() {
        let mark = if i == 0 { "→ " } else { "  " };
        let loc = match &f.file {
            Some(p) => format!("{}:{}", p.display(), f.line),
            None => format!("line {}", f.line),
        };
        frames.push_str(&format!("{}{}    {}\n", mark, esc(&f.function), esc(&loc)));
    }
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Phorj — runtime fault</title>\
         <style>body{{font:14px/1.5 ui-monospace,monospace;background:#1e1e2e;color:#cdd6f4;margin:2rem}}\
         h1{{color:#f38ba8}}pre{{background:#181825;padding:1rem;border-radius:8px;overflow:auto}}\
         .req{{color:#a6adc8}}</style></head><body>\
         <h1>Runtime fault</h1><pre>{msg}</pre>\
         <h2>Stack trace (most recent call first)</h2><pre>{frames}</pre>\
         <h2>Request</h2><pre class=\"req\">{req}</pre>\
         <p class=\"req\">phorj serve --dev — this page is shown in development only.</p>\
         </body></html>",
        msg = esc(&diag.to_string()),
        frames = frames,
        req = esc(&req),
    );
    let head = format!(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
        body.len()
    );
    head.into_bytes()
        .into_iter()
        .chain(body.into_bytes())
        .collect()
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

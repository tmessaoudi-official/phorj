//! Serve — request handlers: interpreter/VM handler factories, the serve loop core,
//! response shaping, dev error pages.

use super::*;

/// Install the graceful-shutdown signal handler (S4.2) and return the flag it flips. With the
/// `signals` feature, SIGINT (Ctrl-C) and SIGTERM set the flag; the accept loops then stop taking new
/// connections, drain in-flight work, and exit cleanly. Without the feature (the WASM playground), the
/// flag is never set and the server runs until killed — verbatim pre-S4.2. `ctrlc`'s own `unsafe`
/// signal registration is confined to that crate, so phorj's code stays `#![forbid(unsafe_code)]`.
#[must_use]
pub fn install_shutdown_handler() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    #[cfg(feature = "signals")]
    {
        let f = Arc::clone(&flag);
        // A second Ctrl-C while draining still hard-kills (the handler only fires once; the default
        // disposition is restored after). Errors (handler already set) are non-fatal — log and proceed.
        if let Err(e) = ctrlc::set_handler(move || f.store(true, Ordering::SeqCst)) {
            eprintln!("serve: could not install shutdown handler ({e}); Ctrl-C will hard-kill");
        }
    }
    flag
}

/// The default Phorj entry the runtime calls per request: `respond(bytes) -> bytes`.
pub const SERVE_ENTRY: &str = "respond";

/// The checker's reified-operand side-table (`expr span → Ty`), threaded into [`compile_with`] so the
/// VM specializes arithmetic operands exactly as the byte-identical `phg run` path does (Invariant 6).
pub type Reified = std::collections::HashMap<usize, crate::types::Ty>;

/// A per-thread request handler: given the raw request bytes, invoke the served program's
/// `respond(bytes) -> bytes` entry, returning its value + captured stdout (or a runtime fault). It is
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

/// The tree-walking-interpreter backend (the correctness oracle; `phg serve --tree-walker`). Each
/// request builds a fresh interpreter via [`call_named`] — verbatim the pre-VM serve behaviour.
#[must_use]
pub fn interp_factory(program: std::sync::Arc<Program>) -> HandlerFactory {
    Box::new(move || {
        let program = std::sync::Arc::clone(&program);
        Box::new(move |raw: &[u8]| {
            call_named(
                &program,
                SERVE_ENTRY,
                vec![Value::Bytes(Rc::new(raw.to_vec()))],
            )
        })
    })
}

/// The bytecode-VM backend (the default `phg serve` — faster than the tree-walker, ~2.3× lower
/// end-to-end latency measured on a representative handler; byte-identical by [`Vm::run_entry`] ≡
/// [`call_named`]). Validates the compile + resolves the `respond` entry index
/// **once up front** (surfacing any error before the socket binds), then hands back a factory whose
/// handlers recompile per worker (deterministic ⇒ the same entry index). A fresh [`Vm`] per request
/// re-seeds program statics, matching the interpreter's fresh-per-request state.
///
/// An **overloaded** `respond` is rejected here (the entry is a single fixed `bytes -> bytes`
/// contract) — a degenerate config; `phg serve --tree-walker` still serves it. A missing `respond`
/// is likewise a startup error.
pub fn vm_factory(
    program: std::sync::Arc<Program>,
    reified: std::sync::Arc<Reified>,
) -> Result<HandlerFactory, Diagnostic> {
    use crate::ast::Item;
    let respond_defs = program
        .items
        .iter()
        .filter(|it| matches!(it, Item::Function(f) if f.name == SERVE_ENTRY))
        .count();
    if respond_defs > 1 {
        return Err(Diagnostic::runtime(format!(
            "serve entry `{SERVE_ENTRY}` cannot be overloaded on the VM backend — run \
             `phg serve --tree-walker` to serve an overloaded entry"
        )));
    }
    // The bytecode compiler requires an entry, but a serve/web program legitimately has no `main`
    // (its entry is `respond`). Inject an inert one so it compiles — never invoked, so byte-inert and
    // matching the interpreter's `call_named`, which never runs `main` either. Do it on the shared
    // program the factory captures, so every per-worker recompile sees the same entry.
    let program = if crate::ast::entry_for(&program, crate::ast::EntryRole::Cli).is_none() {
        let mut p = (*program).clone();
        p.items.push(crate::ast::synth_empty_main());
        std::sync::Arc::new(p)
    } else {
        program
    };
    // Compile once up front: validates it (a checked program should always compile) AND resolves the
    // stable free-function index of `respond`. Free functions compile first and bare-named, so a
    // by-name `position` finds the free `respond` (never a method of the same name — those come after).
    let compiled =
        compile_with(&program, &reified).map_err(|e| Diagnostic::runtime(e.to_string()))?;
    let entry = compiled
        .functions
        .iter()
        .position(|f| f.name == SERVE_ENTRY)
        .ok_or_else(|| {
            Diagnostic::runtime(format!(
                "serve needs a `{SERVE_ENTRY}(bytes): bytes` entry (define one, or `import Core.Http` \
                 and a `handle(Request): Response`)"
            ))
        })?;
    Ok(Box::new(move || {
        // Per-worker compile from the shared (Send+Sync) checked program: the resulting Rc-bearing
        // `BytecodeProgram` stays owned by this handler, on this thread. Deterministic ⇒ `entry` holds.
        let compiled: BytecodeProgram =
            compile_with(&program, &reified).expect("serve program compiled cleanly at startup");
        Box::new(move |raw: &[u8]| {
            Vm::new(&compiled).run_entry(entry, vec![Value::Bytes(Rc::new(raw.to_vec()))])
        })
    }))
}

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
/// `respond(bytes) -> bytes`. **Resilient by design (GA blockers B3/B4):** a fault on one request
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

/// Invoke `respond(bytes) -> bytes` once. Any captured stdout (a handler calling `Output.printLine`)
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
                "serve: `{SERVE_ENTRY}` returned {}, expected bytes",
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

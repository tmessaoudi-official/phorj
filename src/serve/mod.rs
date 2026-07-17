//! M6 W3 — HTTP serve runtime. The ONE place sockets + wall-clock non-determinism live, kept
//! deliberately OUTSIDE the byte-identity spine: `tests/differential.rs` never imports this module —
//! its conformance is covered by `tests/serve.rs` over a deterministic in-memory [`Transport`].
//!
//! The portable unit stays `handle(Request) -> Response` (W1) *inside* the served program; the
//! runtime only shuttles raw bytes to a single Phorj entry **`respond(bytes) -> bytes`** ([`SERVE_ENTRY`])
//! and writes the result back. HTTP/1.1 with **keep-alive** (S4.1) when a `--timeout` is configured —
//! a connection is reused until `Connection: close`, the per-connection cap, or the idle timeout; with
//! no timeout it is one request per connection (the idle-socket guard).
//!
//! Concurrency (M6 W3): a bounded OS-thread pool, **one request per worker thread, each with its own
//! `Rc` `Value` heap** — values never cross threads, so the non-`Send` heap is no obstacle (the
//! `ast::Program` shared across workers IS `Send + Sync`). `--workers N` (default = CPU cores);
//! `--workers 1` keeps the original single-threaded path. This supersedes the old "green-threads"
//! plan (which would have been single-core + needs unstable/unsafe std machinery) — see
//! `docs/specs/2026-06-28-m6-w3-serve-concurrency-design.md`.
use crate::ast::Program;
use crate::chunk::BytecodeProgram;
use crate::compiler::compile_with;
use crate::diagnostic::Diagnostic;
use crate::interpreter::call_named;
use crate::value::Value;
use crate::vm::Vm;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often a poll-accept loop wakes to check the shutdown flag (S4.2). std `TcpListener` has no
/// accept-timeout, so the accept loops run non-blocking and sleep this long between empty polls —
/// bounding shutdown latency without busy-spinning.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

mod handlers;
mod static_files;
mod transport;
pub use handlers::*;
pub use static_files::resolve_site_dir;
pub use transport::*;

/// DEC-282 site mode — the process-global docroot (`phg serve <DIR>` sets it once before any
/// worker runs; one serve per process, the same justification as `Core.Process`'s argv global).
/// `None` = handler-only mode (today's `phg serve file.phg`) — the static layer never runs.
static DOCROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Enable site mode: serve static files from `root` ahead of the program entry. First call wins.
pub fn set_docroot(root: std::path::PathBuf) {
    let _ = DOCROOT.set(root);
}

pub(crate) fn docroot() -> Option<&'static std::path::Path> {
    DOCROOT.get().map(std::path::PathBuf::as_path)
}

//! M6 W3 — HTTP serve runtime. The ONE place sockets + wall-clock non-determinism live, kept
//! deliberately OUTSIDE the byte-identity spine: `tests/differential.rs` never imports this module —
//! its conformance is covered by `tests/serve.rs` over a deterministic in-memory [`Transport`].
//!
//! The portable unit is the `(Request) => Response` closure the served program registers with
//! **`Http.serve(cfg, handler)`** (DEC-331 D5); the runtime only shuttles raw bytes to that handler
//! and writes the result back. The named `respond(bytes) -> bytes` entry it used to resolve was
//! RETIRED in S3.3c — a web entry is now a closure FACTORY ([`web_interp_factory`]). HTTP/1.1 with **keep-alive** (S4.1) when a `--timeout` is configured —
//! a connection is reused until `Connection: close`, the per-connection cap, or the idle timeout; with
//! no timeout it is one request per connection (the idle-socket guard).
//!
//! Concurrency (M6 W3): a bounded OS-thread pool, **one request per worker thread, each with its own
//! `Rc` `Value` heap** — values never cross threads, so the non-`Send` heap is no obstacle (the
//! `ast::Program` shared across workers IS `Send + Sync`). `--workers N` (default = CPU cores);
//! `--workers 1` keeps the original single-threaded path. This supersedes the old "green-threads"
//! plan (which would have been single-core + needs unstable/unsafe std machinery) — see
//! `docs/specs/2026-06-28-m6-w3-serve-concurrency-design.md` (deleted spec; upstream git history).
//!
//! ## Why the loop is NOT inside the `Http.serve` native — do not re-derive this
//!
//! The obvious design, written down in full during S3.3 and DISPROVED before any of it was built, was
//! to invert the loop: bind the listener in the parent, run the `Web` entry once per worker so each
//! builds its own closure on its own `Rc` heap, and have the `Http.serve` native then run the accept
//! loop **on the calling thread**, invoking the closure per request. It is an appealing shape — nothing
//! `Rc`-bearing ever crosses a thread — and it is unbuildable, for two reasons found by reading the
//! code rather than reasoning about it:
//!
//! 1. **A native cannot call a method.** The closure returns a `Response`; the loop needs bytes; the
//!    conversion is `.serialize()`, a METHOD. `ClosureInvoker` invokes closures, not methods, so the
//!    native has no way to perform it. Keeping that step in phorj is also what keeps the
//!    400-on-malformed behaviour byte-identical across backends.
//! 2. **The invoker does not outlive the native call.** `NativeEval::HigherOrder` receives a
//!    `&mut ClosureInvoker` supplied by the backend for the duration of ONE dispatch. A native running
//!    an accept loop would have to hold it for the process lifetime.
//!
//! So `Http.serve(cfg, handler)` REGISTERS and RETURNS, and the loop stays exactly where it always was.
//! The `Web` entry is a closure factory and nothing more: no probe/serve mode split, and the transport,
//! keep-alive, static-file interception and `(Value, String)` stdout contract are all untouched. The
//! native is `NativeEval::Pure` precisely because it never invokes the closure — it stores the handler
//! in a THREAD-LOCAL (a closure `Value` is `Rc`-bearing and must never enter a process global) and the
//! config in a process-global `Mutex<Option<ServeCfg>>` of plain `Send` scalars. Two slots,
//! deliberately different kinds.
//!
//! Rescued verbatim in substance from `docs/plans/2026-08-22-s3-3-http-serve.plan.md` §3/§3c before
//! that plan was archived; the register carries the same note on the DEC-331 block.
use crate::ast::Program;
use crate::compiler::compile_with;
use crate::diagnostic::Diagnostic;
use crate::value::Value;
use crate::vm::Vm;
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often a poll-accept loop wakes to check the shutdown flag (S4.2). std `TcpListener` has no
/// accept-timeout, so the accept loops run non-blocking and sleep this long between empty polls —
/// bounding shutdown latency without busy-spinning.
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);

mod framing;
mod handlers;
#[cfg(feature = "http-server-tls")]
mod pem;
mod settings;
mod static_files;
pub mod tls;
mod transport;
mod web_handlers;
/// The registered `Http.ServeConfig` as the serve loop sees it. Re-exported here because `serve` is
/// the module that CONSUMES it — `native::http` is where it is populated, not where it is read.
pub use crate::native::http::serve_register::ServeCfg;
pub use handlers::*;
pub use settings::{
    class_defaults, resolve as resolve_settings, ServeFlags, ServeSettings, DEFAULT_ADDR,
    DEFAULT_TIMEOUT_SECS,
};
pub use static_files::resolve_site_dir;
pub use transport::*;
pub use web_handlers::{web_interp_factory, web_vm_factory};

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

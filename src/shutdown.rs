//! The process's SINGLE signal-shutdown registration point (DEC-204, DEC-487).
//!
//! `ctrlc::set_handler` may be called **once per process** — a second call returns
//! `MultipleHandlers` and the second caller silently gets nothing. Before this module there was
//! exactly one caller (`serve::install_shutdown_handler`), so that was invisible; `Time.sleep` being
//! SIGINT-interruptible (DEC-487) makes a second caller real, and a CLI program that sleeps inside a
//! `phg serve` worker would have been the first to lose. Everything that wants to know "has the
//! process been asked to stop?" goes through [`flag`], which installs at most once.
//!
//! Without the `signals` feature (the WASM playground has no signals) the flag exists but is never
//! set, so callers behave exactly as they did before signals were introduced.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// The shared shutdown flag, installing the signal handler on first call. Idempotent: every caller
/// gets the SAME flag, so `serve`'s accept loop and an in-flight `Time.sleep` observe one signal.
#[must_use]
pub fn flag() -> Arc<AtomicBool> {
    Arc::clone(FLAG.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        #[cfg(feature = "signals")]
        {
            let f = Arc::clone(&flag);
            // A second Ctrl-C still hard-kills: the handler fires once and the default disposition
            // is restored after. An error here is non-fatal — report and carry on uninterruptible.
            if let Err(e) = ctrlc::set_handler(move || f.store(true, Ordering::SeqCst)) {
                eprintln!("phg: could not install shutdown handler ({e}); Ctrl-C will hard-kill");
            }
        }
        flag
    }))
}

/// Has the process been signalled to stop? Cheap enough to poll inside a sleep loop.
#[must_use]
pub fn signalled() -> bool {
    FLAG.get().is_some_and(|f| f.load(Ordering::SeqCst))
}

thread_local! {
    /// `Runtime.onShutdown` handlers, in REGISTRATION order (DEC-204, shape DEC-497).
    ///
    /// Thread-local rather than a global, and deliberately so: a handler is a `Value::Closure`, which
    /// owns `Rc`s and is therefore not `Send`. A process-wide registry would not compile, and forcing
    /// one would mean running user code on a thread that does not own its values. Each `phg serve`
    /// worker consequently keeps its own list, which is the correct semantics anyway — a worker
    /// cleans up what a worker registered.
    static HANDLERS: std::cell::RefCell<Vec<crate::value::Value>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Register a shutdown handler. Duplicate registrations are kept: registering the same closure twice
/// is the caller saying "run it twice", and silently de-duplicating `Value`s would need an equality
/// on closures that phorj does not define.
pub fn register(handler: crate::value::Value) {
    HANDLERS.with(|h| h.borrow_mut().push(handler));
}

/// Drain the handlers, returning them in registration order. Draining (rather than borrowing) is what
/// makes running them re-entrancy-safe: a handler that itself calls `Runtime.onShutdown` appends to a
/// now-empty list instead of mutating the vector being iterated, and a second drain runs nothing —
/// so `Runtime.exit` inside a handler cannot re-run the whole set.
#[must_use]
pub fn take_handlers() -> Vec<crate::value::Value> {
    HANDLERS.with(|h| std::mem::take(&mut *h.borrow_mut()))
}

/// Test-only: clear the flag so one case's signal cannot leak into the next.
#[cfg(test)]
pub(crate) fn reset_for_test() {
    if let Some(f) = FLAG.get() {
        f.store(false, Ordering::SeqCst);
    }
    let _ = take_handlers();
}

/// Test-only: raise the flag without a real signal.
#[cfg(test)]
pub(crate) fn raise_for_test() {
    flag().store(true, Ordering::SeqCst);
}

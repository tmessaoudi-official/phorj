//! Serve — the DEC-331 D5 **web** handler factories (slice S3.3a).
//!
//! The legacy path in `handlers.rs` resolves a fixed named entry, `respond(bytes): bytes`, and calls
//! it once per request. The D5 path resolves nothing by name: the `#[Entry(kind: EntryKind.Web)]`
//! function is a **closure factory** that calls `Http.serve(cfg, handler)`, and the handler it
//! registers is the thing called per request.
//!
//! Both factories therefore have the same two-phase shape, and the phases live on different sides of
//! the thread boundary on purpose:
//!
//! 1. **Once per worker thread** — clear this thread's registration slot, run the web entry, take the
//!    closure it registered. The closure is `Rc`-bearing, so this must happen on the thread that will
//!    use it; that is exactly the constraint [`HandlerFactory`](super::HandlerFactory) already
//!    encodes, which is why no listener seam or loop inversion is needed.
//! 2. **Once per request** — call that closure with the raw request bytes, on a FRESH
//!    interpreter/VM. Fresh is what makes the ruled semantics true: the closure's CAPTURES persist
//!    across requests (they live in the closure value), while program STATICS re-seed per request
//!    (they live in the machine). Reusing one machine would flip the second half silently, and only
//!    on one leg — a divergence the byte-identity differential cannot see, because serve is
//!    Invariant-14 quarantined.
//!
//! BOTH factories return `Result`, which the legacy pair did not: `interp_factory` resolved its entry
//! lazily, per request, so it could not fail at startup at all. Since S3.3c made `Http.serve` the only
//! way to register a handler, "this program is not servable" is a STARTUP error on both backends —
//! the tree-walker no longer discovers it one request at a time.
//!
//! A registration failure is NOT a panic and NOT a silent empty handler: the factory's `Result` is
//! captured and every request on that worker returns the diagnostic, which the serve loop degrades
//! to a 500. `HandlerFactory` returns a `Handler`, not a `Result<Handler>`, so this is the only place
//! the error can live once a worker has started — and a startup-time validation run (below) means the
//! common case is caught before any socket is bound.

use super::{compile_with, Diagnostic, Handler, HandlerFactory, Program, Reified, Value, Vm};
use crate::native::http::serve_register;
use std::rc::Rc;
use std::sync::Arc;

/// The startup refusal raised when a program reaches the serve runtime with nothing registered to
/// handle requests. A NAMED const, not an inline literal, so `scripts/doc-guards.sh` and
/// `scripts/surface-ratchet.sh` can both see it — an inline literal is how `E-CONCURRENCY-NO-PHP`
/// stayed invisible to the ratchets for releases.
///
/// ONE code, two messages. Both are the same condition from the user's side — "nothing will answer a
/// request" — and splitting them would mean a second `phg explain` entry that says the same thing
/// with a different first paragraph. The messages differ because the FIXES differ: write the call, or
/// migrate a retired shape.
pub(super) const E_SERVE_NO_HANDLER: &str = "E-SERVE-NO-HANDLER";

/// Resolve the program's `Web` entry to a free-function name, and REFUSE the shapes this factory
/// cannot run.
///
/// A CLASS-BOUND web entry (`entry_for` returning `Some(class)`) is refused with a named error rather
/// than silently resolving some other function of the same name. It is not supported yet, and a
/// misresolved entry index would serve the wrong function while looking healthy — this stack's
/// characteristic failure mode.
///
/// A PARAMETERISED web entry is the retired `(Request): Response` shape (S3.3c). It is refused HERE,
/// before the entry is called, because the alternative is not a missing check but a WRONG error: the
/// factory would call the legacy entry with no arguments and the user's startup message would be
/// `` `handle` expects 1 argument(s), got 0 `` — an opaque arity complaint about a program that was
/// well-formed one release ago. Every pre-D5 serve program takes this path exactly once, which makes
/// it the most-read diagnostic of the whole retirement. The check is on ARITY, not on the parameter's
/// type: `(): void` is the only shape this factory can run, so anything else is refused whatever it
/// takes. Note the CHECKER still accepts the legacy shape for `kind: Web` — narrowing it there is
/// S3.3d's job, in the same change that migrates the examples.
fn web_entry_name(program: &Program) -> Result<String, Diagnostic> {
    match crate::ast::entry_for(program, crate::ast::EntryRole::Web) {
        Some((None, f)) if f.params.is_empty() => Ok(f.name.clone()),
        Some((None, f)) => Err(Diagnostic::runtime(format!(
            "serve: the web entry `{}` takes {} parameter(s) — the `respond`/`handle` \
             `(Request): Response` entry was RETIRED in DEC-331 S3.3c. A web entry is now a closure \
             FACTORY: make it `(): void` and call `Http.serve(cfg, handler)` in its body, passing the \
             old body as the handler.",
            f.name,
            f.params.len()
        ))
        .with_code(E_SERVE_NO_HANDLER)),
        Some((Some(cls), f)) => Err(Diagnostic::runtime(format!(
            "serve: the web entry `{cls}.{}` is a class method — only a free function is supported \
             (DEC-331 S3.3a)",
            f.name
        ))),
        None => Err(Diagnostic::runtime(
            "serve needs an `#[Entry(kind: EntryKind.Web)]` function that calls `Http.serve(cfg, handler)`"
                .to_string(),
        )
        .with_code(E_SERVE_NO_HANDLER)),
    }
}

/// Run the web entry on THIS thread and take the handler closure it registered.
///
/// `reset` first: a worker thread may build more than one factory over a process lifetime (the test
/// harness does), and a stale closure left by an earlier build is indistinguishable from this one's —
/// it would serve the previous program while reporting healthy.
fn register_on_this_thread(program: &Program, entry: &str) -> Result<Value, Diagnostic> {
    serve_register::reset();
    crate::interpreter::call_named(program, entry, vec![])?;
    serve_register::take_handler().ok_or_else(|| {
        Diagnostic::runtime(format!(
            "serve: the web entry `{entry}` returned without calling `Http.serve(cfg, handler)` — \
             nothing was registered to handle requests"
        ))
        .with_code(E_SERVE_NO_HANDLER)
    })
}

/// Same, on the VM: run the web entry through a VM over `compiled`, then take what it registered.
fn register_on_this_thread_vm(
    compiled: &crate::chunk::BytecodeProgram,
    entry: usize,
) -> Result<Value, Diagnostic> {
    serve_register::reset();
    Vm::new(compiled).run_entry(entry, vec![])?;
    serve_register::take_handler().ok_or_else(|| {
        Diagnostic::runtime(
            "serve: the web entry returned without calling `Http.serve(cfg, handler)` — nothing was \
             registered to handle requests"
                .to_string(),
        )
        .with_code(E_SERVE_NO_HANDLER)
    })
}

/// The tree-walking-interpreter web backend (`phg serve --tree-walker`) — the correctness oracle.
///
/// The entry is run ONCE up front as well, purely to surface a registration failure as a startup
/// error before any socket binds. That run is discarded; each worker registers its own.
pub fn web_interp_factory(program: Arc<Program>) -> Result<HandlerFactory, Diagnostic> {
    let entry = web_entry_name(&program)?;
    register_on_this_thread(&program, &entry)?;
    Ok(Box::new(move || {
        let program = Arc::clone(&program);
        let registered = register_on_this_thread(&program, &entry);
        let handler: Handler = match registered {
            Ok(closure) => Box::new(move |raw: &[u8]| {
                crate::interpreter::call_closure_in(
                    &program,
                    &closure,
                    vec![Value::Bytes(Rc::new(raw.to_vec()))],
                )
            }),
            // Per-worker registration failed after the startup run succeeded. Report it on every
            // request (→ 500) rather than panicking the worker thread.
            Err(d) => Box::new(move |_raw: &[u8]| Err(d.clone())),
        };
        handler
    }))
}

/// The bytecode-VM web backend (the default `phg serve`).
///
/// Mirrors `vm_factory`'s startup discipline: compile once up front so a compile error surfaces
/// before the socket binds, and resolve the entry index there; workers recompile deterministically,
/// so the index holds. The `synth_empty_main` injection is the same one the legacy factory needs — a
/// web-only program legitimately has no `main`, and the bytecode compiler requires an entry.
pub fn web_vm_factory(
    program: Arc<Program>,
    reified: Arc<Reified>,
) -> Result<HandlerFactory, Diagnostic> {
    let entry_name = web_entry_name(&program)?;
    let program = if crate::ast::entry_for(&program, crate::ast::EntryRole::Cli).is_none() {
        let mut p = (*program).clone();
        p.items.push(crate::ast::synth_empty_main());
        Arc::new(p)
    } else {
        program
    };
    let compiled =
        compile_with(&program, &reified).map_err(|e| Diagnostic::runtime(e.to_string()))?;
    let entry = compiled
        .functions
        .iter()
        .position(|f| f.name == entry_name)
        .ok_or_else(|| {
            Diagnostic::runtime(format!(
                "serve: the web entry `{entry_name}` has no compiled function"
            ))
        })?;
    // Startup validation run, discarded — same purpose as the interpreter factory's.
    register_on_this_thread_vm(&compiled, entry)?;
    drop(compiled);
    Ok(Box::new(move || {
        let compiled =
            compile_with(&program, &reified).expect("serve program compiled cleanly at startup");
        let registered = register_on_this_thread_vm(&compiled, entry);
        let handler: Handler = match registered {
            Ok(closure) => Box::new(move |raw: &[u8]| {
                Vm::new(&compiled)
                    .run_closure_entry(&closure, &[Value::Bytes(Rc::new(raw.to_vec()))])
            }),
            Err(d) => Box::new(move |_raw: &[u8]| Err(d.clone())),
        };
        handler
    }))
}

//! `phg serve`'s pipeline step — checking the program, building the web handler factory, resolving
//! the ruled CLI-flag-vs-`ServeConfig` precedence, and entering the blocking accept loop.
//!
//! Split out of `pipeline.rs` by S3.2 Part C: that file is grandfathered at 860 lines in
//! `scripts/size-baseline.txt` (Invariant 13 — a grandfathered file may only SHRINK), and the
//! precedence wiring pushed it over. Serve is also the one pipeline step with a genuinely different
//! shape — it never returns while the server is up — so it is a cohesive unit rather than an
//! arbitrary line-count cut.
use super::pipeline::check_and_expand_reified;
use super::pipeline::on_deep_stack;
use crate::ast::Program;

/// `serve` on an already-loaded program (M6 W4): type-check, build the request handler factory, then
/// run the blocking HTTP serve loop ([`crate::serve::serve_tcp`]) until the process is killed. Defaults
/// to the **bytecode VM** (faster than the tree-walker — measured ~2.3× lower end-to-end latency on a
/// representative handler; byte-identical via [`Vm::run_closure_entry`] ≡ `call_closure_in`);
/// `tree_walker` selects the interpreter oracle (`phg serve --tree-walker`). The single-threaded path
/// runs on the 256 MB deep-stack worker (native-stack headroom for re-entrant natives / the
/// interpreter's deep recursion); `--workers N` pool threads are plain `std::thread::spawn` (~8 MB),
/// not that worker — the VM is iterative so it is far less exposed than the tree-walker was, but a
/// `--tree-walker` pool worker has less headroom than the single-threaded path (pre-existing).
pub fn serve_program(
    prog: &Program,
    diag_src: &str,
    flags: &crate::serve::ServeFlags,
    profile: crate::profile::Profile,
    tree_walker: bool,
) -> Result<String, String> {
    on_deep_stack(|| {
        let (factory, settings) = prepare_serve(prog, diag_src, flags, tree_walker)?;
        // The "loudly" half of the ruling. stderr, not stdout: stdout belongs to the served program's
        // `Output.*` (DEC-220), and a server's startup notes must not land in a piped response body.
        for note in &settings.notices {
            eprintln!("{note}");
        }
        crate::serve::serve_tcp(
            factory,
            &settings.addr,
            settings.timeout,
            profile,
            settings.workers,
        )
        .map_err(|e| format!("serve: {e}"))?;
        Ok(String::new())
    })
}

/// Everything `serve_program` does BEFORE it blocks: check, build the handler factory, then resolve
/// the ruled flag-vs-config precedence.
///
/// **This is split out so the ordering can be TESTED, not merely commented.** DEC-455.14's first
/// pinned fact is that the config is readable only after the factory build — the factory's startup
/// validation run is what executes the `Web` entry and populates the global — and a comment does not
/// fail when someone hoists the read. `serve_program` above is now a shell whose only untestable part
/// is the blocking `serve_tcp` call; every decision lives here, where
/// `serve_settings_are_resolved_from_the_registered_config` exercises it.
pub(crate) fn prepare_serve(
    prog: &Program,
    diag_src: &str,
    flags: &crate::serve::ServeFlags,
    tree_walker: bool,
) -> Result<(crate::serve::HandlerFactory, crate::serve::ServeSettings), String> {
    // Reified side-table is threaded into the VM compile (Invariant 6); the interp path ignores it.
    let (checked, reified) = check_and_expand_reified(prog, diag_src)?;
    let checked = std::sync::Arc::new(checked);
    // `render`, not `to_string`: Display drops the code that `--help` sends the reader to explain.
    let factory = if tree_walker {
        crate::serve::web_interp_factory(checked).map_err(|e| e.render(diag_src))?
    } else {
        crate::serve::web_vm_factory(checked, std::sync::Arc::new(reified))
            .map_err(|e| e.render(diag_src))?
    };
    // S3.2 Part C (DEC-455.14): the factory's STARTUP run has now executed the `Web` entry once on
    // this thread, so `Http.serve`'s registration has populated the config global — this is the
    // first moment the program's own `ServeConfig` can be read, and it is still before any socket
    // binds. Ordering is load-bearing: reading it before the factory is built would always see
    // `None`, i.e. the config would silently never apply.
    let cfg = crate::native::http::serve_register::config();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    Ok((
        factory,
        crate::serve::resolve_settings(flags, cfg.as_ref(), cores),
    ))
}

#[cfg(test)]
#[path = "serve_pipeline_tests.rs"]
mod serve_pipeline_tests;

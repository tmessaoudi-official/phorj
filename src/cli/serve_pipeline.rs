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
        let (factory, settings, tls) = prepare_serve(prog, diag_src, flags, tree_walker)?;
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
            tls,
        )
        .map_err(|e| format!("serve: {e}"))?;
        Ok(String::new())
    })
}

/// The process-wide setup every `phg serve` needs, wherever it was reached from — the real `serve`
/// verb, or S3.4's *"Did you mean `phg serve …`?"* switch. Returns the profile it activated.
///
/// **ONE definition, deliberately.** A second copy at the switch site would drift the moment serve
/// grows another prerequisite, and both of its steps are silent when missed: without
/// `set_stdin_disabled` a worker blocks the whole server on the terminal's stdin, and without the
/// profile reset a switch inherits `phg run`'s `Dev` (`main.rs` sets it unconditionally) and serves
/// rich fault pages — trace and source — from a command the user typed as `serve`. This repo already
/// documents that class of hazard for `toolchain.env` vs CI: a second resolution path is not a copy,
/// it is a future divergence.
pub fn serve_preamble(dev: bool) -> crate::profile::Profile {
    // DEC-281: web input is the `Request` — a worker blocking on the terminal's stdin would hang the
    // server, so `Core.Input` reads behave as an exhausted pipe under serve.
    crate::native::set_stdin_disabled();
    // M-DX S0: `--dev` selects the Dev profile (rich fault pages); the default is the secure Release
    // profile (bare 500, no trace/source leak). Set it as the process profile too.
    let profile = if dev {
        crate::profile::Profile::Dev
    } else {
        crate::profile::Profile::Release
    };
    crate::profile::set_active(profile);
    profile
}

/// `phg serve <file>` with DEFAULTS — exactly, and only, the command S3.4's switch prompt offers.
///
/// The prompt shows `phg serve <file>` with no flags, so this must bind what a bare `phg serve <file>`
/// binds: default address, default timeout, default workers, `Release`. What is displayed is what
/// runs.
pub fn serve_with_defaults(
    unit: &crate::loader::Unit,
    tree_walker: bool,
) -> Result<String, String> {
    let profile = serve_preamble(false);
    serve_program(
        &unit.program,
        &unit.diag_src,
        &crate::serve::ServeFlags::default(),
        profile,
        tree_walker,
    )
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
type PreparedServe = (
    crate::serve::HandlerFactory,
    crate::serve::ServeSettings,
    Option<crate::serve::tls::TlsServer>,
);

pub(crate) fn prepare_serve(
    prog: &Program,
    diag_src: &str,
    flags: &crate::serve::ServeFlags,
    tree_walker: bool,
) -> Result<PreparedServe, String> {
    // S3.4 (DEC-331 D6): the wrong-verb refusal comes FIRST — before the check, and before the
    // factory whose `web_entry_name` would otherwise report the absence with `E-SERVE-NO-HANDLER`.
    // The two diagnoses differ: `E-SERVE-NO-HANDLER` means *nothing will answer a request*, which is
    // the right answer for a library; a program with a `Cli` entry and no `Web` one is a user who
    // typed `serve` when they meant `run`, and telling them to write an `Http.serve` call sends them
    // to rewrite a program that was already correct.
    crate::cli::role_mismatch::guard(prog, crate::ast::EntryRole::Web)?;
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
    // S3.5 (DEC-331 D7): TLS is read from the config DIRECTLY, not through `resolve_settings`. That
    // function is the flag-vs-config PRECEDENCE rule, and D7 rules there is no `--tls` flag — with
    // only one source there is no precedence to resolve, and threading it through would invent a
    // conflict that cannot occur. Both steps are fallible and their ORDER is the ruled one: a config
    // that is wrong (`E-SERVE-TLS-INCOMPLETE`) is reported before a build that cannot honour it
    // (`E-SERVE-TLS-DISABLED`), because the config is wrong regardless of how phg was compiled.
    //
    // This runs BEFORE any socket binds. A server whose certificate is missing or malformed must
    // fail to START, not bind the port and then fail every handshake — the latter looks like a
    // working server to everything except a client.
    // DEC-475: the range gate runs FIRST — before TLS is built and long before a socket binds.
    // A config whose numbers are nonsense is wrong no matter what else succeeds, and the same
    // reasoning the TLS ordering comment gives applies one step earlier.
    if let Some(c) = cfg.as_ref() {
        crate::serve::validate_config(c)?;
    }
    let tls = match cfg.as_ref().map(crate::serve::tls::requested).transpose()? {
        Some(Some(req)) => Some(crate::serve::tls::build(&req)?),
        _ => None,
    };
    Ok((
        factory,
        crate::serve::resolve_settings(flags, cfg.as_ref(), cores),
        tls,
    ))
}

#[cfg(test)]
#[path = "serve_pipeline_tests.rs"]
mod serve_pipeline_tests;

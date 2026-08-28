//! `phg serve`'s ARGV branch — flag parsing, DEC-282 site-mode resolution, the process preamble, and
//! the blocking hand-off to [`serve_program`], plus S3.4's serve→run switch on failure.
//!
//! Split out of `main.rs` (Invariant 13, M-Decomp): that file is grandfathered at 622 lines in
//! `scripts/size-baseline.txt` and may only SHRINK, and S3.4's switch wiring pushed it over. `serve`
//! is the largest and least dispatcher-shaped branch there — 140 lines of flag parsing that belong
//! with the rest of the serve pipeline rather than in a dispatcher — so it is the cohesive cut, not
//! merely the biggest one. Behaviour is unchanged by the move: every message, exit code and ordering
//! is carried over verbatim.
//!
//! `usage` is passed in rather than imported because the usage banner lists every verb and lives with
//! the dispatcher; taking it as a parameter keeps the two from forking.
use std::process::exit;

/// Run the `serve` verb and never return: it either enters the blocking accept loop or exits.
pub fn serve_main(args: &[String], usage_exit: fn() -> !) -> ! {
    let mut file: Option<&str> = None;
    // S3.2 Part C (DEC-455.14): these stay `None` until a flag is actually PASSED. The whole
    // precedence rule turns on that distinction — pre-defaulting them here would make every run
    // look like an explicit override of the program's own `ServeConfig`. Defaults live in
    // `serve::settings` and are applied there, once, alongside the config.
    let mut addr: Option<String> = None;
    // Per-connection read/write timeout (GA blocker B4): default 30s; `--timeout 0` disables it.
    let mut timeout_secs: Option<u64> = None;
    // `--dev` opts into the rich HTML error page on an uncaught handler fault. OFF by default:
    // production must never leak a stack trace / source (a security rule) — it returns a bare 500.
    let mut dev = false;
    // `--workers N` request concurrency (M6 W3). 0 (the sentinel) = auto = CPU cores; 1 = the
    // single-threaded path. Resolved after parsing.
    let mut workers: Option<usize> = None;
    // `--tree-walker` serves on the interpreter oracle instead of the (default) VM — mirrors
    // `phg run --tree-walker`. The VM is faster (measured ~2.3× lower latency) and
    // byte-identical.
    let mut tree_walker = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            // `--addr` is a deprecated alias for `--address` (DEC-276 earned-shortcut rule:
            // word truncations are not earned), silently accepted — remove in a future version.
            "--address" | "--addr" => {
                addr = Some(args.get(i + 1).cloned().unwrap_or_else(|| {
                    usage_exit();
                }));
                i += 2;
            }
            "--timeout" => {
                timeout_secs = Some(
                    args.get(i + 1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or_else(|| {
                            eprintln!("phg serve: --timeout expects a whole number of seconds");
                            exit(2);
                        }),
                );
                i += 2;
            }
            "--dev" => {
                dev = true;
                i += 1;
            }
            "--tree-walker" => {
                tree_walker = true;
                i += 1;
            }
            "--workers" => {
                workers = Some(
                    args.get(i + 1)
                        .and_then(|s| s.parse::<usize>().ok())
                        .filter(|n| *n >= 1)
                        .unwrap_or_else(|| {
                            eprintln!("phg serve: --workers expects a positive whole number");
                            exit(2);
                        }),
                );
                i += 2;
            }
            a if !a.starts_with('-') && file.is_none() => {
                file = Some(a);
                i += 1;
            }
            _ => {
                usage_exit();
            }
        }
    }
    let file = file.unwrap_or_else(|| {
        eprintln!(
            "usage: phg serve <file | site-dir> [--address 127.0.0.1:8080] [--timeout 30] [--workers N] [--tree-walker]"
        );
        exit(2);
    });
    // DEC-282 site mode: `phg serve <DIR>` — DIR is the explicit app root; docroot = DIR/public
    // (the ONLY web surface; static assets served with guards, .phg source never), entry =
    // DIR/public/index.phg. A file argument keeps today's handler-only mode (no docroot).
    let mut entry_path = std::path::PathBuf::from(file);
    // Captured BEFORE site resolution rewrites `entry_path` — S3.4 needs what the user typed.
    let raw_arg = file.to_string();
    let arg_was_dir = entry_path.is_dir();
    if entry_path.is_dir() {
        match crate::serve::resolve_site_dir(&entry_path) {
            Ok((docroot, entry)) => {
                crate::serve::set_docroot(docroot);
                entry_path = entry;
            }
            Err(err) => {
                eprintln!("{err}");
                exit(2);
            }
        }
    }
    let file: &str = entry_path.to_string_lossy().into_owned().leak();
    // The worker count, the address and the timeout are all resolved in ONE place —
    // `serve::settings::resolve` — because each of them now has two possible sources (this flag
    // and the program's registered `ServeConfig`) and the precedence between them is a ruled
    // rule, not three independent `unwrap_or`s scattered across the argument parser.
    let serve_flags = crate::serve::ServeFlags {
        addr,
        timeout_secs,
        workers,
    };
    let unit = match crate::loader::load(std::path::Path::new(file)) {
        Ok(u) => u,
        Err(err) => {
            eprintln!("{err}");
            exit(1);
        }
    };
    // S3.4/D6: decide the VERB before any process-wide serve setup. `serve_preamble` below calls
    // `set_stdin_disabled()`, which is a ONE-WAY global (`src/native/input.rs:49` has a setter and no
    // inverse), and pins the profile to Release. A switched run that ran after it would read stdin as
    // an exhausted pipe and render faults under the wrong profile — i.e. NOT what `phg run <file>`
    // does, breaking the same "what is displayed is what runs" promise `serve_preamble` exists to
    // keep in the other direction. Ordering is the only available fix, since the stdin flag cannot be
    // unset. `prepare_serve` keeps its own guard as the invariant for every other caller; this one is
    // the UX ordering, and it is why that one never fires on this path.
    if let Err(err) = crate::cli::role_mismatch::guard(&unit.program, crate::ast::EntryRole::Web) {
        eprintln!("{err}");
        // S3.4/D6, the serve->run half; `None` unless a switch was offered AND accepted.
        match crate::cli::role_mismatch::switch_serve_to_run(&err, &raw_arg, arg_was_dir, &unit) {
            Some(Ok((text, code))) => {
                print!("{text}");
                exit(i32::try_from(code).unwrap_or(1));
            }
            Some(Err(e)) => {
                eprintln!("{e}");
                exit(1);
            }
            None => exit(1),
        }
    }
    // Shared with S3.4's switch (`crate::cli::serve_with_defaults`) so the two cannot drift.
    let profile = crate::cli::serve_preamble(dev);
    match crate::cli::serve_program(
        &unit.program,
        &unit.diag_src,
        &serve_flags,
        profile,
        tree_walker,
    ) {
        Ok(text) => {
            print!("{text}");
            exit(0);
        }
        Err(err) => {
            // A role mismatch cannot reach here — the guard above returns first, deliberately, so the
            // switch runs before serve touches process state. This arm is every OTHER serve failure.
            eprintln!("{err}");
            exit(1);
        }
    }
}

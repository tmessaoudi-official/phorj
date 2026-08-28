//! DEC-331 S3.4 (spec D6/P3) — the role-mismatch diagnostic: you used the wrong verb.
//!
//! `phg run` needs a `#[Entry(kind: EntryKind.Cli)]` function and `phg serve` needs a
//! `#[Entry(kind: EntryKind.Web)]` one. Before this module a program that declared the OTHER role got
//! a message describing only the absence — *"no entry point: running needs an `#[Entry(kind:
//! EntryKind.Cli)]` function"* — which is true, unhelpful, and identical to what a library with no
//! entry at all is told. The two cases have different fixes: a library needs an entry written, and
//! this one needs a different command typed.
//!
//! **Everything here is pure.** No terminal read, no process spawn, no I/O. TTY-ness and the user's
//! answer are decided by the caller, the way `serve::settings::resolve` takes its `cores` — so the
//! whole ruling is exercised by the test suite rather than by a human at a prompt.

use crate::ast::{entry_for, EntryRole, Program};

/// The role-mismatch code. A named const, never an inline literal, so `scripts/surface-ratchet.sh`
/// can see it — an inline literal is how `E-CONCURRENCY-NO-PHP` stayed invisible to the ratchets for
/// releases (`src/serve/web_handlers.rs:40` carries the same warning for `E-SERVE-NO-HANDLER`).
pub const E_NO_ENTRY_FOR_ROLE: &str = "E-NO-ENTRY-FOR-ROLE";

/// The program declares an entry — just not the one this verb runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mismatch {
    /// The role the verb the user typed needs, and the program does not have.
    pub wanted: EntryRole,
    /// The role the program DOES declare. Never equal to `wanted`.
    pub found: EntryRole,
}

/// Is this a wrong-verb mistake? `Some` **iff** `wanted` is absent AND the other role is present.
///
/// The `iff` is the whole design. A program with NEITHER role is a library, not a mistake: it keeps
/// today's *"no entry point"* / `E-SERVE-NO-HANDLER` message, and must never be offered a verb it has
/// nothing to answer with. A program with a reserved-but-unbuilt kind (`kind: Desktop`) has no active
/// role either — [`crate::ast::entry_declared_role`] is `Active`-only — so it falls through to
/// `E-ENTRY-KIND-RESERVED`, which is the diagnosis it actually needs.
#[must_use]
pub fn detect(program: &Program, wanted: EntryRole) -> Option<Mismatch> {
    if entry_for(program, wanted).is_some() {
        return None;
    }
    let other = match wanted {
        EntryRole::Cli => EntryRole::Web,
        EntryRole::Web => EntryRole::Cli,
    };
    entry_for(program, other).map(|_| Mismatch {
        wanted,
        found: other,
    })
}

/// How a role is spelled in a `kind:` argument, what to call it in prose, the verb that runs it, and
/// the bare imperative that verb answers to. Four facts in one place so the two directions of the message
/// cannot drift apart — the symmetry D6 asks for is structural here, not copied.
const fn role_facts(r: EntryRole) -> RoleFacts {
    match r {
        EntryRole::Cli => RoleFacts {
            kind: "EntryKind.Cli",
            noun: "command-line",
            verb: "phg run",
            imperative: "run",
        },
        EntryRole::Web => RoleFacts {
            kind: "EntryKind.Web",
            noun: "web",
            verb: "phg serve",
            imperative: "serve",
        },
    }
}

/// The four per-role strings [`role_facts`] returns.
struct RoleFacts {
    kind: &'static str,
    noun: &'static str,
    verb: &'static str,
    imperative: &'static str,
}

/// The rendered diagnostic. Names the missing role, the declared one, and the verb that would work.
#[must_use]
pub fn message(m: &Mismatch) -> String {
    let want = role_facts(m.wanted);
    let found = role_facts(m.found);
    format!(
        "{E_NO_ENTRY_FOR_ROLE}: `{}` needs an `#[Entry(kind: {})]` function, but this program \
         declares an `#[Entry(kind: {})]` {} entry and no {} one. The verb is the mismatch, not the \
         program \u{2014} {} it with `{}`, or add a {} entry. (`phg explain {E_NO_ENTRY_FOR_ROLE}`)",
        want.verb,
        want.kind,
        found.kind,
        found.noun,
        want.noun,
        found.imperative,
        found.verb,
        want.noun,
    )
}

/// What the pipeline calls: `Err` with the rendered diagnostic when the verb is wrong, `Ok` otherwise.
pub fn guard(program: &Program, wanted: EntryRole) -> Result<(), String> {
    match detect(program, wanted) {
        Some(m) => Err(message(&m)),
        None => Ok(()),
    }
}

/// The file a *"Did you mean …?"* prompt may name, or `None` when this source must not be prompted.
///
/// Only a plain `.phg` file argument is prompt-eligible. `phg serve` accepts neither stdin nor `-e`
/// (its argv parser takes a file-or-directory positional and nothing else), so for those sources
/// there is no command to offer — the coded diagnostic stands alone.
#[must_use]
pub fn prompt_target(spec: &crate::cli::SourceSpec) -> Option<String> {
    match spec {
        crate::cli::SourceSpec::File(p) => Some(p.clone()),
        crate::cli::SourceSpec::Stdin | crate::cli::SourceSpec::Inline(_) => None,
    }
}

/// The serve→run direction's prompt target: the argument `phg serve` was given, unless it was a
/// DIRECTORY.
///
/// `phg serve <dir>` site-resolves to `<dir>/public/index.phg` (DEC-282), but `phg run` cannot take a
/// directory at all — `loader::load` reads a file, with no `is_dir` branch. Offering
/// `phg run <dir>` would therefore propose a command that cannot run, so a directory source gets the
/// diagnostic naming the RESOLVED entry file and no prompt.
#[must_use]
pub fn serve_prompt_target(arg: &str, arg_was_dir: bool) -> Option<String> {
    if arg_was_dir {
        None
    } else {
        Some(arg.to_string())
    }
}

/// The command a *"Did you mean …?"* prompt should offer, or `None` when no prompt may be shown.
///
/// Pure — every reason to stay silent is decided here, so the I/O wrapper below has no judgement of
/// its own. `None` when the failure was not a role mismatch, or when the source cannot be named as an
/// argument to the other verb ([`prompt_target`] / [`serve_prompt_target`]).
#[must_use]
pub fn switch_command(err: &str, target: Option<&str>, verb: &str) -> Option<String> {
    // `starts_with`, not `contains`: both guard paths return `message()` verbatim, which BEGINS with
    // the code, so the tighter test costs nothing and refuses to be fooled by a user program whose
    // own fault text happens to quote the code (a message about the error, a test fixture, a doc
    // string echoed at runtime). Offering to silently run a different verb on that basis would be a
    // bad trade for one word.
    if !err.starts_with(E_NO_ENTRY_FOR_ROLE) {
        return None;
    }
    target.map(|t| format!("{verb} {t}"))
}

/// Is this answer a yes? **The default is NO** (D6 writes the prompt `[y/N]`), so a bare Enter, an
/// EOF-emptied line and anything unrecognized all decline. Switching verbs runs the user's program —
/// never on an ambiguous answer.
#[must_use]
pub fn answer_is_yes(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Offer the switch on an interactive terminal and report whether the user accepted.
///
/// The whole D6 non-TTY rule is the `is_terminal()` pair: when either end is redirected (a pipe, CI,
/// a test harness) this returns `false` **without reading stdin at all**, so a script can never block
/// on a question nobody is there to answer. The prompt goes to stderr, not stdout — stdout belongs to
/// the program's own `Output.*` (DEC-220).
///
/// **On wasm32 there is no terminal, so no switch is ever offered.** The gate is INSIDE this
/// function, not on it: `scripts/wasm-check.sh` runs `cargo check --target wasm32` over the whole
/// package — the `phg` binary included — so a `#[cfg]` on the function itself only moves the error
/// from the missing terminal to a missing symbol at every call site. The rule is that these helpers
/// exist on every target and decline where they cannot ask.
pub fn accepted_switch(err: &str, target: Option<&str>, verb: &str) -> bool {
    let Some(cmd) = switch_command(err, target, verb) else {
        return false;
    };
    #[cfg(target_arch = "wasm32")]
    {
        let _ = cmd;
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::{IsTerminal, Write};
        if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
            return false;
        }
        eprint!("Did you mean `{cmd}`? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            return false;
        }
        answer_is_yes(&line)
    }
}

/// The run→serve half of the D6 switch: `None` when no switch was offered or it was declined,
/// otherwise the result of the `phg serve <file>` the prompt displayed.
///
/// Returning the outcome rather than exiting keeps `std::process::exit` out of the library — nothing
/// under `src/` calls it today, and a library that can kill the process is a trap for every other
/// caller (the LSP, the test runner, the playground).
pub fn switch_run_to_serve(
    err: &str,
    target: Option<&str>,
    unit: &crate::loader::Unit,
    tree_walker: bool,
) -> Option<Result<String, String>> {
    accepted_switch(err, target, "phg serve")
        .then(|| crate::cli::serve_with_defaults(unit, tree_walker))
}

/// The serve→run half, symmetric. `arg_was_dir` suppresses the offer for a site-mode directory
/// (see [`serve_prompt_target`]); the switched run carries its own exit code back to the caller.
pub fn switch_serve_to_run(
    err: &str,
    arg: &str,
    arg_was_dir: bool,
    unit: &crate::loader::Unit,
) -> Option<Result<(String, i64), String>> {
    let target = serve_prompt_target(arg, arg_was_dir);
    accepted_switch(err, target.as_deref(), "phg run").then(|| {
        // The mirror of `serve_preamble`: a real `phg run` selects the Dev profile (`main.rs`, the
        // interactive run tool). Program args are deliberately NOT set — the user typed
        // `phg serve <file>`, so there were none, and `PROCESS_ARGS` already defaults to empty.
        crate::profile::set_active(crate::profile::Profile::Dev);
        crate::cli::run_program_exit(unit)
    })
}

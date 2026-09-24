//! The run verbs that also carry `main`'s exit code (Batch-1 B) — what `phg run`, `-e`/stdin and a
//! built binary call. Split out of `pipeline.rs` (Invariant 13) when DEC-530 gave their error a
//! second half: a program that prints and then faults hands back the stdout it wrote first, so the
//! binary can print it before the error, as PHP does. The stdout-only `cmd_run`/`cmd_treewalk` keep
//! their plain `String` error for their many callers.

use super::pipeline::{
    check_and_expand, check_and_expand_reified, foreign_runtime_gate, lex_parse, on_deep_stack,
    parse_checked_for_run, run_guard, vm_for,
};
use crate::compiler::compile_with;

/// A failed run: the rendered error, plus the stdout the program wrote before it faulted (empty for
/// a failure before execution — parse, check, compile). `Display` is the error alone, so a caller
/// that only reports the error reads it exactly as before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFailure {
    pub stdout: String,
    pub message: String,
}

impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A pre-execution failure (the front-end stages return `String`) — nothing was printed yet.
impl From<String> for RunFailure {
    fn from(message: String) -> Self {
        RunFailure {
            stdout: String::new(),
            message,
        }
    }
}

use crate::diagnostic::Faulted;

/// Like `cmd_treewalk`, but also returns `main`'s exit code (Batch-1 B). The string source path
/// (`-e`/stdin and standalone built binaries); the project-loader path is [`treewalk_program_exit`].
pub fn cmd_treewalk_exit(src: &str) -> Result<(String, i64), RunFailure> {
    on_deep_stack(|| {
        let prog = parse_checked_for_run(src)?;
        foreign_runtime_gate(&prog)?;
        let plain = |f: Faulted| {
            let (e, stdout) = *f;
            RunFailure {
                stdout,
                message: e.to_string(),
            }
        };
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::interpreter::run_cooperative_interp(&prog).map_err(plain);
        }
        crate::interpreter::interpret_main_keeping_output(&prog).map_err(plain)
    })
}

/// Like `cmd_run`, but also returns `main`'s exit code (Batch-1 B).
pub fn cmd_run_exit(src: &str) -> Result<(String, i64), RunFailure> {
    on_deep_stack(|| {
        let parsed = lex_parse(src)?;
        run_guard(&parsed)?;
        let (prog, reified) = check_and_expand_reified(&parsed, src)?;
        foreign_runtime_gate(&prog)?;
        let program = compile_with(&prog, &reified).map_err(|e| e.to_string())?;
        let plain = |f: Faulted| {
            let (e, stdout) = *f;
            RunFailure {
                stdout,
                message: e.to_string(),
            }
        };
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&prog) {
            return crate::vm::run_cooperative_vm(&program).map_err(plain);
        }
        vm_for(&program).run_main_keeping_output().map_err(plain)
    })
}

/// Like `treewalk_program`, but also returns `main`'s exit code (Batch-1 B). `phg run <file>` uses
/// this to set the process exit status; the stdout-only `treewalk_program` stays for the differential.
pub fn treewalk_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), RunFailure> {
    on_deep_stack(|| {
        run_guard(&unit.program)?;
        let checked = check_and_expand(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let rendered = |f: Faulted| {
            let (mut e, stdout) = *f;
            let src = unit.attribute_frames(&mut e);
            RunFailure {
                stdout,
                message: e.render(&src),
            }
        };
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::interpreter::run_cooperative_interp(&checked).map_err(rendered);
        }
        crate::interpreter::interpret_main_keeping_output(&checked).map_err(rendered)
    })
}

/// Like `run_program`, but also returns `main`'s exit code (Batch-1 B).
pub fn run_program_exit(unit: &crate::loader::Unit) -> Result<(String, i64), RunFailure> {
    on_deep_stack(|| {
        run_guard(&unit.program)?;
        let (checked, reified) = check_and_expand_reified(&unit.program, &unit.diag_src)?;
        foreign_runtime_gate(&checked)?;
        let program = compile_with(&checked, &reified).map_err(|e| e.to_string())?;
        let rendered = |f: Faulted| {
            let (mut e, stdout) = *f;
            let src = unit.attribute_frames(&mut e);
            RunFailure {
                stdout,
                message: e.render(&src),
            }
        };
        #[cfg(all(feature = "green", not(target_arch = "wasm32")))]
        if crate::ast::uses_concurrency(&checked) {
            return crate::vm::run_cooperative_vm(&program).map_err(rendered);
        }
        vm_for(&program).run_main_keeping_output().map_err(rendered)
    })
}

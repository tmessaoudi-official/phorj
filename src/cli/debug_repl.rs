//! Terminal REPL frontend for the interactive debugger (M-DX S5b). One of the two thin adapters over
//! the shared [`crate::debug`] engine (the other is the DAP server). All debugger I/O is on **stderr**
//! (prompts, locals, backtrace) so it never mixes with the program's stdout; commands are read from
//! stdin. Dev-only, interpreter-only, entirely outside the correctness spine.

use std::io::{BufRead, Write};

use crate::debug::{DebugCommand, DebugFrontend, DebugSession, Debugger, PauseCtx};
use crate::loader::Unit;

/// The interactive REPL. Reads one command per pause from a `BufRead` (stdin in production, a byte
/// slice in tests) and writes prompts / inspections to a `Write` sink (stderr in production).
pub struct ReplFrontend<R: BufRead, W: Write> {
    input: R,
    out: W,
}

impl<R: BufRead, W: Write> ReplFrontend<R, W> {
    pub fn new(input: R, out: W) -> Self {
        ReplFrontend { input, out }
    }

    fn render_locals(&mut self, ctx: &PauseCtx) {
        if ctx.locals.is_empty() {
            let _ = writeln!(self.out, "  <no locals>");
            return;
        }
        for (name, value) in &ctx.locals {
            let _ = writeln!(self.out, "  {name} = {}", crate::inspect::render(value));
        }
    }

    fn render_backtrace(&mut self, ctx: &PauseCtx) {
        for (i, f) in ctx.frames.iter().enumerate() {
            let mark = if i == 0 { "→" } else { " " };
            let _ = writeln!(self.out, "  {mark} {:<18} line {}", f.function, f.line);
        }
    }
}

impl<R: BufRead, W: Write> DebugFrontend for ReplFrontend<R, W> {
    fn on_pause(&mut self, ctx: &PauseCtx) -> DebugCommand {
        let _ = writeln!(
            self.out,
            "⏸ paused at line {} (depth {})",
            ctx.line, ctx.depth
        );
        loop {
            let _ = write!(self.out, "(phg-dbg) ");
            let _ = self.out.flush();
            let mut line = String::new();
            match self.input.read_line(&mut line) {
                Ok(0) => return DebugCommand::Quit, // EOF — detach and finish
                Ok(_) => {}
                Err(_) => return DebugCommand::Quit,
            }
            let mut it = line.split_whitespace();
            let Some(cmd) = it.next() else {
                continue; // blank line — re-prompt
            };
            let arg = it.next().and_then(|s| s.parse::<u32>().ok());
            match cmd {
                "s" | "step" => return DebugCommand::StepInto,
                "n" | "next" => return DebugCommand::StepOver,
                "o" | "out" | "stepout" => return DebugCommand::StepOut,
                "c" | "continue" => return DebugCommand::Continue,
                "q" | "quit" => return DebugCommand::Quit,
                "b" | "break" => match arg {
                    Some(l) => {
                        let _ = writeln!(self.out, "  breakpoint set at line {l}");
                        return DebugCommand::SetBreakpoint(l);
                    }
                    None => {
                        let _ = writeln!(self.out, "  usage: break <line>");
                    }
                },
                "d" | "delete" | "clear" => match arg {
                    Some(l) => {
                        let _ = writeln!(self.out, "  breakpoint cleared at line {l}");
                        return DebugCommand::ClearBreakpoint(l);
                    }
                    None => {
                        let _ = writeln!(self.out, "  usage: clear <line>");
                    }
                },
                // Informational commands stay paused (handled here, not the engine).
                "l" | "locals" => {
                    let ctx_ref = ctx;
                    self.render_locals(ctx_ref);
                }
                "bt" | "backtrace" | "where" => {
                    let ctx_ref = ctx;
                    self.render_backtrace(ctx_ref);
                }
                "h" | "help" | "?" => {
                    let _ = writeln!(
                        self.out,
                        "  commands: step(s) next(n) stepout(o) continue(c) \
                         break(b) <line> clear(d) <line> locals(l) backtrace(bt) quit(q)"
                    );
                }
                other => {
                    let _ = writeln!(self.out, "  unknown command `{other}` — try `help`");
                }
            }
        }
    }
}

/// `phg debug <file>`: run `unit` under the interactive REPL debugger, reading commands from stdin
/// and writing the debugger UI to stderr. Returns the program's stdout (printed by the caller). The
/// session starts paused at the first statement so the user can set breakpoints.
pub fn run_repl(unit: &Unit) -> Result<String, String> {
    super::on_deep_stack(|| {
        let checked = super::check_and_expand_for_debug(&unit.program, &unit.diag_src)?;
        let stdin = std::io::stdin();
        let frontend = ReplFrontend::new(stdin.lock(), std::io::stderr());
        let session = DebugSession::new(Debugger::default(), Box::new(frontend));
        eprintln!(
            "phg debug — interpreter debugger. Paused at the first statement; type `help` for commands."
        );
        crate::interpreter::interpret_debug(&checked, session)
            .map(|(out, _exit)| out)
            .map_err(|mut e| {
                let src = unit.attribute_frames(&mut e);
                e.render(&src)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::PauseCtx;
    use crate::value::Value;

    fn ctx() -> PauseCtx {
        PauseCtx {
            line: 7,
            depth: 1,
            locals: vec![
                ("n".to_string(), Value::Int(5)),
                ("secret".to_string(), Value::Str("x".into())),
            ],
            frames: Vec::new(),
        }
    }

    /// Drive the REPL frontend over scripted stdin bytes, capturing the stderr sink.
    fn drive(script: &str) -> (DebugCommand, String) {
        let mut out = Vec::new();
        let cmd = {
            let mut fe = ReplFrontend::new(std::io::Cursor::new(script.as_bytes()), &mut out);
            fe.on_pause(&ctx())
        };
        (cmd, String::from_utf8(out).unwrap())
    }

    #[test]
    fn movement_commands_map_to_engine_commands() {
        assert_eq!(drive("s\n").0, DebugCommand::StepInto);
        assert_eq!(drive("n\n").0, DebugCommand::StepOver);
        assert_eq!(drive("o\n").0, DebugCommand::StepOut);
        assert_eq!(drive("c\n").0, DebugCommand::Continue);
        assert_eq!(drive("q\n").0, DebugCommand::Quit);
    }

    #[test]
    fn eof_detaches() {
        assert_eq!(drive("").0, DebugCommand::Quit);
    }

    #[test]
    fn break_parses_line_argument() {
        assert_eq!(drive("break 42\n").0, DebugCommand::SetBreakpoint(42));
        assert_eq!(drive("b 3\n").0, DebugCommand::SetBreakpoint(3));
    }

    #[test]
    fn locals_command_prints_then_reprompts_until_a_movement() {
        // `l` prints locals and stays paused; the following `c` resumes.
        let (cmd, out) = drive("l\nc\n");
        assert_eq!(cmd, DebugCommand::Continue);
        assert!(out.contains("n = 5"), "locals shown: {out}");
    }

    #[test]
    fn unknown_command_reprompts() {
        let (cmd, out) = drive("wat\ns\n");
        assert_eq!(cmd, DebugCommand::StepInto);
        assert!(out.contains("unknown command `wat`"), "{out}");
    }
}

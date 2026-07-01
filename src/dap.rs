//! Debug Adapter Protocol server (M-DX S5c) — the second thin adapter over the shared
//! [`crate::debug`] engine (the first is the terminal REPL). It speaks a focused subset of DAP over
//! `Content-Length`-framed JSON on stdio (the same framing the LSP uses), so the existing VS Code /
//! JetBrains debug UIs can drive Phorj's interpreter: set line breakpoints, launch, stop, inspect
//! locals, and step. Dev-only, interpreter-only, entirely off the correctness spine.
//!
//! Single-threaded by construction: the adapter owns the streams (shared via `Rc<RefCell<…>>` between
//! the request loop and the pause frontend) and runs the interpreter inline, so no value crosses a
//! thread boundary (the `Rc`-heap `Value` isn't `Send`). Deep recursion in a *debugged* program runs
//! on the default stack rather than the 256 MB worker — acceptable for interactive debugging.
//!
//! **Scope (v1):** `initialize`, `launch`, `setBreakpoints`, `configurationDone`, `threads`,
//! `stackTrace`, `scopes`, `variables`, `continue`, `next`, `stepIn`, `stepOut`, `disconnect`; emits
//! `initialized`, `stopped`, `terminated`, `exited`. Deferred (per the S5 spec): conditional
//! breakpoints, watchpoints, `pause` (async-break), multiple threads, VM stepping.

use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

use crate::debug::{DebugCommand, DebugFrontend, DebugSession, Debugger, PauseCtx};
use crate::json::Json;
use crate::loader::Unit;

/// Escape a string for embedding in a JSON body (the two mandatory escapes + common control chars).
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// A `Content-Length`-framed JSON writer shared by the request loop and the pause frontend.
struct Writer<W: Write> {
    out: W,
    seq: i64,
}

impl<W: Write> Writer<W> {
    fn send(&mut self, body: &str) {
        let _ = write!(self.out, "Content-Length: {}\r\n\r\n{}", body.len(), body);
        let _ = self.out.flush();
    }

    fn next_seq(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    /// A `response` to `request_seq` for `command`, with an inline `body` object (already JSON, or
    /// `"null"`). `success` is always `true` in this v1.
    fn response(&mut self, request_seq: i64, command: &str, body: &str) {
        let seq = self.next_seq();
        self.send(&format!(
            "{{\"seq\":{seq},\"type\":\"response\",\"request_seq\":{request_seq},\"success\":true,\"command\":\"{command}\",\"body\":{body}}}"
        ));
    }

    /// An `event` with an inline `body` object.
    fn event(&mut self, event: &str, body: &str) {
        let seq = self.next_seq();
        self.send(&format!(
            "{{\"seq\":{seq},\"type\":\"event\",\"event\":\"{event}\",\"body\":{body}}}"
        ));
    }
}

/// Read one `Content-Length`-framed message body, or `None` at EOF (mirrors the LSP transport).
fn read_message(reader: &mut impl BufRead) -> Option<String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None; // EOF
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break; // end of headers
        }
        if let Some(v) = trimmed.strip_prefix("Content-Length:") {
            content_length = v.trim().parse().ok();
        }
    }
    let len = content_length?;
    let mut buf = vec![0u8; len];
    std::io::Read::read_exact(reader, &mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// The pause frontend: on each interpreter pause it emits a `stopped` event, then services inspection
/// requests (`threads`/`stackTrace`/`scopes`/`variables`) until a movement request resumes.
struct DapFrontend<R: BufRead, W: Write> {
    input: Rc<RefCell<R>>,
    writer: Rc<RefCell<Writer<W>>>,
}

impl<R: BufRead, W: Write> DebugFrontend for DapFrontend<R, W> {
    fn on_pause(&mut self, ctx: &PauseCtx) -> DebugCommand {
        // "stopped" tells the editor we're paused; reason is generic ("step"/"breakpoint" both fit).
        self.writer.borrow_mut().event(
            "stopped",
            "{\"reason\":\"step\",\"threadId\":1,\"allThreadsStopped\":true}",
        );
        loop {
            let msg = match read_message(&mut *self.input.borrow_mut()) {
                Some(m) => m,
                None => return DebugCommand::Quit, // EOF — detach
            };
            let Some(req) = Json::parse(&msg) else {
                continue;
            };
            let seq = req.get("seq").and_then(Json::as_i64).unwrap_or(0);
            let command = req.get("command").and_then(Json::as_str).unwrap_or("");
            let mut w = self.writer.borrow_mut();
            match command {
                "threads" => {
                    w.response(
                        seq,
                        "threads",
                        "{\"threads\":[{\"id\":1,\"name\":\"main\"}]}",
                    );
                }
                "stackTrace" => {
                    let frames: Vec<String> = ctx
                        .frames
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            format!(
                                "{{\"id\":{i},\"name\":\"{}\",\"line\":{},\"column\":1}}",
                                esc(&f.function),
                                f.line
                            )
                        })
                        .collect();
                    w.response(
                        seq,
                        "stackTrace",
                        &format!(
                            "{{\"stackFrames\":[{}],\"totalFrames\":{}}}",
                            frames.join(","),
                            frames.len()
                        ),
                    );
                }
                "scopes" => {
                    // One "Locals" scope; its variablesReference is 1 (the only reference we vend).
                    w.response(
                        seq,
                        "scopes",
                        "{\"scopes\":[{\"name\":\"Locals\",\"variablesReference\":1,\"expensive\":false}]}",
                    );
                }
                "variables" => {
                    let vars: Vec<String> = ctx
                        .locals
                        .iter()
                        .map(|(name, value)| {
                            format!(
                                "{{\"name\":\"{}\",\"value\":\"{}\",\"variablesReference\":0}}",
                                esc(name),
                                esc(&crate::inspect::render(value))
                            )
                        })
                        .collect();
                    w.response(
                        seq,
                        "variables",
                        &format!("{{\"variables\":[{}]}}", vars.join(",")),
                    );
                }
                "continue" => {
                    w.response(seq, "continue", "{\"allThreadsContinued\":true}");
                    return DebugCommand::Continue;
                }
                "next" => {
                    w.response(seq, "next", "null");
                    return DebugCommand::StepOver;
                }
                "stepIn" => {
                    w.response(seq, "stepIn", "null");
                    return DebugCommand::StepInto;
                }
                "stepOut" => {
                    w.response(seq, "stepOut", "null");
                    return DebugCommand::StepOut;
                }
                "disconnect" | "terminate" => {
                    w.response(seq, command, "null");
                    return DebugCommand::Quit;
                }
                other => {
                    // Unknown/unsupported request while paused — acknowledge so the editor isn't stuck.
                    w.response(seq, other, "null");
                }
            }
        }
    }
}

/// Run a DAP session over `input`/`output` for `unit`. Handles the initialize/configure handshake,
/// launches the program under the debugger, and emits `terminated` when it ends. Single-threaded.
pub fn run_dap<R: BufRead + 'static, W: Write + 'static>(
    unit: &Unit,
    input: R,
    output: W,
) -> Result<(), String> {
    let checked = crate::cli::check_and_expand_for_debug(&unit.program, &unit.diag_src)?;
    let input = Rc::new(RefCell::new(input));
    let writer = Rc::new(RefCell::new(Writer {
        out: output,
        seq: 0,
    }));
    let mut breakpoints: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();

    // ── Handshake: service requests until `launch`/`configurationDone`, then run. ──
    loop {
        let msg = match read_message(&mut *input.borrow_mut()) {
            Some(m) => m,
            None => return Ok(()), // client disconnected before launch
        };
        let Some(req) = Json::parse(&msg) else {
            continue;
        };
        let seq = req.get("seq").and_then(Json::as_i64).unwrap_or(0);
        let command = req.get("command").and_then(Json::as_str).unwrap_or("");
        match command {
            "initialize" => {
                let mut w = writer.borrow_mut();
                w.response(
                    seq,
                    "initialize",
                    "{\"supportsConfigurationDoneRequest\":true}",
                );
                w.event("initialized", "{}");
            }
            "setBreakpoints" => {
                // arguments.breakpoints[].line → the breakpoint set; respond with them "verified".
                let lines: Vec<i64> = req
                    .get("arguments")
                    .and_then(|a| a.get("breakpoints"))
                    .and_then(Json::as_array)
                    .map(|bps| {
                        bps.iter()
                            .filter_map(|b| b.get("line").and_then(Json::as_i64))
                            .collect()
                    })
                    .unwrap_or_default();
                breakpoints = lines
                    .iter()
                    .filter_map(|l| u32::try_from(*l).ok())
                    .collect();
                let verified: Vec<String> = lines
                    .iter()
                    .map(|l| format!("{{\"verified\":true,\"line\":{l}}}"))
                    .collect();
                writer.borrow_mut().response(
                    seq,
                    "setBreakpoints",
                    &format!("{{\"breakpoints\":[{}]}}", verified.join(",")),
                );
            }
            "configurationDone" => {
                writer
                    .borrow_mut()
                    .response(seq, "configurationDone", "null");
            }
            "launch" => {
                writer.borrow_mut().response(seq, "launch", "null");
                break; // proceed to run
            }
            "disconnect" | "terminate" => {
                writer.borrow_mut().response(seq, command, "null");
                return Ok(());
            }
            other => {
                writer.borrow_mut().response(seq, other, "null");
            }
        }
    }

    // ── Run the program under the debugger. ──
    let frontend = DapFrontend {
        input: Rc::clone(&input),
        writer: Rc::clone(&writer),
    };
    // DAP `launch` runs to the first breakpoint (editor convention), so start in Continue mode.
    let dbg = Debugger::running(breakpoints);
    let session = DebugSession::new(dbg, Box::new(frontend));
    let exit = match crate::interpreter::interpret_debug(&checked, session) {
        Ok((out, code)) => {
            if !out.is_empty() {
                writer.borrow_mut().event(
                    "output",
                    &format!("{{\"category\":\"stdout\",\"output\":\"{}\"}}", esc(&out)),
                );
            }
            code
        }
        Err(mut e) => {
            let src = unit.attribute_frames(&mut e);
            writer.borrow_mut().event(
                "output",
                &format!(
                    "{{\"category\":\"stderr\",\"output\":\"{}\"}}",
                    esc(&e.render(&src))
                ),
            );
            1
        }
    };
    let mut w = writer.borrow_mut();
    w.event("exited", &format!("{{\"exitCode\":{exit}}}"));
    w.event("terminated", "{}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_response_has_content_length_and_json() {
        let mut buf = Vec::new();
        {
            let mut w = Writer {
                out: &mut buf,
                seq: 0,
            };
            w.response(3, "initialize", "{\"ok\":true}");
        }
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with("Content-Length: "), "{s}");
        assert!(s.contains("\r\n\r\n"), "header/body separator: {s}");
        assert!(s.contains("\"request_seq\":3"), "{s}");
        assert!(s.contains("\"command\":\"initialize\""), "{s}");
        assert!(s.contains("\"success\":true"), "{s}");
    }

    #[test]
    fn read_message_round_trips_a_framed_body() {
        let body = "{\"seq\":1,\"command\":\"initialize\"}";
        let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut cur = std::io::Cursor::new(framed.into_bytes());
        let got = read_message(&mut cur).unwrap();
        assert_eq!(got, body);
        assert!(
            read_message(&mut cur).is_none(),
            "EOF after the one message"
        );
    }

    /// Build a Content-Length-framed request stream from raw JSON bodies.
    fn framed(bodies: &[&str]) -> Vec<u8> {
        let mut v = Vec::new();
        for b in bodies {
            v.extend_from_slice(format!("Content-Length: {}\r\n\r\n{}", b.len(), b).as_bytes());
        }
        v
    }

    /// A `'static` `Write` sink backed by a shared buffer the test reads afterward (the DAP frontend
    /// is boxed as `dyn DebugFrontend + 'static`, so the output stream can't be a borrowed `&mut Vec`).
    #[derive(Clone)]
    struct SharedBuf(Rc<RefCell<Vec<u8>>>);
    impl std::io::Write for SharedBuf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn full_session_handshake_launch_stop_inspect_continue_terminate() {
        let unit = crate::loader::load_loose_src(
            "package Main;\nimport Core.Output;\n\
             function main() -> void {\n  int n = 41;\n  int m = n + 1;\n  Output.printLine(\"{m}\");\n}\n",
        )
        .expect("load");
        // A breakpoint at line 5; on stop, ask for stackTrace + variables, then continue.
        let requests = framed(&[
            "{\"seq\":1,\"type\":\"request\",\"command\":\"initialize\"}",
            "{\"seq\":2,\"type\":\"request\",\"command\":\"setBreakpoints\",\"arguments\":{\"breakpoints\":[{\"line\":5}]}}",
            "{\"seq\":3,\"type\":\"request\",\"command\":\"configurationDone\"}",
            "{\"seq\":4,\"type\":\"request\",\"command\":\"launch\"}",
            // paused at line 5 now:
            "{\"seq\":5,\"type\":\"request\",\"command\":\"stackTrace\"}",
            "{\"seq\":6,\"type\":\"request\",\"command\":\"scopes\"}",
            "{\"seq\":7,\"type\":\"request\",\"command\":\"variables\"}",
            "{\"seq\":8,\"type\":\"request\",\"command\":\"continue\"}",
        ]);
        let buf = Rc::new(RefCell::new(Vec::new()));
        run_dap(
            &unit,
            std::io::Cursor::new(requests),
            SharedBuf(buf.clone()),
        )
        .expect("dap session");
        let s = String::from_utf8(buf.borrow().clone()).unwrap();

        assert!(s.contains("\"event\":\"initialized\""), "handshake: {s}");
        assert!(
            s.contains("\"command\":\"setBreakpoints\"") && s.contains("\"verified\":true"),
            "{s}"
        );
        assert!(
            s.contains("\"event\":\"stopped\""),
            "paused at the breakpoint: {s}"
        );
        assert!(
            s.contains("\"command\":\"stackTrace\"") && s.contains("\"name\":\"main\""),
            "{s}"
        );
        // `n` is in scope at the line-5 breakpoint (declared line 4), rendered by the secure renderer.
        assert!(
            s.contains("\"command\":\"variables\"") && s.contains("\"name\":\"n\""),
            "vars: {s}"
        );
        assert!(
            s.contains("\"event\":\"terminated\""),
            "clean termination: {s}"
        );
        // Program stdout is delivered as an output event, not mixed into the protocol stream raw.
        assert!(
            s.contains("\"category\":\"stdout\""),
            "program output event: {s}"
        );
    }
}

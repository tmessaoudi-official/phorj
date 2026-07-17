//! `Core.Native.Input` — the stdin natives behind the `Core.Input` prelude (DEC-281).
//!
//! The read side of the process boundary, `Core.Output`'s twin: piped/redirected data
//! (`cat file | phg run s.phg`, `phg run s.phg < file`) becomes readable. All four natives are
//! `pure: false` (their results depend on the process's stdin, not the program text), so any
//! program touching them is quarantined from the byte-identity differential exactly like
//! `Core.Process` — validated instead by `tests/stdin.rs` on both backends under the override
//! seam below. The PHP legs are REAL (`php://stdin` via the CLI `STDIN` constant), so `Core.Input`
//! programs transpile — unlike the `Core.Native.Session`/DB ladder modules.
//!
//! Two process-global controls (same `RwLock` pattern as `process::PROCESS_ARGS`):
//! - **Override seam** — tests inject a stdin buffer (with a read cursor, since `readLine`
//!   consumes progressively); `phg test`-style embedding stays deterministic.
//! - **Disable flag** — `phg serve` disables stdin before workers run: web input is the
//!   `Request`, and a worker blocking on the terminal's stdin would hang the server. Disabled
//!   reads behave as an already-exhausted pipe (empty / EOF), never an error.
//!
//! Line semantics: `readLine` strips the trailing `\n`/`\r\n` (the PHP leg `rtrim`s identically —
//! byte-identity is preserved by construction, not by matching `fgets`'s keep-the-newline shape)
//! and returns `null` at EOF. `readAll` is a lossy UTF-8 read (invalid sequences → U+FFFD);
//! `readAllBytes` is the exact-bytes escape hatch.

use super::*;
use crate::types::Ty;
use crate::value::Value;
use std::io::Read;
use std::rc::Rc;
use std::sync::RwLock;

/// Test-injected stdin: the buffer plus a cursor (`readLine` consumes progressively).
struct StdinOverride {
    buf: Vec<u8>,
    pos: usize,
}

static STDIN_OVERRIDE: RwLock<Option<StdinOverride>> = RwLock::new(None);

/// True once `phg serve` disabled stdin (workers must never block on the terminal).
static STDIN_DISABLED: RwLock<bool> = RwLock::new(false);

/// Inject (or clear) the stdin the natives read — the `tests/stdin.rs` seam. Resets the cursor.
pub fn set_stdin_override(bytes: Option<Vec<u8>>) {
    if let Ok(mut g) = STDIN_OVERRIDE.write() {
        *g = bytes.map(|buf| StdinOverride { buf, pos: 0 });
    }
}

/// Disable stdin for the rest of the process (`phg serve`): reads behave as an exhausted pipe.
pub fn set_stdin_disabled() {
    if let Ok(mut g) = STDIN_DISABLED.write() {
        *g = true;
    }
}

fn disabled() -> bool {
    STDIN_DISABLED.read().map(|g| *g).unwrap_or(false)
}

/// Drain the remaining override bytes, or `None` when no override is active.
fn override_read_all() -> Option<Vec<u8>> {
    let mut g = STDIN_OVERRIDE.write().ok()?;
    let o = g.as_mut()?;
    let rest = o.buf[o.pos..].to_vec();
    o.pos = o.buf.len();
    Some(rest)
}

/// One override line (cursor to the next `\n`, exclusive), or `Some(None)` at override-EOF, or
/// `None` when no override is active.
#[allow(clippy::option_option)]
fn override_read_line() -> Option<Option<Vec<u8>>> {
    let mut g = STDIN_OVERRIDE.write().ok()?;
    let o = g.as_mut()?;
    if o.pos >= o.buf.len() {
        return Some(None);
    }
    let rest = &o.buf[o.pos..];
    let end = rest
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    let line = rest[..end].to_vec();
    o.pos += end;
    Some(Some(line))
}

/// Strip one trailing `\n` or `\r\n` (the PHP leg's `rtrim($l, "\r\n")` on a single `fgets` line —
/// which carries at most one terminator — is byte-identical).
fn strip_eol(mut line: Vec<u8>) -> Vec<u8> {
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    }
    line
}

fn read_all_bytes() -> Vec<u8> {
    if disabled() {
        return Vec::new();
    }
    if let Some(rest) = override_read_all() {
        return rest;
    }
    let mut buf = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut buf);
    buf
}

fn input_read_all(_args: &[Value], _: &mut String) -> Result<Value, String> {
    Ok(Value::Str(
        String::from_utf8_lossy(&read_all_bytes())
            .into_owned()
            .into(),
    ))
}

fn input_read_all_bytes(_args: &[Value], _: &mut String) -> Result<Value, String> {
    Ok(Value::Bytes(Rc::new(read_all_bytes())))
}

fn input_read_line(_args: &[Value], _: &mut String) -> Result<Value, String> {
    if disabled() {
        return Ok(Value::Null);
    }
    let raw = if let Some(over) = override_read_line() {
        over
    } else {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.into_bytes()),
        }
    };
    Ok(match raw {
        None => Value::Null,
        Some(bytes) => Value::Str(
            String::from_utf8_lossy(&strip_eol(bytes))
                .into_owned()
                .into(),
        ),
    })
}

fn input_is_interactive(_args: &[Value], _: &mut String) -> Result<Value, String> {
    if disabled() || STDIN_OVERRIDE.read().map(|g| g.is_some()).unwrap_or(false) {
        return Ok(Value::Bool(false));
    }
    use std::io::IsTerminal;
    Ok(Value::Bool(std::io::stdin().is_terminal()))
}

pub(crate) fn input_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Native.Input",
            name: "readAll",
            params: vec![],
            ret: Ty::String,
            pure: false,
            eval: NativeEval::Pure(input_read_all),
            // CLI SAPI defines STDIN; a non-CLI context (no stdin) reads as empty.
            php: |_| {
                "(defined('STDIN') ? (($__phorj_in = stream_get_contents(STDIN)) === false ? '' : $__phorj_in) : '')"
                    .to_string()
            },
        },
        NativeFn {
            module: "Core.Native.Input",
            name: "readAllBytes",
            params: vec![],
            ret: Ty::Bytes,
            pure: false,
            eval: NativeEval::Pure(input_read_all_bytes),
            // Phorj `bytes` rides a PHP string — the same raw read serves both.
            php: |_| {
                "(defined('STDIN') ? (($__phorj_in = stream_get_contents(STDIN)) === false ? '' : $__phorj_in) : '')"
                    .to_string()
            },
        },
        NativeFn {
            module: "Core.Native.Input",
            name: "readLine",
            params: vec![],
            ret: Ty::Optional(Box::new(Ty::String)),
            pure: false,
            eval: NativeEval::Pure(input_read_line),
            // EXACTLY one terminator strips (`\n` or `\r\n`) — NOT `rtrim($l, "\r\n")`, which would
            // eat every trailing CR (a line body ending in bare `\r` must survive, matching the
            // Rust strip_eol; PCRE is Tier-1).
            php: |_| {
                "(defined('STDIN') ? (($__phorj_l = fgets(STDIN)) === false ? null : preg_replace(\"/\\r?\\n$/\", '', $__phorj_l)) : null)"
                    .to_string()
            },
        },
        NativeFn {
            module: "Core.Native.Input",
            name: "isInteractive",
            params: vec![],
            ret: Ty::Bool,
            pure: false,
            eval: NativeEval::Pure(input_is_interactive),
            php: |_| {
                "(defined('STDIN') && function_exists('stream_isatty') ? stream_isatty(STDIN) : false)"
                    .to_string()
            },
        },
    ]
}

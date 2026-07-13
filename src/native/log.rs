//! `Core.Log` — structured, leveled application/server logging (DEC-220, the 3-sink output system).
//!
//! The second of the three named output sinks: `Output.*` → STDOUT (the program's output),
//! `Core.Log.*` → **STDERR** (out-of-band, leveled logs), `Response` → the browser. Keeping logs on a
//! separate, explicitly-named sink is the fix for the `phg serve` surprise (a handler's `Output` used to
//! be silently rerouted to stderr as a "log"); with `Core.Log` the code says where bytes go.
//!
//! Each native writes a `[LEVEL] message` line to the process's real stderr — a genuine side effect on
//! the ambient environment, so all are `pure: false`. A program importing `Core.Log` is therefore
//! QUARANTINED from the byte-identity differential (exactly like `Core.Process` / `Core.Runtime`): its
//! stderr is not the compared stdout, and the PHP leg's `error_log` destination need not match. The
//! deterministic part — the `[LEVEL] ` framing — is a pure helper ([`format_line`]) covered by a unit
//! test; the stderr write is the thin side effect. `run ≡ runvm` holds unconditionally (both backends
//! call this one shared body). PHP emission is best-effort-equivalent: `Log.info(m) → error_log("[INFO] "
//! . m)` (the quarantine makes byte-identity moot).

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::Value;
use std::io::Write;

/// The four severity levels, in ascending order. The tag is the uppercase name in brackets.
const LEVELS: [&str; 4] = ["DEBUG", "INFO", "WARN", "ERROR"];

/// Frame a log message: `"[LEVEL] message"` (no trailing newline — the writer adds it). Pure and
/// deterministic; this is the unit-tested part of the sink.
fn format_line(level: &str, msg: &str) -> String {
    format!("[{level}] {msg}")
}

/// Write one framed log line to stderr. The `_out` stdout buffer is intentionally ignored — logs are a
/// distinct sink. A stderr write error is swallowed (a failed log must never abort the program, and
/// stderr being closed/redirected is not a program fault).
fn emit(level: &str, args: &[Value]) -> Result<Value, String> {
    match args {
        [Value::Str(msg)] => {
            let line = format_line(level, msg.as_str());
            let _ = writeln!(std::io::stderr(), "{line}");
            Ok(Value::Unit)
        }
        _ => Err(format!(
            "Log.{} expects (string message)",
            level.to_lowercase()
        )),
    }
}

fn log_debug(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit(LEVELS[0], args)
}
fn log_info(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit(LEVELS[1], args)
}
fn log_warn(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit(LEVELS[2], args)
}
fn log_error(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit(LEVELS[3], args)
}

/// The `Core.Log` registry entries. All `pure: false` — they write to the ambient process stderr, so a
/// program importing this module is quarantined from the byte-identity differential (see the module
/// docs). PHP emission maps to `error_log` with the same `[LEVEL]` framing (best-effort; not compared).
pub(crate) fn log_natives() -> Vec<NativeFn> {
    vec![
        NativeFn {
            module: "Core.Log",
            name: "debug",
            params: vec![Ty::String],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_debug),
            php: |a| {
                format!(
                    "error_log(\"[DEBUG] \" . {})",
                    a.first().map_or("''", |s| s)
                )
            },
        },
        NativeFn {
            module: "Core.Log",
            name: "info",
            params: vec![Ty::String],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_info),
            php: |a| format!("error_log(\"[INFO] \" . {})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.Log",
            name: "warn",
            params: vec![Ty::String],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_warn),
            php: |a| format!("error_log(\"[WARN] \" . {})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.Log",
            name: "error",
            params: vec![Ty::String],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_error),
            php: |a| {
                format!(
                    "error_log(\"[ERROR] \" . {})",
                    a.first().map_or("''", |s| s)
                )
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_each_level() {
        assert_eq!(format_line("INFO", "hello"), "[INFO] hello");
        assert_eq!(format_line("ERROR", "boom"), "[ERROR] boom");
        assert_eq!(format_line("DEBUG", ""), "[DEBUG] ");
    }

    #[test]
    fn emit_returns_unit_and_rejects_bad_arity() {
        assert!(emit("INFO", &[Value::Str("x".into())])
            .unwrap()
            .eq_val(&Value::Unit));
        assert!(emit("INFO", &[]).is_err());
        assert!(emit("INFO", &[Value::Int(1)]).is_err());
    }

    #[test]
    fn every_level_has_an_entry() {
        let ns = log_natives();
        assert_eq!(ns.len(), 4);
        let mut names: Vec<&str> = ns.iter().map(|n| n.name).collect();
        names.sort_unstable();
        assert_eq!(names, ["debug", "error", "info", "warn"]);
        assert!(
            ns.iter().all(|n| !n.pure),
            "Log natives must be pure:false (quarantine seam)"
        );
        assert!(ns.iter().all(|n| n.module == "Core.Log"));
    }
}

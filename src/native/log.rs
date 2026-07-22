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
    emit("DEBUG", args)
}
fn log_info(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("INFO", args)
}
fn log_notice(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("NOTICE", args)
}
fn log_warn(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("WARN", args)
}
fn log_error(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("ERROR", args)
}
fn log_critical(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("CRITICAL", args)
}
fn log_alert(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("ALERT", args)
}
fn log_emergency(args: &[Value], _out: &mut String) -> Result<Value, String> {
    emit("EMERGENCY", args)
}

/// The `Core.Log` registry entries. All `pure: false` — they write to the ambient process stderr, so a
/// program importing this module is quarantined from the byte-identity differential (see the module
/// docs). PHP emission maps to `error_log` with the same `[LEVEL]` framing (best-effort; not compared).
pub(crate) fn log_natives() -> Vec<NativeFn> {
    // One row per level (DEC-317: full PSR-3 set; `warning` = PSR-spelled alias of the historical
    // `warn`). The `php:` closure must be a capture-free fn pointer, so the tag is written literally
    // per row via the macro.
    macro_rules! level {
        ($name:literal, $tag:literal, $eval:expr) => {
            NativeFn {
                module: "Core.Log",
                name: $name,
                params: vec![Ty::String],
                ret: Ty::Void,
                pure: false,
                eval: NativeEval::Pure($eval),
                php: |a| {
                    format!(
                        concat!("error_log(\"[", $tag, "] \" . {})"),
                        a.first().map_or("''", |s| s)
                    )
                },
            }
        };
    }
    vec![
        level!("debug", "DEBUG", log_debug),
        level!("info", "INFO", log_info),
        level!("notice", "NOTICE", log_notice),
        level!("warn", "WARN", log_warn),
        level!("warning", "WARN", log_warn),
        level!("error", "ERROR", log_error),
        level!("critical", "CRITICAL", log_critical),
        level!("alert", "ALERT", log_alert),
        level!("emergency", "EMERGENCY", log_emergency),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The severity levels, ascending (DEC-317: PSR-3; `warn` keeps its historical DEC-220 name,
    /// PSR's `warning` is an alias native). B2's min-level filtering will lift this into the
    /// non-test surface.
    const LEVELS: [&str; 8] = [
        "DEBUG",
        "INFO",
        "NOTICE",
        "WARN",
        "ERROR",
        "CRITICAL",
        "ALERT",
        "EMERGENCY",
    ];

    #[test]
    fn frames_each_level() {
        assert_eq!(format_line("INFO", "hello"), "[INFO] hello");
        assert_eq!(format_line("ERROR", "boom"), "[ERROR] boom");
        assert_eq!(format_line("DEBUG", ""), "[DEBUG] ");
        // The ascending PSR-3 severity order is part of the surface (min-level filtering builds on it).
        assert_eq!(
            LEVELS,
            [
                "DEBUG",
                "INFO",
                "NOTICE",
                "WARN",
                "ERROR",
                "CRITICAL",
                "ALERT",
                "EMERGENCY"
            ]
        );
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
        assert_eq!(ns.len(), 9);
        let mut names: Vec<&str> = ns.iter().map(|n| n.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "alert",
                "critical",
                "debug",
                "emergency",
                "error",
                "info",
                "notice",
                "warn",
                "warning"
            ]
        );
        assert!(
            ns.iter().all(|n| !n.pure),
            "Log natives must be pure:false (quarantine seam)"
        );
        assert!(ns.iter().all(|n| n.module == "Core.Log"));
    }
}

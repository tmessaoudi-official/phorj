//! `Core.Log` — structured, leveled logging (DEC-220 base; DEC-317 Log-v2 channels).
//!
//! The second of the three named output sinks: `Output.*` → STDOUT, `Core.Log.*` → STDERR (or
//! configured handlers), `Response` → the browser. Log-v2 layers Monolog-class routing on top:
//! `Log.configure(cfg)` installs channels (each a list of Stream/File/RotatingFile handlers with a
//! min level + line/json formatter — see `prelude.rs`), `Log.channel("name")` returns a `Channel`
//! handle, and the level statics (`Log.info(..)` …) are the `default` channel. UNCONFIGURED, every
//! surface behaves exactly like DEC-220: `[LEVEL] msg` on stderr.
//!
//! All natives are `pure: false` (ambient stderr/file side effects) → a program importing
//! `Core.Log` is QUARANTINED from the byte-identity differential (like `Core.Process`); the
//! deterministic formatting kernel lives in `state.rs` under unit test, and the PHP leg emits the
//! same contract through the gated `__phorj_log_*` helpers (content parity — `tests/log.rs`).

mod prelude;
mod state;

pub use prelude::PRELUDE;

use super::{NativeEval, NativeFn};
use crate::types::Ty;
use crate::value::{ClassLayout, Instance, Value};
use std::rc::Rc;

fn ty_class(name: &str) -> Ty {
    Ty::Named(name.to_string(), vec![])
}

// ── level statics: the `default` channel ────────────────────────────────────────────────────────

fn level_eval(level: i64, args: &[Value]) -> Result<Value, String> {
    match args {
        [Value::Str(msg)] => state::emit_channel("default", level, msg.as_str()),
        _ => Err(format!(
            "Log.{} expects (string message)",
            state::LEVELS[level as usize].to_lowercase()
        )),
    }
}

macro_rules! level_fn {
    ($fn_name:ident, $ord:expr) => {
        fn $fn_name(args: &[Value], _out: &mut String) -> Result<Value, String> {
            level_eval($ord, args)
        }
    };
}
level_fn!(log_debug, 0);
level_fn!(log_info, 1);
level_fn!(log_notice, 2);
level_fn!(log_warn, 3);
level_fn!(log_error, 4);
level_fn!(log_critical, 5);
level_fn!(log_alert, 6);
level_fn!(log_emergency, 7);

// ── Log-v2 channel natives ───────────────────────────────────────────────────────────────────────

/// `Log.channel(name) -> Logger` — a thin prelude-class handle (the `Regex.compile` carrier
/// pattern: the native builds its own single-slot layout, self-consistent per process). Named
/// `Logger` (the Monolog term) because `Channel` is the concurrency built-in `Channel<T>`.
fn log_channel(args: &[Value], _out: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(name)] => {
            let inst = Instance::new("Logger".into(), ClassLayout::from_sorted_names(&["name"]));
            inst.set_field("name", Value::Str(name.clone()));
            Ok(Value::Instance(Rc::new(inst)))
        }
        _ => Err("Log.channel expects (string name)".into()),
    }
}

/// `Core.Native.Log.emit(channel, levelOrdinal, message)` — the kernel the prelude `Channel`
/// methods call.
fn log_emit(args: &[Value], _out: &mut String) -> Result<Value, String> {
    match args {
        [Value::Str(c), Value::Int(l), Value::Str(m)] => {
            state::emit_channel(c.as_str(), *l, m.as_str())
        }
        _ => Err("Core.Native.Log.emit expects (string, int, string)".into()),
    }
}

/// `Log.configure(cfg: LogConfig)` — extract the pure prelude objects into the native registry
/// (plain data only; see `state.rs`). Shape errors are LOUD here, at configure time.
fn log_configure(args: &[Value], _out: &mut String) -> Result<Value, String> {
    let [Value::Instance(cfg)] = args else {
        return Err("Log.configure expects (LogConfig config)".into());
    };
    let Some(Value::List(channels)) = cfg.get_field("channels") else {
        return Err("Log.configure: LogConfig.channels missing".into());
    };
    let mut extracted: state::ChannelTable = Vec::new();
    for ch in channels.iter() {
        let Value::Instance(cc) = ch else {
            return Err("Log.configure: channels must hold ChannelConfig values".into());
        };
        let name = field_str(cc, "name")?;
        let Some(Value::List(handlers)) = cc.get_field("handlers") else {
            return Err("Log.configure: ChannelConfig.handlers missing".into());
        };
        let mut hs = Vec::new();
        for h in handlers.iter() {
            hs.push(extract_handler(h)?);
        }
        extracted.push((name, hs));
    }
    state::install(extracted);
    Ok(Value::Unit)
}

fn extract_handler(v: &Value) -> Result<state::HandlerCfg, String> {
    let Value::Instance(h) = v else {
        return Err("Log.configure: handlers must hold LogSink values".into());
    };
    let kind = match h.class.as_ref() {
        "StreamHandler" => state::SinkKind::Stream(field_str(h, "stream")?),
        "FileHandler" => state::SinkKind::File(field_str(h, "path")?),
        "RotatingFileHandler" => state::SinkKind::Rotating {
            path: field_str(h, "path")?,
            max_bytes: field_int(h, "maxBytes")?,
            keep: field_int(h, "keep")?,
        },
        other => {
            // The LogSink SPI seam: arbitrary userland sinks are the recorded v2 — refuse loudly
            // rather than silently dropping records (THE LADDER RULE's no-silent-downgrade spirit).
            return Err(format!(
                "Log.configure: unsupported handler class `{other}` (v1 supports StreamHandler/FileHandler/RotatingFileHandler)"
            ));
        }
    };
    Ok(state::HandlerCfg {
        kind,
        min: field_level(h, "minLevel")?,
        format: field_format(h, "formatter")?,
    })
}

/// Read a promoted `Level` field as its wire ordinal (the `state::LEVELS` index).
fn field_level(inst: &Instance, name: &str) -> Result<i64, String> {
    const VARIANTS: [&str; 8] = [
        "Debug",
        "Info",
        "Notice",
        "Warn",
        "Error",
        "Critical",
        "Alert",
        "Emergency",
    ];
    match inst.get_field(name) {
        Some(Value::Enum(e)) if e.ty.as_ref() == "Level" => VARIANTS
            .iter()
            .position(|v| *v == e.variant.as_ref())
            .map(|i| i as i64)
            .ok_or_else(|| format!("Log.configure: unknown Level variant `{}`", e.variant)),
        _ => Err(format!(
            "Log.configure: {}.{name} missing or not a Level",
            inst.class
        )),
    }
}

/// Read a promoted `LogFormatter` field down to its `"line"`/`"json"` kind. v1 recognizes the two
/// built-in formatter classes; a userland `LogFormatter` is refused loudly (the recorded SPI v2).
fn field_format(inst: &Instance, name: &str) -> Result<String, String> {
    match inst.get_field(name) {
        Some(Value::Instance(f)) => match f.class.as_ref() {
            "LineFormatter" => Ok("line".to_string()),
            "JsonFormatter" => Ok("json".to_string()),
            other => Err(format!(
                "Log.configure: unsupported formatter class `{other}` (v1 supports LineFormatter/JsonFormatter)"
            )),
        },
        _ => Err(format!(
            "Log.configure: {}.{name} missing or not a LogFormatter",
            inst.class
        )),
    }
}

fn field_str(inst: &Instance, name: &str) -> Result<String, String> {
    match inst.get_field(name) {
        Some(Value::Str(s)) => Ok(s.as_str().to_string()),
        _ => Err(format!(
            "Log.configure: {}.{name} missing or not a string",
            inst.class
        )),
    }
}

fn field_int(inst: &Instance, name: &str) -> Result<i64, String> {
    match inst.get_field(name) {
        Some(Value::Int(i)) => Ok(i),
        _ => Err(format!(
            "Log.configure: {}.{name} missing or not an int",
            inst.class
        )),
    }
}

// ── registry ─────────────────────────────────────────────────────────────────────────────────────

/// The `Core.Log` + `Core.Native.Log` registry entries. All `pure: false` (quarantine seam). PHP
/// emission routes through the gated `__phorj_log_*` helpers (`transpile/runtime_php.rs`), keeping
/// the formatting contract identical; `Log.channel` maps to plain construction of the transpiled
/// prelude `Channel` class.
pub(crate) fn log_natives() -> Vec<NativeFn> {
    // One row per level (DEC-317: full PSR-3 set; `warning` = PSR-spelled alias of the historical
    // `warn`). The `php:` closure must be a capture-free fn pointer, so the ordinal is written
    // literally per row via the macro.
    macro_rules! level {
        ($name:literal, $ord:literal, $eval:expr) => {
            NativeFn {
                module: "Core.Log",
                name: $name,
                params: vec![Ty::String],
                ret: Ty::Void,
                pure: false,
                eval: NativeEval::Pure($eval),
                lift_from: &[],
                php: |a| {
                    format!(
                        concat!("__phorj_log_emit('default', ", $ord, ", {})"),
                        a.first().map_or("''", |s| s)
                    )
                },
            }
        };
    }
    vec![
        level!("debug", 0, log_debug),
        level!("info", 1, log_info),
        level!("notice", 2, log_notice),
        level!("warn", 3, log_warn),
        level!("warning", 3, log_warn),
        level!("error", 4, log_error),
        level!("critical", 5, log_critical),
        level!("alert", 6, log_alert),
        level!("emergency", 7, log_emergency),
        NativeFn {
            module: "Core.Log",
            name: "channel",
            params: vec![Ty::String],
            ret: ty_class("Logger"),
            pure: false,
            eval: NativeEval::Pure(log_channel),
            lift_from: &[],
            php: |a| format!("new Logger({})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.Log",
            name: "configure",
            params: vec![ty_class("LogConfig")],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_configure),
            lift_from: &[],
            php: |a| format!("__phorj_log_configure({})", a.first().map_or("''", |s| s)),
        },
        NativeFn {
            module: "Core.Native.Log",
            name: "emit",
            params: vec![Ty::String, Ty::Int, Ty::String],
            ret: Ty::Void,
            pure: false,
            eval: NativeEval::Pure(log_emit),
            lift_from: &[],
            php: |a| {
                format!(
                    "__phorj_log_emit({}, {}, {})",
                    a.first().map_or("''", |s| s),
                    a.get(1).map_or("0", |s| s),
                    a.get(2).map_or("''", |s| s)
                )
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_statics_route_the_default_channel_and_reject_bad_arity() {
        assert!(level_eval(1, &[Value::Str("x".into())])
            .unwrap()
            .eq_val(&Value::Unit));
        assert!(level_eval(1, &[]).is_err());
        assert!(level_eval(1, &[Value::Int(1)]).is_err());
    }

    #[test]
    fn registry_has_every_level_plus_the_channel_machinery() {
        let ns = log_natives();
        assert_eq!(ns.len(), 12);
        let mut names: Vec<&str> = ns.iter().map(|n| n.name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "alert",
                "channel",
                "configure",
                "critical",
                "debug",
                "emergency",
                "emit",
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
        assert!(ns
            .iter()
            .all(|n| n.module == "Core.Log" || n.module == "Core.Native.Log"));
    }

    #[test]
    fn channel_native_builds_the_carrier_instance() {
        let v = log_channel(&[Value::Str("payments".into())], &mut String::new()).unwrap();
        match v {
            Value::Instance(inst) => {
                assert_eq!(inst.class.as_ref(), "Logger");
                assert!(matches!(
                    inst.get_field("name"),
                    Some(Value::Str(s)) if s.as_str() == "payments"
                ));
            }
            other => panic!("expected Logger instance, got {other:?}"),
        }
    }
}

//! Log-v2 (DEC-317) — the channel REGISTRY + emit kernel.
//!
//! `Log.configure(cfg)` walks the pure prelude config objects ONCE and stores PLAIN DATA here (the
//! `Core.SessionModule` in-process-store precedent — never phorj `Value`s, so the global is
//! `Send + Sync` and backend-agnostic). Every log line then flows through [`emit_channel`]:
//! filter (min-level ordinal) → format (`line`/`json`, both DETERMINISTIC — no timestamps in v1, so
//! the emitted CONTENT is parity-testable against the PHP `__phorj_log_*` helpers) → write
//! (stderr/stdout stream, plain append file, or size-rotated file). An UNCONFIGURED program (or an
//! unknown channel name) falls back to the DEC-220 behavior byte-for-byte: `[LEVEL] msg` on stderr.
//!
//! Write errors are swallowed (a failed log must never abort the program — the DEC-220 stance);
//! configuration SHAPE errors (unknown handler/formatter kind) are loud `Err`s at `configure` time.

use crate::value::Value;
use std::io::Write;
use std::sync::Mutex;

/// The severity levels, ascending (PSR-3; `warn` keeps its historical DEC-220 name). The index IS
/// the wire ordinal the prelude's `Levels.ord` produces — the two tables must stay aligned.
pub(super) const LEVELS: [&str; 8] = [
    "DEBUG",
    "INFO",
    "NOTICE",
    "WARN",
    "ERROR",
    "CRITICAL",
    "ALERT",
    "EMERGENCY",
];

/// One configured handler, as plain data extracted from the prelude objects.
pub(super) struct HandlerCfg {
    pub kind: SinkKind,
    /// Minimum level ordinal (indexes [`LEVELS`]); records below it are dropped by this handler.
    pub min: i64,
    /// `"line"` or `"json"`.
    pub format: String,
    /// DEC-329.4: append the OUT-OF-CONTRACT processor tail (`| ts=<epoch-ms> pid=<pid>` on line,
    /// trailing `"ts"`/`"pid"` keys on json). Env-dependent by nature — parity tests strip it; the
    /// deterministic prefix stays the byte-identity contract (the FS message-tail precedent).
    pub process_info: bool,
}

pub(super) enum SinkKind {
    /// `"stderr"` or `"stdout"`.
    Stream(String),
    /// Plain append-to-path.
    File(String),
    /// Append with size-based rotation: at `max_bytes`, shift `p → p.1 → … → p.keep` (oldest dropped).
    Rotating {
        path: String,
        max_bytes: i64,
        keep: i64,
    },
}

/// channel name → its handlers, in configuration order.
pub(super) type ChannelTable = Vec<(String, Vec<HandlerCfg>)>;

/// The installed table. `None` = unconfigured (the DEC-220 fallback).
static CHANNELS: Mutex<Option<ChannelTable>> = Mutex::new(None);

/// Reset to UNCONFIGURED (audit 2026-07-22, P2): the table is process-global, so a second program
/// run in the same process (tests, playground-style hosts) must not inherit the previous program's
/// channels. Called at every `cmd_run`/`cmd_treewalk` entry.
pub fn reset() {
    let mut guard = CHANNELS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = None;
}

/// Install the configuration extracted by `Log.configure` (replaces any prior one).
pub(super) fn install(channels: ChannelTable) {
    let mut guard = CHANNELS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(channels);
}

/// Frame the DEC-220 fallback line: `[LEVEL] msg`.
pub(super) fn format_line_default(level: &str, msg: &str) -> String {
    format!("[{level}] {msg}")
}

/// The `line` format: the DEC-220 frame for the `default` channel (back-compat byte-for-byte),
/// `[LEVEL] name: msg` for a named channel.
fn format_line(channel: &str, level: &str, msg: &str) -> String {
    if channel == "default" {
        format_line_default(level, msg)
    } else {
        format!("[{level}] {channel}: {msg}")
    }
}

/// The `json` format — fixed key order, minimal escaper (`"`/`\`/`\n`/`\r`/`\t`, other control
/// chars as `\u00XX`). Hand-rolled IDENTICALLY in the PHP `__phorj_log_json` helper (PHP's
/// `json_encode` escapes `/` and unicode differently — never use it for this contract).
fn format_json(channel: &str, level: &str, msg: &str) -> String {
    format!(
        "{{\"channel\":\"{}\",\"level\":\"{}\",\"message\":\"{}\"}}",
        json_escape(channel),
        level,
        json_escape(msg)
    )
}

fn json_escape(s: &str) -> String {
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

/// The emit kernel: route one record through the configured handlers of `channel` (or the DEC-220
/// stderr fallback when unconfigured / unknown). `level` is the [`LEVELS`] ordinal. `out` is the
/// program's captured STDOUT buffer — a `stdout` stream handler writes THERE (audit 2026-07-22,
/// P0): writing to the real process stdout would misorder log lines against `Output.*` (buffered,
/// flushed at exit) on run/runvm while the PHP leg interleaves naturally.
pub(super) fn emit_channel(
    channel: &str,
    level: i64,
    msg: &str,
    out: &mut String,
) -> Result<Value, String> {
    let tag = LEVELS
        .get(usize::try_from(level).unwrap_or(usize::MAX))
        .ok_or_else(|| format!("Core.Log: level ordinal {level} out of range"))?;
    let guard = CHANNELS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let handlers = guard
        .as_ref()
        .and_then(|chans| chans.iter().find(|(n, _)| n == channel))
        .map(|(_, hs)| hs);
    match handlers {
        None => {
            // Unconfigured (or unknown channel): the DEC-220 default — `[LEVEL] msg` on stderr.
            let _ = writeln!(std::io::stderr(), "{}", format_line(channel, tag, msg));
        }
        Some(hs) => {
            for h in hs {
                if level < h.min {
                    continue;
                }
                let mut line = if h.format == "json" {
                    format_json(channel, tag, msg)
                } else {
                    format_line(channel, tag, msg)
                };
                if h.process_info {
                    let ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis());
                    let pid = std::process::id();
                    if h.format == "json" {
                        line.truncate(line.len() - 1); // drop the closing `}`
                        line.push_str(&format!(",\"ts\":{ms},\"pid\":{pid}}}"));
                    } else {
                        line.push_str(&format!(" | ts={ms} pid={pid}"));
                    }
                }
                write_sink(&h.kind, &line, out);
            }
        }
    }
    Ok(Value::Unit)
}

/// Write one formatted line (plus `\n`) to a sink. Errors are swallowed (see module docs).
fn write_sink(kind: &SinkKind, line: &str, out: &mut String) {
    match kind {
        SinkKind::Stream(s) if s == "stdout" => {
            out.push_str(line);
            out.push('\n');
        }
        SinkKind::Stream(_) => {
            let _ = writeln!(std::io::stderr(), "{line}");
        }
        SinkKind::File(path) => append_line(path, line),
        SinkKind::Rotating {
            path,
            max_bytes,
            keep,
        } => {
            rotate_if_needed(path, *max_bytes, *keep);
            append_line(path, line);
        }
    }
}

fn append_line(path: &str, line: &str) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

/// Size-based rotation: when `path` is at/over `max_bytes`, shift `p.(keep-1) → p.keep` … `p → p.1`
/// (the highest suffix falls off). `keep <= 0` truncates instead of keeping history.
fn rotate_if_needed(path: &str, max_bytes: i64, keep: i64) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if max_bytes <= 0 || (meta.len() as i64) < max_bytes {
        return;
    }
    if keep <= 0 {
        let _ = std::fs::remove_file(path);
        return;
    }
    let _ = std::fs::remove_file(format!("{path}.{keep}"));
    let mut i = keep - 1;
    while i >= 1 {
        let _ = std::fs::rename(format!("{path}.{i}"), format!("{path}.{}", i + 1));
        i -= 1;
    }
    let _ = std::fs::rename(path, format!("{path}.1"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_format_keeps_dec220_default_and_prefixes_named_channels() {
        assert_eq!(format_line("default", "INFO", "hi"), "[INFO] hi");
        assert_eq!(
            format_line("payments", "WARN", "late"),
            "[WARN] payments: late"
        );
    }

    #[test]
    fn json_format_is_deterministic_and_minimally_escaped() {
        assert_eq!(
            format_json("app", "ERROR", "a\"b\\c\nd"),
            "{\"channel\":\"app\",\"level\":\"ERROR\",\"message\":\"a\\\"b\\\\c\\nd\"}"
        );
        assert_eq!(json_escape("\u{1}"), "\\u0001");
    }

    #[test]
    fn rotation_shifts_suffixes_and_drops_the_oldest() {
        let dir = std::env::temp_dir().join(format!("phorj-logrot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("app.log");
        let ps = p.to_str().unwrap();
        std::fs::write(&p, b"0123456789").unwrap();
        std::fs::write(format!("{ps}.1"), b"old1").unwrap();
        std::fs::write(format!("{ps}.2"), b"old2").unwrap();
        rotate_if_needed(ps, 10, 2);
        assert!(!p.exists(), "live file rotated away");
        assert_eq!(std::fs::read(format!("{ps}.1")).unwrap(), b"0123456789");
        assert_eq!(std::fs::read(format!("{ps}.2")).unwrap(), b"old1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn emit_unconfigured_is_total_and_bounds_checked() {
        assert!(emit_channel("default", 1, "x", &mut String::new()).is_ok());
        assert!(emit_channel("default", 99, "x", &mut String::new()).is_err());
    }
}

//! The `Core.Log` prelude (DEC-317 Log-v2) — the PURE configuration/handle surface over the
//! `Core.Log` + `Core.Native.Log` natives.
//!
//! Design (SLICE-STATE 2026-07-22, "config-data-in-Rust, objects-in-prelude"): these classes are
//! plain phorj data the user constructs in a `#[Config]` provider (DEC-318 synergy); `Log.configure`
//! extracts them ONCE into the native registry (`state.rs`), and every record then flows through the
//! `log_emit` kernel. handlers carry `Level` + formatter values promoted; the ordinal/kind extraction happens at
//! `Log.configure` time (Rust reads the enum variant + built-in formatter class directly; the PHP
//! helper maps the enum-scoped variant CLASS name — `Level_Error` etc. (DEC-329.3) — and calls `kind()`).
//!
//! `LogFormatter`/`LogSink` are real interfaces — the recorded SPI seam. v1 accepts ONLY the
//! built-in formatter/handler classes: `Log.configure` refuses userland implementations LOUDLY on
//! every leg (see KNOWN_ISSUES §Log-v2 v1 limits); opening the seam is the recorded v2.

pub const PRELUDE: &str = r#"
import Core.Native.Log as NativeLog;

enum Level { Debug(), Info(), Notice(), Warn(), Error(), Critical(), Alert(), Emergency() }

interface LogFormatter { function kind(): string; }
class LineFormatter implements LogFormatter {
  constructor(public bool processInfo = false) {}
  function kind(): string { return "line"; }
}
class JsonFormatter implements LogFormatter {
  constructor(public bool processInfo = false) {}
  function kind(): string { return "json"; }
}

interface LogSink { function sinkKind(): string; }

class StreamHandler implements LogSink {
  constructor(public string stream, public Level minLevel, public LogFormatter formatter) {}
  function sinkKind(): string { return "stream"; }
}

class FileHandler implements LogSink {
  constructor(public string path, public Level minLevel, public LogFormatter formatter) {}
  function sinkKind(): string { return "file"; }
}

class RotatingFileHandler implements LogSink {
  constructor(public string path, public int maxBytes, public int keep, public Level minLevel, public LogFormatter formatter) {}
  function sinkKind(): string { return "rotating"; }
}

class ChannelConfig {
  constructor(public string name, public List<LogSink> handlers) {}
}

class LogConfig {
  constructor(public List<ChannelConfig> channels) {}
}

class Logger {
  constructor(public string name) {}
  function debug(string msg): void { NativeLog.emit(this.name, 0, msg); }
  function info(string msg): void { NativeLog.emit(this.name, 1, msg); }
  function notice(string msg): void { NativeLog.emit(this.name, 2, msg); }
  function warn(string msg): void { NativeLog.emit(this.name, 3, msg); }
  function warning(string msg): void { NativeLog.emit(this.name, 3, msg); }
  function error(string msg): void { NativeLog.emit(this.name, 4, msg); }
  function critical(string msg): void { NativeLog.emit(this.name, 5, msg); }
  function alert(string msg): void { NativeLog.emit(this.name, 6, msg); }
  function emergency(string msg): void { NativeLog.emit(this.name, 7, msg); }
}
"#;

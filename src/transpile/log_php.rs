//! PHP transpiler — the `__phorj_log_*` runtime helpers (DEC-317 Log-v2), gated by `uses_log`.
//!
//! Hand-rolled to the SAME deterministic contract as the Rust kernel (`src/native/log/state.rs`):
//! channel config as plain data in `$GLOBALS['__phorj_log']`, min-level ordinal filtering, the
//! `line` frame (`[TAG] msg` for `default`, `[TAG] chan: msg` otherwise) and the fixed-key `json`
//! frame with the minimal escaper (NEVER `json_encode` — it escapes `/` and unicode differently).
//! The module is impure (quarantined from the byte-identity differential); the CONTENT contract is
//! parity-gated by `tests/log.rs`.

use super::*;

impl Transpiler {
    pub(super) fn emit_log_helpers(&mut self) {
        if !self.uses_log {
            return;
        }
        self.line("function __phorj_log_configure($cfg) {");
        self.indent += 1;
        self.line("$chans = [];");
        self.line("foreach ($cfg->channels as $cc) {");
        self.indent += 1;
        self.line("$hs = [];");
        self.line("foreach ($cc->handlers as $h) {");
        self.indent += 1;
        self.line("$k = $h->sinkKind();");
        self.line("$min = __phorj_log_ord($h->minLevel);");
        self.line("$fmt = $h->formatter->kind();");
        self.line("if ($k === 'stream') { $hs[] = ['stream', $h->stream, $min, $fmt, 0, 0]; }");
        self.line("elseif ($k === 'file') { $hs[] = ['file', $h->path, $min, $fmt, 0, 0]; }");
        self.line("else { $hs[] = ['rotating', $h->path, $min, $fmt, $h->maxBytes, $h->keep]; }");
        self.indent -= 1;
        self.line("}");
        self.line("$chans[$cc->name] = $hs;");
        self.indent -= 1;
        self.line("}");
        self.line("$GLOBALS['__phorj_log'] = $chans;");
        self.line("return null;");
        self.indent -= 1;
        self.line("}");
        // Level variant → ordinal, by variant CLASS basename. Mangling-aware: `Error` is a PHP
        // builtin class, so its variant class transpiles as `Error_` (enum-reserved-variants rule).
        self.line("function __phorj_log_ord($lvl) {");
        self.indent += 1;
        self.line("$n = substr(strrchr('\\\\' . get_class($lvl), '\\\\'), 1);");
        self.line("$m = ['Debug'=>0,'Info'=>1,'Notice'=>2,'Warn'=>3,'Error'=>4,'Error_'=>4,'Critical'=>5,'Alert'=>6,'Emergency'=>7];");
        self.line("return $m[$n];");
        self.indent -= 1;
        self.line("}");
        // The minimal escaper — byte-wise, so multi-byte UTF-8 passes through unchanged exactly like
        // the Rust char-wise version (controls are single-byte; everything else is copied verbatim).
        self.line("function __phorj_log_json_escape($s) {");
        self.indent += 1;
        self.line("$out = '';");
        self.line("$len = strlen($s);");
        self.line("for ($i = 0; $i < $len; $i++) {");
        self.indent += 1;
        self.line("$c = $s[$i]; $o = ord($c);");
        self.line("if ($c === '\"') { $out .= '\\\\\"'; }");
        self.line("elseif ($c === '\\\\') { $out .= '\\\\\\\\'; }");
        self.line("elseif ($c === \"\\n\") { $out .= '\\\\n'; }");
        self.line("elseif ($c === \"\\r\") { $out .= '\\\\r'; }");
        self.line("elseif ($c === \"\\t\") { $out .= '\\\\t'; }");
        self.line("elseif ($o < 0x20) { $out .= sprintf('\\\\u%04x', $o); }");
        self.line("else { $out .= $c; }");
        self.indent -= 1;
        self.line("}");
        self.line("return $out;");
        self.indent -= 1;
        self.line("}");
        self.line("function __phorj_log_fmt($chan, $tag, $msg, $format) {");
        self.indent += 1;
        self.line("if ($format === 'json') {");
        self.indent += 1;
        self.line("return '{\"channel\":\"' . __phorj_log_json_escape($chan) . '\",\"level\":\"' . $tag . '\",\"message\":\"' . __phorj_log_json_escape($msg) . '\"}';");
        self.indent -= 1;
        self.line("}");
        self.line("return ($chan === 'default') ? \"[$tag] $msg\" : \"[$tag] $chan: $msg\";");
        self.indent -= 1;
        self.line("}");
        self.line("function __phorj_log_rotate($path, $max, $keep) {");
        self.indent += 1;
        self.line("if (!file_exists($path) || $max <= 0 || filesize($path) < $max) { return; }");
        self.line("if ($keep <= 0) { @unlink($path); return; }");
        self.line("@unlink($path . '.' . $keep);");
        self.line("for ($i = $keep - 1; $i >= 1; $i--) {");
        self.indent += 1;
        self.line("if (file_exists($path . '.' . $i)) { @rename($path . '.' . $i, $path . '.' . ($i + 1)); }");
        self.indent -= 1;
        self.line("}");
        self.line("@rename($path, $path . '.1');");
        self.indent -= 1;
        self.line("}");
        self.line("function __phorj_log_write($h, $line) {");
        self.indent += 1;
        self.line("if ($h[0] === 'stream') { fwrite($h[1] === 'stdout' ? STDOUT : STDERR, $line . \"\\n\"); return; }");
        self.line("$path = $h[1];");
        self.line("if ($h[0] === 'rotating') { __phorj_log_rotate($path, $h[4], $h[5]); }");
        self.line("$dir = dirname($path);");
        self.line(
            "if ($dir !== '' && $dir !== '.' && !is_dir($dir)) { @mkdir($dir, 0777, true); }",
        );
        self.line("@file_put_contents($path, $line . \"\\n\", FILE_APPEND);");
        self.indent -= 1;
        self.line("}");
        self.line("function __phorj_log_emit($chan, $level, $msg) {");
        self.indent += 1;
        self.line(
            "$tags = ['DEBUG','INFO','NOTICE','WARN','ERROR','CRITICAL','ALERT','EMERGENCY'];",
        );
        self.line("$tag = $tags[$level];");
        self.line("$cfg = $GLOBALS['__phorj_log'] ?? null;");
        self.line("$hs = ($cfg !== null && array_key_exists($chan, $cfg)) ? $cfg[$chan] : null;");
        self.line("if ($hs === null) { fwrite(STDERR, __phorj_log_fmt($chan, $tag, $msg, 'line') . \"\\n\"); return null; }");
        self.line("foreach ($hs as $h) {");
        self.indent += 1;
        self.line("if ($level < $h[2]) { continue; }");
        self.line("__phorj_log_write($h, __phorj_log_fmt($chan, $tag, $msg, $h[3]));");
        self.indent -= 1;
        self.line("}");
        self.line("return null;");
        self.indent -= 1;
        self.line("}");
    }
}

//! PHP transpiler — the `__phorj_http_*` runtime helper family (DEC-331 slice 2, gated by
//! `uses_http`; M-Decomp sibling of `runtime_php.rs`). Each helper mirrors its Rust twin in
//! `src/native/http/` byte-for-byte — decode fallback rules, first-wins accumulation, the
//! multipart acceptance rules (incl. the boundary-guarded `name="…"` lookup), and the stash
//! contract (`-2` over `DEFAULT_MAX_BODY_SIZE` / `-1` inline at-or-under `SPILL_THRESHOLD` /
//! a sequential spill handle otherwise). Keep BOTH sides in lockstep or the differential breaks.
use super::Transpiler;

impl Transpiler {
    pub(super) fn emit_http_runtime_helpers(&mut self) {
        if !self.uses_http {
            return;
        }
        // Component decode: '+'→space (form only), %XX (exactly two hex), invalid escape literal,
        // whole-component fallback to the UNDECODED original when the result is not valid UTF-8.
        self.line("function __phorj_http_pct_decode($s, $plusIsSpace) {");
        self.indent += 1;
        self.line("$out = ''; $n = strlen($s); $i = 0;");
        self.line("while ($i < $n) { $c = $s[$i];");
        self.indent += 1;
        self.line("if ($c === '+' && $plusIsSpace) { $out .= ' '; $i += 1; }");
        // PCRE (tier-1), not ctype_xdigit — the ctype shared extension may be absent under `php -n`.
        self.line("elseif ($c === '%' && preg_match('/^[0-9A-Fa-f]{2}$/', substr($s, $i + 1, 2)) === 1) { $out .= chr(hexdec(substr($s, $i + 1, 2))); $i += 3; }");
        self.line("else { $out .= $c; $i += 1; }");
        self.indent -= 1;
        self.line("}");
        self.line("return preg_match('//u', $out) === 1 ? $out : $s;");
        self.indent -= 1;
        self.line("}");
        self.line(
            "function __phorj_http_decode_path($s) { return __phorj_http_pct_decode($s, false); }",
        );
        // First-wins key order, duplicate values appended; FIRST '=' splits; empty segments skipped.
        self.line("function __phorj_http_parse_query($s) {");
        self.indent += 1;
        self.line("$out = [];");
        self.line("foreach (explode('&', $s) as $seg) {");
        self.indent += 1;
        self.line("if ($seg === '') { continue; }");
        self.line("$eq = strpos($seg, '=');");
        self.line(
            "$k = __phorj_http_pct_decode($eq === false ? $seg : substr($seg, 0, $eq), true);",
        );
        self.line(
            "$v = __phorj_http_pct_decode($eq === false ? '' : substr($seg, $eq + 1), true);",
        );
        self.line("if (!array_key_exists($k, $out)) { $out[$k] = []; }");
        self.line("$out[$k][] = $v;");
        self.indent -= 1;
        self.line("}");
        self.line("return $out;");
        self.indent -= 1;
        self.line("}");
        // Multipart split — the exact acceptance rules of src/native/http/multipart.rs.
        self.line("function __phorj_http_parse_multipart($body, $boundary) {");
        self.indent += 1;
        self.line("if ($boundary === '') { return null; }");
        self.line("$open = '--' . $boundary; $delim = \"\\r\\n--\" . $boundary;");
        self.line("if (!str_starts_with($body, $open)) { return null; }");
        self.line("$parts = []; $cur = strlen($open);");
        self.line("while (true) {");
        self.indent += 1;
        self.line("if (substr($body, $cur, 2) === '--') { return $parts; }");
        self.line("if (substr($body, $cur, 2) !== \"\\r\\n\") { return null; }");
        self.line("$hs = $cur + 2; $he = strpos($body, \"\\r\\n\\r\\n\", $hs);");
        self.line("if ($he === false) { return null; }");
        self.line("$cs = $he + 4; $ce = strpos($body, $delim, $cs);");
        self.line("if ($ce === false) { return null; }");
        self.line("$name = null; $file = ''; $ctype = '';");
        self.line("foreach (explode(\"\\r\\n\", substr($body, $hs, $he - $hs)) as $line) {");
        self.indent += 1;
        self.line("$ci = strpos($line, ':'); if ($ci === false) { continue; }");
        self.line(
            "$key = strtolower(trim(substr($line, 0, $ci))); $val = trim(substr($line, $ci + 1));",
        );
        self.line("if ($key === 'content-disposition') {");
        self.indent += 1;
        // The (?:^|[;\s]) guard = the Rust boundary-char rule (never read `filename` as `name`).
        self.line(
            "if (preg_match('/(?:^|[;\\s])name=\"([^\"]*)\"/', $val, $m)) { $name = $m[1]; }",
        );
        self.line(
            "if (preg_match('/(?:^|[;\\s])filename=\"([^\"]*)\"/', $val, $m)) { $file = $m[1]; }",
        );
        self.indent -= 1;
        self.line("} elseif ($key === 'content-type') { $ctype = $val; }");
        self.indent -= 1;
        self.line("}");
        self.line("if ($name === null) { return null; }");
        self.line(
            "$parts[] = new MultipartPart($name, $file, $ctype, substr($body, $cs, $ce - $cs));",
        );
        self.line("if (count($parts) > 1024) { return null; }");
        self.line("$cur = $ce + strlen($delim);");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        // Stash contract: -2 over the body cap; -1 inline at/under the spill threshold; else a
        // sequential handle into the spill array (the PATH never reaches phorj values).
        self.line("function __phorj_http_stash_body($b) {");
        self.indent += 1;
        self.line("global $__phorj_http_spills;");
        self.line("if (strlen($b) > 8388608) { return -2; }");
        self.line("if (strlen($b) <= 262144) { return -1; }");
        self.line("$p = tempnam(sys_get_temp_dir(), 'phorj-spill-');");
        self.line("if ($p === false || file_put_contents($p, $b) === false) { throw new \\RuntimeException('request body spill failed'); }");
        self.line("$__phorj_http_spills[] = $p;");
        self.line("return count($__phorj_http_spills) - 1;");
        self.indent -= 1;
        self.line("}");
        self.line("function __phorj_http_read_spill($h) {");
        self.indent += 1;
        self.line("global $__phorj_http_spills;");
        self.line("if (!isset($__phorj_http_spills[$h])) { throw new \\RuntimeException('invalid spill handle'); }");
        self.line("$b = file_get_contents($__phorj_http_spills[$h]);");
        self.line(
            "if ($b === false) { throw new \\RuntimeException('request body spill read failed'); }",
        );
        self.line("return $b;");
        self.indent -= 1;
        self.line("}");
        // json hand-off: invalid UTF-8 → null (mirrors the Rust twin), else the one Json parser.
        self.line("function __phorj_http_json_parse($b) {");
        self.indent += 1;
        self.line("return preg_match('//u', $b) === 1 ? __phorj_json_decode($b) : null;");
        self.indent -= 1;
        self.line("}");
    }
}

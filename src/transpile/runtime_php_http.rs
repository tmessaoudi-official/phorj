//! PHP transpiler — the `__phorj_http_*` runtime helper family (DEC-331 slice 2, gated by
//! `uses_http`; M-Decomp sibling of `runtime_php.rs`). Each helper mirrors its Rust twin in
//! `src/native/http/` byte-for-byte — decode fallback rules, first-wins accumulation, the
//! multipart acceptance rules (incl. the boundary-guarded `name="…"` lookup), and the stash
//! contract (`-2` over `DEFAULT_MAX_BODY_SIZE` / `-1` inline at-or-under `SPILL_THRESHOLD` /
//! a sequential spill handle otherwise). Keep BOTH sides in lockstep or the differential breaks.
use super::Transpiler;

impl Transpiler {
    pub(super) fn emit_http_runtime_helpers(&mut self) {
        if !self.gates.uses_http {
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
        self.line("$head = substr($body, $hs, $he - $hs);");
        // F1: the Rust twin gates the head on valid UTF-8 (`from_utf8().ok()?`) — mirror it, or a
        // non-UTF-8 part header would parse on PHP and 400 on interp/VM (byte-identity break).
        self.line("if (preg_match('//u', $head) !== 1) { return null; }");
        self.line("$name = null; $file = ''; $ctype = '';");
        self.line("foreach (explode(\"\\r\\n\", $head) as $line) {");
        self.indent += 1;
        self.line("$ci = strpos($line, ':'); if ($ci === false) { continue; }");
        // F2b: OWS trim = space + htab ONLY (RFC 7230), matching the Rust `ows_trim` — NOT PHP
        // `trim()` (which also strips \n\r\0\x0B) nor Rust `str::trim()` (Unicode whitespace).
        self.line(
            "$key = strtolower(trim(substr($line, 0, $ci), \" \\t\")); $val = trim(substr($line, $ci + 1), \" \\t\");",
        );
        self.line("if ($key === 'content-disposition') {");
        self.indent += 1;
        // F2a: the boundary char class is space/htab/`;` ONLY (the Rust `matches!` arm), NOT PCRE
        // `\s` (which also matches \n\r\f\v).
        self.line(
            "if (preg_match('/(?:^|[; \\t])name=\"([^\"]*)\"/', $val, $m)) { $name = $m[1]; }",
        );
        self.line(
            "if (preg_match('/(?:^|[; \\t])filename=\"([^\"]*)\"/', $val, $m)) { $file = $m[1]; }",
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
        self.emit_http_parse_request();
    }

    /// DEC-338: the whole wire→`Request` parse (`__phorj_http_parse_request`), the twin of the Rust
    /// native in `src/native/http/request.rs`. Builds the IDENTICAL object graph — the (unchanged)
    /// transpiled prelude classes `Request`/`ParamBag`/`HeaderBag`/`AttrBag`/`FileBag`/`RequestBody`/
    /// `UploadedFile`, constructed positionally in their declared order. `null` = malformed/oversize
    /// (the eager D8a contract). Self-contained: the nativized parse path no longer calls
    /// `String.trim`, so `__phorj_text_trim` may not be emitted — `__phorj_http_trim` carries the same
    /// Unicode White_Space class (SSOT: the `WS` const in `runtime_php.rs`, byte-identical to Rust
    /// `str::trim`). `String.indexOf` is inlined as `strpos(...) === false`.
    fn emit_http_parse_request(&mut self) {
        // SINGLE-SOURCED with `__phorj_text_trim` (Invariant 4): the one WS class const in runtime_php.rs.
        // The nativized parse path no longer emits `String.trim`, so this helper is self-contained — but
        // its char class is NOT a copy: it derives from the shared const so PHP-leg trim parity can't drift.
        const WS: &str = super::PHP_TRIM_WS;
        self.line(&format!(
            "function __phorj_http_trim($s) {{ return preg_replace('/^{WS}+|{WS}+$/u', '', $s); }}"
        ));
        // headerPairs: lowercased-key bag, values trimmed, first-wins order.
        self.line("function __phorj_http_header_pairs($lines) {");
        self.indent += 1;
        self.line("$out = [];");
        self.line("foreach ($lines as $line) {");
        self.indent += 1;
        self.line("$ci = strpos($line, ':'); if ($ci === false) { continue; }");
        self.line("$key = strtolower(__phorj_http_trim(substr($line, 0, $ci)));");
        self.line("$val = __phorj_http_trim(substr($line, $ci + 1));");
        self.line("if (!array_key_exists($key, $out)) { $out[$key] = []; }");
        self.line("$out[$key][] = $val;");
        self.indent -= 1;
        self.line("}");
        self.line("return $out;");
        self.indent -= 1;
        self.line("}");
        // cookiePairs: every `cookie` header, `;`-split, FIRST `=`, names case-SENSITIVE, verbatim.
        self.line("function __phorj_http_cookie_pairs($headerMap) {");
        self.indent += 1;
        self.line("$out = [];");
        self.line("foreach ($headerMap['cookie'] ?? [] as $line) {");
        self.indent += 1;
        self.line("foreach (explode(';', $line) as $piece) {");
        self.indent += 1;
        self.line("$p = __phorj_http_trim($piece); if ($p === '') { continue; }");
        self.line("$eq = strpos($p, '=');");
        self.line("if ($eq === false) { $k = $p; $v = ''; } else { $k = substr($p, 0, $eq); $v = substr($p, $eq + 1); }");
        self.line("if (!array_key_exists($k, $out)) { $out[$k] = []; }");
        self.line("$out[$k][] = $v;");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("return $out;");
        self.indent -= 1;
        self.line("}");
        // multipartFields: non-file parts fold into the form bag (values verbatim, first-wins).
        self.line("function __phorj_http_multipart_fields($parts) {");
        self.indent += 1;
        self.line("$out = [];");
        self.line("foreach ($parts as $p) {");
        self.indent += 1;
        self.line("if ($p->fileName === '') {");
        self.indent += 1;
        self.line("$val = preg_match('//u', $p->content) === 1 ? $p->content : '';");
        self.line("if (!array_key_exists($p->name, $out)) { $out[$p->name] = []; }");
        self.line("$out[$p->name][] = $val;");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("return $out;");
        self.indent -= 1;
        self.line("}");
        // boundaryOf: the `boundary=` parameter of a multipart content-type ('' when absent).
        self.line("function __phorj_http_boundary_of($ct) {");
        self.indent += 1;
        self.line("$b = strpos($ct, 'boundary='); if ($b === false) { return ''; }");
        self.line("$rest = substr($ct, $b + 9);");
        self.line("if (str_starts_with($rest, '\"')) {");
        self.indent += 1;
        self.line("$inner = substr($rest, 1); $q = strpos($inner, '\"');");
        self.line("return $q === false ? '' : substr($inner, 0, $q);");
        self.indent -= 1;
        self.line("}");
        self.line("$semi = strpos($rest, ';');");
        self.line("return $semi === false ? __phorj_http_trim($rest) : __phorj_http_trim(substr($rest, 0, $semi));");
        self.indent -= 1;
        self.line("}");
        // The eager wire parse — the twin of `request::parse_request`.
        self.line("function __phorj_http_parse_request($raw) {");
        self.indent += 1;
        self.line("$sep = strpos($raw, \"\\r\\n\\r\\n\"); if ($sep === false) { return null; }");
        self.line("$bodyBytes = substr($raw, $sep + 4);");
        self.line(
            "$head = preg_match('//u', substr($raw, 0, $sep)) === 1 ? substr($raw, 0, $sep) : '';",
        );
        self.line("$lines = explode(\"\\r\\n\", $head);");
        self.line("$rl = explode(' ', $lines[0]); if (count($rl) < 2) { return null; }");
        self.line("$method = $rl[0]; $target = $rl[1];");
        self.line(
            "$stash = __phorj_http_stash_body($bodyBytes); if ($stash === -2) { return null; }",
        );
        self.line("$body = new RequestBody($stash >= 0 ? '' : $bodyBytes, $stash);");
        self.line("$headerLines = array_slice($lines, 1);");
        self.line("$headerMap = __phorj_http_header_pairs($headerLines);");
        self.line("$headers = new HeaderBag($headerMap);");
        self.line("$path = $target; $queryString = ''; $qpos = strpos($target, '?');");
        self.line("if ($qpos !== false) { $path = substr($target, 0, $qpos); $queryString = substr($target, $qpos + 1); }");
        self.line("$query = new ParamBag(__phorj_http_parse_query($queryString));");
        self.line("$cookies = new ParamBag(__phorj_http_cookie_pairs($headerMap));");
        self.line("$contentType = $headerMap['content-type'][0] ?? '';");
        self.line("$formMap = []; $fileItems = []; $fileFields = [];");
        self.line("if (str_starts_with($contentType, 'application/x-www-form-urlencoded')) {");
        self.indent += 1;
        self.line("$formMap = __phorj_http_parse_query(preg_match('//u', $bodyBytes) === 1 ? $bodyBytes : '');");
        self.indent -= 1;
        self.line("}");
        self.line(
            "if (str_starts_with($contentType, 'multipart/form-data') && strlen($bodyBytes) > 0) {",
        );
        self.indent += 1;
        self.line("$boundary = __phorj_http_boundary_of($contentType); if ($boundary === '') { return null; }");
        self.line("$parts = __phorj_http_parse_multipart($bodyBytes, $boundary); if ($parts === null) { return null; }");
        self.line("$formMap = __phorj_http_multipart_fields($parts);");
        self.line("foreach ($parts as $p) {");
        self.indent += 1;
        self.line("if ($p->fileName === '') { continue; }");
        self.line("$fh = __phorj_http_stash_body($p->content); if ($fh === -2) { return null; }");
        self.line("$fileItems[] = new UploadedFile($p->fileName, strlen($p->content), $p->contentType, $fh >= 0 ? '' : $p->content, $fh);");
        self.line("$fileFields[] = $p->name;");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("return new Request($method, __phorj_http_decode_path($path), $query, $headers, $cookies, new ParamBag($formMap), new FileBag($fileItems, $fileFields), $body, new AttrBag([]), $target, $headerLines, $bodyBytes);");
        self.indent -= 1;
        self.line("}");
    }
}

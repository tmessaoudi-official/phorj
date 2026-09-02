//! The `Core.Regex` PHP runtime helpers (`__phorj_regex_*`), split out of `runtime_php.rs` (Invariant
//! 13 — that file is ratcheted at its baseline) when DEC-461 (REGEX-B) grew them.
//!
//! Byte-identity with the Rust engines rests on four decisions made HERE, each pinned by
//! `tests/differential.rs` (`regex_*` / `linear_engine_*` / `backtracking_engine_*`):
//! * `__phorj_regex_delim` emits `uD` — `D` (PCRE_DOLLAR_ENDONLY) makes `$` mean end-of-subject,
//!   like the Rust engines; without it `a$` matched `"a\n"` under PHP only (panel C3).
//! * `__phorj_regex_check` turns every `preg_*` error (`preg_last_error()`) into a thrown
//!   `RuntimeException` — the step budget (`PREG_BACKTRACK_LIMIT_ERROR`, or the JIT's stack limit)
//!   raises the SAME fault text as `fancy-regex`'s `backtrack_limit`; PHP would otherwise return
//!   `false`/`null` and exit 0.
//! * `__phorj_regex_compile` validates like the Rust `engine::build`: the LINEAR engine's reject list
//!   is ported verbatim (`__phorj_regex_linear_unsupported`), and a syntax error is a fault at
//!   compile time, not `false` from the first query.
//! * `__phorj_regex_expand` is the PHP twin of `ext::regex::replace::expand_replacement` — phorj's
//!   own replacement grammar, never PCRE's (`\1` is literal, `$$` is `$`, `${name}` works).
use super::*;

/// The helper bodies. Emitted through `self.line` so they sit at the runtime block's indentation.
const HELPERS: &str = r#"function __phorj_regex_delim($pattern) {
    foreach (['~', '#', '%', '@', '!', '`'] as $d) {
        if (strpos($pattern, $d) === false) { return $d . $pattern . $d . 'uD'; }
    }
    return '~' . str_replace('~', '\\~', $pattern) . '~uD';
}
function __phorj_regex_check($r) {
    $e = preg_last_error();
    if ($e === PREG_NO_ERROR) { return $r; }
    if ($e === PREG_BACKTRACK_LIMIT_ERROR || $e === PREG_JIT_STACKLIMIT_ERROR || $e === PREG_RECURSION_LIMIT_ERROR) {
        throw new \RuntimeException('regex step budget exceeded: the pattern backtracks catastrophically on this subject');
    }
    throw new \RuntimeException('regex error: ' . preg_last_error_msg());
}
function __phorj_regex_linear_unsupported($p) {
    $b = $p; $n = strlen($b); $i = 0; $inClass = false; $afterQ = false;
    while ($i < $n) {
        $c = $b[$i];
        if ($c === '\\') {
            if ($i + 1 >= $n) { break; }
            $x = $b[$i + 1];
            if (!$inClass) {
                if (($x >= '1' && $x <= '9') || $x === 'g' || $x === 'k') { return 'a back-reference'; }
                if (strpos('hHRZGK', $x) !== false) { return 'a PCRE-only escape (`\\h`, `\\R`, `\\Z`, `\\G`, `\\K`)'; }
            }
            $i += 2; $afterQ = false; continue;
        }
        if ($inClass) { if ($c === ']') { $inClass = false; } $i++; continue; }
        if ($c === '[') {
            $inClass = true; $i++;
            if ($i < $n && $b[$i] === '^') { $i++; }
            if ($i < $n && $b[$i] === ']') { $i++; }
            $afterQ = false; continue;
        }
        if ($c === '(') {
            $x = $i + 1 < $n ? $b[$i + 1] : ''; $y = $i + 2 < $n ? $b[$i + 2] : ''; $z = $i + 3 < $n ? $b[$i + 3] : '';
            if ($x === '*') { return 'a PCRE verb `(*…)`'; }
            if ($x === '?') {
                if ($y === '=' || $y === '!') { return 'look-ahead'; }
                if ($y === '<' && ($z === '=' || $z === '!')) { return 'look-behind'; }
                if ($y === '>') { return 'an atomic group'; }
                if ($y === '(') { return 'a conditional group'; }
                if ($y === 'R' || $y === '&' || ($y >= '0' && $y <= '9')) { return 'a recursive group'; }
            }
            $afterQ = false;
        } elseif ($c === '{') {
            if ($i + 1 < $n && $b[$i + 1] === ',') { return 'a `{,n}` quantifier'; }
            $close = strpos($b, '}', $i);
            if ($close !== false) {
                $inner = substr($b, $i + 1, $close - $i - 1);
                if ($inner !== '' && preg_match('/^[0-9,]+$/', $inner) === 1) { $i = $close + 1; $afterQ = true; continue; }
            }
            $afterQ = false;
        } elseif ($c === '*' || $c === '+' || $c === '?') {
            if ($afterQ && $c === '+') { return 'a possessive quantifier'; }
            $afterQ = ($c !== '?') || !$afterQ;
            $i++; continue;
        } else {
            $afterQ = false;
        }
        $i++;
    }
    return null;
}
function __phorj_regex_compile($p, $engine) {
    if ($engine === 'linear') {
        $what = __phorj_regex_linear_unsupported($p);
        if ($what !== null) {
            throw new \RuntimeException('invalid or unsupported regex: ' . $what . ' is not supported by the linear engine (use `Regex.compileBacktracking` for PCRE-class syntax)');
        }
    }
    if (@preg_match(__phorj_regex_delim($p), '') === false) {
        throw new \RuntimeException(($engine === 'linear' ? 'invalid or unsupported regex: ' : 'invalid regex: ') . preg_last_error_msg());
    }
    return new Regex($p, $engine);
}
function __phorj_regex_matches($re, $s) {
    return __phorj_regex_check(preg_match(__phorj_regex_delim($re->pattern), $s)) === 1;
}
function __phorj_regex_find($re, $s) {
    return __phorj_regex_check(preg_match(__phorj_regex_delim($re->pattern), $s, $m)) === 1 ? $m[0] : null;
}
function __phorj_regex_find_all($re, $s) {
    __phorj_regex_check(preg_match_all(__phorj_regex_delim($re->pattern), $s, $m));
    return $m[0];
}
function __phorj_regex_find_groups($re, $s) {
    if (__phorj_regex_check(preg_match(__phorj_regex_delim($re->pattern), $s, $m)) !== 1) { return null; }
    $out = [];
    foreach ($m as $k => $v) { if (is_string($k)) { $out[$k] = $v; } }
    return $out;
}
function __phorj_regex_find_all_groups($re, $s) {
    __phorj_regex_check(preg_match_all(__phorj_regex_delim($re->pattern), $s, $ms, PREG_SET_ORDER));
    $out = [];
    foreach ($ms as $m) {
        $g = [];
        foreach ($m as $k => $v) { if (is_string($k)) { $g[$k] = $v; } }
        $out[] = $g;
    }
    return $out;
}
function __phorj_regex_expand($repl, $m) {
    $out = ''; $n = strlen($repl); $i = 0;
    $grp = function ($k) use ($m) { return (isset($m[$k]) && $m[$k] !== null) ? $m[$k] : ''; };
    while ($i < $n) {
        $c = $repl[$i];
        if ($c !== '$') { $out .= $c; $i++; continue; }
        $x = $i + 1 < $n ? $repl[$i + 1] : '';
        if ($x === '$') { $out .= '$'; $i += 2; continue; }
        if ($x === '{') {
            $close = strpos($repl, '}', $i + 2);
            if ($close === false) { $out .= '$'; $i++; continue; }
            $inner = substr($repl, $i + 2, $close - $i - 2);
            if ($inner === '') { $out .= '${}'; }
            elseif (preg_match('/^[0-9]+$/', $inner) === 1) { $out .= $grp((int)$inner); }
            else { $out .= $grp($inner); }
            $i = $close + 1; continue;
        }
        if ($x >= '0' && $x <= '9') {
            $j = $i + 1; while ($j < $n && $repl[$j] >= '0' && $repl[$j] <= '9') { $j++; }
            $out .= $grp((int)substr($repl, $i + 1, $j - $i - 1)); $i = $j; continue;
        }
        if (preg_match('/^[A-Za-z_]$/', $x) === 1) {
            $j = $i + 1; while ($j < $n && preg_match('/^[A-Za-z0-9_]$/', $repl[$j]) === 1) { $j++; }
            $out .= $grp(substr($repl, $i + 1, $j - $i - 1)); $i = $j; continue;
        }
        $out .= '$'; $i++;
    }
    return $out;
}
function __phorj_regex_replace($re, $s, $repl) {
    return __phorj_regex_check(preg_replace_callback(__phorj_regex_delim($re->pattern), function ($m) use ($repl) {
        return __phorj_regex_expand($repl, $m);
    }, $s, -1, $count, PREG_UNMATCHED_AS_NULL));
}
function __phorj_regex_split($re, $s) {
    return __phorj_regex_check(preg_split(__phorj_regex_delim($re->pattern), $s));
}
function __phorj_regex_quote_meta($s) {
    return addcslashes($s, '\\.+*?()|[]{}^$#&-~');
}
function __phorj_regex_replace_callback($re, $s, $cb) {
    return __phorj_regex_check(preg_replace_callback(__phorj_regex_delim($re->pattern), function ($m) use ($cb) {
        $g = [];
        foreach ($m as $k => $v) { if (is_string($k) && $v !== null) { $g[$k] = $v; } }
        return $cb(new RegexMatch($m[0], $g));
    }, $s, -1, $count, PREG_UNMATCHED_AS_NULL));
}"#;

impl Transpiler {
    /// Emit the `Core.Regex` helper block (called from `emit_runtime_helpers` under `uses_regex`).
    /// `quoteMeta` mirrors the `regex` crate's `escape` meta-set (DEC-296), NOT `preg_quote`;
    /// `replaceCallback` (DEC-295) omits non-participating named groups via `PREG_UNMATCHED_AS_NULL`
    /// + a null-filter, exactly like the Rust native.
    pub(super) fn emit_regex_helpers(&mut self) {
        for l in HELPERS.lines() {
            self.line(l);
        }
    }
}

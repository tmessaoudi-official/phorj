//! PHP transpiler — the `__phorj_wordwrap` helper, gated by `uses_wordwrap`.
//!
//! **This deliberately does NOT call PHP's own `wordwrap`.** That function is byte-oriented and with
//! `cut = true` splits a multi-byte character, emitting bytes that are not valid UTF-8 — verified
//! under php-8.5.9: `wordwrap("ééééééé", 3, "|", true)` gives `c3 a9 c3 7c a9 …`. A phorj `string`
//! cannot hold that, so `String.wordWrap` counts CODE POINTS (developer-ruled 2026-09-04), and this
//! helper runs the same codepoint algorithm as `native::registry_modules::wordwrap`.
//!
//! The consequence is worth stating precisely, because it is easy to describe wrongly: all three
//! legs agree byte-for-byte, so there is NO new byte-identity exception. What differs is
//! `String.wordWrap` versus PHP's `wordwrap` on multi-byte input — a semantic choice, not a break in
//! the spine. On ASCII the two are identical.

use super::*;

impl Transpiler {
    pub(super) fn emit_wordwrap_helper(&mut self) {
        if !self.gates.uses_wordwrap {
            return;
        }
        for line in WORDWRAP_HELPER.lines() {
            self.line(line);
        }
    }
}

/// `//u` splitting is PCRE, always compiled in — never mbstring.
const WORDWRAP_HELPER: &str = r#"function __phorj_wordwrap($s, $w, $b, $c) {
$cs = preg_split('//u', $s, -1, PREG_SPLIT_NO_EMPTY);
$bs = preg_split('//u', $b, -1, PREG_SPLIT_NO_EMPTY);
if ($cs === false || $bs === false) { return $s; }
$n = count($cs); $bn = count($bs);
if ($n === 0 || $bn === 0) { return $s; }
if ($w < 1) { $w = 1; }
$out = ''; $laststart = 0; $lastspace = 0; $current = 0;
while ($current < $n) {
$match = $current + $bn <= $n;
if ($match) { for ($k = 0; $k < $bn; $k++) { if ($cs[$current + $k] !== $bs[$k]) { $match = false; break; } } }
if ($match) {
$out .= implode('', array_slice($cs, $laststart, $current + $bn - $laststart));
$current += $bn; $laststart = $current; $lastspace = $current; continue;
}
if ($cs[$current] === ' ') {
if ($current - $laststart >= $w) {
$out .= implode('', array_slice($cs, $laststart, $current - $laststart)) . $b;
$laststart = $current + 1;
}
$lastspace = $current;
} elseif ($current - $laststart >= $w && $laststart >= $lastspace) {
if ($c) {
$out .= implode('', array_slice($cs, $laststart, $current - $laststart)) . $b;
$laststart = $current; $lastspace = $current;
}
} elseif ($current - $laststart >= $w && $laststart < $lastspace) {
$out .= implode('', array_slice($cs, $laststart, $lastspace - $laststart)) . $b;
$laststart = $lastspace + 1; $lastspace = $laststart;
}
$current++;
}
if ($laststart !== $current) { $out .= implode('', array_slice($cs, $laststart)); }
return $out;
}"#;

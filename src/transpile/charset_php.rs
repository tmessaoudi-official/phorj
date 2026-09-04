//! PHP transpiler — the `__phorj_cs_*` charset helpers (DEC-468 surface, DEC-494 strategy), gated
//! by `uses_charset`.
//!
//! **The two tables are FORMATTED FROM [`crate::charset`]'s consts, never transcribed.** That is the
//! whole point of DEC-494: `mb_convert_encoding` and `iconv` are shared ini extensions that the
//! oracle's `php -n` does not have and the default-deny tier-1 guard rejects, so the PHP leg must
//! re-implement the codec — and a second hand-written copy of the tables would be free to drift from
//! the native leg while every test stayed green. Reading one source removes that failure mode
//! entirely; a table fix lands on both legs at once.
//!
//! Split out of `runtime_php.rs` because that file is grandfathered under Invariant 13 and must not
//! grow — the `db_php`/`fs_php`/`log_php` convention.

use super::*;

impl Transpiler {
    pub(super) fn emit_charset_helpers(&mut self) {
        #[cfg(feature = "encoding")]
        if self.gates.uses_charset {
            // DEC-468 / DEC-494 — charset transcoding with NO ini extension. The two tables are
            // formatted from `ext::encoding::charset`'s consts, so this leg and the native leg read
            // the SAME data: a table fix lands on both at once, and neither can drift.
            use crate::charset::{CP1252_C1, LATIN9_DIFF};
            let l9: Vec<String> = LATIN9_DIFF
                .iter()
                .map(|(b, cp)| format!("{b} => {cp}"))
                .collect();
            let c1: Vec<String> = CP1252_C1.iter().map(u32::to_string).collect();
            // The enum variant's leaf name — `get_class` yields `Charset_Utf16Le` (possibly
            // namespace-qualified), and only the segment after the last `\` identifies the variant.
            self.line("function __phorj_cs_name($cs) {");
            self.indent += 1;
            self.line("$c = get_class($cs);");
            self.line("$p = strrpos($c, '\\\\');");
            self.line("return substr($p === false ? $c : substr($c, $p + 1), 8);");
            self.indent -= 1;
            self.line("}");

            self.line("function __phorj_cs_decode($b, $cs) {");
            self.indent += 1;
            self.line("$n = __phorj_cs_name($cs);");
            // UTF-8 in, UTF-8 out: the only question is validity, and `//u` is PCRE (always compiled
            // in), never mbstring.
            self.line("if ($n === 'Utf8') { return preg_match('//u', $b) === 1 ? $b : null; }");
            self.line("$len = strlen($b); $cps = [];");
            self.line("if ($n === 'Utf16Le' || $n === 'Utf16Be') {");
            self.indent += 1;
            self.line("if ($len % 2 !== 0) { return null; }");
            self.line("$le = $n === 'Utf16Le'; $i = 0;");
            self.line("while ($i < $len) {");
            self.indent += 1;
            self.line("$a = ord($b[$i]); $c = ord($b[$i + 1]); $i += 2;");
            self.line("$u = $le ? ($a | ($c << 8)) : (($a << 8) | $c);");
            self.line("if ($u >= 0xDC00 && $u <= 0xDFFF) { return null; }");
            self.line("if ($u >= 0xD800 && $u <= 0xDBFF) {");
            self.indent += 1;
            self.line("if ($i + 1 >= $len) { return null; }");
            self.line("$a2 = ord($b[$i]); $c2 = ord($b[$i + 1]); $i += 2;");
            self.line("$u2 = $le ? ($a2 | ($c2 << 8)) : (($a2 << 8) | $c2);");
            self.line("if ($u2 < 0xDC00 || $u2 > 0xDFFF) { return null; }");
            self.line("$cps[] = 0x10000 + (($u - 0xD800) << 10) + ($u2 - 0xDC00);");
            self.indent -= 1;
            self.line("} else { $cps[] = $u; }");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            self.line(&format!("$L9 = [{}];", l9.join(", ")));
            self.line(&format!("$C1 = [{}];", c1.join(", ")));
            self.line("for ($i = 0; $i < $len; $i++) {");
            self.indent += 1;
            self.line("$o = ord($b[$i]);");
            self.line("if ($n === 'Ascii') { if ($o >= 0x80) { return null; } $cps[] = $o; }");
            self.line("elseif ($n === 'Latin1') { $cps[] = $o; }");
            self.line(
                "elseif ($n === 'Latin9') { $cps[] = array_key_exists($o, $L9) ? $L9[$o] : $o; }",
            );
            self.line("else {");
            self.indent += 1;
            self.line("if ($o >= 0x80 && $o <= 0x9F) {");
            self.indent += 1;
            self.line("$m = $C1[$o - 0x80];");
            self.line("if ($m === 0) { return null; }");
            self.line("$cps[] = $m;");
            self.indent -= 1;
            self.line("} else { $cps[] = $o; }");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            // Code points → UTF-8, one place for every branch above (the native leg's `char` push).
            self.line("$out = '';");
            self.line("foreach ($cps as $cp) {");
            self.indent += 1;
            self.line("if ($cp < 0x80) { $out .= chr($cp); }");
            self.line("elseif ($cp < 0x800) { $out .= chr(0xC0 | ($cp >> 6)) . chr(0x80 | ($cp & 0x3F)); }");
            self.line("elseif ($cp < 0x10000) { $out .= chr(0xE0 | ($cp >> 12)) . chr(0x80 | (($cp >> 6) & 0x3F)) . chr(0x80 | ($cp & 0x3F)); }");
            self.line("else { $out .= chr(0xF0 | ($cp >> 18)) . chr(0x80 | (($cp >> 12) & 0x3F)) . chr(0x80 | (($cp >> 6) & 0x3F)) . chr(0x80 | ($cp & 0x3F)); }");
            self.indent -= 1;
            self.line("}");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");

            self.line("function __phorj_cs_encode($s, $cs) {");
            self.indent += 1;
            self.line("$n = __phorj_cs_name($cs);");
            self.line("if ($n === 'Utf8') { return $s; }");
            // `//u` split yields whole code points; phorj strings are UTF-8 by construction, so a
            // `false` here would mean an already-broken string rather than user input.
            self.line("$chars = preg_split('//u', $s, -1, PREG_SPLIT_NO_EMPTY);");
            self.line("if ($chars === false) { return null; }");
            self.line(&format!("$L9 = [{}];", l9.join(", ")));
            self.line(&format!("$C1 = [{}];", c1.join(", ")));
            self.line("$out = '';");
            self.line("foreach ($chars as $ch) {");
            self.indent += 1;
            self.line("$l = strlen($ch);");
            self.line("if ($l === 1) { $cp = ord($ch); }");
            self.line(
                "elseif ($l === 2) { $cp = ((ord($ch[0]) & 0x1F) << 6) | (ord($ch[1]) & 0x3F); }",
            );
            self.line("elseif ($l === 3) { $cp = ((ord($ch[0]) & 0x0F) << 12) | ((ord($ch[1]) & 0x3F) << 6) | (ord($ch[2]) & 0x3F); }");
            self.line("else { $cp = ((ord($ch[0]) & 0x07) << 18) | ((ord($ch[1]) & 0x3F) << 12) | ((ord($ch[2]) & 0x3F) << 6) | (ord($ch[3]) & 0x3F); }");
            self.line("if ($n === 'Utf16Le' || $n === 'Utf16Be') {");
            self.indent += 1;
            self.line("$le = $n === 'Utf16Le'; $us = [];");
            self.line("if ($cp >= 0x10000) { $v = $cp - 0x10000; $us[] = 0xD800 + ($v >> 10); $us[] = 0xDC00 + ($v & 0x3FF); }");
            self.line("else { $us[] = $cp; }");
            self.line("foreach ($us as $u) { $hi = ($u >> 8) & 0xFF; $lo = $u & 0xFF; $out .= $le ? (chr($lo) . chr($hi)) : (chr($hi) . chr($lo)); }");
            self.indent -= 1;
            self.line(
                "} elseif ($n === 'Ascii') { if ($cp >= 0x80) { return null; } $out .= chr($cp); }",
            );
            self.line(
                "elseif ($n === 'Latin1') { if ($cp >= 0x100) { return null; } $out .= chr($cp); }",
            );
            self.line("elseif ($n === 'Latin9') {");
            self.indent += 1;
            self.line("$k = array_search($cp, $L9, true);");
            self.line("if ($k !== false) { $out .= chr($k); }");
            // A byte whose Latin-9 meaning was reassigned is unreachable by its Latin-1 code point.
            self.line("elseif ($cp < 0x100 && !array_key_exists($cp, $L9)) { $out .= chr($cp); }");
            self.line("else { return null; }");
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            self.line("$k = $cp === 0 ? false : array_search($cp, $C1, true);");
            self.line("if ($k !== false) { $out .= chr(0x80 + $k); }");
            // 0x80..=0x9F are C1 controls in Latin-1 and simply not encodable in Windows-1252.
            self.line(
                "elseif ($cp < 0x100 && !($cp >= 0x80 && $cp <= 0x9F)) { $out .= chr($cp); }",
            );
            self.line("else { return null; }");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            self.line("return $out;");
            self.indent -= 1;
            self.line("}");
        }
    }
}

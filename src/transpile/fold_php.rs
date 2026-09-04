//! PHP transpiler — the `__phorj_fold_accents` helper (DEC-468), gated by `uses_fold_accents`.
//!
//! **The table is FORMATTED FROM [`crate::fold_accents::FOLD`], never transcribed** — the same
//! single-source discipline as `charset_php`. A second hand-written copy of 190 rows would be free
//! to drift from the native leg while every test stayed green.
//!
//! `strtr($s, $map)` is the whole implementation: with an array argument it replaces the longest
//! matching key first and operates on byte sequences, so UTF-8 keys work without mbstring, and no
//! key here is a prefix of another (every key is exactly one character). It is core PHP, present
//! under the oracle's `php -n`. The helper exists rather than the call being inlined because the
//! map is a 190-entry literal — DEC-468 names `__phorj_fold_accents` for that reason.

use super::*;

impl Transpiler {
    pub(super) fn emit_fold_accents_helper(&mut self) {
        if !self.gates.uses_fold_accents {
            return;
        }
        let pairs: Vec<String> = crate::fold_accents::FOLD
            .iter()
            .map(|(k, v)| format!("\"{k}\" => \"{v}\""))
            .collect();
        self.line("function __phorj_fold_accents($s) {");
        self.indent += 1;
        // Chunked so the emitted line stays readable rather than one 4 kB literal.
        self.line("static $m = null;");
        self.line("if ($m === null) { $m = [");
        self.indent += 1;
        for chunk in pairs.chunks(6) {
            self.line(&format!("{},", chunk.join(", ")));
        }
        self.indent -= 1;
        self.line("]; }");
        self.line("return strtr($s, $m);");
        self.indent -= 1;
        self.line("}");
    }
}

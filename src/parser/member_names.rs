//! Member-name parsing (DEC-531, scout row 5k) — split out of `mod.rs` under Invariant 13.

use super::*;

impl Parser {
    /// A MEMBER name (DEC-531): an identifier, or a reserved word in a position where nothing else
    /// can stand — a method, field or promoted-parameter name, after `.`/`?.`/`::`, a `with` field or a
    /// named argument. PHP 8 allows the same, so a lifted `match()` method keeps its name. `constructor`
    /// stays reserved: `parent.constructor(…)` is the parent-constructor call.
    pub(in crate::parser) fn expect_member_name(
        &mut self,
        what: &str,
    ) -> Result<String, Diagnostic> {
        match crate::tokenizer::reserved_word(self.peek()) {
            Some(w) if w != "constructor" => {
                self.advance();
                Ok(w.to_string())
            }
            _ => self.expect_ident(what),
        }
    }
}

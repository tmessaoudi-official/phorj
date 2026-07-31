//! DEC-419 — printing a lifted declaration's `/** … */` doc comment.
//!
//! Its own module because `printer/items.rs` crossed the Invariant-13 hard cap when this landed. The
//! star column is re-added around the body the PHP lexer stripped, so this is the exact inverse of
//! `lexer::strip_php_doc_stars`.

use super::Printer;
use crate::ast::Item;

impl Printer {
    /// Print the `/** … */` doc comment for `item`, if the lifted PHP had one. The star column is
    /// re-added around the stored body — the lexer stripped it, so this is the exact inverse.
    pub(super) fn doc_comment(&mut self, item: &Item) {
        let Some(name) = crate::ast::item_decl_name(item) else {
            return;
        };
        let Some(doc) = self.docs.get(name).cloned() else {
            return;
        };
        self.line("/**");
        for l in doc.lines() {
            if l.is_empty() {
                self.line(" *");
            } else {
                self.line(&format!(" * {l}"));
            }
        }
        self.line(" */");
    }
}

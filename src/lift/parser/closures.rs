//! PHP-lift parser — closures. Lane R (2026-09-05): scout's pure-logic modules use `fn` arrow
//! closures throughout (`static fn` in 25 of 120 files); a `function (…) { … }` block closure is
//! still Tier-2, because a phorj lambda with a block body has no reason to exist yet in the subset
//! the lifter drafts.

use super::*;

impl PParser {
    /// `fn (params) [: type] => expr`, the `fn` already peeked. PHP's arrow function captures the
    /// enclosing scope by value automatically — exactly what a phorj lambda does — so there is no
    /// `use (…)` list to read (that is the block closure's syntax, which is refused).
    pub(super) fn parse_arrow_closure(&mut self) -> Result<PhpExpr, String> {
        if self.at_static_fn() {
            self.advance(); // `static` — a closure modifier here, not a static call
        }
        self.advance(); // `fn`
        let params = self.parse_params()?;
        let ret = if self.eat(&PTok::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&PTok::FatArrow, "`=>` in an arrow closure")?;
        let body = Box::new(self.parse_expr()?);
        Ok(PhpExpr::Closure { params, ret, body })
    }

    /// `static fn` — `static` followed by the `fn` keyword.
    pub(super) fn at_static_fn(&self) -> bool {
        matches!(self.peek(), PTok::Ident(w) if w == "static")
            && matches!(self.peek_at(1), PTok::Ident(w) if w == "fn")
    }

    /// A block-bodied `function (…) { … }` closure stays Tier-2: its body is a statement list
    /// and its `use (…)` list can capture by reference, neither of which has a faithful draft.
    pub(super) fn refuse_block_closure(&self) -> Result<PhpExpr, String> {
        Err(self.err(
            "a block-bodied closure `function (…) { … }` is Tier-2 — arrow closures `fn (…) => …` lift",
        ))
    }
}

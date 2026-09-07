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

    /// `[static] function (params) [use (…)] [: type] { stmts }` — a block-bodied closure
    /// (lane L1b, 2026-09-07). phorj HAS this shape: `function(int x): int { … }`. What it does not
    /// have is by-REFERENCE capture, and conflating the two behind one "Tier-2" message was the
    /// defect — a construct phorj supports and a construct phorj has RULED OUT read identically.
    ///
    /// Three refusals, each naming its own reason:
    /// * `use (&$x)` — a ruled REJECTION (UNIFIED-SPEC:1619, DEC-506), named with the variable.
    /// * no `: T` — phorj requires a return type on a statement-bodied lambda, and inferring one
    ///   would be the lifter guessing (DEC-166).
    /// * `static` is DROPPED, not refused: it only stops PHP binding `$this`, and a phorj lambda
    ///   never binds one, so the modifier carries no information here.
    pub(super) fn parse_block_closure(&mut self) -> Result<PhpExpr, String> {
        if matches!(self.peek(), PTok::Ident(w) if w == "static") {
            self.advance();
        }
        self.advance(); // `function`
        let params = self.parse_params()?;
        if matches!(self.peek(), PTok::Ident(w) if w == "use") {
            self.advance();
            self.expect(&PTok::LParen, "`(` after `use`")?;
            while !self.at(&PTok::RParen) {
                if self.eat(&PTok::Amp) {
                    let name = match self.peek() {
                        PTok::Var(v) => v.clone(),
                        _ => "?".to_string(),
                    };
                    return Err(self.err(&format!(
                        "a closure capturing `${name}` by reference (`use (&${name})`) has no phorj                          form — references contradict the value/handle split and are RULED OUT                          (DEC-506), not merely unimplemented. Rewrite the closure to return its                          result instead of mutating a captured variable."
                    )));
                }
                match self.peek() {
                    PTok::Var(_) => {
                        self.advance();
                    }
                    _ => return Err(self.err("a variable in a closure `use (…)` list")),
                }
                if !self.eat(&PTok::Comma) {
                    break;
                }
            }
            self.expect(&PTok::RParen, "`)` to close the `use` list")?;
        }
        let ret = if self.eat(&PTok::Colon) {
            Some(self.parse_type()?)
        } else {
            return Err(self.err(
                "a block-bodied closure needs a declared return type (`function (…): T { … }`) —                  phorj requires one on a statement-bodied lambda and the lifter does not infer it",
            ));
        };
        let body = self.parse_block()?;
        Ok(PhpExpr::BlockClosure { params, ret, body })
    }
}

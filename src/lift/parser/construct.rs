//! PHP-lift parser — object construction (`new`). Split out of `exprs.rs` under Invariant 13,
//! which grandfathers that file at its baseline: it may not grow by even one line, and the
//! static-factory work (Lane R-8) adds to exactly this function.

use super::*;

impl PParser {
    pub(super) fn parse_new(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `new`
        if matches!(self.peek(), PTok::Var(_)) {
            return Err(self.err("dynamic `new $class` is Tier-3"));
        }
        // A qualified name is accepted here (`new \RuntimeException(…)`), not just a bare ident: PHP
        // writes root-namespace builtins that way, and `throw`/`catch` both routinely do.
        let mut class = self.parse_qualified_name()?;
        // `new self(…)` is the second half of PHP's static-factory idiom (`static function at(): self
        // { return new self(…); }`) — 17 of scout's files construct that way. R-7 resolved `self` in
        // TYPE position; leaving it here made the draft say `new self(…)`, which phorj reads as a
        // call to a function named `self` (`E-NEW-ON-NONCONSTRUCT`). `static` keeps its refusal for
        // the same reason it does in return position: late static binding is the RECEIVER's class,
        // which the enclosing name would narrow.
        class = self.resolve_self_class(class)?;
        let args = if self.at(&PTok::LParen) {
            self.parse_args()?
        } else {
            Vec::new()
        };
        Ok(PhpExpr::New { class, args })
    }

    /// `self` in any EXPRESSION position (`new self(…)`, `self::CONST`, `self::method()`) is the
    /// enclosing class, exactly — the same rule `selfref.rs` applies to type positions after a class
    /// body is parsed. An expression cannot wait for that pass without a total walk of the PHP
    /// expression tree, and the name is already known here: a class's name is read before its
    /// members.
    ///
    /// `static` is NOT resolved, here or anywhere: late static binding means "the receiver's class",
    /// which the enclosing name would narrow — a silent semantic change, which Invariant 14 forbids
    /// in favour of a loud refusal.
    pub(super) fn resolve_self_class(&mut self, class: String) -> Result<String, String> {
        if class == "self" {
            return match &self.current_class {
                Some(c) => Ok(c.clone()),
                None => Err(self.err("`self` outside a class body")),
            };
        }
        if class == "static" {
            return Err(self.err("`static::` / `new static` (late static binding) is Tier-2"));
        }
        Ok(class)
    }
}

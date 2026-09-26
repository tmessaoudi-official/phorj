//! PHP-lift parser — docblock callable SIGNATURES (row 4e, 2026-09-26; split from `doc_types.rs`).
//!
//! PHPStan and Psalm spell a callable's type as `callable(A, B): R` or `\Closure(A): R`. That is
//! exactly a phorj function type `(A, B) => R`, so a native `callable` / `\Closure` whose docblock
//! carries one is typed by it ([`super::docblock`] decides where it is substituted). The grammar
//! entry point is `doc_atom` in [`super::doc_types`], which peels the `callable` / `Closure` head off
//! before its class-name path — `\Closure` must not register an implicit `use`.

use super::doc_types::Cursor;
use super::*;

impl PParser {
    /// A callable SIGNATURE, `(` already eaten: `callable(A, B): R` / `\Closure(A): R` (PHPStan,
    /// Psalm), the phorj function type `(A, B) => R`. A parameter may be named (`callable(string
    /// $key): R`) and the name is dropped. What a phorj function type cannot say is refused by name:
    /// an optional (`int=`), variadic (`int...`) or by-reference (`int &$x`) parameter, and a missing
    /// `: R` — PHP reads that as `mixed`, which the lifter does not guess at.
    pub(super) fn doc_callable(&mut self, c: &mut Cursor) -> Result<PhpType, String> {
        let mut params = Vec::new();
        if !c.eat(')') {
            loop {
                params.push(self.doc_type(c)?);
                self.doc_param_tail(c)?;
                if !c.eat(',') {
                    break;
                }
            }
            if !c.eat(')') {
                return Err(self.err(&format!("`)` in the callable docblock type `{}`", c.s)));
            }
        }
        if !c.eat(':') {
            return Err(self.err(&format!(
                "the callable docblock type `{}` needs its return type `: R` (without one PHP reads it as `mixed`)",
                c.s
            )));
        }
        let ret = Box::new(self.doc_type(c)?);
        Ok(PhpType::Function { params, ret })
    }

    /// After a signature parameter's type: refuse the modifiers phorj has no form for, drop a name.
    fn doc_param_tail(&mut self, c: &mut Cursor) -> Result<(), String> {
        let whole = c.s;
        let refuse = |p: &mut Self, what: &str| {
            Err(p.err(&format!(
                "{what} in the callable docblock type `{whole}` has no phorj function-type form"
            )))
        };
        c.skip_ws();
        if c.eat_str("...") {
            return refuse(self, "a variadic parameter");
        }
        if c.eat_str("&") {
            return refuse(self, "a by-reference parameter");
        }
        if c.eat_str("$") {
            c.ident();
        }
        if c.eat('=') {
            return refuse(self, "an optional parameter");
        }
        Ok(())
    }
}

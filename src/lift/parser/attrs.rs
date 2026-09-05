//! PHP-lift parser — PHP 8 **attributes** (`#[Name(args)]`), LIFT-ATTR.
//!
//! Split out of `items.rs` by cohesion (Invariant 13): attribute syntax is its own small grammar —
//! a group may hold several attributes, a name may be namespace-qualified, and the argument list
//! admits PHP 8.0 NAMED arguments, which nothing else in the Tier-1 subset does.
//!
//! The name is kept in its **source spelling**, backslashes and any leading root marker included.
//! Resolution needs the file's `namespace` and `use` map, which live one layer up in the lifter, and
//! a parser that resolved eagerly would have to guess: `#[Attribute]` means PHP's built-in in a
//! namespace-less file and `App\Attribute` inside `namespace App;`.

use super::*;

impl PParser {
    /// Zero or more attribute GROUPS: `#[A] #[B(1), C]` → `[A, B(1), C]`.
    ///
    /// PHP allows several attributes per `#[…]` group (comma-separated, trailing comma permitted), so a
    /// group is flattened into the result rather than represented — phorj writes one `#[…]` per
    /// attribute and the grouping carries no meaning of its own.
    pub(super) fn parse_attr_groups(&mut self) -> Result<Vec<PhpAttribute>, String> {
        let mut out = Vec::new();
        while self.at(&PTok::AttrOpen) {
            self.advance(); // `#[`
            loop {
                out.push(self.parse_one_attr()?);
                if !self.eat(&PTok::Comma) || self.at(&PTok::RBracket) {
                    break;
                }
            }
            self.expect(&PTok::RBracket, "`]` to close the attribute")?;
        }
        Ok(out)
    }

    /// One attribute inside a group: `[\]Name[\Seg…][(args)]`.
    fn parse_one_attr(&mut self) -> Result<PhpAttribute, String> {
        let line = self.line();
        let mut name = String::new();
        // A leading `\` is RETAINED: it is the difference between PHP's built-in `\Attribute` and a
        // namespace-relative `Attribute`, which resolve to different classes.
        if self.eat(&PTok::Backslash) {
            name.push('\\');
        }
        name.push_str(&self.expect_ident("an attribute name after `#[`")?);
        while self.eat(&PTok::Backslash) {
            name.push('\\');
            name.push_str(&self.expect_ident("an attribute name segment after `\\`")?);
        }
        let args = if self.eat(&PTok::LParen) {
            let args = self.parse_attr_args()?;
            self.expect(&PTok::RParen, "`)` to close the attribute arguments")?;
            args
        } else {
            Vec::new()
        };
        Ok(PhpAttribute { name, args, line })
    }

    /// An attribute's argument list, up to (not including) `)`. Tolerates a trailing comma.
    fn parse_attr_args(&mut self) -> Result<Vec<PhpExpr>, String> {
        let mut args = Vec::new();
        while !self.at(&PTok::RParen) {
            args.push(self.parse_attr_arg()?);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        Ok(args)
    }

    /// One argument: `name: value` (PHP 8.0 named argument) or a plain expression.
    ///
    /// Named arguments are the dominant real-world attribute spelling (`#[Route(path: '/x')]`), and
    /// phorj accepts them in the same position (DEC-297 for calls/construction, DEC-435 for
    /// attributes), so they lift 1:1 instead of being reordered away. The `name :` lookahead cannot
    /// collide with a static access — `::` lexes as its own [`PTok::DoubleColon`].
    fn parse_attr_arg(&mut self) -> Result<PhpExpr, String> {
        self.parse_arg() // the same reader ordinary argument lists use (Lane R-4)
    }

    /// Refuse an attribute in a position phorj has no target for.
    ///
    /// phorj allows `#[…]` on a top-level `function` or `class` ONLY (`E-ATTR-TARGET`), so a PHP
    /// attribute on a method, property, parameter, constant or enum case has nowhere to land. Dropping
    /// it would be a silent semantic loss — `#[ORM\Column]` on a property is the whole meaning of that
    /// line — so the lift is refused with the position named (DEC-166). A no-op when the cursor is not
    /// at `#[`, so callers can guard unconditionally.
    pub(super) fn reject_attr_here(&self, what: &str) -> Result<(), String> {
        if !self.at(&PTok::AttrOpen) {
            return Ok(());
        }
        Err(self.err_reason(&format!(
            "an attribute on {what} — phorj allows `#[…]` only on a top-level `function` or `class`, \
             and dropping this one would lose what it says"
        )))
    }
}

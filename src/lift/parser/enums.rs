//! PHP-lift parser — the `enum` item (split out of `items.rs`, Invariant 13, 2026-09-07).
//!
//! An enum body IS a class body for `self` (lane L1): without `current_class` every `self::CASE` in
//! an enum method refused with *"`self` outside a class body"*, a message that reads like the file
//! is malformed rather than like a lifter gap. `static` keeps its refusal here exactly as it does in
//! a class — late static binding is the receiver's class, which the enclosing name would narrow.

use super::*;

impl PParser {
    pub(super) fn parse_enum(&mut self) -> Result<PhpEnum, String> {
        let line = self.line();
        self.advance(); // `enum`
        let name = self.expect_ident("enum name")?;
        let backing = if self.eat(&PTok::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let implements = self.parse_implements()?;
        self.expect(&PTok::LBrace, "`{`")?;
        let mut cases = Vec::new();
        let mut methods = Vec::new();
        // An enum body IS a class body for `self` (lane L1, 2026-09-07). Without this every
        // `self::CASE` in an enum method refused with "`self` outside a class body" — a message
        // that reads like the file is malformed rather than like a lifter gap. `static` keeps its
        // refusal here exactly as it does in a class: late static binding is the receiver's class.
        let outer_class = self.current_class.replace(name.clone());
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            self.reject_attr_here("an enum case or method")?;
            if self.is_kw("case") {
                self.advance();
                let cname = self.expect_ident("case name")?;
                let value = if self.eat(&PTok::Assign) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(&PTok::Semi, "`;`")?;
                cases.push(PhpEnumCase { name: cname, value });
            } else {
                let doc = self.doc_here();
                let mut mem = self.parse_member()?;
                self.apply_doc_member(doc.as_deref(), &mut mem)?;
                match mem {
                    PhpMember::Method(m) => methods.push(m),
                    _ => {
                        return Err(self.err("an enum may only contain cases and methods (Tier-1)"))
                    }
                }
            }
        }
        self.current_class = outer_class;
        self.expect(&PTok::RBrace, "`}`")?;
        // Type positions too: `: self` / `(self $other)` on an enum method is the enum, the same
        // rule `selfref.rs` applies to a class after its body is parsed.
        let mut as_members: Vec<PhpMember> = methods.into_iter().map(PhpMember::Method).collect();
        resolve_self(&mut as_members, &name);
        let methods: Vec<PhpMethod> = as_members
            .into_iter()
            .map(|m| match m {
                PhpMember::Method(me) => me,
                _ => unreachable!("only methods were put in"),
            })
            .collect();
        Ok(PhpEnum {
            name,
            backing,
            implements,
            cases,
            methods,
            line,
        })
    }
}

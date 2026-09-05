//! PHP-lift parser — interfaces (Lane R-5, 2026-09-05). `interface` was on the Tier-1 refusal
//! list; scout declares 11. An interface is a name, an optional `extends` list and bodiless
//! methods — constants and properties are refused by name (phorj interfaces carry neither).

use super::*;

impl PParser {
    pub(super) fn parse_interface(&mut self) -> Result<PhpInterface, String> {
        let line = self.line();
        self.advance(); // `interface`
        let name = self.expect_ident("interface name")?;
        let mut extends = Vec::new();
        if self.is_kw("extends") {
            self.advance();
            extends.push(self.parse_class_ref()?);
            while self.eat(&PTok::Comma) {
                extends.push(self.parse_class_ref()?);
            }
        }
        self.expect(&PTok::LBrace, "`{`")?;
        let mut methods = Vec::new();
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            // The same docblock hook the class body runs: `array $items` + `@param list<T>` lives
            // exactly here (scout's `HttpClient`, `Mailbox`).
            let doc = self.doc_here();
            let mut m = self.parse_member()?;
            self.apply_doc_member(doc.as_deref(), &mut m)?;
            match m {
                PhpMember::Method(me) if me.body.is_none() => methods.push(me),
                PhpMember::Method(me) => {
                    return Err(self.err(&format!("interface method `{}` has a body", me.name)))
                }
                PhpMember::Const { name, .. } => {
                    return Err(self.err(&format!(
                    "interface constant `{name}` is Tier-2 — phorj interfaces carry no constants"
                )))
                }
                PhpMember::Prop { .. } => {
                    return Err(self.err("a property inside an interface"));
                }
            }
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(PhpInterface {
            name,
            extends,
            methods,
            line,
        })
    }

    /// A class reference in `extends` / `implements`: bare, or `\`-rooted (→ implicit `use`).
    pub(super) fn parse_class_ref(&mut self) -> Result<String, String> {
        if self.at(&PTok::Backslash) {
            self.root_qualified_local()
        } else {
            self.expect_ident("a class name")
        }
    }
}

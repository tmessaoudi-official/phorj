//! PHP-lift parser — items: cursor helpers, functions, classes, members, enums.

use super::*;

impl PParser {
    // ── cursor ──

    pub(super) fn peek(&self) -> &PTok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    pub(super) fn peek_at(&self, n: usize) -> &PTok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }

    pub(super) fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].line
    }

    pub(super) fn advance(&mut self) -> PTok {
        let tok = self.toks[self.pos.min(self.toks.len() - 1)].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    pub(super) fn at(&self, tok: &PTok) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(tok)
    }

    pub(super) fn eat(&mut self, tok: &PTok) -> bool {
        if self.at(tok) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume a payload-free token of the expected kind, or error with `what`.
    pub(super) fn expect(&mut self, tok: &PTok, what: &str) -> Result<(), String> {
        if self.at(tok) {
            self.advance();
            Ok(())
        } else {
            Err(self.err(&format!("expected {what}")))
        }
    }

    pub(super) fn expect_ident(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            PTok::Ident(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(self.err(&format!("expected {what}"))),
        }
    }

    /// Consume a `$var`, returning the name (without `$`), or error.
    pub(super) fn expect_var(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            PTok::Var(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(self.err(&format!("expected {what}"))),
        }
    }

    pub(super) fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), PTok::Ident(s) if s == kw)
    }

    pub(super) fn err(&self, msg: &str) -> String {
        format!(
            "lift parse error: {msg}, found {:?} (line {})",
            self.peek(),
            self.line()
        )
    }

    // ── program / items ──

    pub(super) fn parse_program(&mut self) -> Result<PhpProgram, String> {
        // An optional leading `<?php` open tag.
        self.eat(&PTok::OpenTag);
        let mut items = Vec::new();
        while !self.at(&PTok::Eof) {
            // A `?>` close tag (and a re-opening `<?php`) are tolerated between items.
            if self.eat(&PTok::CloseTag) {
                self.eat(&PTok::OpenTag);
                continue;
            }
            items.push(self.parse_item()?);
        }
        Ok(PhpProgram { items })
    }

    pub(super) fn parse_item(&mut self) -> Result<PhpItem, String> {
        if self.is_kw("function") {
            return Ok(PhpItem::Function(self.parse_function()?));
        }
        if self.is_kw("class") || self.is_kw("abstract") || self.is_kw("final") {
            return Ok(PhpItem::Class(self.parse_class()?));
        }
        if self.is_kw("enum") {
            return Ok(PhpItem::Enum(self.parse_enum()?));
        }
        // Everything else at top level is a file-level statement (the reserved-keyword guard in
        // `parse_stmt` rejects Tier-1-unsupported constructs like `try`/`interface`).
        Ok(PhpItem::Stmt(self.parse_stmt()?))
    }

    pub(super) fn parse_function(&mut self) -> Result<PhpFunction, String> {
        let line = self.line();
        self.advance(); // `function`
        let name = self.expect_ident("function name")?;
        let params = self.parse_params()?;
        let ret = if self.eat(&PTok::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(PhpFunction {
            name,
            params,
            ret,
            body,
            line,
        })
    }

    // ── classes / enums (L2b) ──

    /// The visibility keyword at the cursor (`public`/`private`/`protected`), if any. Does not consume.
    pub(super) fn visibility_kw(&self) -> Option<PhpVisibility> {
        match self.peek() {
            PTok::Ident(s) if s == "public" => Some(PhpVisibility::Public),
            PTok::Ident(s) if s == "private" => Some(PhpVisibility::Private),
            PTok::Ident(s) if s == "protected" => Some(PhpVisibility::Protected),
            _ => None,
        }
    }

    pub(super) fn parse_class(&mut self) -> Result<PhpClass, String> {
        let line = self.line();
        let mut is_abstract = false;
        let mut is_final = false;
        loop {
            if self.is_kw("abstract") {
                is_abstract = true;
                self.advance();
            } else if self.is_kw("final") {
                is_final = true;
                self.advance();
            } else {
                break;
            }
        }
        if !self.is_kw("class") {
            return Err(self.err("expected `class`"));
        }
        self.advance(); // `class`
        let name = self.expect_ident("class name")?;
        let extends = if self.is_kw("extends") {
            self.advance();
            Some(self.expect_ident("parent class name")?)
        } else {
            None
        };
        let implements = self.parse_implements()?;
        self.expect(&PTok::LBrace, "`{`")?;
        let mut members = Vec::new();
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            members.push(self.parse_member()?);
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(PhpClass {
            name,
            is_abstract,
            is_final,
            extends,
            implements,
            members,
            line,
        })
    }

    /// `implements A, B, …` — an empty list if the keyword is absent.
    pub(super) fn parse_implements(&mut self) -> Result<Vec<String>, String> {
        if !self.is_kw("implements") {
            return Ok(Vec::new());
        }
        self.advance();
        let mut v = vec![self.expect_ident("interface name")?];
        while self.eat(&PTok::Comma) {
            v.push(self.expect_ident("interface name")?);
        }
        Ok(v)
    }

    /// One class member: `const`, a method, or a property — preceded by any modifier order.
    pub(super) fn parse_member(&mut self) -> Result<PhpMember, String> {
        let mut vis = PhpVisibility::Public;
        let mut is_static = false;
        let mut is_abstract = false;
        let mut is_final = false;
        let mut is_readonly = false;
        loop {
            if let Some(v) = self.visibility_kw() {
                vis = v;
                self.advance();
            } else if self.is_kw("static") {
                is_static = true;
                self.advance();
            } else if self.is_kw("abstract") {
                is_abstract = true;
                self.advance();
            } else if self.is_kw("final") {
                is_final = true;
                self.advance();
            } else if self.is_kw("readonly") {
                is_readonly = true;
                self.advance();
            } else {
                break;
            }
        }
        if self.is_kw("const") {
            self.advance();
            let name = self.expect_ident("const name")?;
            self.expect(&PTok::Assign, "`=` in const")?;
            let value = self.parse_expr()?;
            self.expect(&PTok::Semi, "`;`")?;
            return Ok(PhpMember::Const { vis, name, value });
        }
        if self.is_kw("function") {
            return Ok(PhpMember::Method(self.parse_method(
                vis,
                is_static,
                is_abstract,
                is_final,
            )?));
        }
        // Otherwise a property: `[type] $name [= default];`.
        let ty = if self.at_type_start() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let name = self.expect_var("property name")?;
        let default = if self.eat(&PTok::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(&PTok::Semi, "`;`")?;
        Ok(PhpMember::Prop {
            vis,
            is_static,
            is_readonly,
            ty,
            name,
            default,
        })
    }

    pub(super) fn parse_method(
        &mut self,
        vis: PhpVisibility,
        is_static: bool,
        is_abstract: bool,
        is_final: bool,
    ) -> Result<PhpMethod, String> {
        let line = self.line();
        self.advance(); // `function`
        let name = self.expect_ident("method name")?;
        let params = self.parse_params()?;
        let ret = if self.eat(&PTok::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // An abstract method has no body — `function f();` — otherwise a brace block.
        let body = if self.eat(&PTok::Semi) {
            None
        } else {
            Some(self.parse_block()?)
        };
        Ok(PhpMethod {
            vis,
            is_static,
            is_abstract,
            is_final,
            name,
            params,
            ret,
            body,
            line,
        })
    }

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
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
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
                match self.parse_member()? {
                    PhpMember::Method(m) => methods.push(m),
                    _ => {
                        return Err(self.err("an enum may only contain cases and methods (Tier-1)"))
                    }
                }
            }
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(PhpEnum {
            name,
            backing,
            implements,
            cases,
            methods,
            line,
        })
    }

    /// `( param, param, … )` — tolerates a trailing comma. Each param is `[?]Type $name [= default]`.
    pub(super) fn parse_params(&mut self) -> Result<Vec<PhpParam>, String> {
        self.expect(&PTok::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.at(&PTok::RParen) {
            // Constructor promotion: a leading `public`/`private`/`protected` (optionally with
            // `readonly`) makes the param a promoted property.
            let mut promotion = None;
            loop {
                if let Some(v) = self.visibility_kw() {
                    promotion = Some(v);
                    self.advance();
                } else if self.is_kw("readonly") {
                    self.advance(); // readonly is accepted on a promoted param; flag not retained
                } else {
                    break;
                }
            }
            let ty = if self.at_type_start() {
                Some(self.parse_type()?)
            } else {
                None
            };
            let name = self.expect_var("parameter name")?;
            let default = if self.eat(&PTok::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(PhpParam {
                ty,
                name,
                default,
                promotion,
            });
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RParen, "`)`")?;
        Ok(params)
    }

    /// A type hint begins with `?` (nullable) or a bare type-name identifier.
    pub(super) fn at_type_start(&self) -> bool {
        self.at(&PTok::Question) || matches!(self.peek(), PTok::Ident(_))
    }

    pub(super) fn parse_type(&mut self) -> Result<PhpType, String> {
        if self.eat(&PTok::Question) {
            return Ok(PhpType::Nullable(Box::new(self.parse_type()?)));
        }
        let name = self.expect_ident("type name")?;
        Ok(PhpType::Named(name))
    }

    /// `{ stmt* }`.
    pub(super) fn parse_block(&mut self) -> Result<Vec<PhpStmt>, String> {
        self.expect(&PTok::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !self.at(&PTok::RBrace) && !self.at(&PTok::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(stmts)
    }

    /// Parse one statement, or — when the next token isn't `{` — a single brace-less statement (so
    /// `if ($x) return;` works). Used for `if`/`while`/`for`/`foreach` bodies.
    pub(super) fn parse_body(&mut self) -> Result<Vec<PhpStmt>, String> {
        if self.at(&PTok::LBrace) {
            self.parse_block()
        } else {
            Ok(vec![self.parse_stmt()?])
        }
    }

    // ── statements ──
}

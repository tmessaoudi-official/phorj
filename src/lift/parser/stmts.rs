//! PHP-lift parser — statements.

use super::*;

impl PParser {
    pub(super) fn parse_stmt(&mut self) -> Result<PhpStmt, String> {
        // Reject Tier-1-unsupported leading keywords loudly (never misread as an expression).
        if let PTok::Ident(w) = self.peek() {
            if UNSUPPORTED_KW.contains(&w.as_str()) {
                return Err(self.err(&format!("`{w}` is not supported in Tier-1")));
            }
        }
        if self.at(&PTok::LBrace) {
            return Ok(PhpStmt::Block(self.parse_block()?));
        }
        if self.is_kw("return") {
            self.advance();
            let e = if self.at(&PTok::Semi) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect(&PTok::Semi, "`;`")?;
            return Ok(PhpStmt::Return(e));
        }
        if self.is_kw("if") {
            return self.parse_if();
        }
        if self.is_kw("while") {
            self.advance();
            self.expect(&PTok::LParen, "`(`")?;
            let cond = self.parse_expr()?;
            self.expect(&PTok::RParen, "`)`")?;
            let body = self.parse_body()?;
            return Ok(PhpStmt::While { cond, body });
        }
        if self.is_kw("for") {
            return self.parse_for();
        }
        if self.is_kw("foreach") {
            return self.parse_foreach();
        }
        if self.is_kw("echo") {
            self.advance();
            let mut args = vec![self.parse_expr()?];
            while self.eat(&PTok::Comma) {
                args.push(self.parse_expr()?);
            }
            self.expect(&PTok::Semi, "`;`")?;
            return Ok(PhpStmt::Echo(args));
        }
        if self.is_kw("break") {
            self.advance();
            self.expect(&PTok::Semi, "`;`")?;
            return Ok(PhpStmt::Break);
        }
        if self.is_kw("continue") {
            self.advance();
            self.expect(&PTok::Semi, "`;`")?;
            return Ok(PhpStmt::Continue);
        }
        // Fallthrough: an expression statement.
        let e = self.parse_expr()?;
        self.expect(&PTok::Semi, "`;`")?;
        Ok(PhpStmt::Expr(e))
    }

    pub(super) fn parse_if(&mut self) -> Result<PhpStmt, String> {
        self.advance(); // `if`
        self.expect(&PTok::LParen, "`(`")?;
        let cond = self.parse_expr()?;
        self.expect(&PTok::RParen, "`)`")?;
        let then = self.parse_body()?;
        let mut elifs = Vec::new();
        let mut els = None;
        loop {
            if self.is_kw("elseif") {
                self.advance();
                self.expect(&PTok::LParen, "`(`")?;
                let c = self.parse_expr()?;
                self.expect(&PTok::RParen, "`)`")?;
                elifs.push((c, self.parse_body()?));
            } else if self.is_kw("else") {
                self.advance();
                if self.is_kw("if") {
                    // `else if` (two words) — same as `elseif`.
                    self.advance();
                    self.expect(&PTok::LParen, "`(`")?;
                    let c = self.parse_expr()?;
                    self.expect(&PTok::RParen, "`)`")?;
                    elifs.push((c, self.parse_body()?));
                } else {
                    els = Some(self.parse_body()?);
                    break;
                }
            } else {
                break;
            }
        }
        Ok(PhpStmt::If {
            cond,
            then,
            elifs,
            els,
        })
    }

    pub(super) fn parse_for(&mut self) -> Result<PhpStmt, String> {
        self.advance(); // `for`
        self.expect(&PTok::LParen, "`(`")?;
        let init = if self.at(&PTok::Semi) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&PTok::Semi, "`;`")?;
        let cond = if self.at(&PTok::Semi) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&PTok::Semi, "`;`")?;
        let step = if self.at(&PTok::RParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&PTok::RParen, "`)`")?;
        let body = self.parse_body()?;
        Ok(PhpStmt::For {
            init,
            cond,
            step,
            body,
        })
    }

    pub(super) fn parse_foreach(&mut self) -> Result<PhpStmt, String> {
        self.advance(); // `foreach`
        self.expect(&PTok::LParen, "`(`")?;
        let array = self.parse_expr()?;
        if !self.is_kw("as") {
            return Err(self.err("expected `as` in foreach"));
        }
        self.advance(); // `as`
        let first = self.expect_var("foreach variable")?;
        let (key, value) = if self.eat(&PTok::FatArrow) {
            (Some(first), self.expect_var("foreach value variable")?)
        } else {
            (None, first)
        };
        self.expect(&PTok::RParen, "`)`")?;
        let body = self.parse_body()?;
        Ok(PhpStmt::Foreach {
            array,
            key,
            value,
            body,
        })
    }

    // ── expressions ──
}

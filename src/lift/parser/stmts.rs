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
        // `throw <expr>;` — the statement form. PHP 8's throw-as-expression is not handled here and is
        // left to the expression parser to refuse, so it reports rather than being silently relocated.
        if self.is_kw("throw") {
            self.advance();
            let e = self.parse_expr()?;
            self.expect(&PTok::Semi, "`;` after `throw`")?;
            return Ok(PhpStmt::Throw(e));
        }
        // LIFT-TRY: `try { … } catch (T $e) { … }* finally { … }?`.
        if self.is_kw("try") {
            self.advance();
            let body = self.parse_block()?;
            let mut catches = Vec::new();
            while self.is_kw("catch") {
                self.advance();
                self.expect(&PTok::LParen, "`(` after `catch`")?;
                // A union type list: `catch (A | B $e)`. At least one member is required.
                let mut types = vec![self.parse_qualified_name()?];
                while self.eat(&PTok::Bar) {
                    types.push(self.parse_qualified_name()?);
                }
                // PHP 8 permits `catch (T)` with NO variable — hence `Option`, not a required name.
                let var = match self.peek() {
                    PTok::Var(v) => {
                        let v = v.clone();
                        self.advance();
                        Some(v)
                    }
                    _ => None,
                };
                self.expect(&PTok::RParen, "`)` closing the catch clause")?;
                catches.push(PhpCatch {
                    types,
                    var,
                    body: self.parse_block()?,
                });
            }
            let finally_block = if self.is_kw("finally") {
                self.advance();
                Some(self.parse_block()?)
            } else {
                None
            };
            // `try` alone is a PHP syntax error; requiring one arm here reports it as a lift error
            // rather than silently producing a bare block.
            if catches.is_empty() && finally_block.is_none() {
                return Err(self.err("`try` needs at least one `catch` or a `finally`"));
            }
            return Ok(PhpStmt::Try {
                body,
                catches,
                finally_block,
            });
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

impl super::PParser {
    /// A class name, optionally `\`-qualified (`\RuntimeException`) or namespaced (`Acme\MyError`).
    ///
    /// Shared by catch clauses and `new`: PHP writes root-namespace builtins qualified, so
    /// `throw new \RuntimeException(…)` inside a `catch (\RuntimeException $e)` needs BOTH to read the
    /// same way. The `\` is kept verbatim here — the LIFTER strips the root marker, keeping the parser a
    /// faithful reader of the source.
    pub(super) fn parse_qualified_name(&mut self) -> Result<String, String> {
        let mut out = String::new();
        if self.eat(&PTok::Backslash) {
            out.push('\\');
        }
        loop {
            match self.peek() {
                PTok::Ident(n) => {
                    out.push_str(n);
                    self.advance();
                }
                _ => return Err(self.err("a class name in the catch clause")),
            }
            if self.eat(&PTok::Backslash) {
                out.push('\\');
                continue;
            }
            break;
        }
        Ok(out)
    }
}

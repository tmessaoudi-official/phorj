//! M-Lift L2 — a recursive-descent + precedence-climbing parser for the **Tier-1 PHP** subset,
//! turning the [`super::lexer`] token stream into a [`super::ast::PhpProgram`].
//!
//! Mirrors the house parser style (`src/parser/`): cursor helpers, precedence climbing via
//! [`infix_op`], a `depth` guard against [`MAX_NEST_DEPTH`] (the input is untrusted PHP). Errors are
//! line-numbered `lift parse error:` strings, like the lexer — and anything outside Tier-1 is
//! rejected *loudly* rather than represented and guessed at (the never-guess contract).
//!
//! Precedence follows **PHP 8**: concatenation `.` binds *looser* than `+`/`-` but *tighter* than the
//! comparison operators — a real 8.0 change, pinned by tests.

use super::ast::{
    PhpArrayElem, PhpBinOp, PhpClass, PhpEnum, PhpEnumCase, PhpExpr, PhpFunction, PhpItem,
    PhpMatchArm, PhpMember, PhpMethod, PhpParam, PhpProgram, PhpStmt, PhpType, PhpUnOp,
    PhpVisibility,
};
use super::lexer::{PTok, PTokenSpanned};
use crate::limits::MAX_NEST_DEPTH;

/// Keywords that exist in PHP but are outside the Tier-1 subset. Encountered in statement-leading
/// position they produce a clear "not supported" error rather than being misread as an expression.
const UNSUPPORTED_KW: &[&str] = &[
    "try",
    "catch",
    "finally",
    "switch",
    "throw",
    "do",
    "namespace",
    "use",
    "trait",
    "interface",
    "global",
    "goto",
    "declare",
    "const",
    "static",
    "function", // a *nested* function is a closure-ish construct; top-level fns are caught earlier
    "fn",
];

/// PHP cast type names (`(int)$x`). Detected to reject casts loudly (Tier-2) instead of misparsing.
const CAST_TYPES: &[&str] = &[
    "int", "integer", "float", "double", "string", "bool", "boolean", "array", "object",
];

struct PParser {
    toks: Vec<PTokenSpanned>,
    pos: usize,
    /// Live expression-nesting depth, checked in [`PParser::parse_unary`] (every operand passes
    /// through it once per level) to bound recursion on pathologically nested input.
    depth: usize,
}

/// Parse a Tier-1 PHP token stream into a [`PhpProgram`]. The stream must end in [`PTok::Eof`]
/// (the lexer guarantees this).
pub fn parse_php(toks: Vec<PTokenSpanned>) -> Result<PhpProgram, String> {
    let mut p = PParser {
        toks,
        pos: 0,
        depth: 0,
    };
    p.parse_program()
}

impl PParser {
    // ── cursor ──

    fn peek(&self) -> &PTok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }

    fn peek_at(&self, n: usize) -> &PTok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].line
    }

    fn advance(&mut self) -> PTok {
        let tok = self.toks[self.pos.min(self.toks.len() - 1)].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn at(&self, tok: &PTok) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(tok)
    }

    fn eat(&mut self, tok: &PTok) -> bool {
        if self.at(tok) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume a payload-free token of the expected kind, or error with `what`.
    fn expect(&mut self, tok: &PTok, what: &str) -> Result<(), String> {
        if self.at(tok) {
            self.advance();
            Ok(())
        } else {
            Err(self.err(&format!("expected {what}")))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            PTok::Ident(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(self.err(&format!("expected {what}"))),
        }
    }

    /// Consume a `$var`, returning the name (without `$`), or error.
    fn expect_var(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            PTok::Var(n) => {
                self.advance();
                Ok(n)
            }
            _ => Err(self.err(&format!("expected {what}"))),
        }
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), PTok::Ident(s) if s == kw)
    }

    fn err(&self, msg: &str) -> String {
        format!(
            "lift parse error: {msg}, found {:?} (line {})",
            self.peek(),
            self.line()
        )
    }

    // ── program / items ──

    fn parse_program(&mut self) -> Result<PhpProgram, String> {
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

    fn parse_item(&mut self) -> Result<PhpItem, String> {
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

    fn parse_function(&mut self) -> Result<PhpFunction, String> {
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
    fn visibility_kw(&self) -> Option<PhpVisibility> {
        match self.peek() {
            PTok::Ident(s) if s == "public" => Some(PhpVisibility::Public),
            PTok::Ident(s) if s == "private" => Some(PhpVisibility::Private),
            PTok::Ident(s) if s == "protected" => Some(PhpVisibility::Protected),
            _ => None,
        }
    }

    fn parse_class(&mut self) -> Result<PhpClass, String> {
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
    fn parse_implements(&mut self) -> Result<Vec<String>, String> {
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
    fn parse_member(&mut self) -> Result<PhpMember, String> {
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

    fn parse_method(
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

    fn parse_enum(&mut self) -> Result<PhpEnum, String> {
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
    fn parse_params(&mut self) -> Result<Vec<PhpParam>, String> {
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
    fn at_type_start(&self) -> bool {
        self.at(&PTok::Question) || matches!(self.peek(), PTok::Ident(_))
    }

    fn parse_type(&mut self) -> Result<PhpType, String> {
        if self.eat(&PTok::Question) {
            return Ok(PhpType::Nullable(Box::new(self.parse_type()?)));
        }
        let name = self.expect_ident("type name")?;
        Ok(PhpType::Named(name))
    }

    /// `{ stmt* }`.
    fn parse_block(&mut self) -> Result<Vec<PhpStmt>, String> {
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
    fn parse_body(&mut self) -> Result<Vec<PhpStmt>, String> {
        if self.at(&PTok::LBrace) {
            self.parse_block()
        } else {
            Ok(vec![self.parse_stmt()?])
        }
    }

    // ── statements ──

    fn parse_stmt(&mut self) -> Result<PhpStmt, String> {
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

    fn parse_if(&mut self) -> Result<PhpStmt, String> {
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

    fn parse_for(&mut self) -> Result<PhpStmt, String> {
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

    fn parse_foreach(&mut self) -> Result<PhpStmt, String> {
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

    fn parse_expr(&mut self) -> Result<PhpExpr, String> {
        self.parse_assign()
    }

    /// Assignment level (lowest, right-associative): `=` and the compound forms `+= .= ??= …`.
    fn parse_assign(&mut self) -> Result<PhpExpr, String> {
        let lhs = self.parse_ternary()?;
        if self.at(&PTok::Assign) {
            if !is_lvalue(&lhs) {
                return Err(self.err("invalid assignment target"));
            }
            self.advance();
            let value = self.parse_assign()?;
            return Ok(PhpExpr::Assign {
                target: Box::new(lhs),
                value: Box::new(value),
            });
        }
        if let Some(op) = compound_op(self.peek()) {
            if !is_lvalue(&lhs) {
                return Err(self.err("invalid assignment target"));
            }
            self.advance();
            let value = self.parse_assign()?;
            return Ok(PhpExpr::CompoundAssign {
                target: Box::new(lhs),
                op,
                value: Box::new(value),
            });
        }
        Ok(lhs)
    }

    /// Ternary `cond ? then : els` and the elvis form `cond ?: els` (then = `None`).
    fn parse_ternary(&mut self) -> Result<PhpExpr, String> {
        let cond = self.parse_coalesce()?;
        if self.eat(&PTok::Question) {
            let then = if self.at(&PTok::Colon) {
                None
            } else {
                Some(Box::new(self.parse_assign()?))
            };
            self.expect(&PTok::Colon, "`:` in ternary")?;
            let els = self.parse_assign()?;
            return Ok(PhpExpr::Ternary {
                cond: Box::new(cond),
                then,
                els: Box::new(els),
            });
        }
        Ok(cond)
    }

    /// Null-coalesce `??` (right-associative, below the left-assoc binary operators).
    fn parse_coalesce(&mut self) -> Result<PhpExpr, String> {
        let left = self.parse_binary(0)?;
        if self.eat(&PTok::Coalesce) {
            let right = self.parse_coalesce()?;
            return Ok(PhpExpr::Binary {
                op: PhpBinOp::Coalesce,
                left: Box::new(left),
                right: Box::new(right),
            });
        }
        Ok(left)
    }

    /// Precedence-climbing over the left-associative binary operators (PHP-8 table — see [`infix_op`]).
    fn parse_binary(&mut self, min_bp: u8) -> Result<PhpExpr, String> {
        let mut left = self.parse_unary()?;
        while let Some((bp, op)) = infix_op(self.peek()) {
            if bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_binary(bp + 1)?;
            left = PhpExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<PhpExpr, String> {
        self.depth += 1;
        if self.depth > MAX_NEST_DEPTH {
            return Err(self.err("expression nests too deeply"));
        }
        let e = self.parse_unary_inner()?;
        self.depth -= 1;
        Ok(e)
    }

    fn parse_unary_inner(&mut self) -> Result<PhpExpr, String> {
        if self.eat(&PTok::Not) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.eat(&PTok::Minus) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::Neg,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.eat(&PTok::Tilde) {
            return Ok(PhpExpr::Unary {
                op: PhpUnOp::BitNot,
                expr: Box::new(self.parse_unary()?),
            });
        }
        // Prefix increment/decrement.
        if self.at(&PTok::Inc) || self.at(&PTok::Dec) {
            let inc = self.at(&PTok::Inc);
            self.advance();
            let target = self.parse_unary()?;
            if !is_lvalue(&target) {
                return Err(self.err("invalid increment/decrement target"));
            }
            return Ok(PhpExpr::IncDec {
                target: Box::new(target),
                inc,
                prefix: true,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<PhpExpr, String> {
        let mut e = self.parse_primary()?;
        loop {
            if self.at(&PTok::LParen) {
                let args = self.parse_args()?;
                e = PhpExpr::Call {
                    callee: Box::new(e),
                    args,
                };
            } else if self.at(&PTok::Arrow) || self.at(&PTok::NullArrow) {
                let nullsafe = self.at(&PTok::NullArrow);
                self.advance();
                let name = self.expect_ident("member name")?;
                if self.at(&PTok::LParen) {
                    let args = self.parse_args()?;
                    e = PhpExpr::MethodCall {
                        recv: Box::new(e),
                        name,
                        args,
                        nullsafe,
                    };
                } else {
                    e = PhpExpr::Member {
                        recv: Box::new(e),
                        name,
                        nullsafe,
                    };
                }
            } else if self.at(&PTok::DoubleColon) {
                e = self.parse_static_access(e)?;
            } else if self.at(&PTok::LBracket) {
                self.advance();
                if self.at(&PTok::RBracket) {
                    return Err(self.err("empty `[]` (array append) is Tier-2"));
                }
                let index = self.parse_expr()?;
                self.expect(&PTok::RBracket, "`]`")?;
                e = PhpExpr::Index {
                    base: Box::new(e),
                    index: Box::new(index),
                };
            } else if self.at(&PTok::Inc) || self.at(&PTok::Dec) {
                let inc = self.at(&PTok::Inc);
                if !is_lvalue(&e) {
                    return Err(self.err("invalid increment/decrement target"));
                }
                self.advance();
                e = PhpExpr::IncDec {
                    target: Box::new(e),
                    inc,
                    prefix: false,
                };
            } else {
                break;
            }
        }
        // C-46: `value instanceof ClassName` — a single, non-associative trailing clause at the
        // postfix level (binds tighter than the `!`/`-`/`~` unary layer above). A dynamic RHS
        // (`$x instanceof $cls`) has no Phorge equivalent and is refused loudly.
        if matches!(self.peek(), PTok::Ident(w) if w == "instanceof") {
            self.advance();
            if matches!(self.peek(), PTok::Var(_)) {
                return Err(self.err("dynamic `instanceof $var` is Tier-2"));
            }
            let class = self.expect_ident("a class name after `instanceof`")?;
            e = PhpExpr::InstanceOf {
                value: Box::new(e),
                class,
            };
        }
        Ok(e)
    }

    /// `Class::CONST` / `Class::$prop` / `Class::method(args)`. The left side must be a class name
    /// (`Name`) — a dynamic `$obj::…` is Tier-3 and rejected.
    fn parse_static_access(&mut self, lhs: PhpExpr) -> Result<PhpExpr, String> {
        let class = match lhs {
            PhpExpr::Name(n) => n,
            _ => return Err(self.err("dynamic `::` access is Tier-3")),
        };
        self.advance(); // `::`
        if let PTok::Var(prop) = self.peek().clone() {
            self.advance();
            return Ok(PhpExpr::StaticProp { class, name: prop });
        }
        let name = self.expect_ident("static member name")?;
        if self.at(&PTok::LParen) {
            let args = self.parse_args()?;
            Ok(PhpExpr::StaticCall { class, name, args })
        } else {
            Ok(PhpExpr::ClassConst { class, name })
        }
    }

    /// `( expr, expr, … )` — tolerates a trailing comma.
    fn parse_args(&mut self) -> Result<Vec<PhpExpr>, String> {
        self.expect(&PTok::LParen, "`(`")?;
        let mut args = Vec::new();
        while !self.at(&PTok::RParen) {
            args.push(self.parse_expr()?);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RParen, "`)`")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<PhpExpr, String> {
        match self.peek().clone() {
            PTok::Int(n) => {
                self.advance();
                Ok(PhpExpr::Int(n))
            }
            PTok::Float(f) => {
                self.advance();
                Ok(PhpExpr::Float(f))
            }
            PTok::Str(s) => {
                self.advance();
                Ok(PhpExpr::Str(s))
            }
            PTok::InterpStr(_) => Err(self.err("string interpolation is Tier-2")),
            PTok::Var(name) => {
                self.advance();
                Ok(PhpExpr::Var(name))
            }
            PTok::LParen => {
                // Reject a C-style cast `(int)$x` rather than misparsing it.
                if let PTok::Ident(t) = self.peek_at(1) {
                    if CAST_TYPES.contains(&t.as_str()) && matches!(self.peek_at(2), PTok::RParen) {
                        return Err(self.err("cast expressions are Tier-2"));
                    }
                }
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&PTok::RParen, "`)`")?;
                Ok(inner)
            }
            PTok::LBracket => self.parse_array(),
            PTok::Ident(word) => self.parse_ident_primary(&word),
            _ => Err(self.err("expected an expression")),
        }
    }

    fn parse_ident_primary(&mut self, word: &str) -> Result<PhpExpr, String> {
        match word {
            "true" => {
                self.advance();
                Ok(PhpExpr::Bool(true))
            }
            "false" => {
                self.advance();
                Ok(PhpExpr::Bool(false))
            }
            "null" => {
                self.advance();
                Ok(PhpExpr::Null)
            }
            "new" => self.parse_new(),
            "match" => self.parse_match(),
            "function" | "fn" => Err(self.err("closures and arrow functions are Tier-2")),
            "clone" | "print" | "yield" | "throw" | "include" | "require" | "include_once"
            | "require_once" => Err(self.err(&format!("`{word}` is Tier-2/Tier-3"))),
            _ => {
                self.advance();
                Ok(PhpExpr::Name(word.to_string()))
            }
        }
    }

    fn parse_new(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `new`
        if matches!(self.peek(), PTok::Var(_)) {
            return Err(self.err("dynamic `new $class` is Tier-3"));
        }
        let class = self.expect_ident("class name after `new`")?;
        let args = if self.at(&PTok::LParen) {
            self.parse_args()?
        } else {
            Vec::new()
        };
        Ok(PhpExpr::New { class, args })
    }

    fn parse_array(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `[`
        let mut elems = Vec::new();
        while !self.at(&PTok::RBracket) {
            let first = self.parse_expr()?;
            let elem = if self.eat(&PTok::FatArrow) {
                PhpArrayElem {
                    key: Some(first),
                    value: self.parse_expr()?,
                }
            } else {
                PhpArrayElem {
                    key: None,
                    value: first,
                }
            };
            elems.push(elem);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RBracket, "`]`")?;
        Ok(PhpExpr::Array(elems))
    }

    fn parse_match(&mut self) -> Result<PhpExpr, String> {
        self.advance(); // `match`
        self.expect(&PTok::LParen, "`(`")?;
        let subject = self.parse_expr()?;
        self.expect(&PTok::RParen, "`)`")?;
        self.expect(&PTok::LBrace, "`{`")?;
        let mut arms = Vec::new();
        while !self.at(&PTok::RBrace) {
            let conds = if self.is_kw("default") {
                self.advance();
                None
            } else {
                let mut cs = vec![self.parse_expr()?];
                while self.eat(&PTok::Comma) {
                    if self.at(&PTok::FatArrow) {
                        break; // tolerate a trailing comma before `=>`
                    }
                    cs.push(self.parse_expr()?);
                }
                Some(cs)
            };
            self.expect(&PTok::FatArrow, "`=>` in match arm")?;
            let body = self.parse_expr()?;
            arms.push(PhpMatchArm { conds, body });
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RBrace, "`}`")?;
        Ok(PhpExpr::Match {
            subject: Box::new(subject),
            arms,
        })
    }
}

/// Left binding power + `PhpBinOp` for an infix operator token (the left-associative subset).
/// `??`, ternary, and assignment are handled in their own recursive layers, so they are absent here.
/// **PHP 8 precedence** (higher binds tighter): `* / %` (11) > `+ -` (10) > `<< >>` (9) > `.` (8) >
/// comparison (7) > equality (6) > `&` (5) > `^` (4) > `|` (3) > `&&` (2) > `||` (1). (C-47 inserts
/// the bitwise/shift levels; the prior ops keep their relative order.)
fn infix_op(tok: &PTok) -> Option<(u8, PhpBinOp)> {
    Some(match tok {
        PTok::OrOr => (1, PhpBinOp::Or),
        PTok::AndAnd => (2, PhpBinOp::And),
        PTok::Bar => (3, PhpBinOp::BitOr),
        PTok::Caret => (4, PhpBinOp::BitXor),
        PTok::Amp => (5, PhpBinOp::BitAnd),
        PTok::EqEq => (6, PhpBinOp::Eq),
        PTok::EqEqEq => (6, PhpBinOp::Identical),
        PTok::NotEq => (6, PhpBinOp::NotEq),
        PTok::NotEqEq => (6, PhpBinOp::NotIdentical),
        PTok::Lt => (7, PhpBinOp::Lt),
        PTok::Le => (7, PhpBinOp::Le),
        PTok::Gt => (7, PhpBinOp::Gt),
        PTok::Ge => (7, PhpBinOp::Ge),
        PTok::Dot => (8, PhpBinOp::Concat),
        PTok::Shl => (9, PhpBinOp::Shl),
        PTok::Shr => (9, PhpBinOp::Shr),
        PTok::Plus => (10, PhpBinOp::Add),
        PTok::Minus => (10, PhpBinOp::Sub),
        PTok::Star => (11, PhpBinOp::Mul),
        PTok::Slash => (11, PhpBinOp::Div),
        PTok::Percent => (11, PhpBinOp::Rem),
        _ => return None,
    })
}

/// Map a compound-assignment token to the `PhpBinOp` it combines with (`+=` → `Add`, `??=` →
/// `Coalesce`, …). `None` for any non-compound token.
fn compound_op(tok: &PTok) -> Option<PhpBinOp> {
    Some(match tok {
        PTok::PlusEq => PhpBinOp::Add,
        PTok::MinusEq => PhpBinOp::Sub,
        PTok::StarEq => PhpBinOp::Mul,
        PTok::SlashEq => PhpBinOp::Div,
        PTok::PercentEq => PhpBinOp::Rem,
        PTok::DotEq => PhpBinOp::Concat,
        PTok::CoalesceEq => PhpBinOp::Coalesce,
        _ => return None,
    })
}

/// A valid assignment / increment target: a variable, an index, an instance/static property.
fn is_lvalue(e: &PhpExpr) -> bool {
    matches!(
        e,
        PhpExpr::Var(_)
            | PhpExpr::Index { .. }
            | PhpExpr::Member { .. }
            | PhpExpr::StaticProp { .. }
    )
}

//! PHP-lift parser — expressions: precedence climbing, operator tables, chains. The interpolation
//! scanner lives in `interp.rs` and the assignment-target helpers in `targets.rs` (Invariant 13).

use super::interp::parse_interp;
use super::targets::{destructure_binders, is_lvalue};
use super::*;

impl PParser {
    pub(super) fn parse_expr(&mut self) -> Result<PhpExpr, String> {
        self.parse_assign()
    }

    /// Assignment level (lowest, right-associative): `=` and the compound forms `+= .= ??= …`.
    pub(super) fn parse_assign(&mut self) -> Result<PhpExpr, String> {
        let lhs = self.parse_ternary()?;
        if self.at(&PTok::Assign) {
            // DEC-510: `[$a, $b] = …` and `list($a, $b) = …` are DESTRUCTURING, not an assignment to
            // an array literal. Both spellings mean the same thing and both lift to phorj's
            // `var (a, b) = …`. Recognised here, before the lvalue test, because the LHS has already
            // parsed as an array literal (or as a call to `list`) by the time we get here.
            if let Some(binders) = destructure_binders(&lhs)? {
                self.advance();
                let value = self.parse_assign()?;
                return Ok(PhpExpr::Destructure {
                    binders,
                    value: Box::new(value),
                });
            }
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
    pub(super) fn parse_ternary(&mut self) -> Result<PhpExpr, String> {
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
    pub(super) fn parse_coalesce(&mut self) -> Result<PhpExpr, String> {
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
    pub(super) fn parse_binary(&mut self, min_bp: u8) -> Result<PhpExpr, String> {
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

    pub(super) fn parse_unary(&mut self) -> Result<PhpExpr, String> {
        self.depth += 1;
        if self.depth > MAX_NEST_DEPTH {
            return Err(self.err("expression nests too deeply"));
        }
        let e = self.parse_unary_inner()?;
        self.depth -= 1;
        Ok(e)
    }

    pub(super) fn parse_unary_inner(&mut self) -> Result<PhpExpr, String> {
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

    pub(super) fn parse_postfix(&mut self) -> Result<PhpExpr, String> {
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
                e = if self.eat(&PTok::RBracket) {
                    PhpExpr::AppendSlot(Box::new(e)) // `$xs[]` — only ever the target of `=`
                } else {
                    let index = self.parse_expr()?;
                    self.expect(&PTok::RBracket, "`]`")?;
                    PhpExpr::Index {
                        base: Box::new(e),
                        index: Box::new(index),
                    }
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
        // C-46: `value instanceof ClassName` — a single, non-associative trailing clause at the postfix
        // level (tighter than the unary layer above). A dynamic RHS (`$x instanceof $cls`) is refused loudly.
        if matches!(self.peek(), PTok::Ident(w) if w == "instanceof") {
            self.advance();
            if matches!(self.peek(), PTok::Var(_)) {
                return Err(self.err("dynamic `instanceof $var` is Tier-2"));
            }
            let class = self.parse_class_ref()?; // bare, or `\\A\\B` → implicit `use`
            e = PhpExpr::InstanceOf {
                value: Box::new(e),
                class,
            };
        }
        Ok(e)
    }

    /// `Class::CONST` / `Class::$prop` / `Class::method(args)`. The left side must be a class name
    /// (`Name`) — a dynamic `$obj::…` is Tier-3 and rejected.
    pub(super) fn parse_static_access(&mut self, lhs: PhpExpr) -> Result<PhpExpr, String> {
        let class = match lhs {
            PhpExpr::Name(n) => n,
            _ => return Err(self.err("dynamic `::` access is Tier-3")),
        };
        // The third `self` site (Lane R-8): `self::CONST` / `self::method()` — 411 occurrences in
        // scout's source, 19 of them in files that already lift, where the draft said `self::X` and
        // phorj reported `unknown identifier self`. Same rule as the type and `new` positions: the
        // enclosing class exactly, and `static::` keeps its refusal (late static binding).
        let class = self.resolve_self_class(class)?;
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
    pub(super) fn parse_args(&mut self) -> Result<Vec<PhpExpr>, String> {
        self.expect(&PTok::LParen, "`(`")?;
        let mut args = Vec::new();
        while !self.at(&PTok::RParen) {
            args.push(self.parse_arg()?);
            if !self.eat(&PTok::Comma) {
                break;
            }
        }
        self.expect(&PTok::RParen, "`)`")?;
        Ok(args)
    }

    pub(super) fn parse_primary(&mut self) -> Result<PhpExpr, String> {
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
            PTok::InterpStr(raw) => {
                let raw = raw.clone();
                self.advance();
                Ok(PhpExpr::Interp(parse_interp(&raw)?))
            }
            PTok::Var(name) => {
                self.advance();
                Ok(PhpExpr::Var(name))
            }
            PTok::LParen => {
                if self.at_cast() {
                    return self.parse_cast();
                }
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&PTok::RParen, "`)`")?;
                Ok(inner)
            }
            PTok::LBracket => self.parse_array(),
            PTok::Backslash => self.parse_root_qualified(),
            PTok::Ident(word) => self.parse_ident_primary(&word),
            _ => Err(self.err("expected an expression")),
        }
    }

    pub(super) fn parse_ident_primary(&mut self, word: &str) -> Result<PhpExpr, String> {
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
            "fn" => self.parse_arrow_closure(),
            "static" if self.at_static_fn() => self.parse_arrow_closure(),
            "static" if matches!(self.peek_at(1), PTok::Ident(w) if w == "function") => {
                self.parse_block_closure()
            }
            "function" => self.parse_block_closure(),
            "clone" | "print" | "yield" | "throw" | "include" | "require" | "include_once"
            | "require_once" => Err(self.err(&format!("`{word}` is Tier-2/Tier-3"))),
            _ => {
                self.advance();
                Ok(PhpExpr::Name(word.to_string()))
            }
        }
    }

    pub(super) fn parse_array(&mut self) -> Result<PhpExpr, String> {
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

    pub(super) fn parse_match(&mut self) -> Result<PhpExpr, String> {
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
pub(super) fn infix_op(tok: &PTok) -> Option<(u8, PhpBinOp)> {
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
        // `<=>` is in PHP's EQUALITY tier, alongside `==`/`!=` — and non-associative there, which a
        // left-associative fold accepts as a superset. Same tier the Phorj parser gives it.
        PTok::Spaceship => (6, PhpBinOp::Spaceship),
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

/// Map a compound-assignment token to the `PhpBinOp` it combines with (`+=` → `Add`, `??=` → `Coalesce`, …); `None` for a non-compound token.
pub(super) fn compound_op(tok: &PTok) -> Option<PhpBinOp> {
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

// ── C-1: string interpolation ──
// PHP's double-quoted interpolation grammar is exactly a `$`-rooted *access chain* — a variable
// followed by `->prop` / `[idx]` / method-call steps; a top-level operator is a PHP parse error
// (verified against 8.5: `"{$a + $b}"` errors with `expecting "->" or "?->" or "["`). That is also
// precisely Phorj's `"{…}"` hole grammar, so the faithful subset round-trips 1:1. Anything richer
// (variable-variable `${…}`, dynamic `{$o->$p}`, a bareword simple subscript whose key silently
// coerces to a string) is rejected loudly — never lifted to a guess.

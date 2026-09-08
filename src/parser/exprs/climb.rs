//! Expression parsing — precedence climbing: ranges, binary/unary.
//!
//! The postfix chain (calls, indexing, member access, `new`, parent dispatch, argument lists) is
//! in the sibling `postfix.rs` — split under Invariant 13, see its header.

use super::*;

impl Parser {
    /// Entry point: parse a full expression (lowest precedence). Every fresh expression context —
    /// including a bracketed sub-expression (parens / call args / index / list & map literals all
    /// re-enter here) — re-enables the `as`-cast fold: the `foreach` separator-vs-cast ambiguity
    /// (M4 casting) only exists at the *top level* of the iterable, never inside brackets.
    pub fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let saved = self.no_as_cast;
        self.no_as_cast = false;
        let r = self.parse_range();
        self.no_as_cast = saved;
        r
    }

    /// Ranges bind looser than every binary operator: `a..b` reads `a` and `b` as full
    /// (binary) sub-expressions, so `0..n + 1` is `0..(n + 1)`. Non-chaining (no `a..b..c`); a
    /// single optional `..`/`..=` follows the first operand. Used mainly as `for (int i in 0..n)`.
    pub(in crate::parser) fn parse_range(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.parse_binary(0)?;
        let inclusive = match self.peek() {
            TokenKind::DotDot => false,
            TokenKind::DotDotEq => true,
            _ => return Ok(start),
        };
        let sp = self.peek_span();
        self.advance(); // consume `..` / `..=`
        let end = self.parse_binary(0)?;
        Ok(Expr::Range {
            start: Box::new(start),
            end: Box::new(end),
            inclusive,
            span: sp,
        })
    }

    /// Left binding power for an infix operator token, plus its `BinaryOp`.
    /// Returns None if the token is not an infix operator. Higher binds tighter.
    pub(in crate::parser) fn infix_op(kind: &TokenKind) -> Option<(u8, BinaryOp)> {
        use TokenKind as T;
        // Precedence follows PHP (higher binds tighter): `??` `||` `&&` then bitwise
        // `|` `^` `&`, then `==`/`!=`, comparison, `|>`, shifts, `+ -`, `* / %`. Shift-right `>>`
        // is not a token (two `Gt`); it is handled at level 11 directly in `parse_binary`.
        // DEC-239 precedence fix: `|>` sits in PHP 8.5's exact slot — tighter than comparison
        // (`x |> f == 6` is `(x |> f) == 6`), looser than shifts/arithmetic (`10 + 6 |> inc` is
        // `(10 + 6) |> inc` → 17). Verified against php-8.5.8: pipe binds tighter than
        // `== < & ?? &&` and looser than `+ <<`.
        Some(match kind {
            T::QuestionQuestion => (2, BinaryOp::Coalesce),
            T::OrOr => (3, BinaryOp::Or),
            T::AndAnd => (4, BinaryOp::And),
            T::Bar => (5, BinaryOp::BitOr),
            T::Caret => (6, BinaryOp::BitXor),
            T::Amp => (7, BinaryOp::BitAnd),
            T::EqEq => (8, BinaryOp::Eq),
            T::NotEq => (8, BinaryOp::NotEq),
            // `<=>` sits in the EQUALITY tier, not the relational one — measured against the 8.5
            // oracle rather than recalled: `1 < 2 <=> 0` parses as `(1 < 2) <=> 0` (→ `int(1)`), so
            // `<` binds tighter. PHP additionally makes it non-associative (`1 <=> 2 <=> 3` is a
            // parse error); phorj's fold is uniformly left-associative, so it accepts that form and
            // the transpiler's `paren_if_compound` emits `(1 <=> 2) <=> 3` to keep the PHP leg valid.
            T::Spaceship => (8, BinaryOp::Spaceship),
            T::Lt => (9, BinaryOp::Lt),
            T::Gt => (9, BinaryOp::Gt),
            T::Le => (9, BinaryOp::Le),
            T::Ge => (9, BinaryOp::Ge),
            T::Pipe => (10, BinaryOp::Pipe),
            T::Shl => (11, BinaryOp::Shl),
            T::Plus => (12, BinaryOp::Add),
            T::Minus => (12, BinaryOp::Sub),
            T::Star => (13, BinaryOp::Mul),
            T::Slash => (13, BinaryOp::Div),
            T::Percent => (13, BinaryOp::Rem),
            // `**` power binds tighter than `* / %` and is **right-associative** (PHP-identical):
            // `2 ** 3 ** 2` is `2 ** (3 ** 2)`. Right-assoc is applied in `parse_binary`.
            T::StarStar => (14, BinaryOp::Pow),
            _ => return None,
        })
    }

    /// Precedence-climbing: parse a unary, then fold infix operators whose
    /// binding power is >= `min_bp`. All our binary operators are left-associative,
    /// so the right operand is parsed with `bp + 1`.
    pub(in crate::parser) fn parse_binary(&mut self, min_bp: u8) -> Result<Expr, Diagnostic> {
        let lhs = self.parse_unary()?;
        self.parse_binary_from(lhs, min_bp)
    }

    /// The fold half of [`Self::parse_binary`], entered with a pre-parsed left operand — used by the
    /// contextual-pipe-lambda RHS (DEC-239), which parses `(v => expr)` itself but must then climb
    /// exactly like any other operand so the pipe's RHS grammar stays uniform.
    pub(in crate::parser) fn parse_binary_from(
        &mut self,
        mut lhs: Expr,
        min_bp: u8,
    ) -> Result<Expr, Diagnostic> {
        loop {
            // `instanceof` is a type test at precedence 8 (like `==`), but its right operand is a
            // *type name*, not an expression — so it is parsed here rather than via `infix_op`. The
            // left operand and result type (`bool`) are validated by the checker (M-RT S1).
            if matches!(self.peek(), TokenKind::Instanceof) && 8 >= min_bp {
                let sp = self.peek_span();
                self.advance(); // consume `instanceof`
                let type_name = match self.peek().clone() {
                    TokenKind::Ident(n) => {
                        self.advance();
                        n
                    }
                    // `null` lexes as a keyword token, not an `Ident`; accept it as the discriminable
                    // primitive `null` (DEC-184 — `x instanceof null` ≡ `x is null` ≡ `is_null`).
                    TokenKind::Null => {
                        self.advance();
                        "null".to_string()
                    }
                    _ => return Err(self.error("a class name or primitive after `instanceof`")),
                };
                lhs = Expr::InstanceOf {
                    value: Box::new(lhs),
                    type_name,
                    span: sp,
                };
                continue;
            }
            // `value is TypeName` — the type test (DEC-184), a full synonym for `instanceof` that
            // also accepts a discriminable primitive (`x is int`). `is` is a *contextual* word (like
            // `as`) — it lexes as `Ident("is")`; in infix position after an expression it is the
            // type-test operator, so an identifier named `is` elsewhere is unaffected. Same
            // precedence (8) and type-name RHS as `instanceof`; both lower to `Expr::InstanceOf`, so
            // every downstream stage treats them identically. The checker validates the RHS
            // (primitive or class/interface) and types it `bool`.
            if matches!(self.peek(), TokenKind::Ident(s) if s == "is") && 8 >= min_bp {
                let sp = self.peek_span();
                self.advance(); // consume `is`
                let type_name = match self.peek().clone() {
                    TokenKind::Ident(n) => {
                        self.advance();
                        n
                    }
                    // `null` lexes as a keyword token, not an `Ident` — accept it as the `null`
                    // primitive test (`x is null` ⇒ `is_null`), and narrow the optional in the branch.
                    TokenKind::Null => {
                        self.advance();
                        "null".to_string()
                    }
                    _ => return Err(self.error("a type name after `is`")),
                };
                lhs = Expr::InstanceOf {
                    value: Box::new(lhs),
                    type_name,
                    span: sp,
                };
                continue;
            }
            // `value as TypeName` — the checked downcast (M4 casting axis 2), result `TypeName?`. `as`
            // is a *contextual* word (it also aliases imports), so it lexes as `Ident("as")`; here in
            // expression position it is the cast operator. Same precedence (8) and type-name RHS shape
            // as `instanceof` — so `a.b as T ?? d` is `((a.b) as T) ?? d` (tighter than `??`, looser
            // than member/call). The checker validates the RHS is a class/interface and types it `T?`.
            if !self.no_as_cast
                && matches!(self.peek(), TokenKind::Ident(s) if s == "as")
                && 8 >= min_bp
            {
                let sp = self.peek_span();
                self.advance(); // consume `as`
                let type_name = match self.peek().clone() {
                    TokenKind::Ident(n) => {
                        self.advance();
                        n
                    }
                    _ => return Err(self.error("a class or interface name after `as`")),
                };
                lhs = Expr::Cast {
                    value: Box::new(lhs),
                    type_name,
                    span: sp,
                };
                continue;
            }
            // Shift-right `>>` is two adjacent `Gt` tokens (never a single token — that protects
            // nested generics `List<List<int>>`). In expression position two consecutive `Gt` can
            // only be `>>`; a single `>` falls through to `infix_op` as comparison. Level 11.
            if matches!(self.peek(), TokenKind::Gt)
                && matches!(self.peek2(), TokenKind::Gt)
                && 11 >= min_bp
            {
                let sp = self.peek_span();
                self.advance(); // first `>`
                self.advance(); // second `>`
                let rhs = self.parse_binary(11 + 1)?;
                lhs = Expr::Binary {
                    op: BinaryOp::Shr,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span: sp,
                };
                continue;
            }
            let Some((bp, op)) = Self::infix_op(self.peek()) else {
                break;
            };
            if bp < min_bp {
                break;
            }
            let sp = self.peek_span();
            self.advance(); // consume the operator
                            // All binary operators are left-associative (`bp + 1`) except `**`, which is
                            // right-associative (`bp`): `2 ** 3 ** 2` parses as `2 ** (3 ** 2)`, PHP-identical.
            let right_bp = if matches!(op, BinaryOp::Pow) {
                bp
            } else {
                bp + 1
            };
            // DEC-239: a pipe RHS parses through `parse_pipe_rhs` (exprs/pipe.rs) — it enables the
            // `%` placeholder, recognizes the contextual pipe lambda `(v => …)`, and validates the
            // placeholder shape before returning.
            let rhs = if matches!(op, BinaryOp::Pipe) {
                self.parse_pipe_rhs(right_bp)?
            } else {
                self.parse_binary(right_bp)?
            };
            lhs = if matches!(op, BinaryOp::Pipe) {
                // `lhs |> rhs` — kept as a real `Expr::Pipe` node so the formatter round-trips
                // the surface syntax; `checker::lower_pipes` expands it to `rhs(lhs)` before the
                // checker and every backend (DEC-239). `BinaryOp::Pipe` is never placed in an
                // `Expr::Binary` node; the precedence-table entry at `infix_op` is kept to drive
                // the precedence-climbing loop.
                Expr::Pipe {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span: sp,
                }
            } else {
                Expr::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span: sp,
                }
            };
        }
        Ok(lhs)
    }

    /// Prefix unary operators: `-expr`, `!expr`. Right-associative by recursion.
    ///
    /// Every nesting vector — parens (`parse_primary` → `parse_expr`), unary chains (self-recursion
    /// here), and index/list/arg re-entry — routes through this function exactly once per level, so
    /// the depth guard here bounds all of them with a single counter. Past [`MAX_NEST_DEPTH`] it
    /// faults cleanly rather than overflowing the native stack. `depth` is balanced on both the `Ok`
    /// and `Err` paths (the result is captured before the decrement); the over-limit path aborts the
    /// whole parse, so leaving `depth` incremented there is harmless.
    pub(in crate::parser) fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        self.depth += 1;
        if self.depth > MAX_NEST_DEPTH {
            let sp = self.peek_span();
            return Err(Diagnostic::new(
                Stage::Parse,
                format!("expression nests too deeply (limit {MAX_NEST_DEPTH})"),
                sp.line,
                sp.col,
            ));
        }
        let sp = self.peek_span();
        // Return-type overload selector `<Type>f(args)` (M-RT Slice C1). A leading `<` cannot begin an
        // operand anywhere else (`<` is infix-only — less-than / generic args), so it is unambiguously a
        // selector here. Parse `< Type >` then the postfix call it applies to; the checker resolves which
        // return-overload it names and erases this wrapper (it is NOT a cast — see `Expr::OverloadSelect`).
        if matches!(self.peek(), TokenKind::Lt) {
            self.advance(); // '<'
            let ty = self.parse_type()?;
            self.expect(&TokenKind::Gt, "'>' to close an overload selector `<Type>`")?;
            let call = self.parse_postfix()?;
            self.depth -= 1;
            return Ok(Expr::OverloadSelect {
                ty,
                call: Box::new(call),
                span: sp,
            });
        }
        // `spawn <call>` (M6 W4): a contextual prefix keyword that starts a green task. It binds like a
        // unary prefix over the following postfix expression (the call), so `spawn a.b(x)` is
        // `spawn (a.b(x))`. The checker validates the operand is a call.
        if self.at_spawn() {
            self.advance(); // consume `spawn`
            let call = self.parse_postfix()?;
            self.depth -= 1;
            return Ok(Expr::Spawn {
                call: Box::new(call),
                span: sp,
            });
        }
        let op = match self.peek() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        };
        let result = if let Some(op) = op {
            self.advance();
            self.parse_unary().map(|expr| Expr::Unary {
                op,
                expr: Box::new(expr),
                span: sp,
            })
        } else {
            self.parse_postfix()
        };
        self.depth -= 1;
        result
    }

    /// Parse the explicit-turbofish tail of a DI composition root — the `<T>()` after a `inject` /
    /// `DependencyInjection.inject` head already consumed by the caller. `qualified` records whether the head was the
    /// `DI.`-qualified surface (`import Core.DependencyInjection;`) or bare (`import Core.DependencyInjection.inject;`); the gate is
    /// enforced later in [`crate::checker::desugar_di`]. `sp` spans the composition root.
    pub(in crate::parser) fn parse_inject_turbofish(
        &mut self,
        qualified: bool,
        sp: Span,
    ) -> Result<Expr, Diagnostic> {
        self.expect(&TokenKind::Lt, "'<' to open `inject<T>`")?;
        let t = self.parse_type()?;
        self.expect(&TokenKind::Gt, "'>' to close `inject<T>`")?;
        self.expect(&TokenKind::LParen, "'(' after `inject<T>`")?;
        self.expect(&TokenKind::RParen, "')' to close `inject<T>()`")?;
        Ok(Expr::Inject {
            ty: Some(t),
            qualified,
            span: sp,
        })
    }
}

//! Expression parsing — precedence climbing: ranges, binary/unary, postfix, calls.

use super::*;

/// The built-in collection type name → its `CollKind` for `new` construction (DEC-214:
/// `new List<T>()` / `new Map<K,V>()`). `Set` is intentionally excluded (deferred — the VM has no
/// empty-set construction op), so `new Set<…>()` stays an ordinary (and currently invalid) `new`.
fn collection_kind(name: &str) -> Option<crate::ast::CollKind> {
    use crate::ast::CollKind;
    match name {
        "List" => Some(CollKind::List),
        "Map" => Some(CollKind::Map),
        _ => None,
    }
}

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
        // Precedence follows PHP (higher binds tighter): `|>` `??` `||` `&&` then bitwise
        // `|` `^` `&`, then `==`/`!=`, comparison, shifts, `+ -`, `* / %`. Shift-right `>>` is not a
        // token (two `Gt`); it is handled at level 10 directly in `parse_binary`.
        Some(match kind {
            T::Pipe => (1, BinaryOp::Pipe),
            T::QuestionQuestion => (2, BinaryOp::Coalesce),
            T::OrOr => (3, BinaryOp::Or),
            T::AndAnd => (4, BinaryOp::And),
            T::Bar => (5, BinaryOp::BitOr),
            T::Caret => (6, BinaryOp::BitXor),
            T::Amp => (7, BinaryOp::BitAnd),
            T::EqEq => (8, BinaryOp::Eq),
            T::NotEq => (8, BinaryOp::NotEq),
            T::Lt => (9, BinaryOp::Lt),
            T::Gt => (9, BinaryOp::Gt),
            T::Le => (9, BinaryOp::Le),
            T::Ge => (9, BinaryOp::Ge),
            T::Shl => (10, BinaryOp::Shl),
            T::Plus => (11, BinaryOp::Add),
            T::Minus => (11, BinaryOp::Sub),
            T::Star => (12, BinaryOp::Mul),
            T::Slash => (12, BinaryOp::Div),
            T::Percent => (12, BinaryOp::Rem),
            // `**` power binds tighter than `* / %` and is **right-associative** (PHP-identical):
            // `2 ** 3 ** 2` is `2 ** (3 ** 2)`. Right-assoc is applied in `parse_binary`.
            T::StarStar => (13, BinaryOp::Pow),
            _ => return None,
        })
    }

    /// Precedence-climbing: parse a unary, then fold infix operators whose
    /// binding power is >= `min_bp`. All our binary operators are left-associative,
    /// so the right operand is parsed with `bp + 1`.
    pub(in crate::parser) fn parse_binary(&mut self, min_bp: u8) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_unary()?;
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
            // only be `>>`; a single `>` falls through to `infix_op` as comparison. Level 10.
            if matches!(self.peek(), TokenKind::Gt)
                && matches!(self.peek2(), TokenKind::Gt)
                && 10 >= min_bp
            {
                let sp = self.peek_span();
                self.advance(); // first `>`
                self.advance(); // second `>`
                let rhs = self.parse_binary(10 + 1)?;
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
            let rhs = self.parse_binary(right_bp)?;
            lhs = if matches!(op, BinaryOp::Pipe) {
                // `lhs |> rhs` is syntactic sugar for `rhs(lhs)` — lower to a Call in the
                // parser so all four backends see an ordinary function call. `BinaryOp::Pipe`
                // is never placed in an `Expr::Binary` node; the precedence-table entry at
                // `infix_op` is kept to drive the precedence-climbing loop.
                Expr::Call {
                    callee: Box::new(rhs),
                    args: vec![lhs],
                    type_args: Vec::new(),
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

    /// Parse a primary, then apply any chain of postfix operators.
    /// Parse the explicit-turbofish tail of a DI composition root — the `<T>()` after a `inject` /
    /// `DI.inject` head already consumed by the caller. `qualified` records whether the head was the
    /// `DI.`-qualified surface (`import Core.DI;`) or bare (`import Core.DI.inject;`); the gate is
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

    /// DEC-208 slice A: try to parse a turbofish `< TypeList >` that is **immediately followed by
    /// `(`**, at a call head. Returns `Some(types)` (cursor positioned on the `(`) on success, or
    /// `None` with the cursor **fully restored** on any failure — so the caller falls back to treating
    /// `<` as the comparison operator. The type list reuses [`Self::parse_type`], so a nested generic
    /// (`foo<List<int>>(x)`) consumes its own inner `>` and the outer `>` closes the turbofish; the
    /// `>>`-is-two-`Gt` tokenization (never a single shift token) is what makes that work. Backtracking
    /// is clean because the parser records errors only through `Result` (no side buffer) — restoring
    /// `self.pos` is sufficient.
    fn try_turbofish(&mut self) -> Option<Vec<Type>> {
        let start = self.pos;
        if !self.eat(&TokenKind::Lt) {
            return None;
        }
        let mut types = Vec::new();
        loop {
            match self.parse_type() {
                Ok(t) => types.push(t),
                Err(_) => {
                    self.pos = start;
                    return None;
                }
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        // Close the type list, then require an immediate `(` — the disambiguator that makes this a
        // call turbofish rather than a chain of comparisons.
        if !self.eat(&TokenKind::Gt) || !matches!(self.peek(), TokenKind::LParen) {
            self.pos = start;
            return None;
        }
        Some(types)
    }

    pub(in crate::parser) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        // Feature C: `new <Name>(<args>)` — the mandatory construction keyword. Parse exactly the
        // construction call (a primary callee + its argument list) and wrap it in `Expr::New`; the
        // postfix loop below then applies any `.`/`[]`/`!`/`?`/`with` to the constructed value (so
        // `new C().m()` is `(new C()).m()`). A bare `new` not followed by a call is a parse error.
        let mut e = if self.at_parent_call() {
            // M-RT super/parent: `parent.m(args)` / `parent(A).m(args)`. `parent` is contextual (a
            // call head only); the postfix loop below still applies to the result (so `parent.m().x`
            // chains). The resolved target is computed lexically by the checker/backends.
            self.parse_parent_call()?
        } else if matches!(self.peek(), TokenKind::New) {
            let sp = self.peek_span();
            self.advance();
            // DEC-214: `new List<T>()` / `new Map<K,V>()` — explicit empty-collection construction.
            // Recognized when `new` is followed by a built-in collection type name; the whole
            // `List<T>` is parsed via the generic type parser (so nested `<…>>` works) and the value
            // argument list must be empty. Any other head (`new Counter()`, `new Enum.Variant(…)`) is
            // ordinary construction, below.
            if matches!(self.peek(), TokenKind::Ident(n) if collection_kind(n).is_some()) {
                let kind = match self.peek() {
                    TokenKind::Ident(n) => collection_kind(n).expect("guarded above"),
                    _ => unreachable!(),
                };
                let ty = self.parse_type()?;
                let args = match ty {
                    Type::Named { args, .. } => args,
                    _ => Vec::new(),
                };
                self.expect(
                    &TokenKind::LParen,
                    "'(' — `new List<T>()` / `new Map<K,V>()` takes no value arguments",
                )?;
                self.expect(&TokenKind::RParen, "')' to close `new List<T>()`")?;
                Expr::NewColl {
                    kind,
                    args,
                    span: sp,
                }
            } else {
                let mut callee = self.parse_primary()?;
                // Qualified enum-variant construction `new Enum.Variant(args)` (injected-enum
                // qualification): consume a dotted-ident chain before the argument list so the callee is
                // a `Member` path the checker resolves to a specific enum's variant. `new Counter()` (no
                // dot) keeps the plain `Ident` callee.
                // DEC-207: also accept `::` here so `new Color::Red()` parses identically to
                // `new Color.Red()`, recording the surface separator on `Member.sep`.
                while matches!(self.peek(), TokenKind::Dot | TokenKind::ColonColon) {
                    let sep = if matches!(self.peek(), TokenKind::ColonColon) {
                        crate::ast::MemberSep::ColonColon
                    } else {
                        crate::ast::MemberSep::Dot
                    };
                    self.advance();
                    let nsp = self.peek_span();
                    let name = self.expect_ident(
                        "a variant name after `.`/`::` in a qualified constructor (`new Enum.Variant(…)`)",
                    )?;
                    callee = Expr::Member {
                        object: Box::new(callee),
                        name,
                        safe: false,
                        sep,
                        span: nsp,
                    };
                }
                self.expect(
                    &TokenKind::LParen,
                    "'(' — `new` must be followed by a constructor call, e.g. `new Counter()`",
                )?;
                let args = self.parse_arg_list()?;
                self.expect(&TokenKind::RParen, "')' to close arguments")?;
                let call = Expr::Call {
                    callee: Box::new(callee),
                    args,
                    type_args: Vec::new(),
                    span: sp,
                };
                Expr::New(Box::new(call), sp)
            }
        } else {
            self.parse_primary()?
        };
        // DEC-208 slice A: turbofish type arguments parsed just before a call's `(` — `foo<A, B>(x)`,
        // `obj.method<T>(args)`. The try-parse below leaves them here; the next `(` iteration attaches
        // them to the `Expr::Call` it builds. Empty in the common (inferred) form.
        let mut pending_type_args: Vec<Type> = Vec::new();
        loop {
            let sp = self.peek_span();
            match self.peek() {
                // DEC-208 slice A: a `<` after a call head (`name` / `obj.method`) is a turbofish only
                // if it parses as `< TypeList >` immediately followed by `(`. Otherwise BACKTRACK
                // cleanly (restore the cursor) and fall through to break, so `<` is the comparison
                // operator `parse_binary` handles. The only program this could shadow is
                // `(head < T) > (args)` — comparing a method/function reference against a type name —
                // which is always type-invalid, so no valid program changes meaning.
                TokenKind::Lt => match self.try_turbofish() {
                    Some(targs) => {
                        pending_type_args = targs;
                        continue;
                    }
                    None => break,
                },
                TokenKind::Dot | TokenKind::QuestionDot | TokenKind::ColonColon => {
                    let safe = matches!(self.peek(), TokenKind::QuestionDot);
                    // DEC-207: `::` is the class/type-level access separator, recorded on the
                    // resulting `Member.sep`; `.`/`?.` record `Dot`. Purely syntactic here (no
                    // enforcement) — both parse to the same `Member` shape. `::` is never nullsafe.
                    let sep = if matches!(self.peek(), TokenKind::ColonColon) {
                        crate::ast::MemberSep::ColonColon
                    } else {
                        crate::ast::MemberSep::Dot
                    };
                    self.advance();
                    let name = match self.peek().clone() {
                        TokenKind::Ident(n) => {
                            self.advance();
                            n
                        }
                        _ => {
                            return Err(self.error("a field or method name after '.', '?.' or '::'"))
                        }
                    };
                    // DI composition root, qualified turbofish surface `DI.inject<T>()` (§7). Recognized
                    // only in this exact shape (`DI` head, `.inject`, `<`); any other `.inject` stays an
                    // ordinary member access, and `DI.inject()` (no turbofish) is converted by
                    // `desugar_di` when `Core.DI` is imported. `?.` is never a composition root.
                    if !safe
                        && sep == crate::ast::MemberSep::Dot
                        && name == "inject"
                        && matches!(&e, Expr::Ident(q, _) if q == "DI")
                        && matches!(self.peek(), TokenKind::Lt)
                    {
                        e = self.parse_inject_turbofish(true, sp)?;
                        continue;
                    }
                    e = Expr::Member {
                        object: Box::new(e),
                        name,
                        safe,
                        sep,
                        span: sp,
                    };
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&TokenKind::RParen, "')' to close arguments")?;
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        // Attach any turbofish parsed by the `Lt` arm immediately above; `try_turbofish`
                        // guarantees a `(` follows, so this is the very next iteration and nothing else
                        // can consume `pending_type_args`.
                        type_args: std::mem::take(&mut pending_type_args),
                        span: sp,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&TokenKind::RBracket, "']' to close index")?;
                    e = Expr::Index {
                        object: Box::new(e),
                        index: Box::new(index),
                        span: sp,
                    };
                }
                // Postfix `!` is the force-unwrap (M3 S2.5). It can only appear here, after a
                // primary/postfix expr; prefix `!x` (logical not) is handled in `parse_unary`, and
                // `!=` lexes as a single `NotEq`, so there is no ambiguity.
                TokenKind::Bang => {
                    self.advance();
                    e = Expr::Force {
                        inner: Box::new(e),
                        span: sp,
                    };
                }
                // Postfix `?` is error propagation (M-faults Slice 2a). The tokenizer munches `??`/`?.`
                // into `QuestionQuestion`/`QuestionDot`, so a lone `Question` here is unambiguous.
                TokenKind::Question => {
                    self.advance();
                    e = Expr::Propagate {
                        inner: Box::new(e),
                        span: sp,
                    };
                }
                // `obj with { f = e, … }` — functional update (M-mut.4a). Postfix, so it binds to the
                // immediately-preceding expression; the brace block is unambiguous in expr position.
                TokenKind::With => {
                    self.advance();
                    self.expect(&TokenKind::LBrace, "'{' after 'with'")?;
                    let mut fields = Vec::new();
                    while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
                        let name = self.expect_ident("a field name in `with { … }`")?;
                        self.expect(&TokenKind::Eq, "'=' after a `with` field name")?;
                        let value = self.parse_expr()?;
                        fields.push((name, value));
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace, "'}' to close `with { … }`")?;
                    e = Expr::CloneWith {
                        object: Box::new(e),
                        fields,
                        span: sp,
                    };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    /// `parent.m(args)` / `parent(A).m(args)` — a super/parent dispatch call (M-RT super/parent). The
    /// `at_parent_call` gate has confirmed the head. `A` (a bare ancestor class name) selects the
    /// qualified form; the method may be an ordinary name or the `constructor` keyword (parent ctor).
    pub(in crate::parser) fn parse_parent_call(&mut self) -> Result<Expr, Diagnostic> {
        let sp = self.peek_span();
        self.advance(); // `parent`
        let ancestor = if self.eat(&TokenKind::LParen) {
            let a = self.expect_ident("an ancestor class name in `parent(A)`")?;
            self.expect(&TokenKind::RParen, "')' after the ancestor in `parent(A)`")?;
            Some(a)
        } else {
            None
        };
        // DEC-207: accept `::` as an alternative to `.` after `parent` (`parent::m(…)`).
        if !(self.eat(&TokenKind::Dot) || self.eat(&TokenKind::ColonColon)) {
            return Err(self.error("'.' or '::' after `parent` in a super call"));
        }
        // The method is an ordinary name, or the `constructor` keyword (a parent-constructor call).
        let method = if matches!(self.peek(), TokenKind::Constructor) {
            self.advance();
            "constructor".to_string()
        } else {
            self.expect_ident("a method name after `parent.`")?
        };
        self.expect(&TokenKind::LParen, "'(' to open the super-call arguments")?;
        let args = self.parse_arg_list()?;
        self.expect(&TokenKind::RParen, "')' to close arguments")?;
        Ok(Expr::ParentCall {
            ancestor,
            method,
            args,
            span: sp,
        })
    }

    /// Comma-separated expressions until the closing delimiter (caller consumes the closer).
    /// Allows zero args; allows a trailing comma.
    pub(in crate::parser) fn parse_arg_list(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
                break; // trailing comma
            }
        }
        Ok(args)
    }
}

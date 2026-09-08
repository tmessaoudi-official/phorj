//! `impl Parser` — the POSTFIX layer: everything that parses what follows a primary.
//!
//! Split out of `climb.rs` under Invariant 13 (M-Decomp) when the `<=>` build (`eba09d4e`) pushed
//! that file past its size-baseline. The boundary is a real one rather than a line count: `climb.rs`
//! keeps precedence climbing (ranges, binding powers, binary/unary), while the postfix CHAIN —
//! `.`/`[]`/`!`/`?`/`with`, `new`, call turbofish, parent dispatch and argument lists — lives here.
//! `collection_kind` and `try_turbofish` move with it: both are private helpers used only by this
//! cluster.

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

    /// Parse a primary, then apply any chain of postfix operators.
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
                    // DI composition root, qualified turbofish surface `DependencyInjection.inject<T>()` (§7). Recognized
                    // only in this exact shape (`DependencyInjection` head, `.inject`, `<`); any other `.inject` stays an
                    // ordinary member access, and `DependencyInjection.inject()` (no turbofish) is converted by
                    // `desugar_di` when `Core.DependencyInjection` is imported. `?.` is never a composition root.
                    if !safe
                        && sep == crate::ast::MemberSep::Dot
                        && name == "inject"
                        && matches!(&e, Expr::Ident(q, _) if q == "DependencyInjection")
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
            // DEC-297: a NAMED argument `name: value` — an Ident immediately followed by `:`. This is
            // unambiguous at arg-start (`:` is not a binary operator there; map literals use `=>`, and
            // a ternary's `:` never follows the bare arg-head ident directly). The checker normalizes
            // named args into positional slots before any backend.
            if matches!(self.peek(), TokenKind::Ident(_))
                && matches!(self.peek2(), TokenKind::Colon)
            {
                let span = self.peek_span();
                let name = self.expect_ident("a named-argument name")?;
                self.expect(&TokenKind::Colon, "':' after the named-argument name")?;
                let value = Box::new(self.parse_expr()?);
                args.push(Expr::NamedArg { name, value, span });
            } else {
                args.push(self.parse_expr()?);
            }
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

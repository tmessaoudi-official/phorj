//! Expression parsing — primaries: literals, interpolation, match, if-expr.

use super::*;

impl Parser {
    /// Lowest-level expression: a literal, identifier, `this`, string, list, match, or `( expr )`.
    pub(in crate::parser) fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let sp = self.peek_span();
        match self.peek().clone() {
            TokenKind::Int(n) => {
                self.advance();
                Ok(Expr::Int(n, sp))
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::Float(f, sp))
            }
            TokenKind::Decimal(unscaled, scale) => {
                self.advance();
                Ok(Expr::Decimal {
                    unscaled,
                    scale,
                    span: sp,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true, sp))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false, sp))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Null(sp))
            }
            TokenKind::This => {
                self.advance();
                Ok(Expr::This(sp))
            }
            TokenKind::Ident(name) => {
                self.advance();
                // DI composition root (§7 import discipline): `inject` is NOT a keyword — a `Core.DI`
                // member freed as an ordinary identifier. Only the explicit turbofish surface
                // `inject<T>()` is recognized here (otherwise unparseable — Phorj has no general
                // turbofish). The no-turbofish `inject()` stays a plain call, converted to the
                // composition root by `desugar_di` ONLY when `Core.DI.inject` is member-imported.
                if name == "inject" && matches!(self.peek(), TokenKind::Lt) {
                    return self.parse_inject_turbofish(false, sp);
                }
                Ok(Expr::Ident(name, sp))
            }
            TokenKind::Str(segs) => {
                self.advance();
                let parts = self.segments_to_parts(segs, sp)?;
                Ok(Expr::Str(parts, sp))
            }
            TokenKind::Bytes(b) => {
                self.advance();
                Ok(Expr::Bytes(b, sp))
            }
            TokenKind::TaggedTemplate(tag, body) => {
                self.advance();
                // Reuse the exact `{expr}` splitter as plain strings; the type-directed desugar
                // into `html.concat([…])` kernel calls happens in the checker (which has types).
                let parts = self.split_interpolation(&body, sp)?;
                // `html"…"` keeps its dedicated node (unchanged path through every backend); any other
                // tag becomes the general `TaggedTemplate` node, which the checker rejects with
                // `E-UNKNOWN-TAG` (the scaffold hook for the human's two-mode desugar).
                if tag == "html" {
                    Ok(Expr::Html(parts, sp))
                } else {
                    Ok(Expr::TaggedTemplate {
                        tag,
                        parts,
                        span: sp,
                    })
                }
            }
            TokenKind::Match => self.parse_match(sp),
            TokenKind::If => self.parse_if_expr(sp),
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "')'")?;
                Ok(inner)
            }
            TokenKind::LBracket => {
                self.advance();
                // `[]` is the empty *list* (an empty map literal is deferred — it needs a builder).
                if self.check(&TokenKind::RBracket) {
                    self.advance();
                    Ok(Expr::List(Vec::new(), sp))
                } else {
                    // Parse the first element, then disambiguate: a following `=>` makes this a map
                    // literal (`[k => v, …]`); otherwise it's a list (`[a, b, …]`). A lambda element
                    // (`function(x) => x`) consumes its own `=>` inside `parse_expr`, so it never trips the
                    // map peek. Once chosen, a mismatched separator errors cleanly at `expect`.
                    let first = self.parse_expr()?;
                    if self.eat(&TokenKind::FatArrow) {
                        let val = self.parse_expr()?;
                        let mut pairs = vec![(first, val)];
                        while self.eat(&TokenKind::Comma) {
                            if self.check(&TokenKind::RBracket) {
                                break; // trailing comma
                            }
                            let k = self.parse_expr()?;
                            self.expect(&TokenKind::FatArrow, "'=>' in map literal")?;
                            let v = self.parse_expr()?;
                            pairs.push((k, v));
                        }
                        self.expect(&TokenKind::RBracket, "']' to close map literal")?;
                        Ok(Expr::Map(pairs, sp))
                    } else {
                        let mut items = vec![first];
                        while self.eat(&TokenKind::Comma) {
                            if self.check(&TokenKind::RBracket) {
                                break; // trailing comma
                            }
                            items.push(self.parse_expr()?);
                        }
                        self.expect(&TokenKind::RBracket, "']' to close list literal")?;
                        Ok(Expr::List(items, sp))
                    }
                }
            }
            // Lambda expression: `function(int x, int y) -> int => x + y` (expression body only;
            // statement-body lambdas land in S3 Task 6).
            TokenKind::Function => {
                self.advance(); // consume 'fn'
                self.expect(&TokenKind::LParen, "'(' after 'function'")?;
                let params = self.parse_params()?;
                self.expect(&TokenKind::RParen, "')' to close lambda parameters")?;
                // Optional return-type annotation before the `=>`/`{` body. A-1: `:` is canonical
                // (`function(int x): string => …`); `->` stays as a silent transition alias.
                let ret = if self.eat(&TokenKind::Colon) || self.eat(&TokenKind::Arrow) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = if self.eat(&TokenKind::FatArrow) {
                    LambdaBody::Expr(Box::new(self.parse_expr()?))
                } else if self.check(&TokenKind::LBrace) {
                    LambdaBody::Block(self.parse_block()?)
                } else {
                    return Err(self.error("'=>' (expression body) or '{' (statement body)"));
                };
                Ok(Expr::Lambda {
                    params,
                    ret,
                    body,
                    span: sp,
                })
            }
            _ => Err(self.error("an expression")),
        }
    }

    /// Turn the tokenizer's pre-split string segments into `StrPart`s: a `Lit` becomes a literal run
    /// (escapes, incl. `\{`, already expanded by the tokenizer); an `Interp` carries raw expression
    /// source that is re-lexed + parsed here. An all-empty input yields a single empty literal. This
    /// is the `Str` path; `html"…"` still uses [`Self::split_interpolation`] on its flat body.
    pub(in crate::parser) fn segments_to_parts(
        &self,
        segs: Vec<crate::token::StrSeg>,
        sp: Span,
    ) -> Result<Vec<StrPart>, Diagnostic> {
        use crate::token::StrSeg;
        let mut parts = Vec::new();
        for seg in segs {
            match seg {
                StrSeg::Lit(s) => parts.push(StrPart::Literal(s)),
                StrSeg::Interp(src, base) => {
                    let mut sub_tokens = crate::tokenizer::lex(&src).map_err(|e| {
                        Diagnostic::new(
                            Stage::Parse,
                            format!("in interpolation: {}", e.message),
                            sp.line,
                            sp.col,
                        )
                    })?;
                    // The sub-tokenizer restarts spans at 0; shift every token's `start` to its absolute
                    // position in the original source so interpolated expressions carry globally-unique
                    // offsets (a span-keyed rewrite like UFCS keys on `start`). `line`/`col` keep the
                    // sub-tokenizer's values, so interpolation diagnostics are unchanged.
                    for t in &mut sub_tokens {
                        t.span.start += base;
                    }
                    let mut sub = Parser::new(sub_tokens);
                    let e = sub.parse_expr()?;
                    sub.expect(&TokenKind::Eof, "end of interpolation expression")?;
                    parts.push(StrPart::Expr(Box::new(e)));
                }
            }
        }
        if parts.is_empty() {
            parts.push(StrPart::Literal(String::new()));
        }
        Ok(parts)
    }

    /// Split a string body into literal runs and `{expr}` interpolations.
    /// Each interpolation is re-lexed + re-parsed as a standalone expression.
    /// Used by `html"…"` (whose body is still a flat string).
    pub(in crate::parser) fn split_interpolation(
        &self,
        body: &str,
        sp: Span,
    ) -> Result<Vec<StrPart>, Diagnostic> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = body.chars();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if !literal.is_empty() {
                        parts.push(StrPart::Literal(std::mem::take(&mut literal)));
                    }
                    // collect until the matching '}'
                    let mut inner = String::new();
                    let mut closed = false;
                    for ic in chars.by_ref() {
                        if ic == '}' {
                            closed = true;
                            break;
                        }
                        inner.push(ic);
                    }
                    if !closed {
                        return Err(Diagnostic::new(
                            Stage::Parse,
                            "unterminated interpolation '{' in string",
                            sp.line,
                            sp.col,
                        ));
                    }
                    let sub_tokens = crate::tokenizer::lex(&inner).map_err(|e| {
                        Diagnostic::new(
                            Stage::Parse,
                            format!("in interpolation: {}", e.message),
                            sp.line,
                            sp.col,
                        )
                    })?;
                    let mut sub = Parser::new(sub_tokens);
                    let e = sub.parse_expr()?;
                    sub.expect(&TokenKind::Eof, "end of interpolation expression")?;
                    parts.push(StrPart::Expr(Box::new(e)));
                }
                '}' => {
                    return Err(Diagnostic::new(
                        Stage::Parse,
                        "unexpected '}' in string (no matching '{')",
                        sp.line,
                        sp.col,
                    ));
                }
                _ => literal.push(c),
            }
        }
        if !literal.is_empty() {
            parts.push(StrPart::Literal(literal));
        }
        // an empty string is a single empty literal part
        if parts.is_empty() {
            parts.push(StrPart::Literal(String::new()));
        }
        Ok(parts)
    }

    /// `match EXPR { PAT => EXPR, ... }` — assumes the current token is `match`.
    pub(in crate::parser) fn parse_match(&mut self, sp: Span) -> Result<Expr, Diagnostic> {
        self.expect(&TokenKind::Match, "'match'")?;
        let scrutinee = self.parse_expr()?;
        self.expect(&TokenKind::LBrace, "'{' to open match arms")?;
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) {
            let arm_sp = self.peek_span();
            // Or-pattern `p1 | p2 | … => body` (Phase 1 operators slice): collect `|`-separated
            // alternatives. A single pattern (no `|`) is the common case and behaves exactly as
            // before. `|` is unambiguous here — a pattern is followed only by `|`, `when`, or `=>`.
            let mut alts = vec![self.parse_arm_pattern()?];
            while self.eat(&TokenKind::Bar) {
                alts.push(self.parse_pattern()?);
            }
            // Optional arm guard: a contextual `when <cond>` between the pattern and `=>`.
            // `when` is recognized only here (and in if/while-let) — a normal identifier elsewhere.
            let guard = if matches!(self.peek(), TokenKind::Ident(k) if k.as_str() == "when") {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "'=>' after match pattern")?;
            let body = self.parse_expr()?;
            if alts.len() == 1 {
                arms.push(MatchArm {
                    pattern: alts.pop().expect("one alternative"),
                    guard,
                    body,
                    span: arm_sp,
                });
            } else {
                // Desugar the or-pattern to one arm per alternative, each sharing the (cloned)
                // body + guard. Every backend then sees ordinary arms — zero backend change, and
                // exhaustiveness / duplicate-arm (`W-MATCH-UNREACHABLE`) / flow-narrowing all work
                // unchanged. Alternatives must be binding-free: a bare binding, `_`, or any
                // variable-binding sub-pattern is rejected, since the shared body cannot depend on
                // which alternative matched (`Some(_) | None()` is fine; `Some(n) | None()` is not).
                for pat in &alts {
                    if Self::or_alt_invalid(pat) {
                        return Err(Diagnostic::new(
                            Stage::Parse,
                            "an or-pattern `a | b` alternative must be a concrete pattern with no bindings (no `_`, no bare name, no variable-binding sub-pattern)",
                            arm_sp.line,
                            arm_sp.col,
                        )
                        .with_code("E-OR-PATTERN-BIND")
                        .with_hint("use literals/variants without binders, or split into separate arms if you need to bind"));
                    }
                }
                for pat in alts {
                    arms.push(MatchArm {
                        pattern: pat,
                        guard: guard.clone(),
                        body: body.clone(),
                        span: arm_sp,
                    });
                }
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "'}' to close match")?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span: sp,
        })
    }

    /// `if (cond) { e } else { e }` in **expression** position — parens and a single-expression
    /// body per arm, with a mandatory `else` (the value must come from somewhere). Reached only via
    /// `parse_primary`; a top-level `if` statement is matched first by `parse_stmt`, so the two
    /// never collide. Mirrors statement-`if`'s `if (cond)` shape for intra-language consistency.
    pub(in crate::parser) fn parse_if_expr(&mut self, sp: Span) -> Result<Expr, Diagnostic> {
        self.expect(&TokenKind::If, "'if'")?;
        self.expect(&TokenKind::LParen, "'(' after 'if'")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')' after if condition")?;
        self.expect(&TokenKind::LBrace, "'{' to open the then-branch")?;
        let then_expr = self.parse_expr()?;
        self.expect(&TokenKind::RBrace, "'}' to close the then-branch")?;
        self.expect(
            &TokenKind::Else,
            "'else' (an expression `if` must have an else branch)",
        )?;
        self.expect(&TokenKind::LBrace, "'{' to open the else-branch")?;
        let else_expr = self.parse_expr()?;
        self.expect(&TokenKind::RBrace, "'}' to close the else-branch")?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
            span: sp,
        })
    }
}

//! DEC-504 — parsing the field labels of a named tuple, for BOTH the value form
//! `(bp: 3, source: "x")` and the type form `(bp: int, source: string)`.
//!
//! **The lookahead is `Ident` then `:`, and it is unambiguous** — nothing valid before this slice
//! could start that way. In expression position the `(` arm parses a sub-expression and then
//! demands `,` or `)`, so `(bp: …` was a parse error; in type position the paren list parses types
//! separated by `,`, so it was an error there too. A ternary starts its colon after a `?`, and the
//! named arguments of a call (`f(a: 1)`, DEC-297) and of an attribute (`#[Entry(kind: …)]`) are
//! parsed by their own argument-list parsers, never by these two paren arms.
//!
//! `::` is a DISTINCT token (`ColonColon`, DEC-207), so a qualified `(Enum::Variant, x)` does not
//! trip the lookahead.
//!
//! Both parsers demand ALL-OR-NOTHING labelling: the first field decides, and a tuple that mixes
//! the two forms is refused by name rather than silently treated as partially named. A mixed tuple
//! has no defensible reading — `(a: 1, 2)` would have to invent a name for position 1 or make the
//! type half-structural — and DEC-166's no-invention rule settles it.

use super::Parser;
use crate::ast::{Expr, Label, Type};
use crate::diagnostic::Diagnostic;
use crate::token::{Span, TokenKind};

impl Parser {
    /// Does a labelled field start here? `Ident` followed by a single `:`.
    pub(super) fn at_tuple_label(&self) -> bool {
        matches!(self.peek(), TokenKind::Ident(_)) && matches!(self.peek2(), TokenKind::Colon)
    }

    /// Consume `name:` and return the label. Only call when [`Self::at_tuple_label`] is true.
    pub(super) fn eat_tuple_label(&mut self) -> Result<Label, Diagnostic> {
        let span = self.peek_span();
        let name = self.expect_ident("a tuple field name")?;
        self.expect(&TokenKind::Colon, "':' after a tuple field name")?;
        Ok(Label { name, span })
    }

    /// Enforce all-or-nothing labelling for one field, given whether the FIRST field was labelled.
    ///
    /// `labelled` says whether the FIRST field carried a name. Returns this field's label, or an
    /// error naming the mismatch — which is what makes `(a: 1, 2)` and `(1, b: 2)` both refused,
    /// each pointing at the field that broke the rule, rather than one of them silently parsing.
    pub(super) fn tuple_label_for_field(
        &mut self,
        labelled: bool,
    ) -> Result<Option<Label>, Diagnostic> {
        match (self.at_tuple_label(), labelled) {
            (true, true) => Ok(Some(self.eat_tuple_label()?)),
            (false, false) => Ok(None),
            (false, true) => Err(self.error(
                "a `name:` label on this field — a tuple names either ALL of its fields or none of them",
            )),
            (true, false) => Err(self.error(
                "no label on this field — the first field is unlabelled, so a tuple names either ALL of its fields or none",
            )),
        }
    }

    /// `(name: e[, name: e …])` — the named-tuple VALUE form; the `(` is consumed, the lookahead
    /// has already matched. Unlike the positional form there is a legal 1-field tuple here.
    pub(super) fn parse_named_tuple(&mut self, sp: Span) -> Result<Expr, Diagnostic> {
        let mut elems = Vec::new();
        let mut labels = Vec::new();
        loop {
            labels.push(match self.tuple_label_for_field(true)? {
                Some(l) => l,
                None => unreachable!("tuple_label_for_field(true) returns Some or errors"),
            });
            elems.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
                break; // trailing comma
            }
        }
        self.expect(&TokenKind::RParen, "')' to close a named tuple")?;
        Ok(Expr::Tuple(elems, Some(Box::new(labels)), sp))
    }

    /// `(name: T[, name: T …])` — the named-tuple TYPE form; the `(` is consumed.
    pub(super) fn parse_named_tuple_type(&mut self, sp: Span) -> Result<Type, Diagnostic> {
        let mut members = Vec::new();
        let mut labels = Vec::new();
        loop {
            labels.push(match self.tuple_label_for_field(true)? {
                Some(l) => l,
                None => unreachable!("tuple_label_for_field(true) returns Some or errors"),
            });
            members.push(self.parse_type()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "')' to close a named tuple type")?;
        Ok(Type::Tuple(members, Some(Box::new(labels)), sp))
    }
}

//! Ordering and three-way comparison — `< > <= >=` and `<=>` (DEC-505, as amended by DEC-512).
//!
//! Split out of `operators.rs` (M-Decomp, Invariant 13) when `<=>` landed: that file was already at
//! the 500-line hard cap, and this cluster is genuinely cohesive — the relational operators and
//! `<=>` must admit *exactly* the same operand types or a program can order a pair it cannot spell
//! a `<=>` for. Keeping the two arms adjacent, sharing one leaf predicate and one refusal emitter,
//! is what makes that impossible rather than merely intended.

use super::*;

/// The first element type that makes `t` unorderable, or `None` when `t` is orderable. Returns the
/// offending *leaf* so a diagnostic about `((int, decimal), int)` can name `decimal` instead of
/// reprinting the whole shape.
///
/// The admitted set is `int`, `float`, and tuples of those, recursively. It is deliberately NARROWER
/// than the scalar rule `< > <= >=` has always used, which also admits `decimal`: a decimal's PHP
/// carrier is a numeric *string* and PHP compares numeric strings as floats, so the exact i128
/// `compare_ord` and the PHP leg part company past 2^53 — `9007199254740992.00 < 9007199254740993.00`
/// is `true` natively and `false` in PHP (measured on php-8.5.9). Bare `d1 < d2` keeps that
/// pre-existing hole, which is recorded in `KNOWN_ISSUES.md` and gated on question Q-0908-3; the new
/// surfaces must not inherit it. Closing it means a `__phorj_dec_cmp` helper, which is an
/// Invariant-16 byte-identity trade and therefore the developer's to rule, not this slice's.
pub(in crate::checker) fn unorderable_leaf(t: &Ty) -> Option<Ty> {
    match t {
        Ty::Int | Ty::Float => None,
        Ty::Tuple(elems) => elems.iter().find_map(unorderable_leaf),
        other => Some(other.clone()),
    }
}

impl Checker {
    /// The DEC-512 refusal, shared by `<=>` and by tuple `< > <= >=` so the two cannot drift. Each
    /// arm names the *ruling* rather than reporting a generic type mismatch — a bare "requires int
    /// or float" would send a reader looking for a missing cast when the real answer is that the
    /// operation is deliberately not offered.
    pub(in crate::checker) fn order_refusal(&mut self, span: Span, bad: &Ty, op: &str) -> Ty {
        match bad {
            Ty::List(_) => self.err_coded(
                span,
                format!("`{op}` is not available on `{bad}` — a list is not orderable"),
                "E-ORDER-LIST",
                Some(
                    "DEC-512: ordering is a TUPLE capability. A tuple's arity is static, so \
                     lexicographic order and PHP's count-first array rule provably coincide; a \
                     list's arity is not, and the two disagree — `[2] <=> [1, 1]` is `-1` in PHP \
                     and `+1` lexicographically. Use a tuple, or compare element by element."
                        .into(),
                ),
            ),
            Ty::Decimal => self.err_coded(
                span,
                format!("`{op}` is not available on `decimal`"),
                "E-ORDER-DECIMAL",
                Some(
                    "A decimal's PHP carrier is a numeric string, which PHP compares as a float, \
                     so the native and PHP legs diverge past 2^53. Pending question Q-0908-3 — a \
                     `__phorj_dec_cmp` helper would close it, for bare `<` too."
                        .into(),
                ),
            ),
            _ => self.err_coded(
                span,
                format!("`{op}` requires `int`, `float`, or a tuple of those, found `{bad}`"),
                "E-ORDER-OPERANDS",
                None,
            ),
        }
    }

    /// `< > <= >=`. Scalars keep their long-standing rule (int⊕int, float⊕float, and the decimal
    /// widen); tuples are new in DEC-512 and compare lexicographically.
    pub(in crate::checker) fn check_relational(
        &mut self,
        l: &Ty,
        r: &Ty,
        op: &str,
        span: Span,
    ) -> Ty {
        // `decimal` compares against `decimal` or `int` (numeric, scale-insensitive); same operand
        // rule as decimal arithmetic, a `float` mix is `E-DECIMAL-FLOAT-MIX`.
        let dec_ok = (*l == Ty::Decimal && (*r == Ty::Decimal || *r == Ty::Int))
            || (*r == Ty::Decimal && *l == Ty::Int);
        if (*l == Ty::Int && *r == Ty::Int) || (*l == Ty::Float && *r == Ty::Float) || dec_ok {
            return Ty::Bool;
        }
        // Tuple ordering (DEC-512): identical shape on both sides, orderable leaves. Requiring
        // `l == r` rather than element-wise assignability keeps `(int, int) < (int, float)` an
        // error — the same no-coercion stance the scalar rule takes for `1 < 2.0`.
        if matches!(l, Ty::Tuple(_)) || matches!(r, Ty::Tuple(_)) {
            if l != r {
                return self.err_coded(
                    span,
                    format!(
                        "`{op}` requires the same tuple shape on both sides, found `{l}` and `{r}`"
                    ),
                    "E-ORDER-TUPLE-SHAPE",
                    None,
                );
            }
            if let Some(bad) = unorderable_leaf(l) {
                return self.order_refusal(span, &bad, op);
            }
            return Ty::Bool;
        }
        // A list reaches the ruling-aware refusal rather than the generic numeric message.
        if matches!(l, Ty::List(_)) || matches!(r, Ty::List(_)) {
            let bad = if matches!(l, Ty::List(_)) { l } else { r };
            return self.order_refusal(span, &bad.clone(), op);
        }
        if *l == Ty::Decimal || *r == Ty::Decimal {
            return self.err_coded(
                span,
                format!(
                    "cannot compare `decimal` with `{}`",
                    if *l == Ty::Decimal { r } else { l }
                ),
                "E-DECIMAL-FLOAT-MIX",
                Some("compare a `decimal` only with another `decimal` or an `int`".into()),
            );
        }
        self.err(
            span,
            format!("comparison requires matching int or float operands, found `{l}` and `{r}`"),
        );
        Ty::Bool
    }

    /// `<=>`. Yields an `int` (`-1`/`0`/`1`) rather than a `bool`, which is what makes it a legal
    /// arithmetic operand and so obliges the compiler's `CTy` resolver to type it (Invariant 7).
    ///
    /// Its operand set is the *narrow* one above — no `decimal` — so a `<=>` never introduces a leg
    /// divergence, and it always recovers as `Ty::Int` so a refusal reports one error rather than
    /// cascading through every enclosing expression.
    pub(in crate::checker) fn check_spaceship(&mut self, l: &Ty, r: &Ty, span: Span) -> Ty {
        if l != r {
            self.err_coded(
                span,
                format!("`<=>` requires matching operands, found `{l}` and `{r}`"),
                "E-SPACESHIP-OPERANDS",
                Some("compare two `int`s, two `float`s, or two tuples of the same shape".into()),
            );
        } else if let Some(bad) = unorderable_leaf(l) {
            self.order_refusal(span, &bad, "<=>");
        }
        Ty::Int
    }
}

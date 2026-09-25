//! Primitive narrowing on the VM (DEC-184, widened by DEC-535) — the compiler's half of the
//! checker's `narrow_from_condition` (`checker/stmt/narrow.rs`).
//!
//! The checker narrows `x: int | string` to `int` wherever a condition proves `x is int`; the VM must
//! give that local its concrete operand `CTy` at the same places, or `x + 1` is checker-accepted and
//! VM-rejected (`compile error: x is not numeric` — the CTy-operand trap, Invariant 7). Only POSITIVE
//! primitive facts are mirrored: an optional already carries its inner `CTy`, a class resolves members
//! through the field table, and neither side narrows a union complement (W2-12).
//!
//! The places, each paired with the checker site that narrows it: a statement `if`'s then- AND
//! else-block (`compile_if`; the else-block was missing until DEC-535, a check-clean program the VM
//! refused), the right operand of `&&` (true) and `||` (false), and each if-expression arm — plus the
//! if-expression's own compile-time type in `ctype`, through [`Compiler::ctype_narrowed`].

use super::*;

impl Compiler<'_> {
    /// The primitive narrowings `cond` implies when it evaluates to `polarity`, as `(slot, CTy)`: the
    /// checker's recursion — `!` flips, `&&` narrows its true side, `||` its false side (De Morgan).
    pub(in crate::compiler) fn prim_narrowings(
        &self,
        cond: &Expr,
        polarity: bool,
    ) -> Vec<(usize, CTy)> {
        use crate::ast::{BinaryOp, UnaryOp};
        let mut out = Vec::new();
        match cond {
            Expr::InstanceOf {
                value, type_name, ..
            } if polarity => {
                if let Expr::Ident(name, _) = &**value {
                    let cty = cty_of_type_name(type_name);
                    if matches!(cty, CTy::Int | CTy::Float | CTy::Str | CTy::Decimal) {
                        if let Some(slot) = self.resolve_local(name) {
                            out.push((slot, cty));
                        }
                    }
                }
            }
            Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
                ..
            } if polarity => {
                out.extend(self.prim_narrowings(lhs, true));
                out.extend(self.prim_narrowings(rhs, true));
            }
            Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
                ..
            } if !polarity => {
                out.extend(self.prim_narrowings(lhs, false));
                out.extend(self.prim_narrowings(rhs, false));
            }
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => out.extend(self.prim_narrowings(expr, !polarity)),
            _ => {}
        }
        out
    }

    /// Give each narrowed local its `CTy`, returning what it displaced for [`Self::unnarrow`].
    pub(in crate::compiler) fn narrow(&mut self, narrowings: &[(usize, CTy)]) -> Vec<(usize, CTy)> {
        let saved = narrowings
            .iter()
            .map(|(slot, _)| (*slot, self.locals[*slot].ty.clone()))
            .collect();
        for (slot, cty) in narrowings {
            self.locals[*slot].ty = cty.clone();
        }
        saved
    }

    /// Put back what [`Self::narrow`] displaced — in reverse, so a slot narrowed twice ends as it began.
    pub(in crate::compiler) fn unnarrow(&mut self, saved: Vec<(usize, CTy)>) {
        for (slot, cty) in saved.into_iter().rev() {
            self.locals[slot].ty = cty;
        }
    }

    /// Compile `e` under `cond`'s `polarity` narrowings (an `&&`/`||` right operand, an if-expression
    /// arm).
    pub(in crate::compiler) fn expr_narrowed(
        &mut self,
        e: &Expr,
        cond: &Expr,
        polarity: bool,
    ) -> Result<(), String> {
        let n = self.prim_narrowings(cond, polarity);
        let saved = self.narrow(&n);
        let r = self.expr(e);
        self.unnarrow(saved);
        r
    }

    /// `ctype` of `e` as if `cond`'s `polarity` narrowings were installed — `ctype` takes `&self`, so
    /// the narrowed types go through the `narrow_overlay` it consults for a local, not into `locals`.
    pub(in crate::compiler) fn ctype_narrowed(
        &self,
        e: &Expr,
        cond: &Expr,
        polarity: bool,
    ) -> Result<CTy, String> {
        let n = self.prim_narrowings(cond, polarity);
        let depth = self.narrow_overlay.borrow().len();
        self.narrow_overlay.borrow_mut().extend(n);
        let r = self.ctype(e);
        self.narrow_overlay.borrow_mut().truncate(depth);
        r
    }
}

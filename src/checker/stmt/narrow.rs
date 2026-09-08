//! Flow-narrowing (S5.3) — the variable refinements a condition implies when it is true, and the
//! complementary ones when it is false (`instanceof`, null checks, union member tests).
//!
//! Split out of `flow.rs` (Invariant 13): narrowing is a self-contained analysis over a condition
//! expression; the rest of `flow.rs` is destructuring and loop checking.

use super::*;

impl Checker {
    pub(in crate::checker) fn narrow_from_condition(
        &self,
        cond: &crate::ast::Expr,
        polarity: bool,
    ) -> Vec<(String, Ty)> {
        use crate::ast::{BinaryOp, Expr, UnaryOp};
        let mut out = Vec::new();
        match cond {
            Expr::InstanceOf {
                value, type_name, ..
            } => {
                if let Expr::Ident(name, _) = &**value {
                    // Slice 3 (DEC-184): a primitive type-test narrows the variable to the tested
                    // primitive in the then-branch (`if (x is int)` ⇒ `x: int`). The VM compiler
                    // replicates this exact then-branch narrowing (`compile_if`), so arithmetic on the
                    // narrowed value (`x + 1`) is lockstep.
                    if let Some(prim) = prim_pat_ty(type_name) {
                        if polarity {
                            out.push((name.clone(), prim));
                        } else if let Some((Ty::Optional(inner), _)) = self.lookup_binding(name) {
                            // `is null` over an optional: the complement is the non-null inner.
                            // Lockstep-safe — an optional local already carries its inner `CTy` on the
                            // VM (`resolve_cty`), so no compiler narrowing is needed to specialize it.
                            if matches!(prim, Ty::Null) {
                                out.push((name.clone(), *inner));
                            }
                        }
                        // Deliberately NO union-minus-primitive complement (`(int | string)` else-branch
                        // ⇒ `string`): the VM compiler can't derive it — a union local is `CTy::Other`
                        // and the member set is lost — so narrowing it here would be a
                        // checker-accepts/VM-rejects divergence. Reach the complement with a nested
                        // `is`/`match`. General fix tracked as W2-12 (erased-operand dynamic fallback).
                        return out;
                    }
                    let known = self.classes.contains_key(type_name)
                        || self.interfaces.contains_key(type_name);
                    if !known {
                        return out;
                    }
                    if polarity {
                        // then-branch: narrow to the tested type. `instanceof` carries no type
                        // arguments at runtime (`instanceof Box<int>` ≡ `instanceof Box`), so a
                        // generic class narrows with erased (poison) args — its generic members read
                        // as `mixed` (M-RT generics-all).
                        let arity = self
                            .classes
                            .get(type_name)
                            .map_or(0, |c| c.type_params.len());
                        out.push((
                            name.clone(),
                            Ty::Named(type_name.clone(), vec![Ty::Error; arity]),
                        ));
                    } else if let Some((Ty::Union(members), _)) = self.lookup_binding(name) {
                        // else-branch: drop the tested member (and any subtype of it) from the union.
                        let orig = members.len();
                        let rest: Vec<Ty> = members
                            .into_iter()
                            .filter(|m| {
                                !matches!(m, Ty::Named(n, _)
                                    if n == type_name || self.is_subtype(n, type_name))
                            })
                            .collect();
                        if !rest.is_empty() && rest.len() < orig {
                            out.push((name.clone(), Ty::union_of(rest)));
                        }
                    }
                }
            }
            // (Phorj has no `x == null` / `x != null` comparison — the checker rejects comparing a
            // `T?` to the null literal; optionals are tested via if-let / `??` / match-over-optional,
            // so there is no null-equality narrowing source here.)
            // `a && b` narrows the conjunction on its true side; `a || b` narrows on its false side
            // (De Morgan: `!(a || b)` ≡ `!a && !b`). The other polarity yields a disjunction — no
            // single narrowing — so it contributes nothing.
            Expr::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
                ..
            } if polarity => {
                out.extend(self.narrow_from_condition(lhs, true));
                out.extend(self.narrow_from_condition(rhs, true));
            }
            Expr::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
                ..
            } if !polarity => {
                out.extend(self.narrow_from_condition(lhs, false));
                out.extend(self.narrow_from_condition(rhs, false));
            }
            // `!c` flips the polarity.
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => out.extend(self.narrow_from_condition(expr, !polarity)),
            _ => {}
        }
        out
    }
}

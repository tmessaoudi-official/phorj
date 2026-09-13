//! Lane R-6 — the builder idiom `$out = []; …; return $out;` gets its type from the function's
//! declared return (`@return list<T>` / `array<K, V>` after R-4, or a phorj-typed return): a local
//! declared with an EMPTY list literal, returned at the top level of the same body, becomes
//! `mutable var out = new List<T>()` typed as the return. Nothing is inferred from the elements —
//! the type comes from a declaration the program wrote. Any other empty literal is left as is (the
//! checker then asks for its type, exactly as before).

use super::*;

/// Lane L1c (2026-09-07): a POSITIONAL `array{…}` shape lifts to a tuple TYPE, so the literal that
/// satisfies it must lift to a tuple VALUE. `return [$n, 'x'];` under `@return array{int, string}`
/// becomes `return (n, "x");`. Without this the lifter emits two halves that disagree with each
/// other — the draft lifts and then `phg check` reports `expected (int, string), found List<int>`.
///
/// The arity must MATCH. A literal of the wrong length is left as a list rather than padded or
/// truncated into the declared shape: the disagreement is then reported by the checker, which is the
/// honest outcome (DEC-166 — the lifter does not guess).
pub(super) fn seed_returned_tuple_literals(body: &mut [Stmt], ret: &Option<Type>) {
    let Some(ret) = ret else {
        return;
    };
    // A nullable tuple return (`?array` + `@return array{…}|null`) seeds the same way: `return
    // [1, "one"];` is the tuple, and the `null` arm is unaffected.
    let ty = match ret {
        Type::Optional { inner, .. } => inner.as_ref(),
        other => other,
    };
    let Type::Tuple(elems, labels, _) = ty else {
        return;
    };
    let shape = TupleShape {
        arity: elems.len(),
        labels: labels
            .as_ref()
            .map(|ls| ls.iter().map(|l| l.name.clone()).collect()),
    };
    seed_tuple_returns_in(body, &shape);
}

/// The declared tuple a `return` must satisfy: how many positions, and — for a NAMED-field tuple
/// (DEC-504) — what they are called.
struct TupleShape {
    arity: usize,
    labels: Option<Vec<String>>,
}

impl TupleShape {
    /// The tuple literal this expression should become, or `None` to leave it exactly as it is.
    ///
    /// Two literal forms answer a declared tuple, and both must MATCH rather than be coerced into
    /// it — a literal of the wrong arity or the wrong keys is left alone so that the checker reports
    /// the disagreement, which is the honest outcome (DEC-166 — the lifter does not guess).
    fn tuple_from(&self, e: &mut Expr) -> Option<Expr> {
        // A POSITIONAL literal answers by arity, as it has since Lane L1c. When the declared shape
        // is named, its own labels name the positions — nothing is invented.
        if let Expr::List(items, _) = e {
            if items.len() != self.arity {
                return None;
            }
            return Some(Expr::Tuple(
                std::mem::take(items),
                crate::ast::to_ast_labels(self.labels.as_ref(), SP),
                SP,
            ));
        }
        // DEC-515, the WRITE half: a KEYED literal answers a NAMED shape when its keys are exactly
        // the declared fields, in the declared ORDER.
        //
        // The order requirement is not pedantry. Field order is part of a named tuple's type
        // (`ast::tuple_labels`) because erasure is positional, so reordering the literal here would
        // move the evaluation order of the value expressions relative to the order they are stored
        // in — a byte-identity surface, and a silent semantic change in any literal whose values
        // have side effects. A differently-ordered literal is therefore left a Map for the checker
        // to report. Every keyed literal in the scout corpus is already written in declared order.
        if let Expr::Map(pairs, _) = e {
            let labels = self.labels.as_ref()?;
            if pairs.len() != labels.len() {
                return None;
            }
            for ((key, _), want) in pairs.iter().zip(labels) {
                let Expr::Str(parts, _) = key else {
                    return None;
                };
                if super::super::str_literal_text(parts).as_deref() != Some(want.as_str()) {
                    return None;
                }
            }
            let values: Vec<Expr> = pairs.drain(..).map(|(_, v)| v).collect();
            return Some(Expr::Tuple(
                values,
                crate::ast::to_ast_labels(Some(labels), SP),
                SP,
            ));
        }
        None
    }
}

/// The recursive half: EVERY `return` in the body answers to the declared return type, not only the
/// ones at the top level — `if ($yes) { return [1, "one"]; }` is the shape the idiom actually takes.
/// The walk descends into statement blocks ONLY and never into an expression, because a `return`
/// inside a lambda body belongs to that lambda's own signature, not to this function's.
///
/// The match is exhaustive by design (Invariant 3): a new block-bearing `Stmt` must decide whether
/// its returns are this function's, and a `_` arm would silently answer "no".
fn seed_tuple_returns_in(body: &mut [Stmt], shape: &TupleShape) {
    for s in body.iter_mut() {
        match s {
            Stmt::Return {
                value: Some(v),
                span,
            } => {
                if let Some(tuple) = shape.tuple_from(v) {
                    *s = Stmt::Return {
                        value: Some(tuple),
                        span: *span,
                    };
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                seed_tuple_returns_in(then_block, shape);
                if let Some(e) = else_block {
                    seed_tuple_returns_in(e, shape);
                }
            }
            Stmt::For { body, .. }
            | Stmt::While { body, .. }
            | Stmt::CFor { body, .. }
            | Stmt::Block(body, _)
            | Stmt::Using { body, .. } => seed_tuple_returns_in(body, shape),
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                seed_tuple_returns_in(body, shape);
                for c in catches.iter_mut() {
                    seed_tuple_returns_in(&mut c.body, shape);
                }
                if let Some(f) = finally_block {
                    seed_tuple_returns_in(f, shape);
                }
            }
            Stmt::Destructure { else_block, .. } => {
                if let Some(e) = else_block {
                    seed_tuple_returns_in(e, shape);
                }
            }
            // No block of this function's statements inside: nothing to descend into.
            Stmt::Return { value: None, .. }
            | Stmt::VarDecl { .. }
            | Stmt::Assign { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Expr(..)
            | Stmt::Discard(..)
            | Stmt::Throw { .. } => {}
        }
    }
}

pub(super) fn seed_returned_empty_literals(body: &mut [Stmt], ret: &Option<Type>) {
    let Some(ret) = ret else {
        return;
    };
    // A nullable return (`?array` + `@return list<T>`) seeds from its inner type: the local is a
    // plain `List<T>`, and `return $out;` widens to `List<T>?` at the return, as it would in PHP.
    let ret = match ret {
        Type::Optional { inner, .. } => inner.as_ref(),
        other => other,
    };
    if !matches!(ret, Type::Named { name, .. } if name == "List" || name == "Map") {
        return;
    }
    let returned: Vec<String> = body
        .iter()
        .filter_map(|s| match s {
            Stmt::Return {
                value: Some(Expr::Ident(n, _)),
                ..
            } => Some(n.clone()),
            _ => None,
        })
        .collect();
    for s in body.iter_mut() {
        if let Stmt::VarDecl { name, init, .. } = s {
            if returned.contains(name) && matches!(init, Expr::List(items, _) if items.is_empty()) {
                // The declaration stays `var`: `new Map<K, V>()` already carries the type, and the
                // `@var` path prints the same shape.
                if let Ok(coll) = super::super::mappings::new_coll(ret) {
                    *init = coll;
                }
            }
        }
    }
}

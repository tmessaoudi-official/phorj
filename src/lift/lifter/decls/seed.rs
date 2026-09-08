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
    let elems = match ret {
        Type::Tuple(e, _, _) => e,
        Type::Optional { inner, .. } => match inner.as_ref() {
            Type::Tuple(e, _, _) => e,
            _ => return,
        },
        _ => return,
    };
    seed_tuple_returns_in(body, elems.len());
}

/// The recursive half: EVERY `return` in the body answers to the declared return type, not only the
/// ones at the top level — `if ($yes) { return [1, "one"]; }` is the shape the idiom actually takes.
/// The walk descends into statement blocks ONLY and never into an expression, because a `return`
/// inside a lambda body belongs to that lambda's own signature, not to this function's.
///
/// The match is exhaustive by design (Invariant 3): a new block-bearing `Stmt` must decide whether
/// its returns are this function's, and a `_` arm would silently answer "no".
fn seed_tuple_returns_in(body: &mut [Stmt], arity: usize) {
    for s in body.iter_mut() {
        match s {
            Stmt::Return {
                value: Some(v),
                span,
            } => {
                if let Expr::List(items, _) = v {
                    if items.len() == arity {
                        *s = Stmt::Return {
                            value: Some(Expr::Tuple(std::mem::take(items), None, SP)),
                            span: *span,
                        };
                    }
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                seed_tuple_returns_in(then_block, arity);
                if let Some(e) = else_block {
                    seed_tuple_returns_in(e, arity);
                }
            }
            Stmt::For { body, .. }
            | Stmt::While { body, .. }
            | Stmt::CFor { body, .. }
            | Stmt::Block(body, _)
            | Stmt::Using { body, .. } => seed_tuple_returns_in(body, arity),
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                seed_tuple_returns_in(body, arity);
                for c in catches.iter_mut() {
                    seed_tuple_returns_in(&mut c.body, arity);
                }
                if let Some(f) = finally_block {
                    seed_tuple_returns_in(f, arity);
                }
            }
            Stmt::Destructure { else_block, .. } => {
                if let Some(e) = else_block {
                    seed_tuple_returns_in(e, arity);
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

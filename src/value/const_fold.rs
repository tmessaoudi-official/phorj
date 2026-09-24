//! Folding a class `const` initializer to its `Value` (DEC-533) — the one kernel every leg reads a
//! constant through ([`crate::ast::class_consts`] feeds the interpreter and the VM's `Op::Const`; the
//! checker accepts exactly what folds here).

use super::*;
use crate::ast::Expr;

/// A class constant's value: a literal constant ([`const_literal`]) or a List/Map literal whose every
/// element is itself a constant, to any depth. A Map is built through the shared [`build_map`], so a
/// duplicate key keeps its first position and last value — PHP's rule, and the runtime literal's.
/// `None` for anything else, including a key that is not a map key. Deliberately NOT used for a
/// backed-enum value or a static default: those stay scalar ([`const_literal`]).
pub fn const_value(e: &Expr) -> Option<Value> {
    match e {
        Expr::List(items, _) => items
            .iter()
            .map(const_value)
            .collect::<Option<Vec<_>>>()
            .map(|xs| Value::List(Rc::new(xs))),
        Expr::Map(pairs, _) => {
            let mut folded = Vec::with_capacity(pairs.len());
            for (k, v) in pairs {
                folded.push((const_value(k)?, const_value(v)?));
            }
            build_map(folded).ok().map(|m| Value::Map(Rc::new(m)))
        }
        _ => const_literal(e),
    }
}

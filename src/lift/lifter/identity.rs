//! PHP lifter — a STRICT identity test against a literal that phorj spells as a test, not an
//! equality (moved out of `exprs.rs` under Invariant 13, row 4h).
//!
//! Two literals qualify, and both are handled in either operand order, because Yoda style
//! (`null === $x`) is common in real PHP:
//!
//! - `null` — `$x === null` asks exactly "is this the null case of an optional", which is phorj's
//!   `is null`. `==` cannot express it: the checker rejects `T? == null` as a cross-type comparison,
//!   so lifting it as an equality produced a draft that lifted and then failed `phg check`.
//! - `[]` (row 4h) — phorj has no untyped empty literal (DEC-214 part-2), so `$xs === []` becomes
//!   `List.isEmpty(xs)` / `Map.isEmpty(m)`, for a variable DECLARED as a non-null `List` or `Map`
//!   ([`super::shapes::coll_leaf`]); anything else is left as it was and the checker names the bare
//!   `[]`. The call is QUALIFIED, never `xs.isEmpty()`, because the declaration is only a hint the
//!   lifter cannot keep flow-sensitive (a `foreach` binder, a closure parameter or a `catch` can
//!   rebind the name), and `isEmpty` also exists on `string`, where `"" === []` is FALSE in PHP
//!   and `"".isEmpty()` is true. The qualified native type-checks only on that collection, so — as
//!   in `spread.rs` — the only way for the draft to be wrong is to be loud.
//!
//! STRICT ONLY. PHP's LOOSE `$x == null` / `$x == []` is also true for `0`, `""`, `[]`, `null` and
//! `false`, so it is NOT this test; it stays a plain `==` and the checker then reports it, which is
//! the honest DEC-166 outcome — the lifter does not guess which of the five the author meant.

use super::*;

fn is_empty_array(e: &php::PhpExpr) -> bool {
    matches!(e, php::PhpExpr::Array(items) if items.is_empty())
}

/// The operand compared against `lit`, in either order, when the other side matches `lit`.
fn subject<'a>(
    left: &'a php::PhpExpr,
    right: &'a php::PhpExpr,
    lit: impl Fn(&php::PhpExpr) -> bool,
) -> Option<&'a php::PhpExpr> {
    if lit(left) {
        Some(right)
    } else if lit(right) {
        Some(left)
    } else {
        None
    }
}

/// The declared collection an empty-array test is about, with the leaf (`List`/`Map`) of its type.
fn empty_test_subject<'a>(
    left: &'a php::PhpExpr,
    right: &'a php::PhpExpr,
) -> Option<(&'a php::PhpExpr, &'static str)> {
    let s = subject(left, right, is_empty_array)?;
    match s {
        php::PhpExpr::Var(v) => super::shapes::coll_leaf(v).map(|m| (s, m)),
        _ => None,
    }
}

/// Whether [`lift_identity`] owns this `===` / `!==`.
pub(super) fn handles(left: &php::PhpExpr, right: &php::PhpExpr) -> bool {
    let is_null = |e: &php::PhpExpr| matches!(e, php::PhpExpr::Null);
    subject(left, right, is_null).is_some() || empty_test_subject(left, right).is_some()
}

pub(super) fn lift_identity(
    op: &php::PhpBinOp,
    left: &php::PhpExpr,
    right: &php::PhpExpr,
) -> Result<Expr, String> {
    let test = match empty_test_subject(left, right) {
        Some((coll, leaf)) => super::spread::module_call(leaf, "isEmpty", vec![lift_expr(coll)?]),
        None => {
            let is_null = |e: &php::PhpExpr| matches!(e, php::PhpExpr::Null);
            let value = subject(left, right, is_null)
                .ok_or_else(|| "identity: not a null test".to_string())?;
            Expr::InstanceOf {
                value: Box::new(lift_expr(value)?),
                type_name: "null".to_string(),
                span: SP,
            }
        }
    };
    Ok(match op {
        php::PhpBinOp::Identical => test,
        // phorj has no `is not null`: the negation is spelled `!(x is null)`.
        _ => Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(test),
            span: SP,
        },
    })
}

//! Row 4i — PHP builtins whose phorj native exists but takes its arguments in ANOTHER ORDER, so the
//! registry's same-order `lift_from` cannot invert them. Each lifts to the native whose PHP template
//! IS the builtin (`List.contains` erases to `in_array(x, xs, true)`, `String.join` to `implode`,
//! `String.split` to `explode`, `String.replace` to `str_replace`) — never an approximation.
//!
//! **Evaluation order.** PHP evaluates arguments left to right; the receiver form evaluates the
//! receiver first. `implode($sep, $xs)` → `xs.join(sep)` therefore reorders `$sep` and `$xs` — which
//! is only unobservable when at most one argument can DO anything. [`reorderable`] is that test: every
//! argument but one must be [`is_inert`] (a literal, a variable, a constant, a read of those). A call
//! whose arguments fail it falls through to the plain unresolved call — loud, never a reordered effect.
//! "Inert" does not consider a FAULT (a division by zero, a missing key's warning) inside an argument:
//! two faulting arguments would report the other one first — a `// lifted (verify)` draft's limit.

use super::*;

/// Whether evaluating `e` can have no effect a reordering could expose: literals, variables,
/// constants, closure literals and first-class callables (creating one runs nothing), and reads,
/// casts, operators, interpolations and array literals built only from such.
pub(super) fn is_inert(e: &php::PhpExpr) -> bool {
    use php::PhpExpr as P;
    match e {
        P::Int(_) | P::Float(_) | P::Str(_) | P::Bool(_) | P::Null | P::Var(_) | P::Name(_) => true,
        P::ClassConst { .. } | P::StaticProp { .. } | P::CallableRef(_) => true,
        P::Closure { .. } | P::BlockClosure { .. } => true,
        P::Member { recv, .. } => is_inert(recv),
        P::Index { base, index } => is_inert(base) && is_inert(index),
        P::Cast { value, .. } | P::InstanceOf { value, .. } => is_inert(value),
        P::Unary { expr, .. } => is_inert(expr),
        P::Binary { left, right, .. } => is_inert(left) && is_inert(right),
        P::Ternary { cond, then, els } => {
            is_inert(cond) && then.as_deref().is_none_or(is_inert) && is_inert(els)
        }
        P::Interp(parts) => parts.iter().all(|p| match p {
            php::PhpStrPart::Lit(_) => true,
            php::PhpStrPart::Expr(e) => is_inert(e),
        }),
        P::Array(items) => items
            .iter()
            .all(|i| i.key.as_ref().is_none_or(is_inert) && is_inert(&i.value)),
        _ => false,
    }
}

/// At most one argument does anything, so the receiver form's order cannot be observed.
pub(super) fn reorderable(args: &[&php::PhpExpr]) -> bool {
    args.iter().filter(|a| !is_inert(a)).count() <= 1
}

fn string_call(recv: Expr, name: &str, args: Vec<Expr>) -> Expr {
    record_native_module("Core.String");
    super::arity_fns::member_call(recv, name, args)
}

/// `PHP_INT_MAX` is phorj's `int` bound exactly (both are 64-bit).
pub(super) fn lift_php_constant(name: &str) -> Option<Expr> {
    (name == "PHP_INT_MAX").then_some(Expr::Int(i64::MAX, SP))
}

pub(super) fn lift_reordered(
    callee: &php::PhpExpr,
    args: &[php::PhpExpr],
) -> Option<Result<Expr, String>> {
    let php::PhpExpr::Name(n) = callee else {
        return None;
    };
    let plain =
        |a: &php::PhpExpr| !matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. });
    if !args.iter().all(plain) {
        return None;
    }
    let refs: Vec<&php::PhpExpr> = args.iter().collect();
    match (n.as_str(), args) {
        // QUALIFIED, as `List.isEmpty` is (row 4h): `contains` also exists on `string`, where
        // `in_array($x, $s, true)` is a PHP TypeError and `s.contains(x)` a substring test.
        ("in_array", [x, xs, php::PhpExpr::Bool(true)]) if reorderable(&refs) => Some((|| {
            let (xs, x) = (lift_expr(xs)?, lift_expr(x)?);
            Ok(super::spread::module_call("List", "contains", vec![xs, x]))
        })()),
        ("in_array", [_, _]) => Some(Err(
            "lift: a two-argument `in_array` compares LOOSELY (`in_array(\"1\", [\"01\"])` is true) — pass \
             `true` as the third argument for the strict test, which lifts to `xs.contains(x)`"
                .into(),
        )),
        ("implode", [sep, xs]) if reorderable(&refs) => {
            Some((|| Ok(string_call(lift_expr(xs)?, "join", vec![lift_expr(sep)?])))())
        }
        ("explode", [sep, s]) if reorderable(&refs) => {
            Some((|| Ok(string_call(lift_expr(s)?, "split", vec![lift_expr(sep)?])))())
        }
        ("explode", [_, _, _]) => Some(Err(
            "lift: `explode` with a limit has no phorj native — split, then `take`/`join` the tail".into(),
        )),
        ("str_replace", [search, replace, subject]) if reorderable(&refs) => {
            Some(lift_str_replace(search, replace, subject))
        }
        ("str_replace", [_, _, _, _]) => Some(Err(
            "lift: `str_replace` with a count argument (by reference) has no phorj form — count the \
             occurrences separately"
                .into(),
        )),
        _ => None,
    }
}

/// `str_replace` over a literal search ARRAY replaces each needle in turn, over the result of the
/// previous one — which is exactly a chain of `replace`. A scalar replacement is reused for every
/// needle; a replacement array must be the same length (PHP pads a shorter one with `""`, which is
/// spelled out rather than guessed). A chain re-evaluates the replacement, so it must be inert.
fn lift_str_replace(
    search: &php::PhpExpr,
    replace: &php::PhpExpr,
    subject: &php::PhpExpr,
) -> Result<Expr, String> {
    fn list(e: &php::PhpExpr) -> Option<Vec<&php::PhpExpr>> {
        match e {
            php::PhpExpr::Array(items) if items.iter().all(|i| i.key.is_none()) => {
                Some(items.iter().map(|i| &i.value).collect())
            }
            _ => None,
        }
    }
    let pairs: Vec<(&php::PhpExpr, &php::PhpExpr)> = match (list(search), list(replace)) {
        (None, None) => vec![(search, replace)],
        (Some(needles), None) if is_inert(replace) => {
            needles.into_iter().map(|n| (n, replace)).collect()
        }
        (Some(needles), Some(reps)) if needles.len() == reps.len() => {
            needles.into_iter().zip(reps).collect()
        }
        (Some(_), Some(_)) => {
            return Err(
                "lift: `str_replace` with search and replacement arrays of different lengths pads \
                 the shorter one with \"\" — write both arrays at the same length"
                    .into(),
            )
        }
        _ => return Err(
            "lift: `str_replace` over an array needs literal array operands (the lift chains one \
                 `replace` per needle) and an inert replacement"
                .into(),
        ),
    };
    let mut acc = lift_expr(subject)?;
    for (needle, rep) in pairs {
        acc = string_call(acc, "replace", vec![lift_expr(needle)?, lift_expr(rep)?]);
    }
    Ok(acc)
}

//! Row 4g — PHP builtins whose phorj form depends on how many arguments the call passes. The
//! registry's `lift_of` is arity-EXACT (one native per PHP name), so these arities fell through to an
//! unresolved call. Only the arities the registry CANNOT place are handled here — `min($a, $b)`,
//! `substr($s, $i, $n)`, `array_filter($xs, $f)` and `strtr($s, $from, $to)` keep their registry lift —
//! and a spread or named argument is never read as one of these shapes (it falls through, loud later).
//!
//! * `min($xs)` / `max($xs)` — the one-argument form takes an ARRAY → `xs.min()!` / `xs.max()!`. Three
//!   costs, disclosed: every site carries a `W-FORCE-UNWRAP` warning; an empty list is a phorj
//!   unwrap fault where PHP throws `ValueError` (both fatal, a different class); and a list of strings
//!   orders byte-wise, not by PHP's numeric-string juggling (`Core.List`'s documented `natural_cmp`).
//! * `min($a, $b, $c, …)` — a left fold of the two-argument `Math.min`, `a.min(b).min(c)`: arguments
//!   are still evaluated in source order, and `min` is associative.
//! * `substr($s, $i)` — "to the end" → `s.substring(i, 9223372036854775807)`, PHP's own idiom (a length
//!   of `PHP_INT_MAX`); `substring` saturates exactly as PHP clamps (row 5z). Not `s.length()`, which
//!   would evaluate `s` twice.
//! * `array_filter($xs)` and the two-argument `strtr($s, $pairs)` — refused by name: the first keeps the
//!   PHP-TRUTHY values, which depends on the element type; the second replaces substrings
//!   longest-first, which no phorj native does yet (row 4i).
//!
//! [`handles`] is the single source for "this builtin's native depends on its arity" — row 4f2's
//! first-class-callable lift consults it rather than keeping a second list.
use super::*;

/// The PHP builtins whose phorj native is chosen by arity. Row 4f2 refuses their `f(...)` by name.
pub(super) fn handles(name: &str) -> bool {
    matches!(name, "min" | "max" | "substr" | "array_filter" | "strtr")
}

pub(super) fn member_call(object: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(object),
            name: name.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args,
        type_args: Vec::new(),
        span: SP,
    }
}

pub(super) fn lift_by_arity(
    callee: &php::PhpExpr,
    args: &[php::PhpExpr],
) -> Option<Result<Expr, String>> {
    let php::PhpExpr::Name(n) = callee else {
        return None;
    };
    let plain =
        |a: &php::PhpExpr| !matches!(a, php::PhpExpr::Spread(_) | php::PhpExpr::NamedArg { .. });
    if !handles(n) || !args.iter().all(plain) {
        return None;
    }
    match (n.as_str(), args) {
        ("min" | "max", [xs]) => Some((|| {
            record_native_module("Core.List");
            Ok(Expr::Force {
                inner: Box::new(member_call(lift_expr(xs)?, n, Vec::new())),
                span: SP,
            })
        })()),
        ("min" | "max", [first, rest @ ..]) if rest.len() >= 2 => Some((|| {
            record_native_module("Core.Math");
            let mut acc = lift_expr(first)?;
            for a in rest {
                acc = member_call(acc, n, vec![lift_expr(a)?]);
            }
            Ok(acc)
        })()),
        ("substr", [s, i]) => Some((|| {
            record_native_module("Core.String");
            Ok(member_call(
                lift_expr(s)?,
                "substring",
                vec![lift_expr(i)?, Expr::Int(i64::MAX, SP)],
            ))
        })()),
        ("array_filter", [_]) => Some(Err(
            "lift: `array_filter($xs)` without a callback keeps the PHP-TRUTHY values, and truthiness \
             depends on the element type (`0`, `\"\"`, `\"0\"`, `null`, `[]`, `false`) — write the callback, \
             e.g. `array_filter($xs, fn (?Item $x): bool => $x !== null)`; for a list of ARRAYS, whose \
             callback would need an untyped `array` parameter, write a `foreach` instead"
                .into(),
        )),
        ("strtr", [_, _]) => Some(Err(
            "lift: the two-argument `strtr($s, $pairs)` replaces substrings longest-first, and no phorj \
             native does that yet (row 4i) — the three-argument `strtr($s, $from, $to)` lifts to \
             `String.translate`"
                .into(),
        )),
        _ => None,
    }
}

//! Row 4i — PHP builtins whose phorj native exists but takes its arguments in ANOTHER ORDER, so the
//! registry's same-order `lift_from` cannot invert them. Each lifts to the native whose PHP template
//! IS the builtin (`List.contains` erases to `in_array(x, xs, true)`, `String.join` to `implode`,
//! `String.split` to `explode`, `String.replace` to `str_replace`) — never an approximation.
//!
//! **Evaluation order.** PHP evaluates arguments left to right; the receiver form evaluates the
//! receiver first. `implode($sep, $xs)` → `xs.join(sep)` therefore reorders `$sep` and `$xs` — which
//! is only unobservable when there is one EFFECT at most and nothing else READS state it could change
//! ([`reorderable`]). A call whose arguments fail it falls through to the plain unresolved call —
//! loud, never a reordered effect. The classification does not see a FAULT (a division by zero, a
//! missing key's warning) nor a magic `__get` / `__toString` inside an argument: those would run in the
//! other order — a `// lifted (verify)` draft's limit.

use super::*;

/// What evaluating an argument can do, ordered: a FIXED value no call can change (a literal, a
/// local variable — PHP locals cannot be reached by a callee here: no by-reference capture, DEC-506,
/// no `static` locals, row 5w — a constant, a closure or first-class callable), a READ of state a
/// call could change (a property, a static property, an element of one), or an EFFECT (a call,
/// `new`, an assignment). A compound takes the strongest of its parts.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Fixed,
    Read,
    Effect,
}

fn kind(e: &php::PhpExpr) -> Kind {
    use php::PhpExpr as P;
    let all =
        |es: &mut dyn Iterator<Item = &php::PhpExpr>| es.map(kind).max().unwrap_or(Kind::Fixed);
    match e {
        P::Int(_) | P::Float(_) | P::Str(_) | P::Bool(_) | P::Null | P::Var(_) | P::Name(_) => {
            Kind::Fixed
        }
        P::ClassConst { .. } | P::CallableRef(_) | P::Closure { .. } | P::BlockClosure { .. } => {
            Kind::Fixed
        }
        P::StaticProp { .. } => Kind::Read,
        P::Member { recv, .. } => kind(recv).max(Kind::Read),
        P::Index { base, index } => kind(base).max(kind(index)),
        P::Cast { value, .. } | P::InstanceOf { value, .. } => kind(value),
        P::Unary { expr, .. } => kind(expr),
        P::Binary { left, right, .. } => kind(left).max(kind(right)),
        P::Ternary { cond, then, els } => all(&mut [Some(&**cond), then.as_deref(), Some(&**els)]
            .into_iter()
            .flatten()),
        P::Interp(parts) => all(&mut parts.iter().filter_map(|p| match p {
            php::PhpStrPart::Lit(_) => None,
            php::PhpStrPart::Expr(e) => Some(&**e),
        })),
        P::Array(items) => all(&mut items
            .iter()
            .flat_map(|i| i.key.iter().chain(std::iter::once(&i.value)))),
        _ => Kind::Effect,
    }
}

/// Whether evaluating `e` DOES nothing — it may still read state, so it can be evaluated again or
/// out of order next to anything that is not an effect.
pub(super) fn is_inert(e: &php::PhpExpr) -> bool {
    kind(e) != Kind::Effect
}

/// The receiver form's order cannot be observed: no argument has an effect, or exactly one does and
/// no other argument reads state it could change (`implode($this->sep, $this->build())` must keep
/// reading `$this->sep` BEFORE `build()` runs, so it is not reordered).
pub(super) fn reorderable(args: &[&php::PhpExpr]) -> bool {
    let kinds: Vec<Kind> = args.iter().map(|a| kind(a)).collect();
    let effects = kinds.iter().filter(|k| **k == Kind::Effect).count();
    let reads = kinds.iter().filter(|k| **k == Kind::Read).count();
    effects == 0 || (effects == 1 && reads == 0)
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

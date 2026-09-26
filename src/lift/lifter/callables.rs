//! Row 4e — `\Closure::fromCallable($f)` lifts to `f` itself. PHP needs the call to normalise any
//! `callable` (a function name, an `[obj, 'method']` pair) into a `\Closure`; in the draft the
//! argument is already a phorj function VALUE — its parameter was typed from the docblock signature —
//! so the conversion is the identity. A string or array callable lifts to a non-function value, which
//! `phg check` then refuses loudly: never a silent guess. Any other shape returns `None` and falls
//! through to the generic static call. The caller lifts the returned argument.
use super::*;

pub(super) fn from_callable_arg<'a>(
    class: &str,
    name: &str,
    args: &'a [php::PhpExpr],
) -> Option<&'a php::PhpExpr> {
    match (class, name, args) {
        ("Closure", "fromCallable", [f]) if !matches!(f, php::PhpExpr::Spread(_)) => Some(f),
        _ => None,
    }
}

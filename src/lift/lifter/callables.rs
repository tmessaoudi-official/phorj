//! Row 4f — `name(...)`, a first-class callable of a named function, lifts to a phorj function
//! reference (`len`), which runs on all three legs and transpiles back to `len(...)`.
//!
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

/// `name(...)` → the reference `name`. A registered BUILTIN is refused: which native it means can
/// depend on the arity it is later called with (`min($xs)` is `List.min`, `min($a, $b)` `Math.min`),
/// and that data is row 4g's. An unregistered name lifts as a reference and `phg check` names it if it
/// resolves to nothing — the same outcome as an unmapped call.
pub(super) fn lift_callable_ref(name: &str) -> Result<Expr, String> {
    if crate::native::lift_of(name).is_some() {
        return Err(format!(
            "lift: a first-class callable of the builtin `{name}` is Tier-2 — which phorj native it means \
             can depend on the arity it is called with (`min($xs)` vs `min($a, $b)`, row 4g). Write a \
             closure with a typed parameter, `fn (string $s): int => {name}($s)`; an `array` parameter \
             needs a docblock type a closure cannot carry, so write a `foreach` there"
        ));
    }
    Ok(Expr::Ident(name.to_string(), SP))
}

//! Row 5u — DEC-538: PHP spread `...` lifts to `Core.List` calls, with no new phorj syntax.
//!
//! `[...$a, $x, ...$b]` → `List.flatten([a, [x], b])` (two segments: `List.concat(a, b)`),
//! `array_merge(...$xss)` → `List.flatten(xss)`, and the statement `array_unshift($r, ...$xs);` →
//! `r = List.concat(xs, r);`. An operand spelled `array_values($m)` lifts to `Map.values(m)`: that is
//! the PHP idiom for spreading a map's values, and phorj `Map` keeps PHP's insertion order,
//! overwrites included (measured on every leg before the build; `spread_over_lists` round-trip).
//!
//! The CHECKER arbitrates list-ness (refinement 2026-09-26 08:05): `concat`/`flatten`/`values`
//! type-check only on the right collection, and a phorj List is sequential by construction, so PHP's
//! renumbering spread agrees whenever the draft checks — the only way to be wrong is to be loud.
//! That is also why a lone `[...$a]` is `List.flatten([a])` and not a bare `a`: a bare operand would
//! let a map through unchecked. Only a PROVABLY-map operand — a keyed literal, a known map constant,
//! a Map-declared variable — is refused here, by name. Every spread this file does not consume
//! reaches `lift_expr`'s `Spread` arm and is refused there, also by name.

use super::*;

/// The refusal for a spread `lift_expr` meets outside the shapes this file consumes.
pub(super) const UNCONSUMED: &str = "lift: argument unpacking `...` has no phorj form here — \
     DEC-538 lifts a spread only inside a list literal, as `array_merge(...$x)` and in the \
     statement `array_unshift($v, ...$x)`; unpacking into other calls is DEC-299 (ruled, not built)";

/// `Module.name(args)`, recording `Core.<Module>` for the import pass. Called only after every
/// operand lifted, so a failed lift leaves no import behind.
fn module_call(module: &'static str, name: &str, args: Vec<Expr>) -> Expr {
    record_native_module(if module == "Map" {
        "Core.Map"
    } else {
        "Core.List"
    });
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident(module.to_string(), SP)),
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

/// One spread operand. A provably-map operand is refused by name; `array_values($m)` becomes
/// `Map.values(m)`; anything else lifts as itself and the checker decides.
fn lift_operand(e: &php::PhpExpr) -> Result<Expr, String> {
    let is_map_var = matches!(e, php::PhpExpr::Var(v) if super::shapes::is_map_var(v));
    if is_map_var || super::map_consts::is_known_map(e) {
        return Err(
            "lift: a spread of a map has map semantics (string keys become names or \
             overwrite) — DEC-538 lifts a spread of a list only; spread `array_values(…)` for its \
             values"
                .into(),
        );
    }
    if let php::PhpExpr::Call { callee, args } = e {
        if let (php::PhpExpr::Name(n), [m]) = (callee.as_ref(), args.as_slice()) {
            if n == "array_values" && !matches!(m, php::PhpExpr::Spread(_)) {
                let m = lift_expr(m)?;
                return Ok(module_call("Map", "values", vec![m]));
            }
        }
    }
    lift_expr(e)
}

/// `[… ...x …]`: `None` when no element is a spread, so the ordinary literal lift runs.
pub(super) fn lift_spread_array(elems: &[php::PhpArrayElem]) -> Option<Result<Expr, String>> {
    if !elems
        .iter()
        .any(|e| matches!(e.value, php::PhpExpr::Spread(_)))
    {
        return None;
    }
    Some((|| {
        if elems.iter().any(|e| e.key.is_some()) {
            return Err(
                "lift: a spread inside a keyed array literal has map semantics (a later \
                 string key overwrites) — DEC-538 lifts a spread in a list literal only"
                    .into(),
            );
        }
        let mut segments = Vec::new();
        let mut run = Vec::new();
        for e in elems {
            match &e.value {
                php::PhpExpr::Spread(x) => {
                    if !run.is_empty() {
                        segments.push(Expr::List(std::mem::take(&mut run), SP));
                    }
                    segments.push(lift_operand(x)?);
                }
                plain => run.push(lift_expr(plain)?),
            }
        }
        if !run.is_empty() {
            segments.push(Expr::List(run, SP));
        }
        Ok(if segments.len() == 2 {
            module_call("List", "concat", segments)
        } else {
            module_call("List", "flatten", vec![Expr::List(segments, SP)])
        })
    })())
}

/// `array_merge(...$xss)` — exactly one argument, a spread. Every other `array_merge` shape is left
/// to the ordinary call lift, where a spread argument is refused by name.
pub(super) fn lift_spread_merge(
    callee: &php::PhpExpr,
    args: &[php::PhpExpr],
) -> Option<Result<Expr, String>> {
    let (php::PhpExpr::Name(n), [php::PhpExpr::Spread(x)]) = (callee, args) else {
        return None;
    };
    if n != "array_merge" {
        return None;
    }
    Some(lift_operand(x).map(|xs| module_call("List", "flatten", vec![xs])))
}

/// The statement `array_unshift($v, ...$xs);` → `v = List.concat(xs, v);`. Like `usort`, it
/// mutates its first argument by reference, so it has a meaning only as a statement on a plain
/// variable; any other shape falls through and its spread is refused by name.
pub(in crate::lift::lifter) fn lift_unshift_stmt(e: &php::PhpExpr) -> Option<Result<Stmt, String>> {
    let php::PhpExpr::Call { callee, args } = e else {
        return None;
    };
    let (php::PhpExpr::Name(n), [target @ php::PhpExpr::Var(_), php::PhpExpr::Spread(xs)]) =
        (callee.as_ref(), args.as_slice())
    else {
        return None;
    };
    if n != "array_unshift" {
        return None;
    }
    Some((|| {
        let front = lift_operand(xs)?;
        let back = lift_expr(target)?;
        Ok(Stmt::Assign {
            target: lift_expr(target)?,
            value: module_call("List", "concat", vec![front, back]),
            span: SP,
        })
    })())
}

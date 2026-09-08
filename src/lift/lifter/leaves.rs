//! PHP lifter — DECLARATION leaf conversions: an enum, a parameter list, a constructor's promoted
//! parameters and a return type. Split out of `exprs.rs` under Invariant 13 (lane L1c, 2026-09-07);
//! these are the non-expression half its header always named.

use super::*;

pub(super) fn lift_enum(e: &php::PhpEnum) -> Result<EnumDecl, String> {
    // Methods are NOT refused any more: DEC-509 lowers each to a free function reachable by UFCS
    // (`super::enums::lower_methods`), which the item loop emits beside the enum. Phorj enums still
    // carry no methods — that design property is unchanged; what changed is that the lifter now has
    // a faithful target for the PHP idiom instead of a Tier-2 refusal.
    // DEC-302: a PHP backed enum (`enum Suit: string { case Hearts = "H"; }`) lifts to a Phorj
    // backed enum — backing type + per-variant value preserved. Only `int`/`string` back an enum
    // (both PHP and Phorj), and every case of a backed enum carries a value (PHP requires it).
    let backing_type = match &e.backing {
        Some(bt) => {
            let ty = lift_type(bt)?;
            if !matches!(&ty, Type::Named { name, .. } if name == "int" || name == "string") {
                return Err(format!(
                    "lift: enum `{}` backing type must be `int` or `string` (Tier-2)",
                    e.name
                ));
            }
            Some(ty)
        }
        None => None,
    };
    let variants = e
        .cases
        .iter()
        .map(|c| {
            let backing_value = match (&backing_type, &c.value) {
                (Some(_), Some(v)) => Some(Box::new(lift_expr(v)?)),
                (Some(_), None) => {
                    return Err(format!(
                        "lift: backed enum `{}` case `{}` has no value",
                        e.name, c.name
                    ))
                }
                (None, _) => None,
            };
            Ok(EnumVariant {
                name: c.name.clone(),
                fields: Vec::new(),
                backing_value,
                span: SP,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(EnumDecl {
        vis: crate::ast::Visibility::Public,
        name: e.name.clone(),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        backing_type,
        variants,
        injected: false,
        span: SP,
    })
}

pub(super) fn lift_params(params: &[php::PhpParam]) -> Result<Vec<Param>, String> {
    let mut out = Vec::new();
    for p in params {
        // Lane R-5: the default lifts as written; the checker enforces literal-only and
        // trailing-only (`E-DEFAULT-PARAM-EXPR` / `E-DEFAULT-PARAM-ORDER`), so nothing is guessed.
        let default = match &p.default {
            Some(d) => Some(Box::new(lift_expr(d)?)),
            None => None,
        };
        let ty = lift_type(p.ty.as_ref().ok_or_else(|| {
            format!("lift: parameter `{}` has no type (Tier-1 is typed)", p.name)
        })?)?;
        out.push(Param {
            ty,
            name: p.name.clone(),
            default,
            // Lifting PHP `...$x` variadics is a Tier-2 follow-up (DEC-298 lift leg).
            variadic: false,
            span: SP,
        });
    }
    Ok(out)
}

pub(super) fn lift_ctor_params(
    params: &[php::PhpParam],
    readonly_class: bool,
) -> Result<Vec<CtorParam>, String> {
    let mut out = Vec::new();
    for p in params {
        // Lane R-5: a promoted default (`public int $tier = 0`, 22 of scout's 120 files) lifts as
        // written — DEC-236 trailing-only literal defaults on the phorj side.
        let default = match &p.default {
            Some(d) => Some(Box::new(lift_expr(d)?)),
            None => None,
        };
        let ty = lift_type(
            p.ty.as_ref()
                .ok_or_else(|| format!("lift: ctor parameter `{}` has no type", p.name))?,
        )?;
        let mut modifiers = Vec::new();
        if let Some(vis) = p.promotion {
            // A promoted property — mirror PHP's mutability: mutable unless `readonly` on the
            // parameter or on the whole class (PHP 8.2 `readonly class`), in which case phorj's
            // default (immutable) is exactly right and no `mutable` is written.
            modifiers.push(vis_modifier(vis));
            if !(readonly_class || p.is_readonly) {
                modifiers.push(Modifier::Mutable);
            }
        }
        out.push(CtorParam {
            modifiers,
            ty,
            name: p.name.clone(),
            default,
            span: SP,
        });
    }
    Ok(out)
}

/// Lift a function/method's declared return type (C-45). A PHP `: T` lifts directly. **No** hint is
/// the trap: the old code emitted a Phorj function with no return type, which *parses* but fails the
/// checker (Tier-1 requires explicit returns) — a silent non-compiling draft. Instead: if the body
/// never returns a value, the function is provably `void` (a fact from the body, not a guess); if it
/// returns a value we cannot infer the type, so reject loudly rather than emit invalid Phorj.
pub(super) fn lift_ret(
    php_ret: &Option<php::PhpType>,
    body: Option<&[php::PhpStmt]>,
) -> Result<Option<Type>, String> {
    match php_ret {
        Some(t) => Ok(Some(lift_type(t)?)),
        None => match body {
            Some(b) if !body_has_value_return(b) => Ok(Some(named("void"))),
            Some(_) => Err(
                "lift: function has no return type but returns a value — add an explicit return type (Tier-2)"
                    .into(),
            ),
            None => {
                Err("lift: an abstract method with no return type needs an explicit one (Tier-2)".into())
            }
        },
    }
}

//! PHP lifter — expression lifting + leaf conversions (types, ops, params).

use super::*;
use crate::ast::LambdaBody;

// ── expressions (no scope state) ──

pub(super) fn lift_expr(e: &php::PhpExpr) -> Result<Expr, String> {
    Ok(match e {
        php::PhpExpr::Int(n) => Expr::Int(*n, SP),
        php::PhpExpr::Closure { params, ret, body } => Expr::Lambda {
            params: lift_params(params)?,
            ret: ret.as_ref().map(lift_type).transpose()?,
            throws: Vec::new(),
            body: LambdaBody::Expr(Box::new(lift_expr(body)?)),
            span: SP,
        },
        php::PhpExpr::Float(f) => Expr::Float(*f, SP),
        php::PhpExpr::Str(s) => Expr::Str(vec![StrPart::Literal(s.clone())], SP),
        php::PhpExpr::Interp(parts) => {
            let mut out = Vec::with_capacity(parts.len());
            for part in parts {
                out.push(match part {
                    php::PhpStrPart::Lit(s) => StrPart::Literal(s.clone()),
                    php::PhpStrPart::Expr(e) => StrPart::Expr(Box::new(lift_expr(e)?)),
                });
            }
            Expr::Str(out, SP)
        }
        php::PhpExpr::Bool(b) => Expr::Bool(*b, SP),
        php::PhpExpr::Null => Expr::Null(SP),
        php::PhpExpr::Var(name) if name == "this" => Expr::This(SP),
        php::PhpExpr::Var(name) | php::PhpExpr::Name(name) => Expr::Ident(name.clone(), SP),
        php::PhpExpr::Array(elems) => lift_array(elems)?,
        // LIFT-ATTR: `name: value` lifts 1:1 — phorj spells a named argument exactly the same way
        // (DEC-297), so nothing is reordered here. The checker normalizes named args into their
        // positional slots later, and rejects the positions phorj does not support yet, which keeps
        // that judgement in ONE place instead of duplicating it in the lifter.
        php::PhpExpr::NamedArg { name, value } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(lift_expr(value)?),
            span: SP,
        },
        php::PhpExpr::Unary { op, expr } => Expr::Unary {
            op: match op {
                php::PhpUnOp::Not => UnaryOp::Not,
                php::PhpUnOp::Neg => UnaryOp::Neg,
                php::PhpUnOp::BitNot => UnaryOp::BitNot,
            },
            expr: Box::new(lift_expr(expr)?),
            span: SP,
        },
        php::PhpExpr::Binary { op, left, right } => Expr::Binary {
            op: lift_binop(*op)?,
            lhs: Box::new(lift_expr(left)?),
            rhs: Box::new(lift_expr(right)?),
            span: SP,
        },
        // C-46: PHP `value instanceof ClassName` → Phorj's existing `instanceof` (M-RT S1).
        php::PhpExpr::InstanceOf { value, class } => Expr::InstanceOf {
            value: Box::new(lift_expr(value)?),
            type_name: class.clone(),
            span: SP,
        },
        php::PhpExpr::Cast { ty, value } => Expr::Cast {
            value: Box::new(lift_expr(value)?),
            type_name: ty.clone(),
            span: SP,
        },
        php::PhpExpr::EmptyColl(ty) => new_coll(&lift_type(ty)?)?,
        php::PhpExpr::AppendSlot(_) => {
            return Err("lift: `$xs[]` is only meaningful as the target of `=`".into());
        }
        php::PhpExpr::Assign { .. }
        | php::PhpExpr::CompoundAssign { .. }
        | php::PhpExpr::IncDec { .. } => {
            return Err("lift: assignment / `++` / `--` as a sub-expression is Tier-2".into());
        }
        php::PhpExpr::Ternary { cond, then, els } => {
            let then = then
                .as_ref()
                .ok_or("lift: elvis `?:` is Tier-2 (use a full ternary)")?;
            Expr::If {
                cond: Box::new(lift_expr(cond)?),
                then_expr: Box::new(lift_expr(then)?),
                else_expr: Box::new(lift_expr(els)?),
                span: SP,
            }
        }
        php::PhpExpr::Call { callee, args } => {
            // DEC-312: a bare PHP builtin with a registered inverse lifts to its Core form
            // (`strlen($s)` → `String.length(s)`, one registry row for both directions). Arity must
            // match the native's signature — a mismatched call falls through to the plain unresolved
            // lift (loud later, never a wrong guess).
            if let php::PhpExpr::Name(n) = &**callee {
                if let Some(nat) = crate::native::lift_of(n) {
                    if nat.params.len() == args.len() {
                        super::record_native_module(nat.module);
                        let mut lifted = lift_exprs(args)?;
                        // DEC-326: the RECEIVER form is the canonical style — a subject-first
                        // native lifts to `subject.name(rest…)` (UFCS erases it back to the module
                        // call pre-backend); a zero-arg native keeps the module form.
                        let (object, rest) = if lifted.is_empty() {
                            let q = nat.module.rsplit('.').next().expect("dotted module");
                            (Expr::Ident(q.to_string(), SP), Vec::new())
                        } else {
                            let recv = lifted.remove(0);
                            (recv, lifted)
                        };
                        return Ok(Expr::Call {
                            callee: Box::new(Expr::Member {
                                object: Box::new(object),
                                name: nat.name.to_string(),
                                safe: false,
                                sep: crate::ast::MemberSep::Dot,
                                span: SP,
                            }),
                            args: rest,
                            type_args: Vec::new(),
                            span: SP,
                        });
                    }
                }
            }
            Expr::Call {
                callee: Box::new(lift_expr(callee)?),
                args: lift_exprs(args)?,
                type_args: Vec::new(),
                span: SP,
            }
        }
        php::PhpExpr::MethodCall {
            recv,
            name,
            args,
            nullsafe,
        } => Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(lift_expr(recv)?),
                name: name.clone(),
                safe: *nullsafe,
                // PHP instance call `->`/`?->` (DEC-207).
                sep: crate::ast::MemberSep::Dot,
                span: SP,
            }),
            args: lift_exprs(args)?,
            type_args: Vec::new(),
            span: SP,
        },
        php::PhpExpr::Member {
            recv,
            name,
            nullsafe,
        } => Expr::Member {
            object: Box::new(lift_expr(recv)?),
            name: name.clone(),
            safe: *nullsafe,
            // PHP instance property `->`/`?->` (DEC-207).
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        },
        php::PhpExpr::StaticCall { class, name, args } => Expr::Call {
            callee: Box::new(static_member(class, name)),
            args: lift_exprs(args)?,
            type_args: Vec::new(),
            span: SP,
        },
        php::PhpExpr::ClassConst { class, name } | php::PhpExpr::StaticProp { class, name } => {
            static_member(class, name)
        }
        php::PhpExpr::Index { base, index } => Expr::Index {
            object: Box::new(lift_expr(base)?),
            index: Box::new(lift_expr(index)?),
            span: SP,
        },
        php::PhpExpr::New { class, args } => Expr::New(
            Box::new(Expr::Call {
                // Two rules in one call (`phorj_error_name`): a PHP builtin EXCEPTION maps onto phorj's
                // standard taxonomy (DEC-421 — `\RuntimeException` → `RuntimeError`), and anything else
                // just loses PHP's root marker, which phorj has no spelling for. Mapping only the CATCH
                // clause and not the `new` left `throw new RuntimeException(…)` naming a type that does
                // not exist, so the draft still failed to check.
                callee: Box::new(Expr::Ident(
                    crate::lift::lifter::exceptions::phorj_error_name(class),
                    SP,
                )),
                args: lift_exprs(args)?,
                type_args: Vec::new(),
                span: SP,
            }),
            SP,
        ),
        php::PhpExpr::Match { subject, arms } => lift_match(subject, arms)?,
    })
}

pub(super) fn lift_exprs(es: &[php::PhpExpr]) -> Result<Vec<Expr>, String> {
    es.iter().map(lift_expr).collect()
}

pub(super) fn lift_array(elems: &[php::PhpArrayElem]) -> Result<Expr, String> {
    if elems.is_empty() {
        return Ok(Expr::List(Vec::new(), SP));
    }
    let any_key = elems.iter().any(|e| e.key.is_some());
    let all_key = elems.iter().all(|e| e.key.is_some());
    if any_key && !all_key {
        return Err("lift: a mixed keyed/positional array is Tier-2".into());
    }
    if all_key {
        let mut pairs = Vec::new();
        for e in elems {
            pairs.push((lift_expr(e.key.as_ref().unwrap())?, lift_expr(&e.value)?));
        }
        Ok(Expr::Map(pairs, SP))
    } else {
        let items: Result<Vec<_>, _> = elems.iter().map(|e| lift_expr(&e.value)).collect();
        Ok(Expr::List(items?, SP))
    }
}

pub(super) fn lift_match(
    subject: &php::PhpExpr,
    arms: &[php::PhpMatchArm],
) -> Result<Expr, String> {
    let mut out = Vec::new();
    for arm in arms {
        match &arm.conds {
            None => out.push(MatchArm {
                pattern: Pattern::Wildcard(SP),
                guard: None,
                body: lift_expr(&arm.body)?,
                span: SP,
            }),
            Some(conds) => {
                // PHP shares one body across comma-separated conditions; Phorj has one pattern per
                // arm, so duplicate the (cloned) body per literal condition.
                let body = lift_expr(&arm.body)?;
                for c in conds {
                    out.push(MatchArm {
                        pattern: literal_pattern(c)?,
                        guard: None,
                        body: body.clone(),
                        span: SP,
                    });
                }
            }
        }
    }
    Ok(Expr::Match {
        scrutinee: Box::new(lift_expr(subject)?),
        arms: out,
        span: SP,
    })
}

/// A PHP `match` condition must be a literal to become a Phorj pattern (a non-literal arm compares
/// by `===` at runtime — no pattern equivalent, so it's a loud Tier-2 error).
pub(super) fn literal_pattern(e: &php::PhpExpr) -> Result<Pattern, String> {
    Ok(match e {
        php::PhpExpr::Int(n) => Pattern::Int(*n, SP),
        php::PhpExpr::Float(f) => Pattern::Float(*f, SP),
        php::PhpExpr::Str(s) => Pattern::Str(s.clone(), SP),
        php::PhpExpr::Bool(b) => Pattern::Bool(*b, SP),
        php::PhpExpr::Null => Pattern::Null(SP),
        _ => return Err("lift: a `match` arm with a non-literal condition is Tier-2".into()),
    })
}

// ── enums + types + small helpers ──

pub(super) fn lift_enum(e: &php::PhpEnum) -> Result<EnumDecl, String> {
    if !e.methods.is_empty() {
        return Err(format!(
            "lift: enum `{}` has methods — Phorj enums carry no methods (Tier-2)",
            e.name
        ));
    }
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

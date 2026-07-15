//! PHP lifter — expression lifting + leaf conversions (types, ops, params).

use super::*;

// ── expressions (no scope state) ──

pub(super) fn lift_expr(e: &php::PhpExpr) -> Result<Expr, String> {
    Ok(match e {
        php::PhpExpr::Int(n) => Expr::Int(*n, SP),
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
        php::PhpExpr::Call { callee, args } => Expr::Call {
            callee: Box::new(lift_expr(callee)?),
            args: lift_exprs(args)?,
            type_args: Vec::new(),
            span: SP,
        },
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
                callee: Box::new(Expr::Ident(class.clone(), SP)),
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
    if e.backing.is_some() {
        return Err(format!(
            "lift: backed enum `{}` (cases with scalar values) has no Phorj equivalent (Tier-2)",
            e.name
        ));
    }
    if !e.methods.is_empty() {
        return Err(format!(
            "lift: enum `{}` has methods — Phorj enums carry no methods (Tier-2)",
            e.name
        ));
    }
    let variants = e
        .cases
        .iter()
        .map(|c| EnumVariant {
            name: c.name.clone(),
            fields: Vec::new(),
            span: SP,
        })
        .collect();
    Ok(EnumDecl {
        vis: crate::ast::Visibility::Public,
        name: e.name.clone(),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        variants,
        injected: false,
        span: SP,
    })
}

pub(super) fn lift_params(params: &[php::PhpParam]) -> Result<Vec<Param>, String> {
    let mut out = Vec::new();
    for p in params {
        if p.default.is_some() {
            return Err(format!(
                "lift: default value on parameter `{}` is Tier-2 (Wave 2)",
                p.name
            ));
        }
        let ty = lift_type(p.ty.as_ref().ok_or_else(|| {
            format!("lift: parameter `{}` has no type (Tier-1 is typed)", p.name)
        })?)?;
        out.push(Param {
            ty,
            name: p.name.clone(),
            // Lifting a PHP default parameter is a Tier-2 follow-up; Tier-1 params have no default.
            default: None,
            span: SP,
        });
    }
    Ok(out)
}

pub(super) fn lift_ctor_params(params: &[php::PhpParam]) -> Result<Vec<CtorParam>, String> {
    let mut out = Vec::new();
    for p in params {
        if p.default.is_some() {
            return Err(format!(
                "lift: default value on constructor parameter `{}` is Tier-2 (Wave 2)",
                p.name
            ));
        }
        let ty = lift_type(
            p.ty.as_ref()
                .ok_or_else(|| format!("lift: ctor parameter `{}` has no type", p.name))?,
        )?;
        let mut modifiers = Vec::new();
        if let Some(vis) = p.promotion {
            // A promoted property — mirror PHP's mutability (promoted props are mutable).
            modifiers.push(vis_modifier(vis));
            modifiers.push(Modifier::Mutable);
        }
        out.push(CtorParam {
            modifiers,
            ty,
            name: p.name.clone(),
            // The lift draft never synthesizes defaults (PHP promoted defaults are a lift TODO).
            default: None,
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

/// Does any path in `body` `return` a *value* (`return expr;`)? Recurses through nested control flow
/// and blocks. A bare `return;` (and `echo`/`break`/`continue`/expr statements) do not count.
pub(super) fn body_has_value_return(body: &[php::PhpStmt]) -> bool {
    use php::PhpStmt::{Block, Echo, Expr, For, Foreach, If, Return, While};
    body.iter().any(|s| match s {
        Return(opt) => opt.is_some(),
        If {
            then, elifs, els, ..
        } => {
            body_has_value_return(then)
                || elifs.iter().any(|(_, b)| body_has_value_return(b))
                || els.as_deref().is_some_and(body_has_value_return)
        }
        While { body, .. } | For { body, .. } | Foreach { body, .. } | Block(body) => {
            body_has_value_return(body)
        }
        Expr(_) | Echo(_) | php::PhpStmt::Break | php::PhpStmt::Continue => false,
    })
}

pub(super) fn lift_type(t: &php::PhpType) -> Result<Type, String> {
    match t {
        php::PhpType::Named(name) => match name.as_str() {
            "int" | "float" | "string" | "bool" | "void" => Ok(named(name)),
            "array" => Err("lift: an `array` type needs List/Map/Set inference (Tier-2)".into()),
            "mixed" | "iterable" | "object" | "callable" | "self" | "static" | "parent" => {
                Err(format!("lift: the `{name}` type is Tier-2/Tier-3"))
            }
            // A class/enum/interface name.
            _ => Ok(named(name)),
        },
        php::PhpType::Nullable(inner) => Ok(Type::Optional {
            inner: Box::new(lift_type(inner)?),
            span: SP,
        }),
    }
}

pub(super) fn lift_binop(op: php::PhpBinOp) -> Result<BinaryOp, String> {
    use php::PhpBinOp as P;
    Ok(match op {
        P::Add => BinaryOp::Add,
        P::Sub => BinaryOp::Sub,
        P::Mul => BinaryOp::Mul,
        P::Div => BinaryOp::Div,
        P::Rem => BinaryOp::Rem,
        // PHP string concatenation `.` is Phorj's type-directed `+`.
        P::Concat => BinaryOp::Add,
        // Phorj is statically typed, so loose and strict equality coincide.
        P::Eq | P::Identical => BinaryOp::Eq,
        P::NotEq | P::NotIdentical => BinaryOp::NotEq,
        P::Lt => BinaryOp::Lt,
        P::Le => BinaryOp::Le,
        P::Gt => BinaryOp::Gt,
        P::Ge => BinaryOp::Ge,
        P::And => BinaryOp::And,
        P::Or => BinaryOp::Or,
        P::Coalesce => BinaryOp::Coalesce,
        // C-47: bitwise / shift map 1:1 to Phorj's existing operators (PHP-identical int semantics).
        P::BitAnd => BinaryOp::BitAnd,
        P::BitOr => BinaryOp::BitOr,
        P::BitXor => BinaryOp::BitXor,
        P::Shl => BinaryOp::Shl,
        P::Shr => BinaryOp::Shr,
    })
}

/// A static-member access `Class.name` (covers `Class::CONST`, `Class::$prop`, and the callee of
/// `Class::method(...)`).
pub(super) fn static_member(class: &str, name: &str) -> Expr {
    Expr::Member {
        object: Box::new(Expr::Ident(class.to_string(), SP)),
        name: name.to_string(),
        safe: false,
        // PHP `::` static access (DEC-207) — round-trips back to `::`.
        sep: crate::ast::MemberSep::ColonColon,
        span: SP,
    }
}

/// `Output.print(arg)` — the lift target of a PHP `echo`.
pub(super) fn console_print(arg: Expr) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident("Output".into(), SP)),
            name: "print".into(),
            safe: false,
            // Synthesized echo target, not lifted from a PHP `::` (DEC-207).
            sep: crate::ast::MemberSep::Dot,
            span: SP,
        }),
        args: vec![arg],
        type_args: Vec::new(),
        span: SP,
    }
}

pub(super) fn vis_modifier(v: php::PhpVisibility) -> Modifier {
    match v {
        php::PhpVisibility::Public => Modifier::Public,
        php::PhpVisibility::Private => Modifier::Private,
        php::PhpVisibility::Protected => Modifier::Protected,
    }
}

/// The Phorj type-name for a literal expression (used to type a lifted class `const`).
pub(super) fn lit_type(e: &php::PhpExpr) -> Option<&'static str> {
    match e {
        php::PhpExpr::Int(_) => Some("int"),
        php::PhpExpr::Float(_) => Some("float"),
        php::PhpExpr::Str(_) => Some("string"),
        php::PhpExpr::Bool(_) => Some("bool"),
        _ => None,
    }
}

pub(super) fn named(name: &str) -> Type {
    Type::Named {
        name: name.to_string(),
        args: Vec::new(),
        span: SP,
    }
}

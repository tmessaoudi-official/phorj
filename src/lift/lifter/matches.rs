//! PHP lifter — the `match` cluster: a PHP `match` expression and the literal patterns its arms
//! carry. Split out of `exprs.rs` under Invariant 13 (lane L1c, 2026-09-07).

use super::*;

pub(super) fn lift_match(
    subject: &php::PhpExpr,
    arms: &[php::PhpMatchArm],
) -> Result<Expr, String> {
    if matches!(subject, php::PhpExpr::Bool(true)) {
        return lift_match_true(arms);
    }
    let mut out = Vec::new();
    for arm in arms {
        match &arm.conds {
            None => out.push(MatchArm {
                pattern: Pattern::Wildcard(SP),
                guard: None,
                body: super::throw_expr::lift_throwable(&arm.body)?,
                span: SP,
            }),
            Some(conds) => {
                // PHP shares one body across comma-separated conditions; Phorj has one pattern per
                // arm, so duplicate the (cloned) body per literal condition.
                let body = super::throw_expr::lift_throwable(&arm.body)?;
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

/// Row 4d — `match (true) { c1, c2 => v, … default => d }` is PHP's guard chain: the first condition
/// that is `=== true`, in source order, selects its arm. phorj's form is an if-/else-if chain, and a
/// comma list is `c1 || c2`, which short-circuits exactly as PHP stops at the first match. `default`
/// may be written anywhere and is always the fallback, so it becomes the final `else`. With no
/// `default` PHP throws `UnhandledMatchError` and an if-expression has no `else`: refused by name.
fn lift_match_true(arms: &[php::PhpMatchArm]) -> Result<Expr, String> {
    let mut fallback = None;
    let mut guarded = Vec::new();
    for arm in arms {
        let body = super::throw_expr::lift_throwable(&arm.body)?;
        match &arm.conds {
            None => fallback = Some(body),
            Some(conds) => {
                let mut cond: Option<Expr> = None;
                for c in conds {
                    let e = lift_expr(c)?;
                    cond = Some(match cond {
                        None => e,
                        Some(l) => Expr::Binary {
                            op: BinaryOp::Or,
                            lhs: Box::new(l),
                            rhs: Box::new(e),
                            span: SP,
                        },
                    });
                }
                guarded.push((cond.ok_or("lift: a `match` arm with no condition")?, body));
            }
        }
    }
    let mut chain = fallback.ok_or(
        "lift: a `match (true)` with no `default` arm has no phorj form — PHP throws \
         `UnhandledMatchError` when no condition holds, and an if-chain needs an `else`; add a \
         `default` arm",
    )?;
    for (cond, body) in guarded.into_iter().rev() {
        chain = Expr::If {
            cond: Box::new(cond),
            then_expr: Box::new(body),
            else_expr: Box::new(chain),
            span: SP,
        };
    }
    Ok(chain)
}

/// A PHP `match` condition must be a literal to become a Phorj pattern (a non-literal arm compares
/// by `===` at runtime — no pattern equivalent, so it's a loud Tier-2 error).
/// Flatten a left-nested `.` chain into interpolation parts, in source order.
///
pub(super) fn literal_pattern(e: &php::PhpExpr) -> Result<Pattern, String> {
    Ok(match e {
        php::PhpExpr::Int(n) => Pattern::Int(*n, SP),
        php::PhpExpr::Float(f) => Pattern::Float(*f, SP),
        php::PhpExpr::Str(s) => Pattern::Str(s.clone(), SP),
        php::PhpExpr::Bool(b) => Pattern::Bool(*b, SP),
        php::PhpExpr::Null => Pattern::Null(SP),
        // `match ($t) { Tenure::PLAI => …, default => … }` — an ENUM CASE in pattern position is a
        // variant pattern, not a literal (lane L1). It carries its enum as the qualifier, which the
        // checker validates against the scrutinee, so a case from the WRONG enum is still an error
        // rather than a silently-never-matching arm. A class constant in pattern position keeps its
        // Tier-2 refusal below: `Limits::MAX => …` is a value comparison PHP allows and phorj's
        // pattern grammar has no form for.
        php::PhpExpr::ClassConst { class, name } if super::enums::is_enum(class) => {
            Pattern::Variant {
                name: name.clone(),
                fields: Vec::new(),
                enum_qualifier: Some(class.rsplit('\\').next().unwrap_or(class).to_string()),
                span: SP,
            }
        }
        _ => return Err("lift: a `match` arm with a non-literal condition is Tier-2".into()),
    })
}

// ── enums + types + small helpers ──

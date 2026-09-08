//! PHP lifter — the `match` cluster: a PHP `match` expression and the literal patterns its arms
//! carry. Split out of `exprs.rs` under Invariant 13 (lane L1c, 2026-09-07).

use super::*;

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

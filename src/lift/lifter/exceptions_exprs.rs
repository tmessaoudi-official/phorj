//! PHP lifter — the EXPRESSION half of the exception-site walk (`exceptions.rs`). PHP 8's
//! throw-expression (DEC-532) puts a `throw new X(…)` inside an expression — `return $v ?? throw
//! new \RuntimeException(…)` — where a statement-only walk never looks, so the mapped `RuntimeError`
//! was printed with no import. Exhaustive over `PhpExpr` (Invariant 3): no `_` arm.

use super::exceptions::visit_body;
use crate::lift::ast as php;

pub(super) fn visit_expr(e: &php::PhpExpr, f: &mut impl FnMut(&str)) {
    use php::PhpExpr as E;
    match e {
        E::Throw(inner) => {
            if let E::New { class, .. } = &**inner {
                f(class);
            }
            visit_expr(inner, f);
        }
        E::BlockClosure { body, .. } => visit_body(body, f),
        E::Closure { body, .. } => visit_expr(body, f),
        E::Interp(parts) => {
            for p in parts {
                if let php::PhpStrPart::Expr(x) = p {
                    visit_expr(x, f);
                }
            }
        }
        E::Array(elems) => {
            for el in elems {
                if let Some(k) = &el.key {
                    visit_expr(k, f);
                }
                visit_expr(&el.value, f);
            }
        }
        E::Destructure { value, .. }
        | E::NamedArg { value, .. }
        | E::Cast { value, .. }
        | E::Declared { value, .. }
        | E::InstanceOf { value, .. } => visit_expr(value, f),
        E::Unary { expr, .. } | E::Spread(expr) => visit_expr(expr, f),
        E::AppendSlot(x) => visit_expr(x, f),
        E::Binary { left, right, .. } => {
            visit_expr(left, f);
            visit_expr(right, f);
        }
        E::Assign { target, value } | E::CompoundAssign { target, value, .. } => {
            visit_expr(target, f);
            visit_expr(value, f);
        }
        E::IncDec { target, .. } => visit_expr(target, f),
        E::Ternary { cond, then, els } => {
            visit_expr(cond, f);
            if let Some(t) = then {
                visit_expr(t, f);
            }
            visit_expr(els, f);
        }
        E::Call { callee, args } => {
            visit_expr(callee, f);
            visit_all(args, f);
        }
        E::MethodCall { recv, args, .. } => {
            visit_expr(recv, f);
            visit_all(args, f);
        }
        E::Member { recv, .. } => visit_expr(recv, f),
        E::StaticCall { args, .. } | E::New { args, .. } => visit_all(args, f),
        E::Index { base, index } => {
            visit_expr(base, f);
            visit_expr(index, f);
        }
        E::Match { subject, arms } => {
            visit_expr(subject, f);
            for arm in arms {
                if let Some(conds) = &arm.conds {
                    visit_all(conds, f);
                }
                visit_expr(&arm.body, f);
            }
        }
        E::Int(_)
        | E::Float(_)
        | E::Str(_)
        | E::Bool(_)
        | E::Null
        | E::Var(_)
        | E::Name(_)
        | E::EmptyColl(_)
        | E::ClassConst { .. }
        | E::StaticProp { .. } => {}
    }
}

fn visit_all(es: &[php::PhpExpr], f: &mut impl FnMut(&str)) {
    for e in es {
        visit_expr(e, f);
    }
}

//! The total walks over the PHP AST behind [`super::rename`] (split out under Invariant 13). Every
//! variable-name site — `Var`, parameters, `foreach` key/value, `catch` variables, destructure
//! binders, closure parameters — reaches the visitor; no `_` arm, so a new AST variant cannot be
//! skipped silently.

use super::{Site, Sites};
use crate::lift::ast as php;

/// The default values of `params` (their NAMES are the caller's business — promoted ones differ).
pub(super) fn walk_params(params: &mut [php::PhpParam], f: &mut Sites) {
    for p in params {
        if let Some(d) = &mut p.default {
            walk_expr(d, f);
        }
    }
}

/// A closure's own parameters are variables of the enclosing scope's namespace for renaming: the
/// name is a pure function of the word, so a capture and its source always agree.
fn walk_closure_params(params: &mut [php::PhpParam], f: &mut Sites) {
    for p in params.iter_mut() {
        f(&mut p.name, Site::Var);
    }
    walk_params(params, f);
}

pub(super) fn walk_stmts(stmts: &mut [php::PhpStmt], f: &mut Sites) {
    for s in stmts {
        walk_stmt(s, f);
    }
}

fn walk_stmt(s: &mut php::PhpStmt, f: &mut Sites) {
    use php::PhpStmt as S;
    match s {
        S::Return(e) => {
            if let Some(e) = e {
                walk_expr(e, f);
            }
        }
        S::Expr(e) | S::Throw(e) => walk_expr(e, f),
        S::If {
            cond,
            then,
            elifs,
            els,
        } => {
            walk_expr(cond, f);
            walk_stmts(then, f);
            for (c, b) in elifs {
                walk_expr(c, f);
                walk_stmts(b, f);
            }
            if let Some(b) = els {
                walk_stmts(b, f);
            }
        }
        S::While { cond, body } => {
            walk_expr(cond, f);
            walk_stmts(body, f);
        }
        S::For {
            init,
            cond,
            step,
            body,
        } => {
            for e in [init, cond, step].into_iter().flatten() {
                walk_expr(e, f);
            }
            walk_stmts(body, f);
        }
        S::Foreach {
            array,
            key,
            value,
            body,
        } => {
            walk_expr(array, f);
            if let Some(k) = key {
                f(k, Site::Var);
            }
            f(value, Site::Var);
            walk_stmts(body, f);
        }
        S::Echo(es) => {
            for e in es {
                walk_expr(e, f);
            }
        }
        S::Break | S::Continue => {}
        S::Block(b) => walk_stmts(b, f),
        S::Try {
            body,
            catches,
            finally_block,
        } => {
            walk_stmts(body, f);
            for c in catches {
                if let Some(v) = &mut c.var {
                    f(v, Site::Var);
                }
                walk_stmts(&mut c.body, f);
            }
            if let Some(b) = finally_block {
                walk_stmts(b, f);
            }
        }
    }
}

/// Call arguments: a named argument follows its parameter's renaming, except on `new` (see the
/// module doc — a reserved-word constructor parameter can only be a promoted field).
fn walk_args(args: &mut [php::PhpExpr], is_new: bool, f: &mut Sites) {
    for a in args {
        if let php::PhpExpr::NamedArg { name, value } = a {
            if !is_new {
                f(name, Site::NamedArg);
            }
            walk_expr(value, f);
        } else {
            walk_expr(a, f);
        }
    }
}

fn walk_expr(e: &mut php::PhpExpr, f: &mut Sites) {
    use php::PhpExpr as E;
    match e {
        E::Int(_)
        | E::Float(_)
        | E::Str(_)
        | E::Bool(_)
        | E::Null
        | E::Name(_)
        | E::EmptyColl(_)
        | E::ClassConst { .. }
        | E::StaticProp { .. } => {}
        E::Var(n) => f(n, Site::Var),
        E::Interp(parts) => {
            for p in parts {
                match p {
                    php::PhpStrPart::Lit(_) => {}
                    php::PhpStrPart::Expr(e) => walk_expr(e, f),
                }
            }
        }
        E::Array(elems) => {
            for el in elems {
                if let Some(k) = &mut el.key {
                    walk_expr(k, f);
                }
                walk_expr(&mut el.value, f);
            }
        }
        E::Destructure { binders, value } => {
            for b in binders {
                f(b, Site::Var);
            }
            walk_expr(value, f);
        }
        // Outside an argument list (the parser only builds one inside), the value is still walked.
        E::NamedArg { value, .. } | E::Spread(value) => walk_expr(value, f),
        E::Unary { expr, .. } => walk_expr(expr, f),
        E::Binary { left, right, .. } => {
            walk_expr(left, f);
            walk_expr(right, f);
        }
        E::Closure { params, body, .. } => {
            walk_closure_params(params, f);
            walk_expr(body, f);
        }
        E::BlockClosure { params, body, .. } => {
            walk_closure_params(params, f);
            walk_stmts(body, f);
        }
        E::AppendSlot(e) => walk_expr(e, f),
        E::Cast { value, .. } | E::InstanceOf { value, .. } | E::Declared { value, .. } => {
            walk_expr(value, f)
        }
        E::Assign { target, value } | E::CompoundAssign { target, value, .. } => {
            walk_expr(target, f);
            walk_expr(value, f);
        }
        E::IncDec { target, .. } => walk_expr(target, f),
        E::Throw(value) => walk_expr(value, f),
        E::Ternary { cond, then, els } => {
            walk_expr(cond, f);
            if let Some(t) = then {
                walk_expr(t, f);
            }
            walk_expr(els, f);
        }
        E::Call { callee, args } => {
            walk_expr(callee, f);
            walk_args(args, false, f);
        }
        E::MethodCall { recv, args, .. } => {
            walk_expr(recv, f);
            walk_args(args, false, f);
        }
        E::Member { recv, .. } => walk_expr(recv, f),
        E::StaticCall { args, .. } => walk_args(args, false, f),
        E::Index { base, index } => {
            walk_expr(base, f);
            walk_expr(index, f);
        }
        E::New { args, .. } => walk_args(args, true, f),
        E::Match { subject, arms } => {
            walk_expr(subject, f);
            for a in arms {
                if let Some(cs) = &mut a.conds {
                    for c in cs {
                        walk_expr(c, f);
                    }
                }
                walk_expr(&mut a.body, f);
            }
        }
    }
}

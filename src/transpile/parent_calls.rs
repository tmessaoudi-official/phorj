//! PHP transpiler — the `parent.m(…)` / `parent(A).m(…)` scan behind trait-aliased MI emission
//! (moved out of `classes_synth.rs`, which sat at its size-gate ceiling).

use super::*;

// ---------------------------------------------------------------------------------------------------
// B2 — `parent.m(…)` / `parent(A).m(…)` collection (for trait-aliased MI emission). A read-only walk
// over every expression position in a class's method/constructor/hook bodies, mirroring the complete
// `checker::rewrite_new` walker so no parent call is missed. Returns each call's
// `(ancestor-as-written, method)`; constructor (`parent.constructor`) calls are already inlined out by
// the front-end before transpilation, so only method calls remain.
// ---------------------------------------------------------------------------------------------------

pub(super) fn collect_parent_method_calls(c: &ClassDecl) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    for m in &c.members {
        match m {
            ClassMember::Method(f) => pc_block(&f.body, &mut out),
            ClassMember::Constructor { body, .. } => pc_block(body, &mut out),
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    pc_expr(g, &mut out);
                }
                if let Some((_, b)) = set {
                    pc_block(b, &mut out);
                }
            }
            ClassMember::Field { .. } => {} // `parent` is rejected in a field initializer (checker)
        }
    }
    out
}

fn pc_block(stmts: &[Stmt], out: &mut Vec<(Option<String>, String)>) {
    for s in stmts {
        pc_stmt(s, out);
    }
}

fn pc_stmt(s: &Stmt, out: &mut Vec<(Option<String>, String)>) {
    match s {
        Stmt::VarDecl { init, .. } => pc_expr(init, out),
        Stmt::Assign { target, value, .. } => {
            pc_expr(target, out);
            pc_expr(value, out);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                pc_expr(e, out);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            pc_expr(cond, out);
            pc_block(then_block, out);
            if let Some(b) = else_block {
                pc_block(b, out);
            }
        }
        Stmt::For { iter: e, body, .. } | Stmt::Using { init: e, body, .. } => {
            pc_expr(e, out);
            pc_block(body, out);
        }
        Stmt::While { cond, body, .. } => {
            pc_expr(cond, out);
            pc_block(body, out);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                pc_stmt(i, out);
            }
            if let Some(co) = cond {
                pc_expr(co, out);
            }
            if let Some(st) = step {
                pc_stmt(st, out);
            }
            pc_block(body, out);
        }
        Stmt::Block(b, _) => pc_block(b, out),
        Stmt::Destructure {
            init, else_block, ..
        } => {
            pc_expr(init, out);
            if let Some(eb) = else_block {
                pc_block(eb, out);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => pc_expr(e, out),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            pc_block(body, out);
            for CatchClause { body, .. } in catches {
                pc_block(body, out);
            }
            if let Some(fb) = finally_block {
                pc_block(fb, out);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn pc_expr(e: &Expr, out: &mut Vec<(Option<String>, String)>) {
    match e {
        Expr::ParentCall {
            ancestor,
            method,
            args,
            ..
        } => {
            out.push((ancestor.clone(), method.clone()));
            for a in args {
                pc_expr(a, out);
            }
        }
        Expr::Unary { expr, .. } => pc_expr(expr, out),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => pc_expr(inner, out),
        Expr::Binary { lhs, rhs, .. } => {
            pc_expr(lhs, out);
            pc_expr(rhs, out);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => pc_expr(value, out),
        Expr::Call { callee, args, .. } => {
            pc_expr(callee, out);
            for a in args {
                pc_expr(a, out);
            }
        }
        Expr::OverloadSelect { call, .. } => pc_expr(call, out),
        Expr::Member { object, .. } => pc_expr(object, out),
        Expr::Index { object, index, .. } => {
            pc_expr(object, out);
            pc_expr(index, out);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    pc_expr(x, out);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                pc_expr(x, out);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                pc_expr(k, out);
                pc_expr(v, out);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            pc_expr(scrutinee, out);
            for MatchArm { guard, body, .. } in arms {
                if let Some(g) = guard {
                    pc_expr(g, out);
                }
                pc_expr(body, out);
            }
        }
        Expr::Range { start, end, .. } => {
            pc_expr(start, out);
            pc_expr(end, out);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            pc_expr(cond, out);
            pc_expr(then_expr, out);
            pc_expr(else_expr, out);
        }
        Expr::Throw { value, .. } => pc_expr(value, out),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => pc_expr(x, out),
            LambdaBody::Block(b) => pc_block(b, out),
        },
        Expr::CloneWith { object, fields, .. } => {
            pc_expr(object, out);
            for (_, v) in fields {
                pc_expr(v, out);
            }
        }
        Expr::New(inner, _) => pc_expr(inner, out),
        Expr::Tuple(xs, ..) => {
            for x in xs {
                pc_expr(x, out);
            }
        }
        Expr::NamedArg { value, .. } => pc_expr(value, out),
        Expr::Spawn { call, .. } => pc_expr(call, out),
        Expr::TaggedTemplate { parts, .. } => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    pc_expr(x, out);
                }
            }
        }
        Expr::Pipe { lhs, rhs, .. } => {
            pc_expr(lhs, out);
            pc_expr(rhs, out);
        }
        // Total (DEC-356): no catch-all, so a new `Expr` variant must be placed here by rustc rather
        // than silently skipped. These carry no nested `Expr`.
        crate::expr_leaves!() | Expr::NewColl { .. } | Expr::Inject { .. } => {}
    }
}

//! DEC-257 — foreach-over-Iterator lowering: rewrite each `for (T x in it)` whose iterable the
//! checker proved to be an `Iterator<T>` implementor (recorded in `for_iter_lowerings`, keyed by
//! the `Stmt::For` span start) into the hasNext/next while-pull:
//!
//! ```text
//! { var __for_it_<start> = <iter>; while (__for_it_<start>.hasNext()) { T x = __for_it_<start>.next(); <body> } }
//! ```
//!
//! Pre-backend (invariant 5 discipline): the interpreter, VM, and transpiler never see a
//! foreach-over-Iterator — all three run the identical lowered block, so byte-identity holds by
//! construction. The generated pulls are BARE calls: `?` is a checker-only marker (the runtime
//! unwind of a thrown fault is the same either way), and `check_for` already discharged the
//! concrete `hasNext`/`next` throws at the loop site (the ruled auto-propagation). The loop
//! variable is span-derived (`__for_it_<start>`), so nested lowered loops never collide.

use crate::ast::{
    CatchClause, ClassMember, Expr, Item, LambdaBody, MemberSep, Program, Stmt, Type,
};
use crate::token::Span;
use std::collections::HashSet;

/// Lower every recorded foreach-over-Iterator in the program. A no-op (identity) when the checker
/// recorded none — the common case.
pub fn lower_foreach_iter(mut program: Program, spans: &HashSet<usize>) -> Program {
    if spans.is_empty() {
        return program;
    }
    for item in &mut program.items {
        match item {
            Item::Function(f) => lower_block(&mut f.body, spans),
            Item::Class(c) => lower_members(&mut c.members, spans),
            Item::Trait(t) => lower_members(&mut t.members, spans),
            Item::Test { body, .. } => lower_block(body, spans),
            _ => {}
        }
    }
    // A `for` can also live inside a lambda's BLOCK body anywhere in expression position; the
    // shared expression walker visits every expr (including inside already-lowered loop bodies),
    // and `lower_block` is idempotent (a lowered loop is a While — no `For` span remains).
    super::rewrite_pipe::walk::visit_exprs_mut(&mut program, &mut |e| {
        if let Expr::Lambda {
            body: LambdaBody::Block(stmts),
            ..
        } = e
        {
            lower_block(stmts, spans);
        }
    });
    program
}

fn lower_members(members: &mut [ClassMember], spans: &HashSet<usize>) {
    for m in members {
        match m {
            ClassMember::Method(f) => lower_block(&mut f.body, spans),
            ClassMember::Constructor { body, .. } => lower_block(body, spans),
            ClassMember::Hook { set, .. } => {
                if let Some((_, body)) = set {
                    lower_block(body, spans);
                }
            }
            ClassMember::Field { .. } => {}
        }
    }
}

fn lower_block(stmts: &mut [Stmt], spans: &HashSet<usize>) {
    for s in stmts.iter_mut() {
        lower_stmt(s, spans);
    }
}

/// A `__for_it_<start>.<method>()` pull call — the shape a user would hand-write, so every
/// backend resolves it through the ordinary method-call path.
fn pull(it_name: &str, method: &str, span: Span) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident(it_name.to_string(), span)),
            name: method.to_string(),
            safe: false,
            sep: MemberSep::Dot,
            span,
        }),
        args: Vec::new(),
        type_args: Vec::new(),
        span,
    }
}

fn lower_stmt(s: &mut Stmt, spans: &HashSet<usize>) {
    // The recorded case: replace the `For` with its lowered block. Take ownership via a cheap
    // placeholder swap, recurse into the moved body FIRST (nested lowered loops), then rebuild.
    if let Stmt::For { span, .. } = s {
        if spans.contains(&span.start) {
            let placeholder = Stmt::Block(Vec::new(), *span);
            if let Stmt::For {
                ty,
                name,
                iter,
                mut body,
                span,
                ..
            } = std::mem::replace(s, placeholder)
            {
                lower_block(&mut body, spans);
                let it_name = format!("__for_it_{}", span.start);
                let decl_it = Stmt::VarDecl {
                    ty: Type::Infer(span),
                    name: it_name.clone(),
                    init: iter,
                    mutable: false,
                    span,
                };
                let bind = Stmt::VarDecl {
                    ty,
                    name,
                    init: pull(&it_name, "next", span),
                    mutable: false,
                    span,
                };
                let mut wbody = Vec::with_capacity(body.len() + 1);
                wbody.push(bind);
                wbody.extend(body);
                let while_loop = Stmt::While {
                    cond: pull(&it_name, "hasNext", span),
                    body: wbody,
                    post_cond: false,
                    span,
                };
                *s = Stmt::Block(vec![decl_it, while_loop], span);
            }
            return;
        }
    }
    // Every other statement: recurse into nested statement lists.
    match s {
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            lower_block(then_block, spans);
            if let Some(b) = else_block {
                lower_block(b, spans);
            }
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } => lower_block(body, spans),
        Stmt::CFor {
            init, step, body, ..
        } => {
            if let Some(i) = init {
                lower_stmt(i, spans);
            }
            if let Some(st) = step {
                lower_stmt(st, spans);
            }
            lower_block(body, spans);
        }
        Stmt::Block(b, _) => lower_block(b, spans),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            lower_block(body, spans);
            for CatchClause { body, .. } in catches {
                lower_block(body, spans);
            }
            if let Some(fb) = finally_block {
                lower_block(fb, spans);
            }
        }
        Stmt::Destructure {
            else_block: Some(eb),
            ..
        } => lower_block(eb, spans),
        _ => {}
    }
}

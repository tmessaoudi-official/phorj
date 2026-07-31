//! `rewrite_ufcs`' total AST walk — split out of `rewrite_ufcs.rs` (Invariant 13, M-Decomp).
//!
//! Nested `fn`s lifted to module level: they close over nothing. DEC-356 added four `Expr` recursion
//! arms (`Tuple`, `NamedArg`, `TaggedTemplate`, `Pipe`). `apply_repl` keeps a catch-all on purpose —
//! CD-27: its domain is checker-CONSTRUCTED replacement shapes, not user AST, so exhaustiveness over 37
//! variants would mean two dozen arms asserting nothing.

use crate::ast::{Expr, LambdaBody, MatchArm, Stmt, StrPart};
use std::collections::HashMap;

pub(super) type Map = HashMap<usize, Expr>;

#[inline(never)]
pub(super) fn apply_repl(repl: &Expr, u: &Map) -> Expr {
    match repl {
        // A resolved UFCS call, or a `Reflection.typeName` object-case `className` / erased-case
        // `kind` call: re-walk the children. The call's own span is the original key or synthetic
        // (never re-matched — the root is reconstructed directly), and its args carry their own
        // distinct spans, so nested sugar in them resolves without looping.
        Expr::Call {
            callee,
            args,
            type_args,
            span,
        } => Expr::Call {
            callee: Box::new(rexpr((**callee).clone(), u)),
            args: args.iter().cloned().map(|a| rexpr(a, u)).collect(),
            type_args: type_args.clone(),
            span: *span,
        },
        // A `match` replacement: the `?.` UFCS null-safe desugar (`x?.f(a)`) AND `typeName`'s
        // optional null-branch. Re-walk ONLY the scrutinee (the embedded original receiver /
        // argument — its span differs from this call's key, so nested sugar in it resolves
        // safely); CLONE the arms. The arms must not be walked: the `?.` desugar reuses this
        // call's own span for its arm-body call, so walking it would re-match this very key and
        // recurse forever. Reflect's optional arms hold only a literal or `className(<fresh
        // binding>)` (no user sugar), so cloning them is complete; the `?.` arm body was never
        // walked in the original either, so this is no regression.
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(rexpr((**scrutinee).clone(), u)),
            arms: arms.clone(),
            span: *span,
        },
        // Any other replacement shape (e.g. `typeName`'s value-type baked string literal) carries
        // no embedded original subtree — clone it without walking.
        //
        // DEC-356 EXEMPTION (CD-27), deliberate and recorded. This match's domain is NOT user AST: it
        // dispatches on replacements the CHECKER constructed, so making it exhaustive over all 37
        // `Expr` variants would mean 24 arms asserting nothing about reachable code. `unreachable!()`
        // is worse still — DEC-356's own headline find was a valid program hitting exactly such a
        // panic (`html literal not resolved before compilation`), so adding one here on a checker path
        // would be repeating the mistake. The exemption is the arm below; every OTHER catch-all in
        // this file is fixed.
        other => other.clone(),
    }
}

pub(super) fn rexpr(e: Expr, u: &Map) -> Expr {
    match e {
        // A recorded call site (UFCS rewrite, or a `Reflection.typeName` substitution): apply the
        // replacement via the out-of-line `apply_repl` — kept a SEPARATE function on purpose, so
        // its (larger) match over replacement shapes does not inflate `rexpr`'s own stack frame.
        // `rexpr` is the deeply-recursive walker (a nested expression recurses one `rexpr` frame
        // per level), so its frame size is stack-critical; a recorded match is rare and shallow.
        Expr::Call {
            callee,
            args,
            type_args,
            span,
        } => match u.get(&span.start) {
            Some(repl) => apply_repl(repl, u),
            None => Expr::Call {
                callee: Box::new(rexpr(*callee, u)),
                args: args.into_iter().map(|a| rexpr(a, u)).collect(),
                type_args,
                span,
            },
        },
        Expr::Str(parts, span) => Expr::Str(
            parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, u))),
                    lit => lit,
                })
                .collect(),
            span,
        ),
        Expr::List(items, span) => {
            Expr::List(items.into_iter().map(|e| rexpr(e, u)).collect(), span)
        }
        Expr::Map(pairs, span) => Expr::Map(
            pairs
                .into_iter()
                .map(|(k, v)| (rexpr(k, u), rexpr(v, u)))
                .collect(),
            span,
        ),
        Expr::Unary { op, expr, span } => Expr::Unary {
            op,
            expr: Box::new(rexpr(*expr, u)),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(rexpr(*lhs, u)),
            rhs: Box::new(rexpr(*rhs, u)),
            span,
        },
        Expr::InstanceOf {
            value,
            type_name,
            span,
        } => Expr::InstanceOf {
            value: Box::new(rexpr(*value, u)),
            type_name,
            span,
        },
        // A primitive `as`-cast the checker rewrote to a native conversion call (M4 as-matrix),
        // keyed by the `Cast` node's span (the `as` token). `apply_repl` re-walks the embedded
        // original value (its span differs from this key) but reconstructs the call root directly,
        // so it never re-matches this key. An identity cast is not recorded → falls to the None
        // branch (the `Cast` survives; each backend emits the value).
        Expr::Cast {
            value,
            type_name,
            span,
        } => match u.get(&span.start) {
            Some(repl) => apply_repl(repl, u),
            None => Expr::Cast {
                value: Box::new(rexpr(*value, u)),
                type_name,
                span,
            },
        },
        Expr::Member {
            object,
            name,
            safe,
            sep: _,
            span,
        } => Expr::Member {
            object: Box::new(rexpr(*object, u)),
            name,
            safe,
            sep: crate::ast::MemberSep::Dot,
            span,
        },
        Expr::Index {
            object,
            index,
            span,
        } => Expr::Index {
            object: Box::new(rexpr(*object, u)),
            index: Box::new(rexpr(*index, u)),
            span,
        },
        Expr::Force { inner, span } => Expr::Force {
            inner: Box::new(rexpr(*inner, u)),
            span,
        },
        // A return-overload selector `<Type>f(args)` (M-RT Slice C1): the checker recorded the
        // resolved mangled `Call` keyed by this node's span. A successful check guarantees an
        // entry (an unresolved selector is a hard error → the program never reaches a backend);
        // `apply_repl` re-walks the embedded args without re-matching this key.
        Expr::OverloadSelect { ty, call, span } => match u.get(&span.start) {
            Some(repl) => apply_repl(repl, u),
            None => Expr::OverloadSelect {
                ty,
                call: Box::new(rexpr(*call, u)),
                span,
            },
        },
        // A `parent` call (super/parent): not rewritten itself, but its args may contain UFCS /
        // resolved overload / cast / default-fill sites — recurse them.
        Expr::ParentCall {
            ancestor,
            method,
            args,
            span,
        } => Expr::ParentCall {
            ancestor,
            method,
            args: args.into_iter().map(|a| rexpr(a, u)).collect(),
            span,
        },
        Expr::Propagate { inner, span } => Expr::Propagate {
            inner: Box::new(rexpr(*inner, u)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(rexpr(*scrutinee, u)),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| rexpr(g, u)),
                    body: rexpr(a.body, u),
                    span: a.span,
                })
                .collect(),
            span,
        },
        Expr::Range {
            start,
            end,
            inclusive,
            span,
        } => Expr::Range {
            start: Box::new(rexpr(*start, u)),
            end: Box::new(rexpr(*end, u)),
            inclusive,
            span,
        },
        Expr::If {
            cond,
            then_expr,
            else_expr,
            span,
        } => Expr::If {
            cond: Box::new(rexpr(*cond, u)),
            then_expr: Box::new(rexpr(*then_expr, u)),
            else_expr: Box::new(rexpr(*else_expr, u)),
            span,
        },
        Expr::Lambda {
            params,
            ret,
            throws,
            body,
            span,
        } => Expr::Lambda {
            params,
            ret,
            throws,
            body: match body {
                LambdaBody::Expr(e) => LambdaBody::Expr(Box::new(rexpr(*e, u))),
                LambdaBody::Block(stmts) => LambdaBody::Block(rblock(stmts, u)),
            },
            span,
        },
        Expr::CloneWith {
            object,
            fields,
            span,
        } => Expr::CloneWith {
            object: Box::new(rexpr(*object, u)),
            fields: fields.into_iter().map(|(n, e)| (n, rexpr(e, u))).collect(),
            span,
        },
        // A `new` reaching here is inside a RELOCATED UFCS replacement subtree: the `ufcs` map
        // captured the call site during checking, BEFORE `unwrap_new` ran, so its embedded
        // receiver/argument subtrees still carry `Expr::New` wrappers (a `new` in the MAIN tree was
        // already stripped — `unwrap_new` runs before this pass, and every `New` must be gone before
        // any backend). Strip it exactly as `checker::rewrite_new::ue_expr` does: recurse first
        // (nested UFCS / nested `new`), then erase the wrapper, collapsing a qualified variant
        // construction `new Enum.Variant(args)` (a `Member` callee) to the bare `Variant(args)` every
        // backend builds. Without this, `xs.map(function(x) => new C(x))` (any UFCS call with a
        // constructing lambda/arg) panics the interpreter/compiler with a surviving `Expr::New`.
        Expr::New(inner, _span) => {
            let mut inner = rexpr(*inner, u);
            if let Expr::Call { callee, .. } = &mut inner {
                if let Expr::Member {
                    name, span: msp, ..
                } = callee.as_ref()
                {
                    *callee.as_mut() = Expr::Ident(name.clone(), *msp);
                }
            }
            inner
        }
        // `spawn <call>` (M6 W4): walk the nested call so a UFCS method call inside it rewrites.
        Expr::Spawn { call, span } => Expr::Spawn {
            call: Box::new(rexpr(*call, u)),
            span,
        },
        Expr::Html(parts, span) => Expr::Html(parts, span),
        // leaves carry no nested expression: Int / Float / Bool / Null / Bytes / Ident / This
        // DEC-356: these bear expressions and were silently passed through by `leaf => leaf`.
        Expr::Tuple(items, span) => {
            Expr::Tuple(items.into_iter().map(|e| rexpr(e, u)).collect(), span)
        }
        Expr::NamedArg { name, value, span } => Expr::NamedArg {
            name,
            value: Box::new(rexpr(*value, u)),
            span,
        },
        Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
            lhs: Box::new(rexpr(*lhs, u)),
            rhs: Box::new(rexpr(*rhs, u)),
            span,
        },
        Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
            tag,
            parts: parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, u))),
                    StrPart::Literal(s) => StrPart::Literal(s),
                })
                .collect(),
            span,
        },
        // Carries no nested expression — single-sourced in `ast::leaves` (DEC-356).
        e @ (crate::expr_leaves!() | Expr::NewColl { .. } | Expr::Inject { .. }) => e,
    }
}

pub(super) fn rstmt(s: Stmt, u: &Map) -> Stmt {
    match s {
        Stmt::VarDecl {
            ty,
            name,
            init,
            mutable,
            span,
        } => Stmt::VarDecl {
            ty,
            name,
            init: rexpr(init, u),
            mutable,
            span,
        },
        Stmt::Assign {
            target,
            value,
            span,
        } => Stmt::Assign {
            target: rexpr(target, u),
            value: rexpr(value, u),
            span,
        },
        Stmt::Return { value, span } => Stmt::Return {
            value: value.map(|e| rexpr(e, u)),
            span,
        },
        Stmt::If {
            cond,
            bind,
            then_block,
            else_block,
            span,
        } => Stmt::If {
            cond: rexpr(cond, u),
            bind,
            then_block: rblock(then_block, u),
            else_block: else_block.map(|b| rblock(b, u)),
            span,
        },
        Stmt::For {
            ty,
            name,
            val,
            iter,
            body,
            span,
        } => Stmt::For {
            ty,
            name,
            val,
            iter: rexpr(iter, u),
            body: rblock(body, u),
            span,
        },
        Stmt::Using {
            ty,
            name,
            init,
            body,
            span,
        } => Stmt::Using {
            ty,
            name,
            init: rexpr(init, u),
            body: rblock(body, u),
            span,
        },
        Stmt::While {
            cond,
            body,
            post_cond,
            span,
        } => Stmt::While {
            cond: rexpr(cond, u),
            body: rblock(body, u),
            post_cond,
            span,
        },
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            span,
        } => Stmt::CFor {
            init: init.map(|s| Box::new(rstmt(*s, u))),
            cond: cond.map(|e| rexpr(e, u)),
            step: step.map(|s| Box::new(rstmt(*s, u))),
            body: rblock(body, u),
            span,
        },
        Stmt::Break(span) => Stmt::Break(span),
        Stmt::Continue(span) => Stmt::Continue(span),
        Stmt::Block(stmts, span) => Stmt::Block(rblock(stmts, u), span),
        Stmt::Expr(e, span) => Stmt::Expr(rexpr(e, u), span),
        Stmt::Discard(e, span) => Stmt::Discard(rexpr(e, u), span),
        Stmt::Throw { value, span } => Stmt::Throw {
            value: rexpr(value, u),
            span,
        },
        Stmt::Try {
            body,
            catches,
            finally_block,
            span,
        } => Stmt::Try {
            body: rblock(body, u),
            catches: catches
                .into_iter()
                .map(|c| crate::ast::CatchClause {
                    ty: c.ty,
                    name: c.name,
                    body: rblock(c.body, u),
                    span: c.span,
                })
                .collect(),
            finally_block: finally_block.map(|b| rblock(b, u)),
            span,
        },
        Stmt::Destructure {
            pat,
            init,
            else_block,
            span,
        } => Stmt::Destructure {
            pat,
            init: rexpr(init, u),
            else_block: else_block.map(|b| rblock(b, u)),
            span,
        },
    }
}

pub(super) fn rblock(stmts: Vec<Stmt>, u: &Map) -> Vec<Stmt> {
    stmts.into_iter().map(|s| rstmt(s, u)).collect()
}

//! `resolve_variant_imports`'s total AST walk — split out of the parent (Invariant 13, M-Decomp).
//!
//! Holds the `Pattern`/`Expr`/`Stmt` traversal, including DEC-356's four new `Expr` recursion arms and
//! the now-explicit `Pattern` arm (`Binding` and `Type` are decisions, not leftovers).

use super::*;

/// Rewrite a pattern: qualify an imported `Pattern::Variant` head, recurse into nested sub-patterns.
pub(super) fn rpat(p: Pattern, m: &VarMap) -> Pattern {
    match p {
        Pattern::Variant {
            name,
            fields,
            enum_qualifier,
            span,
        } => {
            let fields: Vec<Pattern> = fields.into_iter().map(|f| rpat(f, m)).collect();
            // Only an UNqualified head is a candidate — a `Enum.Variant(..)` pattern already carries its
            // qualifier. If the bare head is an imported variant, qualify it to the real (enum, variant).
            if enum_qualifier.is_none() {
                if let Some((enum_name, real)) = m.get(&name) {
                    return Pattern::Variant {
                        name: real.clone(),
                        fields,
                        enum_qualifier: Some(enum_name.clone()),
                        span,
                    };
                }
            }
            Pattern::Variant {
                name,
                fields,
                enum_qualifier,
                span,
            }
        }
        Pattern::Struct {
            type_name,
            fields,
            span,
        } => Pattern::Struct {
            type_name,
            fields: fields
                .into_iter()
                .map(|fp| FieldPat {
                    field: fp.field,
                    pat: rpat(fp.pat, m),
                })
                .collect(),
            span,
        },
        // A bare identifier (`X =>`) is a catch-all binding, NOT a variant (the existing zero-payload
        // rule) — deliberately not rewritten. Named explicitly (DEC-356) rather than swept into a
        // catch-all: `Binding` and `Type` are decisions, not leftovers, and the leaf set is
        // single-sourced in `ast::leaves`.
        p @ (crate::pattern_leaves!() | Pattern::Binding { .. } | Pattern::Type { .. }) => p,
    }
}

pub(super) fn rexpr(e: Expr, m: &VarMap) -> Expr {
    match e {
        // The one rewrite site for construction: `new X(args)` → `new Enum.Variant(args)` when `X` is an
        // imported variant. Recurse the inner call first (nested `new`s / args), then qualify the callee;
        // the `New` wrapper SURVIVES (the checker needs it; `unwrap_new` strips it post-check).
        Expr::New(inner, span) => {
            let inner = rexpr(*inner, m);
            if let Expr::Call {
                callee,
                args,
                type_args,
                span: cspan,
            } = inner
            {
                let callee = match *callee {
                    Expr::Ident(name, isp) => match m.get(&name) {
                        Some((enum_name, real)) => Box::new(Expr::Member {
                            object: Box::new(Expr::Ident(enum_name.clone(), isp)),
                            name: real.clone(),
                            safe: false,
                            sep: crate::ast::MemberSep::Dot,
                            span: isp,
                        }),
                        None => Box::new(Expr::Ident(name, isp)),
                    },
                    other => Box::new(rexpr(other, m)),
                };
                Expr::New(
                    Box::new(Expr::Call {
                        callee,
                        args,
                        type_args,
                        span: cspan,
                    }),
                    span,
                )
            } else {
                Expr::New(Box::new(inner), span)
            }
        }
        Expr::Call {
            callee,
            args,
            type_args,
            span,
        } => Expr::Call {
            callee: Box::new(rexpr(*callee, m)),
            args: args.into_iter().map(|a| rexpr(a, m)).collect(),
            type_args,
            span,
        },
        Expr::Str(parts, span) => Expr::Str(
            parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, m))),
                    lit => lit,
                })
                .collect(),
            span,
        ),
        Expr::List(items, span) => {
            Expr::List(items.into_iter().map(|e| rexpr(e, m)).collect(), span)
        }
        Expr::Map(pairs, span) => Expr::Map(
            pairs
                .into_iter()
                .map(|(k, v)| (rexpr(k, m), rexpr(v, m)))
                .collect(),
            span,
        ),
        Expr::Unary { op, expr, span } => Expr::Unary {
            op,
            expr: Box::new(rexpr(*expr, m)),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(rexpr(*lhs, m)),
            rhs: Box::new(rexpr(*rhs, m)),
            span,
        },
        Expr::InstanceOf {
            value,
            type_name,
            span,
        } => Expr::InstanceOf {
            value: Box::new(rexpr(*value, m)),
            type_name,
            span,
        },
        Expr::Cast {
            value,
            type_name,
            span,
        } => Expr::Cast {
            value: Box::new(rexpr(*value, m)),
            type_name,
            span,
        },
        Expr::Member {
            object,
            name,
            safe,
            sep: _,
            span,
        } => Expr::Member {
            object: Box::new(rexpr(*object, m)),
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
            object: Box::new(rexpr(*object, m)),
            index: Box::new(rexpr(*index, m)),
            span,
        },
        Expr::Force { inner, span } => Expr::Force {
            inner: Box::new(rexpr(*inner, m)),
            span,
        },
        Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
            ty,
            call: Box::new(rexpr(*call, m)),
            span,
        },
        Expr::ParentCall {
            ancestor,
            method,
            args,
            span,
        } => Expr::ParentCall {
            ancestor,
            method,
            args: args.into_iter().map(|a| rexpr(a, m)).collect(),
            span,
        },
        Expr::Propagate { inner, span } => Expr::Propagate {
            inner: Box::new(rexpr(*inner, m)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(rexpr(*scrutinee, m)),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: rpat(a.pattern, m),
                    guard: a.guard.map(|g| rexpr(g, m)),
                    body: rexpr(a.body, m),
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
            start: Box::new(rexpr(*start, m)),
            end: Box::new(rexpr(*end, m)),
            inclusive,
            span,
        },
        Expr::If {
            cond,
            then_expr,
            else_expr,
            span,
        } => Expr::If {
            cond: Box::new(rexpr(*cond, m)),
            then_expr: Box::new(rexpr(*then_expr, m)),
            else_expr: Box::new(rexpr(*else_expr, m)),
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
                LambdaBody::Expr(e) => LambdaBody::Expr(Box::new(rexpr(*e, m))),
                LambdaBody::Block(stmts) => LambdaBody::Block(rblock(stmts, m)),
            },
            span,
        },
        Expr::CloneWith {
            object,
            fields,
            span,
        } => Expr::CloneWith {
            object: Box::new(rexpr(*object, m)),
            fields: fields.into_iter().map(|(n, e)| (n, rexpr(e, m))).collect(),
            span,
        },
        Expr::Spawn { call, span } => Expr::Spawn {
            call: Box::new(rexpr(*call, m)),
            span,
        },
        Expr::Html(parts, span) => Expr::Html(parts, span),
        // DEC-356: these four bear expressions and were silently passed through by `leaf => leaf`.
        Expr::Tuple(items, span) => {
            Expr::Tuple(items.into_iter().map(|e| rexpr(e, m)).collect(), span)
        }
        Expr::NamedArg { name, value, span } => Expr::NamedArg {
            name,
            value: Box::new(rexpr(*value, m)),
            span,
        },
        Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
            tag,
            parts: parts
                .into_iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, m))),
                    StrPart::Literal(s) => StrPart::Literal(s),
                })
                .collect(),
            span,
        },
        Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
            lhs: Box::new(rexpr(*lhs, m)),
            rhs: Box::new(rexpr(*rhs, m)),
            span,
        },
        // Carries no nested expression — single-sourced in `ast::leaves` (DEC-356).
        e @ (crate::expr_leaves!() | Expr::NewColl { .. } | Expr::Inject { .. }) => e,
    }
}

pub(super) fn rstmt(s: Stmt, m: &VarMap) -> Stmt {
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
            init: rexpr(init, m),
            mutable,
            span,
        },
        Stmt::Assign {
            target,
            value,
            span,
        } => Stmt::Assign {
            target: rexpr(target, m),
            value: rexpr(value, m),
            span,
        },
        Stmt::Return { value, span } => Stmt::Return {
            value: value.map(|e| rexpr(e, m)),
            span,
        },
        Stmt::If {
            cond,
            bind,
            then_block,
            else_block,
            span,
        } => Stmt::If {
            cond: rexpr(cond, m),
            bind,
            then_block: rblock(then_block, m),
            else_block: else_block.map(|b| rblock(b, m)),
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
            iter: rexpr(iter, m),
            body: rblock(body, m),
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
            init: rexpr(init, m),
            body: rblock(body, m),
            span,
        },
        Stmt::While {
            cond,
            body,
            post_cond,
            span,
        } => Stmt::While {
            cond: rexpr(cond, m),
            body: rblock(body, m),
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
            init: init.map(|s| Box::new(rstmt(*s, m))),
            cond: cond.map(|e| rexpr(e, m)),
            step: step.map(|s| Box::new(rstmt(*s, m))),
            body: rblock(body, m),
            span,
        },
        Stmt::Break(span) => Stmt::Break(span),
        Stmt::Continue(span) => Stmt::Continue(span),
        Stmt::Block(stmts, span) => Stmt::Block(rblock(stmts, m), span),
        Stmt::Expr(e, span) => Stmt::Expr(rexpr(e, m), span),
        Stmt::Discard(e, span) => Stmt::Discard(rexpr(e, m), span),
        Stmt::Throw { value, span } => Stmt::Throw {
            value: rexpr(value, m),
            span,
        },
        Stmt::Try {
            body,
            catches,
            finally_block,
            span,
        } => Stmt::Try {
            body: rblock(body, m),
            catches: catches
                .into_iter()
                .map(|c| crate::ast::CatchClause {
                    ty: c.ty,
                    name: c.name,
                    body: rblock(c.body, m),
                    span: c.span,
                })
                .collect(),
            finally_block: finally_block.map(|b| rblock(b, m)),
            span,
        },
        // `Destructure.pat` is a `DestructurePat` (list/map/struct binding — no enum-variant head), so it
        // needs no variant rewrite; only its initializer expression can contain a `new`/`match`.
        Stmt::Destructure {
            pat,
            init,
            else_block,
            span,
        } => Stmt::Destructure {
            pat,
            init: rexpr(init, m),
            else_block: else_block.map(|b| rblock(b, m)),
            span,
        },
    }
}

pub(super) fn rblock(stmts: Vec<Stmt>, m: &VarMap) -> Vec<Stmt> {
    stmts.into_iter().map(|s| rstmt(s, m)).collect()
}

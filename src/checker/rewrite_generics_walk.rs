//! `erase_generics`' total AST walk — split out of `rewrite_generics.rs` (Invariant 13, M-Decomp).
//!
//! These were nested `fn`s inside `erase_generics`; they close over nothing (every input is a
//! parameter), so lifting them to a sibling module is behaviour-preserving. DEC-356 added five `Expr`
//! recursion arms here — `Tuple`, `NamedArg`, `New`, `TaggedTemplate` and `Pipe` were all being returned
//! un-erased by one `leaf => leaf.clone()`.

use crate::ast::{Expr, LambdaBody, MatchArm, Param, Stmt, StrPart, Type};
use std::collections::HashSet;

pub(super) type Params<'a> = HashSet<&'a str>;

pub(super) fn rty(ty: &Type, params: &Params) -> Type {
    match ty {
        Type::Named { name, args, span } => {
            // A bare reference to a type parameter erases; a real generic container (`List<T>`)
            // keeps its head and recurses into its arguments.
            if args.is_empty() && params.contains(name.as_str()) {
                Type::Erased(*span)
            } else {
                Type::Named {
                    name: name.clone(),
                    args: args.iter().map(|a| rty(a, params)).collect(),
                    span: *span,
                }
            }
        }
        Type::Optional { inner, span } => Type::Optional {
            inner: Box::new(rty(inner, params)),
            span: *span,
        },
        Type::Function {
            params: ps,
            ret,
            throws,
            span,
        } => Type::Function {
            params: ps.iter().map(|p| rty(p, params)).collect(),
            ret: Box::new(rty(ret, params)),
            // DEC-222: erase generics in the throws types too (a type-param thrown type becomes
            // `Type::Erased`, like any other position — the whole function type survives erasure).
            throws: throws.iter().map(|t| rty(t, params)).collect(),
            span: *span,
        },
        // A union erases each member (a type-param member becomes `Type::Erased`); the union
        // itself is structural and survives to the backend (M-RT S4).
        Type::Union(members, span) => {
            Type::Union(members.iter().map(|m| rty(m, params)).collect(), *span)
        }
        Type::Tuple(members, span) => {
            Type::Tuple(members.iter().map(|m| rty(m, params)).collect(), *span)
        }
        // An intersection erases each member (a type-param member becomes `Type::Erased`); the
        // intersection itself is structural and survives to the backend (M-RT S5).
        Type::Intersection(members, span) => {
            Type::Intersection(members.iter().map(|m| rty(m, params)).collect(), *span)
        }
        // `[T; N]`: erase a type-param element (`[T; 2]` → `[<erased>; 2]`); the fixed-list head
        // survives to the backend, which treats it as a list either way (Phase 1 types slice).
        Type::FixedList { elem, len, span } => Type::FixedList {
            elem: Box::new(rty(elem, params)),
            len: *len,
            span: *span,
        },
        Type::Infer(s) => Type::Infer(*s),
        Type::Erased(s) => Type::Erased(*s),
    }
}
pub(super) fn rparam(p: &Param, params: &Params) -> Param {
    Param {
        ty: rty(&p.ty, params),
        name: p.name.clone(),
        // A default is a literal (no type params inside) — carry it verbatim.
        default: p.default.clone(),
        variadic: p.variadic,
        span: p.span,
    }
}
pub(super) fn rctorparam(p: &crate::ast::CtorParam, params: &Params) -> crate::ast::CtorParam {
    crate::ast::CtorParam {
        modifiers: p.modifiers.clone(),
        ty: rty(&p.ty, params),
        name: p.name.clone(),
        // A default is a literal (no type params inside) — carry it verbatim.
        default: p.default.clone(),
        span: p.span,
    }
}
pub(super) fn rparts(parts: &[StrPart], params: &Params) -> Vec<StrPart> {
    parts
        .iter()
        .map(|p| match p {
            StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(e, params))),
            StrPart::Literal(s) => StrPart::Literal(s.clone()),
        })
        .collect()
}
pub(super) fn rexpr(e: &Expr, params: &Params) -> Expr {
    match e {
        // The only expression that carries types: a lambda's parameters, return, and throws.
        Expr::Lambda {
            params: lp,
            ret,
            throws,
            body,
            span,
        } => Expr::Lambda {
            params: lp.iter().map(|p| rparam(p, params)).collect(),
            ret: ret.as_ref().map(|t| rty(t, params)),
            // DEC-222: erase generics in a lambda's declared throws types too.
            throws: throws.iter().map(|t| rty(t, params)).collect(),
            body: match body {
                LambdaBody::Expr(inner) => LambdaBody::Expr(Box::new(rexpr(inner, params))),
                LambdaBody::Block(stmts) => {
                    LambdaBody::Block(stmts.iter().map(|s| rstmt(s, params)).collect())
                }
            },
            span: *span,
        },
        Expr::Str(parts, span) => Expr::Str(rparts(parts, params), *span),
        Expr::Html(parts, span) => Expr::Html(rparts(parts, params), *span),
        Expr::List(items, span) => {
            Expr::List(items.iter().map(|i| rexpr(i, params)).collect(), *span)
        }
        Expr::Map(pairs, span) => Expr::Map(
            pairs
                .iter()
                .map(|(k, v)| (rexpr(k, params), rexpr(v, params)))
                .collect(),
            *span,
        ),
        Expr::Unary { op, expr, span } => Expr::Unary {
            op: *op,
            expr: Box::new(rexpr(expr, params)),
            span: *span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op: *op,
            lhs: Box::new(rexpr(lhs, params)),
            rhs: Box::new(rexpr(rhs, params)),
            span: *span,
        },
        Expr::InstanceOf {
            value,
            type_name,
            span,
        } => Expr::InstanceOf {
            value: Box::new(rexpr(value, params)),
            type_name: type_name.clone(),
            span: *span,
        },
        Expr::Cast {
            value,
            type_name,
            span,
        } => Expr::Cast {
            value: Box::new(rexpr(value, params)),
            type_name: type_name.clone(),
            span: *span,
        },
        Expr::Call {
            callee,
            args,
            type_args,
            span,
        } => Expr::Call {
            callee: Box::new(rexpr(callee, params)),
            args: args.iter().map(|a| rexpr(a, params)).collect(),
            type_args: type_args.clone(),
            span: *span,
        },
        // A return-overload selector (Slice C1) / a `parent` call (super/parent): recurse the
        // sub-expressions so a generic-typed annotation inside them is erased too.
        Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
            ty: ty.clone(),
            call: Box::new(rexpr(call, params)),
            span: *span,
        },
        Expr::ParentCall {
            ancestor,
            method,
            args,
            span,
        } => Expr::ParentCall {
            ancestor: ancestor.clone(),
            method: method.clone(),
            args: args.iter().map(|a| rexpr(a, params)).collect(),
            span: *span,
        },
        Expr::Member {
            object,
            name,
            safe,
            sep: _,
            span,
        } => Expr::Member {
            object: Box::new(rexpr(object, params)),
            name: name.clone(),
            safe: *safe,
            sep: crate::ast::MemberSep::Dot,
            span: *span,
        },
        Expr::Index {
            object,
            index,
            span,
        } => Expr::Index {
            object: Box::new(rexpr(object, params)),
            index: Box::new(rexpr(index, params)),
            span: *span,
        },
        Expr::Force { inner, span } => Expr::Force {
            inner: Box::new(rexpr(inner, params)),
            span: *span,
        },
        Expr::Propagate { inner, span } => Expr::Propagate {
            inner: Box::new(rexpr(inner, params)),
            span: *span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(rexpr(scrutinee, params)),
            arms: arms
                .iter()
                .map(|a| MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.as_ref().map(|g| rexpr(g, params)),
                    body: rexpr(&a.body, params),
                    span: a.span,
                })
                .collect(),
            span: *span,
        },
        Expr::Range {
            start,
            end,
            inclusive,
            span,
        } => Expr::Range {
            start: Box::new(rexpr(start, params)),
            end: Box::new(rexpr(end, params)),
            inclusive: *inclusive,
            span: *span,
        },
        Expr::If {
            cond,
            then_expr,
            else_expr,
            span,
        } => Expr::If {
            cond: Box::new(rexpr(cond, params)),
            then_expr: Box::new(rexpr(then_expr, params)),
            else_expr: Box::new(rexpr(else_expr, params)),
            span: *span,
        },
        Expr::CloneWith {
            object,
            fields,
            span,
        } => Expr::CloneWith {
            object: Box::new(rexpr(object, params)),
            fields: fields
                .iter()
                .map(|(n, e)| (n.clone(), rexpr(e, params)))
                .collect(),
            span: *span,
        },
        // `spawn <call>` (M6 W4): walk the nested call so a generic call inside it is erased too
        // (not front-end-erased itself — it reaches every rewrite pass).
        Expr::Spawn { call, span } => Expr::Spawn {
            call: Box::new(rexpr(call, params)),
            span: *span,
        },
        // leaves carry no type and no nested expression: Int / Float / Bool / Null / Bytes /
        // Ident / This — clone unchanged.
        // DEC-356: these five bear expressions and were silently passed through by `leaf => leaf`.
        Expr::Tuple(items, span) => {
            Expr::Tuple(items.iter().map(|e| rexpr(e, params)).collect(), *span)
        }
        Expr::NamedArg { name, value, span } => Expr::NamedArg {
            name: name.clone(),
            value: Box::new(rexpr(value, params)),
            span: *span,
        },
        Expr::New(inner, span) => Expr::New(Box::new(rexpr(inner, params)), *span),
        Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
            tag: tag.clone(),
            parts: parts
                .iter()
                .map(|p| match p {
                    StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(e, params))),
                    StrPart::Literal(s) => StrPart::Literal(s.clone()),
                })
                .collect(),
            span: *span,
        },
        Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
            lhs: Box::new(rexpr(lhs, params)),
            rhs: Box::new(rexpr(rhs, params)),
            span: *span,
        },
        // Carries no nested expression — the set is single-sourced in `ast::leaves`, so adding an
        // `Expr` variant breaks the build here until someone rules whether it is a leaf (DEC-356).
        e @ (crate::expr_leaves!() | Expr::NewColl { .. } | Expr::Inject { .. }) => e.clone(),
    }
}
pub(super) fn rstmt(s: &Stmt, params: &Params) -> Stmt {
    match s {
        Stmt::VarDecl {
            ty,
            name,
            init,
            mutable,
            span,
        } => Stmt::VarDecl {
            ty: rty(ty, params),
            name: name.clone(),
            init: rexpr(init, params),
            mutable: *mutable,
            span: *span,
        },
        Stmt::Assign {
            target,
            value,
            span,
        } => Stmt::Assign {
            target: rexpr(target, params),
            value: rexpr(value, params),
            span: *span,
        },
        Stmt::Return { value, span } => Stmt::Return {
            value: value.as_ref().map(|e| rexpr(e, params)),
            span: *span,
        },
        Stmt::If {
            cond,
            bind,
            then_block,
            else_block,
            span,
        } => Stmt::If {
            cond: rexpr(cond, params),
            bind: bind.clone(),
            then_block: then_block.iter().map(|s| rstmt(s, params)).collect(),
            else_block: else_block
                .as_ref()
                .map(|b| b.iter().map(|s| rstmt(s, params)).collect()),
            span: *span,
        },
        Stmt::For {
            ty,
            name,
            val,
            iter,
            body,
            span,
        } => Stmt::For {
            ty: rty(ty, params),
            name: name.clone(),
            val: val.as_ref().map(|(t, n)| (rty(t, params), n.clone())),
            iter: rexpr(iter, params),
            body: body.iter().map(|s| rstmt(s, params)).collect(),
            span: *span,
        },
        Stmt::Using {
            ty,
            name,
            init,
            body,
            span,
        } => Stmt::Using {
            ty: rty(ty, params),
            name: name.clone(),
            init: rexpr(init, params),
            body: body.iter().map(|s| rstmt(s, params)).collect(),
            span: *span,
        },
        Stmt::While {
            cond,
            body,
            post_cond,
            span,
        } => Stmt::While {
            cond: rexpr(cond, params),
            body: body.iter().map(|s| rstmt(s, params)).collect(),
            post_cond: *post_cond,
            span: *span,
        },
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            span,
        } => Stmt::CFor {
            init: init.as_ref().map(|s| Box::new(rstmt(s, params))),
            cond: cond.as_ref().map(|e| rexpr(e, params)),
            step: step.as_ref().map(|s| Box::new(rstmt(s, params))),
            body: body.iter().map(|s| rstmt(s, params)).collect(),
            span: *span,
        },
        Stmt::Break(span) => Stmt::Break(*span),
        Stmt::Continue(span) => Stmt::Continue(*span),
        Stmt::Block(stmts, span) => {
            Stmt::Block(stmts.iter().map(|s| rstmt(s, params)).collect(), *span)
        }
        Stmt::Expr(e, span) => Stmt::Expr(rexpr(e, params), *span),
        Stmt::Discard(e, span) => Stmt::Discard(rexpr(e, params), *span),
        Stmt::Throw { value, span } => Stmt::Throw {
            value: rexpr(value, params),
            span: *span,
        },
        Stmt::Try {
            body,
            catches,
            finally_block,
            span,
        } => Stmt::Try {
            body: body.iter().map(|s| rstmt(s, params)).collect(),
            catches: catches
                .iter()
                .map(|c| crate::ast::CatchClause {
                    ty: rty(&c.ty, params),
                    name: c.name.clone(),
                    body: c.body.iter().map(|s| rstmt(s, params)).collect(),
                    span: c.span,
                })
                .collect(),
            finally_block: finally_block
                .as_ref()
                .map(|b| b.iter().map(|s| rstmt(s, params)).collect()),
            span: *span,
        },
        // Slice 5: erase generics in the init expr and the `else` block. The struct head is a bare
        // class name with no type arguments in destructure syntax, so the pattern is cloned as-is.
        Stmt::Destructure {
            pat,
            init,
            else_block,
            span,
        } => Stmt::Destructure {
            pat: pat.clone(),
            init: rexpr(init, params),
            else_block: else_block
                .as_ref()
                .map(|b| b.iter().map(|s| rstmt(s, params)).collect()),
            span: *span,
        },
    }
}

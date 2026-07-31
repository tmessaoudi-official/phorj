//! `desugar_di`'s total AST walk — split out of `walker.rs` (Invariant 13, M-Decomp).
//!
//! A SECOND `impl Di` block: Rust permits inherent impls to be split across files in one crate, so
//! the traversal lives here while `walker.rs` keeps the graph resolution it traverses for.
//!
//! DEC-356 added four `Expr` recursion arms AND a `Stmt::Destructure` arm here. The `Destructure`
//! omission is the notable one: that statement bears an initializer expression, and the old
//! `leaf => leaf` arm dropped both it and the else-block — `desugar_db` walks the same statement
//! correctly three files away, which is exactly what made the gap invisible.

use super::*;

impl Di<'_> {
    pub(super) fn rexpr(&mut self, e: Expr) -> Expr {
        match e {
            // Explicit turbofish `inject<T>()` / `DependencyInjection.inject<T>()` (parser-produced). Gate the import,
            // then resolve. In a non-annotation position, `ty: None` cannot arise from the parser; it
            // only reaches here via `annotation_inject` re-dispatch, so a `None` here means an annotation
            // `inject()` used where no expected type is available → `E-INJECT-NO-TYPE`.
            Expr::Inject {
                ty,
                qualified,
                span,
            } => self.rinject(ty, qualified, None, span),
            // Recognize an annotation-form composition root written as an ordinary call — but only when
            // the matching import is present; otherwise it is a genuine user call and recurses normally.
            Expr::Call {
                callee,
                args,
                type_args,
                span,
            } if args.is_empty() => match self.annotation_inject(&callee) {
                Some(qualified) => self.rinject(None, qualified, None, span),
                None => Expr::Call {
                    callee: Box::new(self.rexpr(*callee)),
                    args: Vec::new(),
                    type_args,
                    span,
                },
            },
            Expr::Call {
                callee,
                args,
                type_args,
                span,
            } => Expr::Call {
                callee: Box::new(self.rexpr(*callee)),
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                type_args,
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
                args: args.into_iter().map(|a| self.rexpr(a)).collect(),
                span,
            },
            Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
                ty,
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Str(parts, span) => Expr::Str(self.rparts(parts), span),
            Expr::List(items, span) => {
                Expr::List(items.into_iter().map(|e| self.rexpr(e)).collect(), span)
            }
            Expr::Map(pairs, span) => Expr::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (self.rexpr(k), self.rexpr(v)))
                    .collect(),
                span,
            ),
            Expr::Unary { op, expr, span } => Expr::Unary {
                op,
                expr: Box::new(self.rexpr(*expr)),
                span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op,
                lhs: Box::new(self.rexpr(*lhs)),
                rhs: Box::new(self.rexpr(*rhs)),
                span,
            },
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => Expr::InstanceOf {
                value: Box::new(self.rexpr(*value)),
                type_name,
                span,
            },
            Expr::Cast {
                value,
                type_name,
                span,
            } => Expr::Cast {
                value: Box::new(self.rexpr(*value)),
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
                object: Box::new(self.rexpr(*object)),
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
                object: Box::new(self.rexpr(*object)),
                index: Box::new(self.rexpr(*index)),
                span,
            },
            Expr::Force { inner, span } => Expr::Force {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Propagate { inner, span } => Expr::Propagate {
                inner: Box::new(self.rexpr(*inner)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: Box::new(self.rexpr(*scrutinee)),
                arms: arms
                    .into_iter()
                    .map(|a| MatchArm {
                        pattern: a.pattern,
                        guard: a.guard.map(|g| self.rexpr(g)),
                        body: self.rexpr(a.body),
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
                start: Box::new(self.rexpr(*start)),
                end: Box::new(self.rexpr(*end)),
                inclusive,
                span,
            },
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => Expr::If {
                cond: Box::new(self.rexpr(*cond)),
                then_expr: Box::new(self.rexpr(*then_expr)),
                else_expr: Box::new(self.rexpr(*else_expr)),
                span,
            },
            Expr::Lambda {
                params,
                ret,
                throws,
                body,
                span,
            } => {
                // A lambda is its own return-type scope: save/restore `current_ret` so a `return
                // inject()` inside never inherits the enclosing function's return type, and its expr-body
                // is itself a return position (draws from the lambda's declared `ret`).
                let prev_ret = std::mem::replace(&mut self.current_ret, ret.clone());
                let new_body = match body {
                    LambdaBody::Expr(e) => {
                        let expected = self.current_ret.clone();
                        LambdaBody::Expr(Box::new(self.rexpr_expected(*e, expected.as_ref())))
                    }
                    LambdaBody::Block(stmts) => LambdaBody::Block(self.rblock(stmts)),
                };
                self.current_ret = prev_ret;
                Expr::Lambda {
                    params,
                    ret,
                    throws,
                    body: new_body,
                    span,
                }
            }
            Expr::CloneWith {
                object,
                fields,
                span,
            } => Expr::CloneWith {
                object: Box::new(self.rexpr(*object)),
                fields: fields
                    .into_iter()
                    .map(|(n, e)| (n, self.rexpr(e)))
                    .collect(),
                span,
            },
            Expr::New(inner, span) => Expr::New(Box::new(self.rexpr(*inner)), span),
            Expr::Spawn { call, span } => Expr::Spawn {
                call: Box::new(self.rexpr(*call)),
                span,
            },
            Expr::Html(parts, span) => Expr::Html(self.rparts(parts), span),
            // true leaves (Int/Float/Decimal/Bool/Null/Bytes/Ident/This) carry no nested expression.
            // DEC-356: these bear expressions and were silently passed through by `leaf => leaf`.
            Expr::Tuple(items, span) => {
                Expr::Tuple(items.into_iter().map(|e| self.rexpr(e)).collect(), span)
            }
            Expr::NamedArg { name, value, span } => Expr::NamedArg {
                name,
                value: Box::new(self.rexpr(*value)),
                span,
            },
            Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
                lhs: Box::new(self.rexpr(*lhs)),
                rhs: Box::new(self.rexpr(*rhs)),
                span,
            },
            Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
                tag,
                parts: parts
                    .into_iter()
                    .map(|p| match p {
                        StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                        StrPart::Literal(s) => StrPart::Literal(s),
                    })
                    .collect(),
                span,
            },
            // Carries no nested expression — single-sourced in `ast::leaves` (DEC-356).
            e @ (crate::expr_leaves!() | Expr::NewColl { .. }) => e,
        }
    }

    pub(super) fn rparts(&mut self, parts: Vec<StrPart>) -> Vec<StrPart> {
        parts
            .into_iter()
            .map(|p| match p {
                StrPart::Expr(e) => StrPart::Expr(Box::new(self.rexpr(*e))),
                lit => lit,
            })
            .collect()
    }

    pub(super) fn rstmt(&mut self, s: Stmt) -> Stmt {
        match s {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => {
                // A typed declaration is an annotation position: `App app = inject();` draws its target
                // from `ty` (slice 2). `var app = …` (`ty` is `Type::Infer`) is not an annotation and is
                // stripped inside `rexpr_expected`.
                let init = self.rexpr_expected(init, Some(&ty));
                Stmt::VarDecl {
                    ty,
                    name,
                    init,
                    mutable,
                    span,
                }
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => Stmt::Assign {
                target: self.rexpr(target),
                value: self.rexpr(value),
                span,
            },
            Stmt::Return { value, span } => Stmt::Return {
                // A `return` draws its annotation from the enclosing function/method/lambda return type.
                value: value.map(|e| {
                    let expected = self.current_ret.clone();
                    self.rexpr_expected(e, expected.as_ref())
                }),
                span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: self.rexpr(cond),
                bind,
                then_block: self.rblock(then_block),
                else_block: else_block.map(|b| self.rblock(b)),
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
                iter: self.rexpr(iter),
                body: self.rblock(body),
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
                init: self.rexpr(init),
                body: self.rblock(body),
                span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: self.rexpr(cond),
                body: self.rblock(body),
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
                init: init.map(|s| Box::new(self.rstmt(*s))),
                cond: cond.map(|e| self.rexpr(e)),
                step: step.map(|s| Box::new(self.rstmt(*s))),
                body: self.rblock(body),
                span,
            },
            Stmt::Block(stmts, span) => Stmt::Block(self.rblock(stmts), span),
            Stmt::Expr(e, span) => Stmt::Expr(self.rexpr(e), span),
            Stmt::Discard(e, span) => Stmt::Discard(self.rexpr(e), span),
            Stmt::Throw { value, span } => Stmt::Throw {
                value: self.rexpr(value),
                span,
            },
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: self.rblock(body),
                catches: catches
                    .into_iter()
                    .map(|c| CatchClause {
                        ty: c.ty,
                        name: c.name,
                        body: self.rblock(c.body),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block.map(|b| self.rblock(b)),
                span,
            },
            // DEC-356: `Destructure` BEARS an expression (its initializer) and an optional else-block,
            // and the old `leaf => leaf` arm swallowed both — so `inject<T>()` inside a destructuring
            // initializer was never desugared by this pass. `desugar_db` walks it correctly three files
            // away, which is what made the omission invisible.
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => Stmt::Destructure {
                pat,
                init: self.rexpr(init),
                else_block: else_block.map(|b| self.rblock(b)),
                span,
            },
            // `break`/`continue` carry nothing — single-sourced in `ast::leaves` (DEC-356).
            s @ crate::stmt_leaves!() => s,
        }
    }
}

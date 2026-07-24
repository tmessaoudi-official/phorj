use super::*;

/// DEC-331 D9 — lower the two attribute-method sugars into ordinary method calls, on the LIVE
/// (fully-desugared, fully-filled) AST, so no backend needs to know about `#[Invoke]`/`#[ToString]`.
/// `invoke` (keyed by a `Call` node's `Span.start`): `x(args)` → `x.<chosen>(args)`, the method the
/// checker already resolved. `tostring` (keyed by any expression's `Span.start`): an object in string
/// context — an interpolation hole or a `Conversion.toString` argument — → `<expr>.<chosen>()`.
/// Runs OUTERMOST in `cli::check_and_expand_reified` on the final nodes (never a check-time clone —
/// which would drop a later default-fill, so these can't ride `rewrite_ufcs`; see `rewrite_html`).
/// Bottom-up: children first, THEN this node — so `"{x(5)}"` (both maps, one span) becomes
/// `x.add(5)` (invoke) then `(x.add(5)).toStr()` (tostring). Both maps empty ⇒ program untouched.
pub fn resolve_invoke_tostring(
    program: Program,
    invoke: &HashMap<usize, String>,
    tostring: &HashMap<usize, String>,
) -> Program {
    use crate::ast::{ClassMember, Expr, Item, LambdaBody, MatchArm, MemberSep, Stmt, StrPart};
    if invoke.is_empty() && tostring.is_empty() {
        return program;
    }
    type Names = HashMap<usize, String>;

    // Wrap `recv` (already child-rewritten) into `recv.<method>()`, reusing `full` as the synthesized
    // span (post-check — spans only feed diagnostics, which never fire on synthesized nodes).
    fn wrap_tostring(recv: Expr, method: &str, full: Span) -> Expr {
        Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(recv),
                name: method.to_string(),
                safe: false,
                sep: MemberSep::Dot,
                span: full,
            }),
            args: Vec::new(),
            type_args: Vec::new(),
            span: full,
        }
    }

    fn rexpr(e: Expr, inv: &Names, ts: &Names) -> Expr {
        let full = Checker::expr_span(&e);
        let key = full.start;
        // 1) rewrite children + apply the `#[Invoke]` call rewrite (only meaningful on a `Call`).
        let rewritten = match e {
            Expr::Call {
                callee,
                args,
                type_args,
                span,
            } => {
                let callee = Box::new(rexpr(*callee, inv, ts));
                let args = args.into_iter().map(|a| rexpr(a, inv, ts)).collect();
                match inv.get(&span.start) {
                    // `x(args)` → `x.<method>(args)`: the resolved callee becomes the receiver of a
                    // member call; the turbofish (if any) belongs to the invoke method call.
                    Some(method) => Expr::Call {
                        callee: Box::new(Expr::Member {
                            object: callee,
                            name: method.clone(),
                            safe: false,
                            sep: MemberSep::Dot,
                            span,
                        }),
                        args,
                        type_args,
                        span,
                    },
                    None => Expr::Call {
                        callee,
                        args,
                        type_args,
                        span,
                    },
                }
            }
            Expr::Str(parts, span) => Expr::Str(
                parts
                    .into_iter()
                    .map(|p| match p {
                        StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, inv, ts))),
                        lit => lit,
                    })
                    .collect(),
                span,
            ),
            Expr::List(items, span) => {
                Expr::List(items.into_iter().map(|e| rexpr(e, inv, ts)).collect(), span)
            }
            Expr::Tuple(items, span) => {
                Expr::Tuple(items.into_iter().map(|e| rexpr(e, inv, ts)).collect(), span)
            }
            Expr::Map(pairs, span) => Expr::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| (rexpr(k, inv, ts), rexpr(v, inv, ts)))
                    .collect(),
                span,
            ),
            Expr::NamedArg { name, value, span } => Expr::NamedArg {
                name,
                value: Box::new(rexpr(*value, inv, ts)),
                span,
            },
            Expr::Unary { op, expr, span } => Expr::Unary {
                op,
                expr: Box::new(rexpr(*expr, inv, ts)),
                span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op,
                lhs: Box::new(rexpr(*lhs, inv, ts)),
                rhs: Box::new(rexpr(*rhs, inv, ts)),
                span,
            },
            Expr::InstanceOf {
                value,
                type_name,
                span,
            } => Expr::InstanceOf {
                value: Box::new(rexpr(*value, inv, ts)),
                type_name,
                span,
            },
            Expr::Cast {
                value,
                type_name,
                span,
            } => Expr::Cast {
                value: Box::new(rexpr(*value, inv, ts)),
                type_name,
                span,
            },
            Expr::Member {
                object,
                name,
                safe,
                sep,
                span,
            } => Expr::Member {
                object: Box::new(rexpr(*object, inv, ts)),
                name,
                safe,
                sep,
                span,
            },
            Expr::Index {
                object,
                index,
                span,
            } => Expr::Index {
                object: Box::new(rexpr(*object, inv, ts)),
                index: Box::new(rexpr(*index, inv, ts)),
                span,
            },
            Expr::Force { inner, span } => Expr::Force {
                inner: Box::new(rexpr(*inner, inv, ts)),
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
                args: args.into_iter().map(|a| rexpr(a, inv, ts)).collect(),
                span,
            },
            Expr::OverloadSelect { ty, call, span } => Expr::OverloadSelect {
                ty,
                call: Box::new(rexpr(*call, inv, ts)),
                span,
            },
            Expr::Propagate { inner, span } => Expr::Propagate {
                inner: Box::new(rexpr(*inner, inv, ts)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: Box::new(rexpr(*scrutinee, inv, ts)),
                arms: arms
                    .into_iter()
                    .map(|a| MatchArm {
                        pattern: a.pattern,
                        guard: a.guard.map(|g| rexpr(g, inv, ts)),
                        body: rexpr(a.body, inv, ts),
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
                start: Box::new(rexpr(*start, inv, ts)),
                end: Box::new(rexpr(*end, inv, ts)),
                inclusive,
                span,
            },
            Expr::If {
                cond,
                then_expr,
                else_expr,
                span,
            } => Expr::If {
                cond: Box::new(rexpr(*cond, inv, ts)),
                then_expr: Box::new(rexpr(*then_expr, inv, ts)),
                else_expr: Box::new(rexpr(*else_expr, inv, ts)),
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
                    LambdaBody::Expr(e) => LambdaBody::Expr(Box::new(rexpr(*e, inv, ts))),
                    LambdaBody::Block(stmts) => LambdaBody::Block(rblock(stmts, inv, ts)),
                },
                span,
            },
            Expr::CloneWith {
                object,
                fields,
                span,
            } => Expr::CloneWith {
                object: Box::new(rexpr(*object, inv, ts)),
                fields: fields
                    .into_iter()
                    .map(|(n, e)| (n, rexpr(e, inv, ts)))
                    .collect(),
                span,
            },
            Expr::Spawn { call, span } => Expr::Spawn {
                call: Box::new(rexpr(*call, inv, ts)),
                span,
            },
            Expr::New(inner, span) => Expr::New(Box::new(rexpr(*inner, inv, ts)), span),
            // `html"…"`/tagged-template holes may still theoretically carry an interpolated object;
            // recurse the parts so any recorded target inside resolves (defensive — these are
            // normally erased before this pass). Reuses the `Str` shape.
            Expr::Html(parts, span) => Expr::Html(
                parts
                    .into_iter()
                    .map(|p| match p {
                        StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, inv, ts))),
                        lit => lit,
                    })
                    .collect(),
                span,
            ),
            Expr::TaggedTemplate { tag, parts, span } => Expr::TaggedTemplate {
                tag,
                parts: parts
                    .into_iter()
                    .map(|p| match p {
                        StrPart::Expr(e) => StrPart::Expr(Box::new(rexpr(*e, inv, ts))),
                        lit => lit,
                    })
                    .collect(),
                span,
            },
            Expr::Pipe { lhs, rhs, span } => Expr::Pipe {
                lhs: Box::new(rexpr(*lhs, inv, ts)),
                rhs: Box::new(rexpr(*rhs, inv, ts)),
                span,
            },
            // Leaves: no nested expression, and none is a `#[ToString]` target in practice (a target
            // is a value expression, checked below regardless via `ts.get(&key)`).
            leaf @ (Expr::Int(..)
            | Expr::Float(..)
            | Expr::Decimal { .. }
            | Expr::Bool(..)
            | Expr::Null(..)
            | Expr::Bytes(..)
            | Expr::Ident(..)
            | Expr::This(..)
            | Expr::NewColl { .. }
            | Expr::Inject { .. }
            | Expr::PipePlaceholder(_)) => leaf,
        };
        // 2) apply the `#[ToString]` wrap if THIS node's original span is a recorded string-context
        //    target (an interpolation hole or a `Conversion.toString` argument).
        match ts.get(&key) {
            Some(method) => wrap_tostring(rewritten, method, full),
            None => rewritten,
        }
    }

    fn rstmt(s: Stmt, inv: &Names, ts: &Names) -> Stmt {
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
                init: rexpr(init, inv, ts),
                mutable,
                span,
            },
            Stmt::Assign {
                target,
                value,
                span,
            } => Stmt::Assign {
                target: rexpr(target, inv, ts),
                value: rexpr(value, inv, ts),
                span,
            },
            Stmt::Return { value, span } => Stmt::Return {
                value: value.map(|e| rexpr(e, inv, ts)),
                span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: rexpr(cond, inv, ts),
                bind,
                then_block: rblock(then_block, inv, ts),
                else_block: else_block.map(|b| rblock(b, inv, ts)),
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
                iter: rexpr(iter, inv, ts),
                body: rblock(body, inv, ts),
                span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: rexpr(cond, inv, ts),
                body: rblock(body, inv, ts),
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
                init: init.map(|s| Box::new(rstmt(*s, inv, ts))),
                cond: cond.map(|e| rexpr(e, inv, ts)),
                step: step.map(|s| Box::new(rstmt(*s, inv, ts))),
                body: rblock(body, inv, ts),
                span,
            },
            Stmt::Break(span) => Stmt::Break(span),
            Stmt::Continue(span) => Stmt::Continue(span),
            Stmt::Block(stmts, span) => Stmt::Block(rblock(stmts, inv, ts), span),
            Stmt::Expr(e, span) => Stmt::Expr(rexpr(e, inv, ts), span),
            Stmt::Discard(e, span) => Stmt::Discard(rexpr(e, inv, ts), span),
            Stmt::Throw { value, span } => Stmt::Throw {
                value: rexpr(value, inv, ts),
                span,
            },
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: rblock(body, inv, ts),
                catches: catches
                    .into_iter()
                    .map(|c| crate::ast::CatchClause {
                        ty: c.ty,
                        name: c.name,
                        body: rblock(c.body, inv, ts),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block.map(|b| rblock(b, inv, ts)),
                span,
            },
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => Stmt::Destructure {
                pat,
                init: rexpr(init, inv, ts),
                else_block: else_block.map(|b| rblock(b, inv, ts)),
                span,
            },
        }
    }

    fn rblock(stmts: Vec<Stmt>, inv: &Names, ts: &Names) -> Vec<Stmt> {
        stmts.into_iter().map(|s| rstmt(s, inv, ts)).collect()
    }

    // Rewrite every member that can hold an invoke/tostring site. Shared by classes AND traits (a
    // trait flattens into using classes and its bodies reach both backends). Method/ctor/hook bodies
    // AND field initializers are all walked — a field init like `string s = "{obj}";` records a
    // `#[ToString]` target at check time, so it MUST be lowered here too (else interp/VM fault while
    // the PHP leg's `__toString` prints — a byte-identity break; the sibling passes recurse fields too).
    fn rmembers(members: &mut [ClassMember], inv: &Names, ts: &Names) {
        for m in members {
            match m {
                ClassMember::Method(f) => {
                    let body = std::mem::take(&mut f.body);
                    f.body = rblock(body, inv, ts);
                }
                ClassMember::Constructor { body, .. } => {
                    let b = std::mem::take(body);
                    *body = rblock(b, inv, ts);
                }
                ClassMember::Hook { get, set, .. } => {
                    if let Some(e) = get.take() {
                        *get = Some(rexpr(e, inv, ts));
                    }
                    if let Some((p, body)) = set.take() {
                        *set = Some((p, rblock(body, inv, ts)));
                    }
                }
                ClassMember::Field { init, .. } => {
                    if let Some(e) = init.take() {
                        *init = Some(rexpr(e, inv, ts));
                    }
                }
            }
        }
    }

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                f.body = rblock(f.body, invoke, tostring);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                rmembers(&mut c.members, invoke, tostring);
                Item::Class(c)
            }
            Item::Trait(mut t) => {
                rmembers(&mut t.members, invoke, tostring);
                Item::Trait(t)
            }
            other => other,
        })
        .collect();

    Program {
        package: program.package,
        items,
        span: program.span,
    }
}

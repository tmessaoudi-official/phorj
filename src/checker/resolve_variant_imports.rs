//! Wave B B-2c part 2 (DEC-186) — resolve **imported injected-enum variants** to their qualified form
//! BEFORE the checker runs, so a bare (or `as`-aliased) variant brought in by `import Core.Result.Success;`
//! / `import Core.Option.None as Nothing;` / a group `import Core.Result.{ Success, Failure as Xzs };`
//! becomes the ordinary qualified `Enum.Variant` the rest of the pipeline already handles byte-identically.
//!
//! **Why rewrite to qualified rather than teach the resolver a third form:** the qualified construction
//! (`new Result.Success(v)`) and qualified pattern (`Result.Success(v) =>`) paths are already proven and
//! byte-identical across run/runvm/PHP (variant-qualification A2/B). Rewriting every imported use into that
//! form means we reuse them wholesale — no new resolution site, no bespoke bare-Ident→variant backend
//! rename, and `unwrap_new` still collapses the `Enum.Variant` callee to the bare variant for the backend.
//!
//! Two positions carry a variant name:
//!   * construction — `new X(args)` is `Expr::New(Call { callee: Ident(x), .. })`; when `x` is imported,
//!     the callee `Ident` becomes `Member { Ident(enum), realvariant }` (the qualified form).
//!   * `match` pattern — `Pattern::Variant { name: x, enum_qualifier: None, .. }` (the PARENS form
//!     `X(..)` / `X()`); when `x` is imported it gains `enum_qualifier = Some(enum)` and `name` becomes the
//!     real variant. A BARE zero-payload identifier `X =>` (no parens) parses as `Pattern::Binding` (a
//!     catch-all) — the existing zero-payload-needs-parens rule, identical for user/qualified/imported
//!     variants — so it is deliberately left untouched.
//!
//! Runs in [`crate::cli::check_and_expand_reified`] after prelude injection + qualifier collapse and before
//! `check_resolutions` — the single chokepoint every vm-compile path shares (Invariant 6). A no-op unless
//! at least one variant import is present, so programs without them are byte-for-byte unchanged.

use crate::ast::{
    ClassMember, Expr, FieldPat, Item, LambdaBody, MatchArm, Pattern, Program, Stmt, StrPart,
};
use std::collections::HashMap;

/// bound name (the `as` alias, else the variant leaf) → (enum name, real variant name).
type VarMap = HashMap<String, (String, String)>;

/// The raw variant-import bindings a program declares: for each `import Core.<Enum>.<Variant> [as A];`
/// whose `<Enum>` is an injected enum in this program that owns `<Variant>`, one `(bound, enum, variant)`.
/// Shared by the rewrite here AND the checker's collision check (`collect`) so the two never diverge on
/// what counts as a variant import. Non-matching Core paths (`Core.Http.Router` — a member TYPE import,
/// `Core.Output.printLine` — a function) yield nothing here and are handled by the existing import maps.
pub(crate) fn variant_import_bindings(items: &[Item]) -> Vec<(String, String, String)> {
    // The program's enums (post-injection) and their variant sets, for validation.
    let mut variants: HashMap<&str, std::collections::HashSet<&str>> = HashMap::new();
    for it in items {
        if let Item::Enum(e) = it {
            variants
                .entry(e.name.as_str())
                .or_default()
                .extend(e.variants.iter().map(|v| v.name.as_str()));
        }
    }
    let mut out = Vec::new();
    for it in items {
        if let Item::Import { path, alias, .. } = it {
            if path.len() == 3 && path[0] == "Core" {
                let (enum_name, variant) = (&path[1], &path[2]);
                if variants
                    .get(enum_name.as_str())
                    .is_some_and(|vs| vs.contains(variant.as_str()))
                {
                    let bound = alias.clone().unwrap_or_else(|| variant.clone());
                    out.push((bound, enum_name.clone(), variant.clone()));
                }
            }
        }
    }
    out
}

/// Rewrite every imported-variant use to its qualified form. A no-op when no variant imports are present.
pub fn resolve_variant_imports(program: Program) -> Program {
    let bindings = variant_import_bindings(&program.items);
    if bindings.is_empty() {
        return program;
    }
    // A name that also denotes a top-level item (class/enum/interface/trait/function) OR a variant of a
    // USER (non-injected) enum is a collision — left UNresolved here so it is never silently mis-rewritten
    // (else `import Core.Result.Success;` would hijack a local `enum Local { Success(..) }`'s bare
    // `new Success(..)`); the checker's `check_variant_import_collisions` then reports `E-IMPORT-CONFLICT`
    // and compilation stops. Injected enums are exempt from the variant side — their variants are exactly
    // what a variant import binds. Two imports binding the same name likewise drop out (kept only if
    // unique), so an ambiguous bare name is never rewritten to one arbitrary target.
    let mut local: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for it in &program.items {
        match it {
            Item::Class(c) => {
                local.insert(c.name.as_str());
            }
            Item::Enum(e) => {
                local.insert(e.name.as_str());
                if !e.injected {
                    local.extend(e.variants.iter().map(|v| v.name.as_str()));
                }
            }
            Item::Interface(i) => {
                local.insert(i.name.as_str());
            }
            Item::Trait(t) => {
                local.insert(t.name.as_str());
            }
            Item::Function(f) => {
                local.insert(f.name.as_str());
            }
            _ => {}
        }
    }
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for (b, _, _) in &bindings {
        *seen.entry(b.as_str()).or_default() += 1;
    }
    let map: VarMap = bindings
        .iter()
        .filter(|(b, _, _)| seen[b.as_str()] == 1 && !local.contains(b.as_str()))
        .map(|(b, e, v)| (b.clone(), (e.clone(), v.clone())))
        .collect();
    if map.is_empty() {
        return program;
    }

    let items = program
        .items
        .into_iter()
        .map(|item| match item {
            Item::Function(mut f) => {
                f.body = rblock(f.body, &map);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                for m in &mut c.members {
                    match m {
                        ClassMember::Method(f) => {
                            let body = std::mem::take(&mut f.body);
                            f.body = rblock(body, &map);
                        }
                        ClassMember::Constructor { body, .. } => {
                            let b = std::mem::take(body);
                            *body = rblock(b, &map);
                        }
                        ClassMember::Hook { get, set, .. } => {
                            if let Some(e) = get.take() {
                                *get = Some(rexpr(e, &map));
                            }
                            if let Some((p, body)) = set.take() {
                                *set = Some((p, rblock(body, &map)));
                            }
                        }
                        ClassMember::Field { init, .. } => {
                            if let Some(e) = init.take() {
                                *init = Some(rexpr(e, &map));
                            }
                        }
                    }
                }
                Item::Class(c)
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

/// Rewrite a pattern: qualify an imported `Pattern::Variant` head, recurse into nested sub-patterns.
fn rpat(p: Pattern, m: &VarMap) -> Pattern {
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
        // A bare identifier (`X =>`) is a catch-all binding, NOT a variant (the existing zero-payload rule)
        // — deliberately not rewritten. Literals/wildcard/type patterns carry no variant head.
        leaf => leaf,
    }
}

fn rexpr(e: Expr, m: &VarMap) -> Expr {
    match e {
        // The one rewrite site for construction: `new X(args)` → `new Enum.Variant(args)` when `X` is an
        // imported variant. Recurse the inner call first (nested `new`s / args), then qualify the callee;
        // the `New` wrapper SURVIVES (the checker needs it; `unwrap_new` strips it post-check).
        Expr::New(inner, span) => {
            let inner = rexpr(*inner, m);
            if let Expr::Call {
                callee,
                args,
                span: cspan,
            } = inner
            {
                let callee = match *callee {
                    Expr::Ident(name, isp) => match m.get(&name) {
                        Some((enum_name, real)) => Box::new(Expr::Member {
                            object: Box::new(Expr::Ident(enum_name.clone(), isp)),
                            name: real.clone(),
                            safe: false,
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
                        span: cspan,
                    }),
                    span,
                )
            } else {
                Expr::New(Box::new(inner), span)
            }
        }
        Expr::Call { callee, args, span } => Expr::Call {
            callee: Box::new(rexpr(*callee, m)),
            args: args.into_iter().map(|a| rexpr(a, m)).collect(),
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
            span,
        } => Expr::Member {
            object: Box::new(rexpr(*object, m)),
            name,
            safe,
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
            body,
            span,
        } => Expr::Lambda {
            params,
            ret,
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
        // leaves carry no nested expression: Int / Float / Bool / Null / Bytes / Ident / This
        leaf => leaf,
    }
}

fn rstmt(s: Stmt, m: &VarMap) -> Stmt {
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

fn rblock(stmts: Vec<Stmt>, m: &VarMap) -> Vec<Stmt> {
    stmts.into_iter().map(|s| rstmt(s, m)).collect()
}

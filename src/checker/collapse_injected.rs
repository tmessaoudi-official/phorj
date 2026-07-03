use super::*;

/// Import-redesign S1 — collapse a **qualified injected-type reference** in TYPE-ANNOTATION position
/// (`Http.Router`, `Time.Duration`, `Decimal.RoundingMode`) down to its bare injected type name, so the
/// checker and every backend see the plain `Router` / `Duration` / `RoundingMode` the injection preludes
/// declare. Runs AFTER the six preludes are injected and `desugar_auto_router`, BEFORE `check` — the
/// single chokepoint [`crate::cli::check_and_expand_reified`] wires it, covering run/runvm/transpile/
/// disassemble/benchmark/playground alike (Invariant 6).
///
/// The registry is static and mirrors the preludes' multi-type modules (single-type modules
/// `Json`/`Regex`/`Secret` are leaf==type and need no qualifier). A `Qual.Member` whose `(Qual, Member)`
/// pair is registered is rewritten to bare `Member`; any other dotted name is left untouched (it will
/// fail type resolution exactly as before — this pass never invents a type). The transpiler thus emits
/// bare PHP (`new Router()`), because by transpile time the qualifier is already gone.
///
/// Modeled deliberately on [`super::expand_aliases`] (same rt/rparam/rstmt/rfunc/rmember walk + Item
/// assembly) but with two differences: (a) `Item::TypeAlias` is KEPT (this pass runs before `check`,
/// which still needs the alias declarations), its target type rewritten in place; (b) the `rt` Named
/// rule collapses registered qualifiers instead of expanding aliases. Expr nodes carry no `Type`, so
/// they clone unchanged (identical to `expand_aliases`).
pub fn collapse_injected_type_qualifiers(program: Program) -> Program {
    use crate::ast::{
        ClassDecl, ClassMember, CtorParam, EnumDecl, EnumVariant, FunctionDecl, InterfaceDecl,
        Item, Param, Stmt, Type,
    };

    /// Is `(qual, member)` a registered injected multi-type module member? Mirrors the preludes in
    /// `src/cli/mod.rs`: `Http` → {Request, Response, Route, Router}; `Time` → {Duration, Date,
    /// Instant}; `Decimal` → {RoundingMode}.
    fn is_injected_member(qual: &str, member: &str) -> bool {
        matches!(
            (qual, member),
            ("Http", "Request" | "Response" | "Route" | "Router")
                | ("Time", "Duration" | "Date" | "Instant")
                | ("Decimal", "RoundingMode")
        )
    }

    fn rt(ty: &Type) -> Type {
        match ty {
            Type::Named { name, args, span } => {
                let collapsed = name
                    .split_once('.')
                    .filter(|(q, m)| is_injected_member(q, m))
                    .map(|(_, m)| m.to_string())
                    .unwrap_or_else(|| name.clone());
                Type::Named {
                    name: collapsed,
                    args: args.iter().map(rt).collect(),
                    span: *span,
                }
            }
            Type::Optional { inner, span } => Type::Optional {
                inner: Box::new(rt(inner)),
                span: *span,
            },
            Type::Function { params, ret, span } => Type::Function {
                params: params.iter().map(rt).collect(),
                ret: Box::new(rt(ret)),
                span: *span,
            },
            Type::Union(members, span) => Type::Union(members.iter().map(rt).collect(), *span),
            Type::Intersection(members, span) => {
                Type::Intersection(members.iter().map(rt).collect(), *span)
            }
            Type::FixedList { elem, len, span } => Type::FixedList {
                elem: Box::new(rt(elem)),
                len: *len,
                span: *span,
            },
            Type::Infer(s) => Type::Infer(*s),
            Type::Erased(s) => Type::Erased(*s),
        }
    }
    fn rparam(p: &Param) -> Param {
        Param {
            ty: rt(&p.ty),
            name: p.name.clone(),
            default: p.default.clone(),
            span: p.span,
        }
    }
    fn rstmt(s: &Stmt) -> Stmt {
        match s {
            Stmt::VarDecl {
                ty,
                name,
                init,
                mutable,
                span,
            } => Stmt::VarDecl {
                ty: rt(ty),
                name: name.clone(),
                init: init.clone(),
                mutable: *mutable,
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
                ty: rt(ty),
                name: name.clone(),
                val: val.as_ref().map(|(t, n)| (rt(t), n.clone())),
                iter: iter.clone(),
                body: body.iter().map(rstmt).collect(),
                span: *span,
            },
            Stmt::If {
                cond,
                bind,
                then_block,
                else_block,
                span,
            } => Stmt::If {
                cond: cond.clone(),
                bind: bind.clone(),
                then_block: then_block.iter().map(rstmt).collect(),
                else_block: else_block.as_ref().map(|b| b.iter().map(rstmt).collect()),
                span: *span,
            },
            Stmt::While {
                cond,
                body,
                post_cond,
                span,
            } => Stmt::While {
                cond: cond.clone(),
                body: body.iter().map(rstmt).collect(),
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
                init: init.as_ref().map(|s| Box::new(rstmt(s))),
                cond: cond.clone(),
                step: step.as_ref().map(|s| Box::new(rstmt(s))),
                body: body.iter().map(rstmt).collect(),
                span: *span,
            },
            Stmt::Block(stmts, span) => Stmt::Block(stmts.iter().map(rstmt).collect(), *span),
            Stmt::Try {
                body,
                catches,
                finally_block,
                span,
            } => Stmt::Try {
                body: body.iter().map(rstmt).collect(),
                catches: catches
                    .iter()
                    .map(|c| crate::ast::CatchClause {
                        ty: rt(&c.ty),
                        name: c.name.clone(),
                        body: c.body.iter().map(rstmt).collect(),
                        span: c.span,
                    })
                    .collect(),
                finally_block: finally_block
                    .as_ref()
                    .map(|b| b.iter().map(rstmt).collect()),
                span: *span,
            },
            Stmt::Destructure {
                pat,
                init,
                else_block,
                span,
            } => Stmt::Destructure {
                pat: pat.clone(),
                init: init.clone(),
                else_block: else_block.as_ref().map(|b| b.iter().map(rstmt).collect()),
                span: *span,
            },
            Stmt::Throw { .. }
            | Stmt::Return { .. }
            | Stmt::Expr(..)
            | Stmt::Discard(..)
            | Stmt::Assign { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_) => s.clone(),
        }
    }
    fn rfunc(f: &FunctionDecl) -> FunctionDecl {
        FunctionDecl {
            modifiers: f.modifiers.clone(),
            attrs: f.attrs.clone(),
            vis: f.vis,
            name: f.name.clone(),
            type_params: f.type_params.clone(),
            params: f.params.iter().map(rparam).collect(),
            ret: f.ret.as_ref().map(rt),
            throws: f.throws.iter().map(rt).collect(),
            body: f.body.iter().map(rstmt).collect(),
            foreign: f.foreign,
            generic_ret_from_param: f.generic_ret_from_param,
            span: f.span,
        }
    }
    fn rmember(m: &ClassMember) -> ClassMember {
        match m {
            ClassMember::Field {
                modifiers,
                ty,
                name,
                init,
                span,
            } => ClassMember::Field {
                modifiers: modifiers.clone(),
                ty: rt(ty),
                name: name.clone(),
                init: init.clone(),
                span: *span,
            },
            ClassMember::Constructor {
                modifiers,
                params,
                body,
                span,
            } => ClassMember::Constructor {
                modifiers: modifiers.clone(),
                params: params
                    .iter()
                    .map(|p| CtorParam {
                        modifiers: p.modifiers.clone(),
                        ty: rt(&p.ty),
                        name: p.name.clone(),
                        span: p.span,
                    })
                    .collect(),
                body: body.iter().map(rstmt).collect(),
                span: *span,
            },
            ClassMember::Method(f) => ClassMember::Method(rfunc(f)),
            ClassMember::Hook {
                ty,
                name,
                get,
                set,
                span,
            } => ClassMember::Hook {
                ty: rt(ty),
                name: name.clone(),
                get: get.clone(),
                set: set.as_ref().map(|(p, b)| {
                    (
                        Param {
                            ty: rt(&p.ty),
                            name: p.name.clone(),
                            default: p.default.clone(),
                            span: p.span,
                        },
                        b.iter().map(rstmt).collect(),
                    )
                }),
                span: *span,
            },
        }
    }

    let items = program
        .items
        .iter()
        .map(|item| match item {
            // KEEP the alias declaration (this pass runs before `check`), but collapse any qualifier in
            // its target type so `type R = Http.Router;` resolves.
            Item::TypeAlias { name, ty, span } => Item::TypeAlias {
                name: name.clone(),
                ty: rt(ty),
                span: *span,
            },
            Item::Import { .. } => item.clone(),
            Item::Function(f) => Item::Function(rfunc(f)),
            Item::Class(c) => Item::Class(ClassDecl {
                vis: c.vis,
                name: c.name.clone(),
                type_params: c.type_params.clone(),
                extends: c.extends.clone(),
                implements: c.implements.clone(),
                open: c.open,
                is_abstract: c.is_abstract,
                resolutions: c.resolutions.clone(),
                uses: c.uses.clone(),
                members: c.members.iter().map(rmember).collect(),
                foreign: c.foreign,
                span: c.span,
            }),
            Item::Trait(t) => Item::Trait(crate::ast::TraitDecl {
                name: t.name.clone(),
                members: t.members.iter().map(rmember).collect(),
                span: t.span,
            }),
            Item::Interface(i) => Item::Interface(InterfaceDecl {
                vis: i.vis,
                name: i.name.clone(),
                extends: i.extends.clone(),
                methods: i.methods.iter().map(rfunc).collect(),
                span: i.span,
            }),
            Item::Enum(e) => Item::Enum(EnumDecl {
                vis: e.vis,
                name: e.name.clone(),
                type_params: e.type_params.clone(),
                variants: e
                    .variants
                    .iter()
                    .map(|v| EnumVariant {
                        name: v.name.clone(),
                        fields: v.fields.iter().map(rparam).collect(),
                        span: v.span,
                    })
                    .collect(),
                injected: e.injected,
                span: e.span,
            }),
            Item::Test { name, body, span } => Item::Test {
                name: name.clone(),
                body: body.iter().map(rstmt).collect(),
                span: *span,
            },
        })
        .collect();

    Program {
        package: program.package.clone(),
        items,
        span: program.span,
    }
}

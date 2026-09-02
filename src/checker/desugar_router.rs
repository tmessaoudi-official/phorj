//! `Http.autoRouter()` compile-time desugar (M6 W2).
//!
//! Collects every free function annotated `#[Route("METHOD", "/pattern")]` (in source order) and
//! rewrites each `Http.autoRouter()` call into an explicit router construction —
//! `new Router([]).route("M1", "/p1", fn1).route("M2", "/p2", fn2) …` — referencing each handler as a
//! first-class function value. This runs in `cli::check_and_expand`'s injection chain **before** the
//! type-checker, so the generated registration is type-checked like hand-written code and every
//! backend sees the *same* explicit `.route(…)` chain (the expand-before-backends discipline ⇒
//! byte-identity is trivial, with no runtime attribute machinery). The `#[Route]` attributes stay on
//! the functions for the checker's validation pass, then are inert for the backends.
//!
//! Loop-safe by construction: the synthesized router expression contains only `new`/`.route(…)` calls
//! — never an `Http.autoRouter()` — so re-walking it can match nothing. The walker mirrors
//! `rewrite_ufcs::rexpr` (the proven complete Expr/Stmt walk); the one behavioural difference is the
//! `Expr::Call` arm, which substitutes a freshly built router for an `Http.autoRouter()` shape.

use crate::ast::{
    CatchClause, ClassMember, CollKind, Expr, Item, LambdaBody, MatchArm, Modifier, Param, Program,
    Stmt, StrPart, Type,
};
use crate::token::Span;

/// One collected route: the `#[Route]` method literal, the pattern literal (both kept as the original
/// argument `Expr`s, so a raw-string pattern survives), and the **handler expression** — a bare
/// `Ident` for a free function, or a `function(Request req) => Class.method(req)` lambda for a (static)
/// method (M6 W2-ext slice 3).
type Route = (Expr, Expr, Expr);

/// Rewrite `Http.autoRouter()` calls into explicit `Router` construction. A no-op (returns the program
/// unchanged) unless `Core.Http` is imported — so a user's own unrelated `Http.autoRouter()` is never
/// touched when the web layer isn't in play.
pub fn desugar_auto_router(program: Program) -> Program {
    let imports_http = program.items.iter().any(|it| {
        matches!(it, Item::Import { path, .. }
            if path.len() == 2 && path[0] == "Core" && path[1] == "Http")
    });
    if !imports_http {
        return program;
    }
    let routes = collect_routes(&program);

    /// Desugar `Http.autoRouter()` across a member list — shared by the class and trait arms, which
    /// carry the identical `Vec<ClassMember>` (CD-31).
    fn rmembers(members: &mut [ClassMember], routes: &[Route]) {
        for m in members {
            match m {
                ClassMember::Method(f) => {
                    let body = std::mem::take(&mut f.body);
                    f.body = rblock(body, routes);
                }
                ClassMember::Constructor { body, .. } => {
                    let b = std::mem::take(body);
                    *body = rblock(b, routes);
                }
                ClassMember::Hook { get, set, .. } => {
                    if let Some(e) = get.take() {
                        *get = Some(rexpr(e, routes));
                    }
                    if let Some((p, body)) = set.take() {
                        *set = Some((p, rblock(body, routes)));
                    }
                }
                ClassMember::Field { init, .. } => {
                    if let Some(e) = init.take() {
                        *init = Some(rexpr(e, routes));
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
                f.body = rblock(f.body, &routes);
                Item::Function(f)
            }
            Item::Class(mut c) => {
                rmembers(&mut c.members, &routes);
                Item::Class(c)
            }
            // CD-31: a trait's members execute (they flatten into the using class), so an
            // `Http.autoRouter()` written in a trait method must be desugared like one in a class.
            // NOTE the asymmetry this arm does NOT resolve: `collect_routes` still reads `#[Route]`
            // from free functions and classes only, so a `#[Route]` static declared IN a trait is
            // silently not registered even though it flattens into the using class. Whether it
            // should register is a ruling, not a mechanical fix — recorded in CD-31, not decided here.
            Item::Trait(mut t) => {
                rmembers(&mut t.members, &routes);
                Item::Trait(t)
            }
            // A `test "…" { … }` body is checked and executed like a function body under `phg test`
            // (the panel's reported instance of this class).
            Item::Test { name, body, span } => Item::Test {
                name,
                body: rblock(body, &routes),
                span,
            },
            // An enum's `backing_value` is scalar-checked and an interface's `methods` are
            // signatures, so no `Http.autoRouter()` call can reach either. Named rather than folded
            // into `item_leaves!()` so neither is claimed expression-free (CD-31).
            it @ (Item::Enum(..) | Item::Interface(..)) => it,
            it @ (crate::item_leaves!()) => it,
        })
        .collect();

    Program {
        package: program.package,
        items,
        span: program.span,
    }
}

/// Every well-formed `#[Route(method, pattern)]` handler, in source order: free functions (handler =
/// a bare `Ident`) and **static** class methods (handler = a `function(Request req) => Class.method(req)`
/// lambda). A malformed `Route` (wrong arg count) is skipped — the checker reports `E-ROUTE-ARGS`; a
/// non-`static` `#[Route]` method is skipped here and reported `E-ROUTE-METHOD-STATIC`.
fn collect_routes(program: &Program) -> Vec<Route> {
    let mut out = Vec::new();
    for it in &program.items {
        match it {
            Item::Function(f) => {
                for attr in &f.attrs {
                    if attr.is_route() && attr.args.len() == 2 {
                        let handler = Expr::Ident(f.name.clone(), f.span);
                        out.push((attr.args[0].clone(), attr.args[1].clone(), handler));
                    }
                }
            }
            Item::Class(c) => {
                for m in &c.members {
                    let ClassMember::Method(f) = m else { continue };
                    if !f.modifiers.contains(&Modifier::Static) {
                        continue; // a non-static #[Route] method is an E-ROUTE-METHOD-STATIC error
                    }
                    for attr in &f.attrs {
                        if attr.is_route() && attr.args.len() == 2 {
                            let handler = method_handler_lambda(&c.name, &f.name, f.span);
                            out.push((attr.args[0].clone(), attr.args[1].clone(), handler));
                        }
                    }
                }
            }
            // CD-31: named rather than `_ => {}`, because one of these is an OPEN QUESTION rather
            // than an obvious no-op. A `#[Route]` static declared in a TRAIT flattens into the using
            // class, so arguably it should register — it does not today, and this arm preserves that
            // rather than changing routing behaviour on my own judgement (Invariant 15: a
            // user-visible behaviour change is the developer's call). Enums, interfaces, tests and
            // the two leaves genuinely declare no routable handler.
            Item::Trait(..)
            | Item::Enum(..)
            | Item::Interface(..)
            | Item::Test { .. }
            | crate::item_leaves!() => {}
        }
    }
    out
}

/// `function(Request req) -> Response { return Class.method(req); }` — the handler value for a `#[Route]`
/// static method (a static call isn't itself a first-class value, so it's wrapped in a lambda).
fn method_handler_lambda(class: &str, method: &str, sp: Span) -> Expr {
    let named = |n: &str| Type::Named {
        name: n.to_string(),
        args: Vec::new(),
        span: sp,
    };
    let call = Expr::Call {
        callee: Box::new(Expr::Member {
            object: Box::new(Expr::Ident(class.to_string(), sp)),
            name: method.to_string(),
            safe: false,
            sep: crate::ast::MemberSep::Dot,
            span: sp,
        }),
        args: vec![Expr::Ident("req".to_string(), sp)],
        type_args: Vec::new(),
        span: sp,
    };
    Expr::Lambda {
        params: vec![Param {
            ty: named("Request"),
            name: "req".to_string(),
            default: None,
            variadic: false,
            span: sp,
        }],
        ret: Some(named("Response")),
        throws: Vec::new(),
        body: LambdaBody::Block(vec![Stmt::Return {
            value: Some(call),
            span: sp,
        }]),
        span: sp,
    }
}

/// `new Router([], []).route(m1, p1, h1).route(m2, p2, h2) …` — built fresh at each call site. Each
/// handler `hN` is the collected handler `Expr` (a bare `Ident` for a free function, or a
/// `function(Request req) => Class.method(req)` lambda for a static method). The `new`/`.route` wrapper nodes
/// carry the `Http.autoRouter()` call's span, so a downstream type error points at the call site.
fn build_router(routes: &[Route], sp: Span) -> Expr {
    // `new Router(new List<Route>(), new List<(Request, (Request) -> Response) -> Response>())` —
    // an empty route table + empty middleware list (M6 W2-ext slice 1). DEC-214 part-2: empty
    // collections are CONSTRUCTED (bare `[]` is now `E-EMPTY-LITERAL`), so the type args are spelled
    // to match the `Router` constructor's `List<Route>` / `List<mw>` parameters exactly.
    let named = |n: &str| Type::Named {
        name: n.into(),
        args: Vec::new(),
        span: sp,
    };
    // `(Request) -> Response` (the `next` continuation), then `(Request, next) -> Response` (a mw).
    let next_fn = Type::Function {
        params: vec![named("Request")],
        ret: Box::new(named("Response")),
        throws: Vec::new(),
        span: sp,
    };
    let mw_ty = Type::Function {
        params: vec![named("Request"), next_fn],
        ret: Box::new(named("Response")),
        throws: Vec::new(),
        span: sp,
    };
    let empty_routes = Expr::NewColl {
        kind: CollKind::List,
        args: vec![named("Route")],
        span: sp,
    };
    let empty_mws = Expr::NewColl {
        kind: CollKind::List,
        args: vec![mw_ty],
        span: sp,
    };
    let mut e = Expr::New(
        Box::new(Expr::Call {
            callee: Box::new(Expr::Ident("Router".into(), sp)),
            args: vec![empty_routes, empty_mws],
            type_args: Vec::new(),
            span: sp,
        }),
        sp,
    );
    for (method, pattern, handler) in routes {
        e = Expr::Call {
            callee: Box::new(Expr::Member {
                object: Box::new(e),
                name: "route".into(),
                safe: false,
                sep: crate::ast::MemberSep::Dot,
                span: sp,
            }),
            args: vec![method.clone(), pattern.clone(), handler.clone()],
            type_args: Vec::new(),
            span: sp,
        };
    }
    e
}

/// Is this `callee(args)` an `Http.autoRouter()` (no-arg, the exact `Http.autoRouter` member shape)?
fn is_auto_router(callee: &Expr, args: &[Expr]) -> bool {
    if !args.is_empty() {
        return false;
    }
    matches!(callee, Expr::Member { object, name, safe: false, .. }
        if name == "autoRouter"
            && matches!(object.as_ref(), Expr::Ident(q, _) if q == "Http"))
}

#[path = "desugar_router_walk.rs"]
mod walk;
use walk::*;

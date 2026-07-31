//! DEC-258 — the connection-NAMING fact analysis (`scan_naming_facts`). Split out of `desugar_db.rs`
//! when DEC-364 pushed that file past Invariant 13's cap; wholly self-contained — it takes a function
//! body and returns proven facts, touching no `Db` state.
//!
//! Its statement walk is exhaustive over `Stmt` (Invariant 3 / DEC-356), which matters more here than
//! almost anywhere: the analysis decides when to skip runtime dispatch, so a statement form it fails to
//! consider is a binding it fails to POISON — i.e. a wrong compile-time answer, not a missing one.

use super::desugar_db::{Connection, Naming};
use crate::ast::{Expr, LambdaBody, Param, Stmt};
use std::collections::BTreeMap;

/// DEC-258 — scan one function body (recursively: nested blocks, loops, try/catch, lambda bodies)
/// and return the names PROVEN to hold a `Connection` with a compile-time-known naming strategy.
/// The proof standard is deliberately brutal — a name qualifies only when EVERY binding of it in
/// the whole function is the same immutable literal-strategy construction, and it is never
/// reassigned, never a loop/catch/if-let/destructure binder, and never any function/lambda
/// parameter. Anything else ⇒ absent ⇒ the (always-correct) runtime dispatch tier.
pub(super) fn scan_naming_facts(params: &[Param], body: &[Stmt]) -> BTreeMap<String, Naming> {
    #[derive(Clone, Copy, PartialEq)]
    enum Fact {
        Lit(Naming),
        Poison,
    }
    fn merge(map: &mut BTreeMap<String, Fact>, name: &str, f: Fact) {
        let v = match (map.get(name), f) {
            (None, f) => f,
            (Some(Fact::Lit(a)), Fact::Lit(b)) if *a == b => Fact::Lit(b),
            _ => Fact::Poison,
        };
        map.insert(name.to_string(), v);
    }
    /// The naming carried by a `Connection` construction expr — shared with the receiver-side walk.
    fn ctor_fact(e: &Expr) -> Option<Naming> {
        Connection::inline_ctor_naming(e)
    }
    fn scan_expr(e: &Expr, map: &mut BTreeMap<String, Fact>) {
        if let Expr::Lambda { params, body, .. } = e {
            for p in params {
                merge(map, &p.name, Fact::Poison);
            }
            match body {
                LambdaBody::Expr(inner) => scan_expr(inner, map),
                LambdaBody::Block(stmts) => scan_block(stmts, map),
            }
            return;
        }
        let mut work = Vec::new();
        crate::ast::push_subexprs(e, &mut work);
        for sub in work {
            scan_expr(sub, map);
        }
    }
    fn scan_block(stmts: &[Stmt], map: &mut BTreeMap<String, Fact>) {
        for s in stmts {
            match s {
                Stmt::VarDecl {
                    name,
                    init,
                    mutable,
                    ..
                } => {
                    let fact = match (mutable, ctor_fact(init)) {
                        (false, Some(n)) => Fact::Lit(n),
                        _ => Fact::Poison,
                    };
                    merge(map, name, fact);
                    scan_expr(init, map);
                }
                Stmt::Assign { target, value, .. } => {
                    if let Expr::Ident(n, _) = target {
                        merge(map, n, Fact::Poison);
                    }
                    scan_expr(target, map);
                    scan_expr(value, map);
                }
                Stmt::Return { value, .. } => {
                    if let Some(v) = value {
                        scan_expr(v, map);
                    }
                }
                Stmt::If {
                    cond,
                    bind,
                    then_block,
                    else_block,
                    ..
                } => {
                    if let Some(b) = bind {
                        merge(map, b, Fact::Poison);
                    }
                    scan_expr(cond, map);
                    scan_block(then_block, map);
                    if let Some(eb) = else_block {
                        scan_block(eb, map);
                    }
                }
                Stmt::For {
                    name,
                    val,
                    iter,
                    body,
                    ..
                } => {
                    merge(map, name, Fact::Poison);
                    if let Some((_, v)) = val {
                        merge(map, v, Fact::Poison);
                    }
                    scan_expr(iter, map);
                    scan_block(body, map);
                }
                Stmt::Using {
                    name, init, body, ..
                } => {
                    merge(map, name, Fact::Poison); // a `using` binding is opaque here, like a loop var
                    scan_expr(init, map);
                    scan_block(body, map);
                }
                Stmt::While { cond, body, .. } => {
                    scan_expr(cond, map);
                    scan_block(body, map);
                }
                Stmt::CFor {
                    init,
                    cond,
                    step,
                    body,
                    ..
                } => {
                    if let Some(i) = init {
                        scan_block(std::slice::from_ref(i), map);
                    }
                    if let Some(c) = cond {
                        scan_expr(c, map);
                    }
                    if let Some(st) = step {
                        scan_block(std::slice::from_ref(st), map);
                    }
                    scan_block(body, map);
                }
                Stmt::Break(_) | Stmt::Continue(_) => {}
                Stmt::Block(inner, _) => scan_block(inner, map),
                Stmt::Expr(e, _) | Stmt::Discard(e, _) => scan_expr(e, map),
                Stmt::Throw { value, .. } => scan_expr(value, map),
                Stmt::Try {
                    body,
                    catches,
                    finally_block,
                    ..
                } => {
                    scan_block(body, map);
                    for c in catches {
                        merge(map, &c.name, Fact::Poison);
                        scan_block(&c.body, map);
                    }
                    if let Some(fb) = finally_block {
                        scan_block(fb, map);
                    }
                }
                Stmt::Destructure {
                    pat,
                    init,
                    else_block,
                    ..
                } => {
                    for (b, _) in pat.binders() {
                        merge(map, &b, Fact::Poison);
                    }
                    scan_expr(init, map);
                    if let Some(eb) = else_block {
                        scan_block(eb, map);
                    }
                }
            }
        }
    }
    let mut map = BTreeMap::new();
    for p in params {
        merge(&mut map, &p.name, Fact::Poison);
    }
    scan_block(body, &mut map);
    map.into_iter()
        .filter_map(|(k, v)| match v {
            Fact::Lit(n) => Some((k, n)),
            Fact::Poison => None,
        })
        .collect()
}

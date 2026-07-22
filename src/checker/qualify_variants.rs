//! DEC-329.3 commit B — canonicalize every ENUM-VARIANT USE to its checker-resolved qualified form.
//!
//! Post-check, the AST still carries variant uses in whatever shape the source wrote them: a bare
//! construction is a `Call { callee: Ident(variant) }` (and a *qualified* one was erased BACK to
//! that same bare shape by `unwrap_new`), while a `match` pattern's `enum_qualifier` is `Some` only
//! when the source wrote `Enum.Variant(..)`. Every backend then resolves the bare name through a
//! name-keyed map — which is ambiguous the moment two enums declare the same variant name (the
//! checker's `E-VARIANT-AMBIGUOUS` covers the BARE use, but a legal qualified use of a shared name
//! would still hit the wrong map entry downstream).
//!
//! This pass consumes the checker's `variant_resolutions` side-table (span.start → owning enum,
//! recorded at every construction and pattern site — the reified-operands precedent) and rewrites:
//!   * construction — `Call { callee: Ident(v) }` whose span is in the table becomes the canonical
//!     `Call { callee: Member { Ident(enum), v } }` (the qualified form every backend now keys on);
//!   * pattern — `Pattern::Variant`'s `enum_qualifier` is (over)written with the checker-resolved
//!     enum, so an already-qualified pattern is *canonicalized* too (the checker verified equality,
//!     so this only normalizes spelling — e.g. nothing, today; it future-proofs alias forms).
//!
//! A rewrite fires only when the resolved enum actually OWNS the variant name (defense against a
//! span collision from synthesized nodes); a table miss leaves the node untouched — the backends
//! keep their bare-name fallback path, so a miss degrades to today's behavior, never to a crash.
//! Runs in [`crate::cli::check_and_expand_reified`] OUTERMOST (after `unwrap_new`/`rewrite_ufcs`/
//! `inline_parent_ctors` splice their clones back — clones keep source spans, so the lookups hold).

use crate::ast::{
    CatchClause, ClassMember, Expr, Item, LambdaBody, MemberSep, Pattern, Program, Stmt,
};
use std::collections::{HashMap, HashSet};

struct Ctx<'a> {
    /// span.start of the use-site → the checker-resolved owning enum.
    table: &'a HashMap<usize, String>,
    /// enum name → its declared variant names (rewrite guard: the resolved enum must own the name).
    owns: HashMap<String, HashSet<String>>,
}

impl Ctx<'_> {
    fn resolve(&self, start: usize, variant: &str) -> Option<&String> {
        self.table
            .get(&start)
            .filter(|e| self.owns.get(*e).is_some_and(|vs| vs.contains(variant)))
    }
}

/// Rewrite every resolved variant use to its canonical qualified form (see module docs).
pub fn qualify_variants(mut program: Program, table: &HashMap<usize, String>) -> Program {
    if table.is_empty() {
        return program;
    }
    let mut owns: HashMap<String, HashSet<String>> = HashMap::new();
    for it in &program.items {
        if let Item::Enum(e) = it {
            owns.entry(e.name.clone())
                .or_default()
                .extend(e.variants.iter().map(|v| v.name.clone()));
        }
    }
    let ctx = Ctx { table, owns };
    for item in &mut program.items {
        match item {
            Item::Function(f) => qblock(&mut f.body, &ctx),
            Item::Class(c) => qmembers(&mut c.members, &ctx),
            Item::Trait(t) => qmembers(&mut t.members, &ctx),
            // Enums (backing literals), interfaces, imports, aliases, tests: no variant use-sites
            // reach the backends from these (tests are checker-gated out — same set `unwrap_new`
            // walks).
            _ => {}
        }
    }
    program
}

fn qmembers(members: &mut [ClassMember], ctx: &Ctx) {
    for m in members {
        match m {
            ClassMember::Method(f) => qblock(&mut f.body, ctx),
            ClassMember::Constructor { body, .. } => qblock(body, ctx),
            ClassMember::Field { init, .. } => {
                if let Some(e) = init {
                    qe(e, ctx);
                }
            }
            ClassMember::Hook { get, set, .. } => {
                if let Some(g) = get {
                    qe(g, ctx);
                }
                if let Some((_, body)) = set {
                    qblock(body, ctx);
                }
            }
        }
    }
}

fn qblock(stmts: &mut [Stmt], ctx: &Ctx) {
    for s in stmts {
        qs(s, ctx);
    }
}

fn qp(p: &mut Pattern, ctx: &Ctx) {
    match p {
        Pattern::Variant {
            name,
            fields,
            enum_qualifier,
            span,
        } => {
            for f in fields.iter_mut() {
                qp(f, ctx);
            }
            if let Some(en) = ctx.resolve(span.start, name) {
                *enum_qualifier = Some(en.clone());
            }
        }
        Pattern::Struct { fields, .. } => {
            for fp in fields {
                qp(&mut fp.pat, ctx);
            }
        }
        // Literals / wildcard / binding / type patterns carry no variant head.
        _ => {}
    }
}

fn qs(s: &mut Stmt, ctx: &Ctx) {
    match s {
        Stmt::VarDecl { init, .. } => qe(init, ctx),
        Stmt::Assign { target, value, .. } => {
            qe(target, ctx);
            qe(value, ctx);
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                qe(e, ctx);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            qe(cond, ctx);
            qblock(then_block, ctx);
            if let Some(b) = else_block {
                qblock(b, ctx);
            }
        }
        Stmt::For { iter, body, .. } => {
            qe(iter, ctx);
            qblock(body, ctx);
        }
        Stmt::While { cond, body, .. } => {
            qe(cond, ctx);
            qblock(body, ctx);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                qs(i, ctx);
            }
            if let Some(c) = cond {
                qe(c, ctx);
            }
            if let Some(st) = step {
                qs(st, ctx);
            }
            qblock(body, ctx);
        }
        Stmt::Block(b, _) => qblock(b, ctx),
        // `Destructure.pat` is a `DestructurePat` (list/map/struct — no enum-variant head).
        Stmt::Destructure {
            init, else_block, ..
        } => {
            qe(init, ctx);
            if let Some(eb) = else_block {
                qblock(eb, ctx);
            }
        }
        Stmt::Expr(e, _) | Stmt::Discard(e, _) | Stmt::Throw { value: e, .. } => qe(e, ctx),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            qblock(body, ctx);
            for CatchClause { body, .. } in catches {
                qblock(body, ctx);
            }
            if let Some(fb) = finally_block {
                qblock(fb, ctx);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn qe(e: &mut Expr, ctx: &Ctx) {
    match e {
        Expr::Call {
            callee, args, span, ..
        } => {
            for a in args.iter_mut() {
                qe(a, ctx);
            }
            qe(callee, ctx);
            // The one construction rewrite: a bare `Ident` callee whose CALL span the checker
            // resolved to an owning enum (both the originally-bare form and the qualified form
            // `unwrap_new` erased back to bare arrive here).
            let target = match &**callee {
                Expr::Ident(v, isp) => ctx
                    .resolve(span.start, v)
                    .map(|en| (en.clone(), v.clone(), *isp)),
                _ => None,
            };
            if let Some((en, v, isp)) = target {
                **callee = Expr::Member {
                    object: Box::new(Expr::Ident(en, isp)),
                    name: v,
                    safe: false,
                    sep: MemberSep::Dot,
                    span: isp,
                };
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            qe(scrutinee, ctx);
            for a in arms {
                qp(&mut a.pattern, ctx);
                if let Some(g) = &mut a.guard {
                    qe(g, ctx);
                }
                qe(&mut a.body, ctx);
            }
        }
        Expr::Unary { expr, .. } => qe(expr, ctx),
        Expr::Force { inner, .. } | Expr::Propagate { inner, .. } => qe(inner, ctx),
        Expr::Binary { lhs, rhs, .. } => {
            qe(lhs, ctx);
            qe(rhs, ctx);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => qe(value, ctx),
        Expr::Member { object, .. } => qe(object, ctx),
        Expr::Index { object, index, .. } => {
            qe(object, ctx);
            qe(index, ctx);
        }
        Expr::Str(parts, _) | Expr::Html(parts, _) => {
            for p in parts {
                if let crate::ast::StrPart::Expr(x) = p {
                    qe(x, ctx);
                }
            }
        }
        Expr::List(xs, _) => {
            for x in xs {
                qe(x, ctx);
            }
        }
        Expr::Map(ps, _) => {
            for (k, v) in ps {
                qe(k, ctx);
                qe(v, ctx);
            }
        }
        Expr::Range { start, end, .. } => {
            qe(start, ctx);
            qe(end, ctx);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            qe(cond, ctx);
            qe(then_expr, ctx);
            qe(else_expr, ctx);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => qe(x, ctx),
            LambdaBody::Block(b) => qblock(b, ctx),
        },
        Expr::CloneWith { object, fields, .. } => {
            qe(object, ctx);
            for (_, v) in fields {
                qe(v, ctx);
            }
        }
        // Robustness: `New` is already erased when this pass runs (it is OUTERMOST, after
        // `unwrap_new`), but the walker still descends it so a unit test on a pre-erasure AST
        // behaves identically.
        Expr::New(inner, _) => qe(inner, ctx),
        Expr::Spawn { call, .. } => qe(call, ctx),
        Expr::OverloadSelect { call, .. } => qe(call, ctx),
        Expr::ParentCall { args, .. } => {
            for a in args {
                qe(a, ctx);
            }
        }
        // Literals / `Ident` / `This` have no sub-expressions.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{Expr, Item, Pattern, Stmt};

    fn checked(
        src: &str,
    ) -> (
        crate::ast::Program,
        std::collections::HashMap<usize, String>,
    ) {
        let toks = crate::tokenizer::lex(src).expect("lex");
        let prog = crate::parser::Parser::new(toks)
            .parse_program()
            .expect("parse");
        let (.., table) = crate::checker::check_resolutions(&prog).expect("checks clean");
        (prog, table)
    }

    #[test]
    fn constructions_and_patterns_gain_their_owning_enum() {
        let src = "package Main;\nenum A { Dup(int x) }\nenum B { Dup(string y) }\n\
                   function f(A a): int { return match (a) { Dup(x) => x }; }\n\
                   function g(): B { return new B.Dup(\"s\"); }\n";
        let (prog, table) = checked(src);
        let prog = super::qualify_variants(prog, &table);
        // The pattern in `f` is now qualified to `A` (its scrutinee's enum).
        let Item::Function(f) = &prog.items[2] else {
            panic!("f")
        };
        let Stmt::Return { value: Some(m), .. } = &f.body[0] else {
            panic!("return")
        };
        let Expr::Match { arms, .. } = m else {
            panic!("match")
        };
        let Pattern::Variant { enum_qualifier, .. } = &arms[0].pattern else {
            panic!("variant pat")
        };
        assert_eq!(enum_qualifier.as_deref(), Some("A"), "{prog:?}");
        // The construction in `g` carries `B` as a canonical `Member` callee.
        let Item::Function(g) = &prog.items[3] else {
            panic!("g")
        };
        let Stmt::Return { value: Some(e), .. } = &g.body[0] else {
            panic!("return")
        };
        let Expr::New(inner, _) = e else {
            panic!("new")
        };
        let Expr::Call { callee, .. } = &**inner else {
            panic!("call")
        };
        let Expr::Member { object, name, .. } = &**callee else {
            panic!("qualified callee, got {callee:?}")
        };
        assert!(matches!(&**object, Expr::Ident(en, _) if en == "B"));
        assert_eq!(name, "Dup");
    }

    #[test]
    fn unresolved_spans_are_left_untouched() {
        let src = "package Main;\nenum A { One(int x) }\n\
                   function f(): A { return new A.One(1); }\n";
        let (prog, table) = checked(src);
        // An empty table (simulating a miss) must be a byte-level no-op.
        let empty = std::collections::HashMap::new();
        let same = super::qualify_variants(prog.clone(), &empty);
        assert_eq!(format!("{prog:?}"), format!("{same:?}"));
        let _ = table;
    }
}

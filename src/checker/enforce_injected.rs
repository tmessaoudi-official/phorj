use super::*;
use crate::ast::{ClassMember, Expr, Item, LambdaBody, Stmt, Type};
use crate::diagnostic::{Diagnostic, Stage};
use crate::token::Span;
use std::collections::HashSet;

/// Import-redesign S2 stage C — reject a **bare** injected Core member type used without a
/// member-import. Runs on the RAW user program at the top of [`crate::cli::check_and_expand_reified`],
/// BEFORE any prelude injection or the S1 qualifier collapse — so (a) the injected preludes' own
/// internal bare uses are never scanned (they are not in the program yet), and (b) the qualified form
/// `Http.Router` is still distinguishable from bare `Router` (S1 collapses it only afterwards).
///
/// Legal ways to name an injected member type:
///
/// - member-import it: `import Core.Http.Router;` → bare `Router`;
/// - qualify it: `Http.Router` (dotted — always allowed here; S1 collapses it to bare later).
///
/// A bare use with neither is `E-INJECTED-TYPE-BARE`. A user who declares their OWN type of the same
/// name shadows the injected one (its prelude is not injected), so such names are exempt.
pub fn enforce_injected_discipline(prog: &Program) -> Vec<Diagnostic> {
    let mut ctx = Ctx {
        imported: HashSet::new(),
        user_types: HashSet::new(),
    };
    for it in &prog.items {
        match it {
            // Member-import `import Core.<Module…>.<Type>;` binds the leaf bare. `>= 3` covers deeper
            // modules too (e.g. `import Core.Runtime.Integer.UncheckedOverflow;` → binds
            // `UncheckedOverflow`); registering a module-import leaf (`Core.Runtime.Integer` → `Integer`)
            // is harmless (nothing bare-checks a module name against `module_of`).
            Item::Import { path, .. } if path.len() >= 3 && path[0] == "Core" => {
                if let Some(leaf) = path.last() {
                    ctx.imported.insert(leaf.clone());
                }
            }
            Item::Class(c) => drop(ctx.user_types.insert(c.name.clone())),
            Item::Enum(e) => drop(ctx.user_types.insert(e.name.clone())),
            Item::Interface(i) => drop(ctx.user_types.insert(i.name.clone())),
            Item::Trait(t) => drop(ctx.user_types.insert(t.name.clone())),
            Item::TypeAlias { name, .. } => drop(ctx.user_types.insert(name.clone())),
            _ => {}
        }
    }

    let mut errs = Vec::new();
    for it in &prog.items {
        match it {
            Item::Function(f) => ctx.walk_fn(f, &mut errs),
            Item::Class(c) => {
                // Class-level `#[…]` attributes (DEC-194 2a/2b) obey the same nothing-in-the-wind rule
                // as a function's: a bare injected attribute (e.g. `#[Attribute]`) must be imported.
                for attr in &c.attrs {
                    ctx.check_name(&attr.name, attr.span, &mut errs);
                    for a in &attr.args {
                        ctx.walk_expr(a, &mut errs);
                    }
                }
                for m in &c.members {
                    ctx.walk_member(m, &mut errs);
                }
            }
            Item::Interface(i) => {
                for m in &i.methods {
                    ctx.walk_fn(m, &mut errs);
                }
            }
            Item::Trait(t) => {
                for m in &t.members {
                    ctx.walk_member(m, &mut errs);
                }
            }
            Item::Enum(e) => {
                for v in &e.variants {
                    for p in &v.fields {
                        ctx.walk_type(&p.ty, &mut errs);
                    }
                }
            }
            Item::TypeAlias { ty, .. } => ctx.walk_type(ty, &mut errs),
            Item::Test { body, .. } => ctx.walk_block(body, &mut errs),
            Item::Import { .. } => {}
        }
    }
    errs
}

/// The injected member type → owning module qualifier. UA-L2 (registry-unification): the mapping is
/// now derived from the single `cli::CORE_MODULES` registry (a row's `bare_types` → its `qualifier`),
/// so a new Core module contributes its gated types there, not in a hand-synced match here. Reused by
/// the qualified-construction dispatch in `calls.rs`/`expr.rs`. Single-type value modules
/// (`Json`/`Option`/`Result`/`Regex`/`Secret`) are leaf==type — no bare-vs-qualified ambiguity — so
/// they carry no `bare_types` and correctly return `None`.
pub(super) fn module_of(name: &str) -> Option<&'static str> {
    crate::cli::core_module_of(name)
}

struct Ctx {
    imported: HashSet<String>,
    user_types: HashSet<String>,
}

impl Ctx {
    /// Flag a bare injected member type name (span points at the use site) unless it is member-imported
    /// or shadowed by a user declaration. Dotted (qualified) names are always fine.
    fn check_name(&self, name: &str, span: Span, errs: &mut Vec<Diagnostic>) {
        if name.contains('.') || self.user_types.contains(name) || self.imported.contains(name) {
            return;
        }
        if let Some(module) = module_of(name) {
            errs.push(
                Diagnostic::new(
                    Stage::Type,
                    format!(
                        "`{name}` is an injected `Core.{module}` type used bare without importing it"
                    ),
                    span.line,
                    span.col,
                )
                .with_code("E-INJECTED-TYPE-BARE")
                .with_hint(format!(
                    "member-import it — `import Core.{module}.{name};` — or write it qualified as `{module}.{name}`"
                )),
            );
        }
    }

    fn walk_type(&self, ty: &Type, errs: &mut Vec<Diagnostic>) {
        match ty {
            Type::Named { name, args, span } => {
                self.check_name(name, *span, errs);
                for a in args {
                    self.walk_type(a, errs);
                }
            }
            Type::Optional { inner, .. } => self.walk_type(inner, errs),
            Type::Function { params, ret, .. } => {
                for p in params {
                    self.walk_type(p, errs);
                }
                self.walk_type(ret, errs);
            }
            Type::Union(members, _) | Type::Intersection(members, _) => {
                for m in members {
                    self.walk_type(m, errs);
                }
            }
            Type::FixedList { elem, .. } => self.walk_type(elem, errs),
            Type::Infer(_) | Type::Erased(_) => {}
        }
    }

    fn walk_fn(&self, f: &crate::ast::FunctionDecl, errs: &mut Vec<Diagnostic>) {
        // `#[Route]` (and any future attribute naming an injected type) is subject to the same rule.
        for attr in &f.attrs {
            self.check_name(&attr.name, attr.span, errs);
            for a in &attr.args {
                self.walk_expr(a, errs);
            }
        }
        for p in &f.params {
            self.walk_type(&p.ty, errs);
        }
        if let Some(r) = &f.ret {
            self.walk_type(r, errs);
        }
        for t in &f.throws {
            self.walk_type(t, errs);
        }
        self.walk_block(&f.body, errs);
    }

    fn walk_member(&self, m: &ClassMember, errs: &mut Vec<Diagnostic>) {
        match m {
            ClassMember::Field { ty, .. } => self.walk_type(ty, errs),
            ClassMember::Constructor { params, body, .. } => {
                for p in params {
                    self.walk_type(&p.ty, errs);
                }
                self.walk_block(body, errs);
            }
            ClassMember::Method(f) => self.walk_fn(f, errs),
            ClassMember::Hook { ty, get, set, .. } => {
                self.walk_type(ty, errs);
                if let Some(g) = get {
                    self.walk_expr(g, errs);
                }
                if let Some((p, b)) = set {
                    self.walk_type(&p.ty, errs);
                    self.walk_block(b, errs);
                }
            }
        }
    }

    fn walk_block(&self, stmts: &[Stmt], errs: &mut Vec<Diagnostic>) {
        for s in stmts {
            self.walk_stmt(s, errs);
        }
    }

    fn walk_stmt(&self, s: &Stmt, errs: &mut Vec<Diagnostic>) {
        match s {
            Stmt::VarDecl { ty, init, .. } => {
                self.walk_type(ty, errs);
                self.walk_expr(init, errs);
            }
            Stmt::Assign { target, value, .. } => {
                self.walk_expr(target, errs);
                self.walk_expr(value, errs);
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.walk_expr(v, errs);
                }
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.walk_expr(cond, errs);
                self.walk_block(then_block, errs);
                if let Some(b) = else_block {
                    self.walk_block(b, errs);
                }
            }
            Stmt::For {
                ty,
                val,
                iter,
                body,
                ..
            } => {
                self.walk_type(ty, errs);
                if let Some((t, _)) = val {
                    self.walk_type(t, errs);
                }
                self.walk_expr(iter, errs);
                self.walk_block(body, errs);
            }
            Stmt::While { cond, body, .. } => {
                self.walk_expr(cond, errs);
                self.walk_block(body, errs);
            }
            Stmt::CFor {
                init,
                cond,
                step,
                body,
                ..
            } => {
                if let Some(i) = init {
                    self.walk_stmt(i, errs);
                }
                if let Some(c) = cond {
                    self.walk_expr(c, errs);
                }
                if let Some(st) = step {
                    self.walk_stmt(st, errs);
                }
                self.walk_block(body, errs);
            }
            Stmt::Block(stmts, _) => self.walk_block(stmts, errs),
            Stmt::Expr(e, _) | Stmt::Discard(e, _) => self.walk_expr(e, errs),
            Stmt::Throw { value, .. } => self.walk_expr(value, errs),
            Stmt::Try {
                body,
                catches,
                finally_block,
                ..
            } => {
                self.walk_block(body, errs);
                for c in catches {
                    self.walk_type(&c.ty, errs);
                    self.walk_block(&c.body, errs);
                }
                if let Some(b) = finally_block {
                    self.walk_block(b, errs);
                }
            }
            Stmt::Destructure {
                init, else_block, ..
            } => {
                self.walk_expr(init, errs);
                if let Some(b) = else_block {
                    self.walk_block(b, errs);
                }
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn walk_expr(&self, e: &Expr, errs: &mut Vec<Diagnostic>) {
        match e {
            // Construction `new Router(…)` — the callee is a bare injected type name. A qualified
            // `new Http.Router(…)` (callee is a `Member`) is fine. Recurse into the args regardless.
            Expr::New(inner, span) => {
                if let Expr::Call { callee, args, .. } = inner.as_ref() {
                    if let Expr::Ident(name, _) = callee.as_ref() {
                        self.check_name(name, *span, errs);
                    }
                    for a in args {
                        self.walk_expr(a, errs);
                    }
                } else {
                    self.walk_expr(inner, errs);
                }
            }
            // `value instanceof Router` / `value as Router` — the type name is a bare string.
            Expr::InstanceOf {
                value,
                type_name,
                span,
            }
            | Expr::Cast {
                value,
                type_name,
                span,
            } => {
                self.check_name(type_name, *span, errs);
                self.walk_expr(value, errs);
            }
            Expr::OverloadSelect { ty, call, .. } => {
                self.walk_type(ty, errs);
                self.walk_expr(call, errs);
            }
            // Structural recursion through every expr that holds sub-expressions.
            Expr::Unary { expr, .. }
            | Expr::Force { inner: expr, .. }
            | Expr::Propagate { inner: expr, .. } => self.walk_expr(expr, errs),
            Expr::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs, errs);
                self.walk_expr(rhs, errs);
            }
            Expr::Call { callee, args, .. } => {
                self.walk_expr(callee, errs);
                for a in args {
                    self.walk_expr(a, errs);
                }
            }
            Expr::Member { object, .. } => self.walk_expr(object, errs),
            Expr::Index { object, index, .. } => {
                self.walk_expr(object, errs);
                self.walk_expr(index, errs);
            }
            Expr::ParentCall { args, .. } => {
                for a in args {
                    self.walk_expr(a, errs);
                }
            }
            Expr::List(items, _) => {
                for i in items {
                    self.walk_expr(i, errs);
                }
            }
            // `new List<T>()` — built-in collection kinds; type args are resolved (and injected-import
            // discipline enforced) during type resolution, not in this expr walk.
            Expr::NewColl { .. } => {}
            Expr::Map(pairs, _) => {
                for (k, v) in pairs {
                    self.walk_expr(k, errs);
                    self.walk_expr(v, errs);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.walk_expr(scrutinee, errs);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        self.walk_expr(g, errs);
                    }
                    self.walk_expr(&arm.body, errs);
                }
            }
            Expr::Range { start, end, .. } => {
                self.walk_expr(start, errs);
                self.walk_expr(end, errs);
            }
            Expr::If {
                cond,
                then_expr,
                else_expr,
                ..
            } => {
                self.walk_expr(cond, errs);
                self.walk_expr(then_expr, errs);
                self.walk_expr(else_expr, errs);
            }
            Expr::Lambda {
                params, ret, body, ..
            } => {
                for p in params {
                    self.walk_type(&p.ty, errs);
                }
                if let Some(r) = ret {
                    self.walk_type(r, errs);
                }
                match body {
                    LambdaBody::Expr(ex) => self.walk_expr(ex, errs),
                    LambdaBody::Block(b) => self.walk_block(b, errs),
                }
            }
            Expr::CloneWith { object, fields, .. } => {
                self.walk_expr(object, errs);
                for (_, v) in fields {
                    self.walk_expr(v, errs);
                }
            }
            Expr::Spawn { call, .. } => self.walk_expr(call, errs),
            // Leaves — no sub-expressions and no type name to check.
            Expr::Int(..)
            | Expr::Float(..)
            | Expr::Decimal { .. }
            | Expr::Bool(..)
            | Expr::Null(..)
            | Expr::Str(..)
            | Expr::Bytes(..)
            | Expr::Ident(..)
            | Expr::This(..)
            | Expr::Inject { .. }
            | Expr::TaggedTemplate { .. }
            | Expr::Html(..) => {}
        }
    }
}

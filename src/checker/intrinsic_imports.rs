//! DEC-196 Q3 — fault-intrinsic import discipline (`Core.Assert` / `Core.Abort`).
//!
//! The four fault intrinsics are no longer import-free. They live in two reserved language-core
//! modules — `Core.Assert` = { `assert` } (the conditional check) and `Core.Abort` =
//! { `panic`, `todo`, `unreachable` } (the unconditional aborts) — and follow the SAME two-mode
//! discipline as types/variants (the developer's ruled model, 2026-07-05):
//!
//!   * **whole-module import → QUALIFIED call:** `import Core.Assert;` ⇒ `Assert.assert(x)`;
//!     `import Core.Abort;` ⇒ `Abort.panic("m")` / `Abort.todo()` / `Abort.unreachable()`.
//!   * **member import → BARE call:** `import Core.Abort.panic;` ⇒ `panic("m")`; grouped
//!     `import Core.Abort.{ panic, todo };` (DEC-186 group syntax) ⇒ bare `panic`/`todo`.
//!
//! Any intrinsic call not covered by the matching import is **`E-UNIMPORTED`** — this is the
//! "nothing in the wind" principle honored: a bare intrinsic requires an explicit member import that
//! names it; the module import gives the attributed qualified form.
//!
//! [`resolve_intrinsic_imports`] runs on the RAW program at the top of
//! [`crate::cli::check_and_expand_reified`] (before any prelude injection / qualifier collapse, so
//! bare-vs-qualified is still distinguishable). In ONE `&mut` traversal it BOTH (a) validates
//! coverage — accumulating `E-UNIMPORTED` diagnostics — AND (b) normalizes every valid QUALIFIED
//! call (`Assert.assert(x)`) down to the bare intrinsic (`assert(x)`) the rest of the pipeline and
//! every backend already lower. Backends are therefore UNCHANGED and byte-identity is preserved.
//! A no-op (no rewrite, no error) unless the program uses an intrinsic. The `Core.Assert`/`Core.Abort`
//! module qualifiers themselves are treated as intrinsic ONLY when their module is imported, so a
//! user class named `Assert`/`Abort` is never hijacked.

use crate::ast::{ClassMember, Expr, Item, LambdaBody, Program, Stmt, StrPart};
use crate::checker::common::intrinsic_module_of;
use crate::diagnostic::{Diagnostic, Stage};
use crate::token::Span;
use std::collections::HashSet;

/// What the program's imports enable, per the two-mode model.
#[derive(Default)]
struct Enabled {
    /// Intrinsic modules imported whole (`Core.Assert`, `Core.Abort`) — enable the QUALIFIED form.
    modules: HashSet<&'static str>,
    /// Intrinsic leaf names member-imported (`import Core.Abort.panic;`) — enable the BARE form.
    bare: HashSet<String>,
}

/// The module qualifier leaf (`Assert`/`Abort`) for a reserved intrinsic module path segment.
fn intrinsic_module_leaf(seg: &str) -> Option<&'static str> {
    match seg {
        "Assert" => Some("Core.Assert"),
        "Abort" => Some("Core.Abort"),
        _ => None,
    }
}

/// Scan imports → what's enabled, plus any import-shape errors (a member import naming a non-intrinsic
/// leaf, an intrinsic import carrying an unsupported `as` alias).
fn collect_enabled(items: &[Item]) -> (Enabled, Vec<Diagnostic>) {
    let mut en = Enabled::default();
    let mut errs = Vec::new();
    for it in items {
        let Item::Import {
            path, alias, span, ..
        } = it
        else {
            continue;
        };
        if path.first().map(String::as_str) != Some("Core") {
            continue;
        }
        // Whole-module import: `import Core.Assert;` / `import Core.Abort;`.
        if path.len() == 2 {
            if let Some(module) = intrinsic_module_leaf(&path[1]) {
                if alias.is_some() {
                    errs.push(alias_unsupported(module, *span));
                    continue;
                }
                en.modules.insert(module);
            }
            continue;
        }
        // Member import: `import Core.Abort.panic;` (grouped forms are pre-desugared to one each).
        if path.len() == 3 {
            let Some(module) = intrinsic_module_leaf(&path[1]) else {
                continue;
            };
            let leaf = &path[2];
            if intrinsic_module_of(leaf) != Some(module) {
                errs.push(
                    Diagnostic::new(
                        Stage::Type,
                        format!("`{module}` has no intrinsic `{leaf}`"),
                        span.line,
                        span.col,
                    )
                    .with_code("E-IMPORT-UNKNOWN")
                    .with_hint(intrinsic_menu()),
                );
                continue;
            }
            if alias.is_some() {
                errs.push(alias_unsupported(module, *span));
                continue;
            }
            en.bare.insert(leaf.clone());
        }
    }
    (en, errs)
}

fn alias_unsupported(module: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        Stage::Type,
        format!("an `{module}` intrinsic import cannot be aliased with `as`"),
        span.line,
        span.col,
    )
    .with_code("E-IMPORT-CONFLICT")
    .with_hint("import it without `as` — intrinsics keep their canonical name".to_string())
}

fn intrinsic_menu() -> String {
    "the fault intrinsics are `Core.Assert.assert` and `Core.Abort.{ panic, todo, unreachable }`"
        .to_string()
}

/// `E-UNIMPORTED` for a BARE intrinsic call with no covering member import.
fn bare_unimported(name: &str, module: &str, span: Span) -> Diagnostic {
    let qual = module.strip_prefix("Core.").unwrap_or(module);
    Diagnostic::new(
        Stage::Type,
        format!("`{name}` is a fault intrinsic and needs an import to be called"),
        span.line,
        span.col,
    )
    .with_code("E-UNIMPORTED")
    .with_hint(format!(
        "member-import it for the bare form — `import {module}.{name};` — or import the module \
         `import {module};` and call it qualified as `{qual}.{name}(…)`"
    ))
}

/// `E-UNIMPORTED` for a QUALIFIED intrinsic call whose module was not imported.
fn qualified_unimported(qual: &str, name: &str, module: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        Stage::Type,
        format!("`{qual}.{name}` uses the `{module}` intrinsic module without importing it"),
        span.line,
        span.col,
    )
    .with_code("E-UNIMPORTED")
    .with_hint(format!(
        "add `import {module};` to call `{qual}.{name}(…)` qualified"
    ))
}

/// Validate coverage and normalize qualified intrinsic calls to their bare form, in one pass on the
/// raw program. `Ok` carries the rewritten program (qualified → bare); `Err` carries every
/// `E-UNIMPORTED` / import-shape diagnostic found. A no-op unless an intrinsic module is touched.
pub fn resolve_intrinsic_imports(mut program: Program) -> Result<Program, Vec<Diagnostic>> {
    let (en, mut errs) = collect_enabled(&program.items);
    for item in &mut program.items {
        walk_item(item, &en, &mut errs);
    }
    if errs.is_empty() {
        Ok(program)
    } else {
        Err(errs)
    }
}

fn walk_item(it: &mut Item, en: &Enabled, errs: &mut Vec<Diagnostic>) {
    match it {
        Item::Function(f) => walk_block(&mut f.body, en, errs),
        Item::Class(c) => {
            for m in &mut c.members {
                walk_member(m, en, errs);
            }
        }
        Item::Interface(i) => {
            for m in &mut i.methods {
                walk_block(&mut m.body, en, errs);
            }
        }
        Item::Trait(t) => {
            for m in &mut t.members {
                walk_member(m, en, errs);
            }
        }
        Item::Test { body, .. } => walk_block(body, en, errs),
        Item::Enum(_) | Item::TypeAlias { .. } | Item::Import { .. } => {}
    }
}

fn walk_member(m: &mut ClassMember, en: &Enabled, errs: &mut Vec<Diagnostic>) {
    match m {
        ClassMember::Method(f) => walk_block(&mut f.body, en, errs),
        ClassMember::Constructor { body, .. } => walk_block(body, en, errs),
        ClassMember::Hook { get, set, .. } => {
            if let Some(g) = get {
                walk_expr(g, en, errs);
            }
            if let Some((_, b)) = set {
                walk_block(b, en, errs);
            }
        }
        ClassMember::Field { .. } => {}
    }
}

fn walk_block(stmts: &mut [Stmt], en: &Enabled, errs: &mut Vec<Diagnostic>) {
    for s in stmts {
        walk_stmt(s, en, errs);
    }
}

fn walk_stmt(s: &mut Stmt, en: &Enabled, errs: &mut Vec<Diagnostic>) {
    match s {
        Stmt::VarDecl { init, .. } => walk_expr(init, en, errs),
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, en, errs);
            walk_expr(value, en, errs);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                walk_expr(v, en, errs);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            walk_expr(cond, en, errs);
            walk_block(then_block, en, errs);
            if let Some(b) = else_block {
                walk_block(b, en, errs);
            }
        }
        Stmt::For { iter, body, .. } => {
            walk_expr(iter, en, errs);
            walk_block(body, en, errs);
        }
        Stmt::While { cond, body, .. } => {
            walk_expr(cond, en, errs);
            walk_block(body, en, errs);
        }
        Stmt::CFor {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(i) = init {
                walk_stmt(i, en, errs);
            }
            if let Some(c) = cond {
                walk_expr(c, en, errs);
            }
            if let Some(st) = step {
                walk_stmt(st, en, errs);
            }
            walk_block(body, en, errs);
        }
        Stmt::Block(stmts, _) => walk_block(stmts, en, errs),
        Stmt::Expr(e, _) | Stmt::Discard(e, _) => walk_expr(e, en, errs),
        Stmt::Throw { value, .. } => walk_expr(value, en, errs),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            walk_block(body, en, errs);
            for c in catches {
                walk_block(&mut c.body, en, errs);
            }
            if let Some(b) = finally_block {
                walk_block(b, en, errs);
            }
        }
        Stmt::Destructure {
            init, else_block, ..
        } => {
            walk_expr(init, en, errs);
            if let Some(b) = else_block {
                walk_block(b, en, errs);
            }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn walk_expr(e: &mut Expr, en: &Enabled, errs: &mut Vec<Diagnostic>) {
    if let Expr::Call { callee, args, span } = e {
        // Recurse into children first (nested intrinsic calls, args), then inspect/normalize THIS
        // call's callee. Inspect read-only, decide, THEN mutate — so the `*callee` reassignment for
        // the qualified→bare rewrite does not overlap the borrow used to inspect it.
        for a in args.iter_mut() {
            walk_expr(a, en, errs);
        }
        walk_expr(callee, en, errs);
        let span = *span;
        let mut rewrite_to: Option<String> = None;
        match callee.as_ref() {
            // BARE form — `assert(x)` / `panic("m")`. Valid only if member-imported.
            Expr::Ident(name, _) => {
                if let Some(module) = intrinsic_module_of(name) {
                    if !en.bare.contains(name) {
                        errs.push(bare_unimported(name, module, span));
                    }
                }
            }
            // QUALIFIED form — `Assert.assert(x)` / `Abort.panic("m")`. Treated as an intrinsic ONLY
            // when its module is imported (else `Qual.member` may be a user static call — leave it
            // alone). A valid, imported qualified intrinsic is normalized to the bare intrinsic the
            // backends already lower.
            Expr::Member {
                object,
                name: member,
                safe: false,
                ..
            } => {
                if let Expr::Ident(qual, _) = object.as_ref() {
                    if let Some(module) = intrinsic_module_leaf(qual) {
                        let is_member_of_module = intrinsic_module_of(member) == Some(module);
                        if en.modules.contains(module) {
                            if is_member_of_module {
                                rewrite_to = Some(member.clone());
                            } else {
                                errs.push(
                                    Diagnostic::new(
                                        Stage::Type,
                                        format!("`{module}` has no intrinsic `{member}`"),
                                        span.line,
                                        span.col,
                                    )
                                    .with_code("E-IMPORT-UNKNOWN")
                                    .with_hint(intrinsic_menu()),
                                );
                            }
                        } else if is_member_of_module {
                            errs.push(qualified_unimported(qual, member, module, span));
                        }
                    }
                }
            }
            _ => {}
        }
        if let Some(name) = rewrite_to {
            // Reuse the existing box (clippy::replace_box) — replace its inner value, no new alloc.
            **callee = Expr::Ident(name, span);
        }
        return;
    }
    walk_children(e, en, errs);
}

/// Recurse through every non-`Call` expression that holds sub-expressions.
fn walk_children(e: &mut Expr, en: &Enabled, errs: &mut Vec<Diagnostic>) {
    match e {
        Expr::New(inner, _)
        | Expr::Unary { expr: inner, .. }
        | Expr::Force { inner, .. }
        | Expr::Propagate { inner, .. } => walk_expr(inner, en, errs),
        Expr::OverloadSelect { call, .. } => walk_expr(call, en, errs),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, en, errs);
            walk_expr(rhs, en, errs);
        }
        Expr::Member { object, .. } => walk_expr(object, en, errs),
        Expr::Index { object, index, .. } => {
            walk_expr(object, en, errs);
            walk_expr(index, en, errs);
        }
        Expr::InstanceOf { value, .. } | Expr::Cast { value, .. } => walk_expr(value, en, errs),
        Expr::ParentCall { args, .. } => {
            for a in args {
                walk_expr(a, en, errs);
            }
        }
        Expr::List(items, _) => {
            for i in items {
                walk_expr(i, en, errs);
            }
        }
        // `new List<T>()` — no intrinsic-import obligations (built-in collection kinds).
        Expr::NewColl { .. } => {}
        Expr::Map(pairs, _) => {
            for (k, v) in pairs {
                walk_expr(k, en, errs);
                walk_expr(v, en, errs);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(scrutinee, en, errs);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    walk_expr(g, en, errs);
                }
                walk_expr(&mut arm.body, en, errs);
            }
        }
        Expr::Range { start, end, .. } => {
            walk_expr(start, en, errs);
            walk_expr(end, en, errs);
        }
        Expr::If {
            cond,
            then_expr,
            else_expr,
            ..
        } => {
            walk_expr(cond, en, errs);
            walk_expr(then_expr, en, errs);
            walk_expr(else_expr, en, errs);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(ex) => walk_expr(ex, en, errs),
            LambdaBody::Block(b) => walk_block(b, en, errs),
        },
        Expr::CloneWith { object, fields, .. } => {
            walk_expr(object, en, errs);
            for (_, v) in fields {
                walk_expr(v, en, errs);
            }
        }
        Expr::Spawn { call, .. } => walk_expr(call, en, errs),
        Expr::Str(parts, _) => {
            for p in parts {
                if let StrPart::Expr(ex) = p {
                    walk_expr(ex, en, errs);
                }
            }
        }
        // `Call` is handled by `walk_expr`; `Str` interpolation is handled above; the rest are leaves.
        Expr::Call { .. }
        | Expr::Int(..)
        | Expr::Float(..)
        | Expr::Decimal { .. }
        | Expr::Bool(..)
        | Expr::Null(..)
        | Expr::Bytes(..)
        | Expr::Ident(..)
        | Expr::This(..)
        | Expr::Inject { .. }
        | Expr::TaggedTemplate { .. }
        | Expr::Html(..) => {}
    }
}

//! Row 4j2(b) — the LIFT-TIME half: which declaration, class and `try` the lift is inside, so a
//! declaration gets its `throws` and a call that must propagate is spelled `call?`. Scoped by guards
//! ([`Scope`]) that restore on drop, so an early `?` return in the lifter cannot leak a context.

use super::{covered, facts, facts_of, leaf, type_name, Analysis, Catches, Facts, Key};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

thread_local! {
    /// The directory lift's whole-tree facts, set before any file lifts; `None` for a single file.
    static PROJECT: RefCell<Option<Rc<Analysis>>> = const { RefCell::new(None) };
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
}

#[derive(Default)]
struct Ctx {
    analysis: Option<Rc<Analysis>>,
    class: Option<String>,
    decl: Option<Key>,
    catches: Catches,
    /// Row 4j3: the catch variables in scope — `$e->getMessage()` on one reads `e.message`.
    caught: Vec<String>,
    /// Types named by a `throws` clause this file emitted — each needs a type in scope.
    named: BTreeSet<String>,
}

pub(in crate::lift) fn set_project(facts: Option<Facts>) {
    PROJECT.with(|p| *p.borrow_mut() = facts.map(|f| Rc::new(Analysis::of(&f))));
}

/// Start one file's lift: the project's analysis, or this file's own.
pub(in crate::lift) fn begin_file(prog: &crate::lift::ast::PhpProgram) {
    let analysis = PROJECT
        .with(|p| p.borrow().clone())
        .unwrap_or_else(|| Rc::new(Analysis::of(&facts_of(prog))));
    CTX.with(|c| {
        *c.borrow_mut() = Ctx {
            analysis: Some(analysis),
            ..Ctx::default()
        }
    });
}

/// Restores the context it replaced when dropped, so an early `?` return cannot leak it.
pub(in crate::lift) struct Scope(Option<Saved>);

/// The scoped half of [`Ctx`] — everything a guard restores.
type Saved = (Option<String>, Option<Key>, Catches, Vec<String>);

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((class, decl, catches, caught)) = self.0.take() {
            CTX.with(|c| {
                let mut c = c.borrow_mut();
                c.class = class;
                c.decl = decl;
                c.catches = catches;
                c.caught = caught;
            });
        }
    }
}

fn scope(edit: impl FnOnce(&mut Ctx)) -> Scope {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        let saved = (
            c.class.clone(),
            c.decl.clone(),
            c.catches.clone(),
            c.caught.clone(),
        );
        edit(&mut c);
        Scope(Some(saved))
    })
}

/// Lifting a class's members: its name resolves `$this->`/`self::`; no declaration is open yet.
pub(in crate::lift) fn enter_class(name: &str) -> Scope {
    scope(|c| {
        c.class = Some(name.to_string());
        c.decl = None;
        c.catches.clear();
        c.caught.clear();
    })
}

/// Lifting a declaration's body. `None` (a constructor, a magic method, `main`, a lambda) is a body
/// that cannot declare `throws`, so no call in it is propagated.
pub(in crate::lift) fn enter_decl(name: Option<&str>) -> Scope {
    scope(|c| {
        c.decl = name
            .filter(|n| facts::can_declare(n))
            .map(|n| match &c.class {
                Some(class) => Key::Method(class.clone(), n.to_string()),
                None => Key::Function(n.to_string()),
            });
        c.catches.clear();
    })
}

/// Lifting a `try` body: its catch types cover the calls inside it.
pub(in crate::lift) fn enter_try(types: &[String]) -> Scope {
    scope(|c| c.catches.push(types.iter().map(|t| type_name(t)).collect()))
}

/// Lifting a `catch` body that binds `var` (row 4j3).
pub(in crate::lift) fn enter_catch(var: &str) -> Scope {
    scope(|c| c.caught.push(var.to_string()))
}

/// `$var->getMessage()` on a caught exception → `var.message`: phorj's error types carry the
/// message as a public field, and the transpiler feeds it to `\Exception`'s own store, so both
/// directions agree. Any other receiver is a user method and stays a call.
pub(in crate::lift::lifter) fn caught_message(
    recv: &crate::lift::ast::PhpExpr,
    name: &str,
    args: &[crate::lift::ast::PhpExpr],
) -> Option<crate::ast::Expr> {
    let crate::lift::ast::PhpExpr::Var(v) = recv else {
        return None;
    };
    let caught = CTX.with(|c| c.borrow().caught.iter().any(|x| x == v));
    (caught && name == "getMessage" && args.is_empty()).then(|| crate::ast::Expr::Member {
        object: Box::new(crate::ast::Expr::Ident(v.clone(), super::super::SP)),
        name: "message".to_string(),
        safe: false,
        sep: crate::ast::MemberSep::Dot,
        span: super::super::SP,
    })
}

/// The open declaration's `throws`, sorted (Invariant 10).
pub(in crate::lift) fn clause() -> Vec<crate::ast::Type> {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        let (Some(a), Some(k)) = (&c.analysis, &c.decl) else {
            return Vec::new();
        };
        let names: Vec<String> = a.throws.get(k).into_iter().flatten().cloned().collect();
        c.named.extend(names.iter().cloned());
        names
            .into_iter()
            .map(|name| crate::ast::Type::Named {
                name,
                args: Vec::new(),
                span: super::super::SP,
            })
            .collect()
    })
}

/// Whether the call `e` throws something no enclosing catch covers, inside a declaration that can
/// declare it — i.e. whether the lift spells it `e?`.
fn propagates(e: &crate::lift::ast::PhpExpr) -> bool {
    use crate::lift::ast::PhpExpr as E;
    CTX.with(|c| {
        let c = c.borrow();
        let (Some(a), Some(_)) = (&c.analysis, &c.decl) else {
            return false;
        };
        let key = match e {
            E::Call { callee, .. } => match &**callee {
                E::Name(n) => Key::Function(leaf(n).to_string()),
                _ => return false,
            },
            E::MethodCall {
                recv,
                name,
                nullsafe: false,
                ..
            } if matches!(&**recv, E::Var(v) if v == "this") => match &c.class {
                Some(class) => Key::Method(class.clone(), name.clone()),
                None => return false,
            },
            E::StaticCall { class, name, .. } => {
                let class = match class.as_str() {
                    "self" | "static" => c.class.clone(),
                    "parent" => c.class.as_ref().and_then(|k| a.parents.get(k).cloned()),
                    other => Some(leaf(other).to_string()),
                };
                match class {
                    Some(class) => Key::Method(class, name.clone()),
                    None => return false,
                }
            }
            _ => return false,
        };
        let Some(key) = a.resolve(&key) else {
            return false;
        };
        a.throws[&key]
            .iter()
            .any(|t| !covered(&a.parents, t, &c.catches))
    })
}

/// The one door every PHP expression is lifted through (`exprs::lift_expr`): a lambda body is lifted
/// with no declaration open, and a propagating call comes back wrapped as `call?`.
pub(in crate::lift::lifter) fn lift_site(
    e: &crate::lift::ast::PhpExpr,
    raw: fn(&crate::lift::ast::PhpExpr) -> Result<crate::ast::Expr, String>,
) -> Result<crate::ast::Expr, String> {
    use crate::lift::ast::PhpExpr as E;
    match e {
        E::Closure { .. } | E::BlockClosure { .. } => {
            let _g = enter_decl(None);
            raw(e)
        }
        E::Call { .. } | E::MethodCall { .. } | E::StaticCall { .. } => {
            let lifted = raw(e)?;
            Ok(if propagates(e) {
                crate::ast::Expr::Propagate {
                    inner: Box::new(lifted),
                    span: super::super::SP,
                }
            } else {
                lifted
            })
        }
        _ => raw(e),
    }
}

/// The user classes this file's `throws` clauses named, with the namespace declaring each — the
/// imports a propagated type from another file needs.
pub(in crate::lift) fn named_homes() -> Vec<(String, Vec<String>)> {
    CTX.with(|c| {
        let c = c.borrow();
        let Some(a) = &c.analysis else {
            return Vec::new();
        };
        c.named
            .iter()
            .filter_map(|n| a.homes.get(n).map(|h| (n.clone(), h.clone())))
            .collect()
    })
}

/// The `Core.ErrorModule` types this file's `throws` clauses named.
pub(in crate::lift) fn named_error_types() -> Vec<String> {
    CTX.with(|c| {
        c.borrow()
            .named
            .iter()
            .filter(|n| {
                crate::native::error_prelude::ERROR_PRELUDE.contains(&format!("class {n} "))
            })
            .cloned()
            .collect()
    })
}

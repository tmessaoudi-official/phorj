//! Row 4j2(b) — the PHP-side FACTS the `throws` analysis runs on, one walk per parsed file.
//!
//! Per function/method (a [`Key`]) it records every `throw` whose type can be READ off the source and
//! every call whose callee can be resolved statically, each with the `catch` types enclosing it — the
//! same try-nesting the lift later pushes while it lifts the body, so the two halves agree on what is
//! covered. A closure body is not entered: its throws belong to the lambda, which cannot declare them.

#[path = "facts_exprs.rs"]
mod exprs;

use super::{leaf, type_name, Key};
use crate::lift::ast as php;
use std::collections::HashMap;

/// A throw site's type: named at the site, or the class a static factory returns (resolved once the
/// whole project's factories are known).
#[derive(Clone, Debug)]
pub(super) enum Thrown {
    Named(String),
    Factory(Key),
}

/// The catch types enclosing a site, innermost last, as phorj names.
pub(super) type Catches = Vec<Vec<String>>;

#[derive(Clone, Debug, Default)]
pub(super) struct Summary {
    pub(super) throws: Vec<(Thrown, Catches)>,
    pub(super) calls: Vec<(Key, Catches)>,
}

#[derive(Clone, Debug, Default)]
pub(in crate::lift) struct Facts {
    /// Class leaf → its parent's phorj name (a mapped PHP base becomes its `Core.ErrorModule` type).
    pub(super) parents: HashMap<String, String>,
    /// A static method whose declared return is its own class (`self`/`static`/the name).
    pub(super) factories: HashMap<Key, String>,
    pub(super) decls: HashMap<Key, Summary>,
    /// Class leaf → the namespace of the file declaring it (a cross-file `throws` needs its import).
    pub(super) homes: HashMap<String, Vec<String>>,
    /// Leaves declared more than once across the tree (two `NotFoundException`s in two namespaces):
    /// the facts cannot tell them apart, so [`Facts::pruned`] drops them and their sites stay loud.
    pub(super) ambiguous: std::collections::BTreeSet<String>,
}

impl Facts {
    pub(in crate::lift) fn extend(&mut self, other: Facts) {
        for (class, home) in &other.homes {
            if self.homes.get(class).is_some_and(|h| h != home) {
                self.ambiguous.insert(class.clone());
            }
        }
        for key in other.decls.keys() {
            if let (Key::Function(name), true) = (key, self.decls.contains_key(key)) {
                self.ambiguous.insert(name.clone());
            }
        }
        self.ambiguous.extend(other.ambiguous);
        self.parents.extend(other.parents);
        self.factories.extend(other.factories);
        self.decls.extend(other.decls);
        self.homes.extend(other.homes);
    }
}

impl Facts {
    /// These facts without any ambiguous leaf — as a declaration, a parent, a factory, a home, or a
    /// thrown type — so nothing can be declared, propagated or imported on a guess.
    pub(super) fn pruned(&self) -> Facts {
        let bad = |n: &str| self.ambiguous.contains(n);
        let key_bad = |k: &Key| match k {
            Key::Function(n) => bad(n),
            Key::Method(c, _) => bad(c),
        };
        let mut f = self.clone();
        f.decls.retain(|k, _| !key_bad(k));
        f.factories.retain(|k, c| !key_bad(k) && !bad(c));
        f.parents.retain(|c, p| !bad(c) && !bad(p));
        f.homes.retain(|c, _| !bad(c));
        for s in f.decls.values_mut() {
            s.throws
                .retain(|(t, _)| !matches!(t, Thrown::Named(n) if bad(n)));
        }
        f
    }
}

/// Whether a declaration can carry `throws`: the entry cannot (`main` reports `E-UNCAUGHT-THROW`), a
/// constructor cannot, and a magic method lifts to an attribute-marked member the lift leaves alone.
pub(super) fn can_declare(name: &str) -> bool {
    name != "main" && !name.starts_with("__")
}

pub(in crate::lift) fn facts_of(prog: &php::PhpProgram) -> Facts {
    let mut f = Facts::default();
    for item in &prog.items {
        match item {
            php::PhpItem::Function(func) if can_declare(&func.name) => {
                let mut w = Walk::new(None, None);
                w.block(&func.body);
                f.decls.insert(Key::Function(func.name.clone()), w.out);
            }
            php::PhpItem::Class(c) => {
                f.homes.insert(c.name.clone(), prog.namespace.clone());
                if let Some(base) = &c.extends {
                    f.parents.insert(c.name.clone(), type_name(base));
                }
                for m in &c.members {
                    let php::PhpMember::Method(m) = m else {
                        continue;
                    };
                    let key = Key::Method(c.name.clone(), m.name.clone());
                    if m.is_static && returns_own_class(m, &c.name) {
                        f.factories.insert(key.clone(), c.name.clone());
                    }
                    if let (Some(body), true) = (&m.body, can_declare(&m.name)) {
                        let mut w = Walk::new(Some(&c.name), c.extends.as_deref());
                        w.block(body);
                        f.decls.insert(key, w.out);
                    }
                }
            }
            _ => {}
        }
    }
    f
}

fn returns_own_class(m: &php::PhpMethod, class: &str) -> bool {
    matches!(&m.ret, Some(php::PhpType::Named(n))
        if n == "self" || n == "static" || leaf(n) == class)
}

struct Walk<'a> {
    class: Option<&'a str>,
    parent: Option<&'a str>,
    catches: Catches,
    /// Catch variables in scope, with the phorj names of the types each was caught as.
    vars: Vec<(String, Vec<String>)>,
    out: Summary,
}

impl<'a> Walk<'a> {
    fn new(class: Option<&'a str>, parent: Option<&'a str>) -> Self {
        Walk {
            class,
            parent,
            catches: Vec::new(),
            vars: Vec::new(),
            out: Summary::default(),
        }
    }

    /// `self`/`static`/`parent` → the class they name here; anything else → its leaf.
    fn class_of(&self, class: &str) -> Option<String> {
        match class {
            "self" | "static" => self.class.map(str::to_string),
            "parent" => self.parent.map(|p| leaf(p).to_string()),
            other => Some(leaf(other).to_string()),
        }
    }

    fn block(&mut self, body: &[php::PhpStmt]) {
        for s in body {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &php::PhpStmt) {
        use php::PhpStmt as S;
        match s {
            S::Return(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            S::Expr(e) => self.expr(e),
            S::If {
                cond,
                then,
                elifs,
                els,
            } => {
                self.expr(cond);
                self.block(then);
                for (c, b) in elifs {
                    self.expr(c);
                    self.block(b);
                }
                if let Some(b) = els {
                    self.block(b);
                }
            }
            S::While { cond, body } => {
                self.expr(cond);
                self.block(body);
            }
            S::For {
                init,
                cond,
                step,
                body,
            } => {
                for e in [init, cond, step].into_iter().flatten() {
                    self.expr(e);
                }
                self.block(body);
            }
            S::Foreach { array, body, .. } => {
                self.expr(array);
                self.block(body);
            }
            S::Echo(es) => es.iter().for_each(|e| self.expr(e)),
            S::Break | S::Continue => {}
            S::Block(b) => self.block(b),
            S::Throw(e) => self.throw(e),
            S::Try {
                body,
                catches,
                finally_block,
            } => {
                // The try's catches cover its BODY only — not the catch bodies, not the finally.
                self.catches.push(
                    catches
                        .iter()
                        .flat_map(|c| c.types.iter().map(|t| type_name(t)))
                        .collect(),
                );
                self.block(body);
                self.catches.pop();
                for c in catches {
                    let types: Vec<String> = c.types.iter().map(|t| type_name(t)).collect();
                    let bound = c.var.clone().map(|v| self.vars.push((v, types)));
                    self.block(&c.body);
                    if bound.is_some() {
                        self.vars.pop();
                    }
                }
                if let Some(b) = finally_block {
                    self.block(b);
                }
            }
        }
    }

    /// A `throw`: its type when the source says it, and the calls inside its operand either way.
    fn throw(&mut self, e: &php::PhpExpr) {
        let at = self.catches.clone();
        match e {
            php::PhpExpr::New { class, .. } => {
                self.out.throws.push((Thrown::Named(type_name(class)), at));
            }
            php::PhpExpr::StaticCall { class, name, .. } => {
                if let Some(c) = self.class_of(class) {
                    let key = Key::Method(c, name.clone());
                    self.out.throws.push((Thrown::Factory(key), at));
                }
            }
            // A rethrow of a caught exception: its type is what it was caught as.
            php::PhpExpr::Var(v) => {
                if let Some((_, types)) = self.vars.iter().rev().find(|(n, _)| n == v) {
                    for t in types.clone() {
                        self.out.throws.push((Thrown::Named(t), at.clone()));
                    }
                }
            }
            _ => {}
        }
        self.expr(e);
    }

    fn call(&mut self, key: Key) {
        let at = self.catches.clone();
        self.out.calls.push((key, at));
    }
}

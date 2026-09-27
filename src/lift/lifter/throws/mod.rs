//! Row 4j2(b), DEC-547 — `throws` synthesis and propagation.
//!
//! PHP exceptions are unchecked; phorj's are checked (DEC-068): a thrown type must be declared, and a
//! bare call to a `throws` function is `E-CALL-UNHANDLED` unless a `try` covers it or it is
//! propagated with `f(…)?`. So the lifter works out, from the PHP alone, what each function and method
//! throws — its own `throw` sites whose type the source names ([`facts`]), then closed over the STATIC
//! call graph (a fixpoint: `outer` → `classify` → `fold` reaches `outer` however deep the chain). At
//! lift time a declaration carries that set as its `throws`, and each resolvable call to a throwing
//! callee that no enclosing `catch` covers is spelled `f(…)?`.
//!
//! What cannot be declared stays LOUD rather than guessed: a lambda, a constructor and `main` cannot
//! carry `throws`, a call through a value cannot be resolved, and a throw whose type the source does
//! not say (`throw $this->error()`) adds nothing — each leaves the checker's own diagnostic in place.
//! Coverage is judged in PHORJ names, as the checker will: `catch (\RuntimeException)` covers a user
//! class extending it, because both lift onto `RuntimeError`.

mod context;
mod facts;

pub(in crate::lift) use context::{
    begin_file, clause, enter_catch, enter_class, enter_decl, enter_try, named_error_types,
    named_homes, set_project,
};
pub(in crate::lift::lifter) use context::{caught_message, lift_site};
pub(in crate::lift) use facts::{facts_of, Facts};
use facts::{Catches, Thrown};
use std::collections::{BTreeSet, HashMap};

/// A declaration that can throw: a free function, or a method of a class (by leaf names).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::lift) enum Key {
    Function(String),
    Method(String, String),
}

fn leaf(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// A PHP class name as the phorj type the lift will print (the DEC-421 mapping, else its leaf).
fn type_name(php: &str) -> String {
    leaf(&super::exceptions::phorj_error_name(php)).to_string()
}

pub(in crate::lift) struct Analysis {
    throws: HashMap<Key, BTreeSet<String>>,
    parents: HashMap<String, String>,
    homes: HashMap<String, Vec<String>>,
}

impl Analysis {
    pub(in crate::lift) fn of(f: &Facts) -> Analysis {
        let f = &f.pruned();
        let decl = |k: &Key| resolve_in(f, k, |k| f.decls.contains_key(k));
        let factory = |k: &Key| resolve_in(f, k, |k| f.factories.contains_key(k));
        let mut throws: HashMap<Key, BTreeSet<String>> = HashMap::new();
        for (key, s) in &f.decls {
            let mut own = BTreeSet::new();
            for (t, at) in &s.throws {
                let name = match t {
                    Thrown::Named(n) => Some(n.clone()),
                    Thrown::Factory(k) => factory(k).and_then(|k| f.factories.get(&k).cloned()),
                };
                if let Some(n) = name.filter(|n| !covered(&f.parents, n, at)) {
                    own.insert(n);
                }
            }
            throws.insert(key.clone(), own);
        }
        // Close over the call graph. Each round only ADDS names drawn from a finite set, so it ends.
        loop {
            let mut changed = false;
            for (key, s) in &f.decls {
                let mut add = Vec::new();
                for (callee, at) in &s.calls {
                    let Some(callee) = decl(callee) else {
                        continue;
                    };
                    for t in &throws[&callee] {
                        if !covered(&f.parents, t, at) && !throws[key].contains(t) {
                            add.push(t.clone());
                        }
                    }
                }
                if !add.is_empty() {
                    changed = true;
                    throws.get_mut(key).expect("seeded").extend(add);
                }
            }
            if !changed {
                break;
            }
        }
        Analysis {
            throws,
            parents: f.parents.clone(),
            homes: f.homes.clone(),
        }
    }

    fn resolve(&self, k: &Key) -> Option<Key> {
        let mut k = k.clone();
        for _ in 0..=self.parents.len() {
            if self.throws.contains_key(&k) {
                return Some(k);
            }
            let Key::Method(c, m) = k else {
                return None;
            };
            k = Key::Method(self.parents.get(&c)?.clone(), m);
        }
        None
    }
}

/// `k`, or the ancestor declaring the method `k` names (a `$this->m()` may be inherited).
fn resolve_in(f: &Facts, k: &Key, known: impl Fn(&Key) -> bool) -> Option<Key> {
    let mut k = k.clone();
    for _ in 0..=f.parents.len() {
        if known(&k) {
            return Some(k);
        }
        let Key::Method(c, m) = k else {
            return None;
        };
        k = Key::Method(f.parents.get(&c)?.clone(), m);
    }
    None
}

/// Whether `t` is caught by an enclosing catch — itself or an ancestor named. Bounded, so a PHP
/// inheritance cycle (a fatal error PHP reports at load) cannot hang the lift.
fn covered(parents: &HashMap<String, String>, t: &str, at: &Catches) -> bool {
    let mut cur = Some(t.to_string());
    for _ in 0..=parents.len() {
        let Some(c) = cur else {
            return false;
        };
        if at.iter().flatten().any(|x| *x == c) {
            return true;
        }
        cur = parents.get(&c).cloned();
    }
    false
}

//! Static fields, single-sourced for the compiler, the interpreter and the transpiler (plan row 4c,
//! KNOWN_ISSUES STATIC-COMPOSITION-PARITY): which class's slot `C.field` names (item 2), and which
//! non-literal initializers run before `main`, in what order (item 1). One answer each, so the three
//! backends can never disagree on where an inherited static lives or when a side effect happens.

use super::{ClassMember, Expr, Item, Modifier, Program};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `(class, static field) → owning class` for every static field reachable as `class.field`.
///
/// A class OWNS the statics it declares and those of the traits it `use`s — a trait's static is a
/// per-using-class copy (M-RT S8, PHP `use` semantics). Every other static a class can name is
/// inherited: it resolves to the nearest owner in `[class] ++ class_mro[class]`, so a redeclaring
/// subclass shadows its parent and an inheriting one shares the parent's single slot, as in PHP
/// (spec "Comprehensive statics", Area A — the rule static methods already follow). An owner maps to
/// itself.
pub fn static_field_owners(program: &Program) -> BTreeMap<(String, String), String> {
    let mut traits: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for it in &program.items {
        if let Item::Trait(t) = it {
            traits.insert(&t.name, static_fields(&t.members));
        }
    }
    let mut own: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            let mut fields = static_fields(&c.members);
            for u in &c.uses {
                if let Some(ts) = traits.get(u.name.as_str()) {
                    fields.extend(ts.iter().cloned());
                }
            }
            own.insert(&c.name, fields);
        }
    }
    let mro = super::class_mro(program);
    let mut out = BTreeMap::new();
    for it in &program.items {
        let Item::Class(c) = it else { continue };
        let chain =
            std::iter::once(c.name.clone()).chain(mro.get(&c.name).cloned().unwrap_or_default());
        for owner in chain {
            for field in own.get(owner.as_str()).into_iter().flatten() {
                out.entry((c.name.clone(), field.clone()))
                    .or_insert_with(|| owner.clone());
            }
        }
    }
    out
}

fn static_fields(members: &[ClassMember]) -> BTreeSet<String> {
    members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Field {
                modifiers, name, ..
            } if modifiers.contains(&Modifier::Static) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// The NON-literal static initializers to evaluate once before `main` (a literal one is seeded at
/// load), as `(owning class, field, initializer)` in the one order every backend runs them: each
/// class's own statics in program and declaration order, then — class by class, in `uses` order — the
/// statics of the traits it `use`s, once per using class (the S8 per-user copy). A trait is never a
/// static holder itself, so its own initializers are not run in the trait's name.
pub fn static_runtime_inits(program: &Program) -> Vec<(String, String, &Expr)> {
    let mut out = Vec::new();
    for it in &program.items {
        if let Item::Class(c) = it {
            for (field, e) in runtime_static_fields(&c.members) {
                out.push((c.name.clone(), field, e));
            }
        }
    }
    for it in &program.items {
        let Item::Class(c) = it else { continue };
        for u in &c.uses {
            for t in &program.items {
                if let Item::Trait(td) = t {
                    if td.name == u.name {
                        for (field, e) in runtime_static_fields(&td.members) {
                            out.push((c.name.clone(), field, e));
                        }
                    }
                }
            }
        }
    }
    out
}

/// A member list's non-literal, non-`const` static initializers, in declaration order.
fn runtime_static_fields(members: &[ClassMember]) -> Vec<(String, &Expr)> {
    members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Field {
                modifiers,
                name,
                init: Some(e),
                ..
            } if modifiers.contains(&Modifier::Static)
                && !modifiers.contains(&Modifier::Const)
                && crate::value::const_literal(e).is_none() =>
            {
                Some((name.clone(), e))
            }
            _ => None,
        })
        .collect()
}

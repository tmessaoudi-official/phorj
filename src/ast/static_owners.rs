//! The one answer to "which class's slot does `C.field` name?" for a static field (plan row 4c item 2,
//! KNOWN_ISSUES STATIC-COMPOSITION-PARITY). Shared by the compiler, the interpreter and the transpiler
//! so the three can never disagree on where an inherited static lives.

use super::{ClassMember, Item, Modifier, Program};
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

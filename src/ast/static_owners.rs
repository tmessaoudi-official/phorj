//! Static fields, single-sourced for the compiler, the interpreter and the transpiler (plan row 4c,
//! KNOWN_ISSUES STATIC-COMPOSITION-PARITY): which class's slot `C.field` names (item 2), and which
//! non-literal initializers run before `main`, in what order (item 1). One answer each, so the three
//! backends agree on where an inherited static lives and when a side effect happens (KNOWN_ISSUES
//! STATIC-REDECL-TYPE records the type-changing redeclarations still outside this).

use super::{ClassMember, Expr, Item, Modifier, Program};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `(class, static field) → owning class` for every static field reachable as `class.field`.
///
/// A class OWNS the statics it declares and those of the traits it `use`s — a trait's static is a
/// per-using-class copy (M-RT S8, PHP `use` semantics). Every other static a class can name is
/// inherited: it resolves to the first owner in [`class_lookup_order`], so a redeclaring
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
    let mut out = BTreeMap::new();
    for it in &program.items {
        let Item::Class(c) = it else { continue };
        for owner in class_lookup_order(program, &c.name) {
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
/// load), as `(owning class, field, initializer)` in the one order every backend runs them: class by
/// class in program order, its traits' statics (in `uses` order, once per using class — the S8 per-user
/// copy), then its own in declaration order — traits first, as for instance fields, so a class static
/// may read its trait's. A trait is never a static holder itself, so its own initializers are not run
/// in the trait's name.
pub fn static_runtime_inits(program: &Program) -> Vec<(String, String, &Expr)> {
    let mut out = Vec::new();
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
        for (field, e) in runtime_static_fields(&c.members) {
            out.push((c.name.clone(), field, e));
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

/// `[class]` then its ancestors DEPTH-FIRST in `extends` order, each once — the order the checker
/// merges inherited members in (`merge_inherited_pass`: a class takes a member from its first parent,
/// whose own set already holds its first parent's), so a member resolved here has the type the checker
/// gave it. Not `class_mro`, which is breadth-first: in a diamond `D extends L, R` whose `R` redeclares
/// `Base`'s static, the two disagree (row 4c close panel). Cycle-safe.
pub fn class_lookup_order(program: &Program, class: &str) -> Vec<String> {
    fn walk(program: &Program, class: &str, out: &mut Vec<String>) {
        if out.iter().any(|c| c == class) {
            return;
        }
        out.push(class.to_string());
        let parents = program
            .items
            .iter()
            .find_map(|it| match it {
                Item::Class(c) if c.name == class => Some(c.extends.clone()),
                _ => None,
            })
            .unwrap_or_default();
        for p in &parents {
            walk(program, p, out);
        }
    }
    let mut out = Vec::new();
    walk(program, class, &mut out);
    out
}

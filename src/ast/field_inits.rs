//! Expression field initializers (Feature B): which initializers run when a class is constructed,
//! in what order — one list for all three backends and the PHP constructor prelude. It mirrors
//! [`super::ctor_plan`] class for class (plan row 4c item 4, KNOWN_ISSUES STATIC-COMPOSITION-PARITY):
//! the class whose constructor plan runs owns the initializer set, and a trait's initializers are
//! part of its user's set.

use super::*;

/// A class's or trait's **own** expression field initializers: each `(field_name, init_expr)` for a
/// plain instance field (non-`static`, non-`const`) carrying an initializer, in declaration order.
pub fn own_field_initializers(decl: &ClassDecl) -> Vec<(String, Expr)> {
    own_initializers(&decl.members)
}

/// [`own_field_initializers`] over a bare member list (a trait's).
pub fn own_initializers(members: &[ClassMember]) -> Vec<(String, Expr)> {
    members
        .iter()
        .filter_map(|m| match m {
            ClassMember::Field {
                modifiers,
                name,
                init: Some(e),
                ..
            } if !modifiers.contains(&Modifier::Static)
                && !modifiers.contains(&Modifier::Const) =>
            {
                Some((name.clone(), e.clone()))
            }
            _ => None,
        })
        .collect()
}

/// A class's initializer set: each `use`d trait's own initializers, in `uses` order, then the
/// class's own — a trait's fields are composed into the class, and `use` comes first in the body.
/// So a class initializer may read a trait field, never the reverse (a trait cannot name it).
pub fn own_field_initializers_with_traits(
    program: &Program,
    decl: &ClassDecl,
) -> Vec<(String, Expr)> {
    let mut out = Vec::new();
    for u in &decl.uses {
        if let Some(t) = trait_decl(program, &u.name) {
            out.extend(own_initializers(&t.members));
        }
    }
    out.extend(own_field_initializers(decl));
    out
}

/// True when one of the traits `decl` uses declares a constructor — which then becomes the class's
/// constructor when it declares none of its own (`ctor_plan`, M-RT S8).
pub fn uses_trait_ctor(program: &Program, decl: &ClassDecl) -> bool {
    decl.uses.iter().any(|u| {
        trait_decl(program, &u.name).is_some_and(|t| {
            t.members
                .iter()
                .any(|m| matches!(m, ClassMember::Constructor { .. }))
        })
    })
}

fn trait_decl<'p>(program: &'p Program, name: &str) -> Option<&'p TraitDecl> {
    program.items.iter().find_map(|it| match it {
        Item::Trait(t) if t.name == name => Some(t),
        _ => None,
    })
}

/// The ordered **expression field initializers** to run when constructing `class` (Feature B).
///
/// PHP does not auto-chain `parent::__construct`, so construction runs ONE constructor plan, and
/// the class that plan belongs to owns the initializers, exactly as [`ctor_plan`] decides: a class
/// owns its set (traits' then its own, [`own_field_initializers_with_traits`]) when it declares a
/// constructor, composes a trait constructor, or has any initializer of its own or its traits';
/// otherwise a single parent's set is inherited, and a multi-parent class runs every parent's set in
/// `extends` order (S6c.2b — every parent's constructor runs, so every parent's initializers do; a
/// diamond-shared base's run once per arm, as its constructor does). Each initializer may read `this`
/// and an earlier one (a later one is `E-FIELD-INIT-FORWARD-REF`). Cycle-safe via `seen`.
pub fn field_initializers(program: &Program, class: &str) -> Vec<(String, Expr)> {
    fn walk(program: &Program, class: &str, seen: &mut Vec<String>) -> Vec<(String, Expr)> {
        if seen.iter().any(|c| c == class) {
            return Vec::new();
        }
        seen.push(class.to_string());
        let Some(decl) = program.items.iter().find_map(|it| match it {
            Item::Class(c) if c.name == class => Some(c),
            _ => None,
        }) else {
            return Vec::new();
        };
        let has_ctor = decl
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor { .. }));
        let own = own_field_initializers_with_traits(program, decl);
        if has_ctor || uses_trait_ctor(program, decl) || !own.is_empty() {
            return own;
        }
        let parents = decl.extends.clone();
        parents
            .iter()
            .flat_map(|p| walk(program, p, &mut seen.clone()))
            .collect()
    }
    walk(program, class, &mut Vec::new())
}

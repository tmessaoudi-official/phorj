//! Expression field initializers (Feature B): which initializers run when a class is constructed,
//! in what order — one list for all three backends, the PHP constructor prelude and the
//! `parent.constructor(…)` inliner. Every class's set runs, base-first down the chain (DEC-575); a
//! constructor that forwards initializes its ancestors at the call (DEC-585); a diamond-shared base
//! runs once per arm (DEC-586). A trait's initializers are part of its user's set.

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

/// The ordered **expression field initializers** to run when constructing `class` (Feature B), in the
/// prelude of its constructor plan (after promotion, before any body).
///
/// Every class's initializers run, base-first down the chain, each class's traits' then its own
/// (DEC-575). A class whose own constructor forwards with `parent.constructor(…)` stops the walk: its
/// ancestors are initialized at that call instead, by the inliner (DEC-585), so its set is only its
/// own. Otherwise the set is each parent's set in `extends` order, then the class's own; a
/// diamond-shared base therefore runs once per arm, as its constructor does (DEC-586). An inherited
/// constructor plan whose owner forwards is covered by the same rule: the walk stops at that owner,
/// whose (inlined) body initializes what lies above it. Each initializer may read `this` and an
/// earlier one (a later one is `E-FIELD-INIT-FORWARD-REF`). Cycle-safe via `seen`, cloned per branch.
pub fn field_initializers(program: &Program, class: &str) -> Vec<(String, Expr)> {
    init_chain_classes(program, class)
        .iter()
        .filter_map(|c| class_decl(program, c))
        .flat_map(|d| own_field_initializers_with_traits(program, d))
        .collect()
}

/// The classes whose own sets [`field_initializers`] runs for `class`, in order (a diamond-shared base
/// once per arm). The PHP leg emits one `__phorj_init_<Class>()` call per entry (DEC-575's mapping).
pub fn init_chain_classes(program: &Program, class: &str) -> Vec<String> {
    fn walk(program: &Program, class: &str, seen: &mut Vec<String>, out: &mut Vec<String>) {
        if seen.iter().any(|c| c == class) {
            return;
        }
        seen.push(class.to_string());
        let Some(decl) = class_decl(program, class) else {
            return;
        };
        if !decl.ctor_forwards {
            for p in &decl.extends {
                walk(program, p, &mut seen.clone(), out);
            }
        }
        out.push(class.to_string());
    }
    let mut out = Vec::new();
    walk(program, class, &mut Vec::new(), &mut out);
    out
}

fn class_decl<'p>(program: &'p Program, name: &str) -> Option<&'p ClassDecl> {
    program.items.iter().find_map(|it| match it {
        Item::Class(c) if c.name == name => Some(c),
        _ => None,
    })
}

/// True when `body` holds a statement-position `parent.constructor(…)` anywhere (nested in a block,
/// `if`, loop, `using` or `try` too): the walk `checker::inline_parent_ctor` rewrites, read before it
/// does, to set [`ClassDecl::ctor_forwards`].
pub fn body_forwards(body: &[Stmt]) -> bool {
    body.iter().any(stmt_forwards)
}

fn stmt_forwards(s: &Stmt) -> bool {
    match s {
        Stmt::Expr(e, _) | Stmt::Discard(e, _) => {
            matches!(e, Expr::ParentCall { method, .. } if method == "constructor")
        }
        Stmt::Block(b, _) => body_forwards(b),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => body_forwards(then_block) || else_block.as_deref().is_some_and(body_forwards),
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Using { body, .. } => {
            body_forwards(body)
        }
        Stmt::CFor {
            init, step, body, ..
        } => {
            init.as_deref().is_some_and(stmt_forwards)
                || step.as_deref().is_some_and(stmt_forwards)
                || body_forwards(body)
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            body_forwards(body)
                || catches.iter().any(|c| body_forwards(&c.body))
                || finally_block.as_deref().is_some_and(body_forwards)
        }
        Stmt::Destructure {
            else_block: Some(eb),
            ..
        } => body_forwards(eb),
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::Throw { .. }
        | Stmt::Destructure {
            else_block: None, ..
        }
        | crate::stmt_leaves!() => false,
    }
}

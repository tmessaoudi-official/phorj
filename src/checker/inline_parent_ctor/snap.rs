//! The read-only class facts `parent.constructor(…)` inlining needs (split from `mod.rs`, Invariant 13):
//! each class's constructor and initializer sets, snapshotted before any body is rewritten, and the
//! single-parent chain walks over them.

use crate::ast::{
    field_initializers, own_field_initializers_with_traits, ClassMember, CtorParam, Expr, Item,
    Program, Stmt,
};
use std::collections::HashMap;

/// Read-only per-class facts the inline needs while ctor bodies are mutated in place.
pub(super) struct ClassSnap {
    /// Direct parents (`extends` order). Single inheritance this slice.
    pub(super) extends: Vec<String>,
    /// This class's constructor (params, body): its own, else a composed trait's (`ast::ctor_plan`'s
    /// order, M-RT S8), so forwarding into a trait-constructed parent runs that constructor.
    pub(super) own_ctor: Option<(Vec<CtorParam>, Vec<Stmt>)>,
    /// This class's own expression field initializers: its traits', then its own (declaration order).
    pub(super) field_inits: Vec<(String, Expr)>,
    /// What a forwarding call into this class runs first: `ast::field_initializers` (its chain,
    /// base-first, stopping at a class whose own constructor forwards; DEC-575, DEC-585, DEC-586).
    pub(super) chain_inits: Vec<(String, Expr)>,
}

pub(super) type Snap = HashMap<String, ClassSnap>;

/// Snapshot the read-only class facts before any in-place mutation (avoids a program-wide borrow
/// conflict while a single class's ctor body is rewritten).
pub(super) fn snapshot(program: &Program) -> Snap {
    let mut snap = Snap::new();
    for item in &program.items {
        if let Item::Class(c) = item {
            let ctor_of = |members: &[ClassMember]| {
                members.iter().find_map(|m| match m {
                    ClassMember::Constructor { params, body, .. } => {
                        Some((params.clone(), body.clone()))
                    }
                    _ => None,
                })
            };
            let own_ctor = ctor_of(&c.members).or_else(|| {
                c.uses.iter().find_map(|u| {
                    program.items.iter().find_map(|it| match it {
                        Item::Trait(t) if t.name == u.name => ctor_of(&t.members),
                        _ => None,
                    })
                })
            });
            snap.insert(
                c.name.clone(),
                ClassSnap {
                    extends: c.extends.clone(),
                    own_ctor,
                    // The owner's set includes its traits' initializers (plan row 4c item 4).
                    field_inits: own_field_initializers_with_traits(program, c),
                    chain_inits: field_initializers(program, &c.name),
                },
            );
        }
    }
    snap
}

/// The parent class `parent.constructor` (written in `lexical`) targets: the named `ancestor`, else the
/// single direct parent. `None` only for a parentless class (checker `E-PARENT-NO-PARENT`).
pub(super) fn resolve_target(lexical: &str, ancestor: Option<&str>, snap: &Snap) -> Option<String> {
    match ancestor {
        Some(a) => Some(a.to_string()),
        None => snap.get(lexical).and_then(|c| c.extends.first().cloned()),
    }
}

/// The classes strictly between `lexical` and its ancestor `target` on the single-parent chain,
/// base-first; empty for a direct parent (or when `target` is not on the chain).
pub(super) fn between(lexical: &str, target: &str, snap: &Snap) -> Vec<String> {
    let mut path = Vec::new();
    let mut cur = snap.get(lexical).and_then(|c| c.extends.first().cloned());
    while let Some(c) = cur {
        if c == target {
            path.reverse();
            return path;
        }
        cur = snap.get(&c).and_then(|s| s.extends.first().cloned());
        path.push(c);
    }
    Vec::new()
}

/// The nearest class at or above `start` (single-parent chain) that declares its own constructor — the
/// one PHP's inherited `__construct` would invoke. `None` if no constructor exists in the chain.
pub(super) fn find_owner(start: &str, snap: &Snap) -> Option<String> {
    let mut cur = start.to_string();
    loop {
        let c = snap.get(&cur)?;
        if c.own_ctor.is_some() {
            return Some(cur);
        }
        cur = c.extends.first()?.clone();
    }
}

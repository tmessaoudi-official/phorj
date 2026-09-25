//! Row 5o — §LSP-CLASS-QUALIFIED-MEMBERS: a CLASS-QUALIFIED member (`K.MAX`, `K.twice(1)`,
//! `K::MAX`) resolved for hover, go-to-definition and completion.
//!
//! Hover and definition otherwise resolve the identifier under the cursor as a top-level name or a
//! local, so `K.MAX` answered nothing — and `K.twice` would have answered with an unrelated top-level
//! `function twice`. Keying the lookup on the RECEIVER is what rules that decoy out, the shape
//! `hover_member` uses for tuple fields. Only class-level members qualify: a constant, a `static`
//! field or a `static` method — the same `Modifier::Const`/`Modifier::Static` test the checker's
//! collector applies — searched on the class first, then up its supertype chain.

use crate::ast::{ClassMember, Item, Modifier, Program};
use crate::token::Span;

/// `(name, completion kind, declaration span)` of every class-level member reachable as
/// `<class>.<member>`, own members first, then inherited ones not already shadowed. Empty when
/// `receiver` is not a user class of `program`, or when a local of that name is in scope at `offset`
/// (then the receiver is the local, not the class).
pub(super) fn static_members(
    program: &Program,
    offset: usize,
    receiver: &str,
) -> Vec<(String, u32, Span)> {
    let is_class = program
        .items
        .iter()
        .any(|it| matches!(it, Item::Class(c) if c.name == receiver));
    if !is_class || super::scope::local_definition(program, receiver, offset).is_some() {
        return Vec::new();
    }
    let supers = crate::ast::class_supertypes(program);
    let chain = std::iter::once(receiver.to_string())
        .chain(supers.get(receiver).cloned().unwrap_or_default());
    let mut out: Vec<(String, u32, Span)> = Vec::new();
    for owner in chain {
        for (name, kind, span) in own_statics(program, &owner) {
            if !out.iter().any(|(n, ..)| *n == name) {
                out.push((name, kind, span));
            }
        }
    }
    out
}

/// The member span behind `<class>.<name>` (or `<class>::<name>`) at `offset`, if the identifier
/// under the cursor is one.
pub(super) fn member_span(
    text: &str,
    offset: usize,
    name: &str,
    program: &Program,
) -> Option<Span> {
    let receiver = super::hover_member::receiver_before(text, offset, true)?;
    static_members(program, offset, &receiver)
        .into_iter()
        .find_map(|(n, _, span)| (n == name).then_some(span))
}

/// The class-level members DECLARED by the class or trait `owner` (inheritance is the caller's).
fn own_statics(program: &Program, owner: &str) -> Vec<(String, u32, Span)> {
    let members = program.items.iter().find_map(|it| match it {
        Item::Class(c) if c.name == owner => Some(&c.members),
        Item::Trait(t) if t.name == owner => Some(&t.members),
        _ => None,
    });
    let mut out = Vec::new();
    for m in members.into_iter().flatten() {
        match m {
            ClassMember::Field {
                modifiers,
                name,
                span,
                ..
            } if modifiers.contains(&Modifier::Const) => out.push((name.clone(), 21, *span)),
            ClassMember::Field {
                modifiers,
                name,
                span,
                ..
            } if modifiers.contains(&Modifier::Static) => out.push((name.clone(), 5, *span)),
            ClassMember::Method(f) if f.modifiers.contains(&Modifier::Static) => {
                out.push((f.name.clone(), 2, f.span))
            }
            _ => {}
        }
    }
    out
}

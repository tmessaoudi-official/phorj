//! Member-name collectors shared by the class-layout and member-origin analyses (split out of
//! `member_origins.rs`, 2026-11-17): a member list's own storage fields and its hook accessors.

use super::*;

/// A class's or trait's own per-instance storage fields: promoted constructor params and non-static,
/// non-const `Field` members. Shared by [`class_field_layout`] and [`class_hook_origins`].
pub(crate) fn own_storage_fields(members: &[ClassMember]) -> Vec<String> {
    let mut v = Vec::new();
    for m in members {
        match m {
            ClassMember::Field {
                name, modifiers, ..
            } if !modifiers.contains(&Modifier::Static)
                && !modifiers.contains(&Modifier::Const) =>
            {
                v.push(name.clone());
            }
            ClassMember::Constructor { params, .. } => {
                for p in params {
                    if p.modifiers.iter().any(|m| m.is_member_visibility()) {
                        v.push(p.name.clone());
                    }
                }
            }
            _ => {}
        }
    }
    v
}

/// The synthetic accessor names (`<name>$get` / `<name>$set`) of the hooks among `members`.
pub(crate) fn hook_accessors(
    members: &[ClassMember],
    out: &mut std::collections::BTreeSet<String>,
) {
    for m in members {
        if let ClassMember::Hook { name, get, set, .. } = m {
            if get.is_some() {
                out.insert(format!("{name}$get"));
            }
            if set.is_some() {
                out.insert(format!("{name}$set"));
            }
        }
    }
}

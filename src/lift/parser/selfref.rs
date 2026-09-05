//! PHP-lift parser — `self` in a class body (Lane R-7, 2026-09-05). `: self` is the fluent
//! return in 31 places across 10 of scout's 120 files; it names the enclosing class exactly, so
//! the parser writes that name where `self` was — in method returns and parameters, property and
//! constant types, through `?` and docblock generics. `static` (late static binding) is NOT
//! resolved: on an open class it means "the receiver's class", which the enclosing name would
//! narrow, so it keeps its Tier-2 refusal by name.

use super::*;

pub(super) fn resolve_self(members: &mut [PhpMember], class: &str) {
    for m in members.iter_mut() {
        match m {
            PhpMember::Method(me) => {
                for p in &mut me.params {
                    if let Some(t) = &mut p.ty {
                        resolve(t, class);
                    }
                }
                if let Some(t) = &mut me.ret {
                    resolve(t, class);
                }
            }
            PhpMember::Prop { ty: Some(t), .. } | PhpMember::Const { ty: Some(t), .. } => {
                resolve(t, class)
            }
            PhpMember::Prop { ty: None, .. } | PhpMember::Const { ty: None, .. } => {}
        }
    }
}

fn resolve(t: &mut PhpType, class: &str) {
    match t {
        PhpType::Named(n) if n == "self" => *n = class.to_string(),
        PhpType::Named(_) => {}
        PhpType::Nullable(inner) => resolve(inner, class),
        PhpType::Generic { args, .. } => {
            for a in args {
                resolve(a, class);
            }
        }
    }
}

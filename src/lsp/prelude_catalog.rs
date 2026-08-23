//! The PRELUDE half of the completion catalog — the stdlib classes injected by `cli::preludes`
//! rather than declared in the user's buffer, enumerated by parsing the `CORE_MODULES` registry's
//! own source on demand.
//!
//! Split out of `catalog.rs` at Invariant 13's soft cap when S3.3e added instance members: the two
//! halves answer the same questions from different worlds (`catalog` reads the parsed USER program,
//! this file reads the registry), and only this one has to know that a prelude is a `&'static str`.
//!
//! Nothing here consults imports. That is deliberate and matches `module_members`: a receiver only
//! resolves to a stdlib type when the buffer DECLARES that type, which in a real program already
//! required the member import ("nothing in the wind" is enforced by the checker, not by completion).
use crate::ast::{ClassMember, Item, Modifier};

/// The PUBLIC static methods of the prelude class named `qualifier`, if some `CORE_MODULES` row
/// injects such a class. `private` statics are omitted: they are internals (`FileSystem.acquireLock`
/// is the `using` subject behind `withLock`), and offering them would advertise the very shape the
/// DEC-348 ruling rejected. Parsed from the registry's own prelude source, so a new prelude static is
/// completable the moment it is written — no LSP edit, which is what Invariant 17 requires.
pub(super) fn prelude_class_statics(qualifier: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    each_prelude_decl(qualifier, |it| {
        let Item::Class(c) = it else { return };
        for m in &c.members {
            if let ClassMember::Method(f) = m {
                if has_mod(&f.modifiers, Modifier::Static)
                    && !has_mod(&f.modifiers, Modifier::Private)
                {
                    out.push(f.name.clone());
                }
            }
        }
    });
    out
}

/// The completable INSTANCE members of the PRELUDE class named `class_name` — the stdlib half of
/// [`class_members`], which only ever sees the user's own program. Without this, a receiver whose
/// declared type is a stdlib class (`ServeConfig cfg` → `cfg.`, and equally `Request`/`Response`/
/// `Date`/`Instant`/`Uri`/`Session`) completed to NOTHING: the compiler knew `cfg.port`, the editor
/// did not, which Invariant 17's 100% rule counts as an incomplete feature.
///
/// `class_name` may arrive in either spelling — bare `ServeConfig` or qualified `Http.ServeConfig`
/// (D4's own §1 surface writes the latter) — so the LEAF is what names the class.
///
/// Filters, both load-bearing on `Request`: `private`/`protected` members are internals (its wire
/// fields `rawTarget`/`rawHeaderLines`/`rawBody` are private PROMOTED ctor params, i.e. real members
/// a naive walk would offer), and `static` methods are not instance surface (`req.parse(…)` is not a
/// call anyone can write). Same reasoning [`prelude_class_statics`] applies in the other direction.
///
/// **Own members only** — a prelude class that `extends` another would not show inherited members.
/// No shipped prelude class does; when one does, compose it the way [`class_members`] composes user
/// hierarchies via `class_supertypes`.
pub(super) fn prelude_class_members(class_name: &str) -> Vec<(String, u32)> {
    let leaf = class_name.rsplit('.').next().unwrap_or(class_name);
    let mut out: Vec<(String, u32)> = Vec::new();
    each_prelude_decl(leaf, |it| match it {
        Item::Class(c) => out.extend(public_instance_members(&c.members)),
        Item::Trait(t) => out.extend(public_instance_members(&t.members)),
        Item::Interface(i) => out.extend(i.methods.iter().map(|m| (m.name.clone(), 2))),
        _ => {}
    });
    out.sort();
    out.dedup();
    out
}

/// [`collect_members`] restricted to the members a receiver of that type can actually reach: no
/// `private`/`protected`, no `static`. Hooks carry no modifiers in the AST, so they are always
/// instance-visible read/write surface.
fn public_instance_members(members: &[ClassMember]) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for m in members {
        match m {
            ClassMember::Method(f)
                if !has_mod(&f.modifiers, Modifier::Static) && is_visible(&f.modifiers) =>
            {
                out.push((f.name.clone(), 2))
            }
            ClassMember::Field {
                modifiers, name, ..
            } if !has_mod(modifiers, Modifier::Static) && is_visible(modifiers) => {
                out.push((name.clone(), 5))
            }
            ClassMember::Hook { name, .. } => out.push((name.clone(), 5)),
            ClassMember::Constructor { params, .. } => {
                for p in params {
                    // Promotion (a visibility modifier) is what makes a ctor param a member — the
                    // same single-sourced rule `collect_members` applies.
                    if p.modifiers.iter().any(|m| m.is_member_visibility())
                        && is_visible(&p.modifiers)
                    {
                        out.push((p.name.clone(), 5));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn has_mod(modifiers: &[Modifier], want: Modifier) -> bool {
    modifiers.contains(&want)
}

/// Reachable from OUTSIDE the declaring class: everything except `private`/`protected`. (`internal`
/// is package-visible and the preludes are injected INTO the user's `Main` package, so it stays.)
fn is_visible(modifiers: &[Modifier]) -> bool {
    !modifiers
        .iter()
        .any(|m| matches!(m, Modifier::Private | Modifier::Protected))
}

/// Run `visit` on every prelude declaration named `leaf`, across every `CORE_MODULES` row that binds
/// that bare type. Parsed from the registry's own source on demand, so a new prelude declaration is
/// completable the moment it is written — no LSP edit, which is what Invariant 17 requires.
fn each_prelude_decl(leaf: &str, mut visit: impl FnMut(&Item)) {
    for vm in crate::cli::preludes::CORE_MODULES {
        if !vm.bare_types.contains(&leaf) {
            continue;
        }
        for src in vm.srcs {
            let Ok(prog) = crate::cli::parse_program(&format!("package Main;\n{src}\n")) else {
                continue; // unreachable: registry preludes parse
            };
            for it in &prog.items {
                let named = match it {
                    Item::Class(c) => c.name == leaf,
                    Item::Trait(t) => t.name == leaf,
                    Item::Interface(i) => i.name == leaf,
                    _ => false,
                };
                if named {
                    visit(it);
                }
            }
        }
    }
}

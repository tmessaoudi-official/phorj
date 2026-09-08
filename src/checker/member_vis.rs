//! Member-level visibility (`public` / `internal` / `protected` / `private`) and the rules for
//! deriving it from a modifier set.
//!
//! Split out of `mod.rs` (Invariant 13): it is a self-contained value type with its own derivation
//! rules, unrelated to the `Checker` state the module root otherwise holds.

use crate::ast::Modifier;
/// Member-level visibility (Feature A — `const` class constants). Distinct from `ast::Visibility`
/// (declaration/file scope): a *member* is `public` (default), `protected`, or `private`, derived from
/// the `Modifier::{Public,Private,Protected}` set. Const access is the one site Phorj enforces member
/// visibility — required because the transpiler emits a PHP `private const`, which PHP would reject if
/// read from outside the class (a `run`↔PHP byte-identity break otherwise).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::checker) enum MemberVis {
    Public,
    /// Q-B DV-3: package-subtree-visible — reachable from the declaring class's package and its
    /// descendant packages (the same subtree meaning as top-level `internal`). Enforced via the
    /// package derived from mangled names; erases to PHP `public` (PHP has no package concept).
    Internal,
    Protected,
    Private,
}

impl MemberVis {
    /// The member visibility carried by a modifier set: `private` > `protected` > `public` (default).
    /// DEC-241: the SET visibility carried by a modifier set — `Some(Private)` for
    /// `private(set)`, `Some(Protected)` for `protected(set)`, `None` when symmetric.
    pub(in crate::checker) fn set_of(mods: &[crate::ast::Modifier]) -> Option<MemberVis> {
        use Modifier as M;
        if mods.contains(&M::PrivateSet) {
            Some(MemberVis::Private)
        } else if mods.contains(&M::ProtectedSet) {
            Some(MemberVis::Protected)
        } else {
            None
        }
    }

    pub(in crate::checker) fn of(mods: &[crate::ast::Modifier]) -> MemberVis {
        use crate::ast::Modifier;
        if mods.contains(&Modifier::Private) {
            MemberVis::Private
        } else if mods.contains(&Modifier::Protected) {
            MemberVis::Protected
        } else if mods.contains(&Modifier::Internal) {
            MemberVis::Internal
        } else {
            MemberVis::Public
        }
    }
}

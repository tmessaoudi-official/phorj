//! The declaration-visibility lattice (visibility modifiers): the package-hierarchy relation and
//! the `private`/`internal`/`public` violation check. Split out of `loader/mod.rs` (M-Decomp) to
//! keep the root under the file-size cap; `DefInfo` (the provenance record) stays in the root and
//! is reached here via `use super::*`. Re-exported by the root so the resolver / import modules
//! keep reaching `vis_violation`/`vis_word` through their own `use super::*`.

use super::*;

/// Q-B DV-1: the package HIERARCHY relation — is `ancestor` the same package as `descendant`, or a
/// dotted-prefix ANCESTOR of it? `Acme.App` is an ancestor-or-equal of `Acme.App` and `Acme.App.Sub`
/// (and `Acme.App.Sub.Deep`), but NOT of `Acme.AppX` (a prefix that does not end on a `.` boundary is
/// a different package) nor of `Acme` (that is the reverse direction). This is the single relation the
/// subtree-`internal` visibility rule is built on.
pub(super) fn pkg_is_ancestor_or_equal(ancestor: &str, descendant: &str) -> bool {
    descendant == ancestor
        || descendant
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// The visibility lattice check. `None` ⇒ the reference is legal; `Some(code)` ⇒ the diagnostic code
/// to report. Same file → always legal. `private` = this FILE only (any other file, even same package,
/// is `E-VIS-PRIVATE`). `internal` (Q-B DV-2) = the declaring package AND its descendant packages
/// (subtree, via [`pkg_is_ancestor_or_equal`]) — reaches DOWN the dotted hierarchy, never up or across
/// to a sibling. `public` = everywhere.
pub(super) fn vis_violation(
    info: &DefInfo,
    referrer_file: &Path,
    referrer_pkg: &str,
) -> Option<&'static str> {
    if info.file == referrer_file {
        return None;
    }
    match info.vis {
        Visibility::Public => None,
        // File-scoped: a different file is always a violation, regardless of package.
        Visibility::Private => Some("E-VIS-PRIVATE"),
        // Subtree-scoped: legal iff the referrer's package is the declaring package or a descendant.
        Visibility::Internal => {
            if pkg_is_ancestor_or_equal(&info.package, referrer_pkg) {
                None
            } else {
                Some("E-VIS-INTERNAL")
            }
        }
    }
}

/// Render the visibility keyword for a diagnostic.
pub(super) fn vis_word(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "public",
        Visibility::Internal => "internal",
        Visibility::Private => "private",
    }
}

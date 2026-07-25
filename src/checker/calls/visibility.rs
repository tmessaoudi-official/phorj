//! Member-visibility enforcement (Wave 1.1 / DEC-241 / Q-B DV-3) at access, write, and ctor sites.

use super::*;

impl Checker {
    /// Enforce member visibility (Wave 1.1) at an instance-member access site. `entry` is the
    /// member's `(visibility, declaring-owner)` (cloned out of the receiver class's `field_vis` /
    /// `method_vis`); `None` ⇒ no recorded visibility ⇒ public by construction (e.g. an interface
    /// method) ⇒ no-op. `private` is reachable only from inside the owner; `protected` from the owner
    /// and its subclasses (`cur_class` is the enclosing class, `None` in a free function). Mirrors the
    /// `const` check (`E-CONST-VISIBILITY`) so `interp ≡ VM ≡ transpiled PHP` all reject the same
    /// out-of-scope access — closing the documented byte-identity hole. `is_field` picks the code.
    /// DEC-241: enforce a member's asymmetric SET visibility at a WRITE site (`o.f = e`,
    /// `C.f = e`, a `with { f = … }` override). `entry` is the `set_vis`/`static_set_vis` row —
    /// absent means writes follow the ordinary read visibility (already enforced by the caller).
    /// Q-B DV-3: the package prefix of a mangled name (`Pkg\Sub\Name` → `Pkg\Sub`; a bare `Name` → "").
    /// The merged program mangles every non-`Main` definition to `Pkg\…\Name`, so this recovers the
    /// declaring package straight from the name the checker already holds — no loader plumbing.
    pub(in crate::checker) fn pkg_of_mangled(name: &str) -> &str {
        match name.rfind('\\') {
            Some(i) => &name[..i],
            None => "",
        }
    }

    /// Q-B DV-3: is `ancestor` the same package as `descendant`, or a `\`-boundary ancestor of it?
    /// The subtree relation `internal` member visibility uses (backslash-separated to match the mangled
    /// form). `"" ` is `Main`/loose — an ancestor only of itself, never of a namespaced package.
    pub(in crate::checker) fn pkg_subtree_contains(ancestor: &str, descendant: &str) -> bool {
        descendant == ancestor
            || descendant
                .strip_prefix(ancestor)
                .is_some_and(|r| r.starts_with('\\'))
    }

    /// Q-B DV-3: is an `internal` member of `owner` (a mangled class name) reachable from the code
    /// currently being checked? Legal iff the current package is the owner's package or a descendant.
    pub(in crate::checker) fn internal_member_visible(&self, owner: &str) -> bool {
        Self::pkg_subtree_contains(Self::pkg_of_mangled(owner), &self.cur_package)
    }

    pub(in crate::checker) fn enforce_set_vis(
        &mut self,
        entry: Option<(MemberVis, String)>,
        name: &str,
        span: Span,
    ) {
        let Some((vis, owner)) = entry else { return };
        let cur = self.cur_class.clone();
        let allowed = match vis {
            MemberVis::Public => true,
            MemberVis::Internal => self.internal_member_visible(&owner),
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if allowed {
            return;
        }
        let (visword, scope) = if vis == MemberVis::Private {
            ("private(set)", format!("inside `{owner}`"))
        } else {
            (
                "protected(set)",
                format!("inside `{owner}` and its subclasses"),
            )
        };
        self.err_coded(
            span,
            format!("field `{name}` is {visword} — assignable only {scope}"),
            "E-ASSIGN-SET-VISIBILITY",
            Some("read access is unaffected; assign it from within the owning scope, or widen the `(set)` modifier".into()),
        );
    }

    pub(in crate::checker) fn enforce_member_vis(
        &mut self,
        entry: Option<(MemberVis, String)>,
        name: &str,
        span: Span,
        is_field: bool,
    ) {
        let Some((vis, owner)) = entry else { return };
        if vis == MemberVis::Public {
            return;
        }
        let cur = self.cur_class.clone();
        let visible = match vis {
            MemberVis::Public => true,
            MemberVis::Internal => self.internal_member_visible(&owner),
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if visible {
            return;
        }
        let (kindword, code) = if is_field {
            ("field", "E-FIELD-VISIBILITY")
        } else {
            ("method", "E-METHOD-VISIBILITY")
        };
        let (visword, scope) = match vis {
            MemberVis::Private => ("private", format!("inside `{owner}`")),
            MemberVis::Protected => ("protected", format!("inside `{owner}` and its subclasses")),
            // Internal (public is early-returned above): package-subtree scope.
            _ => (
                "internal",
                format!("inside `{owner}`'s package and its sub-packages"),
            ),
        };
        let article = if visword == "internal" { "an" } else { "a" };
        self.err_coded(
            span,
            format!("`{name}` is {article} {visword} {kindword} of `{owner}`"),
            code,
            Some(format!("it is accessible only {scope}")),
        );
    }

    /// Enforce a constructor's visibility at a `new C(...)` site (Soundness Batch A — the 7th
    /// member-visibility access site). A `private` ctor is constructible only inside its declaring
    /// class (`cur_class == owner`); a `protected` ctor inside the declaring class or a subclass. The
    /// in-scope cases are the factory/singleton patterns (a static factory method or a static field
    /// initializer, both running in the class's scope). Public (the default) is always allowed.
    pub(in crate::checker) fn enforce_ctor_vis(&mut self, class_name: &str, span: Span) {
        let Some(info) = self.classes.get(class_name) else {
            return;
        };
        let vis = info.ctor_vis;
        if vis == MemberVis::Public {
            return;
        }
        let owner = info.ctor_owner.clone();
        let cur = self.cur_class.clone();
        let visible = match vis {
            MemberVis::Public => true,
            MemberVis::Internal => self.internal_member_visible(&owner),
            MemberVis::Private => cur.as_deref() == Some(owner.as_str()),
            MemberVis::Protected => cur.as_deref().is_some_and(|c| self.is_subtype(c, &owner)),
        };
        if visible {
            return;
        }
        let (visword, scope) = match vis {
            MemberVis::Private => ("private", format!("inside `{owner}`")),
            MemberVis::Protected => ("protected", format!("inside `{owner}` and its subclasses")),
            _ => (
                "internal",
                format!("inside `{owner}`'s package and its sub-packages"),
            ),
        };
        self.err_coded(
            span,
            format!("the constructor of `{class_name}` is {visword}"),
            "E-CTOR-VISIBILITY",
            Some(format!(
                "construct it only {scope} — e.g. a static factory method or a static field initializer"
            )),
        );
    }
}

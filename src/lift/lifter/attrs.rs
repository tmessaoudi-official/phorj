//! PHP lifter — **attributes** (LIFT-ATTR): a PHP `#[…]` becomes a phorj `#[…]`.
//!
//! The whole difficulty is the NAME. A PHP attribute name is a CLASS name, so it resolves through the
//! file's `use` map and `namespace` exactly like any other class reference — `#[Column]` means
//! `App\Column` inside `namespace App;`, `Doctrine\ORM\Mapping\Column` after
//! `use Doctrine\ORM\Mapping as ORM;` if written `#[ORM\Column]`, and root `Column` in a file with
//! neither.
//!
//! Resolving it and emitting the **fully-qualified** dotted path is not tidiness — it is the only
//! sound spelling. phorj recognizes a built-in attribute by segment-boundary SUFFIX match
//! ([`crate::ast::attr_path_matches`]): the canonical `Core.Http.Route` is matched by `Route`,
//! `Http.Route` and `Core.Http.Route` alike. So a Symfony controller's `#[Route("/home")]` lifted as a
//! bare `#[Route]` would bind to phorj's OWN routing attribute — a different class taking different
//! arguments — and check clean while meaning something else. Emitting
//! `#[Symfony.Component.Routing.Attribute.Route("/home")]` cannot be captured, because a written name
//! LONGER than a canonical path never matches it. This is the DEC-435 bug class (leaf-only resolution
//! collapsing distinct attributes) caught one layer up, in the direction that creates the names.
//!
//! ARGUMENTS are lifted verbatim — never rewritten, dropped or reordered. `#[Attribute(TARGET_CLASS)]`
//! therefore lifts to a phorj `#[Core.Runtime.Attribute(...)]` that the CHECKER rejects
//! (`E-ATTRIBUTE-ARGS`: target restriction is not implemented yet) instead of the lifter quietly
//! dropping the restriction, and `#[Deprecated(since: "8.4")]` fails on the unknown argument name
//! rather than losing it. A draft that fails `phg check` with a precise message is in-contract; a draft
//! that checks clean and means less than the PHP did is not (DEC-166).

use super::*;

/// What the file provides for resolving an attribute name to a class.
pub(in crate::lift::lifter) struct AttrCtx {
    /// `namespace A\B;` segments — empty when the file declares none (everything is root-relative).
    namespace: Vec<String>,
    /// One row per `use A\B\C [as D];`: the LOCAL name it binds → the fully-qualified segments.
    uses: Vec<(String, Vec<String>)>,
}

impl AttrCtx {
    pub(in crate::lift::lifter) fn new(prog: &php::PhpProgram) -> Self {
        let uses = prog
            .uses
            .iter()
            .filter_map(|u| {
                let local = u.alias.clone().or_else(|| u.path.last().cloned())?;
                Some((local, u.path.clone()))
            })
            .collect();
        AttrCtx {
            namespace: prog.namespace.clone(),
            uses,
        }
    }

    /// PHP's own class-name resolution, applied to an attribute name:
    ///
    /// * `\A\B` — root-qualified: the leading `\` is dropped and nothing else applies;
    /// * `X\…` where `X` is bound by a `use` — the `use` path replaces `X`;
    /// * anything else — relative to the current namespace.
    ///
    /// Returns the fully-qualified segments.
    fn resolve(&self, name: &str) -> Vec<String> {
        if let Some(rest) = name.strip_prefix('\\') {
            return rest.split('\\').map(str::to_string).collect();
        }
        let segs: Vec<&str> = name.split('\\').collect();
        if let Some((_, path)) = self.uses.iter().find(|(local, _)| local == segs[0]) {
            let mut out = path.clone();
            out.extend(segs[1..].iter().map(|s| (*s).to_string()));
            return out;
        }
        let mut out = self.namespace.clone();
        out.extend(segs.iter().map(|s| (*s).to_string()));
        out
    }

    /// Lift one PHP attribute. `Err` names the reason (DEC-166); the caller propagates it.
    pub(in crate::lift::lifter) fn lift_attribute(
        &self,
        a: &php::PhpAttribute,
    ) -> Result<crate::ast::Attribute, String> {
        let fqn = self.resolve(&a.name);
        let name = phorj_attr_path(&a.name, &fqn, &self.namespace)?;
        let mut args = Vec::with_capacity(a.args.len());
        for arg in &a.args {
            args.push(lift_expr(arg)?);
        }
        Ok(crate::ast::Attribute {
            name,
            args,
            span: SP,
        })
    }

    /// Every attribute on a declaration, in source order.
    pub(in crate::lift::lifter) fn lift_attributes(
        &self,
        attrs: &[php::PhpAttribute],
    ) -> Result<Vec<crate::ast::Attribute>, String> {
        attrs.iter().map(|a| self.lift_attribute(a)).collect()
    }
}

/// The `// CANNOT LIFT:` notes for every attribute naming a class this file does not declare — the
/// framework case: `#[ORM\Column]`'s class lives in `vendor/`, so the lifted draft names an attribute
/// nothing declares and `phg check` reports `E-UNKNOWN-ATTRIBUTE`.
///
/// The note exists because the draft must SAY what it could not do rather than leaving the reader to
/// work it out from a diagnostic at the bottom of the file — exactly the discipline
/// [`super::exceptions::unmapped_exception_classes`] already applies to unmapped exception classes. It
/// is not a refusal: the attribute IS emitted, with its own identity intact, so porting the class is all
/// that is left to do.
///
/// First-seen order, deduped (Invariant 10 — a lifted draft must not vary run to run).
pub(in crate::lift::lifter) fn unresolved_attribute_notes(prog: &php::PhpProgram) -> String {
    let ctx = AttrCtx::new(prog);
    let declared: Vec<&str> = prog
        .items
        .iter()
        .filter_map(|it| match it {
            php::PhpItem::Class(c) => Some(c.name.as_str()),
            _ => None,
        })
        .collect();
    let mut seen: Vec<String> = Vec::new();
    let all = prog.items.iter().flat_map(|it| match it {
        php::PhpItem::Function(f) => f.attrs.iter(),
        php::PhpItem::Class(c) => c.attrs.iter(),
        php::PhpItem::Enum(_) | php::PhpItem::Stmt(_) => [].iter(),
    });
    for a in all {
        // A name that failed to lift is already a hard refusal upstream — nothing to note here.
        let Ok(name) = ctx.lift_attribute(a).map(|lifted| lifted.name) else {
            continue;
        };
        let is_builtin = crate::ast::BUILTIN_ATTRIBUTE_PATHS
            .iter()
            .any(|(canonical, _)| crate::ast::attr_path_matches(&name, canonical));
        // A class declared in THIS file lifts to the bare leaf (see `phorj_attr_path`), so a bare name
        // matching a declared class resolves; anything else names something the file does not contain.
        if is_builtin || declared.contains(&name.as_str()) {
            continue;
        }
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen.iter()
        .map(|name| {
            format!(
                "// CANNOT LIFT: attribute `#[{name}]` names a class this file does not declare, so \
                 `phg check` reports `E-UNKNOWN-ATTRIBUTE`. Port the attribute class \
                 (`#[Attribute] class … {{ … }}`) or remove the attribute — the attribute itself was \
                 lifted with its own identity intact.\n"
            )
        })
        .collect()
}

/// PHP's two built-in attributes that phorj HAS a counterpart for, as (root PHP name, phorj canonical).
///
/// Both are the same concept under the same name, so mapping them is a rename, not an interpretation:
/// `#[\Attribute]` declares a class to BE an attribute in both languages, and `#[\Deprecated]` (PHP
/// 8.4) is phorj's `#[Deprecated]` (DEC-417). PHP's other built-ins (`Override`,
/// `AllowDynamicProperties`, `ReturnTypeWillChange`, `SensitiveParameter`, `NoDiscard`) have no phorj
/// counterpart and are deliberately absent: they pass through as ordinary names and the checker reports
/// `E-UNKNOWN-ATTRIBUTE`, which is a truthful "phorj has no such attribute" rather than a guess.
const PHP_BUILTIN_ATTRIBUTES: &[(&str, &str)] = &[
    ("Attribute", crate::ast::paths::ATTRIBUTE),
    ("Deprecated", crate::ast::paths::DEPRECATED),
];

/// A resolved PHP class path → the phorj attribute spelling.
///
/// Two spellings, and which one is correct is a property of phorj's own resolution, not a preference:
///
/// * **bare leaf** when the class belongs to the file's own package (or to the root, when the file
///   declares no namespace). This is how phorj refers to a same-package type, and it is the only
///   spelling that works in BOTH compile modes: a single-file compile keys the class BARE (`Tag`), a
///   project compile keys it package-mangled (`App\Meta\Tag`), and `attr_path_matches` accepts `Tag`
///   against either — while `App.Meta.Tag` is longer than the flat key and matches nothing.
///   [Verified: `phg check` on a one-file `package App.Meta;` fixture accepts `#[Tag]` and rejects
///   `#[App.Meta.Tag]` with `E-ATTR-TARGET`.]
/// * **fully qualified** for everything else — a class from another namespace. Never the bare leaf:
///   phorj matches a built-in attribute as a segment-boundary suffix, so a Symfony `#[Route]` lifted
///   bare would bind to `Core.Http.Route`.
///
/// `written` is the source form, used only in error messages so the reader sees what they wrote.
fn phorj_attr_path(written: &str, fqn: &[String], namespace: &[String]) -> Result<String, String> {
    let Some((leaf, packages)) = fqn.split_last() else {
        return Err("lift: an attribute with an empty name".to_string());
    };
    if packages.is_empty() {
        if let Some((_, canonical)) = PHP_BUILTIN_ATTRIBUTES.iter().find(|(php, _)| *php == leaf) {
            return Ok((*canonical).to_string());
        }
    }
    if packages.is_empty() || packages == namespace {
        return bare_attr_name(written, leaf);
    }
    let mut out = String::new();
    for seg in packages {
        // A leading segment is a PACKAGE segment on the phorj side, pascalized exactly as
        // `lift_package` and the `use`→`import` loop do it — otherwise the attribute path would not
        // match the class registry key those two produce.
        out.push_str(&super::package_segment(seg)?);
        out.push('.');
    }
    out.push_str(&super::type_segment(leaf)?);
    Ok(out)
}

/// The bare-leaf spelling, refused when the leaf is one of phorj's own built-in attribute names.
///
/// There is no longer path to disambiguate with here — the class is in this file's package (or the
/// root), so `#[Leaf]` is the only spelling that resolves, and phorj's suffix match binds the BUILT-IN
/// for those eleven names. Emitting it anyway would produce a program that checks clean and means
/// something else than the PHP did, which is the one outcome the lifter must never produce (DEC-166).
/// Qualifying it instead (`App.Route`) is not a fix: that spelling resolves only under a project
/// compile and is `E-ATTR-TARGET` in the flat single-file draft `phg lift` actually emits.
fn bare_attr_name(written: &str, leaf: &str) -> Result<String, String> {
    if let Some((canonical, _)) = crate::ast::BUILTIN_ATTRIBUTE_PATHS
        .iter()
        .find(|(canonical, _)| crate::ast::attr_path_leaf(canonical) == leaf)
    {
        return Err(format!(
            "lift: `#[{written}]` has the same name as phorj's built-in `{canonical}` attribute, and \
             phorj resolves an unqualified attribute name to the built-in — so the lifted program \
             would mean something different from the PHP it came from. Rename the class, or move it \
             to a namespace this file only `use`s."
        ));
    }
    super::type_segment(leaf)
}

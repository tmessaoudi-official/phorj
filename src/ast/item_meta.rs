//! Per-`Item` metadata used by doc-comment plumbing (DEC-419): a top-level declaration's SPAN (what
//! the transpiler keys on) and its NAME (what the lifter keys on).
//!
//! Its own module rather than an addition to `class_hierarchy.rs`, which the size gate showed was one
//! edit from the Invariant-13 hard cap. Both matches are wildcard-free on purpose — a new `Item`
//! variant must decide whether it is documentable instead of inheriting `None` and losing its docs
//! silently (Invariant 3's rule, applied to `Item`).

use super::Item;

/// The span of a top-level item's DECLARATION (its keyword/name), or `None` for items that declare no
/// named entity (`import`, `test`).
///
/// Exists so doc-comment lookup (DEC-419) has ONE place that knows which items can carry documentation.
/// The match is exhaustive on purpose — a new `Item` variant must decide whether it is documentable
/// rather than inheriting `None` from a wildcard and losing its docs in silence (Invariant 3's rule
/// applied to `Item`).
pub fn item_decl_span(item: &Item) -> Option<crate::token::Span> {
    match item {
        Item::Function(f) => Some(f.span),
        Item::Class(c) => Some(c.span),
        Item::Enum(e) => Some(e.span),
        Item::Interface(i) => Some(i.span),
        Item::Trait(t) => Some(t.span),
        Item::TypeAlias { span, .. } => Some(*span),
        // An `import` binds a module qualifier and a `test` is checker-gated out of every build —
        // neither is a documentable declaration.
        Item::Import { .. } | Item::Test { .. } => None,
    }
}

/// The declared NAME of a top-level item, or `None` for items that name nothing (`import`, `test`).
///
/// The name-keyed twin of [`item_decl_span`]: the lifter carries PHPDoc by name (a lifted PHP program
/// has no phorj spans to key on), while the transpiler carries it by span. Exhaustive for the same
/// reason — a new `Item` variant must decide, not inherit `None` from a wildcard.
pub fn item_decl_name(item: &Item) -> Option<&str> {
    match item {
        Item::Function(f) => Some(&f.name),
        Item::Class(c) => Some(&c.name),
        Item::Enum(e) => Some(&e.name),
        Item::Interface(i) => Some(&i.name),
        Item::Trait(t) => Some(&t.name),
        Item::TypeAlias { name, .. } => Some(name),
        Item::Import { .. } | Item::Test { .. } => None,
    }
}

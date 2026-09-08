//! DEC-504 — the field labels of a **named-field tuple**: `(bp: int, source: string)`.
//!
//! A named tuple is the SAME construct as a positional one, wearing names. It is checked as a
//! [`crate::types::Ty::Tuple`] and erased to a plain `List` before any backend (Invariant 5), so
//! the labels are purely a front-end fact: they never reach the interpreter, the VM, the JIT or
//! the transpiler. The developer ruled the PHP leg POSITIONAL (2026-09-08), so `(bp: 3, source:
//! "x")` transpiles to `[3, 'x']` and `t.bp` to `$t[0]`, exactly as a positional tuple already does.
//!
//! **The labels need no backend changes; the POSITIONS do.** The value form is erased, but the TYPE
//! form (`Type::Tuple`) deliberately survives into the backends, and a tuple's positions have
//! DIFFERENT types where a list's element type is one and homogeneous. So the operand lattices on
//! both sides — the VM compiler's [`crate::compiler::CTy`] and the transpiler's `OpKind` — carry a
//! per-position tuple shape. Resolving a tuple as a list there gives every position the type of
//! position 0: the interpreter reads an `int`, the VM refuses to infer a numeric type, and the PHP
//! leg picks `.` over `+`. That is the Invariant-7 CTy-operand trap, and it went live the moment
//! `t.bp` — and so `t[1]` — became writable.
//!
//! **Field ORDER IS PART OF THE TYPE** (same ruling). `(a: int, b: string)` and
//! `(b: string, a: int)` are different types, neither assignable to the other. This is not a
//! preference: erasure is positional, so the field's index IS its written position. Making the
//! order insignificant would mean canonicalizing at erasure, which would divorce the order the
//! field expressions are EVALUATED in from the order they are stored in — a byte-identity
//! surface. Order-significance also carries the soundness of extending `<=>` to named tuples: it
//! keeps DEC-512's static-arity argument true position-by-position.
//!
//! Labels are `None` for an ordinary positional tuple, so a tuple-free (or label-free) program is
//! byte-identical to before and pays no allocation.

use crate::token::Span;

/// One field label, with the span of the NAME alone — the span is what lets a duplicate-label or
/// unknown-field diagnostic underline the offending name rather than the whole tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub name: String,
    pub span: Span,
}

/// The labels of a tuple *expression* or *type*, in field order: `None` for a positional tuple,
/// `Some(v)` for a named one with `v.len()` equal to the tuple's arity.
///
/// Boxed behind an `Option` so the positional case — every tuple in the language before this slice
/// — stays a null pointer's worth of payload on the `Expr`/`Type` node.
pub type TupleLabels = Option<Box<Vec<Label>>>;

/// The labels as the CHECKER carries them on [`crate::types::Ty::Tuple`]: names only, no spans.
///
/// `Ty` is cloned constantly during inference and compared by `PartialEq` for type identity, so
/// spans must not ride along — two identical types written in two places would otherwise compare
/// unequal. Dropping them here is what makes "order is part of the type" a plain `==`.
pub type TyLabels = Option<Box<Vec<String>>>;

/// The names of `labels`, or `None` for a positional tuple — the `Expr`/`Type` → `Ty` projection.
pub fn to_ty_labels(labels: &TupleLabels) -> TyLabels {
    labels
        .as_ref()
        .map(|ls| Box::new(ls.iter().map(|l| l.name.clone()).collect()))
}

/// The `Ty` → AST projection: rebuild `Expr`/`Type` labels from the checker's name-only form, all
/// sharing `sp`. The inverse of [`to_ty_labels`], for the passes that MATERIALIZE an inferred type
/// back into the tree (`checker::materialize_inferred_types`).
///
/// One span for every label is correct and not a shortcut: a materialized type was never written by
/// the user, so there is no name to underline. `sp` is the node the type is being written onto, which
/// is where any diagnostic about it should point.
pub fn to_ast_labels(labels: Option<&Vec<String>>, sp: Span) -> TupleLabels {
    labels.map(|ls| {
        Box::new(
            ls.iter()
                .map(|n| Label {
                    name: n.clone(),
                    span: sp,
                })
                .collect(),
        )
    })
}

/// Render a tuple type's members for `Display`: `(int, string)` positional, `(bp: int, source:
/// string)` named. Lives here rather than inline in `types.rs` so the two spellings cannot drift,
/// and so diagnostics, LSP hover and `phg explain` all read a tuple type the same way.
pub fn render_members<T: std::fmt::Display>(
    f: &mut std::fmt::Formatter<'_>,
    members: &[T],
    labels: Option<&Vec<String>>,
) -> std::fmt::Result {
    write!(f, "(")?;
    for (i, m) in members.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        match labels.and_then(|ls| ls.get(i)) {
            Some(name) => write!(f, "{name}: {m}")?,
            None => write!(f, "{m}")?,
        }
    }
    write!(f, ")")
}

/// The index of `field` in `labels`, or `None` when the tuple is positional or has no such field.
///
/// This is the whole of `t.bp` → `Index(t, 1)`: the checker resolves the name to a position and
/// the erasure rewrites the access, so no backend ever learns that fields have names.
pub fn field_index(labels: Option<&Vec<String>>, field: &str) -> Option<usize> {
    labels?.iter().position(|n| n == field)
}

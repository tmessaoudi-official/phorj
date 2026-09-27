//! Row 5d (6C) — a closure body is a shape scope: the three function-scoped maps of `shapes.rs` are
//! snapshotted before it and restored after, so a `@var` registered inside a closure never answers
//! outside it. Split out of `shapes.rs` (Invariant 13, row 4p).

use super::*;

/// Both maps as they stood before a closure body — restored by [`leave_closure`] (row 5d, 6C).
pub(in crate::lift::lifter) struct ClosureScope {
    tuple: std::collections::HashMap<String, Vec<String>>,
    elem: std::collections::HashMap<String, Vec<String>>,
    colls: std::collections::HashMap<String, (&'static str, bool)>,
}

/// Snapshot both maps before lifting a closure body. A SNAPSHOT, not a clear: a by-value `use`
/// capture is the same variable with the same shape inside the closure.
pub(in crate::lift::lifter) fn enter_closure() -> ClosureScope {
    ClosureScope {
        tuple: TUPLE_FIELDS.with(|m| m.borrow().clone()),
        elem: ELEM_FIELDS.with(|m| m.borrow().clone()),
        colls: COLL_VARS.with(|m| m.borrow().clone()),
    }
}

/// Put the enclosing function's maps back, dropping whatever the closure body registered. Called on
/// both exits, like [`leave_binder`].
pub(in crate::lift::lifter) fn leave_closure(scope: ClosureScope) {
    TUPLE_FIELDS.with(|m| *m.borrow_mut() = scope.tuple);
    ELEM_FIELDS.with(|m| *m.borrow_mut() = scope.elem);
    COLL_VARS.with(|m| *m.borrow_mut() = scope.colls);
}

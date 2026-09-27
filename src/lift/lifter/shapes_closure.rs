//! Row 5d (6C) — a closure body is a shape scope: the three function-scoped maps of `shapes.rs` are
//! snapshotted before it and restored after, so a `@var` registered inside a closure never answers
//! outside it. Split out of `shapes.rs` (Invariant 13, row 4p). FILE scope is the same kind of scope
//! (row 4p, 6C): [`swap_scope`] installs the file's own maps around each top-level statement.

use super::*;

/// Both maps as they stood before a closure body — restored by [`leave_closure`] (row 5d, 6C).
/// `Default` is the empty scope a file starts with.
#[derive(Default)]
pub(in crate::lift::lifter) struct ClosureScope {
    tuple: std::collections::HashMap<String, Vec<String>>,
    elem: std::collections::HashMap<String, Vec<String>>,
    colls: std::collections::HashMap<String, (&'static str, bool)>,
    /// Row 4l-b2: the variables holding `preg_match` captures.
    #[cfg(feature = "regex")]
    captures: super::preg::CapturesScope,
}

/// Snapshot both maps before lifting a closure body. A SNAPSHOT, not a clear: a by-value `use`
/// capture is the same variable with the same shape inside the closure.
pub(in crate::lift::lifter) fn enter_closure() -> ClosureScope {
    ClosureScope {
        tuple: TUPLE_FIELDS.with(|m| m.borrow().clone()),
        elem: ELEM_FIELDS.with(|m| m.borrow().clone()),
        colls: COLL_VARS.with(|m| m.borrow().clone()),
        #[cfg(feature = "regex")]
        captures: super::preg::snapshot_captures(),
    }
}

/// Put the enclosing function's maps back, dropping whatever the closure body registered. Called on
/// both exits, like [`leave_binder`].
pub(in crate::lift::lifter) fn leave_closure(scope: ClosureScope) {
    TUPLE_FIELDS.with(|m| *m.borrow_mut() = scope.tuple);
    ELEM_FIELDS.with(|m| *m.borrow_mut() = scope.elem);
    COLL_VARS.with(|m| *m.borrow_mut() = scope.colls);
    #[cfg(feature = "regex")]
    super::preg::restore_captures(scope.captures);
}

/// Install `scope` and hand back what it replaced — the file-scope maps swapped in around one
/// top-level statement, so a function lifted between two statements neither leaks its shapes into
/// file scope nor clears a file-scope `@var` (row 4p, 6C).
pub(in crate::lift::lifter) fn swap_scope(scope: ClosureScope) -> ClosureScope {
    let old = enter_closure();
    leave_closure(scope);
    old
}

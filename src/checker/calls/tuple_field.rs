//! DEC-504 — `t.bp` on a named tuple: resolve the field NAME to a POSITION.
//!
//! This is the whole of named-field access. A named tuple's runtime representation is the same
//! positional list every tuple erases to (Invariant 5), so a field read is an ordinary index read
//! once its position is known — and the position is a compile-time fact, because field order is
//! part of the type (developer ruling, 2026-09-08).
//!
//! The resolution is RECORDED here, keyed by the `Member` expression's `span.start`, and written
//! into the AST later by `rewrite_tuple_fields` — the same record-then-materialize shape
//! `materialize_tuple_binds` uses for destructuring binders. Two reasons it is not rewritten in
//! place at check time: the checker must not mutate the tree it is still walking, and a rewrite
//! built from a check-time clone goes stale when a later pass edits that subtree (the clone-
//! staleness class that produced three real bugs on 2026-07-16).
//!
//! Safe navigation (`t?.bp`) is REFUSED. The rewrite turns a field read into an `Expr::Index`,
//! which carries no `safe` flag, so a `?.` would silently erase to an ordinary index — the checker
//! would promise `int?` (null is possible) while the runtime faulted on a null receiver instead of
//! short-circuiting. Rather than lower it to a conditional or grow a safe-index Op for a construct
//! that cannot be null AT a position anyway, the access is refused by name and the user narrows
//! first (`if (var u = t) { u.bp }`).
//!
//! A miss is refused BY NAME. `t.nope` names the fields the tuple actually has, and `t.bp` on a
//! POSITIONAL tuple says that the tuple has no field names at all rather than reporting an unknown
//! field — the two are different mistakes and a reader needs to know which one they made.

use super::*;

impl Checker {
    /// Type `t.name` where `t` is a tuple. Returns `None` when the receiver is not a tuple, so the
    /// caller falls through to the class/enum paths unchanged.
    pub(in crate::checker) fn check_tuple_field(
        &mut self,
        base: &Ty,
        name: &str,
        safe: bool,
        span: Span,
    ) -> Option<Ty> {
        let Ty::Tuple(elems, labels) = base else {
            return None;
        };
        if safe {
            return Some(self.err_coded(
                span,
                format!("`?.` is not supported on a tuple — `{base}` has no nullable field `{name}`"),
                "E-TUPLE-SAFE-FIELD",
                Some(
                    "a tuple field read erases to an index, which has no null short-circuit; narrow \
                     the receiver first — `if (var u = t) { u.bp }` — then read the field with `.`"
                        .into(),
                ),
            ));
        }
        let Some(ls) = labels else {
            return Some(self.err_coded(
                span,
                format!("`{base}` is a positional tuple, so it has no field `{name}`"),
                "E-TUPLE-POSITIONAL-FIELD",
                Some(
                    "name the fields in the type — `(bp: int, source: string)` — or destructure it \
                     with `var (a, b) = t`"
                        .into(),
                ),
            ));
        };
        match crate::ast::field_index(Some(ls), name) {
            Some(i) => {
                self.tuple_field_indices.insert(span.start, i);
                Some(elems[i].clone())
            }
            None => Some(self.err_coded(
                span,
                format!("`{base}` has no field `{name}`"),
                "E-TUPLE-UNKNOWN-FIELD",
                Some(format!("its fields are {}", ls.join(", "))),
            )),
        }
    }
}
